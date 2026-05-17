//! Strongly-typed identifiers for entities in the data model.
//!
//! The model addresses layers, frames, palettes, tilesets, slices,
//! animations, and pixel buffers by ID. Each ID is its own newtype so
//! the compiler refuses to swap a [`LayerId`] for a [`FrameIndex`] at
//! a call site. On the wire and in TypeScript they round-trip as plain
//! numbers — the type-safety is purely a Rust-side guard.
//!
//! IDs are minted monotonically by the editor; deletions tombstone an
//! ID rather than reuse it, so undo can resurrect entities by their
//! original handle.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

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
            TS,
        )]
        #[serde(transparent)]
        #[ts(export, type = "number")]
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
    /// `u32` matches the other IDs in this module and the TS-side
    /// `number` mirror; the 4-billion-buffer headroom is more than any
    /// realistic editing session will mint.
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
    /// Identifier of a [`SheetVariant`](super::library::SheetVariant) within
    /// a [`ReferenceSheet`](super::library::ReferenceSheet).
    ///
    /// Reference sheets keep older variants in their `history`; the id makes
    /// it possible to point at one specific variant from a generation log.
    SheetVariantId, u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_round_trip_via_messagepack() {
        let id = LayerId::new(7);
        let bytes = rmp_serde::to_vec_named(&id).unwrap();
        let back: LayerId = rmp_serde::from_slice(&bytes).unwrap();
        assert_eq!(id, back);
    }

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
    fn pixel_buffer_id_is_u32() {
        let id = PixelBufferId::new(u32::MAX);
        let bytes = rmp_serde::to_vec_named(&id).unwrap();
        let back: PixelBufferId = rmp_serde::from_slice(&bytes).unwrap();
        assert_eq!(back.get(), u32::MAX);
    }
}
