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

use pixhaus_core::canvas::PixelBuffer;
use pixhaus_core::project::library::ai::ProjectAi;
use pixhaus_core::project::{Entity, EntityId, PixelBufferId, Size, Sprite, SpriteId};
use pixhaus_core::transforms::{self, CanvasAnchor};
use pixhaus_core::undo::{Command, CommandError, CommandResult};

use crate::document::DocumentStore;
use crate::editor::EditorState;

/// A whole-canvas operation on the active sprite: resize, resample, flip, or
/// 90°-multiple rotation. Every variant rewrites every raster cel buffer and,
/// where dimensions change, the sprite's canvas [`Size`]. Carries only scalars,
/// so it is cheap to copy into a `spawn_blocking` closure for large canvases.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
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
}

impl CanvasOp {
    /// The canvas size that results from applying this op to `current`.
    #[must_use]
    pub fn result_size(self, current: Size) -> Size {
        match self {
            Self::Resize { width, height, .. } | Self::Resample { width, height } => Size::new(width, height),
            Self::FlipHorizontal | Self::FlipVertical | Self::Rotate180 => current,
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
            !doc.active_sprite().expect("sprite").cels.iter().any(|c| matches!(c.data, CelData::Raster { buffer, .. } if buffer == new_id)),
            "undo also drops the cel"
        );

        editor.history.redo(&mut doc).expect("redo");
        assert!(doc.pixel_buffers.contains_key(&new_id), "redo restores the buffer");
        assert_eq!(doc.pixel_buffers.len(), before_count + 1);
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
}
