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
pub mod approval;
pub mod blend;
pub mod brush;
pub mod canvas;
pub mod cel;
pub mod color;
pub mod frame;
pub mod geometry;
pub mod id;
pub mod layer;
pub mod library;
pub mod palette;
pub mod schema;
pub mod selection;
pub mod slice;
pub mod sprite;
pub mod tilemap;
pub mod tileset;
pub mod user_data;

pub use animation::Animation;
pub use approval::{Approval, ApprovalError, approve_sheet_variant, set_entity_anchor};
pub use blend::BlendMode;
pub use brush::{BrushShape, BrushState};
pub use canvas::CanvasState;
pub use cel::{Cel, CelData};
pub use color::{ColorMode, Rgba};
pub use frame::{Frame, FrameRange, FrameTag, LoopDirection};
pub use geometry::{IVec2, Rect, Size};
pub use id::{
    AnimationId, EntityId, FrameIndex, GroupId, LayerId, LoraId, PaletteId, PixelBufferId,
    SheetVariantId, SliceId, SpriteId, StateId, TagId, TileIndex, TilesetId,
};
pub use layer::{Layer, LayerKind};
pub use library::{
    ActiveTarget, AiMetadata, AssetInfo, Entity, EntityContent, EntityDefaults, EntityGroup,
    EntityKind, GenerationProvenance, Library, NamedSprite, ProjectAi, PromptEntry,
    PromptHistoryEntry, PromptResult, ReferenceImage, ReferenceSheet, SheetComposition, SheetPanel,
    SheetVariant, TagDefinition, TilemapLayer, TilemapScene, TilesetReference,
};
pub use palette::{Palette, PaletteEntry, PaletteFrameOverride};
pub use schema::{FeatureFlags, SchemaError, SchemaVersion};
pub use selection::{SelectionRegion, SelectionState};
pub use slice::{NineSlice, Pivot, Slice, SliceKey};
pub use sprite::Sprite;
pub use tilemap::{TileCell, TileFlags, TilemapData};
pub use tileset::{
    AnimLoopMode, CollisionShape, TileAnimation, TileAnimationFrame, TileProperties, Tileset,
    TilesetSource,
};
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
/// One `Project` corresponds to one document on disk. The
/// [`Self::library`] holds every named asset: `Custom`-kind entities
/// with their sprite states, tilesets, tilemap scenes, and references.
/// [`Self::active`] tracks what the editor is foregrounding inside the
/// library.
///
/// Accessor methods ([`Self::sprite`], [`Self::sprite_mut`],
/// [`Self::sprites_iter`], [`Self::sprites_iter_mut`],
/// [`Self::active_sprite_id`]) walk the library so call sites don't
/// have to know about the entity / state nesting.
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
    /// Project library: entities, groups, palettes, tags, AI metadata.
    /// The canonical home for every named asset.
    #[serde(default, skip_serializing_if = "Library::is_empty")]
    pub library: Library,
    /// Editor canvas viewport state, persisted across save/load.
    pub canvas: CanvasState,
    /// Editor brush state, persisted across save/load.
    pub brush: BrushState,
    /// Editor selection state, persisted across save/load.
    pub selection: SelectionState,
    /// What the user is currently editing inside the library.
    #[serde(default, skip_serializing_if = "ActiveTarget::is_none")]
    pub active: ActiveTarget,
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
            library: Library::default(),
            canvas: CanvasState::default(),
            brush: BrushState::default(),
            selection: SelectionState::default(),
            active: ActiveTarget::None,
        }
    }

    /// Looks up a sprite by its [`SpriteId`] across every `Custom`-kind
    /// entity in the library. Returns `None` if no state's sprite
    /// matches.
    #[must_use]
    pub fn sprite(&self, id: SpriteId) -> Option<&Sprite> {
        self.sprites_iter()
            .find_map(|(named, _)| (named.sprite.id == id).then_some(&named.sprite))
    }

    /// Mutable variant of [`Self::sprite`].
    #[must_use]
    pub fn sprite_mut(&mut self, id: SpriteId) -> Option<&mut Sprite> {
        for entity in &mut self.library.entities {
            if let EntityContent::Sprites { states } = &mut entity.content {
                for state in states {
                    if state.sprite.id == id {
                        return Some(&mut state.sprite);
                    }
                }
            }
        }
        None
    }

    /// Iterates over every sprite in the library, paired with the
    /// [`EntityId`] of its containing entity. Order is library-entity
    /// order, then state order within each entity.
    pub fn sprites_iter(&self) -> impl Iterator<Item = (&NamedSprite, EntityId)> {
        self.library.entities.iter().flat_map(|entity| {
            let entity_id = entity.id;
            let states = match &entity.content {
                EntityContent::Sprites { states } => states.as_slice(),
                _ => &[],
            };
            states.iter().map(move |state| (state, entity_id))
        })
    }

    /// Mutable iterator that yields each [`NamedSprite`] in the library
    /// paired with its containing [`EntityId`]. Order matches
    /// [`Self::sprites_iter`].
    pub fn sprites_iter_mut(&mut self) -> impl Iterator<Item = (&mut NamedSprite, EntityId)> {
        self.library.entities.iter_mut().flat_map(|entity| {
            let entity_id = entity.id;
            let states: &mut [NamedSprite] = match &mut entity.content {
                EntityContent::Sprites { states } => states.as_mut_slice(),
                _ => &mut [],
            };
            states.iter_mut().map(move |state| (state, entity_id))
        })
    }

    /// Returns the currently active sprite's [`SpriteId`] when
    /// [`Self::active`] points at a state of a `Custom`-kind entity.
    /// `None` for any other active target (tileset, tilemap, reference,
    /// or no active target).
    #[must_use]
    pub fn active_sprite_id(&self) -> Option<SpriteId> {
        let ActiveTarget::State {
            entity_id,
            state_id,
        } = self.active
        else {
            return None;
        };
        let entity = self.library.entities.iter().find(|e| e.id == entity_id)?;
        let EntityContent::Sprites { states } = &entity.content else {
            return None;
        };
        states
            .iter()
            .find(|s| s.id == state_id)
            .map(|s| s.sprite.id)
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

    use library::EntityContent;

    fn project_with_one_state() -> Project {
        let mut project = Project::new("with-state");
        let sprite = Sprite::empty(SpriteId::new(7), "main", Size::new(8, 8));
        project.library.entities.push(library::Entity {
            id: EntityId::new(1),
            kind: library::EntityKind::Custom("Character".into()),
            name: "Hero".into(),
            group_id: None,
            tags: Vec::new(),
            defaults: library::EntityDefaults::default(),
            content: EntityContent::Sprites {
                states: vec![library::NamedSprite {
                    id: StateId::new(3),
                    state_name: "idle".into(),
                    sprite,
                    engine_tags: Vec::new(),
                }],
            },
            ai: library::AiMetadata::default(),
            anchor_reference_id: None,
            user_data: UserData::default(),
            created_at: 0,
            updated_at: 0,
        });
        project.active = ActiveTarget::State {
            entity_id: EntityId::new(1),
            state_id: StateId::new(3),
        };
        project
    }

    #[test]
    fn new_project_has_current_schema() {
        let p = Project::new("test");
        assert_eq!(p.schema_version, SchemaVersion::current());
        assert_eq!(p.feature_flags, FeatureFlags::empty());
        assert!(p.library.is_empty());
        assert_eq!(p.sprites_iter().count(), 0);
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

    #[test]
    fn sprite_accessor_walks_library() {
        let p = project_with_one_state();
        let s = p.sprite(SpriteId::new(7)).unwrap();
        assert_eq!(s.id, SpriteId::new(7));
        assert!(p.sprite(SpriteId::new(99)).is_none());
    }

    #[test]
    fn sprites_iter_pairs_sprite_with_entity() {
        let p = project_with_one_state();
        let collected: Vec<_> = p.sprites_iter().collect();
        assert_eq!(collected.len(), 1);
        assert_eq!(collected[0].1, EntityId::new(1));
        assert_eq!(collected[0].0.sprite.id, SpriteId::new(7));
    }

    #[test]
    fn active_sprite_id_resolves_state_target() {
        let p = project_with_one_state();
        assert_eq!(p.active_sprite_id(), Some(SpriteId::new(7)));
    }

    #[test]
    fn active_sprite_id_is_none_for_other_targets() {
        let mut p = project_with_one_state();
        p.active = ActiveTarget::Tileset {
            entity_id: EntityId::new(1),
        };
        assert!(p.active_sprite_id().is_none());
        p.active = ActiveTarget::None;
        assert!(p.active_sprite_id().is_none());
    }

    #[test]
    fn sprite_mut_walks_library() {
        let mut p = project_with_one_state();
        let s = p.sprite_mut(SpriteId::new(7)).unwrap();
        s.name = "renamed".into();
        assert_eq!(p.sprite(SpriteId::new(7)).unwrap().name, "renamed");
    }
}
