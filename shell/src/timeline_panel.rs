//! The interactive timeline: a cel matrix (rows = layers, columns = frames),
//! transport controls, frame operations, frame tags, cel linking, and the
//! onion-skin controls.
//!
//! The cel-matrix layout and cel linking are adopted from Pixelorama; the
//! onion-skin model from `OpenToonz`. See `THIRD_PARTY_NOTICES.md`.

use eframe::egui;
use pixhaus_core::project::{Cel, CelData, Frame, FrameIndex, FrameRange, FrameTag, LayerId, LoopDirection, PixelBufferId};

use pixhaus_core::canvas::PixelBuffer;

use crate::app::ShellApp;
use crate::commands::{push_sprite_edit, push_sprite_edit_with_buffers};
use crate::icons;

impl ShellApp {
    /// Draws the collapsible timeline dock: a slim transport line is always
    /// visible; the full cel matrix and frame tools show only when expanded.
    pub(crate) fn timeline_dock(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let caret = if self.timeline_expanded { icons::DOWN } else { icons::UP };
            let hint = if self.timeline_expanded { "Collapse timeline" } else { "Expand timeline" };
            if ui.button(caret).on_hover_text(hint).clicked() {
                self.timeline_expanded = !self.timeline_expanded;
            }
            ui.separator();
            self.transport_controls(ui);
        });
        if self.timeline_expanded {
            ui.separator();
            self.timeline_body(ui);
        }
    }

    /// Transport: play/pause, stop, step, and the frame counter — always shown.
    fn transport_controls(&mut self, ui: &mut egui::Ui) {
        let play = if self.playing { icons::PAUSE } else { icons::PLAY };
        if ui.button(play).on_hover_text("Play / pause").clicked() {
            self.toggle_play();
        }
        if ui.button(icons::STOP).on_hover_text("Stop").clicked() {
            self.playing = false;
            self.doc.active_frame = FrameIndex::new(0);
            self.refresh_canvas(false);
        }
        if ui.button(icons::PREV).on_hover_text("Previous frame").clicked() {
            self.step_frame(-1);
        }
        if ui.button(icons::NEXT).on_hover_text("Next frame").clicked() {
            self.step_frame(1);
        }
        ui.label(format!("{} / {}", self.doc.active_frame.get() + 1, self.doc.frame_count().max(1)));
    }

    /// The expanded body: loop mode, duration, frame ops, tags, onion, and the
    /// cel matrix.
    #[allow(clippy::too_many_lines, clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn timeline_body(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            // Loop direction of the first tag (or a default applied to new tags).
            let mut dir = self
                .doc
                .active_sprite()
                .and_then(|s| s.frame_tags.first())
                .map_or(LoopDirection::Forward, |t| t.loop_direction);
            egui::ComboBox::from_id_salt("loop_dir").selected_text(loop_label(dir)).show_ui(ui, |ui| {
                let before = dir;
                ui.selectable_value(&mut dir, LoopDirection::Forward, "Forward");
                ui.selectable_value(&mut dir, LoopDirection::Reverse, "Reverse");
                ui.selectable_value(&mut dir, LoopDirection::PingPong, "Ping-pong");
                if dir != before {
                    push_sprite_edit(&mut self.editor, &mut self.doc, "Loop mode", |sprite| {
                        if let Some(t) = sprite.frame_tags.first_mut() {
                            t.loop_direction = dir;
                        }
                    });
                }
            });

            ui.separator();
            // Per-frame duration of the active frame.
            let active = self.doc.active_frame;
            let mut dur = self
                .doc
                .active_sprite()
                .and_then(|s| s.frames.get(active.get() as usize))
                .map_or(100u32, |f| f.duration_ms);
            if ui.add(egui::Slider::new(&mut dur, 16..=1000).text("ms")).changed() {
                push_sprite_edit(&mut self.editor, &mut self.doc, "Frame duration", |sprite| {
                    if let Some(f) = sprite.frames.get_mut(active.get() as usize) {
                        f.duration_ms = dur;
                    }
                });
            }
            ui.label(format!("{:.0} fps", 1000.0 / f64::from(dur.max(1))));
        });

        ui.horizontal_wrapped(|ui| {
            if ui.button(format!("{} Frame", icons::ADD)).on_hover_text("Add empty frame").clicked() {
                self.add_frame();
            }
            if ui.button("Dup").on_hover_text("Duplicate frame").clicked() {
                self.duplicate_frame();
            }
            if ui.button(format!("{} Frame", icons::TRASH)).on_hover_text("Delete frame").clicked() {
                self.delete_frame();
            }
            ui.separator();
            if ui.button(format!("{} Link", icons::LINK)).on_hover_text("Link to previous frame").clicked() {
                self.link_to_previous();
            }
            if ui.button(format!("{} Unlink", icons::UNLINK)).on_hover_text("Break the link (copy)").clicked() {
                self.unlink_cel();
            }
        });

        ui.horizontal_wrapped(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.editor.new_tag_name)
                    .hint_text("tag name")
                    .desired_width(90.0),
            );
            if ui.button("Tag frames").on_hover_text("Tag the whole range").clicked() {
                self.add_tag();
            }
            // Existing tags as removable chips.
            let tags: Vec<(usize, String)> = self
                .doc
                .active_sprite()
                .map(|s| s.frame_tags.iter().enumerate().map(|(i, t)| (i, t.name.clone())).collect())
                .unwrap_or_default();
            let mut remove: Option<usize> = None;
            for (i, name) in tags {
                if ui.small_button(format!("{name} ×")).on_hover_text("Remove tag").clicked() {
                    remove = Some(i);
                }
            }
            if let Some(i) = remove {
                push_sprite_edit(&mut self.editor, &mut self.doc, "Remove tag", |sprite| {
                    if i < sprite.frame_tags.len() {
                        sprite.frame_tags.remove(i);
                    }
                });
            }
        });

        // Onion-skin controls.
        ui.horizontal_wrapped(|ui| {
            let mut onion_changed = ui.checkbox(&mut self.editor.onion.enabled, "Onion").changed();
            ui.add_enabled_ui(self.editor.onion.enabled, |ui| {
                onion_changed |= ui.add(egui::Slider::new(&mut self.editor.onion.prev, 0..=4).text("prev")).changed();
                onion_changed |= ui.add(egui::Slider::new(&mut self.editor.onion.next, 0..=4).text("next")).changed();
                onion_changed |= ui.add(egui::Slider::new(&mut self.editor.onion.opacity, 0.05..=1.0).text("opacity")).changed();
            });
            if onion_changed {
                self.refresh_canvas(false);
            }
        });

        ui.separator();
        self.background_removal_panel(ui);

        ui.separator();
        self.cel_matrix(ui);
    }

    /// The cel grid: layer rows (top-first) by frame columns.
    #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
    fn cel_matrix(&mut self, ui: &mut egui::Ui) {
        let Some(sprite) = self.doc.active_sprite() else {
            ui.label("No sprite.");
            return;
        };
        let frame_count = sprite.frames.len();
        let layers: Vec<(LayerId, String)> = sprite.layers.iter().rev().map(|l| (l.id, l.name.clone())).collect();
        // For each (layer, frame): (has_content, is_linked).
        let mut cells: Vec<Vec<(bool, bool)>> = Vec::with_capacity(layers.len());
        for (lid, _) in &layers {
            let mut row = Vec::with_capacity(frame_count);
            for f in 0..frame_count {
                let frame = FrameIndex::new(f as u32);
                let cel = sprite.cel(*lid, frame);
                let has = cel.is_some();
                let linked = matches!(cel.map(|c| &c.data), Some(CelData::Linked { .. }));
                row.push((has, linked));
            }
            cells.push(row);
        }
        // Tag spans for the header strip.
        let tag_spans: Vec<(u32, u32, egui::Color32)> = sprite
            .frame_tags
            .iter()
            .enumerate()
            .map(|(i, t)| (t.range.start.get(), t.range.end.get(), tag_color(i)))
            .collect();

        let active_frame = self.doc.active_frame.get();
        let active_layer = self.doc.active_layer;
        let cs = self.editor.cel_size.clamp(20.0, 96.0);

        let mut pick: Option<(LayerId, FrameIndex)> = None;

        egui::ScrollArea::both().id_salt("cel_matrix").show(ui, |ui| {
            // Header: frame numbers with tag colour underline.
            ui.horizontal(|ui| {
                ui.allocate_exact_size(egui::vec2(96.0, cs * 0.6), egui::Sense::hover());
                for f in 0..frame_count {
                    let (rect, resp) = ui.allocate_exact_size(egui::vec2(cs, cs * 0.6), egui::Sense::click());
                    let is_active = f as u32 == active_frame;
                    let bg = if is_active {
                        egui::Color32::from_rgb(70, 110, 170)
                    } else {
                        egui::Color32::from_gray(45)
                    };
                    ui.painter().rect_filled(rect, 2.0, bg);
                    ui.painter().text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        f.to_string(),
                        egui::FontId::proportional(11.0),
                        egui::Color32::WHITE,
                    );
                    // Tag underline.
                    for (s, e, col) in &tag_spans {
                        if (f as u32) >= *s && (f as u32) <= *e {
                            let bar = egui::Rect::from_min_size(rect.left_bottom() - egui::vec2(0.0, 3.0), egui::vec2(rect.width(), 3.0));
                            ui.painter().rect_filled(bar, 0.0, *col);
                        }
                    }
                    if resp.clicked() {
                        pick = Some((active_layer.unwrap_or(LayerId::new(0)), FrameIndex::new(f as u32)));
                    }
                }
            });

            // Layer rows.
            for (li, (lid, name)) in layers.iter().enumerate() {
                ui.horizontal(|ui| {
                    let (lrect, _) = ui.allocate_exact_size(egui::vec2(96.0, cs), egui::Sense::hover());
                    let active = active_layer == Some(*lid);
                    ui.painter().text(
                        lrect.left_center() + egui::vec2(4.0, 0.0),
                        egui::Align2::LEFT_CENTER,
                        name,
                        egui::FontId::proportional(12.0),
                        if active { egui::Color32::WHITE } else { egui::Color32::from_gray(170) },
                    );
                    for f in 0..frame_count {
                        let (rect, resp) = ui.allocate_exact_size(egui::vec2(cs, cs), egui::Sense::click());
                        let (has, linked) = cells[li][f];
                        let is_active = active && f as u32 == active_frame;
                        let bg = if is_active {
                            egui::Color32::from_rgb(60, 90, 140)
                        } else {
                            egui::Color32::from_gray(32)
                        };
                        ui.painter().rect_filled(rect, 2.0, bg);
                        if has {
                            let dot = rect.shrink(cs * 0.32);
                            let col = if linked {
                                egui::Color32::from_rgb(120, 200, 140)
                            } else {
                                egui::Color32::from_gray(200)
                            };
                            ui.painter().rect_filled(dot, 2.0, col);
                        }
                        ui.painter().rect_stroke(
                            rect,
                            2.0,
                            egui::Stroke::new(1.0, egui::Color32::from_black_alpha(120)),
                            egui::StrokeKind::Middle,
                        );
                        if resp.clicked() {
                            pick = Some((*lid, FrameIndex::new(f as u32)));
                        }
                    }
                });
            }
        });

        if let Some((lid, frame)) = pick {
            self.playing = false;
            self.doc.active_layer = Some(lid);
            self.doc.active_frame = frame;
            self.refresh_canvas(false);
        }

        // Cel-thumbnail size control.
        ui.add(egui::Slider::new(&mut self.editor.cel_size, 20.0..=96.0).text("cel size"));
    }

    /// Steps the playhead by `delta` frames, clamped, and refreshes.
    fn step_frame(&mut self, delta: i32) {
        self.playing = false;
        let count = self.doc.frame_count() as i32;
        if count == 0 {
            return;
        }
        let cur = self.doc.active_frame.get() as i32;
        let next = (cur + delta).rem_euclid(count);
        #[allow(clippy::cast_sign_loss)]
        {
            self.doc.active_frame = FrameIndex::new(next as u32);
        }
        self.refresh_canvas(false);
    }

    /// Appends an empty frame and selects it.
    fn add_frame(&mut self) {
        push_sprite_edit(&mut self.editor, &mut self.doc, "Add frame", |sprite| {
            sprite.frames.push(Frame::default());
        });
        let last = self.doc.frame_count().saturating_sub(1) as u32;
        self.doc.active_frame = FrameIndex::new(last);
        self.refresh_canvas(false);
    }

    /// Duplicates the active frame and selects the new frame. Raster cels get
    /// independent pixel copies; linked and tilemap cels carry over so the new
    /// frame looks identical. Per-cel position, opacity, and user data are
    /// preserved rather than reset to defaults.
    #[allow(clippy::cast_possible_truncation)]
    fn duplicate_frame(&mut self) {
        let idx = self.doc.active_frame.get();
        let insert_at = idx + 1;
        // Clone every cel on the active frame, releasing the sprite borrow
        // before touching the pixel-buffer registry below.
        let cels_on_frame: Vec<Cel> = match self.doc.active_sprite() {
            Some(sprite) => sprite.cels.iter().filter(|c| c.frame_index.get() == idx).cloned().collect(),
            None => return,
        };
        // Copy raster bytes into fresh buffers and hand them to the command so
        // undo removes them instead of leaking; linked sources are shifted to
        // track `shift_frames` below.
        let mut added: Vec<(PixelBufferId, PixelBuffer)> = Vec::new();
        let new_cels = retarget_duplicated_cels(cels_on_frame, insert_at, |src| {
            let bytes = self.doc.pixel_buffers.get(&src).cloned()?;
            let new_id = PixelBufferId::new(self.doc.alloc_id());
            added.push((new_id, bytes));
            Some(new_id)
        });

        push_sprite_edit_with_buffers(&mut self.editor, &mut self.doc, "Duplicate frame", added, |sprite| {
            shift_frames(sprite, insert_at, 1);
            sprite.frames.insert(insert_at as usize, Frame::default());
            sprite.cels.extend(new_cels.iter().cloned());
        });
        self.doc.active_frame = FrameIndex::new(insert_at);
        self.refresh_canvas(false);
    }

    /// Deletes the active frame, keeping at least one.
    #[allow(clippy::cast_possible_truncation)]
    fn delete_frame(&mut self) {
        let count = self.doc.frame_count();
        if count <= 1 {
            return;
        }
        let idx = self.doc.active_frame.get();
        push_sprite_edit(&mut self.editor, &mut self.doc, "Delete frame", |sprite| {
            if (idx as usize) < sprite.frames.len() {
                sprite.frames.remove(idx as usize);
            }
            sprite.cels.retain(|c| c.frame_index.get() != idx);
            shift_frames(sprite, idx, -1);
            clamp_tags(sprite);
        });
        let new_count = self.doc.frame_count() as u32;
        if idx >= new_count {
            self.doc.active_frame = FrameIndex::new(new_count.saturating_sub(1));
        }
        self.refresh_canvas(false);
    }

    /// Links the active cel to the previous frame's cel on the same layer, so
    /// the held drawing is shared rather than duplicated.
    fn link_to_previous(&mut self) {
        let frame = self.doc.active_frame.get();
        if frame == 0 {
            return;
        }
        let Some(layer) = self.doc.active_layer else {
            return;
        };
        let source = FrameIndex::new(frame - 1);
        push_sprite_edit(&mut self.editor, &mut self.doc, "Link cel", |sprite| {
            // Only link when the previous frame actually owns a cel.
            if sprite.cel(layer, source).is_none() {
                return;
            }
            let cur = FrameIndex::new(frame);
            sprite.cels.retain(|c| !(c.layer_id == layer && c.frame_index == cur));
            sprite.cels.push(Cel {
                layer_id: layer,
                frame_index: cur,
                position: pixhaus_core::project::IVec2::zero(),
                opacity: 255,
                data: CelData::Linked { source_frame: source },
                user_data: pixhaus_core::project::UserData::default(),
            });
        });
        self.refresh_canvas(false);
    }

    /// Breaks the active cel's link by copying the source buffer into a fresh
    /// owned raster cel.
    fn unlink_cel(&mut self) {
        let frame = self.doc.active_frame;
        let Some(layer) = self.doc.active_layer else {
            return;
        };
        let Some(sprite) = self.doc.active_sprite() else {
            return;
        };
        let Some(cel) = sprite.cel(layer, frame) else {
            return;
        };
        let CelData::Linked { source_frame } = cel.data else {
            return;
        };
        // Find the source buffer and clone it into a new buffer.
        let canvas = sprite.canvas;
        let Some(src_cel) = sprite.cel(layer, source_frame) else {
            return;
        };
        let CelData::Raster { buffer, .. } = src_cel.data else {
            return;
        };
        let Some(bytes) = self.doc.pixel_buffers.get(&buffer).cloned() else {
            return;
        };
        let new_id = PixelBufferId::new(self.doc.alloc_id());
        push_sprite_edit_with_buffers(&mut self.editor, &mut self.doc, "Unlink cel", vec![(new_id, bytes)], |sprite| {
            sprite.cels.retain(|c| !(c.layer_id == layer && c.frame_index == frame));
            sprite.cels.push(Cel::raster(layer, frame, new_id, canvas));
        });
        self.refresh_canvas(false);
    }

    /// Tags the whole frame range with the draft name (or "tag").
    #[allow(clippy::cast_possible_truncation)]
    fn add_tag(&mut self) {
        let count = self.doc.frame_count();
        if count == 0 {
            return;
        }
        let name = if self.editor.new_tag_name.trim().is_empty() {
            "tag".to_owned()
        } else {
            self.editor.new_tag_name.trim().to_owned()
        };
        let range = FrameRange::new(FrameIndex::new(0), FrameIndex::new(count as u32 - 1));
        push_sprite_edit(&mut self.editor, &mut self.doc, "Add tag", |sprite| {
            sprite.frame_tags.push(FrameTag {
                name,
                range,
                loop_direction: LoopDirection::Forward,
                repeat: 0,
                user_data: pixhaus_core::project::UserData::default(),
            });
        });
        self.editor.new_tag_name.clear();
    }
}

/// Shifts cel frame indices (and linked sources) at or after `at` by `delta`,
/// for insert (+1) or delete (-1).
#[allow(clippy::cast_possible_wrap, clippy::cast_sign_loss)]
fn shift_frames(sprite: &mut pixhaus_core::project::Sprite, at: u32, delta: i32) {
    for cel in &mut sprite.cels {
        if cel.frame_index.get() >= at {
            let v = cel.frame_index.get() as i32 + delta;
            cel.frame_index = FrameIndex::new(v.max(0) as u32);
        }
        if let CelData::Linked { source_frame } = &mut cel.data {
            if source_frame.get() >= at {
                let v = source_frame.get() as i32 + delta;
                *source_frame = FrameIndex::new(v.max(0) as u32);
            }
        }
    }
}

/// Retargets a frame's cels onto the duplicate inserted at `insert_at`.
///
/// Each cel keeps its position, opacity, and user data. Raster cels get an
/// independent buffer via `alloc_buffer` (which copies the source bytes and
/// returns a fresh id); a raster cel whose buffer cannot be resolved is
/// dropped. Linked sources are shifted by `+1` when they sit at or past
/// `insert_at`, mirroring what `shift_frames(insert_at, 1)` does to the
/// existing cels, so old and new linked cels resolve to the same content.
fn retarget_duplicated_cels(cels_on_frame: Vec<Cel>, insert_at: u32, mut alloc_buffer: impl FnMut(PixelBufferId) -> Option<PixelBufferId>) -> Vec<Cel> {
    let mut out = cels_on_frame;
    out.retain_mut(|cel| {
        cel.frame_index = FrameIndex::new(insert_at);
        match &mut cel.data {
            CelData::Raster { buffer, .. } => match alloc_buffer(*buffer) {
                Some(new_id) => {
                    *buffer = new_id;
                    true
                }
                None => false,
            },
            CelData::Linked { source_frame } if source_frame.get() >= insert_at => {
                *source_frame = FrameIndex::new(source_frame.get() + 1);
                true
            }
            _ => true,
        }
    });
    out
}

/// Clamps tag/animation ranges to the current frame count, dropping any that
/// fall entirely off the end.
fn clamp_tags(sprite: &mut pixhaus_core::project::Sprite) {
    let last = sprite.frames.len().saturating_sub(1) as u32;
    sprite.frame_tags.retain(|t| t.range.start.get() <= last);
    for t in &mut sprite.frame_tags {
        if t.range.end.get() > last {
            t.range.end = FrameIndex::new(last);
        }
    }
    sprite.animations.retain(|a| a.range.start.get() <= last);
    for a in &mut sprite.animations {
        if a.range.end.get() > last {
            a.range.end = FrameIndex::new(last);
        }
    }
}

fn loop_label(dir: LoopDirection) -> &'static str {
    match dir {
        LoopDirection::Forward => "Forward",
        LoopDirection::Reverse => "Reverse",
        LoopDirection::PingPong => "Ping-pong",
        LoopDirection::PingPongReverse => "Ping-pong rev",
    }
}

/// A distinct colour per tag index for the header underline.
fn tag_color(i: usize) -> egui::Color32 {
    const COLORS: [egui::Color32; 6] = [
        egui::Color32::from_rgb(230, 120, 90),
        egui::Color32::from_rgb(120, 200, 120),
        egui::Color32::from_rgb(120, 160, 230),
        egui::Color32::from_rgb(220, 200, 110),
        egui::Color32::from_rgb(200, 120, 210),
        egui::Color32::from_rgb(110, 210, 200),
    ];
    COLORS[i % COLORS.len()]
}

#[cfg(test)]
mod tests {
    use super::*;
    use pixhaus_core::project::{IVec2, Size, UserData};

    fn raster_cel(layer: u32, frame: u32, buffer: u32, opacity: u8, position: IVec2) -> Cel {
        Cel {
            layer_id: LayerId::new(layer),
            frame_index: FrameIndex::new(frame),
            position,
            opacity,
            data: CelData::Raster {
                buffer: PixelBufferId::new(buffer),
                size: Size::new(8, 8),
            },
            user_data: UserData::default(),
        }
    }

    fn linked_cel(layer: u32, frame: u32, source: u32) -> Cel {
        Cel {
            layer_id: LayerId::new(layer),
            frame_index: FrameIndex::new(frame),
            position: IVec2::zero(),
            opacity: 255,
            data: CelData::Linked {
                source_frame: FrameIndex::new(source),
            },
            user_data: UserData::default(),
        }
    }

    #[test]
    fn retarget_preserves_attributes_and_remaps_raster_buffer() {
        let cel = raster_cel(1, 2, 7, 128, IVec2 { x: 5, y: 6 });
        let out = retarget_duplicated_cels(vec![cel], 3, |src| {
            assert_eq!(src, PixelBufferId::new(7), "the source buffer must be the one to copy");
            Some(PixelBufferId::new(99))
        });
        assert_eq!(out.len(), 1);
        let new = &out[0];
        assert_eq!(new.frame_index, FrameIndex::new(3), "cel moves to the inserted frame");
        assert_eq!(new.opacity, 128, "per-cel opacity is preserved, not reset to 255");
        assert_eq!(new.position, IVec2 { x: 5, y: 6 }, "per-cel position is preserved");
        match new.data {
            CelData::Raster { buffer, .. } => assert_eq!(buffer, PixelBufferId::new(99), "raster cel points at the fresh buffer"),
            ref other => panic!("expected raster cel, got {other:?}"),
        }
    }

    #[test]
    fn retarget_shifts_only_forward_links() {
        // A back-link (source < insert_at) stays put; a forward-link
        // (source >= insert_at) shifts +1 to track shift_frames.
        let back = linked_cel(1, 2, 0);
        let forward = linked_cel(2, 2, 5);
        let out = retarget_duplicated_cels(vec![back, forward], 3, |_| panic!("links must not allocate buffers"));
        assert_eq!(out.len(), 2, "linked cels are carried over, not dropped");
        let sources: Vec<u32> = out
            .iter()
            .map(|c| match &c.data {
                CelData::Linked { source_frame } => source_frame.get(),
                other => panic!("expected linked cel, got {other:?}"),
            })
            .collect();
        assert_eq!(sources, vec![0, 6], "back-link unchanged; forward-link shifted +1");
    }

    #[test]
    fn retarget_drops_raster_cels_with_unresolvable_buffers() {
        let cel = raster_cel(1, 2, 7, 255, IVec2::zero());
        let out = retarget_duplicated_cels(vec![cel], 3, |_| None);
        assert!(out.is_empty(), "a raster cel whose buffer cannot be copied is dropped");
    }
}
