//! Strongly-typed identifiers for entities in the data model.
//!
//! The model addresses layers, frames, palettes, tilesets, slices,
//! animations, and pixel buffers by ID. Each ID is its own newtype so
//! the compiler refuses to swap a [`LayerId`] for a [`FrameIndex`] at
//! a call site. On the wire they round-trip as plain numbers — the
//! type-safety is purely a Rust-side guard.
//!
//! IDs are minted monotonically by the editor; deletions tombstone an
//! ID rather than reuse it, so undo can resurrect entities by their
//! original handle.

use serde::{Deserialize, Serialize};

macro_rules! id_newtype {
    ($(#[$meta:meta])* $name:ident, $inner:ty) => {
        $(#[$meta])*
        #[derive(
            Copy,
            Clone,
            Debug,
            Eq,
            PartialEq,
            Ord,
            PartialOrd,
            Hash,
            Default,
            Serialize,
            Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(pub $inner);

        impl $name {
            /// Constructs an ID from a raw value. Prefer the editor's
            /// minting routines to this constructor outside of tests
            /// and (de)serialization.
            #[must_use]
            pub const fn new(value: $inner) -> Self {
                Self(value)
            }

            /// Returns the wrapped value.
            #[must_use]
            pub const fn get(self) -> $inner {
                self.0
            }
        }

        impl From<$inner> for $name {
            fn from(value: $inner) -> Self {
                Self(value)
            }
        }

        impl From<$name> for $inner {
            fn from(value: $name) -> Self {
                value.0
            }
        }
    };
}

id_newtype! {
    /// Identifier of a [`Sprite`](super::sprite::Sprite) within a project.
    SpriteId, u32
}

id_newtype! {
    /// Identifier of a [`Layer`](super::layer::Layer) within a sprite.
    LayerId, u32
}

id_newtype! {
    /// Position of a frame in the timeline. Frames are dense and
    /// 0-indexed, but the type is a newtype so it cannot be confused
    /// with a layer ID at a call site.
    FrameIndex, u32
}

id_newtype! {
    /// Identifier of a [`Palette`](super::palette::Palette).
    PaletteId, u32
}

/// Identifier of a [`PalettePage`](super::palette::PalettePage) within a palette.
///
/// Pages are named subset views over a palette's entries (see
/// [`PalettePage`](super::palette::PalettePage)). Each page carries its own
/// stable id so renames, reorders, and undo/redo can reference a specific
/// page without leaning on its index in the page list.
///
/// Adapted from `OpenToonz` `toonz/sources/include/tpalette.h` under
/// BSD-3-Clause. See `THIRD_PARTY_NOTICES.md`.
#[derive(
    Copy, Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, Default, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct PalettePageId(pub u32);

impl PalettePageId {
    /// Constructs a page id from a raw value.
    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    /// Returns the wrapped value.
    #[must_use]
    pub const fn value(self) -> u32 {
        self.0
    }

    /// Alias of [`Self::value`] matching the `id_newtype!` macro accessor name
    /// used by the other ID newtypes in this module.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

impl std::fmt::Display for PalettePageId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "page-{}", self.0)
    }
}

impl From<u32> for PalettePageId {
    fn from(value: u32) -> Self {
        Self(value)
    }
}

impl From<PalettePageId> for u32 {
    fn from(value: PalettePageId) -> Self {
        value.0
    }
}

id_newtype! {
    /// Identifier of a [`Tileset`](super::tileset::Tileset).
    TilesetId, u32
}

id_newtype! {
    /// Identifier of a [`Slice`](super::slice::Slice).
    SliceId, u32
}

id_newtype! {
    /// Identifier of an [`Animation`](super::animation::Animation).
    AnimationId, u32
}

id_newtype! {
    /// Opaque handle to a pixel buffer held outside the data model.
    ///
    /// The data model does not own pixel bytes; it references them by
    /// this ID. Resolution is the responsibility of whichever subsystem
    /// owns the buffer registry (loader, undo stack, render compositor).
    /// `u32` matches the other IDs in this module; the 4-billion-buffer
    /// headroom is more than any realistic editing session will mint.
    PixelBufferId, u32
}

id_newtype! {
    /// Index of a tile within a tileset.
    ///
    /// Index `0` is conventionally the empty / transparent tile. Code
    /// that walks a tilemap should treat `TileIndex(0)` as "no tile"
    /// rather than reading from the tileset.
    TileIndex, u32
}

id_newtype! {
    /// Identifier of an [`Entity`](super::library::Entity) in the project library.
    ///
    /// Stable across renames so cross-entity references (group membership,
    /// tilemap-to-tileset) and per-entity AI metadata survive when the user
    /// renames an entity.
    EntityId, u32
}

id_newtype! {
    /// Identifier of a state within a Custom-kind entity.
    ///
    /// A `Custom("Character")` entity holds named states such as `idle`,
    /// `walk`, `attack`. The id stays stable across state renames so engine
    /// handoff metadata and animation references don't break.
    StateId, u32
}

id_newtype! {
    /// Identifier of an [`EntityGroup`](super::library::EntityGroup).
    ///
    /// Groups are folder-style containers in the library tree; stable ids let
    /// a renamed group keep its membership intact.
    GroupId, u32
}

id_newtype! {
    /// Identifier of a [`TagDefinition`](super::library::TagDefinition).
    TagId, u32
}

id_newtype! {
    /// Identifier of a per-project trained `LoRA`.
    ///
    /// The data model carries the id; the trained weights live next to the
    /// project in a sibling file referenced by [`super::library::ProjectAi`].
    LoraId, u32
}

id_newtype! {
    /// Identifier of a project-scoped AI asset in
    /// [`AssetLibrary`](super::library::AssetLibrary).
    AssetId, u32
}

id_newtype! {
    /// Identifier of an async `LoRA` training job tracked by
    /// [`TrainingJob`](super::library::TrainingJob).
    TrainingJobId, u32
}

id_newtype! {
    /// Identifier of a [`SheetVariant`](super::library::SheetVariant) within
    /// a [`ReferenceSheet`](super::library::ReferenceSheet).
    ///
    /// Reference sheets keep older variants in their `variants`; the id makes
    /// it possible to point at one specific variant from a generation log.
    SheetVariantId, u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_serialize_transparently_to_json() {
        let id = FrameIndex::new(42);
        assert_eq!(serde_json::to_string(&id).unwrap(), "42");
    }

    #[test]
    fn ids_are_distinct_at_the_type_level() {
        // This test is mostly here as documentation of intent: the line
        // below should not compile if uncommented, because LayerId and
        // FrameIndex are different types.
        // let _: LayerId = FrameIndex::new(1);
        let l = LayerId::new(1);
        let f = FrameIndex::new(1);
        assert_eq!(l.get(), f.get());
    }

    #[test]
    fn palette_page_id_serializes_transparently() {
        let id = PalettePageId::new(7);
        assert_eq!(serde_json::to_string(&id).unwrap(), "7");
    }

    #[test]
    fn palette_page_id_display_uses_page_prefix() {
        let id = PalettePageId::new(3);
        assert_eq!(format!("{id}"), "page-3");
    }
}
