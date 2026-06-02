//! [`ApplyGeneratedAsset`]: insert generated pixels as a new sprite.
//!
//! This is the keystone command for the Generate loop. It takes plain decoded RGBA8
//! plus a size and name — never a `services` result type — so `core` stays unaware
//! of the generation layer. The generation module builds it from a `GeneratedAsset`.

use crate::command::{Command, CommandError};
use crate::document::{Document, Sprite};
use crate::ids::{PixelBufferId, SpriteId};
use crate::pixel::PixelBuffer;

/// Records what [`ApplyGeneratedAsset::apply`] inserted, so undo removes exactly that.
struct Inserted {
    sprite: SpriteId,
    buffer: PixelBufferId,
    prev_active: Option<SpriteId>,
}

/// Inserts a generated image as a new single-layer sprite and makes it active.
pub struct ApplyGeneratedAsset {
    name: String,
    width: u32,
    height: u32,
    stride: u32,
    /// Source pixels: `Some` while pending (before apply, or after undo); `None`
    /// while applied, when the bytes live in the document's buffer store.
    rgba: Option<Vec<u8>>,
    inserted: Option<Inserted>,
}

impl ApplyGeneratedAsset {
    /// A command that will insert `rgba` (validated against `width`/`height`/
    /// `stride` at apply time) as a new sprite named `name`.
    pub fn new(name: String, width: u32, height: u32, stride: u32, rgba: Vec<u8>) -> Self {
        Self {
            name,
            width,
            height,
            stride,
            rgba: Some(rgba),
            inserted: None,
        }
    }
}

impl Command for ApplyGeneratedAsset {
    fn apply(&mut self, doc: &mut Document) -> Result<(), CommandError> {
        let rgba = self.rgba.take().ok_or(CommandError::InvalidState)?;
        let pixels = PixelBuffer::from_rgba8(self.width, self.height, self.stride, rgba)?;
        let buffer = doc.buffers.insert(pixels);
        let sprite_id = doc.mint_sprite_id();
        let mut sprite = Sprite::new_single_frame(sprite_id, self.name.clone(), self.width, self.height);
        sprite
            .active_frame_mut()
            .ok_or(CommandError::InvalidState)?
            .add_layer(self.name.clone(), buffer);
        let prev_active = doc.active_sprite;
        doc.sprites.push(sprite);
        doc.active_sprite = Some(sprite_id);
        doc.bump_revision();
        self.inserted = Some(Inserted {
            sprite: sprite_id,
            buffer,
            prev_active,
        });
        Ok(())
    }

    fn undo(&mut self, doc: &mut Document) -> Result<(), CommandError> {
        let inserted = self.inserted.take().ok_or(CommandError::InvalidState)?;
        let pos = doc
            .sprites
            .iter()
            .position(|s| s.id == inserted.sprite)
            .ok_or(CommandError::SpriteNotFound(inserted.sprite))?;
        doc.sprites.remove(pos);
        let buffer = doc.buffers.remove(inserted.buffer).ok_or(CommandError::InvalidState)?;
        doc.active_sprite = inserted.prev_active;
        doc.bump_revision();
        // Reclaim the pixels so a redo re-inserts the identical asset.
        self.rgba = Some(buffer.into_bytes());
        Ok(())
    }

    fn label_key(&self) -> &'static str {
        "command.apply_generated_asset"
    }

    fn estimated_size_bytes(&self) -> usize {
        // The pixels count toward history memory only while the command holds them
        // (pending / undone); while applied they live in the document, not here.
        self.rgba.as_ref().map_or(0, Vec::len) + std::mem::size_of::<Self>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::composite::composite_sprite;

    fn red_2x2() -> Vec<u8> {
        // four opaque red pixels
        vec![255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255]
    }

    #[test]
    fn apply_inserts_a_sprite_matching_the_input() {
        let mut doc = Document::new();
        let mut cmd = ApplyGeneratedAsset::new("gen".to_owned(), 2, 2, 8, red_2x2());
        cmd.apply(&mut doc).unwrap();

        let id = doc.active_sprite().unwrap();
        let out = composite_sprite(&doc, id).unwrap();
        assert_eq!(out.width(), 2);
        assert_eq!(out.as_bytes(), red_2x2().as_slice());
    }

    #[test]
    fn undo_removes_sprite_and_buffer_then_redo_restores() {
        let mut doc = Document::new();
        let mut cmd = ApplyGeneratedAsset::new("gen".to_owned(), 2, 2, 8, red_2x2());

        cmd.apply(&mut doc).unwrap();
        assert_eq!(doc.sprites().len(), 1);
        assert_eq!(doc.buffers().len(), 1);

        cmd.undo(&mut doc).unwrap();
        assert_eq!(doc.sprites().len(), 0);
        assert_eq!(doc.buffers().len(), 0);

        cmd.apply(&mut doc).unwrap();
        let id = doc.active_sprite().unwrap();
        assert_eq!(composite_sprite(&doc, id).unwrap().as_bytes(), red_2x2());
    }

    #[test]
    fn invalid_stride_surfaces_as_command_error() {
        let mut doc = Document::new();
        // stride 4 < width*4 = 8
        let mut cmd = ApplyGeneratedAsset::new("bad".to_owned(), 2, 2, 4, vec![0u8; 8]);
        assert!(matches!(cmd.apply(&mut doc), Err(CommandError::Pixel(_))));
    }
}
