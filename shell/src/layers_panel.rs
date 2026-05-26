//! The layers panel: per-layer visibility, lock, blend mode, opacity, inline
//! rename, reorder, grouping, and clipping. Structural edits flow through
//! [`push_sprite_edit`] so each is one undo step.
//!
//! Layers are listed top-first (the visual stacking order) even though the
//! model stores index 0 as the bottom layer.

use eframe::egui;
use pixhaus_core::project::{BlendMode, Layer, LayerId, LayerKind};

use crate::app::ShellApp;
use crate::commands::push_sprite_edit;
use crate::icons;

/// Blend modes offered in the per-layer dropdown, in a sensible grouping.
const BLEND_MODES: &[(BlendMode, &str)] = &[
    (BlendMode::Normal, "Normal"),
    (BlendMode::Multiply, "Multiply"),
    (BlendMode::Screen, "Screen"),
    (BlendMode::Overlay, "Overlay"),
    (BlendMode::Darken, "Darken"),
    (BlendMode::Lighten, "Lighten"),
    (BlendMode::ColorDodge, "Color dodge"),
    (BlendMode::ColorBurn, "Color burn"),
    (BlendMode::HardLight, "Hard light"),
    (BlendMode::SoftLight, "Soft light"),
    (BlendMode::Difference, "Difference"),
    (BlendMode::Exclusion, "Exclusion"),
    (BlendMode::Addition, "Addition"),
    (BlendMode::Subtract, "Subtract"),
    (BlendMode::Hue, "Hue"),
    (BlendMode::Saturation, "Saturation"),
    (BlendMode::Color, "Color"),
    (BlendMode::Luminosity, "Luminosity"),
];

impl ShellApp {
    /// Draws the layers panel.
    #[allow(clippy::too_many_lines)]
    pub(crate) fn layers_panel(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading("Layers");
            if ui.button(icons::ADD).on_hover_text("Add raster layer").clicked() {
                self.add_layer(false);
            }
            if ui.button(icons::GROUP).on_hover_text("Add group").clicked() {
                self.add_layer(true);
            }
        });

        let Some(sprite) = self.doc.active_sprite() else {
            ui.label("No sprite.");
            return;
        };

        // Snapshot the rows we need so we can mutate self afterwards.
        struct Row {
            id: LayerId,
            name: String,
            visible: bool,
            locked: bool,
            opacity: u8,
            blend: BlendMode,
            is_group: bool,
            depth: usize,
        }
        let rows: Vec<Row> = sprite
            .layers
            .iter()
            .rev()
            .map(|l| Row {
                id: l.id,
                name: l.name.clone(),
                visible: l.visible,
                locked: l.locked,
                opacity: l.opacity,
                blend: l.blend_mode,
                is_group: matches!(l.kind, LayerKind::Group { .. }),
                depth: usize::from(l.parent.is_some()),
            })
            .collect();
        let active = self.doc.active_layer;
        let layer_count = rows.len();

        let mut select: Option<LayerId> = None;
        let mut toggle_visible: Option<LayerId> = None;
        let mut toggle_lock: Option<LayerId> = None;
        let mut set_opacity: Option<(LayerId, u8)> = None;
        let mut set_blend: Option<(LayerId, BlendMode)> = None;
        let mut move_up: Option<LayerId> = None;
        let mut move_down: Option<LayerId> = None;
        let mut delete: Option<LayerId> = None;
        let mut commit_rename: Option<(LayerId, String)> = None;

        egui::ScrollArea::vertical().max_height(220.0).id_salt("layers_scroll").show(ui, |ui| {
            for row in &rows {
                let is_active = active == Some(row.id);
                ui.horizontal(|ui| {
                    ui.add_space((row.depth as f32) * 12.0);
                    let eye = if row.visible { icons::EYE } else { icons::EYE_OFF };
                    if ui.small_button(eye).on_hover_text("Visibility").clicked() {
                        toggle_visible = Some(row.id);
                    }
                    let lock = if row.locked { icons::LOCK } else { icons::UNLOCK };
                    if ui.small_button(lock).on_hover_text("Lock").clicked() {
                        toggle_lock = Some(row.id);
                    }

                    // Inline rename when this layer is being renamed.
                    let renaming = self.editor.layer_rename.as_ref().is_some_and(|(id, _)| *id == row.id);
                    if renaming {
                        if let Some((_, draft)) = self.editor.layer_rename.as_mut() {
                            let resp = ui.add(egui::TextEdit::singleline(draft).desired_width(110.0));
                            if resp.lost_focus() {
                                commit_rename = Some((row.id, draft.clone()));
                            }
                        }
                    } else {
                        let mut label = egui::RichText::new(if row.is_group {
                            format!("{} {}", icons::GROUP, row.name)
                        } else {
                            row.name.clone()
                        });
                        if is_active {
                            label = label.strong();
                        }
                        let resp = ui.selectable_label(is_active, label);
                        if resp.clicked() {
                            select = Some(row.id);
                        }
                        if resp.double_clicked() {
                            self.editor.layer_rename = Some((row.id, row.name.clone()));
                        }
                    }
                });

                if is_active && !row.is_group {
                    ui.horizontal(|ui| {
                        ui.add_space(16.0);
                        egui::ComboBox::from_id_salt(("blend", row.id.get()))
                            .width(110.0)
                            .selected_text(blend_label(row.blend))
                            .show_ui(ui, |ui| {
                                for (mode, name) in BLEND_MODES {
                                    let mut current = row.blend;
                                    if ui.selectable_value(&mut current, *mode, *name).clicked() {
                                        set_blend = Some((row.id, *mode));
                                    }
                                }
                            });
                        let mut op = row.opacity;
                        if ui.add(egui::Slider::new(&mut op, 0..=255).text("α")).changed() {
                            set_opacity = Some((row.id, op));
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.add_space(16.0);
                        if ui.small_button(icons::UP).on_hover_text("Move up").clicked() {
                            move_up = Some(row.id);
                        }
                        if ui.small_button(icons::DOWN).on_hover_text("Move down").clicked() {
                            move_down = Some(row.id);
                        }
                        if layer_count > 1 && ui.small_button(icons::TRASH).on_hover_text("Delete").clicked() {
                            delete = Some(row.id);
                        }
                    });
                }
            }
        });

        // Apply the single action chosen this frame.
        if let Some(id) = select {
            self.doc.active_layer = Some(id);
        }
        if let Some(id) = toggle_visible {
            self.edit_layer("Toggle visibility", id, |l| l.visible = !l.visible);
        }
        if let Some(id) = toggle_lock {
            self.edit_layer("Toggle lock", id, |l| l.locked = !l.locked);
        }
        if let Some((id, op)) = set_opacity {
            self.edit_layer("Layer opacity", id, |l| l.opacity = op);
            self.refresh_canvas(false);
        }
        if let Some((id, mode)) = set_blend {
            self.edit_layer("Blend mode", id, |l| l.blend_mode = mode);
            self.refresh_canvas(false);
        }
        if let Some(id) = move_up {
            self.reorder_layer(id, 1);
        }
        if let Some(id) = move_down {
            self.reorder_layer(id, -1);
        }
        if let Some(id) = delete {
            self.delete_layer(id);
        }
        if let Some((id, name)) = commit_rename {
            self.editor.layer_rename = None;
            self.edit_layer("Rename layer", id, |l| l.name = name);
        }
    }

    /// Mutates the layer with `id` through an undoable edit.
    fn edit_layer(&mut self, label: &str, id: LayerId, f: impl FnOnce(&mut Layer)) {
        push_sprite_edit(&mut self.editor, &mut self.doc, label, |sprite| {
            if let Some(l) = sprite.layers.iter_mut().find(|l| l.id == id) {
                f(l);
            }
        });
    }

    /// Adds a new raster or group layer above the active layer and selects it.
    fn add_layer(&mut self, group: bool) {
        let new_id = LayerId::new(self.doc.alloc_id());
        let name = if group { "Group" } else { "Layer" };
        push_sprite_edit(&mut self.editor, &mut self.doc, "Add layer", |sprite| {
            let mut layer = Layer::raster(new_id, name);
            if group {
                layer.kind = LayerKind::Group { collapsed: false };
            }
            sprite.layers.push(layer);
        });
        self.doc.active_layer = Some(new_id);
    }

    /// Deletes the layer and its cels, keeping at least one layer.
    fn delete_layer(&mut self, id: LayerId) {
        let count = self.doc.active_sprite().map_or(0, |s| s.layers.len());
        if count <= 1 {
            return;
        }
        push_sprite_edit(&mut self.editor, &mut self.doc, "Delete layer", |sprite| {
            sprite.layers.retain(|l| l.id != id);
            sprite.cels.retain(|c| c.layer_id != id);
        });
        self.doc.active_layer = self.doc.active_sprite().and_then(|s| s.layers.last().map(|l| l.id));
        self.refresh_canvas(false);
    }

    /// Moves the layer up (`dir > 0`, toward the top) or down within the stack.
    fn reorder_layer(&mut self, id: LayerId, dir: i32) {
        push_sprite_edit(&mut self.editor, &mut self.doc, "Reorder layer", |sprite| {
            let Some(pos) = sprite.layers.iter().position(|l| l.id == id) else {
                return;
            };
            // Model index 0 is the bottom, so "up" is +1 in vec order.
            let target = pos as i32 + dir;
            if target >= 0 && (target as usize) < sprite.layers.len() {
                sprite.layers.swap(pos, target as usize);
            }
        });
        self.refresh_canvas(false);
    }
}

fn blend_label(mode: BlendMode) -> &'static str {
    BLEND_MODES.iter().find(|(m, _)| *m == mode).map_or("Normal", |(_, n)| *n)
}
