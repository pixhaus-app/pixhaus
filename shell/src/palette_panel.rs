//! The palette panel: a swatch grid bound to the active sprite's first palette.
//!
//! Click a swatch to set the foreground colour, Ctrl-click to multi-select,
//! right-click for the per-swatch menu (edit / rename / lock / remove), and
//! double-click to open the colour editor. Drag a swatch to reorder it (unless
//! the grid is locked). Auto-add-on-draw, sorting, and a grid lock are the
//! Pixelorama-derived curation aids; see `THIRD_PARTY_NOTICES.md`.

use eframe::egui;
use pixhaus_core::color::space::to_hsv;
use pixhaus_core::project::{Palette, PaletteEntry, Rgba};

use crate::app::ShellApp;
use crate::color_picker::color_picker_ui;
use crate::commands::push_sprite_edit;
use crate::editor::{PaletteSort, to_color32};
use crate::icons;

/// Drag payload carrying the swatch index being moved. Mirrors the layers
/// panel's `LayerDrag`, so the grid reuses the shared `crate::dnd` helpers.
#[derive(Clone, Copy)]
struct SwatchDrag(usize);

impl ShellApp {
    /// Draws the palette panel.
    #[allow(clippy::too_many_lines)]
    pub(crate) fn palette_panel(&mut self, ui: &mut egui::Ui) {
        let Some(palette) = self.doc.active_palette() else {
            ui.label("No palette.");
            return;
        };
        let colors: Vec<Rgba> = palette.colors.iter().map(|e| e.color).collect();
        let names: Vec<Option<String>> = palette.colors.iter().map(|e| e.name.clone()).collect();

        ui.horizontal(|ui| {
            ui.checkbox(&mut self.editor.auto_add_palette, "Auto-add")
                .on_hover_text("Add painted colours to the palette");
            ui.checkbox(&mut self.editor.lock_palette_grid, "Lock")
                .on_hover_text("Lock the grid against drag-to-reorder");
        });
        ui.add(egui::Slider::new(&mut self.editor.swatch_size, 12.0..=32.0).text("swatch"));

        ui.horizontal(|ui| {
            ui.label("Sort:");
            if ui.small_button("Hue").clicked() {
                self.sort_palette(PaletteSort::Hue);
            }
            if ui.small_button("Sat").clicked() {
                self.sort_palette(PaletteSort::Saturation);
            }
            if ui.small_button("Val").clicked() {
                self.sort_palette(PaletteSort::Value);
            }
            if ui.small_button("Lum").clicked() {
                self.sort_palette(PaletteSort::Luminance);
            }
        });

        ui.separator();

        let sw = self.editor.swatch_size;
        let locked_grid = self.editor.lock_palette_grid;
        // Single action chosen this frame, applied after the grid borrow ends.
        let mut set_bg: Option<Rgba> = None;
        let mut edit_idx: Option<usize> = None;
        let mut remove_idx: Option<usize> = None;
        let mut start_rename: Option<usize> = None;
        let mut commit_rename: Option<usize> = None;
        let mut cancel_rename = false;
        let mut toggle_lock: Option<usize> = None;
        let mut toggle_select: Option<usize> = None;
        let mut clear_then_fg: Option<(usize, Rgba)> = None;
        // (from, to): a drag-and-drop reorder applied after the grid borrow.
        let mut reorder: Option<(usize, usize)> = None;

        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(3.0, 3.0);
            for (i, &color) in colors.iter().enumerate() {
                let is_renaming = self.editor.renaming_swatch == Some(i);
                let is_locked = self.editor.locked_swatches.contains(&i);
                let in_selection = self.editor.selected_swatches.contains(&i);

                // The inline rename field replaces the swatch tile in place. The
                // draft commits on Enter / focus loss and discards on Escape;
                // both are applied after the grid borrow ends (see below).
                if is_renaming {
                    let resp = ui.add(egui::TextEdit::singleline(&mut self.editor.rename_draft).desired_width(sw * 4.0));
                    resp.request_focus();
                    if resp.lost_focus() {
                        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                            cancel_rename = true;
                        } else {
                            commit_rename = Some(i);
                        }
                    }
                    continue;
                }

                // A drop target wrapping the swatch tile draws an insertion line
                // when a compatible drag hovers; the released payload reorders.
                let ((), payload) = crate::dnd::drop_target::<SwatchDrag, _>(ui, crate::dnd::DropHint::Before, |ui| {
                    let (rect, resp) = ui.allocate_exact_size(egui::vec2(sw, sw), egui::Sense::click_and_drag());
                    if color.a < 255 {
                        paint_checker(ui, rect);
                    }
                    ui.painter().rect_filled(rect, 2.0, to_color32(color));
                    let selected = in_selection || color == self.editor.fg;
                    let stroke = if selected {
                        egui::Stroke::new(2.0, egui::Color32::WHITE)
                    } else {
                        egui::Stroke::new(1.0, egui::Color32::from_black_alpha(120))
                    };
                    ui.painter().rect_stroke(rect, 2.0, stroke, egui::StrokeKind::Middle);
                    if is_locked {
                        // A lock glyph overlays the locked swatch's corner.
                        ui.painter().text(
                            rect.left_top() + egui::vec2(2.0, 1.0),
                            egui::Align2::LEFT_TOP,
                            icons::LOCK,
                            egui::FontId::proportional(sw * 0.5),
                            egui::Color32::WHITE,
                        );
                    }

                    if resp.clicked() {
                        if ui.input(|i| i.modifiers.command) {
                            toggle_select = Some(i);
                        } else {
                            // A plain click sets the foreground and clears the set.
                            clear_then_fg = Some((i, color));
                        }
                    }
                    if resp.secondary_clicked() {
                        set_bg = Some(color);
                    }
                    if resp.double_clicked() && !is_locked {
                        edit_idx = Some(i);
                    }
                    // Lock the grid or the swatch to refuse the drag source.
                    if !locked_grid && !is_locked {
                        resp.dnd_set_drag_payload(SwatchDrag(i));
                    }
                    let name = names.get(i).and_then(Clone::clone);
                    resp.context_menu(|ui| {
                        if ui.button(format!("{} Edit color", icons::PENCIL)).clicked() {
                            edit_idx = Some(i);
                            ui.close();
                        }
                        if ui.button(format!("{} Rename", icons::RENAME)).clicked() {
                            start_rename = Some(i);
                            ui.close();
                        }
                        let lock_label = if is_locked { "Unlock" } else { "Lock" };
                        let lock_icon = if is_locked { icons::UNLOCK } else { icons::LOCK };
                        if ui.button(format!("{lock_icon} {lock_label}")).clicked() {
                            toggle_lock = Some(i);
                            ui.close();
                        }
                        if ui
                            .add_enabled(!is_locked, egui::Button::new(format!("{} Remove swatch", icons::TRASH)))
                            .on_disabled_hover_text("Unlock the swatch first")
                            .clicked()
                        {
                            remove_idx = Some(i);
                            ui.close();
                        }
                        if let Some(name) = name {
                            ui.separator();
                            ui.label(egui::RichText::new(name).weak());
                        }
                    });
                });
                if let Some(payload) = payload {
                    let SwatchDrag(from) = *payload;
                    if from != i {
                        reorder = Some((from, i));
                    }
                }
            }
        });

        // Apply the single chosen action.
        if let Some((i, c)) = clear_then_fg {
            self.editor.selected_swatches.clear();
            self.editor.selected_swatches.insert(i);
            self.editor.fg = c;
        }
        if let Some(c) = set_bg {
            self.editor.bg = c;
        }
        if let Some(i) = toggle_select {
            if !self.editor.selected_swatches.insert(i) {
                self.editor.selected_swatches.remove(&i);
            }
        }
        if let Some(i) = edit_idx {
            self.editor.editing_swatch = Some(i);
        }
        if cancel_rename {
            self.editor.renaming_swatch = None;
        }
        if let Some(i) = commit_rename {
            self.commit_swatch_rename(i);
        }
        if let Some(i) = start_rename {
            self.editor.renaming_swatch = Some(i);
            self.editor.rename_draft = names.get(i).and_then(Clone::clone).unwrap_or_default();
        }
        if let Some(i) = toggle_lock {
            if !self.editor.locked_swatches.insert(i) {
                self.editor.locked_swatches.remove(&i);
            }
        }
        if let Some(i) = remove_idx {
            self.remove_swatch(i);
        }
        if let Some((from, to)) = reorder {
            // Locked swatches never move and never displace a drop target.
            if !self.editor.locked_swatches.contains(&from) && !self.editor.locked_swatches.contains(&to) {
                push_sprite_edit(&mut self.editor, &mut self.doc, "Reorder swatch", |sprite| {
                    if let Some(p) = sprite.palettes.first_mut() {
                        reorder_in_place(p, from, to);
                    }
                });
            }
        }

        ui.separator();
        ui.horizontal(|ui| {
            if ui.button(format!("{} add fg", icons::ADD)).clicked() {
                let c = self.editor.fg;
                push_sprite_edit(&mut self.editor, &mut self.doc, "Add swatch", |sprite| {
                    if let Some(p) = sprite.palettes.first_mut() {
                        p.colors.push(PaletteEntry::new(c));
                    }
                });
            }
            if !self.editor.selected_swatches.is_empty() && ui.button(format!("{} remove selected", icons::TRASH)).clicked() {
                self.remove_selected_swatches();
            }
        });

        self.swatch_editor(ui);
    }

    /// The popup multi-tab colour editor for the double-clicked swatch.
    ///
    /// The in-progress colour lives in `editor.swatch_scratch`, not the
    /// document: slider drags repaint the swatch live but commit nothing, so a
    /// drag is one undo step. The edit lands as a single
    /// [`push_sprite_edit`] when a slider's drag stops or the hex field loses
    /// focus.
    fn swatch_editor(&mut self, ui: &mut egui::Ui) {
        let Some(idx) = self.editor.editing_swatch else {
            return;
        };
        let current = self.doc.active_palette().and_then(|p| p.colors.get(idx)).map(|e| e.color);
        let Some(current) = current else {
            self.editor.editing_swatch = None;
            self.editor.swatch_scratch = None;
            return;
        };

        // Seed the scratch colour and hex draft the first time the popup opens
        // for this swatch.
        if self.editor.swatch_scratch.is_none() {
            self.editor.swatch_scratch = Some(current);
            self.editor.hex_draft = format!("#{:02X}{:02X}{:02X}", current.r, current.g, current.b);
        }

        let mut color = self.editor.swatch_scratch.unwrap_or(current);
        let mut close = false;
        let mut commit = false;
        egui::Window::new(format!("Edit swatch {idx}"))
            .collapsible(false)
            .resizable(false)
            .show(ui.ctx(), |ui| {
                let resp = color_picker_ui(ui, &mut color, &mut self.editor.picker_tab, &mut self.editor.hex_draft);
                // Commit one undo entry when a slider drag stops or the hex
                // field loses focus — not on every continuous slider tick.
                if resp.drag_stopped() || resp.lost_focus() {
                    commit = true;
                }
                if ui.button("Done").clicked() {
                    commit = true;
                    close = true;
                }
            });

        self.editor.swatch_scratch = Some(color);

        if commit && color != current {
            push_sprite_edit(&mut self.editor, &mut self.doc, "Edit swatch", |sprite| {
                if let Some(p) = sprite.palettes.first_mut() {
                    if let Some(e) = p.colors.get_mut(idx) {
                        e.color = color;
                    }
                }
            });
        }
        if close {
            self.editor.editing_swatch = None;
            self.editor.swatch_scratch = None;
        }
    }

    /// Commits the inline rename draft to the swatch at `idx`. An empty (or
    /// whitespace-only) draft clears the name to `None`, matching the Tauri
    /// panel's `name.trim() || null`.
    fn commit_swatch_rename(&mut self, idx: usize) {
        let trimmed = self.editor.rename_draft.trim();
        let name = if trimmed.is_empty() { None } else { Some(trimmed.to_owned()) };
        push_sprite_edit(&mut self.editor, &mut self.doc, "Rename swatch", |sprite| {
            if let Some(p) = sprite.palettes.first_mut() {
                set_entry_name(p, idx, name);
            }
        });
        self.editor.renaming_swatch = None;
    }

    /// Removes the swatch at `idx`, then prunes the multi-select / lock sets of
    /// stale indices and clamps the open editor index.
    fn remove_swatch(&mut self, idx: usize) {
        push_sprite_edit(&mut self.editor, &mut self.doc, "Remove swatch", |sprite| {
            if let Some(p) = sprite.palettes.first_mut() {
                remove_swatch_at(p, idx);
            }
        });
        self.prune_palette_ui_state();
    }

    /// Removes every selected swatch in one undo step, descending so each
    /// removal leaves the remaining indices valid. Skips locked swatches.
    fn remove_selected_swatches(&mut self) {
        let locked = self.editor.locked_swatches.clone();
        let mut indices: Vec<usize> = self.editor.selected_swatches.iter().copied().filter(|i| !locked.contains(i)).collect();
        indices.sort_unstable_by(|a, b| b.cmp(a));
        if indices.is_empty() {
            return;
        }
        push_sprite_edit(&mut self.editor, &mut self.doc, "Remove swatches", |sprite| {
            if let Some(p) = sprite.palettes.first_mut() {
                for i in &indices {
                    remove_swatch_at(p, *i);
                }
            }
        });
        self.editor.selected_swatches.clear();
        self.prune_palette_ui_state();
    }

    /// Drops swatch-index UI sets that point past the palette and clamps the
    /// open editor index. Called after any swatch removal so the sets never
    /// dangle.
    fn prune_palette_ui_state(&mut self) {
        let len = self.doc.active_palette().map_or(0, |p| p.colors.len());
        self.editor.selected_swatches.retain(|&i| i < len);
        self.editor.locked_swatches.retain(|&i| i < len);
        if self.editor.editing_swatch.is_some_and(|i| i >= len) {
            self.editor.editing_swatch = None;
            self.editor.swatch_scratch = None;
        }
        if self.editor.renaming_swatch.is_some_and(|i| i >= len) {
            self.editor.renaming_swatch = None;
        }
    }

    /// Reorders the active palette's swatches by the requested key.
    fn sort_palette(&mut self, sort: PaletteSort) {
        push_sprite_edit(&mut self.editor, &mut self.doc, "Sort palette", |sprite| {
            if let Some(p) = sprite.palettes.first_mut() {
                match sort {
                    PaletteSort::Hue => p
                        .colors
                        .sort_by(|a, b| hue_key(a.color).partial_cmp(&hue_key(b.color)).unwrap_or(std::cmp::Ordering::Equal)),
                    PaletteSort::Saturation => p
                        .colors
                        .sort_by(|a, b| saturation_key(a.color).partial_cmp(&saturation_key(b.color)).unwrap_or(std::cmp::Ordering::Equal)),
                    PaletteSort::Value => p.colors.sort_by_key(|e| value_key(e.color)),
                    PaletteSort::Luminance => p.colors.sort_by_key(|e| luminance(e.color)),
                }
            }
        });
    }
}

/// Paints a small 2x2 checkerboard inside `rect` to back transparent swatches.
pub(crate) fn paint_checker(ui: &egui::Ui, rect: egui::Rect) {
    let p = ui.painter();
    p.rect_filled(rect, 2.0, egui::Color32::from_gray(90));
    let h = rect.height() / 2.0;
    let w = rect.width() / 2.0;
    let dark = egui::Color32::from_gray(60);
    p.rect_filled(egui::Rect::from_min_size(rect.min, egui::vec2(w, h)), 0.0, dark);
    p.rect_filled(egui::Rect::from_min_size(rect.min + egui::vec2(w, h), egui::vec2(w, h)), 0.0, dark);
}

/// Removes the swatch at `index` from `palette`, returning whether it removed
/// one. An out-of-range index is a no-op. Pure so the panel and tests share one
/// bounds rule.
pub(crate) fn remove_swatch_at(palette: &mut Palette, index: usize) -> bool {
    if index < palette.colors.len() {
        palette.colors.remove(index);
        true
    } else {
        false
    }
}

/// Moves the swatch at `from` to `to` by remove-then-insert, shifting the
/// intermediate entries. Out-of-range indices and `from == to` are no-ops.
/// Ported from the Tauri `reorder_palette_colors_in_place`; the bounds check
/// returns a bool here instead of an error since the panel has nothing to
/// surface.
pub(crate) fn reorder_in_place(palette: &mut Palette, from: usize, to: usize) -> bool {
    let len = palette.colors.len();
    if from >= len || to >= len || from == to {
        return false;
    }
    let entry = palette.colors.remove(from);
    palette.colors.insert(to, entry);
    true
}

/// Sets (or clears, via `None`) the name of the entry at `index`. Out-of-range
/// is a no-op. Pure so the panel and tests share one rule.
pub(crate) fn set_entry_name(palette: &mut Palette, index: usize, name: Option<String>) {
    if let Some(entry) = palette.colors.get_mut(index) {
        entry.name = name;
    }
}

/// HSV hue angle in `0..360` for the hue sort, via the shared `space`
/// conversion.
fn hue_key(c: Rgba) -> f32 {
    to_hsv(c).0
}

/// HSV saturation in `0..=1` for the saturation sort.
fn saturation_key(c: Rgba) -> f32 {
    to_hsv(c).1
}

/// HSV value scaled to `0..=255` for a stable `sort_by_key`.
fn value_key(c: Rgba) -> u32 {
    // value is in 0..=1; scale and round into the integer key. The clamp keeps
    // the cast in range, so truncation and sign loss cannot occur.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    {
        (to_hsv(c).2 * 255.0).round().clamp(0.0, 255.0) as u32
    }
}

/// Rec.601 luminance scaled to `0..=255` for sorting.
fn luminance(c: Rgba) -> u32 {
    (u32::from(c.r) * 299 + u32::from(c.g) * 587 + u32::from(c.b) * 114) / 1000
}

#[cfg(test)]
mod tests {
    use pixhaus_core::project::{Palette, PaletteId, Rgba};
    use rstest::rstest;

    use super::{luminance, remove_swatch_at, reorder_in_place, saturation_key, set_entry_name, value_key};

    fn palette_with_named_colors(names: &[&str]) -> Palette {
        let mut p = Palette::from_colors(PaletteId::new(1), "main", vec![Rgba::opaque(0, 0, 0); names.len()]);
        for (entry, name) in p.colors.iter_mut().zip(names) {
            entry.name = Some((*name).into());
        }
        p
    }

    fn names_of(palette: &Palette) -> Vec<&str> {
        palette.colors.iter().map(|c| c.name.as_deref().unwrap_or("")).collect()
    }

    // ── remove_swatch_at ──────────────────────────────────────────────────

    #[rstest]
    #[case(0, true, vec!["b", "c"])]
    #[case(1, true, vec!["a", "c"])]
    #[case(2, true, vec!["a", "b"])] // last index
    #[case(3, false, vec!["a", "b", "c"])] // out of range: no-op
    #[case(99, false, vec!["a", "b", "c"])]
    fn remove_swatch_at_is_bounds_checked(#[case] idx: usize, #[case] removed: bool, #[case] expect: Vec<&str>) {
        let mut p = palette_with_named_colors(&["a", "b", "c"]);
        assert_eq!(remove_swatch_at(&mut p, idx), removed);
        assert_eq!(names_of(&p), expect);
    }

    // ── reorder_in_place ──────────────────────────────────────────────────

    #[test]
    fn reorder_forward_shifts_intermediate_entries_left() {
        let mut p = palette_with_named_colors(&["a", "b", "c", "d"]);
        assert!(reorder_in_place(&mut p, 0, 2));
        assert_eq!(names_of(&p), vec!["b", "c", "a", "d"]);
    }

    #[test]
    fn reorder_backward_shifts_intermediate_entries_right() {
        let mut p = palette_with_named_colors(&["a", "b", "c", "d"]);
        assert!(reorder_in_place(&mut p, 3, 1));
        assert_eq!(names_of(&p), vec!["a", "d", "b", "c"]);
    }

    #[test]
    fn reorder_same_index_is_a_noop() {
        let mut p = palette_with_named_colors(&["a", "b", "c"]);
        assert!(!reorder_in_place(&mut p, 1, 1));
        assert_eq!(names_of(&p), vec!["a", "b", "c"]);
    }

    #[test]
    fn reorder_from_index_out_of_range_is_a_noop() {
        let mut p = palette_with_named_colors(&["a", "b"]);
        assert!(!reorder_in_place(&mut p, 5, 0));
        assert_eq!(names_of(&p), vec!["a", "b"]);
    }

    #[test]
    fn reorder_to_index_out_of_range_is_a_noop() {
        let mut p = palette_with_named_colors(&["a", "b"]);
        assert!(!reorder_in_place(&mut p, 0, 9));
        assert_eq!(names_of(&p), vec!["a", "b"]);
    }

    // ── set_entry_name ────────────────────────────────────────────────────

    #[test]
    fn set_entry_name_sets_a_name() {
        let mut p = palette_with_named_colors(&["a", "b"]);
        set_entry_name(&mut p, 1, Some("outline".into()));
        assert_eq!(names_of(&p), vec!["a", "outline"]);
    }

    #[test]
    fn set_entry_name_clears_with_none() {
        let mut p = palette_with_named_colors(&["a", "b"]);
        set_entry_name(&mut p, 0, None);
        assert_eq!(p.colors[0].name, None);
        assert_eq!(p.colors[1].name.as_deref(), Some("b"));
    }

    #[test]
    fn set_entry_name_out_of_range_is_a_noop() {
        let mut p = palette_with_named_colors(&["a"]);
        set_entry_name(&mut p, 9, Some("x".into()));
        assert_eq!(names_of(&p), vec!["a"]);
    }

    // ── sort keys: monotonicity ───────────────────────────────────────────

    #[test]
    fn saturation_key_orders_gray_below_a_saturated_color() {
        // A neutral gray has ~zero saturation; a pure hue is near 1.
        let gray = saturation_key(Rgba::opaque(128, 128, 128));
        let saturated = saturation_key(Rgba::opaque(200, 20, 20));
        assert!(gray < saturated, "gray {gray} should sort before saturated {saturated}");
    }

    #[test]
    fn value_key_orders_black_below_white() {
        assert!(value_key(Rgba::opaque(0, 0, 0)) < value_key(Rgba::opaque(255, 255, 255)));
        // A mid gray sits between.
        let mid = value_key(Rgba::opaque(128, 128, 128));
        assert!(value_key(Rgba::opaque(0, 0, 0)) < mid && mid < value_key(Rgba::opaque(255, 255, 255)));
    }

    #[test]
    fn luminance_orders_black_below_white() {
        assert!(luminance(Rgba::opaque(0, 0, 0)) < luminance(Rgba::opaque(255, 255, 255)));
    }
}
