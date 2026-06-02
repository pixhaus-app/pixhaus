//! The document: the authoritative project state and the single [`Command`] target.
//!
//! A [`Document`] bundles structural data (sprites and their layers) with the
//! [`PixelBufferStore`] that owns their pixels, so one [`Command`](crate::Command)
//! type covers both structural and pixel edits. Every mutation flows through a
//! command and bumps [`Document::revision`], which the renderer watches to decide
//! when to recomposite.

use crate::buffer_store::PixelBufferStore;
use crate::ids::{IdCounter, LayerId, PixelBufferId, SpriteId};
use crate::pixel::BlendMode;

/// One raster layer: metadata plus a handle to its pixels in the buffer store.
#[derive(Clone, Debug)]
pub struct Layer {
    /// Stable id within the owning sprite.
    pub id: LayerId,
    /// User-facing layer name (project content, not a localization key).
    pub name: String,
    /// Whether the layer contributes to the composite.
    pub visible: bool,
    /// Layer opacity, 0.0 transparent .. 1.0 opaque.
    pub opacity: f32,
    /// How the layer composites over the layers beneath it.
    pub blend: BlendMode,
    /// Handle into the document's [`PixelBufferStore`].
    pub buffer: PixelBufferId,
}

/// One sprite: a sized stack of layers.
#[derive(Clone, Debug)]
pub struct Sprite {
    /// Stable id within the document.
    pub id: SpriteId,
    /// User-facing sprite name (project content, not a localization key).
    pub name: String,
    /// Canvas width in pixels.
    pub width: u32,
    /// Canvas height in pixels.
    pub height: u32,
    /// Layers, bottom-first (index 0 is composited first).
    pub layers: Vec<Layer>,
    pub(crate) layer_counter: IdCounter,
}

impl Sprite {
    /// Mints a fresh layer id for this sprite.
    pub(crate) fn mint_layer_id(&mut self) -> LayerId {
        LayerId(self.layer_counter.mint())
    }
}

/// The authoritative project state and the single [`Command`](crate::Command)
/// target. Holds the sprites, the pixel-buffer store, the active sprite, and a
/// revision counter bumped on every mutation.
#[derive(Clone, Debug, Default)]
pub struct Document {
    pub(crate) sprites: Vec<Sprite>,
    pub(crate) sprite_counter: IdCounter,
    pub(crate) buffers: PixelBufferStore,
    pub(crate) active_sprite: Option<SpriteId>,
    pub(crate) revision: u64,
}

impl Document {
    /// An empty document.
    pub fn new() -> Self {
        Self::default()
    }

    /// The sprites, in list order.
    pub fn sprites(&self) -> &[Sprite] {
        &self.sprites
    }

    /// Borrows the sprite for `id`, or `None` if absent.
    pub fn sprite(&self, id: SpriteId) -> Option<&Sprite> {
        self.sprites.iter().find(|s| s.id == id)
    }

    /// Read access to the pixel-buffer store (the renderer composites from here).
    pub fn buffers(&self) -> &PixelBufferStore {
        &self.buffers
    }

    /// The active sprite, if any.
    pub fn active_sprite(&self) -> Option<SpriteId> {
        self.active_sprite
    }

    /// The active sprite's `(width, height)`, or `None` if there is no active sprite.
    pub fn active_sprite_size(&self) -> Option<(u32, u32)> {
        self.active_sprite.and_then(|id| self.sprite(id)).map(|s| (s.width, s.height))
    }

    /// A counter bumped on every mutation; the renderer recomposites when it changes.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    // --- command-only mutators (in-crate; mutation flows through Command) ---

    pub(crate) fn mint_sprite_id(&mut self) -> SpriteId {
        SpriteId(self.sprite_counter.mint())
    }

    pub(crate) fn bump_revision(&mut self) {
        self.revision += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_document_has_no_active_sprite() {
        let doc = Document::new();
        assert!(doc.sprites().is_empty());
        assert_eq!(doc.active_sprite(), None);
        assert_eq!(doc.active_sprite_size(), None);
        assert_eq!(doc.revision(), 0);
    }
}
