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
use pixhaus_core::project::{Entity, EntityId, PixelBufferId, Sprite, SpriteId};
use pixhaus_core::undo::{Command, CommandError, CommandResult};

use crate::document::DocumentStore;
use crate::editor::EditorState;

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
    let EntityContent::Sprites { reference_sheet: Some(sheet), .. } = &entity.content else {
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
}
