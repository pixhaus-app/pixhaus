//! Undo commands over the [`DocumentStore`] and the helpers that build them.
//!
//! Two command shapes cover every editing action, matching the two-tier split
//! the migration playbook calls for (pixel edits vs structural edits):
//!
//! - [`PixelRegionEdit`] — a bounded rectangle of one pixel buffer, storing the
//!   before/after bytes of just the dirty region. Drawing, fill, shapes, and
//!   move commit one of these. Retained memory is bounded by the painted
//!   region, not the canvas (the 8K constraint).
//! - [`SpriteEdit`] — a before/after clone of one [`Sprite`]. Structural edits
//!   (layers, frames, cels, tags, palettes) commit one of these. A `Sprite`
//!   clone is cheap because pixel bytes live in the buffer store, not the
//!   sprite — the clone copies handles and metadata only.

use std::collections::HashSet;

use pixhaus_core::canvas::PixelBuffer;
use pixhaus_core::project::library::ai::ProjectAi;
use pixhaus_core::project::{CelData, Entity, EntityId, FrameRange, LoopDirection, PixelBufferId, Size, Sprite, SpriteId, TagDefinition, TagId};
use pixhaus_core::transforms::{self, CanvasAnchor, MlaaConfig, RotationAlgorithm};
use pixhaus_core::undo::{Command, CommandError, CommandResult};

use crate::document::DocumentStore;
use crate::editor::EditorState;

/// A whole-canvas operation on the active sprite: resize, resample, flip,
/// 90°-multiple or arbitrary-angle rotation, skew, perspective warp, or
/// morphological antialias. Every variant rewrites every raster cel buffer and,
/// where dimensions change, the sprite's canvas [`Size`]. Carries only scalars,
/// so it is cheap to copy into a `spawn_blocking` closure for large canvases.
///
/// Does not derive `Eq`: the arbitrary-rotation, skew, and perspective variants
/// carry `f32` fields. The no-op guard that once relied on `Eq` compares the
/// resulting [`Sprite`]/[`PixelBuffer`], not the op, so dropping it costs
/// nothing.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum CanvasOp {
    /// Change the canvas dimensions without scaling pixels: pad transparent on
    /// grow, crop on shrink, positioned by `anchor`.
    Resize {
        /// Target width in pixels.
        width: u32,
        /// Target height in pixels.
        height: u32,
        /// Where existing content anchors in the new extent.
        anchor: CanvasAnchor,
    },
    /// Scale the pixels to a new size with nearest-neighbor sampling.
    Resample {
        /// Target width in pixels.
        width: u32,
        /// Target height in pixels.
        height: u32,
    },
    /// Mirror left-to-right.
    FlipHorizontal,
    /// Mirror top-to-bottom.
    FlipVertical,
    /// Rotate 90° clockwise (swaps width and height).
    Rotate90Cw,
    /// Rotate 90° counter-clockwise (swaps width and height).
    Rotate90Ccw,
    /// Rotate 180°.
    Rotate180,
    /// Rotate by an arbitrary angle, keeping the source dimensions. The
    /// algorithm chooses the resampling trade-off (hard-edged nearest, smooth
    /// bilinear, or diagonal-preserving `RotSprite`).
    RotateArbitrary {
        /// Clockwise angle in degrees.
        degrees: f32,
        /// Resampling algorithm.
        algorithm: RotationAlgorithm,
    },
    /// Shear the canvas, keeping the source dimensions. `fx` shifts each row
    /// right proportional to its `y`; `fy` shifts each column down proportional
    /// to its `x`. A zero factor leaves that axis unchanged.
    Skew {
        /// Horizontal shear factor.
        fx: f32,
        /// Vertical shear factor.
        fy: f32,
    },
    /// Projective warp of the four source corners `[TL, TR, BR, BL]` onto
    /// `corners`, keeping the source dimensions and sampling bilinearly.
    Perspective {
        /// Destination corners, in the order top-left, top-right, bottom-right,
        /// bottom-left.
        corners: [(f32, f32); 4],
    },
    /// Morphological antialias (MLAA): soften stair-stepped edges in two passes
    /// (rows then columns), keeping the source dimensions.
    Mlaa {
        /// Threshold and softness for the edge classifier.
        config: MlaaConfig,
    },
}

impl CanvasOp {
    /// The canvas size that results from applying this op to `current`.
    #[must_use]
    pub fn result_size(self, current: Size) -> Size {
        match self {
            Self::Resize { width, height, .. } | Self::Resample { width, height } => Size::new(width, height),
            // Arbitrary rotation, skew, perspective, and MLAA all keep the
            // source dimensions in the core algorithms.
            Self::FlipHorizontal
            | Self::FlipVertical
            | Self::Rotate180
            | Self::RotateArbitrary { .. }
            | Self::Skew { .. }
            | Self::Perspective { .. }
            | Self::Mlaa { .. } => current,
            Self::Rotate90Cw | Self::Rotate90Ccw => Size::new(current.height, current.width),
        }
    }

    /// History label shown in the undo panel.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Resize { .. } => "Resize canvas",
            Self::Resample { .. } => "Scale sprite",
            Self::FlipHorizontal => "Flip horizontal",
            Self::FlipVertical => "Flip vertical",
            Self::Rotate90Cw => "Rotate 90° CW",
            Self::Rotate90Ccw => "Rotate 90° CCW",
            Self::Rotate180 => "Rotate 180°",
            Self::RotateArbitrary { .. } => "Rotate",
            Self::Skew { .. } => "Skew",
            Self::Perspective { .. } => "Perspective",
            Self::Mlaa { .. } => "Antialias",
        }
    }

    /// Applies the op to one buffer, returning the transformed buffer.
    ///
    /// # Errors
    ///
    /// Forwards the [`transforms::Error`] from the underlying operation (an
    /// empty source buffer, a zero target dimension, or an allocation failure).
    pub fn apply(self, buf: &PixelBuffer) -> Result<PixelBuffer, transforms::Error> {
        match self {
            Self::Resize { width, height, anchor } => transforms::resize_canvas(buf, width, height, anchor),
            Self::Resample { width, height } => transforms::scale_nearest(buf, width, height),
            Self::FlipHorizontal => transforms::flip_horizontal(buf),
            Self::FlipVertical => transforms::flip_vertical(buf),
            Self::Rotate90Cw => transforms::rotate_90_cw(buf),
            Self::Rotate90Ccw => transforms::rotate_90_ccw(buf),
            Self::Rotate180 => transforms::rotate_180(buf),
            Self::RotateArbitrary { degrees, algorithm } => transforms::rotate(buf, degrees, algorithm),
            // A combined skew is one undo step: shear X first, then Y on the
            // result, so two non-zero factors stay a single op. A zero factor
            // is an identity pass, harmless to chain.
            Self::Skew { fx, fy } => transforms::skew_y(&transforms::skew_x(buf, fx)?, fy),
            Self::Perspective { corners } => transforms::perspective(buf, &corners),
            Self::Mlaa { config } => transforms::morphological_antialias(buf, &config),
        }
    }
}

/// One buffer's before/after pair within a [`CanvasEdit`].
pub struct CanvasBufferSwap {
    /// Buffer being rewritten.
    pub id: PixelBufferId,
    /// Bytes before the op.
    pub before: PixelBuffer,
    /// Bytes after the op.
    pub after: PixelBuffer,
}

/// A reversible whole-canvas op: swaps the sprite (for the canvas [`Size`] and
/// per-cel sizes) and every affected raster buffer as one undo unit. Unlike
/// [`PixelRegionEdit`], the buffers change dimensions, so the whole before/after
/// buffer is retained rather than a bounded region.
pub struct CanvasEdit {
    /// Sprite whose canvas changed.
    pub sprite_id: SpriteId,
    /// Sprite value before the op.
    pub before_sprite: Sprite,
    /// Sprite value after the op.
    pub after_sprite: Sprite,
    /// Per-buffer before/after pairs.
    pub buffers: Vec<CanvasBufferSwap>,
    /// History label.
    pub label: String,
}

impl Command<DocumentStore> for CanvasEdit {
    fn label(&self) -> &str {
        &self.label
    }

    fn apply(&mut self, doc: &mut DocumentStore) -> CommandResult {
        replace_sprite(doc, self.sprite_id, self.after_sprite.clone())?;
        for swap in &self.buffers {
            doc.pixel_buffers.insert(swap.id, swap.after.clone());
        }
        Ok(())
    }

    fn undo(&mut self, doc: &mut DocumentStore) -> CommandResult {
        replace_sprite(doc, self.sprite_id, self.before_sprite.clone())?;
        for swap in &self.buffers {
            doc.pixel_buffers.insert(swap.id, swap.before.clone());
        }
        Ok(())
    }

    fn estimated_size_bytes(&self) -> usize {
        self.buffers.iter().map(|s| s.before.as_bytes().len() + s.after.as_bytes().len()).sum()
    }
}

/// A reversible crop-to-selection: shrinks the canvas to the selection bounds
/// and copies each raster cel's sub-rectangle into a smaller buffer, as one undo
/// unit. Structurally identical to [`CanvasEdit`] — the only difference is how it
/// is built (the new size and buffers come from a crop rect, not a whole-canvas
/// op), so the apply/undo bodies match. Kept as its own type so the history label
/// and intent stay distinct from a generic canvas transform.
pub struct CropEdit {
    /// Sprite being cropped.
    pub sprite_id: SpriteId,
    /// Sprite value before the crop.
    pub before_sprite: Sprite,
    /// Sprite value after the crop (new canvas and per-cel sizes).
    pub after_sprite: Sprite,
    /// Per-buffer before/after pairs, the after being the cropped sub-rect.
    pub buffers: Vec<CanvasBufferSwap>,
    /// History label.
    pub label: String,
}

impl Command<DocumentStore> for CropEdit {
    fn label(&self) -> &str {
        &self.label
    }

    fn apply(&mut self, doc: &mut DocumentStore) -> CommandResult {
        replace_sprite(doc, self.sprite_id, self.after_sprite.clone())?;
        for swap in &self.buffers {
            doc.pixel_buffers.insert(swap.id, swap.after.clone());
        }
        Ok(())
    }

    fn undo(&mut self, doc: &mut DocumentStore) -> CommandResult {
        replace_sprite(doc, self.sprite_id, self.before_sprite.clone())?;
        for swap in &self.buffers {
            doc.pixel_buffers.insert(swap.id, swap.before.clone());
        }
        Ok(())
    }

    fn estimated_size_bytes(&self) -> usize {
        self.buffers.iter().map(|s| s.before.as_bytes().len() + s.after.as_bytes().len()).sum()
    }
}

/// Snapshots the active sprite, applies `f`, and — if anything changed — records
/// a [`SpriteEdit`] undo entry. The single entry point for every structural
/// edit (layers, frames, cels, tags, palettes). Allocate any new ids from the
/// document *before* calling this, then use them inside `f`.
pub fn push_sprite_edit(editor: &mut EditorState, doc: &mut DocumentStore, label: &str, f: impl FnOnce(&mut Sprite)) {
    let Some(id) = doc.project.active_sprite_id() else {
        return;
    };
    let Some(before) = doc.project.sprite(id).cloned() else {
        return;
    };
    let mut after = before.clone();
    f(&mut after);
    if after == before {
        return;
    }
    let cmd = SpriteEdit {
        sprite_id: id,
        before,
        after,
        label: label.to_owned(),
    };
    let _ = editor.history.push(Box::new(cmd), doc);
}

/// Like [`push_sprite_edit`], but for structural edits that also allocate new
/// pixel buffers (duplicate-frame, unlink-cel). The `added_buffers` are inserted
/// into the buffer store as part of the recorded command, so an undo removes
/// them instead of leaking them, and a redo re-inserts them. Pass buffers that
/// are *not yet* in the store; allocate their ids from the document first and
/// reference them inside `f`.
pub fn push_sprite_edit_with_buffers(
    editor: &mut EditorState,
    doc: &mut DocumentStore,
    label: &str,
    added_buffers: Vec<(PixelBufferId, PixelBuffer)>,
    f: impl FnOnce(&mut Sprite),
) {
    let Some(id) = doc.project.active_sprite_id() else {
        return;
    };
    let Some(before) = doc.project.sprite(id).cloned() else {
        return;
    };
    let mut after = before.clone();
    f(&mut after);
    if after == before {
        // Nothing references the new buffers; drop them rather than leak.
        return;
    }
    let cmd = SpriteBufferEdit {
        sprite_id: id,
        before,
        after,
        added_buffers,
        removed_buffers: Vec::new(),
        label: label.to_owned(),
    };
    let _ = editor.history.push(Box::new(cmd), doc);
}

/// Records a destructive layer consolidation (merge down, merge selected,
/// flatten visible) as one undo entry. A merge moves the sprite value and the
/// buffer store together: the merged target gets fresh buffers, the retired
/// source layers and the old destination buffers are dropped, and the sprite
/// structure loses the consumed layers and repoints the survivor's cels.
///
/// `added_buffers` carry the merged target's new bytes under fresh ids; pass
/// buffers that are *not yet* in the store, allocate their ids first, and
/// reference them from the sprite inside `f`. `removed_buffers` carry the bytes
/// of the source and old destination buffers being retired; capture those bytes
/// from the store *before* calling, because the recorded command removes them on
/// apply/redo and restores them on undo. The single `SpriteBufferEdit` keeps the
/// structural drop and the pixel rewrite reversible as one step — unlike the
/// Tauri merge, where the structural drop was not undoable.
// Callers flatten_visible and merge_selected land in later layer-ops tasks;
// merge_down (this batch) is the first to wire onto this undo path.
pub fn push_layer_consolidation(
    editor: &mut EditorState,
    doc: &mut DocumentStore,
    label: &str,
    added_buffers: Vec<(PixelBufferId, PixelBuffer)>,
    removed_buffers: Vec<(PixelBufferId, PixelBuffer)>,
    f: impl FnOnce(&mut Sprite),
) {
    let Some(id) = doc.project.active_sprite_id() else {
        return;
    };
    let Some(before) = doc.project.sprite(id).cloned() else {
        return;
    };
    let mut after = before.clone();
    f(&mut after);
    if after == before {
        // The structure did not change, so nothing references the new buffers
        // and nothing retired the old ones. Drop the edit rather than leak the
        // added buffers or wrongly remove the "retired" ones.
        return;
    }
    let cmd = SpriteBufferEdit {
        sprite_id: id,
        before,
        after,
        added_buffers,
        removed_buffers,
        label: label.to_owned(),
    };
    let _ = editor.history.push(Box::new(cmd), doc);
}

/// Runs [`DocumentStore::integrate_frames`] and records it as one undo entry,
/// so dropping an AI-generated animation onto the timeline is reversible.
///
/// `integrate_frames` mutates the document directly (it is also called from the
/// headless runner and the demo, which have no history). Here the buffer store
/// is diffed around the call to learn which buffers the integration added (the
/// new frames) and retired (the seed frame it replaced, whose bytes are
/// captured beforehand). The resulting [`SpriteBufferEdit`] re-applies the
/// already-applied state idempotently, so the push records the entry without
/// doubling the effect, and undo restores the seed while removing the frames.
///
/// `qc` is the per-frame QC record the studio's normalize review produced; it is
/// threaded into [`DocumentStore::integrate_frames`] so it lands inside the same
/// `SpriteBufferEdit` snapshot — undo removes it with the frames rather than
/// leaving a dangling record. `None` at non-reviewed sites.
pub fn integrate_frames_undoable(
    editor: &mut EditorState,
    doc: &mut DocumentStore,
    frames: Vec<PixelBuffer>,
    frame_duration_ms: u32,
    name: &str,
    loop_direction: LoopDirection,
    qc: Option<pixhaus_core::project::AnimationQc>,
) -> Option<FrameRange> {
    let sprite_id = doc.project.active_sprite_id()?;
    let before = doc.project.sprite(sprite_id)?.clone();
    let before_keys: HashSet<PixelBufferId> = doc.pixel_buffers.keys().copied().collect();
    // Capture the bytes of the seed buffers the integration might retire, so
    // undo can restore them. Superset of what is actually removed; filtered
    // down once we know the post-integration store.
    let seed_buffers: Vec<(PixelBufferId, PixelBuffer)> = before
        .cels
        .iter()
        .filter_map(|cel| match cel.data {
            CelData::Raster { buffer, .. } => doc.pixel_buffers.get(&buffer).map(|b| (buffer, b.clone())),
            _ => None,
        })
        .collect();

    let range = doc.integrate_frames(frames, frame_duration_ms, name, loop_direction, qc)?;

    let after = doc.project.sprite(sprite_id)?.clone();
    let after_keys: HashSet<PixelBufferId> = doc.pixel_buffers.keys().copied().collect();
    let added_buffers: Vec<(PixelBufferId, PixelBuffer)> = after_keys
        .difference(&before_keys)
        .filter_map(|id| doc.pixel_buffers.get(id).map(|b| (*id, b.clone())))
        .collect();
    let removed_buffers: Vec<(PixelBufferId, PixelBuffer)> = seed_buffers.into_iter().filter(|(id, _)| !after_keys.contains(id)).collect();

    let cmd = SpriteBufferEdit {
        sprite_id,
        before,
        after,
        added_buffers,
        removed_buffers,
        label: "Integrate animation".to_owned(),
    };
    let _ = editor.history.push(Box::new(cmd), doc);
    Some(range)
}

/// Snapshots a library entity, applies `f`, and — if anything changed — records
/// an [`EntityEdit`] undo entry. The entry point for library-tier edits the
/// cockpit makes: saving generated reference-sheet variants, approving a
/// canonical anchor, saving a character card. Mirrors [`push_sprite_edit`] but
/// over a whole [`Entity`] in `project.library`, since reference sheets and the
/// asset library hang off the entity, not the sprite.
pub fn push_library_edit(editor: &mut EditorState, doc: &mut DocumentStore, label: &str, entity_id: EntityId, f: impl FnOnce(&mut Entity)) {
    let Some(before) = doc.project.library.entities.iter().find(|e| e.id == entity_id).cloned() else {
        return;
    };
    let mut after = before.clone();
    f(&mut after);
    if after == before {
        return;
    }
    let cmd = EntityEdit {
        entity_id,
        before,
        after,
        label: label.to_owned(),
    };
    let _ = editor.history.push(Box::new(cmd), doc);
}

/// Snapshots the project's composition/AI library, applies `f`, and — if
/// anything changed — records an [`AiLibraryEdit`] undo entry. The entry point
/// for composition-preset CRUD (saved prompts, structures, styles), which live
/// on `project.library.ai` rather than on a sprite or entity.
pub fn push_ai_library_edit(editor: &mut EditorState, doc: &mut DocumentStore, label: &str, f: impl FnOnce(&mut ProjectAi)) {
    let before = doc.project.library.ai.clone();
    let mut after = before.clone();
    f(&mut after);
    if after == before {
        return;
    }
    let cmd = AiLibraryEdit {
        before,
        after,
        label: label.to_owned(),
    };
    let _ = editor.history.push(Box::new(cmd), doc);
}

/// Copies the `w x h` region at `(x, y)` out of `buf` into a packed
/// `w*h*4`-byte vector (row-major, tightly packed).
#[must_use]
pub fn extract_region(buf: &PixelBuffer, x: u32, y: u32, w: u32, h: u32) -> Vec<u8> {
    let mut out = vec![0u8; (w as usize) * (h as usize) * 4];
    let row_bytes = (w as usize) * 4;
    for row in 0..h {
        let Some(src) = buf.row(y + row) else { continue };
        let sx = (x as usize) * 4;
        if sx + row_bytes <= src.len() {
            let dst_start = (row as usize) * row_bytes;
            out[dst_start..dst_start + row_bytes].copy_from_slice(&src[sx..sx + row_bytes]);
        }
    }
    out
}

/// Writes a packed `w*h*4` region back into `buf` at `(x, y)`. The inverse of
/// [`extract_region`]. Out-of-bounds rows/columns are clipped.
pub fn write_region(buf: &mut PixelBuffer, x: u32, y: u32, w: u32, h: u32, data: &[u8]) {
    let row_bytes = (w as usize) * 4;
    for row in 0..h {
        let Some(dst) = buf.row_mut(y + row) else {
            continue;
        };
        let dx = (x as usize) * 4;
        let src_start = (row as usize) * row_bytes;
        if dx + row_bytes <= dst.len() && src_start + row_bytes <= data.len() {
            dst[dx..dx + row_bytes].copy_from_slice(&data[src_start..src_start + row_bytes]);
        }
    }
}

/// A reversible edit of one rectangular region of a pixel buffer.
pub struct PixelRegionEdit {
    /// Buffer the region belongs to.
    pub buffer_id: PixelBufferId,
    /// Region origin x.
    pub x: u32,
    /// Region origin y.
    pub y: u32,
    /// Region width.
    pub w: u32,
    /// Region height.
    pub h: u32,
    /// Packed bytes before the edit.
    pub before: Vec<u8>,
    /// Packed bytes after the edit.
    pub after: Vec<u8>,
    /// History label.
    pub label: String,
}

impl Command<DocumentStore> for PixelRegionEdit {
    fn label(&self) -> &str {
        &self.label
    }

    fn apply(&mut self, doc: &mut DocumentStore) -> CommandResult {
        let buf = doc
            .pixel_buffers
            .get_mut(&self.buffer_id)
            .ok_or_else(|| CommandError::Other("target buffer missing".into()))?;
        write_region(buf, self.x, self.y, self.w, self.h, &self.after);
        Ok(())
    }

    fn undo(&mut self, doc: &mut DocumentStore) -> CommandResult {
        let buf = doc
            .pixel_buffers
            .get_mut(&self.buffer_id)
            .ok_or_else(|| CommandError::Other("target buffer missing".into()))?;
        write_region(buf, self.x, self.y, self.w, self.h, &self.before);
        Ok(())
    }

    fn estimated_size_bytes(&self) -> usize {
        self.before.len() + self.after.len()
    }
}

/// A reversible structural edit: swaps one sprite's whole value.
pub struct SpriteEdit {
    /// Sprite being replaced.
    pub sprite_id: SpriteId,
    /// Value before the edit.
    pub before: Sprite,
    /// Value after the edit.
    pub after: Sprite,
    /// History label.
    pub label: String,
}

impl Command<DocumentStore> for SpriteEdit {
    fn label(&self) -> &str {
        &self.label
    }

    fn apply(&mut self, doc: &mut DocumentStore) -> CommandResult {
        replace_sprite(doc, self.sprite_id, self.after.clone())
    }

    fn undo(&mut self, doc: &mut DocumentStore) -> CommandResult {
        replace_sprite(doc, self.sprite_id, self.before.clone())
    }

    fn estimated_size_bytes(&self) -> usize {
        // Rough: cels + layers + frames metadata. Pixel bytes are not here.
        (self.before.cels.len() + self.after.cels.len()) * 64
    }
}

/// A structural edit that also adds and/or retires pixel buffers, so the sprite
/// value and the buffer-store membership move as one undo unit. Without this,
/// edits that allocate buffers (duplicate-frame, unlink-cel) or retire them
/// (animation integration replacing a seed frame) leaked or lost buffers on
/// undo, because the sprite snapshot only carries handles, not the bytes.
pub struct SpriteBufferEdit {
    /// Sprite being replaced.
    pub sprite_id: SpriteId,
    /// Value before the edit.
    pub before: Sprite,
    /// Value after the edit.
    pub after: Sprite,
    /// Buffers the edit allocates: inserted on apply/redo, removed on undo.
    pub added_buffers: Vec<(PixelBufferId, PixelBuffer)>,
    /// Buffers the edit retires: removed on apply/redo, restored on undo.
    pub removed_buffers: Vec<(PixelBufferId, PixelBuffer)>,
    /// History label.
    pub label: String,
}

impl Command<DocumentStore> for SpriteBufferEdit {
    fn label(&self) -> &str {
        &self.label
    }

    fn apply(&mut self, doc: &mut DocumentStore) -> CommandResult {
        replace_sprite(doc, self.sprite_id, self.after.clone())?;
        for (id, buf) in &self.added_buffers {
            doc.pixel_buffers.insert(*id, buf.clone());
        }
        for (id, _) in &self.removed_buffers {
            doc.pixel_buffers.remove(id);
        }
        Ok(())
    }

    fn undo(&mut self, doc: &mut DocumentStore) -> CommandResult {
        replace_sprite(doc, self.sprite_id, self.before.clone())?;
        for (id, _) in &self.added_buffers {
            doc.pixel_buffers.remove(id);
        }
        for (id, buf) in &self.removed_buffers {
            doc.pixel_buffers.insert(*id, buf.clone());
        }
        Ok(())
    }

    fn estimated_size_bytes(&self) -> usize {
        let buf_bytes = |list: &[(PixelBufferId, PixelBuffer)]| list.iter().map(|(_, b)| b.as_bytes().len()).sum::<usize>();
        buf_bytes(&self.added_buffers) + buf_bytes(&self.removed_buffers) + (self.before.cels.len() + self.after.cels.len()) * 64
    }
}

/// Replaces the sprite with `id` in the document's project.
fn replace_sprite(doc: &mut DocumentStore, id: SpriteId, value: Sprite) -> CommandResult {
    let slot = doc.project.sprite_mut(id).ok_or_else(|| CommandError::Other("target sprite missing".into()))?;
    *slot = value;
    Ok(())
}

/// A reversible structural edit of one library entity: swaps its whole value.
/// Carries the embedded reference sheet, so generated variants and the approved
/// anchor undo/redo as a unit.
pub struct EntityEdit {
    /// Entity being replaced.
    pub entity_id: EntityId,
    /// Value before the edit.
    pub before: Entity,
    /// Value after the edit.
    pub after: Entity,
    /// History label.
    pub label: String,
}

impl Command<DocumentStore> for EntityEdit {
    fn label(&self) -> &str {
        &self.label
    }

    fn apply(&mut self, doc: &mut DocumentStore) -> CommandResult {
        replace_entity(doc, self.entity_id, self.after.clone())
    }

    fn undo(&mut self, doc: &mut DocumentStore) -> CommandResult {
        replace_entity(doc, self.entity_id, self.before.clone())
    }

    fn estimated_size_bytes(&self) -> usize {
        // Dominated by the embedded reference-sheet images on each side.
        sheet_bytes(&self.before) + sheet_bytes(&self.after)
    }
}

/// Replaces the entity with `id` in the library.
fn replace_entity(doc: &mut DocumentStore, id: EntityId, value: Entity) -> CommandResult {
    let slot = doc
        .project
        .library
        .entities
        .iter_mut()
        .find(|e| e.id == id)
        .ok_or_else(|| CommandError::Other("target entity missing".into()))?;
    *slot = value;
    Ok(())
}

/// A reversible edit of the project's composition/AI library: swaps the whole
/// [`ProjectAi`] value. Carries the saved prompts, structures, and styles so a
/// preset add/edit/remove undoes and redoes as a unit.
pub struct AiLibraryEdit {
    /// Value before the edit.
    pub before: ProjectAi,
    /// Value after the edit.
    pub after: ProjectAi,
    /// History label.
    pub label: String,
}

impl Command<DocumentStore> for AiLibraryEdit {
    fn label(&self) -> &str {
        &self.label
    }

    fn apply(&mut self, doc: &mut DocumentStore) -> CommandResult {
        doc.project.library.ai = self.after.clone();
        Ok(())
    }

    fn undo(&mut self, doc: &mut DocumentStore) -> CommandResult {
        doc.project.library.ai = self.before.clone();
        Ok(())
    }

    fn estimated_size_bytes(&self) -> usize {
        // Rough per-record estimate; the bytes live in prompt/structure/style
        // text, not pixel buffers. Enough for the undo memory cap.
        let count = |ai: &ProjectAi| ai.prompts.len() + ai.structures.len() + ai.styles.len();
        (count(&self.before) + count(&self.after)) * 256
    }
}

/// A reversible edit of the project's library tag definitions and the tag ids
/// attached to entities. Tag definitions live on `library.tags` (not
/// `ProjectAi`), and deleting one prunes it from every entity's confirmed and
/// suggested lists, so the snapshot covers both sides as one undo entry.
///
/// The snapshot is cheap: it copies the tag definitions plus each entity's two
/// `Vec<TagId>` lists, never the entities' sprites or reference-sheet bytes.
pub struct LibraryTagEdit {
    /// Tag definitions before the edit.
    pub before_tags: Vec<TagDefinition>,
    /// Tag definitions after the edit.
    pub after_tags: Vec<TagDefinition>,
    /// Per-entity `(confirmed, suggested)` tag ids before the edit.
    pub before_entities: Vec<(EntityId, Vec<TagId>, Vec<TagId>)>,
    /// Per-entity `(confirmed, suggested)` tag ids after the edit.
    pub after_entities: Vec<(EntityId, Vec<TagId>, Vec<TagId>)>,
    /// History label.
    pub label: String,
}

impl Command<DocumentStore> for LibraryTagEdit {
    fn label(&self) -> &str {
        &self.label
    }

    fn apply(&mut self, doc: &mut DocumentStore) -> CommandResult {
        restore_tags(doc, &self.after_tags, &self.after_entities);
        Ok(())
    }

    fn undo(&mut self, doc: &mut DocumentStore) -> CommandResult {
        restore_tags(doc, &self.before_tags, &self.before_entities);
        Ok(())
    }

    fn estimated_size_bytes(&self) -> usize {
        // A tag id is 4 bytes; tag definitions carry a short name string. This is
        // metadata, not pixels — a rough constant per record keeps the cap honest
        // without walking strings.
        let tag_bytes = (self.before_tags.len() + self.after_tags.len()) * 32;
        let entity_lists = self.before_entities.iter().chain(&self.after_entities);
        let id_bytes: usize = entity_lists.map(|(_, c, s)| (c.len() + s.len()) * 4).sum();
        tag_bytes + id_bytes
    }
}

/// Overwrites the library's tag definitions and restores each named entity's
/// confirmed and suggested tag lists. Entities absent from the snapshot are left
/// untouched.
fn restore_tags(doc: &mut DocumentStore, tags: &[TagDefinition], entities: &[(EntityId, Vec<TagId>, Vec<TagId>)]) {
    doc.project.library.tags = tags.to_vec();
    for (id, confirmed, suggested) in entities {
        if let Some(entity) = doc.project.library.entities.iter_mut().find(|e| e.id == *id) {
            entity.tags.clone_from(confirmed);
            entity.ai.suggested_tags.clone_from(suggested);
        }
    }
}

/// Snapshots the library tag definitions and every entity's tag lists, applies
/// `f`, and — if anything changed — records a [`LibraryTagEdit`] undo entry. The
/// entry point for tag-definition CRUD and per-entity tagging, both of which can
/// touch `library.tags` and the entities' tag lists in one step (delete prunes).
pub fn push_tag_edit(editor: &mut EditorState, doc: &mut DocumentStore, label: &str, f: impl FnOnce(&mut DocumentStore)) {
    let before_tags = doc.project.library.tags.clone();
    let before_entities = entity_tag_snapshot(doc);

    f(doc);

    let after_tags = doc.project.library.tags.clone();
    let after_entities = entity_tag_snapshot(doc);
    if after_tags == before_tags && after_entities == before_entities {
        return;
    }

    let cmd = LibraryTagEdit {
        before_tags,
        after_tags,
        before_entities,
        after_entities,
        label: label.to_owned(),
    };
    let _ = editor.history.push(Box::new(cmd), doc);
}

/// Captures every entity's `(id, confirmed_tags, suggested_tags)` for the tag
/// undo snapshot.
fn entity_tag_snapshot(doc: &DocumentStore) -> Vec<(EntityId, Vec<TagId>, Vec<TagId>)> {
    doc.project
        .library
        .entities
        .iter()
        .map(|e| (e.id, e.tags.clone(), e.ai.suggested_tags.clone()))
        .collect()
}

/// Rough byte cost of an entity's embedded reference-sheet images, for the
/// undo memory cap.
fn sheet_bytes(entity: &Entity) -> usize {
    use pixhaus_core::project::EntityContent;
    let EntityContent::Sprites {
        reference_sheet: Some(sheet), ..
    } = &entity.content
    else {
        return 0;
    };
    let variant_bytes = |v: &pixhaus_core::project::SheetVariant| v.image.bytes.len();
    sheet.canonical.as_ref().map_or(0, variant_bytes) + sheet.variants.iter().map(variant_bytes).sum::<usize>()
}

#[cfg(test)]
mod tests {
    use pixhaus_core::project::{EntityContent, ReferenceImage, ReferenceSheet, SheetVariant, SheetVariantId, Size};

    use super::*;
    use crate::editor::EditorState;

    fn variant_count(doc: &DocumentStore, entity_id: EntityId) -> usize {
        doc.project
            .library
            .entities
            .iter()
            .find(|e| e.id == entity_id)
            .and_then(|e| match &e.content {
                EntityContent::Sprites { reference_sheet, .. } => reference_sheet.as_ref().map(|s| s.variants.len()),
                _ => None,
            })
            .unwrap_or(0)
    }

    #[test]
    fn library_edit_persists_variant_and_undo_redo_round_trips() {
        let mut doc = DocumentStore::new();
        doc.create_sprite("hero", Size::new(16, 16));
        let entity_id = doc.active_entity_id().expect("active entity");
        let mut editor = EditorState::default();

        push_library_edit(&mut editor, &mut doc, "Generate", entity_id, |entity| {
            if let EntityContent::Sprites { reference_sheet, .. } = &mut entity.content {
                let sheet = reference_sheet.get_or_insert_with(|| Box::new(ReferenceSheet::default()));
                let image = ReferenceImage {
                    bytes: vec![1, 2, 3, 4],
                    mime: "image/png".into(),
                };
                sheet.variants.push(SheetVariant::from_image(SheetVariantId::new(99), 0, image));
            }
        });

        assert_eq!(variant_count(&doc, entity_id), 1, "variant persisted");
        editor.history.undo(&mut doc).expect("undo");
        assert_eq!(variant_count(&doc, entity_id), 0, "undo removes the variant");
        editor.history.redo(&mut doc).expect("redo");
        assert_eq!(variant_count(&doc, entity_id), 1, "redo restores the variant");
    }

    #[test]
    fn library_edit_with_no_change_pushes_nothing() {
        let mut doc = DocumentStore::new();
        doc.create_sprite("hero", Size::new(8, 8));
        let entity_id = doc.active_entity_id().expect("active entity");
        let mut editor = EditorState::default();

        // A closure that changes nothing must not record an undo entry.
        push_library_edit(&mut editor, &mut doc, "noop", entity_id, |_entity| {});
        assert!(editor.history.undo(&mut doc).is_err(), "no entry was pushed");
    }

    #[test]
    fn ai_library_edit_persists_prompt_and_undo_redo_round_trips() {
        use pixhaus_core::project::library::composition::{PromptId, PromptTemplate};

        let mut doc = DocumentStore::new();
        let mut editor = EditorState::default();
        let prompt_count = |doc: &DocumentStore| doc.project.library.ai.prompts.len();

        assert_eq!(prompt_count(&doc), 0);
        push_ai_library_edit(&mut editor, &mut doc, "Save template", |ai| {
            ai.prompts.push(PromptTemplate {
                id: PromptId("project.prompt.1".into()),
                name: "Hero".into(),
                text: "a {species} hero".into(),
                variables: Vec::new(),
                default_style: None,
                default_structure: None,
            });
        });

        assert_eq!(prompt_count(&doc), 1, "template persisted");
        editor.history.undo(&mut doc).expect("undo");
        assert_eq!(prompt_count(&doc), 0, "undo removes the template");
        editor.history.redo(&mut doc).expect("redo");
        assert_eq!(prompt_count(&doc), 1, "redo restores the template");
    }

    #[test]
    fn ai_library_edit_with_no_change_pushes_nothing() {
        let mut doc = DocumentStore::new();
        let mut editor = EditorState::default();
        push_ai_library_edit(&mut editor, &mut doc, "noop", |_ai| {});
        assert!(editor.history.undo(&mut doc).is_err(), "no entry was pushed");
    }

    #[test]
    fn sprite_buffer_edit_adds_and_removes_buffer_with_undo() {
        use pixhaus_core::project::{Cel, CelData, FrameIndex, PixelBufferId};

        let mut doc = DocumentStore::new();
        doc.create_sprite("hero", Size::new(8, 8));
        let layer = doc.active_sprite().expect("sprite").layers[0].id;
        let before_count = doc.pixel_buffers.len();
        let new_id = PixelBufferId::new(doc.alloc_id());
        let buf = PixelBuffer::filled(8, 8, pixhaus_core::project::Rgba::new(1, 2, 3, 255)).expect("buffer");

        let mut editor = EditorState::default();
        push_sprite_edit_with_buffers(&mut editor, &mut doc, "Add cel", vec![(new_id, buf)], |sprite| {
            sprite.cels.push(Cel::raster(layer, FrameIndex::new(0), new_id, Size::new(8, 8)));
        });

        assert!(doc.pixel_buffers.contains_key(&new_id), "buffer inserted by the command");
        assert_eq!(doc.pixel_buffers.len(), before_count + 1);

        editor.history.undo(&mut doc).expect("undo");
        assert!(!doc.pixel_buffers.contains_key(&new_id), "undo removes the buffer instead of leaking it");
        assert_eq!(doc.pixel_buffers.len(), before_count);
        assert!(
            !doc.active_sprite()
                .expect("sprite")
                .cels
                .iter()
                .any(|c| matches!(c.data, CelData::Raster { buffer, .. } if buffer == new_id)),
            "undo also drops the cel"
        );

        editor.history.redo(&mut doc).expect("redo");
        assert!(doc.pixel_buffers.contains_key(&new_id), "redo restores the buffer");
        assert_eq!(doc.pixel_buffers.len(), before_count + 1);
    }

    #[test]
    fn layer_consolidation_swaps_buffers_and_structure_with_undo() {
        use pixhaus_core::project::{Cel, CelData, FrameIndex, Layer, LayerId, PixelBufferId, Rgba};

        let mut doc = DocumentStore::new();
        doc.create_sprite("hero", Size::new(8, 8));

        // A second raster layer with its own cel and buffer, so the merge has a
        // distinct source to retire onto the base destination.
        let dest_layer = doc.active_sprite().expect("sprite").layers[0].id;
        let CelData::Raster { buffer: dest_buffer, .. } = doc.active_sprite().expect("sprite").cels[0].data else {
            panic!("seed cel is raster")
        };
        let src_layer = LayerId::new(doc.alloc_id());
        let src_buffer = PixelBufferId::new(doc.alloc_id());
        let mut editor = EditorState::default();
        push_sprite_edit_with_buffers(
            &mut editor,
            &mut doc,
            "Add layer",
            vec![(src_buffer, PixelBuffer::filled(8, 8, Rgba::new(0, 0, 255, 255)).expect("buffer"))],
            |sprite| {
                sprite.layers.push(Layer::raster(src_layer, "Layer 2"));
                sprite.cels.push(Cel::raster(src_layer, FrameIndex::new(0), src_buffer, Size::new(8, 8)));
            },
        );
        let baseline_layers = doc.active_sprite().expect("sprite").layers.len();
        let baseline_buffers = doc.pixel_buffers.len();

        // Capture the retired buffers' bytes before the command removes them.
        let removed = vec![
            (src_buffer, doc.pixel_buffers.get(&src_buffer).expect("src buffer").clone()),
            (dest_buffer, doc.pixel_buffers.get(&dest_buffer).expect("dest buffer").clone()),
        ];
        // The merged target lands under a fresh id.
        let merged_id = PixelBufferId::new(doc.alloc_id());
        let merged = PixelBuffer::filled(8, 8, Rgba::new(0, 0, 255, 255)).expect("merged");

        push_layer_consolidation(&mut editor, &mut doc, "Merge down", vec![(merged_id, merged)], removed, |sprite| {
            sprite.remove_layer(src_layer);
            // Repoint the surviving destination cel to the merged buffer.
            for cel in &mut sprite.cels {
                if cel.layer_id == dest_layer {
                    if let CelData::Raster { buffer, .. } = &mut cel.data {
                        *buffer = merged_id;
                    }
                }
            }
        });

        // The source layer is gone and the buffer store swapped the two retired
        // buffers for the one merged buffer.
        assert_eq!(doc.active_sprite().expect("sprite").layers.len(), baseline_layers - 1, "source layer dropped");
        assert!(doc.pixel_buffers.contains_key(&merged_id), "merged buffer inserted");
        assert!(!doc.pixel_buffers.contains_key(&src_buffer), "source buffer retired");
        assert!(!doc.pixel_buffers.contains_key(&dest_buffer), "old destination buffer retired");
        assert_eq!(doc.pixel_buffers.len(), baseline_buffers - 1, "two retired, one added");

        editor.history.undo(&mut doc).expect("undo");
        assert_eq!(
            doc.active_sprite().expect("sprite").layers.len(),
            baseline_layers,
            "undo restores the source layer"
        );
        assert!(doc.pixel_buffers.contains_key(&src_buffer), "undo restores the source buffer");
        assert!(doc.pixel_buffers.contains_key(&dest_buffer), "undo restores the destination buffer");
        assert!(!doc.pixel_buffers.contains_key(&merged_id), "undo removes the merged buffer");
        assert_eq!(doc.pixel_buffers.len(), baseline_buffers, "buffer count returns to baseline");

        editor.history.redo(&mut doc).expect("redo");
        assert_eq!(
            doc.active_sprite().expect("sprite").layers.len(),
            baseline_layers - 1,
            "redo re-applies the merge"
        );
        assert!(doc.pixel_buffers.contains_key(&merged_id), "redo restores the merged buffer");
        assert_eq!(doc.pixel_buffers.len(), baseline_buffers - 1);
    }

    #[test]
    fn integrate_frames_undoable_round_trips() {
        let mut doc = DocumentStore::new();
        doc.create_sprite("hero", Size::new(8, 8));
        let mut editor = EditorState::default();
        let frames: Vec<PixelBuffer> = (0..3)
            .map(|_| PixelBuffer::filled(8, 8, pixhaus_core::project::Rgba::new(9, 9, 9, 255)).expect("buffer"))
            .collect();

        integrate_frames_undoable(&mut editor, &mut doc, frames, 100, "walk", LoopDirection::Forward, None).expect("integrated range");
        // The pristine seed is replaced, leaving exactly the three frames.
        assert_eq!(doc.frame_count(), 3);
        assert_eq!(doc.pixel_buffers.len(), 3);

        editor.history.undo(&mut doc).expect("undo");
        assert_eq!(doc.frame_count(), 1, "undo restores the single seed frame");
        assert_eq!(doc.pixel_buffers.len(), 1, "undo restores the seed buffer and drops the integrated ones");

        editor.history.redo(&mut doc).expect("redo");
        assert_eq!(doc.frame_count(), 3, "redo re-integrates the animation");
        assert_eq!(doc.pixel_buffers.len(), 3);
    }

    /// A small populated QC record for the integrate-with-qc test.
    fn sample_animation_qc() -> pixhaus_core::project::AnimationQc {
        use pixhaus_core::project::qc::{AnimationQc, FrameQc, NormalizeReportSummary, QcSettings};
        use pixhaus_core::project::Rgba;
        use pixhaus_core::transforms::SeamMatch;
        AnimationQc {
            settings: QcSettings {
                key_color: Rgba::opaque(255, 0, 255),
                key_tolerance: 24,
                alpha_threshold: 8,
                canvas: (8, 8),
                bottom_margin: 0,
                reference_height: 6,
                remove_on_land: true,
            },
            report: NormalizeReportSummary {
                baseline_drift_px: 0,
                scale_match_pct: 100,
                seam: SeamMatch::Ok,
                reference_height: 6,
                max_components: 1,
                any_edge_touch: false,
                warnings: Vec::new(),
            },
            frames: vec![FrameQc {
                bbox: (1, 1, 6, 6),
                center_x: 4,
                foot_baseline_y: 6,
                empty: false,
                num_components: 1,
                largest_component_area: 36,
                largest_component_bbox: Some((1, 1, 6, 6)),
                edge_touch: false,
            }],
        }
    }

    #[test]
    fn integrate_frames_undoable_lands_qc_and_undo_removes_it() {
        let mut doc = DocumentStore::new();
        doc.create_sprite("hero", Size::new(8, 8));
        let mut editor = EditorState::default();
        let frames: Vec<PixelBuffer> = (0..3)
            .map(|_| PixelBuffer::filled(8, 8, pixhaus_core::project::Rgba::new(9, 9, 9, 255)).expect("buffer"))
            .collect();
        let qc = sample_animation_qc();

        integrate_frames_undoable(&mut editor, &mut doc, frames, 100, "walk", LoopDirection::Forward, Some(qc.clone())).expect("integrated range");

        // The landed Animation carries the QC record, inside the same undo entry.
        let landed = doc.active_sprite().expect("sprite").animations.last().expect("animation").clone();
        assert_eq!(landed.qc, Some(qc), "the landed animation carries the qc record");

        editor.history.undo(&mut doc).expect("undo");
        // Undo removed the whole animation (and its qc) along with the frames.
        assert!(
            doc.active_sprite().expect("sprite").animations.is_empty(),
            "undo removes the animation and its qc rather than leaving a dangling record"
        );

        editor.history.redo(&mut doc).expect("redo");
        let relanded = doc.active_sprite().expect("sprite").animations.last().expect("animation").clone();
        assert!(relanded.qc.is_some(), "redo restores the qc record with the animation");
    }

    #[test]
    fn canvas_op_result_size_tracks_each_variant() {
        let s = Size::new(16, 8);
        assert_eq!(CanvasOp::Rotate90Cw.result_size(s), Size::new(8, 16));
        assert_eq!(CanvasOp::Rotate90Ccw.result_size(s), Size::new(8, 16));
        assert_eq!(CanvasOp::Rotate180.result_size(s), s);
        assert_eq!(CanvasOp::FlipHorizontal.result_size(s), s);
        assert_eq!(
            CanvasOp::Resize {
                width: 4,
                height: 4,
                anchor: CanvasAnchor::Center
            }
            .result_size(s),
            Size::new(4, 4)
        );
        assert_eq!(CanvasOp::Resample { width: 32, height: 32 }.result_size(s), Size::new(32, 32));
        // The new size-preserving ops keep the source dimensions.
        assert_eq!(
            CanvasOp::RotateArbitrary {
                degrees: 33.0,
                algorithm: RotationAlgorithm::RotSprite
            }
            .result_size(s),
            s
        );
        assert_eq!(CanvasOp::Skew { fx: 0.5, fy: -0.25 }.result_size(s), s);
        assert_eq!(
            CanvasOp::Perspective {
                corners: [(0.0, 0.0), (16.0, 0.0), (16.0, 8.0), (0.0, 8.0)]
            }
            .result_size(s),
            s
        );
        assert_eq!(CanvasOp::Mlaa { config: MlaaConfig::default() }.result_size(s), s);
    }

    #[test]
    fn canvas_op_labels_name_the_new_ops() {
        assert_eq!(
            CanvasOp::RotateArbitrary {
                degrees: 12.0,
                algorithm: RotationAlgorithm::default()
            }
            .label(),
            "Rotate"
        );
        assert_eq!(CanvasOp::Skew { fx: 0.0, fy: 0.0 }.label(), "Skew");
        assert_eq!(CanvasOp::Perspective { corners: [(0.0, 0.0); 4] }.label(), "Perspective");
        assert_eq!(CanvasOp::Mlaa { config: MlaaConfig::default() }.label(), "Antialias");
    }

    #[test]
    fn rotate_arbitrary_zero_is_identity_and_preserves_size() {
        let buf = PixelBuffer::filled(8, 4, pixhaus_core::project::Rgba::new(10, 20, 30, 255)).expect("buffer");
        let out = CanvasOp::RotateArbitrary {
            degrees: 0.0,
            algorithm: RotationAlgorithm::NearestNeighbor,
        }
        .apply(&buf)
        .expect("rotate");
        assert_eq!((out.width(), out.height()), (8, 4));
        assert_eq!(out, buf, "0° rotation is identity");
    }

    #[test]
    fn skew_combined_keeps_dimensions_and_zero_is_identity() {
        let mut buf = PixelBuffer::new(6, 6).expect("buffer");
        buf.set_pixel(2, 2, pixhaus_core::project::Rgba::new(255, 0, 0, 255));
        let identity = CanvasOp::Skew { fx: 0.0, fy: 0.0 }.apply(&buf).expect("skew");
        assert_eq!(identity, buf, "zero skew is identity");
        let sheared = CanvasOp::Skew { fx: 1.0, fy: 0.0 }.apply(&buf).expect("skew");
        assert_eq!((sheared.width(), sheared.height()), (6, 6), "skew preserves dimensions");
    }

    #[test]
    fn perspective_identity_corners_preserve_size() {
        let buf = PixelBuffer::filled(8, 8, pixhaus_core::project::Rgba::new(40, 50, 60, 255)).expect("buffer");
        let out = CanvasOp::Perspective {
            corners: [(0.0, 0.0), (7.0, 0.0), (7.0, 7.0), (0.0, 7.0)],
        }
        .apply(&buf)
        .expect("perspective");
        assert_eq!((out.width(), out.height()), (8, 8));
    }

    #[test]
    fn mlaa_zero_softness_is_identity() {
        let buf = PixelBuffer::filled(8, 8, pixhaus_core::project::Rgba::new(1, 2, 3, 255)).expect("buffer");
        let out = CanvasOp::Mlaa {
            config: MlaaConfig { threshold: 16, softness: 0 },
        }
        .apply(&buf)
        .expect("mlaa");
        assert_eq!(out, buf, "softness 0 is identity");
    }

    #[test]
    fn canvas_edit_round_trips_an_arbitrary_rotation() {
        use pixhaus_core::project::CelData;

        let mut doc = DocumentStore::new();
        doc.create_sprite("hero", Size::new(8, 8));
        let sprite_id = doc.project.active_sprite_id().expect("active sprite");
        let before_sprite = doc.project.sprite(sprite_id).expect("sprite").clone();
        let buffer_id = before_sprite
            .cels
            .iter()
            .find_map(|c| match c.data {
                CelData::Raster { buffer, .. } => Some(buffer),
                _ => None,
            })
            .expect("raster buffer");
        let mut before_buf = doc.pixel_buffers.get(&buffer_id).expect("buffer").clone();
        before_buf.set_pixel(1, 1, pixhaus_core::project::Rgba::new(200, 0, 0, 255));
        doc.pixel_buffers.insert(buffer_id, before_buf.clone());

        let op = CanvasOp::RotateArbitrary {
            degrees: 45.0,
            algorithm: RotationAlgorithm::RotSprite,
        };
        let after_buf = op.apply(&before_buf).expect("rotate");
        // The op keeps the canvas size, so the sprite value is unchanged.
        let edit = CanvasEdit {
            sprite_id,
            before_sprite: before_sprite.clone(),
            after_sprite: before_sprite,
            buffers: vec![CanvasBufferSwap {
                id: buffer_id,
                before: before_buf.clone(),
                after: after_buf.clone(),
            }],
            label: op.label().to_owned(),
        };

        let mut editor = EditorState::default();
        editor.history.push(Box::new(edit), &mut doc).expect("push");
        assert_eq!(doc.pixel_buffers.get(&buffer_id), Some(&after_buf), "rotation applied");
        editor.history.undo(&mut doc).expect("undo");
        assert_eq!(doc.pixel_buffers.get(&buffer_id), Some(&before_buf), "undo restores the source");
    }

    #[test]
    fn canvas_edit_resize_applies_and_undoes() {
        use pixhaus_core::project::CelData;

        let mut doc = DocumentStore::new();
        doc.create_sprite("hero", Size::new(8, 8));
        let sprite_id = doc.project.active_sprite_id().expect("active sprite");
        let before_sprite = doc.project.sprite(sprite_id).expect("sprite").clone();
        let buffer_id = before_sprite
            .cels
            .iter()
            .find_map(|c| match c.data {
                CelData::Raster { buffer, .. } => Some(buffer),
                _ => None,
            })
            .expect("raster buffer");
        let before_buf = doc.pixel_buffers.get(&buffer_id).expect("buffer").clone();
        let after_buf = CanvasOp::Resize {
            width: 16,
            height: 16,
            anchor: CanvasAnchor::Center,
        }
        .apply(&before_buf)
        .expect("resize");

        let mut after_sprite = before_sprite.clone();
        after_sprite.canvas = Size::new(16, 16);
        for cel in &mut after_sprite.cels {
            if let CelData::Raster { size, .. } = &mut cel.data {
                *size = Size::new(16, 16);
            }
        }

        let edit = CanvasEdit {
            sprite_id,
            before_sprite,
            after_sprite,
            buffers: vec![CanvasBufferSwap {
                id: buffer_id,
                before: before_buf,
                after: after_buf,
            }],
            label: "Resize canvas".into(),
        };

        let mut editor = EditorState::default();
        editor.history.push(Box::new(edit), &mut doc).expect("push");
        assert_eq!(doc.project.sprite(sprite_id).unwrap().canvas, Size::new(16, 16));
        assert_eq!(doc.pixel_buffers.get(&buffer_id).unwrap().width(), 16);

        editor.history.undo(&mut doc).expect("undo");
        assert_eq!(doc.project.sprite(sprite_id).unwrap().canvas, Size::new(8, 8));
        assert_eq!(doc.pixel_buffers.get(&buffer_id).unwrap().width(), 8);

        editor.history.redo(&mut doc).expect("redo");
        assert_eq!(doc.pixel_buffers.get(&buffer_id).unwrap().width(), 16);
    }

    #[test]
    fn crop_edit_shrinks_canvas_and_undo_restores_size_and_pixels() {
        use pixhaus_core::project::{CelData, Rgba};

        let mut doc = DocumentStore::new();
        doc.create_sprite("hero", Size::new(8, 8));
        let sprite_id = doc.project.active_sprite_id().expect("active sprite");
        let before_sprite = doc.project.sprite(sprite_id).expect("sprite").clone();
        let buffer_id = before_sprite
            .cels
            .iter()
            .find_map(|c| match c.data {
                CelData::Raster { buffer, .. } => Some(buffer),
                _ => None,
            })
            .expect("raster buffer");

        // Paint a marker inside the crop rect and one outside it.
        let mut before_buf = doc.pixel_buffers.get(&buffer_id).expect("buffer").clone();
        before_buf.set_pixel(3, 3, Rgba::new(200, 0, 0, 255)); // inside (2,2,4,4)
        before_buf.set_pixel(0, 0, Rgba::new(0, 200, 0, 255)); // outside it
        doc.pixel_buffers.insert(buffer_id, before_buf.clone());

        // Crop the 4x4 rect rooted at (2, 2).
        let (cx, cy, cw, ch) = (2u32, 2u32, 4u32, 4u32);
        let after_buf = transforms::crop(&before_buf, cx, cy, cw, ch).expect("crop");
        let mut after_sprite = before_sprite.clone();
        after_sprite.canvas = Size::new(cw, ch);
        for cel in &mut after_sprite.cels {
            if let CelData::Raster { size, .. } = &mut cel.data {
                *size = Size::new(cw, ch);
            }
        }

        let edit = CropEdit {
            sprite_id,
            before_sprite,
            after_sprite,
            buffers: vec![CanvasBufferSwap {
                id: buffer_id,
                before: before_buf.clone(),
                after: after_buf.clone(),
            }],
            label: "Crop to selection".into(),
        };

        let mut editor = EditorState::default();
        editor.history.push(Box::new(edit), &mut doc).expect("push");
        assert_eq!(doc.project.sprite(sprite_id).unwrap().canvas, Size::new(4, 4), "canvas shrank to the crop");
        let cropped = doc.pixel_buffers.get(&buffer_id).unwrap();
        assert_eq!((cropped.width(), cropped.height()), (4, 4));
        // The inside marker moved to rect-local (1, 1); the outside one is gone.
        assert_eq!(cropped.pixel(1, 1), Some(Rgba::new(200, 0, 0, 255)), "inside pixel kept");
        let opaque = cropped.pixels().filter(|p| p.a != 0).count();
        assert_eq!(opaque, 1, "the pixel outside the crop rect is discarded");

        editor.history.undo(&mut doc).expect("undo");
        assert_eq!(doc.project.sprite(sprite_id).unwrap().canvas, Size::new(8, 8), "undo restores the canvas size");
        let restored = doc.pixel_buffers.get(&buffer_id).unwrap();
        assert_eq!(restored, &before_buf, "undo restores every pixel");

        editor.history.redo(&mut doc).expect("redo");
        assert_eq!(doc.project.sprite(sprite_id).unwrap().canvas, Size::new(4, 4), "redo re-crops");
    }
}
