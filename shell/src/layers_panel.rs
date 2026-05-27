//! The layers panel: per-layer visibility, lock, blend mode, opacity, inline
//! rename, drag-and-drop reorder and grouping, duplicate, and delete.
//! Structural edits flow through [`push_sprite_edit`] so each is one undo step.
//!
//! Layers are listed top-first (the visual stacking order) even though the
//! model stores index 0 as the bottom layer. Group hierarchy comes from each
//! layer's `parent`; the tree shape and composite order both live in
//! [`pixhaus_core::project::Sprite`].

use eframe::egui;
use pixhaus_core::project::{BlendMode, CelData, Layer, LayerId, LayerKind, PixelBufferId};

use crate::app::ShellApp;
use crate::commands::push_sprite_edit;
use crate::icons;

/// Drag payload carrying the layer being moved.
#[derive(Clone, Copy)]
struct LayerDrag(LayerId);

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

        // Snapshot the rows we need so we can mutate self afterwards. Order and
        // nesting come straight from the model's display tree.
        struct Row {
            id: LayerId,
            name: String,
            visible: bool,
            locked: bool,
            opacity: u8,
            blend: BlendMode,
            is_group: bool,
            collapsed: bool,
            has_children: bool,
            depth: u16,
            parent: Option<LayerId>,
        }
        let rows: Vec<Row> = sprite
            .layer_display_order()
            .into_iter()
            .filter_map(|(id, depth)| {
                let l = sprite.layer(id)?;
                Some(Row {
                    id,
                    name: l.name.clone(),
                    visible: l.visible,
                    locked: l.locked,
                    opacity: l.opacity,
                    blend: l.blend_mode,
                    is_group: l.is_group(),
                    collapsed: l.collapsed(),
                    has_children: sprite.layers.iter().any(|c| c.parent == Some(id)),
                    depth,
                    parent: l.parent,
                })
            })
            .collect();
        let active = self.doc.active_layer;
        let layer_count = sprite.layers.len();

        let mut select: Option<LayerId> = None;
        let mut toggle_visible: Option<LayerId> = None;
        let mut toggle_lock: Option<LayerId> = None;
        let mut toggle_collapse: Option<LayerId> = None;
        let mut set_opacity: Option<(LayerId, u8)> = None;
        let mut set_blend: Option<(LayerId, BlendMode)> = None;
        let mut move_up: Option<LayerId> = None;
        let mut move_down: Option<LayerId> = None;
        let mut delete: Option<LayerId> = None;
        let mut duplicate: Option<LayerId> = None;
        let mut add_inside: Option<LayerId> = None;
        let mut start_rename: Option<(LayerId, String)> = None;
        let mut commit_rename: Option<(LayerId, String)> = None;
        let mut cancel_rename = false;
        // (dragged layer, new parent, anchor sibling to land above).
        let mut layer_move: Option<(LayerId, Option<LayerId>, Option<LayerId>)> = None;

        egui::ScrollArea::vertical().max_height(220.0).id_salt("layers_scroll").show(ui, |ui| {
            for row in &rows {
                let is_active = active == Some(row.id);
                let hint = if row.is_group {
                    crate::dnd::DropHint::Into
                } else {
                    crate::dnd::DropHint::Before
                };
                let ((), payload) = crate::dnd::drop_target::<LayerDrag, _>(ui, hint, |ui| {
                    ui.horizontal(|ui| {
                        ui.add_space(f32::from(row.depth) * 14.0);
                        // Chevron column: groups toggle collapse; others align past it.
                        if row.is_group {
                            let chevron = if row.collapsed { icons::RIGHT } else { icons::DOWN };
                            if ui.add_enabled(row.has_children, egui::Button::new(chevron).frame(false)).clicked() {
                                toggle_collapse = Some(row.id);
                            }
                        } else {
                            ui.add_space(14.0);
                        }

                        let eye = if row.visible { icons::EYE } else { icons::EYE_OFF };
                        if ui.small_button(eye).on_hover_text("Visibility").clicked() {
                            toggle_visible = Some(row.id);
                        }
                        let lock = if row.locked { icons::LOCK } else { icons::UNLOCK };
                        if ui.small_button(lock).on_hover_text("Lock").clicked() {
                            toggle_lock = Some(row.id);
                        }

                        // Inline rename when this layer is the one being renamed.
                        if let Some((rid, draft, needs_focus)) = self.editor.layer_rename.as_mut() {
                            if *rid == row.id {
                                let resp = ui.add(egui::TextEdit::singleline(draft).desired_width(f32::INFINITY));
                                if *needs_focus {
                                    resp.request_focus();
                                    *needs_focus = false;
                                }
                                if resp.lost_focus() {
                                    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                                        cancel_rename = true;
                                    } else {
                                        commit_rename = Some((row.id, draft.clone()));
                                    }
                                }
                                return;
                            }
                        }

                        let label = if row.is_group {
                            egui::RichText::new(format!("{} {}", icons::GROUP, row.name))
                        } else if is_active {
                            egui::RichText::new(row.name.clone()).strong()
                        } else {
                            egui::RichText::new(row.name.clone())
                        };
                        // One widget senses both click and drag; a clean click stays a
                        // click because `drag_started` needs pointer motion.
                        let resp = ui.add(egui::Button::selectable(is_active, label).sense(egui::Sense::click_and_drag()));
                        if resp.clicked() {
                            if row.is_group {
                                toggle_collapse = Some(row.id);
                            } else {
                                select = Some(row.id);
                            }
                        }
                        if resp.double_clicked() {
                            start_rename = Some((row.id, row.name.clone()));
                        }
                        resp.dnd_set_drag_payload(LayerDrag(row.id));
                        resp.context_menu(|ui| {
                            if ui.button(format!("{} Rename", icons::RENAME)).clicked() {
                                start_rename = Some((row.id, row.name.clone()));
                                ui.close();
                            }
                            if row.is_group {
                                if ui.button(format!("{} Add layer inside", icons::ADD)).clicked() {
                                    add_inside = Some(row.id);
                                    ui.close();
                                }
                            } else if ui.button(format!("{} Duplicate", icons::DUPLICATE)).clicked() {
                                duplicate = Some(row.id);
                                ui.close();
                            }
                            ui.separator();
                            if ui.button(format!("{} Move up", icons::UP)).clicked() {
                                move_up = Some(row.id);
                                ui.close();
                            }
                            if ui.button(format!("{} Move down", icons::DOWN)).clicked() {
                                move_down = Some(row.id);
                                ui.close();
                            }
                            ui.separator();
                            if ui.add_enabled(layer_count > 1, egui::Button::new(format!("{} Delete", icons::TRASH))).clicked() {
                                delete = Some(row.id);
                                ui.close();
                            }
                        });
                    });
                });
                // A drop onto a group nests into it; a drop onto a layer reorders
                // the dragged layer directly above it among its siblings.
                if let Some(payload) = payload {
                    let LayerDrag(dragged) = *payload;
                    if dragged != row.id {
                        layer_move = Some(if row.is_group {
                            (dragged, Some(row.id), None)
                        } else {
                            (dragged, row.parent, Some(row.id))
                        });
                    }
                }

                // Blend mode and opacity for the active raster layer.
                if is_active && !row.is_group {
                    ui.horizontal(|ui| {
                        ui.add_space(f32::from(row.depth) * 14.0 + 28.0);
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
                }
            }

            // A slim strip, shown only while dragging, files the layer at top level.
            if let Some(payload) = crate::dnd::top_level_strip::<LayerDrag>(ui, "Move to top level") {
                let LayerDrag(dragged) = *payload;
                layer_move = Some((dragged, None, None));
            }
        });

        // Apply the single action chosen this frame.
        if let Some(id) = select {
            self.doc.active_layer = Some(id);
        }
        if let Some(id) = toggle_visible {
            self.edit_layer("Toggle visibility", id, |l| l.visible = !l.visible);
            self.refresh_canvas(false);
        }
        if let Some(id) = toggle_lock {
            self.edit_layer("Toggle lock", id, |l| l.locked = !l.locked);
        }
        if let Some(id) = toggle_collapse {
            self.toggle_layer_collapse(id);
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
            self.reorder_layer(id, true);
        }
        if let Some(id) = move_down {
            self.reorder_layer(id, false);
        }
        if let Some((dragged, parent, anchor)) = layer_move {
            self.apply_layer_move(dragged, parent, anchor);
        }
        if let Some(id) = duplicate {
            self.duplicate_layer(id);
        }
        if let Some(group) = add_inside {
            self.insert_layer(false, Some(group));
        }
        if let Some(id) = delete {
            self.delete_layer(id);
        }
        if cancel_rename {
            self.editor.layer_rename = None;
        }
        if let Some((id, name)) = commit_rename {
            self.editor.layer_rename = None;
            self.edit_layer("Rename layer", id, |l| l.name = name);
        }
        if let Some((id, name)) = start_rename {
            self.editor.layer_rename = Some((id, name, true));
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

    /// Adds a new raster or group layer at the active layer's level (inside it
    /// when the active layer is a group) and selects it.
    fn add_layer(&mut self, group: bool) {
        let parent = match self.doc.active_layer.and_then(|id| self.doc.active_sprite().and_then(|s| s.layer(id))) {
            Some(l) if l.is_group() => Some(l.id),
            Some(l) => l.parent,
            None => None,
        };
        self.insert_layer(group, parent);
    }

    /// Inserts a raster (or group) layer with the given parent, expands the
    /// parent group so the new layer is visible, and selects it.
    fn insert_layer(&mut self, group: bool, parent: Option<LayerId>) {
        let new_id = LayerId::new(self.doc.alloc_id());
        let name = if group { "Group" } else { "Layer" };
        push_sprite_edit(&mut self.editor, &mut self.doc, "Add layer", |sprite| {
            let mut layer = Layer::raster(new_id, name);
            if group {
                layer.kind = LayerKind::Group { collapsed: false };
            }
            layer.parent = parent;
            sprite.layers.push(layer);
        });
        if let Some(pid) = parent {
            if let Some(sprite) = self.doc.active_sprite_mut() {
                if let Some(l) = sprite.layers.iter_mut().find(|l| l.id == pid) {
                    l.set_collapsed(false);
                }
            }
        }
        self.doc.active_layer = Some(new_id);
    }

    /// Toggles a group layer's collapsed state. Not an undoable edit — it is a
    /// view concern, like folder collapse in the library.
    fn toggle_layer_collapse(&mut self, id: LayerId) {
        if let Some(sprite) = self.doc.active_sprite_mut() {
            if let Some(l) = sprite.layers.iter_mut().find(|l| l.id == id) {
                let collapsed = l.collapsed();
                l.set_collapsed(!collapsed);
            }
        }
    }

    /// Deletes the layer (re-parenting a group's children) and its cels, keeping
    /// at least one layer in the sprite.
    fn delete_layer(&mut self, id: LayerId) {
        let count = self.doc.active_sprite().map_or(0, |s| s.layers.len());
        if count <= 1 {
            return;
        }
        push_sprite_edit(&mut self.editor, &mut self.doc, "Delete layer", |sprite| sprite.remove_layer(id));
        self.doc.active_layer = self.doc.active_sprite().and_then(|s| s.layers.last().map(|l| l.id));
        self.refresh_canvas(false);
    }

    /// Moves the layer one slot toward the top (`up`) or bottom among its
    /// siblings.
    fn reorder_layer(&mut self, id: LayerId, up: bool) {
        push_sprite_edit(&mut self.editor, &mut self.doc, "Reorder layer", |sprite| sprite.move_layer_in_stack(id, up));
        self.refresh_canvas(false);
    }

    /// Applies a drag-and-drop move: reparents `id` under `new_parent` and lands
    /// it above `anchor` (or at the top of its new group when `anchor` is none).
    fn apply_layer_move(&mut self, id: LayerId, new_parent: Option<LayerId>, anchor: Option<LayerId>) {
        push_sprite_edit(&mut self.editor, &mut self.doc, "Move layer", |sprite| {
            sprite.move_layer(id, new_parent, anchor);
        });
        self.refresh_canvas(false);
    }

    /// Duplicates a layer above the original: a fresh layer id, its cels cloned
    /// onto fresh pixel buffers so the copy shares no pixel data. Duplicating a
    /// group copies only the group header, not its children.
    fn duplicate_layer(&mut self, id: LayerId) {
        let Some(sprite) = self.doc.active_sprite() else {
            return;
        };
        let Some(mut new_layer) = sprite.layer(id).cloned() else {
            return;
        };
        let mut new_cels: Vec<_> = sprite.cels.iter().filter(|c| c.layer_id == id).cloned().collect();

        let new_id = LayerId::new(self.doc.alloc_id());
        new_layer.id = new_id;
        new_layer.name = format!("{} copy", new_layer.name);
        for cel in &mut new_cels {
            cel.layer_id = new_id;
            if let CelData::Raster { buffer, .. } = &mut cel.data {
                let fresh = PixelBufferId::new(self.doc.alloc_id());
                if let Some(bytes) = self.doc.pixel_buffers.get(buffer).cloned() {
                    self.doc.pixel_buffers.insert(fresh, bytes);
                }
                *buffer = fresh;
            }
        }

        push_sprite_edit(&mut self.editor, &mut self.doc, "Duplicate layer", move |sprite| {
            let at = sprite.layer_index(id).map_or(sprite.layers.len(), |i| i + 1);
            sprite.layers.insert(at, new_layer);
            sprite.cels.extend(new_cels);
        });
        self.doc.active_layer = Some(new_id);
        self.refresh_canvas(false);
    }
}

fn blend_label(mode: BlendMode) -> &'static str {
    BLEND_MODES.iter().find(|(m, _)| *m == mode).map_or("Normal", |(_, n)| *n)
}
