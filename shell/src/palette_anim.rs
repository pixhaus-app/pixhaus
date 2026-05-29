//! The palette panel's Pages and Animation sections.
//!
//! Both hang in collapsing sections below the generator tools and depend on the
//! switcher's selected palette (`editor.active_palette_id`). Pages are named
//! subset views over the palette's entries (membership is a set of swatch
//! indices); animation keyframes a swatch's colour per timeline frame and a
//! "Cycle range" control rotates a swatch span. Every edit routes one
//! [`push_sprite_edit`] so it undoes as a single step. The maths lives on the
//! `core` palette types ([`PaletteAnimation`], [`Palette::apply_cycle`]); this
//! module is the UI plus the pure page / resolved-colour helpers it tests.

use eframe::egui;
use pixhaus_core::project::{FrameIndex, Palette, PaletteAnimation, PalettePage, PalettePageId, Rgba};

use crate::app::ShellApp;
use crate::commands::push_sprite_edit;
use crate::document::palette_by_id_mut;
use crate::editor::to_color32;
use crate::palette_panel::paint_checker;

/// Appends a page named `name` to `palette` under the caller-allocated `id`,
/// returning whether it added one. A whitespace-only name is rejected (no-op),
/// matching the palette/sprite/group rename validation. The id is allocated
/// before the surrounding [`push_sprite_edit`] so the closure references a
/// stable id. Pure so the panel and its test share one rule.
pub(crate) fn add_page(palette: &mut Palette, id: PalettePageId, name: &str) -> bool {
    let name = name.trim();
    if name.is_empty() {
        return false;
    }
    palette.pages.push(PalettePage::new(id, name));
    true
}

/// Replaces the membership of the page with `page_id` with `indices`, returning
/// whether it set them. Rejects (no-op) when any index is out of range against
/// `palette.colors.len()` or the page is absent — mirroring the Tauri
/// `page_set_entries_in_doc` bounds rule, so a page can never reference a swatch
/// the palette does not hold. Pure so the panel and its test share one rule.
pub(crate) fn set_page_entries(palette: &mut Palette, page_id: PalettePageId, indices: &[u32]) -> bool {
    let len = palette.colors.len();
    // usize::try_from cannot fail here on a 32/64-bit target (u32 fits usize),
    // but the explicit guard keeps the bound total without a cast lint.
    if indices.iter().any(|&i| usize::try_from(i).map_or(true, |i| i >= len)) {
        return false;
    }
    let Some(page) = palette.pages.iter_mut().find(|p| p.id == page_id) else {
        return false;
    };
    page.entry_indices = indices.to_vec();
    true
}

/// Removes the page with `page_id` from `palette`, returning whether it removed
/// one. An absent id is a no-op. Pure so the panel and its test share one rule.
pub(crate) fn remove_page(palette: &mut Palette, page_id: PalettePageId) -> bool {
    let before = palette.pages.len();
    palette.pages.retain(|p| p.id != page_id);
    palette.pages.len() != before
}

/// The colour the swatch at `index` resolves to at `frame`: its animated colour
/// when the palette carries a [`PaletteAnimation`] with a keyframe at or before
/// `frame` for that entry, else the entry's own stored colour. Out-of-range
/// indices resolve to [`Rgba::transparent`] (there is no entry to read). Pure so
/// the panel's per-swatch resolved-colour paint and its test share one mapping.
#[must_use]
pub(crate) fn resolved_entry_color(palette: &Palette, index: usize, frame: FrameIndex) -> Rgba {
    let Some(entry) = palette.colors.get(index) else {
        return Rgba::transparent();
    };
    let fallback = entry.color;
    // index is a palette position; u32 covers every realistic palette length.
    let Ok(idx) = u32::try_from(index) else {
        return fallback;
    };
    palette.animation.as_ref().map_or(fallback, |anim| anim.resolve(idx, frame, fallback))
}

impl ShellApp {
    /// Draws the Pages and Animation sections. A no-op when there is no active
    /// palette.
    pub(crate) fn palette_pages_and_animation(&mut self, ui: &mut egui::Ui) {
        if self.doc.active_palette_by_id(self.editor.active_palette_id).is_none() {
            return;
        }
        self.pages_section(ui);
        self.animation_section(ui);
    }

    /// The "Pages" collapsing section: a selector over the active palette's
    /// pages, Add / Rename / Remove, and per-swatch membership toggles for the
    /// selected page. Selecting a page can filter the grid to its members.
    #[allow(clippy::too_many_lines)]
    fn pages_section(&mut self, ui: &mut egui::Ui) {
        let sel = self.editor.active_palette_id;
        let Some(palette) = self.doc.active_palette_by_id(sel) else {
            return;
        };
        let pages: Vec<(PalettePageId, String)> = palette.pages.iter().map(|p| (p.id, p.name.clone())).collect();
        let color_count = palette.colors.len();
        // The selected page's membership, by swatch index, for the checkbox column.
        let members: Vec<u32> = self
            .editor
            .active_page_id
            .and_then(|id| palette.pages.iter().find(|p| p.id == id))
            .map(|p| p.entry_indices.clone())
            .unwrap_or_default();
        let colors: Vec<Rgba> = palette.colors.iter().map(|e| e.color).collect();

        // Re-validate the page selection against the live page list so a remove
        // or a palette switch never leaves the selector on a dropped page.
        let selected = self.editor.active_page_id.filter(|id| pages.iter().any(|(pid, _)| pid == id));
        self.editor.active_page_id = selected;

        egui::CollapsingHeader::new("Pages").id_salt("palette_pages").default_open(false).show(ui, |ui| {
            // Actions chosen this frame, applied after the read borrows end.
            let mut select: Option<Option<PalettePageId>> = None;
            let mut add = false;
            let mut rename: Option<PalettePageId> = None;
            let mut remove: Option<PalettePageId> = None;
            let mut toggle_member: Option<usize> = None;

            ui.horizontal(|ui| {
                let selected_name = selected
                    .and_then(|id| pages.iter().find(|(pid, _)| *pid == id))
                    .map_or_else(|| "(no page)".to_owned(), |(_, name)| name.clone());
                egui::ComboBox::from_id_salt("palette_page_selector")
                    .selected_text(selected_name)
                    .show_ui(ui, |ui| {
                        if ui.selectable_label(selected.is_none(), "(no page)").clicked() {
                            select = Some(None);
                        }
                        for (id, name) in &pages {
                            if ui.selectable_label(selected == Some(*id), name).clicked() {
                                select = Some(Some(*id));
                            }
                        }
                    });
                ui.checkbox(&mut self.editor.filter_to_page, "Filter")
                    .on_hover_text("Show only the selected page's swatches");
            });

            // Add / Rename / Remove, driven by the shared name draft.
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut self.editor.page_name_draft).hint_text("page name").desired_width(120.0));
                if ui.button(format!("{} Add", crate::icons::ADD)).clicked() {
                    add = true;
                }
                if let Some(id) = selected {
                    if ui.button(format!("{} Rename", crate::icons::RENAME)).clicked() {
                        rename = Some(id);
                    }
                    if ui.button(format!("{} Remove", crate::icons::TRASH)).clicked() {
                        remove = Some(id);
                    }
                }
            });

            // Membership: a checkbox per swatch when a page is selected. Ticking
            // adds the index to the page; unticking removes it. Bounds are
            // enforced by `set_page_entries` on commit.
            if selected.is_some() {
                ui.label(egui::RichText::new("Members:").weak());
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(3.0, 3.0);
                    for (i, &color) in colors.iter().enumerate() {
                        // i fits u32 for any realistic palette length.
                        let in_page = u32::try_from(i).is_ok_and(|idx| members.contains(&idx));
                        let (rect, resp) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::click());
                        if color.a < 255 {
                            paint_checker(ui, rect);
                        }
                        ui.painter().rect_filled(rect, 2.0, to_color32(color));
                        let stroke = if in_page {
                            egui::Stroke::new(2.0, egui::Color32::WHITE)
                        } else {
                            egui::Stroke::new(1.0, egui::Color32::from_black_alpha(120))
                        };
                        ui.painter().rect_stroke(rect, 2.0, stroke, egui::StrokeKind::Middle);
                        if resp.clicked() {
                            toggle_member = Some(i);
                        }
                    }
                });
            }

            // Apply the single chosen action after the borrows release.
            if let Some(id) = select {
                self.editor.active_page_id = id;
            }
            if add {
                self.add_palette_page();
            }
            if let Some(id) = rename {
                self.rename_palette_page(id);
            }
            if let Some(id) = remove {
                self.remove_palette_page(id);
            }
            if let Some(i) = toggle_member {
                self.toggle_page_member(i, &members, color_count);
            }
        });
    }

    /// Allocates a fresh [`PalettePageId`], appends a page named from the draft,
    /// and selects it. The id is allocated before [`push_sprite_edit`] so the
    /// closure references a stable id (the allocate-then-edit rule). A blank
    /// draft is rejected by [`add_page`], leaving the palette unchanged.
    fn add_palette_page(&mut self) {
        let name = self.editor.page_name_draft.trim().to_owned();
        if name.is_empty() {
            return;
        }
        let id = PalettePageId::new(self.doc.alloc_id());
        let sel = self.editor.active_palette_id;
        push_sprite_edit(&mut self.editor, &mut self.doc, "Add page", |sprite| {
            if let Some(p) = palette_by_id_mut(sprite, sel) {
                add_page(p, id, &name);
            }
        });
        self.editor.active_page_id = Some(id);
        self.editor.page_name_draft.clear();
    }

    /// Renames the page with `id` from the draft. A blank draft is rejected
    /// (no-op), matching the add path.
    fn rename_palette_page(&mut self, id: PalettePageId) {
        let name = self.editor.page_name_draft.trim().to_owned();
        if name.is_empty() {
            return;
        }
        let sel = self.editor.active_palette_id;
        push_sprite_edit(&mut self.editor, &mut self.doc, "Rename page", |sprite| {
            if let Some(p) = palette_by_id_mut(sprite, sel) {
                if let Some(page) = p.pages.iter_mut().find(|pg| pg.id == id) {
                    page.name = name;
                }
            }
        });
        self.editor.page_name_draft.clear();
    }

    /// Removes the page with `id` and clears the selection when it was the
    /// selected one.
    fn remove_palette_page(&mut self, id: PalettePageId) {
        let sel = self.editor.active_palette_id;
        push_sprite_edit(&mut self.editor, &mut self.doc, "Remove page", |sprite| {
            if let Some(p) = palette_by_id_mut(sprite, sel) {
                remove_page(p, id);
            }
        });
        if self.editor.active_page_id == Some(id) {
            self.editor.active_page_id = None;
        }
    }

    /// Toggles swatch `index`'s membership in the selected page, committing the
    /// updated index set through [`set_page_entries`] (which rejects an
    /// out-of-range set against `color_count`). `current` is the page's
    /// membership this frame; the toggle adds or removes `index` from it.
    fn toggle_page_member(&mut self, index: usize, current: &[u32], color_count: usize) {
        let Some(page_id) = self.editor.active_page_id else {
            return;
        };
        if index >= color_count {
            return;
        }
        let Ok(idx) = u32::try_from(index) else {
            return;
        };
        let mut next: Vec<u32> = current.to_vec();
        if let Some(pos) = next.iter().position(|&i| i == idx) {
            next.remove(pos);
        } else {
            next.push(idx);
            next.sort_unstable();
        }
        let sel = self.editor.active_palette_id;
        push_sprite_edit(&mut self.editor, &mut self.doc, "Set page members", |sprite| {
            if let Some(p) = palette_by_id_mut(sprite, sel) {
                set_page_entries(p, page_id, &next);
            }
        });
    }

    /// The "Animation" collapsing section: keyframe a swatch's colour at the
    /// active frame, remove a keyframe, see each swatch's resolved colour at the
    /// current frame, and rotate a swatch range with "Cycle range".
    #[allow(clippy::too_many_lines)]
    fn animation_section(&mut self, ui: &mut egui::Ui) {
        let sel = self.editor.active_palette_id;
        let Some(palette) = self.doc.active_palette_by_id(sel) else {
            return;
        };
        let frame = self.doc.active_frame;
        let color_count = palette.colors.len();
        // Each swatch's colour resolved at the current frame, for the readout.
        let resolved: Vec<Rgba> = (0..color_count).map(|i| resolved_entry_color(palette, i, frame)).collect();
        // The swatch the keyframe controls target: the open editor's index, else
        // the lowest selected swatch, else the first entry.
        let target = self
            .editor
            .editing_swatch
            .or_else(|| self.editor.selected_swatches.iter().next().copied())
            .filter(|&i| i < color_count);
        let fg = self.editor.fg;

        egui::CollapsingHeader::new("Animation").id_salt("palette_animation").default_open(false).show(ui, |ui| {
            if color_count == 0 {
                ui.label("Add a colour to keyframe.");
                return;
            }

            let mut set_keyframe: Option<usize> = None;
            let mut remove_keyframe: Option<usize> = None;
            let mut cycle = false;

            ui.label(format!("Frame {}", frame.get()));
            match target {
                Some(idx) => {
                    ui.horizontal(|ui| {
                        if ui
                            .button(format!("{} Set keyframe", crate::icons::FILM))
                            .on_hover_text("Keyframe the foreground colour for the target swatch at this frame")
                            .clicked()
                        {
                            set_keyframe = Some(idx);
                        }
                        if ui.button("Remove keyframe").clicked() {
                            remove_keyframe = Some(idx);
                        }
                    });
                    ui.label(egui::RichText::new(format!("Target swatch {idx}")).weak());
                }
                None => {
                    ui.label(egui::RichText::new("Select a swatch to keyframe it.").weak());
                }
            }

            // The resolved-at-frame readout: each swatch's colour at the current
            // frame, animation applied.
            ui.label(egui::RichText::new("Resolved at frame:").weak());
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(3.0, 3.0);
                for &color in &resolved {
                    let (rect, _resp) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::hover());
                    if color.a < 255 {
                        paint_checker(ui, rect);
                    }
                    ui.painter().rect_filled(rect, 2.0, to_color32(color));
                    ui.painter()
                        .rect_stroke(rect, 2.0, egui::Stroke::new(1.0, egui::Color32::from_black_alpha(120)), egui::StrokeKind::Middle);
                }
            });

            ui.separator();

            // Colour cycling: rotate a swatch range. Clamp the stored indices to
            // the live palette before they drive the controls.
            let max_index = color_count - 1;
            self.editor.cycle_first = self.editor.cycle_first.min(max_index);
            self.editor.cycle_last = self.editor.cycle_last.min(max_index);
            ui.label(egui::RichText::new("Cycle range:").weak());
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut self.editor.cycle_first).range(0..=max_index).prefix("first "));
                ui.add(egui::DragValue::new(&mut self.editor.cycle_last).range(0..=max_index).prefix("last "));
                ui.add(egui::DragValue::new(&mut self.editor.cycle_offset).prefix("offset "));
            });
            if ui
                .add(egui::Button::new(format!("{} Cycle palette", crate::icons::REPEAT)))
                .on_hover_text("Rotate the range's swatches by the offset, in one undo step")
                .clicked()
            {
                cycle = true;
            }

            // Apply the chosen action after the borrows release.
            if let Some(idx) = set_keyframe {
                self.set_palette_keyframe(idx, frame, fg);
            }
            if let Some(idx) = remove_keyframe {
                self.remove_palette_keyframe(idx, frame);
            }
            if cycle {
                self.cycle_palette();
            }
        });
    }

    /// Sets a keyframe for swatch `idx` at `frame` to `color`, creating the
    /// palette's [`PaletteAnimation`] on first use.
    fn set_palette_keyframe(&mut self, idx: usize, frame: FrameIndex, color: Rgba) {
        let Ok(entry_index) = u32::try_from(idx) else {
            return;
        };
        let sel = self.editor.active_palette_id;
        push_sprite_edit(&mut self.editor, &mut self.doc, "Set palette keyframe", |sprite| {
            if let Some(p) = palette_by_id_mut(sprite, sel) {
                p.animation.get_or_insert_with(PaletteAnimation::default).set(entry_index, frame, color);
            }
        });
    }

    /// Removes the keyframe for swatch `idx` at `frame`, pruning empty inner
    /// tables and collapsing the palette's animation to `None` once the whole
    /// keyframe map empties (mirroring the Tauri collapse-to-None).
    fn remove_palette_keyframe(&mut self, idx: usize, frame: FrameIndex) {
        let Ok(entry_index) = u32::try_from(idx) else {
            return;
        };
        let sel = self.editor.active_palette_id;
        push_sprite_edit(&mut self.editor, &mut self.doc, "Remove palette keyframe", |sprite| {
            if let Some(p) = palette_by_id_mut(sprite, sel) {
                prune_keyframe(p, entry_index, frame);
            }
        });
    }

    /// Rotates the cycle range's swatches by the offset, committing
    /// [`Palette::apply_cycle`]'s rotated palette in one undo step. A zero offset
    /// or an invalid range is a no-op inside `apply_cycle`.
    fn cycle_palette(&mut self) {
        let first = u8::try_from(self.editor.cycle_first).unwrap_or(u8::MAX);
        let last = u8::try_from(self.editor.cycle_last).unwrap_or(u8::MAX);
        let offset = self.editor.cycle_offset;
        let sel = self.editor.active_palette_id;
        push_sprite_edit(&mut self.editor, &mut self.doc, "Cycle palette", move |sprite| {
            if let Some(p) = palette_by_id_mut(sprite, sel) {
                *p = p.apply_cycle(first, last, offset);
            }
        });
    }
}

/// Removes the keyframe for `entry_index` at `frame` from `palette`'s animation,
/// dropping the entry's inner table when it empties and collapsing the whole
/// animation to `None` when no entry keyframes remain. A no-op when the palette
/// has no animation. Pure so the panel and its test share one rule.
pub(crate) fn prune_keyframe(palette: &mut Palette, entry_index: u32, frame: FrameIndex) {
    let Some(anim) = palette.animation.as_mut() else {
        return;
    };
    if let Some(per_entry) = anim.keyframes.get_mut(&entry_index) {
        per_entry.remove(&frame);
        if per_entry.is_empty() {
            anim.keyframes.remove(&entry_index);
        }
    }
    if anim.keyframes.is_empty() {
        palette.animation = None;
    }
}

#[cfg(test)]
mod tests {
    use pixhaus_core::project::{FrameIndex, Palette, PaletteAnimation, PaletteId, PalettePageId, Rgba};
    use rstest::rstest;

    use super::{add_page, prune_keyframe, remove_page, resolved_entry_color, set_page_entries};

    fn palette(len: usize) -> Palette {
        Palette::from_colors(PaletteId::new(1), "main", vec![Rgba::opaque(10, 20, 30); len])
    }

    fn page_id(n: u32) -> PalettePageId {
        PalettePageId::new(n)
    }

    // ── add_page ──────────────────────────────────────────────────────────

    #[test]
    fn add_page_appends_a_named_page() {
        let mut p = palette(3);
        assert!(add_page(&mut p, page_id(7), "warm"));
        assert_eq!(p.pages.len(), 1);
        assert_eq!(p.pages[0].id, page_id(7));
        assert_eq!(p.pages[0].name, "warm");
        assert!(p.pages[0].entry_indices.is_empty());
    }

    #[rstest]
    #[case("")]
    #[case("   ")]
    fn add_page_rejects_a_blank_name(#[case] name: &str) {
        let mut p = palette(3);
        assert!(!add_page(&mut p, page_id(7), name));
        assert!(p.pages.is_empty());
    }

    // ── set_page_entries ──────────────────────────────────────────────────

    #[test]
    fn set_page_entries_sets_in_range_indices() {
        let mut p = palette(4);
        add_page(&mut p, page_id(1), "a");
        assert!(set_page_entries(&mut p, page_id(1), &[0, 2, 3]));
        assert_eq!(p.pages[0].entry_indices, vec![0, 2, 3]);
    }

    #[test]
    fn set_page_entries_rejects_an_out_of_range_index() {
        let mut p = palette(3);
        add_page(&mut p, page_id(1), "a");
        // Index 3 is past the 3-colour palette (valid indices are 0..=2).
        assert!(!set_page_entries(&mut p, page_id(1), &[0, 3]));
        assert!(p.pages[0].entry_indices.is_empty(), "a rejected set leaves the membership unchanged");
    }

    #[test]
    fn set_page_entries_rejects_an_absent_page() {
        let mut p = palette(3);
        assert!(!set_page_entries(&mut p, page_id(99), &[0]));
    }

    #[test]
    fn set_page_entries_accepts_an_empty_set() {
        let mut p = palette(3);
        add_page(&mut p, page_id(1), "a");
        set_page_entries(&mut p, page_id(1), &[0, 1]);
        assert!(set_page_entries(&mut p, page_id(1), &[]), "clearing membership is in range");
        assert!(p.pages[0].entry_indices.is_empty());
    }

    // ── remove_page ───────────────────────────────────────────────────────

    #[test]
    fn remove_page_drops_the_page() {
        let mut p = palette(2);
        add_page(&mut p, page_id(1), "a");
        add_page(&mut p, page_id(2), "b");
        assert!(remove_page(&mut p, page_id(1)));
        assert_eq!(p.pages.len(), 1);
        assert_eq!(p.pages[0].id, page_id(2));
    }

    #[test]
    fn remove_page_absent_id_is_a_noop() {
        let mut p = palette(2);
        add_page(&mut p, page_id(1), "a");
        assert!(!remove_page(&mut p, page_id(99)));
        assert_eq!(p.pages.len(), 1);
    }

    // ── resolved_entry_color ──────────────────────────────────────────────

    #[test]
    fn resolved_entry_color_uses_the_stored_color_without_animation() {
        let p = palette(2);
        assert_eq!(resolved_entry_color(&p, 0, FrameIndex::new(5)), Rgba::opaque(10, 20, 30));
    }

    #[test]
    fn resolved_entry_color_applies_a_keyframe_at_or_before_the_frame() {
        let mut p = palette(2);
        let keyed = Rgba::opaque(200, 0, 0);
        let mut anim = PaletteAnimation::default();
        anim.set(1, FrameIndex::new(3), keyed);
        p.animation = Some(anim);

        // Entry 1 before its first key falls back to the stored colour.
        assert_eq!(resolved_entry_color(&p, 1, FrameIndex::new(2)), Rgba::opaque(10, 20, 30));
        // At and after the key, it resolves to the keyed colour (step semantics).
        assert_eq!(resolved_entry_color(&p, 1, FrameIndex::new(3)), keyed);
        assert_eq!(resolved_entry_color(&p, 1, FrameIndex::new(9)), keyed);
        // Entry 0 has no key, so it keeps its stored colour at the same frame.
        assert_eq!(resolved_entry_color(&p, 0, FrameIndex::new(9)), Rgba::opaque(10, 20, 30));
    }

    #[test]
    fn resolved_entry_color_out_of_range_is_transparent() {
        let p = palette(1);
        assert_eq!(resolved_entry_color(&p, 9, FrameIndex::new(0)), Rgba::transparent());
    }

    // ── prune_keyframe ────────────────────────────────────────────────────

    #[test]
    fn prune_keyframe_collapses_animation_to_none_when_empty() {
        let mut p = palette(2);
        let mut anim = PaletteAnimation::default();
        anim.set(0, FrameIndex::new(4), Rgba::opaque(1, 2, 3));
        p.animation = Some(anim);

        prune_keyframe(&mut p, 0, FrameIndex::new(4));
        assert!(p.animation.is_none(), "removing the last keyframe drops the animation table");
    }

    #[test]
    fn prune_keyframe_keeps_other_keyframes() {
        let mut p = palette(2);
        let mut anim = PaletteAnimation::default();
        anim.set(0, FrameIndex::new(1), Rgba::opaque(1, 0, 0));
        anim.set(0, FrameIndex::new(5), Rgba::opaque(0, 1, 0));
        anim.set(1, FrameIndex::new(2), Rgba::opaque(0, 0, 1));
        p.animation = Some(anim);

        prune_keyframe(&mut p, 0, FrameIndex::new(1));
        let anim = p.animation.as_ref().expect("animation survives a partial prune");
        // Entry 0 keeps its frame-5 key; entry 1 is untouched.
        assert_eq!(anim.resolve(0, FrameIndex::new(5), Rgba::transparent()), Rgba::opaque(0, 1, 0));
        assert_eq!(anim.resolve(0, FrameIndex::new(1), Rgba::transparent()), Rgba::transparent(), "the frame-1 key is gone");
        assert_eq!(anim.resolve(1, FrameIndex::new(2), Rgba::transparent()), Rgba::opaque(0, 0, 1));
    }

    #[test]
    fn prune_keyframe_without_animation_is_a_noop() {
        let mut p = palette(2);
        prune_keyframe(&mut p, 0, FrameIndex::new(0));
        assert!(p.animation.is_none());
    }
}
