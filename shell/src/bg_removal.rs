//! Background removal as a re-runnable timeline operation.
//!
//! Removal left the animation pipeline (see
//! `docs/native-ui-animation-pipeline.md`) and became a `DocumentStore` op
//! over a cel or a whole layer, recorded on the undo history like any pixel
//! edit. It tries keying first — fast, free, deterministic — and falls back to
//! the AI `BACKGROUND_REMOVAL` backend, which the dithered Seedance backgrounds
//! usually need. Because it works on timeline cels it also works on hand-drawn
//! frames, and it can be re-run with different settings until the result is right.

use std::io::Cursor;

use eframe::egui;
use pixhaus_core::canvas::PixelBuffer;
use pixhaus_core::project::PixelBufferId;
use pixhaus_core::transforms::normalize::{ChromaKey, KeyOutcome, chroma_key, detect_key_color, judge_key};

use crate::app::{JobStatus, ShellApp};

/// Alpha cutoff used when judging keyer success — matches the pipeline default.
const ALPHA_THRESHOLD: u8 = 8;

impl ShellApp {
    /// The background-removal panel: key-colour swatch, detect / eyedrop, the
    /// tolerance slider, live preview, the keying button, and the AI fallback.
    /// Lives wherever frame editing is offered — drawn inside the timeline body.
    pub(crate) fn background_removal_panel(&mut self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new("Background removal").id_salt("bg_removal").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("Key");
                let mut col = crate::editor::to_color32(self.bg_key_color);
                if ui.color_edit_button_srgba(&mut col).changed() {
                    self.bg_key_color = crate::editor::from_color32(col);
                    self.refresh_bg_preview();
                }
                if ui
                    .button("Detect")
                    .on_hover_text("Sample the frame's border for the background colour")
                    .clicked()
                {
                    self.detect_bg_key();
                }
                if ui
                    .button("Use FG")
                    .on_hover_text("Use the foreground colour (eyedrop it with the picker tool first)")
                    .clicked()
                {
                    self.bg_key_color = self.editor.fg;
                    self.refresh_bg_preview();
                }
            });

            if ui.add(egui::Slider::new(&mut self.bg_tolerance, 0..=128).text("tolerance")).changed() {
                self.refresh_bg_preview();
            }

            ui.horizontal(|ui| {
                let mut preview = self.bg_preview;
                if ui
                    .checkbox(&mut preview, "Preview")
                    .on_hover_text("Show the keyed result on the canvas")
                    .changed()
                {
                    self.set_bg_preview(preview);
                }
                ui.checkbox(&mut self.bg_all_frames, "All frames")
                    .on_hover_text("Apply to every frame's cel on the active layer");
            });

            let has_target = self.doc.active_buffer_id().is_some();
            let running = !self.bg_remove_pending.is_empty();
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(has_target, egui::Button::new("Remove (key)"))
                    .on_hover_text("Chroma-key — fast, free, deterministic, and undoable")
                    .clicked()
                {
                    self.key_background_now();
                }
                if ui
                    .add_enabled(has_target && self.backend_ready && !running, egui::Button::new("Remove with AI"))
                    .on_hover_text("Background-removal backend — the reliable fallback")
                    .clicked()
                {
                    if let Some(id) = self.doc.active_buffer_id() {
                        self.ai_remove_buffers(vec![id]);
                    }
                }
            });

            if !self.bg_flagged.is_empty() {
                let n = self.bg_flagged.len();
                ui.colored_label(
                    egui::Color32::LIGHT_YELLOW,
                    format!("Keying looks wrong on {n} frame(s) — too little or too much removed."),
                );
                if ui
                    .add_enabled(self.backend_ready && !running, egui::Button::new(format!("Remove {n} flagged with AI")))
                    .clicked()
                {
                    let flagged = std::mem::take(&mut self.bg_flagged);
                    self.ai_remove_buffers(flagged);
                }
            }

            match &self.bg_status {
                JobStatus::Running(msg) => {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(msg);
                    });
                }
                JobStatus::Failed(err) => {
                    ui.colored_label(egui::Color32::LIGHT_RED, format!("AI removal failed: {err}"));
                }
                JobStatus::Idle => {}
            }
        });
    }

    /// Keys the background out of the target cel(s) with the current colour and
    /// tolerance, recording one undo entry per cel, and flags the ones the
    /// keyer judged likely failures.
    pub(crate) fn key_background_now(&mut self) {
        let key = ChromaKey::new(self.bg_key_color, self.bg_tolerance);
        let targets = self.bg_targets();
        if targets.is_empty() {
            return;
        }
        let mut flagged = Vec::new();
        for buffer_id in targets {
            let Some(before) = self.doc.chroma_key_buffer(buffer_id, key) else {
                continue;
            };
            if let Some(after) = self.doc.pixel_buffers.get(&buffer_id) {
                if judge_key(&before, after, ALPHA_THRESHOLD) != KeyOutcome::Ok {
                    flagged.push(buffer_id);
                }
            }
            let (w, h) = (before.width(), before.height());
            self.finish_pixel_edit(&before, buffer_id, 0, 0, w, h, "Remove background");
        }
        self.bg_flagged = flagged;
        self.set_bg_preview(false);
        self.refresh_canvas(false);
    }

    /// Applies a returned AI removal to its cel, recorded as one undo entry.
    /// Called from the channel pump when [`crate::app::ShellMsg::BgRemovalDone`]
    /// arrives.
    pub(crate) fn apply_ai_bg_removal(&mut self, buffer_id: PixelBufferId, png: &[u8]) {
        self.bg_remove_pending.remove(&buffer_id);
        self.bg_flagged.retain(|&b| b != buffer_id);
        let Some(stripped) = decode_png_buffer(png) else {
            self.bg_status = JobStatus::Failed("could not decode the AI result".into());
            return;
        };
        let Some(before) = self.doc.replace_buffer(buffer_id, stripped) else {
            self.bg_status = JobStatus::Failed("AI result size did not match the cel".into());
            return;
        };
        let (w, h) = (before.width(), before.height());
        self.finish_pixel_edit(&before, buffer_id, 0, 0, w, h, "Remove background (AI)");
        if self.bg_remove_pending.is_empty() {
            self.bg_status = JobStatus::Idle;
        }
        self.refresh_canvas(false);
    }

    /// Auto-detects the key colour from the active frame's border and seeds the
    /// swatch.
    fn detect_bg_key(&mut self) {
        if let Some(frame) = self.doc.composite_active_frame() {
            if let Some(color) = detect_key_color(&frame) {
                self.bg_key_color = color;
                self.refresh_bg_preview();
            }
        }
    }

    /// Spawns an AI background removal for each cel in `buffers`.
    fn ai_remove_buffers(&mut self, buffers: Vec<PixelBufferId>) {
        for buffer_id in buffers {
            let Some(png) = self.doc.pixel_buffers.get(&buffer_id).and_then(encode_buffer_png) else {
                continue;
            };
            self.spawn_ai_bg_removal(buffer_id, png);
        }
    }

    /// The cel buffers the op applies to: every frame's active-layer cel when
    /// "All frames" is on, else just the active cel.
    fn bg_targets(&self) -> Vec<PixelBufferId> {
        if self.bg_all_frames {
            self.doc.active_layer_frame_buffers()
        } else {
            self.doc.active_buffer_id().into_iter().collect()
        }
    }

    /// Turns the live preview on (uploading the keyed frame) or off (restoring
    /// the real frame).
    fn set_bg_preview(&mut self, on: bool) {
        self.bg_preview = on;
        if on {
            self.update_bg_preview();
        } else {
            self.refresh_canvas(false);
        }
    }

    /// Re-uploads the keyed preview when one is active (after a control change).
    fn refresh_bg_preview(&mut self) {
        if self.bg_preview {
            self.update_bg_preview();
        }
    }

    /// Keys the active frame's composite with the current settings and uploads
    /// it as a view-only preview.
    fn update_bg_preview(&mut self) {
        let key = ChromaKey::new(self.bg_key_color, self.bg_tolerance);
        if let Some(frame) = self.doc.composite_active_frame() {
            let keyed = chroma_key(&frame, key);
            self.upload_frame(&keyed, false);
        }
    }
}

/// Encodes a [`PixelBuffer`] to PNG bytes, dropping any row padding. `None` on
/// an encode failure.
fn encode_buffer_png(buf: &PixelBuffer) -> Option<Vec<u8>> {
    let (w, h) = (buf.width(), buf.height());
    let row_bytes = (w * 4) as usize;
    let mut tight = Vec::with_capacity(row_bytes * h as usize);
    for y in 0..h {
        let row = buf.row(y)?;
        tight.extend_from_slice(&row[..row_bytes]);
    }
    let img: image::RgbaImage = image::ImageBuffer::from_raw(w, h, tight)?;
    let mut out = Vec::new();
    image::DynamicImage::ImageRgba8(img)
        .write_to(&mut Cursor::new(&mut out), image::ImageFormat::Png)
        .ok()?;
    Some(out)
}

/// Decodes PNG bytes into a tightly packed RGBA [`PixelBuffer`].
fn decode_png_buffer(png: &[u8]) -> Option<PixelBuffer> {
    let rgba = image::load_from_memory(png).ok()?.to_rgba8();
    let (w, h) = (rgba.width(), rgba.height());
    PixelBuffer::from_raw(w, h, w * 4, rgba.into_raw()).ok()
}
