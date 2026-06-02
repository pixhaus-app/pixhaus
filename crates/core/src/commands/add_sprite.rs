//! [`AddSprite`]: add a blank, single-layer sprite to the document.

use crate::command::{Command, CommandError};
use crate::document::{Document, Sprite};
use crate::ids::{PixelBufferId, SpriteId};
use crate::pixel::PixelBuffer;

/// The parameters for a new blank sprite.
#[derive(Clone, Debug)]
pub struct SpriteProto {
    /// User-facing sprite name (project content).
    pub name: String,
    /// Canvas width in pixels.
    pub width: u32,
    /// Canvas height in pixels.
    pub height: u32,
}

/// Records what [`AddSprite::apply`] inserted, so undo removes exactly that.
struct Inserted {
    sprite: SpriteId,
    buffer: PixelBufferId,
    prev_active: Option<SpriteId>,
}

/// Adds a blank, fully-transparent, single-layer sprite and makes it active.
pub struct AddSprite {
    proto: Option<SpriteProto>,
    inserted: Option<Inserted>,
}

impl AddSprite {
    /// A command that will add a blank sprite described by `proto`.
    pub fn new(proto: SpriteProto) -> Self {
        Self {
            proto: Some(proto),
            inserted: None,
        }
    }
}

impl Command for AddSprite {
    fn apply(&mut self, doc: &mut Document) -> Result<(), CommandError> {
        let proto = self.proto.take().ok_or(CommandError::InvalidState)?;
        let buffer = doc.buffers.insert(PixelBuffer::new(proto.width, proto.height)?);
        let sprite_id = doc.mint_sprite_id();
        let mut sprite = Sprite::new_single_frame(sprite_id, proto.name, proto.width, proto.height);
        sprite
            .active_frame_mut()
            .ok_or(CommandError::InvalidState)?
            .add_layer("Layer 1".to_owned(), buffer);
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
        let sprite = doc.sprites.remove(pos);
        doc.buffers.remove(inserted.buffer);
        doc.active_sprite = inserted.prev_active;
        doc.bump_revision();
        // Restore the proto so a redo re-adds the same sprite.
        self.proto = Some(SpriteProto {
            name: sprite.name,
            width: sprite.width,
            height: sprite.height,
        });
        Ok(())
    }

    fn label_key(&self) -> &'static str {
        "command.add_sprite"
    }

    fn estimated_size_bytes(&self) -> usize {
        // A blank sprite stores no pixel snapshot; the buffer is regenerated on redo.
        std::mem::size_of::<Self>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_adds_one_active_sprite_then_undo_removes_it() {
        let mut doc = Document::new();
        let mut cmd = AddSprite::new(SpriteProto {
            name: "hero".to_owned(),
            width: 8,
            height: 8,
        });

        cmd.apply(&mut doc).unwrap();
        assert_eq!(doc.sprites().len(), 1);
        assert!(doc.active_sprite().is_some());
        assert_eq!(doc.buffers().len(), 1);

        cmd.undo(&mut doc).unwrap();
        assert_eq!(doc.sprites().len(), 0);
        assert_eq!(doc.active_sprite(), None);
        assert_eq!(doc.buffers().len(), 0);
    }

    #[test]
    fn redo_after_undo_re_adds() {
        let mut doc = Document::new();
        let mut cmd = AddSprite::new(SpriteProto {
            name: "hero".to_owned(),
            width: 4,
            height: 4,
        });
        cmd.apply(&mut doc).unwrap();
        cmd.undo(&mut doc).unwrap();
        cmd.apply(&mut doc).unwrap();
        assert_eq!(doc.sprites().len(), 1);
    }

    #[test]
    fn undo_before_apply_is_invalid_state() {
        let mut doc = Document::new();
        let mut cmd = AddSprite::new(SpriteProto {
            name: "x".to_owned(),
            width: 1,
            height: 1,
        });
        assert!(matches!(cmd.undo(&mut doc), Err(CommandError::InvalidState)));
    }
}
