//! The Pixhaus core data model.
//!
//! Everything a Pixhaus project contains lives under this module: the
//! root [`Project`] type, its [`Sprite`]s with layers, frames, cels,
//! palettes, tilesets, slices, and animations, plus the editor's
//! per-session brush, selection, and canvas state.
//!
//! # Design notes
//!
//! - **No I/O.** This module declares types only. The on-disk
//!   `.pixhaus` format (B3) and the IPC catalog (B4) consume these
//!   types but live in their own crates.
//! - **No pixel bytes.** Pixel data is referenced by [`PixelBufferId`]
//!   handles. The pixel-buffer subsystem owns the actual `Vec<u8>`s
//!   and lands with stream S01.
//! - **Schema-versioned.** Every project carries a [`SchemaVersion`].
//!   Additive changes bump `MINOR`; breaking changes bump `MAJOR` and
//!   require a documented migration.
//! - **TS mirrors.** Every public type derives [`ts_rs::TS`] and
//!   exports to `ui/src/lib/types/` during `cargo test`. The
//!   `serde-compat` feature means serde attributes (`rename_all`,
//!   `tag`, `skip_serializing_if`) drive the TypeScript output too.

pub mod animation;
pub mod blend;
pub mod brush;
pub mod canvas;
pub mod cel;
pub mod color;
pub mod frame;
pub mod geometry;
pub mod id;
pub mod layer;
pub mod palette;
pub mod schema;
pub mod selection;
pub mod slice;
pub mod sprite;
pub mod tilemap;
pub mod tileset;
pub mod user_data;

pub use animation::Animation;
pub use blend::BlendMode;
pub use brush::{BrushShape, BrushState};
pub use canvas::CanvasState;
pub use cel::{Cel, CelData};
pub use color::{ColorMode, Rgba};
pub use frame::{Frame, FrameRange, FrameTag, LoopDirection};
pub use geometry::{IVec2, Rect, Size};
pub use id::{
    AnimationId, FrameIndex, LayerId, PaletteId, PixelBufferId, SliceId, SpriteId, TileIndex,
    TilesetId,
};
pub use layer::{Layer, LayerKind};
pub use palette::{Palette, PaletteEntry};
pub use schema::{FeatureFlags, SchemaVersion};
pub use selection::{SelectionRegion, SelectionState};
pub use slice::{NineSlice, Pivot, Slice, SliceKey};
pub use sprite::Sprite;
pub use tilemap::{TileCell, TileFlags, TilemapData};
pub use tileset::{Tileset, TilesetSource};
pub use user_data::UserData;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Project-level metadata: who made it, when, in what version of
/// Pixhaus.
///
/// All strings are user-facing; keep them short. Timestamps are
/// stored as UTC seconds-since-epoch so they round-trip without
/// timezone ambiguity.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ProjectMetadata {
    /// Display name shown in the title bar and project tree.
    pub name: String,
    /// Optional human-readable description.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub description: Option<String>,
    /// Optional author string.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub author: Option<String>,
    /// UTC seconds-since-epoch at which the project was first created.
    /// `i64` on the Rust side keeps the full range; the TS mirror is
    /// pinned to `number` because `serde_json` writes plain JSON numbers
    /// and seconds-since-epoch fits in `Number.MAX_SAFE_INTEGER` for
    /// any realistic timestamp.
    #[ts(type = "number")]
    pub created_at: i64,
    /// UTC seconds-since-epoch of the most recent save.
    #[ts(type = "number")]
    pub updated_at: i64,
    /// Pixhaus build that wrote this file (e.g. `"0.1.0"`). Cosmetic
    /// — version-gating happens via [`SchemaVersion`], not this field.
    pub editor_version: String,
}

/// The root of a Pixhaus project.
///
/// One `Project` corresponds to one document on disk. A project may
/// contain multiple sprites; the editor focuses on one sprite at a
/// time via [`CanvasState::active_sprite`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Project {
    /// Schema version. Always serialised first so a header parser can
    /// decide whether to continue without decoding the rest.
    pub schema_version: SchemaVersion,
    /// Optional features advertised by the writer.
    pub feature_flags: FeatureFlags,
    /// Project metadata.
    pub metadata: ProjectMetadata,
    /// Sprites contained in the project. May be empty during the
    /// brief window between "create project" and "add first sprite".
    pub sprites: Vec<Sprite>,
    /// Editor canvas viewport state, persisted across save/load.
    pub canvas: CanvasState,
    /// Editor brush state, persisted across save/load.
    pub brush: BrushState,
    /// Editor selection state, persisted across save/load.
    pub selection: SelectionState,
}

impl Project {
    /// Constructs an empty project named `name`. The created and
    /// updated timestamps are left at zero — the editor sets them on
    /// first save.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            schema_version: SchemaVersion::current(),
            feature_flags: FeatureFlags::empty(),
            metadata: ProjectMetadata {
                name: name.into(),
                description: None,
                author: None,
                created_at: 0,
                updated_at: 0,
                editor_version: env!("CARGO_PKG_VERSION").into(),
            },
            sprites: Vec::new(),
            canvas: CanvasState::default(),
            brush: BrushState::default(),
            selection: SelectionState::default(),
        }
    }
}

impl Default for Project {
    fn default() -> Self {
        Self::new("untitled")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_project_has_current_schema() {
        let p = Project::new("test");
        assert_eq!(p.schema_version, SchemaVersion::current());
        assert_eq!(p.feature_flags, FeatureFlags::empty());
        assert!(p.sprites.is_empty());
    }

    #[test]
    fn empty_project_round_trip() {
        let p = Project::new("test");
        let bytes = rmp_serde::to_vec_named(&p).unwrap();
        let back: Project = rmp_serde::from_slice(&bytes).unwrap();
        assert_eq!(p, back);
    }

    #[test]
    fn editor_version_matches_crate_version() {
        let p = Project::new("test");
        assert_eq!(p.metadata.editor_version, env!("CARGO_PKG_VERSION"));
    }
}
