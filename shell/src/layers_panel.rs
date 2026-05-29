//! The layers panel: per-layer visibility, lock, blend mode, opacity, inline
//! rename, drag-and-drop reorder and grouping, duplicate, and delete.
//! Structural edits flow through [`push_sprite_edit`] so each is one undo step.
//!
//! Layers are listed top-first (the visual stacking order) even though the
//! model stores index 0 as the bottom layer. Group hierarchy comes from each
//! layer's `parent`; the tree shape and composite order both live in
//! [`pixhaus_core::project::Sprite`].

use eframe::egui;
use pixhaus_core::project::sprite::LayerComposite;
use pixhaus_core::project::{BlendMode, CelData, Layer, LayerId, LayerKind, PixelBufferId, Sprite};

use crate::app::ShellApp;
use crate::commands::push_sprite_edit;
use crate::icons;

/// Drag payload carrying the layer being moved.
#[derive(Clone, Copy)]
struct LayerDrag(LayerId);

/// A single layer row's display snapshot, cloned out of the model so the panel
/// can mutate `self` after rendering. Built lazily for the visible range only
/// (see [`build_rows`]); a long layer list never materializes a row per layer.
struct Row {
    id: LayerId,
    name: String,
    visible: bool,
    locked: bool,
    is_group: bool,
    collapsed: bool,
    has_children: bool,
    depth: u16,
    parent: Option<LayerId>,
    can_merge_down: bool,
    clip_below: bool,
}

/// Builds row snapshots for the `range` slice of `display` (the top-first
/// display order) only. `display` and `order` are computed once per frame; this
/// slices them, so the work and the returned `Vec` are bounded by the visible
/// range, not the total layer count. `order` is the back-to-front composite
/// order used to decide whether a row can merge down.
fn build_rows(sprite: &Sprite, display: &[(LayerId, u16)], order: &[LayerComposite], range: std::ops::Range<usize>) -> Vec<Row> {
    // A layer can merge down when it and the composite entry directly below it
    // are both raster (groups never appear in composite order).
    let can_merge_down = |id: LayerId| -> bool {
        let Some(pos) = order.iter().position(|c| c.layer == id) else {
            return false;
        };
        if pos == 0 {
            return false;
        }
        let raster = |lid: LayerId| sprite.layer(lid).is_some_and(|l| matches!(l.kind, LayerKind::Raster));
        raster(order[pos].layer) && raster(order[pos - 1].layer)
    };
    display
        .get(range)
        .unwrap_or(&[])
        .iter()
        .filter_map(|&(id, depth)| {
            let l = sprite.layer(id)?;
            Some(Row {
                id,
                name: l.name.clone(),
                visible: l.visible,
                locked: l.locked,
                is_group: l.is_group(),
                collapsed: l.collapsed(),
                has_children: sprite.layers.iter().any(|c| c.parent == Some(id)),
                depth,
                parent: l.parent,
                can_merge_down: can_merge_down(id),
                clip_below: l.clip_below,
            })
        })
        .collect()
}

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

        // Back-to-front composite order: a layer can merge down when it and the
        // entry directly below it are both raster (groups never appear here).
        let order = sprite.composite_order();
        // The display tree, computed once this frame. Rows are built lazily from
        // a slice of this for the visible range only (see `build_rows`), so a
        // 1000-layer sprite never allocates 1000 row widgets.
        let display = sprite.layer_display_order();
        let total_rows = display.len();
        let active = self.doc.active_layer;
        let layer_count = sprite.layers.len();
        // Flatten visible needs at least two visible raster layers (groups never
        // appear in composite_order).
        let can_flatten = order
            .iter()
            .filter(|c| c.visible && sprite.layer(c.layer).is_some_and(|l| matches!(l.kind, LayerKind::Raster)))
            .count()
            >= 2;
        // Merge selected needs at least two unique raster layers in the
        // multi-select and no non-raster member (the merge is raster-only).
        let selected_set = &self.editor.selected_layers;
        let raster_member = |id: LayerId| sprite.layer(id).is_some_and(|l| matches!(l.kind, LayerKind::Raster));
        let can_merge_selected =
            selected_set.iter().all(|&id| raster_member(id)) && selected_set.iter().filter(|&&id| order.iter().any(|c| c.layer == id)).count() >= 2;
        // Snapshot the selection ids so the per-row Delete can target the whole
        // set without re-borrowing `self.editor` inside the menu closure.
        let selected_ids: Vec<LayerId> = selected_set.iter().copied().collect();

        // (layer, additive, range): a click routed through the multi-select model.
        let mut select: Option<(LayerId, bool, bool)> = None;
        // A right-click on a row outside the selection collapses to that row
        // before the context menu opens.
        let mut collapse_select: Option<LayerId> = None;
        let mut toggle_visible: Option<LayerId> = None;
        let mut toggle_lock: Option<LayerId> = None;
        let mut toggle_collapse: Option<LayerId> = None;
        let mut toggle_clip: Option<LayerId> = None;
        let mut set_opacity: Option<(LayerId, u8)> = None;
        let mut set_blend: Option<(LayerId, BlendMode)> = None;
        let mut move_up: Option<LayerId> = None;
        let mut move_down: Option<LayerId> = None;
        // The layers a Delete should remove: the whole multi-select when the
        // right-clicked row is in it, else just that row.
        let mut delete: Option<Vec<LayerId>> = None;
        let mut merge_down: Option<LayerId> = None;
        let mut merge_selected = false;
        let mut flatten_visible = false;
        let mut duplicate: Option<LayerId> = None;
        let mut add_inside: Option<LayerId> = None;
        let mut start_rename: Option<(LayerId, String)> = None;
        let mut commit_rename: Option<(LayerId, String)> = None;
        let mut cancel_rename = false;
        // (dragged layer, new parent, anchor sibling to land above).
        let mut layer_move: Option<(LayerId, Option<LayerId>, Option<LayerId>)> = None;

        // Blend mode and opacity for the anchor raster layer, in a fixed-height
        // strip above the list. Pulling it out of the per-row body keeps every
        // list row a uniform height, which is what lets `show_rows` virtualize
        // the list honestly. Groups carry no blend/opacity strip.
        if let Some(anchor) = active.and_then(|id| sprite.layer(id)) {
            if !anchor.is_group() {
                let aid = anchor.id;
                let blend = anchor.blend_mode;
                let opacity = anchor.opacity;
                ui.horizontal(|ui| {
                    egui::ComboBox::from_id_salt(("blend", aid.get()))
                        .width(110.0)
                        .selected_text(blend_label(blend))
                        .show_ui(ui, |ui| {
                            for (mode, name) in BLEND_MODES {
                                let mut current = blend;
                                if ui.selectable_value(&mut current, *mode, *name).clicked() {
                                    set_blend = Some((aid, *mode));
                                }
                            }
                        });
                    let mut op = opacity;
                    if ui.add(egui::Slider::new(&mut op, 0..=255).text("α")).changed() {
                        set_opacity = Some((aid, op));
                    }
                });
            }
        }
        ui.separator();

        // Virtualized list: `show_rows` calls the closure with only the visible
        // index range, so we build row widgets and clone names for that slice
        // alone. A uniform row height keeps the scrollbar honest now that the
        // anchor's blend/opacity strip lives above the list.
        let row_height = ui.text_style_height(&egui::TextStyle::Body) + ui.spacing().interact_size.y;
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show_rows(ui, row_height, total_rows, |ui, range| {
                for row in build_rows(sprite, &display, &order, range) {
                    let is_anchor = active == Some(row.id);
                    let selected = self.editor.selected_layers.contains(&row.id);
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

                            // Strong text marks the anchor (active) row only, even when
                            // several rows are selected.
                            let label = if row.is_group {
                                egui::RichText::new(format!("{} {}", icons::GROUP, row.name))
                            } else if is_anchor {
                                egui::RichText::new(row.name.clone()).strong()
                            } else {
                                egui::RichText::new(row.name.clone())
                            };
                            // One widget senses both click and drag; a clean click stays a
                            // click because `drag_started` needs pointer motion. Selected
                            // rows render highlighted via `Button::selectable`.
                            let resp = ui.add(egui::Button::selectable(selected, label).sense(egui::Sense::click_and_drag()));
                            if resp.clicked() {
                                if row.is_group {
                                    toggle_collapse = Some(row.id);
                                } else {
                                    let (cmd, shift) = ui.input(|i| (i.modifiers.command, i.modifiers.shift));
                                    select = Some((row.id, cmd, shift));
                                }
                            }
                            if resp.double_clicked() {
                                start_rename = Some((row.id, row.name.clone()));
                            }
                            // Right-click on a row outside the selection collapses to it
                            // first, so a context-menu batch op targets the clicked row.
                            if resp.secondary_clicked() && !selected {
                                collapse_select = Some(row.id);
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
                                if !row.is_group {
                                    let item = egui::Button::new(format!("{} Merge down", icons::LAYERS));
                                    if ui
                                        .add_enabled(row.can_merge_down, item)
                                        .on_disabled_hover_text("Needs a raster layer directly below")
                                        .clicked()
                                    {
                                        merge_down = Some(row.id);
                                        ui.close();
                                    }
                                    // Clip to the layer below: the layer shows only where
                                    // its clip base is opaque. A toggle, so the label
                                    // reflects the current state.
                                    let clip_label = if row.clip_below { "Unclip from below" } else { "Clip to layer below" };
                                    if ui.button(format!("{} {clip_label}", icons::CLIP)).clicked() {
                                        toggle_clip = Some(row.id);
                                        ui.close();
                                    }
                                }
                                let merge_sel_item = egui::Button::new(format!("{} Merge selected", icons::LAYERS));
                                if ui
                                    .add_enabled(can_merge_selected, merge_sel_item)
                                    .on_disabled_hover_text("Select at least two raster layers")
                                    .clicked()
                                {
                                    merge_selected = true;
                                    ui.close();
                                }
                                let flatten_item = egui::Button::new(format!("{} Flatten visible", icons::LAYERS));
                                if ui
                                    .add_enabled(can_flatten, flatten_item)
                                    .on_disabled_hover_text("Needs at least two visible raster layers")
                                    .clicked()
                                {
                                    flatten_visible = true;
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
                                    // Delete the whole selection when the right-clicked
                                    // row is part of it, else just this row.
                                    delete = Some(if selected { selected_ids.clone() } else { vec![row.id] });
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
                    // The anchor row no longer carries an inline blend/opacity strip;
                    // that strip is rendered once above the list, so every list row is
                    // the same height. The anchor still reads as strong text in the row.
                }
            });

        // A slim strip, shown only while dragging, files the layer at top level.
        if let Some(payload) = crate::dnd::top_level_strip::<LayerDrag>(ui, "Move to top level") {
            let LayerDrag(dragged) = *payload;
            layer_move = Some((dragged, None, None));
        }

        // Apply the single action chosen this frame.
        if let Some(id) = collapse_select {
            self.select_layer(id, false, false);
        }
        if let Some((id, additive, range)) = select {
            self.select_layer(id, additive, range);
        }
        if let Some(id) = toggle_visible {
            self.edit_layer("Toggle visibility", id, |l| l.visible = !l.visible);
            self.refresh_canvas(false);
        }
        if let Some(id) = toggle_lock {
            self.edit_layer("Toggle lock", id, |l| l.locked = !l.locked);
        }
        if let Some(id) = toggle_clip {
            self.edit_layer("Toggle clip", id, |l| l.clip_below = !l.clip_below);
            self.refresh_canvas(false);
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
        if let Some(ids) = delete {
            self.request_delete_layers(&ids);
        }
        if let Some(id) = merge_down {
            // The op merges the active layer down; target the right-clicked row.
            self.doc.active_layer = Some(id);
            self.merge_down();
        }
        if merge_selected {
            // The op reads the multi-select set directly; the right-click-collapse
            // rule above already scopes it to the clicked row when needed.
            self.merge_selected();
        }
        if flatten_visible {
            self.flatten_visible();
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

    /// Routes a layer-panel click through the multi-select model. `additive` is
    /// Ctrl/Cmd-click (toggle), `range` is Shift-click (contiguous span). Keeps
    /// `active_layer` as the anchor and paint target; the selection set is the
    /// batch-op target. See [`crate::editor::EditorState::resolve_layer_selection`].
    pub(crate) fn select_layer(&mut self, id: LayerId, additive: bool, range: bool) {
        let order: Vec<LayerId> = self
            .doc
            .active_sprite()
            .map(|s| s.layer_display_order().into_iter().map(|(lid, _)| lid).collect())
            .unwrap_or_default();
        let anchor = self.doc.active_layer;
        let new_anchor = self.editor.resolve_layer_selection(&order, anchor, id, additive, range);
        self.doc.active_layer = Some(new_anchor);
    }

    /// The current multi-select. Batch ops (merge selected, delete multi) read
    /// this.
    // Consumed by plan tasks 4 and 5; added now so every later task reads one
    // ShellApp accessor.
    #[allow(dead_code)]
    pub(crate) fn selected_layer_ids(&self) -> Vec<LayerId> {
        self.editor.selected_layer_ids()
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

    /// Reseeds the anchor and the multi-select set when the set has emptied or
    /// the anchor no longer names a live layer — e.g. after a delete. Seeds both
    /// from the top remaining layer in display order. A no-op while the anchor
    /// is still a selected, live layer.
    pub(crate) fn reseed_layer_selection(&mut self) {
        let order: Vec<LayerId> = self
            .doc
            .active_sprite()
            .map(|s| s.layer_display_order().into_iter().map(|(id, _)| id).collect())
            .unwrap_or_default();
        let anchor = self.doc.active_layer;
        self.doc.active_layer = crate::editor::reseed_selection(&order, &mut self.editor.selected_layers, anchor);
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

#[cfg(test)]
mod tests {
    use pixhaus_core::project::{BlendMode, Layer, LayerId, LayerKind, Size, Sprite, SpriteId};

    use super::build_rows;

    fn lid(n: u32) -> LayerId {
        LayerId::new(n)
    }

    /// A sprite with `n` flat raster layers, ids 1..=n. Vec order is bottom-first,
    /// so display order is top-first: [n, n-1, ..., 1].
    fn flat_sprite(n: u32) -> Sprite {
        let mut s = Sprite::empty(SpriteId::new(1), "big", Size::new(8, 8));
        s.layers = (1..=n).map(|i| Layer::raster(lid(i), format!("layer {i}"))).collect();
        s
    }

    #[test]
    fn build_rows_only_materializes_the_visible_range() {
        // A 1000-layer sprite must not build 1000 row widgets a frame: the row
        // snapshot is bounded by the visible range we ask for, not the total.
        let s = flat_sprite(1000);
        let display = s.layer_display_order();
        let order = s.composite_order();
        assert_eq!(display.len(), 1000);

        let rows = build_rows(&s, &display, &order, 40..60);
        assert_eq!(rows.len(), 20, "exactly the requested range, never the whole tree");
    }

    #[test]
    fn build_rows_maps_scroll_range_to_the_right_display_rows() {
        // Display order is top-first [1000, 999, ..., 1]; the slice the scroll
        // area hands us must map to those exact ids in order.
        let s = flat_sprite(1000);
        let display = s.layer_display_order();
        let order = s.composite_order();

        let rows = build_rows(&s, &display, &order, 0..3);
        let ids: Vec<u32> = rows.iter().map(|r| r.id.get()).collect();
        assert_eq!(ids, vec![1000, 999, 998], "top of the list is the top-first display order");

        let mid = build_rows(&s, &display, &order, 500..503);
        let mid_ids: Vec<u32> = mid.iter().map(|r| r.id.get()).collect();
        let expect: Vec<u32> = display[500..503].iter().map(|(id, _)| id.get()).collect();
        assert_eq!(mid_ids, expect, "a mid-list slice maps to the matching display rows");
    }

    #[test]
    fn build_rows_clamps_an_out_of_bounds_range() {
        // `show_rows` never hands us a range past the end, but the slice must be
        // total-safe regardless: an out-of-range ask yields no rows, no panic.
        let s = flat_sprite(4);
        let display = s.layer_display_order();
        let order = s.composite_order();
        assert!(build_rows(&s, &display, &order, 10..20).is_empty());
    }

    #[test]
    fn build_rows_reports_merge_down_only_for_raster_over_raster() {
        // Bottom-first vec [raster 1, group 2, raster 3]; display top-first is
        // [3, 2, 1]. Composite order skips the group, so raster 3 sits directly
        // above raster 1 and can merge down; the bottom raster 1 cannot.
        let mut s = flat_sprite(3);
        if let Some(g) = s.layers.iter_mut().find(|l| l.id == lid(2)) {
            g.kind = LayerKind::Group { collapsed: false };
        }
        let display = s.layer_display_order();
        let order = s.composite_order();
        let rows = build_rows(&s, &display, &order, 0..display.len());

        let row = |id: u32| rows.iter().find(|r| r.id == lid(id)).expect("row built for the range");
        assert!(row(3).can_merge_down, "raster above another raster can merge down");
        assert!(!row(1).can_merge_down, "the bottom raster has nothing below it");
        assert!(!row(2).can_merge_down, "a group never merges down");
    }

    #[test]
    fn anchor_strip_reads_blend_and_opacity_from_the_model() {
        // The blend/opacity strip is rendered above the list from the anchor
        // layer directly, not from a row widget, so virtualization never hides
        // it. Assert the data the strip reads is reachable for a raster anchor.
        let mut s = flat_sprite(2);
        if let Some(l) = s.layers.iter_mut().find(|l| l.id == lid(1)) {
            l.blend_mode = BlendMode::Multiply;
            l.opacity = 128;
        }
        let anchor = s.layer(lid(1)).expect("anchor layer present");
        assert!(!anchor.is_group(), "a raster anchor shows the blend/opacity strip");
        assert_eq!(anchor.blend_mode, BlendMode::Multiply);
        assert_eq!(anchor.opacity, 128);
    }
}
