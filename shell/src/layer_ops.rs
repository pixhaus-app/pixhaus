//! Destructive layer-consolidation orchestration: merge down and flatten visible
//! (and, in a later task, merge selected).
//!
//! The work splits in two. A pure function over [`DocumentStore`] +
//! [`EditorState`] ([`merge_down_op`], [`flatten_visible_op`]) resolves layers
//! and buffers, builds the per-frame [`LayerInput`] lists in `composite_order`
//! order, calls a pure `core` primitive ([`merge_pair`] / [`flatten`]), and
//! records exactly one undo entry via [`push_layer_consolidation`]. The
//! [`ShellApp`] wrapper adds the view-side work (status feedback, selection
//! reseed, canvas refresh). The blend math is never re-implemented here — the
//! merged pixels come straight from the live composite path, so they match what
//! was on screen.

use std::collections::BTreeSet;

use pixhaus_core::canvas::{LayerInput, PixelBuffer, flatten, merge_pair};
use pixhaus_core::project::{BlendMode, Cel, CelData, FrameIndex, Layer, LayerId, LayerKind, PixelBufferId, Sprite};

use crate::app::ShellApp;
use crate::commands::push_layer_consolidation;
use crate::document::DocumentStore;
use crate::editor::EditorState;

impl ShellApp {
    /// Merges the active (anchor) layer down onto the layer directly beneath it
    /// in composite order, dropping the source as one undo step, then reseeds
    /// the selection to the destination and refreshes the canvas.
    ///
    /// Rejects with a status-bar message — never a panic — when the anchor is
    /// the bottom-most contributing layer, is a group, or either layer is
    /// non-raster (the `core` merge is defined for raster only, mirroring the
    /// Tauri `Unimplemented` guard).
    pub(crate) fn merge_down(&mut self) {
        let Some(anchor) = self.doc.active_layer else {
            self.set_status("No active layer to merge");
            return;
        };
        match merge_down_op(&mut self.editor, &mut self.doc, anchor) {
            Ok(dest) => {
                self.editor.selected_layers.remove(&anchor);
                self.doc.active_layer = Some(dest);
                self.editor.selected_layers.insert(dest);
                self.reseed_layer_selection();
                self.refresh_canvas(false);
            }
            Err(reason) => self.set_status(reason),
        }
    }

    /// Whether the active layer can merge down right now: it and the entry
    /// directly below it in `composite_order` are both raster. Drives the menu
    /// item's enabled state.
    pub(crate) fn active_layer_can_merge_down(&self) -> bool {
        self.doc.active_layer.is_some_and(|anchor| can_merge_down(self.doc.active_sprite(), anchor))
    }

    /// Flattens every visible raster layer into the bottom-most such layer as one
    /// undo step, then reseeds the selection to that target and refreshes the
    /// canvas. Hidden layers, groups, and non-raster layers stay untouched.
    ///
    /// Rejects with a status-bar message — never a panic — when there are fewer
    /// than two visible raster layers to collapse.
    pub(crate) fn flatten_visible(&mut self) {
        match flatten_visible_op(&mut self.editor, &mut self.doc) {
            Ok(target) => {
                self.doc.active_layer = Some(target);
                self.editor.selected_layers.clear();
                self.editor.selected_layers.insert(target);
                self.reseed_layer_selection();
                self.refresh_canvas(false);
            }
            Err(reason) => self.set_status(reason),
        }
    }

    /// Whether the active sprite has at least two visible raster layers, so
    /// flatten visible has something to collapse. Drives the menu item's enabled
    /// state.
    pub(crate) fn can_flatten_visible(&self) -> bool {
        self.doc.active_sprite().is_some_and(|s| visible_raster_targets(s).len() >= 2)
    }

    /// Merges the multi-select set into its topmost member as one undo step, then
    /// collapses the selection to that target and refreshes the canvas. Reads
    /// [`EditorState::selected_layers`]; the non-target selected layers are
    /// dropped.
    ///
    /// Rejects with a status-bar message — never a panic — when the selection has
    /// fewer than two unique raster layers, or when any selected member is
    /// non-raster.
    pub(crate) fn merge_selected(&mut self) {
        match merge_selected_op(&mut self.editor, &mut self.doc) {
            Ok(target) => {
                self.doc.active_layer = Some(target);
                self.editor.selected_layers.clear();
                self.editor.selected_layers.insert(target);
                self.reseed_layer_selection();
                self.refresh_canvas(false);
            }
            Err(reason) => self.set_status(reason),
        }
    }

    /// Whether the multi-select holds at least two raster layers and no
    /// non-raster member, so merge selected has something to collapse. Drives the
    /// context-menu item's enabled state.
    pub(crate) fn can_merge_selected(&self) -> bool {
        self.doc.active_sprite().is_some_and(|s| can_merge_selected(s, &self.editor.selected_layers))
    }

    /// Requests a delete of `ids`. A single leaf layer deletes immediately; a
    /// multi-layer delete or a group with children is destructive enough to ask
    /// first, so it stages [`EditorState::pending_layer_delete`] for the confirm
    /// modal instead. `ids` are deduped and filtered to live layers; the
    /// last-remaining-layer guard lives in [`delete_layers_op`].
    pub(crate) fn request_delete_layers(&mut self, ids: &[LayerId]) {
        let live = self.live_delete_ids(ids);
        if live.is_empty() {
            return;
        }
        if delete_needs_confirm(self.doc.active_sprite(), &live) {
            self.editor.pending_layer_delete = Some(live);
        } else {
            self.run_delete_layers(&live);
        }
    }

    /// Runs the delete staged in [`EditorState::pending_layer_delete`] and clears
    /// the flag. The confirm modal calls this.
    pub(crate) fn confirm_pending_layer_delete(&mut self) {
        if let Some(ids) = self.editor.pending_layer_delete.take() {
            self.run_delete_layers(&ids);
        }
    }

    /// Clears a staged delete without running it. The confirm modal's Cancel
    /// calls this.
    pub(crate) fn cancel_pending_layer_delete(&mut self) {
        self.editor.pending_layer_delete = None;
    }

    /// Deletes `ids` as one undo step, reseeds the anchor/selection from the top
    /// remaining layer, and refreshes the canvas. Rejects with a status-bar
    /// message — never a panic — when the delete would empty the sprite.
    fn run_delete_layers(&mut self, ids: &[LayerId]) {
        match delete_layers_op(&mut self.editor, &mut self.doc, ids) {
            Ok(()) => {
                for id in ids {
                    self.editor.selected_layers.remove(id);
                }
                self.reseed_layer_selection();
                self.refresh_canvas(false);
            }
            Err(reason) => self.set_status(reason),
        }
    }

    /// Dedupes `ids` and keeps only the ones that name a live layer in the active
    /// sprite, preserving the given order.
    fn live_delete_ids(&self, ids: &[LayerId]) -> Vec<LayerId> {
        let Some(sprite) = self.doc.active_sprite() else {
            return Vec::new();
        };
        let mut out: Vec<LayerId> = Vec::new();
        for &id in ids {
            if sprite.layer(id).is_some() && !out.contains(&id) {
                out.push(id);
            }
        }
        out
    }
}

/// The visible raster layers of `sprite` in back-to-front `composite_order`,
/// the set flatten collapses. Excludes hidden layers, groups (never in
/// `composite_order`), and non-raster layers.
fn visible_raster_targets(sprite: &Sprite) -> Vec<LayerId> {
    sprite
        .composite_order()
        .into_iter()
        .filter(|c| c.visible && sprite.layer(c.layer).is_some_and(|l| matches!(l.kind, LayerKind::Raster)))
        .map(|c| c.layer)
        .collect()
}

/// Whether `anchor` can merge down within `sprite`: it appears in
/// `composite_order`, is not the bottom-most entry, and both it and the entry
/// directly below it are raster.
fn can_merge_down(sprite: Option<&Sprite>, anchor: LayerId) -> bool {
    let Some(sprite) = sprite else {
        return false;
    };
    let order = sprite.composite_order();
    let Some(pos) = order.iter().position(|c| c.layer == anchor) else {
        return false;
    };
    if pos == 0 {
        return false;
    }
    let raster = |id: LayerId| sprite.layer(id).is_some_and(|l| matches!(l.kind, LayerKind::Raster));
    raster(order[pos].layer) && raster(order[pos - 1].layer)
}

/// Merges `anchor` down onto the layer directly below it, recording one
/// [`push_layer_consolidation`] undo entry. Returns the destination layer id on
/// success, or a rejection reason (for the status bar) without mutating
/// anything.
///
/// The destination is the previous non-group [`pixhaus_core::project::sprite::LayerComposite`]
/// entry in back-to-front `composite_order`. Per frame the source has a raster
/// cel on, `above` carries the source's effective blend/opacity (from
/// `composite_order`) and `below` is a `Normal`/full-opacity backdrop built from
/// the destination cel (or transparent when the destination has no cel there).
/// All v2 cels are full-canvas at the origin, so the buffers pass straight into
/// [`merge_pair`] with no offset blit.
fn merge_down_op(editor: &mut EditorState, doc: &mut DocumentStore, anchor: LayerId) -> Result<LayerId, &'static str> {
    let Some(sprite) = doc.active_sprite() else {
        return Err("No active sprite");
    };

    // Groups never appear in `composite_order`, so a missing position also
    // catches a group anchor.
    let order = sprite.composite_order();
    let Some(pos) = order.iter().position(|c| c.layer == anchor) else {
        return Err("Cannot merge a group layer");
    };
    if pos == 0 {
        return Err("Layer is already at the bottom");
    }
    let above_entry = order[pos];
    let dest = order[pos - 1].layer;

    let raster = |id: LayerId| sprite.layer(id).is_some_and(|l| matches!(l.kind, LayerKind::Raster));
    if !raster(anchor) || !raster(dest) {
        return Err("Merge down needs two raster layers");
    }

    let canvas = sprite.canvas;
    let frame_count = sprite.frames.len() as u32;

    // Per-frame merged targets and the destination-cel repoints they drive.
    let mut added_buffers: Vec<(PixelBufferId, PixelBuffer)> = Vec::new();
    let mut repoints: Vec<(FrameIndex, PixelBufferId)> = Vec::new();
    // Ids of the buffers this op retires; deduped, bytes captured below.
    let mut removed_ids: Vec<PixelBufferId> = Vec::new();

    for f in 0..frame_count {
        let frame = FrameIndex::new(f);
        // The source must contribute a raster cel on this frame, else there is
        // nothing to fold down here.
        let Some(src_id) = resolved_raster_buffer(doc, anchor, frame) else {
            continue;
        };
        let dst_id = resolved_raster_buffer(doc, dest, frame);

        let transparent = PixelBuffer::new(canvas.width, canvas.height).map_err(|_| "Merge down failed: out of memory")?;
        let below_buf = dst_id.and_then(|id| doc.pixel_buffers.get(&id)).unwrap_or(&transparent);
        let Some(above_buf) = doc.pixel_buffers.get(&src_id) else {
            continue;
        };
        debug_assert_eq!(
            (above_buf.width(), above_buf.height()),
            (canvas.width, canvas.height),
            "source cel is full-canvas"
        );
        debug_assert_eq!(
            (below_buf.width(), below_buf.height()),
            (canvas.width, canvas.height),
            "destination cel is full-canvas"
        );

        let below = LayerInput {
            buffer: below_buf,
            mode: BlendMode::Normal,
            opacity: 255,
            visible: true,
            clip_below: false,
        };
        let above = LayerInput {
            buffer: above_buf,
            mode: above_entry.mode,
            opacity: above_entry.opacity,
            visible: true,
            clip_below: false,
        };
        let merged = merge_pair(&below, &above, canvas.width, canvas.height).map_err(|_| "Merge down failed")?;

        let target_id = PixelBufferId::new(doc.alloc_id());
        added_buffers.push((target_id, merged));
        repoints.push((frame, target_id));
        if let Some(id) = dst_id {
            push_unique(&mut removed_ids, id);
        }
    }

    if repoints.is_empty() {
        return Err("Nothing to merge");
    }

    // The source layer's cels are dropped wholesale, so every owning raster
    // buffer it references is retired.
    for buffer in layer_owning_buffers(doc, anchor) {
        push_unique(&mut removed_ids, buffer);
    }

    let removed_buffers: Vec<(PixelBufferId, PixelBuffer)> = removed_ids
        .into_iter()
        .filter_map(|id| doc.pixel_buffers.get(&id).map(|buf| (id, buf.clone())))
        .collect();

    push_layer_consolidation(editor, doc, "Merge down", added_buffers, removed_buffers, |sprite| {
        // Drop the source layer and its cels first.
        sprite.remove_layer(anchor);
        // Repoint (or add) the destination cel on each merged frame to its fresh
        // target buffer. All targets are full-canvas at the origin.
        for (frame, target_id) in &repoints {
            if let Some(cel) = sprite.cels.iter_mut().find(|c| c.layer_id == dest && c.frame_index == *frame) {
                cel.data = CelData::Raster {
                    buffer: *target_id,
                    size: canvas,
                };
            } else {
                sprite.cels.push(Cel::raster(dest, *frame, *target_id, canvas));
            }
        }
    });

    Ok(dest)
}

/// Flattens every visible raster layer into the bottom-most such layer,
/// recording one [`push_layer_consolidation`] undo entry. Returns the target
/// layer id on success, or a rejection reason (for the status bar) without
/// mutating anything.
///
/// The visible raster set is `composite_order()` (back-to-front) filtered to
/// `entry.visible` and `LayerKind::Raster`; fewer than two members is rejected.
/// The target is the first such entry (bottom-most). Per frame the
/// [`LayerInput`] list is gathered in `composite_order` order — each entry
/// carrying its effective blend/opacity — and collapsed through [`flatten`] into
/// a fresh target buffer. Hidden layers, groups, and non-raster layers are
/// excluded from the inputs and left untouched. All v2 cels are full-canvas at
/// the origin, so the buffers pass straight through with no offset blit.
fn flatten_visible_op(editor: &mut EditorState, doc: &mut DocumentStore) -> Result<LayerId, &'static str> {
    let Some(sprite) = doc.active_sprite() else {
        return Err("No active sprite");
    };

    // Back-to-front visible raster entries: the order the inputs composite in.
    let visible: Vec<(LayerId, BlendMode, u8)> = sprite
        .composite_order()
        .into_iter()
        .filter(|c| c.visible && sprite.layer(c.layer).is_some_and(|l| matches!(l.kind, LayerKind::Raster)))
        .map(|c| (c.layer, c.mode, c.opacity))
        .collect();
    if visible.len() < 2 {
        return Err("Flatten needs at least two visible raster layers");
    }
    // The bottom-most visible raster layer is the survivor everything folds into.
    let target = visible[0].0;

    let canvas = sprite.canvas;
    let frame_count = sprite.frames.len() as u32;

    // Per-frame flattened targets and the target-cel repoints they drive.
    let mut added_buffers: Vec<(PixelBufferId, PixelBuffer)> = Vec::new();
    let mut repoints: Vec<(FrameIndex, PixelBufferId)> = Vec::new();
    // Ids of the buffers this op retires; deduped, bytes captured below.
    let mut removed_ids: Vec<PixelBufferId> = Vec::new();

    for f in 0..frame_count {
        let frame = FrameIndex::new(f);

        // Resolve the visible raster set's owning buffer ids for this frame, in
        // composite order. A layer with no cel here contributes nothing.
        let resolved: Vec<(PixelBufferId, BlendMode, u8)> = visible
            .iter()
            .filter_map(|&(layer, mode, opacity)| resolved_raster_buffer(doc, layer, frame).map(|id| (id, mode, opacity)))
            .collect();
        if resolved.is_empty() {
            continue;
        }

        // Borrow the buffers from the store to build the LayerInput slice. A
        // buffer that has gone missing skips that input rather than aborting.
        let inputs: Vec<LayerInput<'_>> = resolved
            .iter()
            .filter_map(|&(id, mode, opacity)| {
                let buffer = doc.pixel_buffers.get(&id)?;
                debug_assert_eq!((buffer.width(), buffer.height()), (canvas.width, canvas.height), "flatten input is full-canvas");
                Some(LayerInput {
                    buffer,
                    mode,
                    opacity,
                    visible: true,
                    clip_below: false,
                })
            })
            .collect();
        let flattened = flatten(&inputs, canvas.width, canvas.height).map_err(|_| "Flatten failed")?;

        let target_id = PixelBufferId::new(doc.alloc_id());
        added_buffers.push((target_id, flattened));
        repoints.push((frame, target_id));
    }

    if repoints.is_empty() {
        return Err("Nothing to flatten");
    }

    // Every visible raster source except the target is dropped; its old target
    // buffers are retired too (the target cels repoint to the fresh ones).
    for &(layer, _, _) in &visible {
        for buffer in layer_owning_buffers(doc, layer) {
            push_unique(&mut removed_ids, buffer);
        }
    }

    let removed_buffers: Vec<(PixelBufferId, PixelBuffer)> = removed_ids
        .into_iter()
        .filter_map(|id| doc.pixel_buffers.get(&id).map(|buf| (id, buf.clone())))
        .collect();

    let sources: Vec<LayerId> = visible.iter().map(|&(layer, _, _)| layer).filter(|&id| id != target).collect();

    push_layer_consolidation(editor, doc, "Flatten visible", added_buffers, removed_buffers, |sprite| {
        // Drop every flattened source except the target, with its cels.
        for &id in &sources {
            sprite.remove_layer(id);
        }
        // Repoint (or add) the target cel on each flattened frame to its fresh
        // buffer. All targets are full-canvas at the origin.
        for (frame, target_id) in &repoints {
            if let Some(cel) = sprite.cels.iter_mut().find(|c| c.layer_id == target && c.frame_index == *frame) {
                cel.data = CelData::Raster {
                    buffer: *target_id,
                    size: canvas,
                };
            } else {
                sprite.cels.push(Cel::raster(target, *frame, *target_id, canvas));
            }
        }
    });

    Ok(target)
}

/// Whether `selected` holds at least two unique raster layers and no non-raster
/// member, the precondition merge selected enforces. A group or any non-raster
/// member makes it false (merge is defined for raster only), as does a set with
/// fewer than two members that appear in `composite_order`.
fn can_merge_selected(sprite: &Sprite, selected: &BTreeSet<LayerId>) -> bool {
    let order = sprite.composite_order();
    let raster = |id: LayerId| sprite.layer(id).is_some_and(|l| matches!(l.kind, LayerKind::Raster));
    // Any non-raster (or unknown) member rejects the whole set: merge is
    // raster-only and a group selection cannot fold into a layer.
    if selected.iter().any(|&id| !raster(id)) {
        return false;
    }
    // Need at least two members that actually contribute to the composite.
    selected.iter().filter(|&&id| order.iter().any(|c| c.layer == id)).count() >= 2
}

/// Merges the selected set into its topmost member (last in back-to-front
/// `composite_order`), recording one [`push_layer_consolidation`] undo entry.
/// Returns the target layer id on success, or a rejection reason (for the status
/// bar) without mutating anything.
///
/// The selected ids are ordered by their position in `composite_order`
/// (back-to-front); the set must hold at least two unique raster layers and no
/// non-raster member, else it is rejected. The target is the last (topmost)
/// member. Per frame the [`LayerInput`] list is gathered over the selected set in
/// `composite_order` order — each entry carrying its effective blend/opacity —
/// and collapsed through [`flatten`] into a fresh target buffer. This is the same
/// per-frame composite path as flatten visible, run over the selected set only.
/// All v2 cels are full-canvas at the origin, so the buffers pass straight
/// through with no offset blit.
fn merge_selected_op(editor: &mut EditorState, doc: &mut DocumentStore) -> Result<LayerId, &'static str> {
    let Some(sprite) = doc.active_sprite() else {
        return Err("No active sprite");
    };

    // Back-to-front selected raster entries: the order the inputs composite in.
    // Filtering composite_order keeps the merge set to layers that actually
    // contribute (groups never appear here); the raster/uniqueness guards below
    // reject anything else with no mutation.
    let selected = &editor.selected_layers;
    let members: Vec<(LayerId, BlendMode, u8)> = sprite
        .composite_order()
        .into_iter()
        .filter(|c| selected.contains(&c.layer))
        .map(|c| (c.layer, c.mode, c.opacity))
        .collect();

    // Reject a non-raster member explicitly: it is selected but not a raster
    // layer, so the merge is undefined. (A group selected member never reaches
    // `members` because it is absent from composite_order, so catch it here over
    // the raw selection.)
    if selected
        .iter()
        .any(|&id| !sprite.layer(id).is_some_and(|l| matches!(l.kind, LayerKind::Raster)))
    {
        return Err("Merge selected needs raster layers only");
    }
    if members.len() < 2 {
        return Err("Select at least two raster layers to merge");
    }
    // The topmost selected layer (last in back-to-front order) is the survivor.
    let target = members[members.len() - 1].0;

    let canvas = sprite.canvas;
    let frame_count = sprite.frames.len() as u32;

    // Per-frame merged targets and the target-cel repoints they drive.
    let mut added_buffers: Vec<(PixelBufferId, PixelBuffer)> = Vec::new();
    let mut repoints: Vec<(FrameIndex, PixelBufferId)> = Vec::new();
    // Ids of the buffers this op retires; deduped, bytes captured below.
    let mut removed_ids: Vec<PixelBufferId> = Vec::new();

    for f in 0..frame_count {
        let frame = FrameIndex::new(f);

        // Resolve the selected set's owning buffer ids for this frame, in
        // composite order. A member with no cel here contributes nothing.
        let resolved: Vec<(PixelBufferId, BlendMode, u8)> = members
            .iter()
            .filter_map(|&(layer, mode, opacity)| resolved_raster_buffer(doc, layer, frame).map(|id| (id, mode, opacity)))
            .collect();
        if resolved.is_empty() {
            continue;
        }

        // Borrow the buffers from the store to build the LayerInput slice. A
        // buffer that has gone missing skips that input rather than aborting.
        let inputs: Vec<LayerInput<'_>> = resolved
            .iter()
            .filter_map(|&(id, mode, opacity)| {
                let buffer = doc.pixel_buffers.get(&id)?;
                debug_assert_eq!(
                    (buffer.width(), buffer.height()),
                    (canvas.width, canvas.height),
                    "merge-selected input is full-canvas"
                );
                Some(LayerInput {
                    buffer,
                    mode,
                    opacity,
                    visible: true,
                    clip_below: false,
                })
            })
            .collect();
        let merged = flatten(&inputs, canvas.width, canvas.height).map_err(|_| "Merge selected failed")?;

        let target_id = PixelBufferId::new(doc.alloc_id());
        added_buffers.push((target_id, merged));
        repoints.push((frame, target_id));
    }

    if repoints.is_empty() {
        return Err("Nothing to merge");
    }

    // Every selected source except the target is dropped; its old target buffers
    // are retired too (the target cels repoint to the fresh ones).
    for &(layer, _, _) in &members {
        for buffer in layer_owning_buffers(doc, layer) {
            push_unique(&mut removed_ids, buffer);
        }
    }

    let removed_buffers: Vec<(PixelBufferId, PixelBuffer)> = removed_ids
        .into_iter()
        .filter_map(|id| doc.pixel_buffers.get(&id).map(|buf| (id, buf.clone())))
        .collect();

    let sources: Vec<LayerId> = members.iter().map(|&(layer, _, _)| layer).filter(|&id| id != target).collect();

    push_layer_consolidation(editor, doc, "Merge selected", added_buffers, removed_buffers, |sprite| {
        // Drop every selected source except the target, with its cels.
        for &id in &sources {
            sprite.remove_layer(id);
        }
        // Repoint (or add) the target cel on each merged frame to its fresh
        // buffer. All targets are full-canvas at the origin.
        for (frame, target_id) in &repoints {
            if let Some(cel) = sprite.cels.iter_mut().find(|c| c.layer_id == target && c.frame_index == *frame) {
                cel.data = CelData::Raster {
                    buffer: *target_id,
                    size: canvas,
                };
            } else {
                sprite.cels.push(Cel::raster(target, *frame, *target_id, canvas));
            }
        }
    });

    Ok(target)
}

/// Whether deleting `ids` from `sprite` should ask for confirmation first. A
/// single leaf layer (one raster/empty layer with no children) deletes without a
/// prompt; deleting more than one layer, or any group that has children, is
/// destructive enough to confirm. `ids` are assumed deduped and live.
fn delete_needs_confirm(sprite: Option<&Sprite>, ids: &[LayerId]) -> bool {
    if ids.len() > 1 {
        return true;
    }
    let Some(sprite) = sprite else {
        return false;
    };
    // The single id: confirm only when it is a group that holds children
    // (deleting it reparents them, which is surprising enough to ask).
    ids.iter()
        .any(|&id| sprite.layer(id).is_some_and(Layer::is_group) && sprite.layers.iter().any(|l| l.parent == Some(id)))
}

/// Deletes the layers in `ids` (and their cels) as one undo step, retiring the
/// pixel buffers those cels owned so undo restores them. Returns `Ok(())` on a
/// successful delete, or a rejection reason (for the status bar) without
/// mutating anything when the delete would leave the sprite with no layers.
///
/// Records one [`push_layer_consolidation`] entry with no added buffers and the
/// deleted layers' owning raster buffers as `removed_buffers`. `SpriteEdit`
/// alone would lose those bytes on undo (it carries only handles), so the
/// buffer-carrying command is the one that keeps the structural drop and the
/// pixel bytes reversible together. The last-remaining-layer guard mirrors the
/// single-delete rule: a sprite must always keep at least one layer.
fn delete_layers_op(editor: &mut EditorState, doc: &mut DocumentStore, ids: &[LayerId]) -> Result<(), &'static str> {
    let Some(sprite) = doc.active_sprite() else {
        return Err("No active sprite");
    };
    // `remove_layer` reparents a group's children rather than deleting them, so
    // the survivors are every layer not named in `ids`. Refuse to empty the
    // sprite.
    let removing: usize = sprite.layers.iter().filter(|l| ids.contains(&l.id)).count();
    if removing == 0 {
        return Err("Nothing to delete");
    }
    if sprite.layers.len().saturating_sub(removing) == 0 {
        return Err("Cannot delete the last layer");
    }

    // Capture the bytes of every buffer the deleted cels own, before the command
    // retires them, so undo can restore the pixels.
    let mut removed_ids: Vec<PixelBufferId> = Vec::new();
    for &id in ids {
        for buffer in layer_owning_buffers(doc, id) {
            push_unique(&mut removed_ids, buffer);
        }
    }
    let removed_buffers: Vec<(PixelBufferId, PixelBuffer)> = removed_ids
        .into_iter()
        .filter_map(|id| doc.pixel_buffers.get(&id).map(|buf| (id, buf.clone())))
        .collect();

    let ids = ids.to_vec();
    push_layer_consolidation(editor, doc, "Delete layers", Vec::new(), removed_buffers, move |sprite| {
        for id in ids {
            sprite.remove_layer(id);
        }
    });
    Ok(())
}

/// The owning raster pixel buffer for `(layer, frame)`, resolving a linked cel
/// to its source frame's buffer. `None` when the layer has no raster cel
/// reachable from that frame.
fn resolved_raster_buffer(doc: &DocumentStore, layer: LayerId, frame: FrameIndex) -> Option<PixelBufferId> {
    let sprite = doc.active_sprite()?;
    let source = sprite.resolve_source_frame(layer, frame);
    match sprite.cel(layer, source)?.data {
        CelData::Raster { buffer, .. } => Some(buffer),
        _ => None,
    }
}

/// Every owning raster buffer the layer references across all its cels, deduped.
/// Used to gather the source buffers a merge retires.
fn layer_owning_buffers(doc: &DocumentStore, layer: LayerId) -> Vec<PixelBufferId> {
    let Some(sprite) = doc.active_sprite() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for cel in sprite.cels.iter().filter(|c| c.layer_id == layer) {
        if let CelData::Raster { buffer, .. } = cel.data {
            push_unique(&mut out, buffer);
        }
    }
    out
}

/// Pushes `id` onto `list` only if it is not already present.
fn push_unique(list: &mut Vec<PixelBufferId>, id: PixelBufferId) {
    if !list.contains(&id) {
        list.push(id);
    }
}

#[cfg(test)]
mod tests {
    use pixhaus_core::canvas::{LayerInput, PixelBuffer, composite_layers};
    use pixhaus_core::project::{BlendMode, Cel, CelData, FrameIndex, Layer, LayerId, LayerKind, PixelBufferId, Rgba, Size};

    use super::{delete_layers_op, delete_needs_confirm, flatten_visible_op, merge_down_op, merge_selected_op};
    use crate::commands::push_sprite_edit_with_buffers;
    use crate::document::DocumentStore;
    use crate::editor::EditorState;

    /// Appends a raster layer filled with `color` on frame 0, with the given
    /// visibility, to the active sprite. Returns its layer id. Layers added later
    /// stack on top (higher in `composite_order`).
    fn add_raster_layer(doc: &mut DocumentStore, editor: &mut EditorState, name: &str, color: Rgba, visible: bool) -> LayerId {
        let canvas = doc.active_sprite().expect("sprite").canvas;
        let layer = LayerId::new(doc.alloc_id());
        let buffer = PixelBufferId::new(doc.alloc_id());
        push_sprite_edit_with_buffers(
            editor,
            doc,
            "Add layer",
            vec![(buffer, PixelBuffer::filled(canvas.width, canvas.height, color).expect("layer buffer"))],
            |sprite| {
                let mut l = Layer::raster(layer, name);
                l.visible = visible;
                sprite.layers.push(l);
                sprite.cels.push(Cel::raster(layer, FrameIndex::new(0), buffer, canvas));
            },
        );
        layer
    }

    /// A two-raster-layer sprite: the seed layer (bottom) filled `below`, plus a
    /// second layer (top, the anchor) filled `above`. Returns the store, editor,
    /// the bottom (destination) and top (source) layer ids, and their buffer ids.
    fn two_layer_sprite(below: Rgba, above: Rgba) -> (DocumentStore, EditorState, LayerId, LayerId, PixelBufferId, PixelBufferId) {
        let mut doc = DocumentStore::new();
        doc.create_sprite("hero", Size::new(8, 8));
        let mut editor = EditorState::default();

        let dest_layer = doc.active_sprite().expect("sprite").layers[0].id;
        let CelData::Raster { buffer: dest_buffer, .. } = doc.active_sprite().expect("sprite").cels[0].data else {
            panic!("seed cel is raster");
        };
        // Fill the seed (destination) buffer.
        if let Some(buf) = doc.pixel_buffers.get_mut(&dest_buffer) {
            *buf = PixelBuffer::filled(8, 8, below).expect("below buffer");
        }

        let src_layer = LayerId::new(doc.alloc_id());
        let src_buffer = PixelBufferId::new(doc.alloc_id());
        push_sprite_edit_with_buffers(
            &mut editor,
            &mut doc,
            "Add layer",
            vec![(src_buffer, PixelBuffer::filled(8, 8, above).expect("above buffer"))],
            |sprite| {
                sprite.layers.push(Layer::raster(src_layer, "Layer 2"));
                sprite.cels.push(Cel::raster(src_layer, FrameIndex::new(0), src_buffer, Size::new(8, 8)));
            },
        );
        doc.active_layer = Some(src_layer);
        editor.selected_layers.clear();
        editor.selected_layers.insert(src_layer);

        (doc, editor, dest_layer, src_layer, dest_buffer, src_buffer)
    }

    #[test]
    fn merge_down_matches_a_direct_composite_of_the_two_layers() {
        // A semi-transparent top over an opaque bottom: the merged destination
        // must equal compositing the same two inputs through `composite_layers`
        // directly. This is the "reuse the live composite path" guarantee.
        let below = Rgba::opaque(0, 200, 40);
        let above = Rgba::new(220, 30, 0, 160);
        let (mut doc, mut editor, dest_layer, src_layer, _dest_buf, _src_buf) = two_layer_sprite(below, above);

        // The reference: composite the two inputs (bottom Normal/255, top with the
        // source layer's blend/opacity, both 255 here) on a fresh backdrop.
        let below_ref = PixelBuffer::filled(8, 8, below).expect("below");
        let above_ref = PixelBuffer::filled(8, 8, above).expect("above");
        let reference = composite_layers(
            8,
            8,
            &[
                LayerInput {
                    buffer: &below_ref,
                    mode: BlendMode::Normal,
                    opacity: 255,
                    visible: true,
                    clip_below: false,
                },
                LayerInput {
                    buffer: &above_ref,
                    mode: BlendMode::Normal,
                    opacity: 255,
                    visible: true,
                    clip_below: false,
                },
            ],
        )
        .expect("reference composite");

        let dest = merge_down_op(&mut editor, &mut doc, src_layer).expect("merge down");
        assert_eq!(dest, dest_layer, "merge folds onto the layer below");

        // The destination cel now points at the merged buffer; its bytes must
        // match the direct composite byte-for-byte.
        let CelData::Raster { buffer, .. } = doc.active_sprite().expect("sprite").cel(dest_layer, FrameIndex::new(0)).expect("dest cel").data else {
            panic!("dest cel is raster");
        };
        let merged = doc.pixel_buffers.get(&buffer).expect("merged buffer");
        assert_eq!(merged.as_bytes(), reference.as_bytes(), "merged destination equals a direct composite");
    }

    #[test]
    fn merge_down_drops_source_and_one_undo_restores_everything() {
        let below = Rgba::opaque(0, 0, 255);
        let above = Rgba::opaque(255, 0, 0);
        let (mut doc, mut editor, dest_layer, src_layer, _dest_buf, _src_buf) = two_layer_sprite(below, above);

        let baseline_layers = doc.active_sprite().expect("sprite").layers.len();
        let baseline_buffers = doc.pixel_buffers.len();
        // The destination pixel before the merge (opaque blue).
        let dest_pixel_before = {
            let CelData::Raster { buffer, .. } = doc.active_sprite().expect("sprite").cel(dest_layer, FrameIndex::new(0)).expect("cel").data else {
                panic!("raster");
            };
            doc.pixel_buffers.get(&buffer).expect("buffer").pixel(4, 4).expect("pixel")
        };
        assert_eq!(dest_pixel_before, below);

        merge_down_op(&mut editor, &mut doc, src_layer).expect("merge down");

        // The source layer is gone; the count dropped by one.
        assert_eq!(doc.active_sprite().expect("sprite").layers.len(), baseline_layers - 1, "source layer dropped");
        assert!(doc.active_sprite().expect("sprite").layer(src_layer).is_none(), "source layer removed");
        // Opaque red over blue leaves the destination red where the top covers it.
        let CelData::Raster { buffer: merged_buf, .. } = doc.active_sprite().expect("sprite").cel(dest_layer, FrameIndex::new(0)).expect("cel").data else {
            panic!("raster");
        };
        assert_eq!(
            doc.pixel_buffers.get(&merged_buf).expect("merged").pixel(4, 4),
            Some(above),
            "destination shows the merged pixel"
        );
        // Two buffers retired (source + old destination), one added (merged).
        assert_eq!(doc.pixel_buffers.len(), baseline_buffers - 1, "two retired, one added");

        // One undo fully reverses it: layer count, the sampled destination pixel,
        // and the buffer-store size all return to baseline.
        editor.history.undo(&mut doc).expect("undo");
        assert_eq!(
            doc.active_sprite().expect("sprite").layers.len(),
            baseline_layers,
            "undo restores the source layer"
        );
        let CelData::Raster { buffer: restored, .. } = doc.active_sprite().expect("sprite").cel(dest_layer, FrameIndex::new(0)).expect("cel").data else {
            panic!("raster");
        };
        assert_eq!(
            doc.pixel_buffers.get(&restored).expect("restored").pixel(4, 4),
            Some(dest_pixel_before),
            "undo restores the destination pixel"
        );
        assert_eq!(doc.pixel_buffers.len(), baseline_buffers, "buffer count returns to baseline");

        // Redo re-applies the merge.
        editor.history.redo(&mut doc).expect("redo");
        assert_eq!(
            doc.active_sprite().expect("sprite").layers.len(),
            baseline_layers - 1,
            "redo re-applies the merge"
        );
    }

    #[test]
    fn merge_down_rejects_the_bottom_layer() {
        let (mut doc, mut editor, dest_layer, _src_layer, _d, _s) = two_layer_sprite(Rgba::opaque(0, 0, 0), Rgba::opaque(255, 255, 255));
        let baseline_layers = doc.active_sprite().expect("sprite").layers.len();
        let err = merge_down_op(&mut editor, &mut doc, dest_layer).expect_err("bottom layer cannot merge down");
        assert_eq!(err, "Layer is already at the bottom");
        assert_eq!(doc.active_sprite().expect("sprite").layers.len(), baseline_layers, "rejection mutates nothing");
    }

    #[test]
    fn merge_down_rejects_a_non_raster_source() {
        let mut doc = DocumentStore::new();
        doc.create_sprite("hero", Size::new(8, 8));
        let mut editor = EditorState::default();
        // A group on top of the seed raster: a group anchor is not in
        // composite_order, so it is rejected as "cannot merge a group layer".
        let group = LayerId::new(doc.alloc_id());
        push_sprite_edit_with_buffers(&mut editor, &mut doc, "Add group", Vec::new(), |sprite| {
            let mut layer = Layer::raster(group, "Group");
            layer.kind = LayerKind::Group { collapsed: false };
            sprite.layers.push(layer);
        });
        let baseline = doc.active_sprite().expect("sprite").layers.len();
        let err = merge_down_op(&mut editor, &mut doc, group).expect_err("a group cannot merge down");
        assert_eq!(err, "Cannot merge a group layer");
        assert_eq!(doc.active_sprite().expect("sprite").layers.len(), baseline, "rejection mutates nothing");
    }

    #[test]
    fn flatten_visible_matches_the_on_screen_composite() {
        // Three visible raster layers (seed blue, semi-transparent green, semi-
        // transparent red on top). The flattened target must equal what the live
        // compositor showed before the flatten — composite_frame restricted to the
        // visible raster layers. Since composite_frame already skips hidden layers,
        // the pre-flatten composite IS the visible composite.
        let mut doc = DocumentStore::new();
        doc.create_sprite("hero", Size::new(8, 8));
        let mut editor = EditorState::default();

        // Recolor the seed (bottom) raster layer.
        let seed = doc.active_sprite().expect("sprite").layers[0].id;
        let CelData::Raster { buffer: seed_buf, .. } = doc.active_sprite().expect("sprite").cels[0].data else {
            panic!("seed cel is raster");
        };
        if let Some(buf) = doc.pixel_buffers.get_mut(&seed_buf) {
            *buf = PixelBuffer::filled(8, 8, Rgba::opaque(10, 20, 200)).expect("seed");
        }
        add_raster_layer(&mut doc, &mut editor, "Mid", Rgba::new(0, 220, 40, 170), true);
        add_raster_layer(&mut doc, &mut editor, "Top", Rgba::new(220, 30, 0, 120), true);

        // The reference: the on-screen composite before flatten.
        let reference = doc.composite_frame(FrameIndex::new(0)).expect("reference composite");

        let target = flatten_visible_op(&mut editor, &mut doc).expect("flatten");
        assert_eq!(target, seed, "flatten folds into the bottom-most visible raster layer");

        // The target cel now points at the flattened buffer; its bytes must match
        // the prior composite byte-for-byte (the reuse-the-composite-path guarantee).
        let CelData::Raster { buffer, .. } = doc.active_sprite().expect("sprite").cel(seed, FrameIndex::new(0)).expect("target cel").data else {
            panic!("target cel is raster");
        };
        let flat = doc.pixel_buffers.get(&buffer).expect("flattened buffer");
        assert_eq!(flat.as_bytes(), reference.as_bytes(), "flattened target equals the on-screen composite");

        // Only the target survives the three visible raster layers.
        assert_eq!(
            doc.active_sprite().expect("sprite").layers.len(),
            1,
            "the two upper layers collapsed into the target"
        );
    }

    #[test]
    fn flatten_visible_keeps_hidden_layers_untouched() {
        // A hidden layer sits between two visible raster layers. After flatten it
        // is still present with its pixels intact, and it is not folded into the
        // target.
        let mut doc = DocumentStore::new();
        doc.create_sprite("hero", Size::new(8, 8));
        let mut editor = EditorState::default();

        let CelData::Raster { buffer: seed_buf, .. } = doc.active_sprite().expect("sprite").cels[0].data else {
            panic!("seed cel is raster");
        };
        if let Some(buf) = doc.pixel_buffers.get_mut(&seed_buf) {
            *buf = PixelBuffer::filled(8, 8, Rgba::opaque(0, 0, 255)).expect("seed");
        }
        let hidden = add_raster_layer(&mut doc, &mut editor, "Hidden", Rgba::opaque(255, 255, 0), false);
        add_raster_layer(&mut doc, &mut editor, "Top", Rgba::opaque(255, 0, 0), true);

        let hidden_pixel_before = {
            let CelData::Raster { buffer, .. } = doc.active_sprite().expect("sprite").cel(hidden, FrameIndex::new(0)).expect("hidden cel").data else {
                panic!("raster");
            };
            doc.pixel_buffers.get(&buffer).expect("hidden buffer").pixel(4, 4).expect("pixel")
        };

        flatten_visible_op(&mut editor, &mut doc).expect("flatten");

        // The hidden layer survives, unchanged: two visible raster layers (seed +
        // top) collapsed into one, the hidden layer stayed put.
        assert!(doc.active_sprite().expect("sprite").layer(hidden).is_some(), "hidden layer still present");
        assert_eq!(
            doc.active_sprite().expect("sprite").layers.len(),
            2,
            "hidden layer plus the flatten target remain"
        );
        let CelData::Raster { buffer: hidden_after, .. } = doc.active_sprite().expect("sprite").cel(hidden, FrameIndex::new(0)).expect("hidden cel").data
        else {
            panic!("raster");
        };
        assert_eq!(
            doc.pixel_buffers.get(&hidden_after).expect("hidden buffer").pixel(4, 4),
            Some(hidden_pixel_before),
            "hidden pixels unchanged"
        );
    }

    #[test]
    fn flatten_visible_one_undo_restores_all_layers_and_pixels() {
        let mut doc = DocumentStore::new();
        doc.create_sprite("hero", Size::new(8, 8));
        let mut editor = EditorState::default();

        let seed = doc.active_sprite().expect("sprite").layers[0].id;
        let CelData::Raster { buffer: seed_buf, .. } = doc.active_sprite().expect("sprite").cels[0].data else {
            panic!("seed cel is raster");
        };
        if let Some(buf) = doc.pixel_buffers.get_mut(&seed_buf) {
            *buf = PixelBuffer::filled(8, 8, Rgba::opaque(0, 0, 255)).expect("seed");
        }
        add_raster_layer(&mut doc, &mut editor, "Mid", Rgba::opaque(0, 255, 0), true);
        add_raster_layer(&mut doc, &mut editor, "Top", Rgba::opaque(255, 0, 0), true);

        let baseline_layers = doc.active_sprite().expect("sprite").layers.len();
        let baseline_buffers = doc.pixel_buffers.len();
        let seed_pixel_before = doc.pixel_buffers.get(&seed_buf).expect("seed buffer").pixel(4, 4).expect("pixel");
        assert_eq!(seed_pixel_before, Rgba::opaque(0, 0, 255));

        flatten_visible_op(&mut editor, &mut doc).expect("flatten");
        assert_eq!(
            doc.active_sprite().expect("sprite").layers.len(),
            1,
            "three visible raster layers collapsed to one"
        );
        // Opaque red on top wins: the target shows red.
        let CelData::Raster { buffer: flat_buf, .. } = doc.active_sprite().expect("sprite").cel(seed, FrameIndex::new(0)).expect("target cel").data else {
            panic!("raster");
        };
        assert_eq!(
            doc.pixel_buffers.get(&flat_buf).expect("flat").pixel(4, 4),
            Some(Rgba::opaque(255, 0, 0)),
            "target shows the flattened pixel"
        );

        // One undo restores every layer and pixel: layer count, the seed pixel, and
        // the buffer-store size all return to baseline.
        editor.history.undo(&mut doc).expect("undo");
        assert_eq!(doc.active_sprite().expect("sprite").layers.len(), baseline_layers, "undo restores all layers");
        let CelData::Raster { buffer: restored, .. } = doc.active_sprite().expect("sprite").cel(seed, FrameIndex::new(0)).expect("seed cel").data else {
            panic!("raster");
        };
        assert_eq!(
            doc.pixel_buffers.get(&restored).expect("restored").pixel(4, 4),
            Some(seed_pixel_before),
            "undo restores the seed pixel"
        );
        assert_eq!(doc.pixel_buffers.len(), baseline_buffers, "buffer count returns to baseline");

        // Redo re-applies the flatten.
        editor.history.redo(&mut doc).expect("redo");
        assert_eq!(doc.active_sprite().expect("sprite").layers.len(), 1, "redo re-applies the flatten");
    }

    /// Reads the owning raster buffer of `(layer, frame 0)`. Test-only.
    fn frame0_buffer(doc: &DocumentStore, layer: LayerId) -> &PixelBuffer {
        let CelData::Raster { buffer, .. } = doc.active_sprite().expect("sprite").cel(layer, FrameIndex::new(0)).expect("cel").data else {
            panic!("raster cel");
        };
        doc.pixel_buffers.get(&buffer).expect("buffer")
    }

    #[test]
    fn merge_selected_three_layers_matches_a_direct_composite_into_one() {
        // Three selected raster layers (bottom blue, mid semi-green, top semi-red).
        // Merging them must leave one layer whose pixels equal compositing the
        // three inputs through `composite_layers` directly, in back-to-front order.
        let mut doc = DocumentStore::new();
        doc.create_sprite("hero", Size::new(8, 8));
        let mut editor = EditorState::default();

        let bottom = doc.active_sprite().expect("sprite").layers[0].id;
        let CelData::Raster { buffer: bottom_buf, .. } = doc.active_sprite().expect("sprite").cels[0].data else {
            panic!("seed cel is raster");
        };
        let bottom_color = Rgba::opaque(10, 20, 200);
        if let Some(buf) = doc.pixel_buffers.get_mut(&bottom_buf) {
            *buf = PixelBuffer::filled(8, 8, bottom_color).expect("bottom");
        }
        let mid_color = Rgba::new(0, 220, 40, 170);
        let top_color = Rgba::new(220, 30, 0, 120);
        let mid = add_raster_layer(&mut doc, &mut editor, "Mid", mid_color, true);
        let top = add_raster_layer(&mut doc, &mut editor, "Top", top_color, true);

        // The reference: composite the three inputs back-to-front (bottom, mid,
        // top) each with Normal/255 — the blend/opacity these layers carry.
        let b = PixelBuffer::filled(8, 8, bottom_color).expect("b");
        let m = PixelBuffer::filled(8, 8, mid_color).expect("m");
        let t = PixelBuffer::filled(8, 8, top_color).expect("t");
        let reference = composite_layers(
            8,
            8,
            &[
                LayerInput {
                    buffer: &b,
                    mode: BlendMode::Normal,
                    opacity: 255,
                    visible: true,
                    clip_below: false,
                },
                LayerInput {
                    buffer: &m,
                    mode: BlendMode::Normal,
                    opacity: 255,
                    visible: true,
                    clip_below: false,
                },
                LayerInput {
                    buffer: &t,
                    mode: BlendMode::Normal,
                    opacity: 255,
                    visible: true,
                    clip_below: false,
                },
            ],
        )
        .expect("reference composite");

        editor.selected_layers.clear();
        editor.selected_layers.insert(bottom);
        editor.selected_layers.insert(mid);
        editor.selected_layers.insert(top);

        let target = merge_selected_op(&mut editor, &mut doc).expect("merge selected");
        assert_eq!(target, top, "merge folds into the topmost selected layer");

        // Only the target survives the three selected layers.
        assert_eq!(doc.active_sprite().expect("sprite").layers.len(), 1, "three layers collapsed into the target");
        assert_eq!(
            frame0_buffer(&doc, top).as_bytes(),
            reference.as_bytes(),
            "merged target equals a direct composite of the three"
        );
    }

    #[test]
    fn merge_selected_rejects_a_single_selection_with_no_mutation() {
        let mut doc = DocumentStore::new();
        doc.create_sprite("hero", Size::new(8, 8));
        let mut editor = EditorState::default();
        let only = doc.active_sprite().expect("sprite").layers[0].id;
        add_raster_layer(&mut doc, &mut editor, "Top", Rgba::opaque(255, 0, 0), true);

        let baseline_layers = doc.active_sprite().expect("sprite").layers.len();
        let baseline_buffers = doc.pixel_buffers.len();

        editor.selected_layers.clear();
        editor.selected_layers.insert(only);

        let err = merge_selected_op(&mut editor, &mut doc).expect_err("a single selection cannot merge");
        assert_eq!(err, "Select at least two raster layers to merge");
        assert_eq!(
            doc.active_sprite().expect("sprite").layers.len(),
            baseline_layers,
            "rejection mutates no layers"
        );
        assert_eq!(doc.pixel_buffers.len(), baseline_buffers, "rejection allocates no buffers");
    }

    #[test]
    fn merge_selected_rejects_a_non_raster_member_with_no_mutation() {
        // A selection spanning two raster layers plus a group: the group makes the
        // set non-raster, so the merge is rejected and nothing changes.
        let mut doc = DocumentStore::new();
        doc.create_sprite("hero", Size::new(8, 8));
        let mut editor = EditorState::default();
        let bottom = doc.active_sprite().expect("sprite").layers[0].id;
        let top = add_raster_layer(&mut doc, &mut editor, "Top", Rgba::opaque(0, 255, 0), true);

        let group = LayerId::new(doc.alloc_id());
        push_sprite_edit_with_buffers(&mut editor, &mut doc, "Add group", Vec::new(), |sprite| {
            let mut layer = Layer::raster(group, "Group");
            layer.kind = LayerKind::Group { collapsed: false };
            sprite.layers.push(layer);
        });

        let baseline_layers = doc.active_sprite().expect("sprite").layers.len();
        let baseline_buffers = doc.pixel_buffers.len();

        editor.selected_layers.clear();
        editor.selected_layers.insert(bottom);
        editor.selected_layers.insert(top);
        editor.selected_layers.insert(group);

        let err = merge_selected_op(&mut editor, &mut doc).expect_err("a non-raster member cannot merge");
        assert_eq!(err, "Merge selected needs raster layers only");
        assert_eq!(
            doc.active_sprite().expect("sprite").layers.len(),
            baseline_layers,
            "rejection mutates no layers"
        );
        assert_eq!(doc.pixel_buffers.len(), baseline_buffers, "rejection allocates no buffers");
    }

    #[test]
    fn merge_selected_one_undo_restores_all_three_layers_and_pixels() {
        let mut doc = DocumentStore::new();
        doc.create_sprite("hero", Size::new(8, 8));
        let mut editor = EditorState::default();

        let bottom = doc.active_sprite().expect("sprite").layers[0].id;
        let CelData::Raster { buffer: bottom_buf, .. } = doc.active_sprite().expect("sprite").cels[0].data else {
            panic!("seed cel is raster");
        };
        if let Some(buf) = doc.pixel_buffers.get_mut(&bottom_buf) {
            *buf = PixelBuffer::filled(8, 8, Rgba::opaque(0, 0, 255)).expect("bottom");
        }
        let mid = add_raster_layer(&mut doc, &mut editor, "Mid", Rgba::opaque(0, 255, 0), true);
        let top = add_raster_layer(&mut doc, &mut editor, "Top", Rgba::opaque(255, 0, 0), true);

        let baseline_layers = doc.active_sprite().expect("sprite").layers.len();
        let baseline_buffers = doc.pixel_buffers.len();
        let bottom_pixel_before = frame0_buffer(&doc, bottom).pixel(4, 4).expect("pixel");
        let mid_pixel_before = frame0_buffer(&doc, mid).pixel(4, 4).expect("pixel");

        editor.selected_layers.clear();
        editor.selected_layers.insert(bottom);
        editor.selected_layers.insert(mid);
        editor.selected_layers.insert(top);

        merge_selected_op(&mut editor, &mut doc).expect("merge selected");
        assert_eq!(doc.active_sprite().expect("sprite").layers.len(), 1, "three layers collapsed to one");
        // Opaque red on top wins where it covers: the target shows red.
        assert_eq!(
            frame0_buffer(&doc, top).pixel(4, 4),
            Some(Rgba::opaque(255, 0, 0)),
            "target shows the merged pixel"
        );

        // One undo restores every layer and pixel.
        editor.history.undo(&mut doc).expect("undo");
        assert_eq!(
            doc.active_sprite().expect("sprite").layers.len(),
            baseline_layers,
            "undo restores all three layers"
        );
        assert!(doc.active_sprite().expect("sprite").layer(bottom).is_some(), "bottom restored");
        assert!(doc.active_sprite().expect("sprite").layer(mid).is_some(), "mid restored");
        assert!(doc.active_sprite().expect("sprite").layer(top).is_some(), "top restored");
        assert_eq!(
            frame0_buffer(&doc, bottom).pixel(4, 4),
            Some(bottom_pixel_before),
            "undo restores the bottom pixel"
        );
        assert_eq!(frame0_buffer(&doc, mid).pixel(4, 4), Some(mid_pixel_before), "undo restores the mid pixel");
        assert_eq!(doc.pixel_buffers.len(), baseline_buffers, "buffer count returns to baseline");

        // Redo re-applies the merge.
        editor.history.redo(&mut doc).expect("redo");
        assert_eq!(doc.active_sprite().expect("sprite").layers.len(), 1, "redo re-applies the merge");
    }

    #[test]
    fn flatten_visible_rejects_fewer_than_two_visible_raster_layers() {
        // A single visible raster seed plus a hidden raster layer: only one visible
        // raster layer, so flatten is rejected and nothing changes.
        let mut doc = DocumentStore::new();
        doc.create_sprite("hero", Size::new(8, 8));
        let mut editor = EditorState::default();
        add_raster_layer(&mut doc, &mut editor, "Hidden", Rgba::opaque(255, 0, 0), false);

        let baseline = doc.active_sprite().expect("sprite").layers.len();
        let err = flatten_visible_op(&mut editor, &mut doc).expect_err("one visible raster layer cannot flatten");
        assert_eq!(err, "Flatten needs at least two visible raster layers");
        assert_eq!(doc.active_sprite().expect("sprite").layers.len(), baseline, "rejection mutates nothing");
    }

    // --- delete layers (multi-select + confirm) -----------------------------

    #[test]
    fn delete_multi_removes_every_selected_layer_in_one_undo_with_pixels_intact() {
        // A seed plus two added raster layers. Deleting the two added layers in
        // one op must drop both, retire their buffers, and a single undo must
        // restore both layers with their pixels intact.
        let mut doc = DocumentStore::new();
        doc.create_sprite("hero", Size::new(8, 8));
        let mut editor = EditorState::default();

        let seed = doc.active_sprite().expect("sprite").layers[0].id;
        let mid_color = Rgba::opaque(0, 255, 0);
        let top_color = Rgba::opaque(255, 0, 0);
        let mid = add_raster_layer(&mut doc, &mut editor, "Mid", mid_color, true);
        let top = add_raster_layer(&mut doc, &mut editor, "Top", top_color, true);

        let baseline_layers = doc.active_sprite().expect("sprite").layers.len();
        let baseline_buffers = doc.pixel_buffers.len();
        let mid_pixel_before = frame0_buffer(&doc, mid).pixel(4, 4).expect("mid pixel");
        let top_pixel_before = frame0_buffer(&doc, top).pixel(4, 4).expect("top pixel");

        delete_layers_op(&mut editor, &mut doc, &[mid, top]).expect("delete two layers");

        // Both deleted; only the seed survives. Two buffers retired, none added.
        let sprite = doc.active_sprite().expect("sprite");
        assert_eq!(sprite.layers.len(), baseline_layers - 2, "both selected layers dropped");
        assert!(sprite.layer(mid).is_none(), "mid removed");
        assert!(sprite.layer(top).is_none(), "top removed");
        assert!(sprite.layer(seed).is_some(), "seed survives");
        assert_eq!(doc.pixel_buffers.len(), baseline_buffers - 2, "the two deleted cels' buffers retired");

        // One undo restores both layers and their pixels.
        editor.history.undo(&mut doc).expect("undo");
        let sprite = doc.active_sprite().expect("sprite");
        assert_eq!(sprite.layers.len(), baseline_layers, "undo restores both layers");
        assert!(sprite.layer(mid).is_some(), "mid restored");
        assert!(sprite.layer(top).is_some(), "top restored");
        assert_eq!(frame0_buffer(&doc, mid).pixel(4, 4), Some(mid_pixel_before), "undo restores mid pixels");
        assert_eq!(frame0_buffer(&doc, top).pixel(4, 4), Some(top_pixel_before), "undo restores top pixels");
        assert_eq!(doc.pixel_buffers.len(), baseline_buffers, "buffer count returns to baseline");

        // Redo re-applies the delete.
        editor.history.redo(&mut doc).expect("redo");
        assert_eq!(
            doc.active_sprite().expect("sprite").layers.len(),
            baseline_layers - 2,
            "redo re-applies the delete"
        );
    }

    #[test]
    fn delete_all_but_one_is_blocked_with_no_mutation() {
        // Selecting and deleting every layer must be refused so the sprite keeps
        // at least one layer, with nothing mutated.
        let mut doc = DocumentStore::new();
        doc.create_sprite("hero", Size::new(8, 8));
        let mut editor = EditorState::default();
        let seed = doc.active_sprite().expect("sprite").layers[0].id;
        let top = add_raster_layer(&mut doc, &mut editor, "Top", Rgba::opaque(255, 0, 0), true);

        let baseline_layers = doc.active_sprite().expect("sprite").layers.len();
        let baseline_buffers = doc.pixel_buffers.len();

        let err = delete_layers_op(&mut editor, &mut doc, &[seed, top]).expect_err("cannot delete every layer");
        assert_eq!(err, "Cannot delete the last layer");
        assert_eq!(
            doc.active_sprite().expect("sprite").layers.len(),
            baseline_layers,
            "rejection mutates no layers"
        );
        assert_eq!(doc.pixel_buffers.len(), baseline_buffers, "rejection retires no buffers");
    }

    #[test]
    fn delete_confirm_flag_set_for_multi_and_group_not_single_leaf() {
        // A seed leaf, a group holding one child, and the child layer. Confirm is
        // needed for a multi-layer delete and for the group-with-children, but not
        // for a single leaf.
        let mut doc = DocumentStore::new();
        doc.create_sprite("hero", Size::new(8, 8));
        let mut editor = EditorState::default();

        let leaf = doc.active_sprite().expect("sprite").layers[0].id;
        let group = LayerId::new(doc.alloc_id());
        let child = LayerId::new(doc.alloc_id());
        let child_buffer = PixelBufferId::new(doc.alloc_id());
        push_sprite_edit_with_buffers(
            &mut editor,
            &mut doc,
            "Add group and child",
            vec![(child_buffer, PixelBuffer::filled(8, 8, Rgba::opaque(0, 0, 255)).expect("child buffer"))],
            |sprite| {
                let mut g = Layer::raster(group, "Group");
                g.kind = LayerKind::Group { collapsed: false };
                sprite.layers.push(g);
                let mut c = Layer::raster(child, "Child");
                c.parent = Some(group);
                sprite.layers.push(c);
                sprite.cels.push(Cel::raster(child, FrameIndex::new(0), child_buffer, Size::new(8, 8)));
            },
        );
        let sprite = doc.active_sprite();

        // A single leaf with no children deletes straight away.
        assert!(!delete_needs_confirm(sprite, &[leaf]), "a single leaf needs no confirm");
        assert!(!delete_needs_confirm(sprite, &[child]), "a single childless layer needs no confirm");
        // More than one layer always confirms.
        assert!(delete_needs_confirm(sprite, &[leaf, child]), "a multi-layer delete confirms");
        // A group that holds children confirms even alone.
        assert!(delete_needs_confirm(sprite, &[group]), "a group with children confirms");
    }
}
