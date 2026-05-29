//! The palette panel: a swatch grid bound to the active sprite's selected
//! palette.
//!
//! A switcher (combo box plus add / delete / rename / reorder) picks which of
//! the sprite's palettes the panel shows and edits; the selection lives in
//! [`crate::editor::EditorState::active_palette_id`] and resolves through
//! [`DocumentStore::active_palette_by_id`], falling back to the first palette.
//!
//! Click a swatch to set the foreground colour, Ctrl-click to multi-select,
//! right-click for the per-swatch menu (edit / rename / lock / remove), and
//! double-click to open the colour editor. Drag a swatch to reorder it (unless
//! the grid is locked). Auto-add-on-draw, sorting, and a grid lock are the
//! Pixelorama-derived curation aids; see `THIRD_PARTY_NOTICES.md`.

use std::collections::HashSet;
use std::path::Path;

use eframe::egui;
use pixhaus_core::canvas::PixelBuffer;
use pixhaus_core::color::palette_file::{aco, gpl, hex, pal, PaletteFileError, PaletteFileResult};
use pixhaus_core::color::space::to_hsv;
use pixhaus_core::project::{CelData, ColorMode, Palette, PaletteEntry, PaletteId, PixelBufferId, Rgba};

use crate::app::ShellApp;
use crate::color_picker::color_picker_ui;
use crate::commands::{push_sprite_edit, CanvasBufferSwap, CanvasEdit};
use crate::document::palette_by_id_mut;
use crate::editor::{PaletteExportFormat, PaletteSort, to_color32};
use crate::icons;

/// Drag payload carrying the swatch index being moved. Mirrors the layers
/// panel's `LayerDrag`, so the grid reuses the shared `crate::dnd` helpers.
#[derive(Clone, Copy)]
struct SwatchDrag(usize);

impl ShellApp {
    /// Draws the palette panel.
    #[allow(clippy::too_many_lines)]
    pub(crate) fn palette_panel(&mut self, ui: &mut egui::Ui) {
        // The switcher both selects the active palette and exposes palette CRUD.
        // Draw it before reading the palette so a create/delete this frame picks
        // the right one below.
        self.palette_switcher(ui);

        let sel = self.editor.active_palette_id;
        let Some(palette) = self.doc.active_palette_by_id(sel) else {
            ui.label("No palette.");
            return;
        };
        let colors: Vec<Rgba> = palette.colors.iter().map(|e| e.color).collect();
        let names: Vec<Option<String>> = palette.colors.iter().map(|e| e.name.clone()).collect();

        // When a page is selected and the page filter is on, restrict the grid to
        // that page's member indices; otherwise every swatch shows. `None` means
        // "show all", a tiny set of indices means "show only these".
        let page_filter: Option<HashSet<usize>> = if self.editor.filter_to_page {
            self.editor
                .active_page_id
                .and_then(|id| palette.pages.iter().find(|p| p.id == id))
                .map(|page| page.entry_indices.iter().filter_map(|&i| usize::try_from(i).ok()).collect())
        } else {
            None
        };

        // Indexed-mode affordances: number the swatches, flag index 0 as the
        // transparent index, and read the foreground back as an index. The
        // labels are gated on the mode so an RGBA sprite renders unchanged.
        let is_indexed = self.doc.active_sprite().is_some_and(|s| s.color_mode == ColorMode::Indexed);
        let fg_index = if is_indexed { palette.nearest_index(self.editor.fg) } else { None };

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

        if is_indexed {
            // The foreground's resolved index, the draw layer's index readout.
            let label = fg_index.map_or_else(|| "Index: -".to_owned(), |i| format!("Index: {i}"));
            ui.label(egui::RichText::new(label).weak());
        }

        // The recents strip: the most-recently-painted colours, newest first.
        // Clicking one re-selects it as the foreground. UI-only, never persisted.
        self.recents_strip(ui);

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
                // Hide swatches outside the active page when the page filter is on.
                if page_filter.as_ref().is_some_and(|set| !set.contains(&i)) {
                    continue;
                }
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
                    // In indexed mode, index 0 is the transparent index: show the
                    // checker through it whatever colour the entry stores.
                    let transparent_index = is_indexed && i == 0;
                    if color.a < 255 || transparent_index {
                        paint_checker(ui, rect);
                    }
                    if !transparent_index {
                        ui.painter().rect_filled(rect, 2.0, to_color32(color));
                    }
                    let selected = in_selection || color == self.editor.fg;
                    let stroke = if selected {
                        egui::Stroke::new(2.0, egui::Color32::WHITE)
                    } else {
                        egui::Stroke::new(1.0, egui::Color32::from_black_alpha(120))
                    };
                    ui.painter().rect_stroke(rect, 2.0, stroke, egui::StrokeKind::Middle);
                    if is_indexed {
                        // Number each swatch in its corner, in a contrast colour
                        // so the label reads on light and dark entries alike. The
                        // transparent index reads "0/T".
                        let label = entry_index_label(i, transparent_index);
                        let ink = if transparent_index { egui::Color32::WHITE } else { contrast_ink(color) };
                        ui.painter().text(
                            rect.right_top() + egui::vec2(-2.0, 1.0),
                            egui::Align2::RIGHT_TOP,
                            label,
                            egui::FontId::proportional(sw * 0.4),
                            ink,
                        );
                    }
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
                    if let Some(p) = palette_by_id_mut(sprite, sel) {
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
                    if let Some(p) = palette_by_id_mut(sprite, sel) {
                        p.colors.push(PaletteEntry::new(c));
                    }
                });
            }
            if !self.editor.selected_swatches.is_empty() && ui.button(format!("{} remove selected", icons::TRASH)).clicked() {
                self.remove_selected_swatches();
            }
        });

        ui.horizontal(|ui| {
            let has_colors = self.doc.active_palette_by_id(sel).is_some_and(|p| !p.colors.is_empty());
            if ui
                .add_enabled(has_colors, egui::Button::new(format!("{} Reduce to palette", icons::PALETTE)))
                .on_hover_text("Snap every opaque pixel on the active frame to its nearest palette colour")
                .on_disabled_hover_text("Add a colour to the palette first")
                .clicked()
            {
                self.reduce_to_palette();
            }
        });

        self.swatch_editor(ui);

        ui.separator();
        self.palette_tools(ui);

        ui.separator();
        self.palette_pages_and_animation(ui);

        ui.separator();
        self.import_export_section(ui);
    }

    /// The palette switcher: a combo box over the active sprite's palettes plus
    /// add / delete / rename / reorder controls. Selecting writes
    /// `editor.active_palette_id`; create allocates a fresh [`PaletteId`] before
    /// the edit and points the selection at it; delete refuses the last palette
    /// and re-points the selection at the first survivor. A no-op (draws a hint)
    /// when there is no active sprite.
    fn palette_switcher(&mut self, ui: &mut egui::Ui) {
        // Re-validate the selection against the live palettes so a sprite switch
        // or a delete never leaves the combo pointing at a dropped palette. The
        // combo and every edit resolve through the same id, so seed it here.
        let Some(sprite) = self.doc.active_sprite() else {
            return;
        };
        let palettes: Vec<(PaletteId, String)> = sprite.palettes.iter().map(|p| (p.id, p.name.clone())).collect();
        if palettes.is_empty() {
            return;
        }
        let selected = self
            .editor
            .active_palette_id
            .filter(|id| palettes.iter().any(|(pid, _)| pid == id))
            .or_else(|| palettes.first().map(|(id, _)| *id));
        self.editor.active_palette_id = selected;

        let selected_name = selected
            .and_then(|id| palettes.iter().find(|(pid, _)| *pid == id))
            .map_or_else(|| "—".to_owned(), |(_, name)| name.clone());

        // Actions chosen this frame, applied after the borrow on `palettes` ends.
        let mut select: Option<PaletteId> = None;
        let mut create = false;
        let mut delete = false;
        let mut swap: Option<(usize, usize)> = None;

        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("palette_switcher")
                .selected_text(selected_name)
                .show_ui(ui, |ui| {
                    for (id, name) in &palettes {
                        if ui.selectable_label(selected == Some(*id), name).clicked() {
                            select = Some(*id);
                        }
                    }
                });

            if ui.button(icons::ADD).on_hover_text("New palette").clicked() {
                create = true;
            }
            // Deleting the last palette is refused, so disable the button there.
            let can_delete = palettes.len() > 1;
            if ui
                .add_enabled(can_delete, egui::Button::new(icons::TRASH))
                .on_hover_text("Delete palette")
                .on_disabled_hover_text("A sprite keeps at least one palette")
                .clicked()
            {
                delete = true;
            }

            // Reorder the selected palette among its siblings.
            let cur = selected.and_then(|id| palettes.iter().position(|(pid, _)| *pid == id));
            if let Some(idx) = cur {
                if ui.add_enabled(idx > 0, egui::Button::new(icons::UP)).on_hover_text("Move up").clicked() {
                    swap = Some((idx, idx - 1));
                }
                if ui
                    .add_enabled(idx + 1 < palettes.len(), egui::Button::new(icons::DOWN))
                    .on_hover_text("Move down")
                    .clicked()
                {
                    swap = Some((idx, idx + 1));
                }
            }
        });

        // Inline rename of the selected palette: an empty name is rejected by
        // `rename_palette`, so a blank field leaves the name unchanged.
        if let Some(id) = selected {
            let mut name = palettes
                .iter()
                .find(|(pid, _)| *pid == id)
                .map_or_else(String::new, |(_, n)| n.clone());
            if ui.add(egui::TextEdit::singleline(&mut name).hint_text("palette name").desired_width(f32::INFINITY)).changed() {
                push_sprite_edit(&mut self.editor, &mut self.doc, "Rename palette", |sprite| {
                    sprite.rename_palette(id, &name);
                });
            }
        }

        if let Some(id) = select {
            self.editor.active_palette_id = Some(id);
        }
        if create {
            self.create_palette();
        }
        if delete {
            self.delete_palette();
        }
        if let Some((a, b)) = swap {
            push_sprite_edit(&mut self.editor, &mut self.doc, "Reorder palette", move |sprite| {
                if a < sprite.palettes.len() && b < sprite.palettes.len() {
                    sprite.palettes.swap(a, b);
                }
            });
        }

        ui.separator();
    }

    /// Allocates a fresh [`PaletteId`], appends an empty palette under it, and
    /// selects it. The id is allocated before [`push_sprite_edit`] so the edit
    /// closure references a stable id, matching the playbook's allocate-then-edit
    /// rule for id-allocating CRUD.
    fn create_palette(&mut self) {
        let id = PaletteId::new(self.doc.alloc_id());
        let n = self.doc.active_sprite().map_or(0, |s| s.palettes.len());
        let name = format!("Palette {}", n + 1);
        push_sprite_edit(&mut self.editor, &mut self.doc, "Add palette", move |sprite| {
            sprite.add_palette(id, name);
        });
        self.editor.active_palette_id = Some(id);
    }

    /// Deletes the selected palette (refusing the sprite's last) and re-points
    /// the selection at the first survivor, dropping the per-swatch UI sets that
    /// referred to the old palette's indices.
    fn delete_palette(&mut self) {
        let Some(id) = self.editor.active_palette_id else {
            return;
        };
        push_sprite_edit(&mut self.editor, &mut self.doc, "Delete palette", move |sprite| {
            sprite.delete_palette(id);
        });
        // Point the selection at the first remaining palette, then prune the
        // swatch UI sets against the now-current palette's length.
        self.editor.active_palette_id = self.doc.active_sprite().and_then(|s| s.palettes.first()).map(|p| p.id);
        self.editor.selected_swatches.clear();
        self.editor.locked_swatches.clear();
        self.editor.editing_swatch = None;
        self.editor.swatch_scratch = None;
        self.editor.renaming_swatch = None;
    }

    /// The recent-colours strip: the most-recently-painted colours, newest
    /// first, each a small clickable swatch. Clicking one sets it as the
    /// foreground. A no-op (draws nothing) until at least one colour has been
    /// committed this session. Session-only state, never serialized.
    fn recents_strip(&mut self, ui: &mut egui::Ui) {
        if self.editor.recent_colors.is_empty() {
            return;
        }
        let recents: Vec<Rgba> = self.editor.recent_colors.iter().copied().collect();
        let mut pick: Option<Rgba> = None;
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(3.0, 3.0);
            ui.label(egui::RichText::new("Recent:").weak());
            for color in recents {
                let (rect, resp) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::click());
                if color.a < 255 {
                    paint_checker(ui, rect);
                }
                ui.painter().rect_filled(rect, 2.0, to_color32(color));
                ui.painter()
                    .rect_stroke(rect, 2.0, egui::Stroke::new(1.0, egui::Color32::from_black_alpha(120)), egui::StrokeKind::Middle);
                if resp.clicked() {
                    pick = Some(color);
                }
            }
        });
        if let Some(color) = pick {
            self.editor.fg = color;
        }
    }

    /// The "Import / Export" section: read a `.gpl`/`.hex`/`.pal`/`.aco` file
    /// into the active palette, or write the active palette to a `.gpl`/`.hex`/
    /// `.pal` file. Both buttons open the OS-native [`rfd`] dialog synchronously
    /// on the click — that blocks on the OS dialog, not on app work, and holds
    /// no lock across an `.await` (the shell is synchronous egui). The last
    /// error shows inline; v2 has no toast system.
    fn import_export_section(&mut self, ui: &mut egui::Ui) {
        let has_palette = self.doc.active_palette_by_id(self.editor.active_palette_id).is_some();
        egui::CollapsingHeader::new("Import / Export").id_salt("palette_io").default_open(false).show(ui, |ui| {
            ui.horizontal(|ui| {
                if ui.add_enabled(has_palette, egui::Button::new(format!("{} Import...", icons::IMAGE))).clicked() {
                    self.import_palette();
                }
            });

            ui.horizontal(|ui| {
                ui.label("Export as:");
                for fmt in [PaletteExportFormat::Gpl, PaletteExportFormat::Hex, PaletteExportFormat::Pal] {
                    ui.selectable_value(&mut self.editor.export_format, fmt, fmt.label());
                }
                if ui.add_enabled(has_palette, egui::Button::new(format!("{} Export...", icons::COPY))).clicked() {
                    self.export_palette();
                }
            });

            if let Some(err) = &self.editor.palette_io_error {
                ui.colored_label(egui::Color32::from_rgb(220, 90, 90), err);
            }
        });
    }

    /// Opens a file dialog, parses the picked palette file by extension, and
    /// appends its colours to the active palette in one undo step. A parse
    /// failure or a cancelled dialog leaves the palette unchanged; a failure is
    /// surfaced inline, a cancel is silent.
    fn import_palette(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_title("Import palette")
            .add_filter("Palette", &["gpl", "hex", "pal", "aco"])
            .pick_file()
        else {
            // A cancelled dialog is not an error; leave the prior state.
            return;
        };

        match parse_palette_file(&path) {
            Ok(colors) => {
                self.editor.palette_io_error = None;
                let sel = self.editor.active_palette_id;
                push_sprite_edit(&mut self.editor, &mut self.doc, "Import palette", |sprite| {
                    if let Some(p) = palette_by_id_mut(sprite, sel) {
                        p.colors.extend(colors.into_iter().map(PaletteEntry::new));
                    }
                });
            }
            Err(e) => {
                self.editor.palette_io_error = Some(format!("Import failed: {e}"));
            }
        }
    }

    /// Opens a save dialog seeded from the palette name, encodes the active
    /// palette in the selected format, and writes it to disk. An I/O failure is
    /// surfaced inline; a cancelled dialog is silent.
    fn export_palette(&mut self) {
        let Some(palette) = self.doc.active_palette_by_id(self.editor.active_palette_id) else {
            return;
        };
        let name = palette.name.clone();
        let colors: Vec<Rgba> = palette.colors.iter().map(|e| e.color).collect();
        let fmt = self.editor.export_format;

        let slug = palette_slug(&name);
        let Some(path) = rfd::FileDialog::new()
            .set_title("Export palette")
            .set_file_name(format!("{slug}.{ext}", ext = fmt.extension()))
            .add_filter(fmt.label(), &[fmt.extension()])
            .save_file()
        else {
            return;
        };

        let bytes: Vec<u8> = match fmt {
            PaletteExportFormat::Gpl => gpl::encode(&name, &colors).into_bytes(),
            PaletteExportFormat::Hex => hex::encode(&colors).into_bytes(),
            PaletteExportFormat::Pal => pal::encode_jasc(&colors).into_bytes(),
        };

        match std::fs::write(&path, bytes) {
            Ok(()) => self.editor.palette_io_error = None,
            Err(e) => self.editor.palette_io_error = Some(format!("Export failed: {e}")),
        }
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
        let sel = self.editor.active_palette_id;
        let current = self.doc.active_palette_by_id(sel).and_then(|p| p.colors.get(idx)).map(|e| e.color);
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
                if let Some(p) = palette_by_id_mut(sprite, sel) {
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
        let sel = self.editor.active_palette_id;
        push_sprite_edit(&mut self.editor, &mut self.doc, "Rename swatch", |sprite| {
            if let Some(p) = palette_by_id_mut(sprite, sel) {
                set_entry_name(p, idx, name);
            }
        });
        self.editor.renaming_swatch = None;
    }

    /// Removes the swatch at `idx`, then prunes the multi-select / lock sets of
    /// stale indices and clamps the open editor index.
    fn remove_swatch(&mut self, idx: usize) {
        let sel = self.editor.active_palette_id;
        push_sprite_edit(&mut self.editor, &mut self.doc, "Remove swatch", |sprite| {
            if let Some(p) = palette_by_id_mut(sprite, sel) {
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
        let sel = self.editor.active_palette_id;
        push_sprite_edit(&mut self.editor, &mut self.doc, "Remove swatches", |sprite| {
            if let Some(p) = palette_by_id_mut(sprite, sel) {
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
        let len = self.doc.active_palette_by_id(self.editor.active_palette_id).map_or(0, |p| p.colors.len());
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
        let sel = self.editor.active_palette_id;
        push_sprite_edit(&mut self.editor, &mut self.doc, "Sort palette", |sprite| {
            if let Some(p) = palette_by_id_mut(sprite, sel) {
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

    /// Snaps every opaque pixel on the active frame to its nearest palette
    /// colour, rewriting each raster cel's buffer in place. Routes through one
    /// [`CanvasEdit`] covering every touched buffer — not [`push_sprite_edit`] —
    /// because the edit changes pixel bytes, not the [`Sprite`] structure. The
    /// before/after sprite values are identical; only the buffers swap.
    ///
    /// A no-op when there is no active sprite, the palette is empty, or the
    /// quantization leaves every buffer byte-identical (already on-palette).
    fn reduce_to_palette(&mut self) {
        let Some(sprite_id) = self.doc.project.active_sprite_id() else {
            return;
        };
        let Some(sprite) = self.doc.project.sprite(sprite_id).cloned() else {
            return;
        };
        // Reduce to the switcher's selected palette (falling back to the first),
        // matching what the panel shows. An empty palette has no nearest entry;
        // nothing to reduce to.
        let sel = self.editor.active_palette_id;
        let Some(palette) = sel
            .and_then(|id| sprite.palette(id))
            .or_else(|| sprite.palettes.first())
            .filter(|p| !p.colors.is_empty())
            .cloned()
        else {
            return;
        };

        // The distinct raster buffers the active frame draws from, resolving
        // linked cels to their owning source so a shared drawing is quantized
        // once.
        let frame = self.doc.active_frame;
        let mut ids: Vec<PixelBufferId> = Vec::new();
        let mut seen: HashSet<PixelBufferId> = HashSet::new();
        for entry in sprite.composite_order() {
            let source = sprite.resolve_source_frame(entry.layer, frame);
            let Some(cel) = sprite.cel(entry.layer, source) else {
                continue;
            };
            if let CelData::Raster { buffer, .. } = cel.data {
                if seen.insert(buffer) {
                    ids.push(buffer);
                }
            }
        }

        // Quantize each buffer into a swap, dropping the ones the pass leaves
        // byte-identical so an already-on-palette frame records no entry.
        let mut buffers: Vec<CanvasBufferSwap> = Vec::new();
        for id in ids {
            let Some(before) = self.doc.pixel_buffers.get(&id).cloned() else {
                continue;
            };
            let mut after = before.clone();
            quantize_buffer(&mut after, &palette);
            if after != before {
                buffers.push(CanvasBufferSwap { id, before, after });
            }
        }
        if buffers.is_empty() {
            return;
        }

        // The structure is unchanged; only the pixel bytes move.
        let edit = CanvasEdit {
            sprite_id,
            before_sprite: sprite.clone(),
            after_sprite: sprite,
            buffers,
            label: "Reduce to palette".to_owned(),
        };
        let _ = self.editor.history.push(Box::new(edit), &mut self.doc);
        self.refresh_canvas(false);
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

/// Snaps every opaque pixel in `buf` to its nearest `palette` entry, in place.
///
/// For each pixel with non-zero alpha, maps the colour to
/// [`Palette::nearest_index`] and writes back the entry's colour, keeping the
/// pixel's own alpha. Alpha-0 pixels are left byte-identical (a transparent
/// pixel has no colour to quantize). An empty palette is a no-op:
/// `nearest_index` returns `None`, so there is nothing to map to.
///
/// Works directly on the buffer's `Vec<u8>` stride rows — four bytes per pixel,
/// no intermediate `Vec<Vec<u8>>` — so the cost is bounded by the buffer's pixel
/// count, not the canvas (the 8K constraint).
pub(crate) fn quantize_buffer(buf: &mut PixelBuffer, palette: &Palette) {
    // An empty palette has no nearest entry; skip the whole pass.
    if palette.colors.is_empty() {
        return;
    }
    for y in 0..buf.height() {
        let Some(row) = buf.row_mut(y) else { continue };
        for px in row.chunks_exact_mut(4) {
            // px is a 4-byte RGBA chunk: leave fully-transparent pixels alone.
            if px[3] == 0 {
                continue;
            }
            let here = Rgba::new(px[0], px[1], px[2], px[3]);
            // nearest_index ignores alpha on both sides and returns Some(_) for a
            // non-empty palette, so the get is in range; keep the pixel's alpha.
            if let Some(idx) = palette.nearest_index(here) {
                if let Some(entry) = palette.colors.get(idx) {
                    let c = entry.color;
                    px[0] = c.r;
                    px[1] = c.g;
                    px[2] = c.b;
                }
            }
        }
    }
}

/// Reads `path` and parses it into a colour list, dispatching on the lowercase
/// file extension. `.gpl` drops the parsed palette name (the panel appends into
/// the active palette and keeps its name); `.pal` sniffs RIFF vs JASC via
/// [`parse_pal`]. An unknown extension or a read failure maps to
/// [`PaletteFileError::Invalid`].
fn parse_palette_file(path: &Path) -> PaletteFileResult<Vec<Rgba>> {
    let bytes = std::fs::read(path).map_err(|e| PaletteFileError::Invalid(format!("could not read file: {e}")))?;
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();

    match ext.as_str() {
        "gpl" => {
            let text = bytes_as_utf8(&bytes)?;
            gpl::parse(text).map(|(_name, colors)| colors)
        }
        "hex" => {
            let text = bytes_as_utf8(&bytes)?;
            hex::parse(text)
        }
        "pal" => parse_pal(&bytes),
        "aco" => aco::parse(&bytes),
        other => Err(PaletteFileError::Invalid(format!("unsupported palette extension '{other}'"))),
    }
}

/// Sniffs a `.pal` buffer: the binary RIFF form first (its parser checks the
/// `RIFF` magic and rejects anything else), then the JASC text form. Both
/// formats share the `.pal` extension, so the extension alone cannot
/// disambiguate them. Returns the RIFF error only when the bytes start with the
/// RIFF magic but are otherwise malformed; an absent magic falls through to
/// JASC so a JASC file's RIFF error never masks the real one.
pub(crate) fn parse_pal(bytes: &[u8]) -> PaletteFileResult<Vec<Rgba>> {
    if bytes.starts_with(b"RIFF") {
        // The bytes claim to be RIFF; surface the RIFF parser's error rather
        // than a misleading "missing JASC-PAL header".
        return pal::parse_riff(bytes);
    }
    let text = bytes_as_utf8(bytes)?;
    pal::parse_jasc(text)
}

/// Borrows `bytes` as UTF-8 text for the text palette parsers, mapping a
/// non-UTF-8 buffer to [`PaletteFileError::Invalid`] rather than panicking.
fn bytes_as_utf8(bytes: &[u8]) -> PaletteFileResult<&str> {
    std::str::from_utf8(bytes).map_err(|_| PaletteFileError::Invalid("file is not valid UTF-8 text".into()))
}

/// A filesystem-safe stem for the export dialog's default file name, derived
/// from the palette name. Non-alphanumeric runs collapse to single hyphens; an
/// empty result falls back to `"palette"`.
fn palette_slug(name: &str) -> String {
    let slug: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if slug.is_empty() { "palette".to_owned() } else { slug }
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

/// The corner label for the swatch at `index` in an indexed-mode palette. The
/// transparent index reads `"0/T"`; every other index is its number. Pure so
/// the panel and its test share one rule.
#[must_use]
pub(crate) fn entry_index_label(index: usize, is_transparent: bool) -> String {
    if is_transparent { format!("{index}/T") } else { index.to_string() }
}

/// Picks black or white text for a label drawn over `bg`, whichever reads with
/// more contrast — black on a light swatch, white on a dark one. Uses the same
/// Rec.601 luminance the sort key does, thresholded at the mid grey.
fn contrast_ink(bg: Rgba) -> egui::Color32 {
    if luminance(bg) > 140 { egui::Color32::BLACK } else { egui::Color32::WHITE }
}

#[cfg(test)]
mod tests {
    use pixhaus_core::canvas::PixelBuffer;
    use pixhaus_core::color::palette_file::pal;
    use pixhaus_core::project::{Palette, PaletteId, Rgba};
    use rstest::rstest;

    use super::{
        entry_index_label, luminance, palette_slug, parse_pal, quantize_buffer, remove_swatch_at, reorder_in_place, saturation_key, set_entry_name, value_key,
    };

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

    // ── parse_pal: RIFF vs JASC sniff ─────────────────────────────────────

    fn sample_colors() -> Vec<Rgba> {
        vec![Rgba::opaque(255, 0, 0), Rgba::opaque(0, 255, 0), Rgba::opaque(0, 0, 255)]
    }

    #[test]
    fn parse_pal_sniffs_a_riff_fixture() {
        // A RIFF .pal buffer (binary magic `RIFF`) routes to parse_riff.
        let bytes = pal::encode_riff(&sample_colors()).expect("encode riff fixture");
        assert!(bytes.starts_with(b"RIFF"), "fixture must carry the RIFF magic");
        assert_eq!(parse_pal(&bytes).expect("parse riff via sniff"), sample_colors());
    }

    #[test]
    fn parse_pal_sniffs_a_jasc_fixture() {
        // A JASC .pal buffer (text, no RIFF magic) falls through to parse_jasc.
        let text = pal::encode_jasc(&sample_colors());
        let bytes = text.into_bytes();
        assert!(!bytes.starts_with(b"RIFF"), "fixture must not carry the RIFF magic");
        assert_eq!(parse_pal(&bytes).expect("parse jasc via sniff"), sample_colors());
    }

    #[test]
    fn parse_pal_surfaces_the_riff_error_for_a_truncated_riff() {
        // Bytes that start with RIFF but are too short must report the RIFF
        // failure, not a misleading "missing JASC-PAL header".
        let err = parse_pal(b"RIFF\x00\x00\x00\x00").expect_err("truncated RIFF must error");
        assert!(err.to_string().contains("RIFF"), "expected a RIFF error, got: {err}");
    }

    #[test]
    fn parse_pal_rejects_garbage_that_is_neither_format() {
        // No RIFF magic and no JASC header -> the JASC parser's error.
        assert!(parse_pal(b"not a palette at all").is_err());
    }

    // ── palette_slug ──────────────────────────────────────────────────────

    #[rstest]
    #[case("My Palette", "my-palette")]
    #[case("NES", "nes")]
    #[case("  spaces  ", "spaces")]
    #[case("a!!!b", "a-b")]
    #[case("", "palette")]
    #[case("***", "palette")]
    fn palette_slug_is_filesystem_safe(#[case] name: &str, #[case] expect: &str) {
        assert_eq!(palette_slug(name), expect);
    }

    // ── entry_index_label ─────────────────────────────────────────────────

    #[rstest]
    #[case(0, true, "0/T")] // index 0 is the transparent index
    #[case(0, false, "0")] // a non-indexed-transparent 0 is just its number
    #[case(5, false, "5")]
    #[case(255, false, "255")]
    fn entry_index_label_flags_the_transparent_index(#[case] index: usize, #[case] is_transparent: bool, #[case] expect: &str) {
        assert_eq!(entry_index_label(index, is_transparent), expect);
    }

    // ── quantize_buffer ───────────────────────────────────────────────────

    /// A two-colour palette: near-black and near-white, plus a transparent
    /// index 0 to prove transparent entries never win an opaque pixel.
    fn quantize_palette() -> Palette {
        Palette::from_colors(
            PaletteId::new(1),
            "bw",
            vec![Rgba::transparent(), Rgba::opaque(0, 0, 0), Rgba::opaque(255, 255, 255)],
        )
    }

    #[test]
    fn quantize_buffer_snaps_opaque_pixels_to_nearest_entries() {
        // A 2x2 of off-palette colours: two lean dark, two lean light.
        let mut buf = PixelBuffer::new(2, 2).expect("buffer");
        buf.set_pixel(0, 0, Rgba::opaque(30, 30, 30)); // -> black
        buf.set_pixel(1, 0, Rgba::opaque(200, 210, 220)); // -> white
        buf.set_pixel(0, 1, Rgba::opaque(10, 5, 8)); // -> black
        buf.set_pixel(1, 1, Rgba::opaque(240, 245, 250)); // -> white

        quantize_buffer(&mut buf, &quantize_palette());

        assert_eq!(buf.pixel(0, 0), Some(Rgba::opaque(0, 0, 0)));
        assert_eq!(buf.pixel(1, 0), Some(Rgba::opaque(255, 255, 255)));
        assert_eq!(buf.pixel(0, 1), Some(Rgba::opaque(0, 0, 0)));
        assert_eq!(buf.pixel(1, 1), Some(Rgba::opaque(255, 255, 255)));
    }

    #[test]
    fn quantize_buffer_leaves_transparent_pixels_untouched() {
        let mut buf = PixelBuffer::new(2, 1).expect("buffer");
        // A fully-transparent pixel carrying stray RGB must survive byte-for-byte.
        let ghost = Rgba::new(123, 45, 67, 0);
        buf.set_pixel(0, 0, ghost);
        buf.set_pixel(1, 0, Rgba::opaque(40, 40, 40)); // opaque -> black

        quantize_buffer(&mut buf, &quantize_palette());

        assert_eq!(buf.pixel(0, 0), Some(ghost), "alpha-0 pixel left untouched");
        assert_eq!(buf.pixel(1, 0), Some(Rgba::opaque(0, 0, 0)), "opaque pixel snapped");
    }

    #[test]
    fn quantize_buffer_with_empty_palette_is_a_noop() {
        let mut buf = PixelBuffer::filled(2, 2, Rgba::opaque(13, 17, 19)).expect("buffer");
        let before = buf.as_bytes().to_vec();
        quantize_buffer(&mut buf, &Palette::from_colors(PaletteId::new(2), "empty", vec![]));
        assert_eq!(buf.as_bytes(), before.as_slice(), "an empty palette changes nothing");
    }
}
