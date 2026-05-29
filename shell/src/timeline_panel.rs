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
use crate::commands::{extract_region, push_sprite_edit, push_sprite_edit_with_buffers};
use crate::editor::{ClipCel, ClipFrame, FrameClipboard};
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
        ui.toggle_value(&mut self.editor.loop_playback, icons::REPEAT).on_hover_text("Loop playback");
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
            // Pluralize and retarget when a multi-frame selection is active;
            // an empty selection takes the single-active-frame fast path.
            let sel = self.editor.selected_frames.len();
            let (trash_label, trash_hint) = if sel > 1 {
                (format!("{} Frames", icons::TRASH), "Delete selected frames")
            } else {
                (format!("{} Frame", icons::TRASH), "Delete frame")
            };
            if ui.button(trash_label).on_hover_text(trash_hint).clicked() {
                self.delete_selected_frames();
            }
            ui.separator();
            let active = self.doc.active_frame.get();
            let count = self.doc.frame_count() as u32;
            if ui
                .add_enabled(active > 0, egui::Button::new(icons::PREV))
                .on_hover_text("Move frame left")
                .clicked()
            {
                self.move_frame(-1);
            }
            if ui
                .add_enabled(active + 1 < count, egui::Button::new(icons::NEXT))
                .on_hover_text("Move frame right")
                .clicked()
            {
                self.move_frame(1);
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
        // Read the selection modifiers once for the whole matrix (do not nest
        // input closures while painting).
        let (mod_command, mod_shift) = ui.ctx().input(|i| (i.modifiers.command, i.modifiers.shift));
        // Snapshot the selection set so the painter can read it without holding
        // a borrow on `self.editor` while the row loop also reads `self`.
        let selected_frames = self.editor.selected_frames.clone();

        // The accent stroke marking a frame as part of the multi-selection.
        let selection_stroke = egui::Stroke::new(2.0, egui::Color32::from_rgb(230, 180, 90));

        let mut pick: Option<(LayerId, FrameIndex)> = None;
        // A header-cell pick, resolved against the modifiers after the matrix is
        // drawn so it can mutate `selected_frames` and the active frame.
        let mut header_pick: Option<u32> = None;

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
                    // Selection outline (distinct from the active fill).
                    if selected_frames.contains(&(f as u32)) {
                        ui.painter().rect_stroke(rect, 2.0, selection_stroke, egui::StrokeKind::Inside);
                    }
                    // Tag underline.
                    for (s, e, col) in &tag_spans {
                        if (f as u32) >= *s && (f as u32) <= *e {
                            let bar = egui::Rect::from_min_size(rect.left_bottom() - egui::vec2(0.0, 3.0), egui::vec2(rect.width(), 3.0));
                            ui.painter().rect_filled(bar, 0.0, *col);
                        }
                    }
                    if resp.clicked() {
                        header_pick = Some(f as u32);
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
                        // Selection outline marks the whole column as selected.
                        if selected_frames.contains(&(f as u32)) {
                            ui.painter().rect_stroke(rect, 2.0, selection_stroke, egui::StrokeKind::Inside);
                        }
                        if resp.clicked() {
                            pick = Some((*lid, FrameIndex::new(f as u32)));
                        }
                    }
                });
            }
        });

        // A header-cell click resolves against the modifiers held at click:
        // plain -> clear the set and move the playhead; Ctrl/Cmd -> toggle that
        // frame, keep the playhead; Shift -> select the inclusive range from the
        // active frame to the clicked frame.
        if let Some(f) = header_pick {
            self.playing = false;
            if mod_command {
                if !self.editor.selected_frames.insert(f) {
                    self.editor.selected_frames.remove(&f);
                }
            } else if mod_shift {
                self.editor.selected_frames = crate::editor::frame_range_set(active_frame, f);
                self.doc.active_frame = FrameIndex::new(f);
            } else {
                self.editor.clear_frame_selection();
                self.doc.active_frame = FrameIndex::new(f);
            }
            self.refresh_canvas(false);
        }

        // A cel click selects that layer and frame, clearing any multi-frame
        // selection (it picks a single drawing target, not a batch).
        if let Some((lid, frame)) = pick {
            self.playing = false;
            self.editor.clear_frame_selection();
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

    /// Deletes the selected frames (or just the active frame when nothing is
    /// explicitly selected) in one undoable step, then lands the playhead on a
    /// surviving frame.
    ///
    /// The active-index fix matches the Tauri `deleteFrames`: if the active
    /// frame was deleted, jump to `max(0, lowest_deleted - 1)`; otherwise the
    /// active frame survived but may have shifted left, so subtract one for
    /// every deleted index below it.
    #[allow(clippy::cast_possible_truncation)]
    fn delete_selected_frames(&mut self) {
        let count = self.doc.frame_count() as u32;
        if count <= 1 {
            return;
        }
        let active = self.doc.active_frame.get();
        let set = self.editor.effective_frames(active);
        // Resolve the active-index fix from the *requested* set, before the
        // last-frame guard inside `delete_frames` may trim it. The guard only
        // fires when the set covers every frame, and that path always lands on
        // frame 0 anyway, so reading the requested set here is safe.
        let next = next_active_after_delete(active, &set);

        push_sprite_edit(&mut self.editor, &mut self.doc, "Delete frames", |sprite| {
            delete_frames(sprite, &set);
        });

        let new_count = self.doc.frame_count() as u32;
        self.doc.active_frame = FrameIndex::new(next.min(new_count.saturating_sub(1)));
        self.editor.clear_frame_selection();
        self.refresh_canvas(false);
    }

    /// Moves the active frame one position left (`delta = -1`) or right
    /// (`delta = 1`), keeping its cels, and follows it with the playhead. The
    /// reorder remaps cels, tags, animations, and slice keys in one undo step.
    #[allow(clippy::cast_possible_wrap, clippy::cast_sign_loss)]
    fn move_frame(&mut self, delta: i32) {
        let from = self.doc.active_frame.get();
        let count = self.doc.frame_count() as i32;
        if count <= 1 {
            return;
        }
        let to = from as i32 + delta;
        if to < 0 || to >= count {
            return;
        }
        let to = to as u32;
        push_sprite_edit(&mut self.editor, &mut self.doc, "Move frame", |sprite| {
            reorder_frame(sprite, from, to);
        });
        self.doc.active_frame = FrameIndex::new(to);
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

    /// Snapshots the selected frames (or the active frame when nothing is
    /// explicitly selected) into the frame clipboard, resolving every cel to
    /// owned pixel bytes. Linked cels are followed to their source buffer at
    /// copy time, so the clipboard is self-contained and survives later edits
    /// to the source. Records nothing on the undo stack — copy is read-only.
    // Wired by the frame context menu (plan task 11); kept self-contained now
    // so that task only adds the call site.
    #[allow(dead_code, clippy::cast_possible_truncation)]
    pub(crate) fn copy_frames(&mut self) {
        let active = self.doc.active_frame.get();
        let set = self.editor.effective_frames(active);
        let Some(sprite) = self.doc.active_sprite() else {
            return;
        };
        let canvas = sprite.canvas;
        // Ascending order so paste rebuilds the run in timeline order.
        let mut frames: Vec<ClipFrame> = Vec::with_capacity(set.len());
        for &fi in &set {
            let frame_idx = FrameIndex::new(fi);
            let Some(frame) = sprite.frames.get(fi as usize).cloned() else {
                continue;
            };
            // Each cel sitting on this frame, resolved to owned bytes.
            let mut cels: Vec<ClipCel> = Vec::new();
            for cel in sprite.cels.iter().filter(|c| c.frame_index == frame_idx) {
                // Resolve the layer's source frame (one link hop) and read its
                // raster buffer. Tilemap cels carry no raster bytes here and a
                // link with no resolvable raster source is skipped.
                let source = sprite.resolve_source_frame(cel.layer_id, frame_idx);
                let Some(src_cel) = sprite.cel(cel.layer_id, source) else {
                    continue;
                };
                let CelData::Raster { buffer, size } = src_cel.data else {
                    continue;
                };
                let Some(buf) = self.doc.pixel_buffers.get(&buffer) else {
                    continue;
                };
                let bytes = extract_region(buf, 0, 0, size.width, size.height);
                cels.push(ClipCel {
                    layer_id: cel.layer_id,
                    position: cel.position,
                    opacity: cel.opacity,
                    bytes,
                    size,
                });
            }
            frames.push(ClipFrame { frame, cels });
        }
        if frames.is_empty() {
            return;
        }
        self.editor.frame_clipboard = Some(FrameClipboard { canvas, frames });
    }

    /// Copies the selected frames, then deletes them — the standard cut. The
    /// copy records nothing; the delete is one undoable [`SpriteEdit`].
    // Wired by the frame context menu (plan task 11).
    #[allow(dead_code)]
    pub(crate) fn cut_frames(&mut self) {
        self.copy_frames();
        self.delete_selected_frames();
    }

    /// Pastes the frame clipboard after the active frame, allocating a fresh
    /// buffer per cel so the pasted pixels are independent of the source.
    ///
    /// Rejects (no-op + warn) when the clipboard's canvas size differs from the
    /// active sprite's: a cel's bytes are sized for the source canvas, and v2
    /// does not scale on paste. The new frames and their buffers are recorded as
    /// one [`SpriteBufferEdit`], so undo removes both the frames and the pasted
    /// buffers rather than leaking them. The playhead lands on the first pasted
    /// frame.
    // Wired by the frame context menu (plan task 11).
    #[allow(dead_code, clippy::cast_possible_truncation)]
    pub(crate) fn paste_frames(&mut self) {
        let Some(clip) = self.editor.frame_clipboard.clone() else {
            return;
        };
        let Some(canvas) = self.doc.active_sprite().map(|s| s.canvas) else {
            return;
        };
        if !paste_allowed(clip.canvas, canvas) {
            tracing::warn!(
                clipboard = ?clip.canvas,
                target = ?canvas,
                "paste rejected: frame clipboard canvas size does not match the active sprite"
            );
            return;
        }
        if clip.frames.is_empty() {
            return;
        }
        let insert_at = self.doc.active_frame.get() + 1;
        let plan = build_paste_plan(&clip, insert_at, || PixelBufferId::new(self.doc.alloc_id()));
        let PastePlan { added, frames, cels } = plan;
        let n = frames.len() as i32;

        push_sprite_edit_with_buffers(&mut self.editor, &mut self.doc, "Paste frames", added, |sprite| {
            shift_frames(sprite, insert_at, n);
            for (offset, frame) in frames.iter().enumerate() {
                sprite.frames.insert(insert_at as usize + offset, frame.clone());
            }
            sprite.cels.extend(cels.iter().cloned());
        });
        self.doc.active_frame = FrameIndex::new(insert_at);
        self.editor.clear_frame_selection();
        self.refresh_canvas(false);
    }
}

/// Whether a frame clipboard may paste into a sprite of `target` canvas size.
///
/// A copied cel's bytes are sized for the source canvas, and v2 does not scale
/// on paste, so the canvases must match exactly. Cross-sprite paste of a
/// different size is rejected rather than scaled — the deliberate trade-off for
/// the byte-based clipboard.
fn paste_allowed(clip_canvas: pixhaus_core::project::Size, target: pixhaus_core::project::Size) -> bool {
    clip_canvas == target
}

/// The frames, cels, and fresh buffers a paste inserts, built off the document
/// so the structural insert and the buffer allocation are one testable unit.
struct PastePlan {
    /// Fresh buffers to hand to `push_sprite_edit_with_buffers` (owned by the
    /// undo entry, removed on undo).
    added: Vec<(PixelBufferId, PixelBuffer)>,
    /// The frames to insert at `insert_at`, in clipboard order.
    frames: Vec<Frame>,
    /// The cels to extend onto the sprite, already retargeted to the inserted
    /// frame indices and pointing at the fresh buffers.
    cels: Vec<Cel>,
}

/// Builds the [`PastePlan`] for inserting `clip` at `insert_at`.
///
/// Each clipboard cel becomes an independent raster cel: its packed bytes are
/// rebuilt into a fresh [`PixelBuffer`] under a new id from `alloc_id`, so the
/// pasted pixels never alias the source. A cel whose bytes do not form a valid
/// buffer is dropped rather than aborting the paste. Pure over the document, so
/// the byte-duplication and retargeting are unit-testable without a `ShellApp`.
#[allow(clippy::cast_possible_truncation)]
fn build_paste_plan(clip: &FrameClipboard, insert_at: u32, mut alloc_id: impl FnMut() -> PixelBufferId) -> PastePlan {
    let mut added: Vec<(PixelBufferId, PixelBuffer)> = Vec::new();
    let mut frames: Vec<Frame> = Vec::with_capacity(clip.frames.len());
    let mut cels: Vec<Cel> = Vec::new();
    for (offset, cf) in clip.frames.iter().enumerate() {
        let target = FrameIndex::new(insert_at + offset as u32);
        frames.push(cf.frame.clone());
        for cc in &cf.cels {
            let stride = cc.size.width.saturating_mul(4);
            let Ok(buffer) = PixelBuffer::from_raw(cc.size.width, cc.size.height, stride, cc.bytes.clone()) else {
                continue;
            };
            let new_id = alloc_id();
            added.push((new_id, buffer));
            cels.push(Cel {
                layer_id: cc.layer_id,
                frame_index: target,
                position: cc.position,
                opacity: cc.opacity,
                data: CelData::Raster {
                    buffer: new_id,
                    size: cc.size,
                },
                user_data: pixhaus_core::project::UserData::default(),
            });
        }
    }
    PastePlan { added, frames, cels }
}

/// Shifts every frame-index-bearing field at or after `at` by `delta`, for
/// insert (+1) or delete (-1).
///
/// Walks cels (and their linked sources), `frame_tags` ranges, `animations`
/// ranges, and `slices.keys` in lockstep so cels, tags, animations, and slice
/// keys all stay aligned with their frames across a timeline insert or delete.
/// Mirrors the Tauri `shift_frame_indices` (`app/src/commands/frames.rs`).
#[allow(clippy::cast_possible_wrap, clippy::cast_sign_loss)]
fn shift_frames(sprite: &mut pixhaus_core::project::Sprite, at: u32, delta: i32) {
    let shift = |idx: &mut FrameIndex| {
        if idx.get() >= at {
            let v = idx.get() as i32 + delta;
            *idx = FrameIndex::new(v.max(0) as u32);
        }
    };

    for cel in &mut sprite.cels {
        shift(&mut cel.frame_index);
        if let CelData::Linked { source_frame } = &mut cel.data {
            shift(source_frame);
        }
    }
    for tag in &mut sprite.frame_tags {
        shift(&mut tag.range.start);
        shift(&mut tag.range.end);
    }
    for anim in &mut sprite.animations {
        shift(&mut anim.range.start);
        shift(&mut anim.range.end);
    }
    for slice in &mut sprite.slices {
        for key in &mut slice.keys {
            shift(&mut key.frame);
        }
    }
}

/// Remaps a single frame index across a `reorder_frame(from, to)` permutation.
///
/// - The frame at `from` lands at `to`.
/// - Frames between `from` and `to` shift by one, opposite to the move
///   direction.
/// - Frames outside the moved span are untouched.
///
/// Ported verbatim from the Tauri `remap_for_reorder`
/// (`app/src/commands/frames.rs`).
fn remap_for_reorder(idx: FrameIndex, from: u32, to: u32) -> FrameIndex {
    let n = idx.get();
    if n == from {
        FrameIndex::new(to)
    } else if from < to && n > from && n <= to {
        FrameIndex::new(n - 1)
    } else if from > to && n >= to && n < from {
        FrameIndex::new(n + 1)
    } else {
        idx
    }
}

/// Normalises a `[start, end]` pair so `start <= end`, used after a reorder
/// permutation that may have flipped a tag or animation range's endpoints.
fn ordered_range(a: FrameIndex, b: FrameIndex) -> FrameRange {
    if a.get() <= b.get() { FrameRange::new(a, b) } else { FrameRange::new(b, a) }
}

/// Moves the frame at `from` to position `to`, remapping every cel, tag,
/// animation, and slice key so they track their frames through the move.
///
/// `to` is clamped to the last valid index; an out-of-range `from` is a no-op.
/// Mirrors the Tauri `frame_reorder` (`app/src/commands/frames.rs`).
#[allow(clippy::cast_possible_truncation)]
fn reorder_frame(sprite: &mut pixhaus_core::project::Sprite, from: u32, to: u32) {
    let len = sprite.frames.len();
    if (from as usize) >= len || len == 0 {
        return;
    }
    let to = to.min(len as u32 - 1);
    if from == to {
        return;
    }
    let frame = sprite.frames.remove(from as usize);
    sprite.frames.insert(to as usize, frame);

    for cel in &mut sprite.cels {
        cel.frame_index = remap_for_reorder(cel.frame_index, from, to);
    }
    for tag in &mut sprite.frame_tags {
        let s = remap_for_reorder(tag.range.start, from, to);
        let e = remap_for_reorder(tag.range.end, from, to);
        tag.range = ordered_range(s, e);
    }
    for anim in &mut sprite.animations {
        let s = remap_for_reorder(anim.range.start, from, to);
        let e = remap_for_reorder(anim.range.end, from, to);
        anim.range = ordered_range(s, e);
    }
    for slice in &mut sprite.slices {
        for key in &mut slice.keys {
            key.frame = remap_for_reorder(key.frame, from, to);
        }
    }
}

/// Swaps the frames at `a` and `b`, leaving every frame between them in place.
///
/// Composes two `reorder_frame` calls (move `lo` to `hi`, then move the element
/// now at `hi - 1` back to `lo`), so the cel/range remap stays correct for
/// free. Mirrors the Tauri `swapFrames` (`ui/src/timeline/timeline-state.ts`).
// Consumed by reverse and the timeline context menu (plan tasks 2 and 11);
// kept as a pure free fn now so those tasks reuse one helper.
#[allow(dead_code)]
fn swap_frames(sprite: &mut pixhaus_core::project::Sprite, a: u32, b: u32) {
    if a == b {
        return;
    }
    let (lo, hi) = if a < b { (a, b) } else { (b, a) };
    reorder_frame(sprite, lo, hi);
    reorder_frame(sprite, hi - 1, lo);
}

/// Builds the sequence of `(from, to)` swap pairs that reverse a sorted set of
/// frame indices: pair the outermost inward, leaving any middle frame fixed.
///
/// Ported from the Tauri `buildSwapPairs`
/// (`ui/src/timeline/timeline-state.ts`). `BTreeSet` iterates ascending, so the
/// input is already sorted.
#[allow(dead_code)]
fn build_swap_pairs(indices: &std::collections::BTreeSet<u32>) -> Vec<(u32, u32)> {
    let pts: Vec<u32> = indices.iter().copied().collect();
    let mut pairs = Vec::with_capacity(pts.len() / 2);
    let mut lo = 0usize;
    let mut hi = pts.len().saturating_sub(1);
    while lo < hi {
        pairs.push((pts[lo], pts[hi]));
        lo += 1;
        hi -= 1;
    }
    pairs
}

/// Reverses the order of the selected frames in place, applying one
/// `swap_frames` per pair from [`build_swap_pairs`]. A middle frame in an
/// odd-sized selection stays fixed.
// Wired by the reverse-selected action in the timeline context menu (plan task
// 11); kept pure and tested now so that task only adds the call site.
#[allow(dead_code)]
fn reverse_frames(sprite: &mut pixhaus_core::project::Sprite, indices: &std::collections::BTreeSet<u32>) {
    for (a, b) in build_swap_pairs(indices) {
        swap_frames(sprite, a, b);
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

/// Where the playhead should land after deleting `indices`, given the prior
/// `active` frame. Pure index arithmetic over the *requested* delete set, with
/// no knowledge of the last-frame guard or the surviving count — the caller
/// clamps the result to the new frame count.
///
/// Mirrors the Tauri `deleteFrames` rule: if the active frame was deleted, land
/// on `max(0, lowest_deleted - 1)`; otherwise the active frame survived but may
/// have shifted left, so subtract one for every deleted index below it.
fn next_active_after_delete(active: u32, indices: &std::collections::BTreeSet<u32>) -> u32 {
    if indices.contains(&active) {
        let lowest_deleted = indices.iter().next().copied().unwrap_or(0);
        lowest_deleted.saturating_sub(1)
    } else {
        let deleted_before = indices.iter().filter(|&&i| i < active).count() as u32;
        active.saturating_sub(deleted_before)
    }
}

/// Removes the frames at `indices` and re-indexes everything attached to them.
///
/// Iterates highest-index-first so a removal never shifts an index still
/// pending in the set; for each frame it drops the frame, drops that frame's
/// cels, then shifts every later cel, tag, animation, and slice key back by one
/// via [`shift_frames`]. A final [`clamp_tags`] trims any range left dangling.
///
/// Guards the last surviving frame: a sprite must keep at least one frame, so
/// if `indices` covers every frame the lowest index is dropped from the delete
/// set. Mirrors the Tauri `deleteFrames` (`ui/src/timeline/timeline-state.ts`).
#[allow(clippy::cast_possible_truncation)]
fn delete_frames(sprite: &mut pixhaus_core::project::Sprite, indices: &std::collections::BTreeSet<u32>) {
    let total = sprite.frames.len() as u32;
    // Build the descending delete order, guarding the last frame: never empty
    // the sprite. If the set covers everything, keep the lowest index.
    let mut to_delete: Vec<u32> = indices.iter().copied().filter(|&i| i < total).collect();
    if to_delete.len() as u32 >= total && total > 0 {
        // `indices` is ascending; the lowest is first, so drop it.
        to_delete.remove(0);
    }
    for idx in to_delete.into_iter().rev() {
        let i = idx as usize;
        if i < sprite.frames.len() {
            sprite.frames.remove(i);
        }
        sprite.cels.retain(|c| c.frame_index.get() != idx);
        shift_frames(sprite, idx + 1, -1);
    }
    clamp_tags(sprite);
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
    use std::collections::BTreeSet;

    use super::*;
    use pixhaus_core::project::{Animation, AnimationId, IVec2, Rect, Size, Slice, SliceId, SliceKey, Sprite, SpriteId, UserData};

    fn sprite_with_frames(n: u32) -> Sprite {
        let mut s = Sprite::empty(SpriteId::new(1), "t", Size::new(8, 8));
        for _ in 0..n {
            s.frames.push(Frame::default());
        }
        s
    }

    fn simple_raster_cel(frame: u32, layer: u32) -> Cel {
        Cel::raster(LayerId::new(layer), FrameIndex::new(frame), PixelBufferId::new(1), Size::new(8, 8))
    }

    fn cel_frames(s: &Sprite) -> Vec<u32> {
        s.cels.iter().map(|c| c.frame_index.get()).collect()
    }

    fn slice_with_keys(frames: &[u32]) -> Slice {
        Slice {
            id: SliceId::new(1),
            name: "s".into(),
            keys: frames
                .iter()
                .map(|f| SliceKey {
                    frame: FrameIndex::new(*f),
                    bounds: Rect::from_xywh(0, 0, 1, 1),
                    nine_slice: None,
                    pivot: None,
                })
                .collect(),
            user_data: UserData::default(),
        }
    }

    fn frame_set(ids: &[u32]) -> BTreeSet<u32> {
        ids.iter().copied().collect()
    }

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

    // shift_frames ─────────────────────────────────────────────────────────────

    #[test]
    fn shift_minus_one_drops_indices_from_threshold() {
        let mut s = sprite_with_frames(4);
        s.cels = vec![
            simple_raster_cel(0, 1),
            simple_raster_cel(1, 1),
            simple_raster_cel(2, 1),
            simple_raster_cel(3, 1),
        ];
        // Frame-delete at index 1: the caller drops cels on frame 1 first, then
        // shifts everything past back by one.
        s.cels.retain(|c| c.frame_index.get() != 1);
        shift_frames(&mut s, 2, -1);
        assert_eq!(cel_frames(&s), vec![0, 1, 2]);
    }

    #[test]
    fn shift_plus_one_makes_room_for_insertion() {
        let mut s = sprite_with_frames(3);
        s.cels = vec![simple_raster_cel(0, 1), simple_raster_cel(1, 1), simple_raster_cel(2, 1)];
        shift_frames(&mut s, 1, 1);
        assert_eq!(cel_frames(&s), vec![0, 2, 3]);
    }

    #[test]
    fn shift_walks_tags_animations_and_slices() {
        let mut s = sprite_with_frames(5);
        s.frame_tags.push(FrameTag {
            name: "walk".into(),
            range: FrameRange::new(FrameIndex::new(1), FrameIndex::new(3)),
            loop_direction: LoopDirection::Forward,
            repeat: 0,
            user_data: UserData::default(),
        });
        s.animations.push(Animation::forward(
            AnimationId::new(1),
            "walk",
            FrameRange::new(FrameIndex::new(2), FrameIndex::new(4)),
        ));
        s.slices.push(slice_with_keys(&[1, 3, 4]));

        shift_frames(&mut s, 2, -1);

        assert_eq!(s.frame_tags[0].range.start.get(), 1);
        assert_eq!(s.frame_tags[0].range.end.get(), 2);
        assert_eq!(s.animations[0].range.start.get(), 1);
        assert_eq!(s.animations[0].range.end.get(), 3);
        let key_frames: Vec<u32> = s.slices[0].keys.iter().map(|k| k.frame.get()).collect();
        assert_eq!(key_frames, vec![1, 2, 3]);
    }

    // remap_for_reorder ────────────────────────────────────────────────────────

    #[test]
    fn remap_forward_move_shifts_middle_left() {
        // Move frame 1 to position 3: [A B C D] -> [A C D B].
        let r = |n: u32| remap_for_reorder(FrameIndex::new(n), 1, 3).get();
        assert_eq!(r(0), 0);
        assert_eq!(r(1), 3);
        assert_eq!(r(2), 1);
        assert_eq!(r(3), 2);
    }

    #[test]
    fn remap_backward_move_shifts_middle_right() {
        // Move frame 3 to position 1: [A B C D] -> [A D B C].
        let r = |n: u32| remap_for_reorder(FrameIndex::new(n), 3, 1).get();
        assert_eq!(r(0), 0);
        assert_eq!(r(1), 2);
        assert_eq!(r(2), 3);
        assert_eq!(r(3), 1);
    }

    #[test]
    fn remap_outside_span_is_identity() {
        let r = |n: u32| remap_for_reorder(FrameIndex::new(n), 2, 4).get();
        assert_eq!(r(0), 0);
        assert_eq!(r(1), 1);
        assert_eq!(r(5), 5);
    }

    // reorder_frame ────────────────────────────────────────────────────────────

    #[test]
    fn reorder_moves_frame_and_keeps_cels_attached() {
        // One cel per frame on layer 1; moving frame 1 to 3 must carry each
        // cel with its frame: [c0 c1 c2 c3] -> [c0 c2 c3 c1] reading by position.
        let mut s = sprite_with_frames(4);
        s.cels = vec![
            simple_raster_cel(0, 1),
            simple_raster_cel(1, 1),
            simple_raster_cel(2, 1),
            simple_raster_cel(3, 1),
        ];
        reorder_frame(&mut s, 1, 3);
        let mut frames = cel_frames(&s);
        frames.sort_unstable();
        assert_eq!(frames, vec![0, 1, 2, 3], "every frame still owns exactly one cel");
        // The cel that was at frame 1 now sits at frame 3.
        assert_eq!(cel_frames(&s)[1], 3, "the moved cel landed at its new index");
    }

    #[test]
    fn reorder_clamps_out_of_range_target() {
        let mut s = sprite_with_frames(3);
        s.cels = vec![simple_raster_cel(0, 1), simple_raster_cel(1, 1), simple_raster_cel(2, 1)];
        reorder_frame(&mut s, 0, 99);
        // 0 -> last index (2): cels become [2, 0, 1] in frame terms.
        assert_eq!(cel_frames(&s), vec![2, 0, 1]);
    }

    #[test]
    fn reorder_out_of_range_source_is_noop() {
        let mut s = sprite_with_frames(2);
        s.cels = vec![simple_raster_cel(0, 1), simple_raster_cel(1, 1)];
        reorder_frame(&mut s, 5, 0);
        assert_eq!(cel_frames(&s), vec![0, 1]);
    }

    // swap_frames ──────────────────────────────────────────────────────────────

    #[test]
    fn swap_exchanges_two_positions_inner_unchanged() {
        // Five cels, one per frame. Swap 1 and 3; frames 0, 2, 4 stay put.
        let mut s = sprite_with_frames(5);
        s.cels = (0..5).map(|f| simple_raster_cel(f, f + 10)).collect();
        swap_frames(&mut s, 1, 3);
        // The buffer that was on frame 1 is now on frame 3 and vice versa;
        // frames 0, 2, 4 keep their original buffers.
        let on = |frame: u32| -> u32 {
            let cel = s.cels.iter().find(|c| c.frame_index.get() == frame).expect("cel present");
            cel.layer_id.get()
        };
        assert_eq!(on(0), 10, "frame 0 untouched");
        assert_eq!(on(1), 13, "frame 1 now holds what was on frame 3");
        assert_eq!(on(2), 12, "frame 2 untouched");
        assert_eq!(on(3), 11, "frame 3 now holds what was on frame 1");
        assert_eq!(on(4), 14, "frame 4 untouched");
    }

    #[test]
    fn swap_same_index_is_noop() {
        let mut s = sprite_with_frames(3);
        s.cels = vec![simple_raster_cel(0, 1), simple_raster_cel(1, 2), simple_raster_cel(2, 3)];
        swap_frames(&mut s, 1, 1);
        assert_eq!(cel_frames(&s), vec![0, 1, 2]);
    }

    // build_swap_pairs ─────────────────────────────────────────────────────────

    #[test]
    fn swap_pairs_empty_selection_has_no_pairs() {
        assert!(build_swap_pairs(&frame_set(&[])).is_empty());
    }

    #[test]
    fn swap_pairs_single_frame_has_no_pairs() {
        assert!(build_swap_pairs(&frame_set(&[3])).is_empty());
    }

    #[test]
    fn swap_pairs_two_frames_one_pair() {
        assert_eq!(build_swap_pairs(&frame_set(&[1, 4])), vec![(1, 4)]);
    }

    #[test]
    fn swap_pairs_three_frames_middle_fixed() {
        // [0, 1, 2]: outer pair swaps, middle is left untouched.
        assert_eq!(build_swap_pairs(&frame_set(&[0, 1, 2])), vec![(0, 2)]);
    }

    #[test]
    fn swap_pairs_non_contiguous_selection() {
        // [0, 2, 5, 7] reverses to [7, 5, 2, 0]: pairs (0,7) and (2,5).
        assert_eq!(build_swap_pairs(&frame_set(&[0, 2, 5, 7])), vec![(0, 7), (2, 5)]);
    }

    // reverse_frames ───────────────────────────────────────────────────────────

    #[test]
    fn reverse_three_frames_flips_outer_keeps_middle() {
        let mut s = sprite_with_frames(3);
        s.cels = (0..3).map(|f| simple_raster_cel(f, f + 10)).collect();
        reverse_frames(&mut s, &frame_set(&[0, 1, 2]));
        let on = |frame: u32| s.cels.iter().find(|c| c.frame_index.get() == frame).expect("cel present").layer_id.get();
        assert_eq!(on(0), 12, "frame 0 now holds the last frame's content");
        assert_eq!(on(1), 11, "middle frame stays put");
        assert_eq!(on(2), 10, "frame 2 now holds the first frame's content");
    }

    #[test]
    fn reverse_full_four_frame_selection() {
        let mut s = sprite_with_frames(4);
        s.cels = (0..4).map(|f| simple_raster_cel(f, f + 10)).collect();
        reverse_frames(&mut s, &frame_set(&[0, 1, 2, 3]));
        let on = |frame: u32| s.cels.iter().find(|c| c.frame_index.get() == frame).expect("cel present").layer_id.get();
        assert_eq!(on(0), 13);
        assert_eq!(on(1), 12);
        assert_eq!(on(2), 11);
        assert_eq!(on(3), 10);
    }

    // delete_frames ──────────────────────────────────────────────────────────

    #[test]
    fn delete_two_frames_reindexes_surviving_cels() {
        // Five frames, one cel each, tagged by layer id (f + 10). Delete {1, 3}:
        // survivors were frames 0, 2, 4 and must re-index to 0, 1, 2 while
        // keeping their original drawings.
        let mut s = sprite_with_frames(5);
        s.cels = (0..5).map(|f| simple_raster_cel(f, f + 10)).collect();
        delete_frames(&mut s, &frame_set(&[1, 3]));
        assert_eq!(s.frames.len(), 3, "two of five frames are gone");
        assert_eq!(cel_frames(&s), vec![0, 1, 2], "surviving cels are packed and re-indexed");
        // The drawings ride along: old frame 0/2/4 -> new frame 0/1/2.
        let on = |frame: u32| s.cels.iter().find(|c| c.frame_index.get() == frame).expect("cel present").layer_id.get();
        assert_eq!(on(0), 10, "old frame 0");
        assert_eq!(on(1), 12, "old frame 2");
        assert_eq!(on(2), 14, "old frame 4");
    }

    #[test]
    fn delete_all_frames_leaves_one() {
        // The guard never empties a sprite: deleting every frame keeps the
        // lowest index and its cel.
        let mut s = sprite_with_frames(3);
        s.cels = (0..3).map(|f| simple_raster_cel(f, f + 10)).collect();
        delete_frames(&mut s, &frame_set(&[0, 1, 2]));
        assert_eq!(s.frames.len(), 1, "one frame always survives");
        assert_eq!(cel_frames(&s), vec![0], "the surviving frame keeps its cel at index 0");
        let on = |frame: u32| s.cels.iter().find(|c| c.frame_index.get() == frame).expect("cel present").layer_id.get();
        assert_eq!(on(0), 10, "the lowest frame (0) is the one kept");
    }

    #[test]
    fn delete_frames_out_of_range_indices_are_ignored() {
        let mut s = sprite_with_frames(3);
        s.cels = (0..3).map(|f| simple_raster_cel(f, f + 10)).collect();
        delete_frames(&mut s, &frame_set(&[1, 9]));
        assert_eq!(cel_frames(&s), vec![0, 1], "only the valid index (1) is deleted");
    }

    #[test]
    fn delete_frames_clamps_dangling_tag_ranges() {
        let mut s = sprite_with_frames(4);
        s.frame_tags.push(FrameTag {
            name: "all".into(),
            range: FrameRange::new(FrameIndex::new(0), FrameIndex::new(3)),
            loop_direction: LoopDirection::Forward,
            repeat: 0,
            user_data: UserData::default(),
        });
        delete_frames(&mut s, &frame_set(&[3]));
        assert_eq!(s.frames.len(), 3);
        assert_eq!(s.frame_tags[0].range.end.get(), 2, "the tag end is clamped to the new last frame");
    }

    // next_active_after_delete ─────────────────────────────────────────────────

    #[test]
    fn active_index_when_active_frame_is_deleted() {
        // Active frame 3 deleted alongside 1: land on max(0, lowest_deleted - 1)
        // = max(0, 1 - 1) = 0.
        assert_eq!(next_active_after_delete(3, &frame_set(&[1, 3])), 0);
    }

    #[test]
    fn active_index_when_active_frame_is_lowest_deleted() {
        // Deleting only the active frame 2: lowest_deleted - 1 = 1.
        assert_eq!(next_active_after_delete(2, &frame_set(&[2])), 1);
    }

    #[test]
    fn active_index_when_frames_before_active_are_deleted() {
        // Active frame 4 survives; two deleted indices (0, 2) sit below it, so
        // it shifts left by two to 2.
        assert_eq!(next_active_after_delete(4, &frame_set(&[0, 2])), 2);
    }

    #[test]
    fn active_index_unchanged_when_only_later_frames_deleted() {
        // Active frame 1 survives; deleted index 3 is after it, so no shift.
        assert_eq!(next_active_after_delete(1, &frame_set(&[3])), 1);
    }

    #[test]
    fn active_index_deleting_first_frame_floors_at_zero() {
        assert_eq!(next_active_after_delete(0, &frame_set(&[0])), 0);
    }

    // frame clipboard: copy / paste / undo ─────────────────────────────────────
    mod clipboard {
        use pixhaus_core::canvas::PixelBuffer;
        use pixhaus_core::project::{CelData, Frame, IVec2, Rgba, Size};

        use crate::commands::push_sprite_edit_with_buffers;
        use crate::document::DocumentStore;
        use crate::editor::{ClipCel, ClipFrame, EditorState, FrameClipboard};
        use crate::timeline_panel::{build_paste_plan, paste_allowed, shift_frames};

        /// A clipboard of one frame carrying a single raster cel on `layer`, with
        /// the given packed `8*8*4` bytes.
        fn one_frame_clip(layer: pixhaus_core::project::LayerId, bytes: Vec<u8>) -> FrameClipboard {
            FrameClipboard {
                canvas: Size::new(8, 8),
                frames: vec![ClipFrame {
                    frame: Frame::default(),
                    cels: vec![ClipCel {
                        layer_id: layer,
                        position: IVec2::zero(),
                        opacity: 200,
                        bytes,
                        size: Size::new(8, 8),
                    }],
                }],
            }
        }

        #[test]
        fn paste_duplicates_bytes_into_an_independent_buffer() {
            let mut doc = DocumentStore::new();
            doc.create_sprite("hero", Size::new(8, 8));
            let layer = doc.active_sprite().expect("sprite").layers[0].id;
            // Tag the source bytes with a recognisable colour at one pixel.
            let mut src = PixelBuffer::filled(8, 8, Rgba::new(7, 8, 9, 255)).expect("buffer");
            src.set_pixel(2, 3, Rgba::new(200, 100, 50, 255));
            let want = src.clone().into_raw();

            let baseline_frames = doc.frame_count();
            let baseline_buffers = doc.pixel_buffers.len();

            let clip = one_frame_clip(layer, want.clone());
            // Insert after the active frame (0), so the pasted frame is index 1.
            let plan = build_paste_plan(&clip, 1, || {
                pixhaus_core::project::PixelBufferId::new(doc.alloc_id())
            });
            // The plan allocates exactly one fresh buffer, and that id is not the
            // source cel's id — paste never aliases the source.
            assert_eq!(plan.added.len(), 1, "one cel pastes one fresh buffer");
            let pasted_id = plan.added[0].0;
            let source_id = match doc.active_sprite().expect("sprite").cels[0].data {
                CelData::Raster { buffer, .. } => buffer,
                ref other => panic!("seed cel is raster, got {other:?}"),
            };
            assert_ne!(pasted_id, source_id, "the pasted buffer is a fresh id, not the source");

            let added = plan.added.clone();
            let frames = plan.frames.clone();
            let cels = plan.cels.clone();
            let mut editor = EditorState::default();
            push_sprite_edit_with_buffers(&mut editor, &mut doc, "Paste frames", added, |sprite| {
                shift_frames(sprite, 1, frames.len() as i32);
                for (offset, frame) in frames.iter().enumerate() {
                    sprite.frames.insert(1 + offset, frame.clone());
                }
                sprite.cels.extend(cels.iter().cloned());
            });

            assert_eq!(doc.frame_count(), baseline_frames + 1, "frame count grew by one");
            assert_eq!(doc.pixel_buffers.len(), baseline_buffers + 1, "one buffer added");
            let got = doc.pixel_buffers.get(&pasted_id).expect("pasted buffer present").clone().into_raw();
            assert_eq!(got, want, "the pasted cel's bytes equal the copied source bytes");
            // Mutating the pasted buffer must not touch the source: independent.
            let pasted = doc.pixel_buffers.get_mut(&pasted_id).expect("pasted buffer");
            pasted.set_pixel(0, 0, Rgba::new(1, 1, 1, 1));
            assert_ne!(
                doc.pixel_buffers.get(&pasted_id).expect("pasted").pixel(0, 0),
                doc.pixel_buffers.get(&source_id).expect("source").pixel(0, 0),
                "editing the paste does not change the source buffer"
            );
        }

        #[test]
        fn paste_into_a_different_size_sprite_is_rejected() {
            // The size guard is a pure comparison: equal canvases allow, any
            // mismatch rejects (no scaling on paste).
            assert!(paste_allowed(Size::new(8, 8), Size::new(8, 8)), "equal canvases paste");
            assert!(!paste_allowed(Size::new(8, 8), Size::new(16, 16)), "a larger target is rejected");
            assert!(!paste_allowed(Size::new(8, 8), Size::new(8, 16)), "a mismatched height is rejected");
        }

        #[test]
        fn undo_of_paste_removes_frames_and_buffers_to_baseline() {
            let mut doc = DocumentStore::new();
            doc.create_sprite("hero", Size::new(8, 8));
            let layer = doc.active_sprite().expect("sprite").layers[0].id;
            let baseline_frames = doc.frame_count();
            let baseline_buffers = doc.pixel_buffers.len();

            let bytes = PixelBuffer::filled(8, 8, Rgba::new(5, 6, 7, 255)).expect("buffer").into_raw();
            let clip = one_frame_clip(layer, bytes);
            let plan = build_paste_plan(&clip, 1, || {
                pixhaus_core::project::PixelBufferId::new(doc.alloc_id())
            });
            let pasted_id = plan.added[0].0;
            let added = plan.added.clone();
            let frames = plan.frames.clone();
            let cels = plan.cels.clone();

            let mut editor = EditorState::default();
            push_sprite_edit_with_buffers(&mut editor, &mut doc, "Paste frames", added, |sprite| {
                shift_frames(sprite, 1, frames.len() as i32);
                for (offset, frame) in frames.iter().enumerate() {
                    sprite.frames.insert(1 + offset, frame.clone());
                }
                sprite.cels.extend(cels.iter().cloned());
            });
            assert_eq!(doc.frame_count(), baseline_frames + 1);
            assert_eq!(doc.pixel_buffers.len(), baseline_buffers + 1);

            editor.history.undo(&mut doc).expect("undo");
            assert_eq!(doc.frame_count(), baseline_frames, "undo removes the pasted frame");
            assert_eq!(doc.pixel_buffers.len(), baseline_buffers, "undo removes the pasted buffer, no leak");
            assert!(!doc.pixel_buffers.contains_key(&pasted_id), "the pasted buffer id is gone after undo");

            editor.history.redo(&mut doc).expect("redo");
            assert_eq!(doc.frame_count(), baseline_frames + 1, "redo re-pastes the frame");
            assert_eq!(doc.pixel_buffers.len(), baseline_buffers + 1);
            assert!(doc.pixel_buffers.contains_key(&pasted_id), "redo restores the pasted buffer");
        }
    }
}
