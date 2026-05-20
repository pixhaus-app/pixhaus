//! Core library entities: the [`Library`] container, [`Entity`] and its
//! kind/content, named sprite states, groups, tilemap scenes, and the
//! active-target selection model.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use ts_rs::{Config, TS, TypeVisitor};

use crate::project::color::ColorMode;
use crate::project::geometry::Size;
use crate::project::id::{EntityId, GroupId, LayerId, PaletteId, StateId, TagId, TileIndex};
use crate::project::palette::Palette;
use crate::project::slice::Pivot;
use crate::project::sprite::Sprite;
use crate::project::tilemap::TilemapData;
use crate::project::tileset::Tileset;
use crate::project::user_data::UserData;

use super::ai::{AiMetadata, ProjectAi};
use super::reference_sheets::ReferenceSheet;
use super::tags::TagDefinition;

/// Top-level container of all entities, palettes, tags, and AI metadata
/// in a Pixhaus project.
///
/// Lives on [`super::super::Project::library`] alongside the legacy
/// `sprites` field; new code targets the library, the `sprites` field is
/// the transitional path until the B9 migration completes across
/// consumers.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Library {
    /// Stable list of entities in this project. Order matters for the
    /// library tree UI (insertion order, manually re-orderable).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entities: Vec<Entity>,

    /// Optional groups for tree-style organization (Characters, Enemies,
    /// ...). Any entity belongs to at most one group via
    /// [`Entity::group_id`]. Groups can nest via
    /// [`EntityGroup::parent_id`].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<EntityGroup>,

    /// Project-wide shared palettes. Entities reference by id; sprites
    /// inside an entity may also have local palettes that override
    /// these.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub palettes: Vec<Palette>,

    /// Defined tags. The user creates tags here; entities reference
    /// them by id. Auto-generated tags (from VLM analysis) are stored
    /// here with `auto_generated = true`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<TagDefinition>,

    /// Project-level AI metadata: style learning corpus, project `LoRA`,
    /// prompt history.
    #[serde(default, skip_serializing_if = "ProjectAi::is_empty")]
    pub ai: ProjectAi,
}

impl Library {
    /// Returns `true` if the library has no entities, groups, palettes,
    /// tags, and no AI metadata.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
            && self.groups.is_empty()
            && self.palettes.is_empty()
            && self.tags.is_empty()
            && self.ai.is_empty()
    }
}

/// A named entity in the project library.
///
/// The unit of organization for everything in a project: a Hero, an
/// Enemy, a Forest tileset, a Forest-1 level. Stable ids let renames
/// happen without breaking cross-entity references (tilemap-to-tileset,
/// group membership) or per-entity AI metadata.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Entity {
    /// Stable id. Cross-entity references use this; renames don't break.
    pub id: EntityId,

    /// Entity kind determines the content shape. See [`EntityKind`].
    pub kind: EntityKind,

    /// Display name. User-editable.
    pub name: String,

    /// Optional group membership.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_id: Option<GroupId>,

    /// Tag ids attached to this entity.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<TagId>,

    /// Defaults inherited by states/content within this entity.
    #[serde(default, skip_serializing_if = "EntityDefaults::is_empty")]
    pub defaults: EntityDefaults,

    /// The actual content. Shape depends on `kind`.
    pub content: EntityContent,

    /// Per-entity AI metadata: which sprites within this entity are
    /// part of the style reference corpus, prompt history.
    #[serde(default, skip_serializing_if = "AiMetadata::is_empty")]
    pub ai: AiMetadata,

    /// Free-form user data (text + tint), reusing existing
    /// [`UserData`].
    #[serde(default, skip_serializing_if = "UserData::is_empty")]
    pub user_data: UserData,

    /// UTC seconds-since-epoch at which the entity was created. `i64` on
    /// the Rust side keeps the full range; TS mirror is `number` since
    /// any realistic timestamp fits in `Number.MAX_SAFE_INTEGER`.
    #[ts(type = "number")]
    pub created_at: i64,

    /// UTC seconds-since-epoch of the last edit to this entity.
    #[ts(type = "number")]
    pub updated_at: i64,
}

/// Kinds of entity. Two system kinds plus one user-defined kind.
///
/// The data model deliberately does not bake game-genre taxonomy.
/// "Character", "Enemy", "Hero", "Boss" — none of those are kinds. They
/// live in the [`EntityKind::Custom`] variant where the string is the
/// user's category name.
///
/// The `tag = "kind", content = "value"` form keeps the unit variants
/// small (`{ "kind": "Tileset" }`) while letting `Custom` carry its
/// string in a uniform shape (`{ "kind": "Custom", "value": "Hero" }`).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(tag = "kind", content = "value")]
pub enum EntityKind {
    /// Tile primitives + autotile rules. Project-level so multiple
    /// `Tilemap` entities can share one.
    Tileset,
    /// A level scene that places tiles drawn from one or more `Tileset`
    /// entities. Multi-tileset is a first-class case (Tiled `firstgid`
    /// model).
    Tilemap,
    /// User-defined entity. The string is the user's free-form category
    /// — typically "Character", "Enemy", "NPC", "Prop", "Vehicle", "UI",
    /// "Effect", or anything else they type. Autocomplete suggestions
    /// surface common categories; the data model imposes no schema on
    /// the value.
    Custom(String),
}

/// Type-specific content. Variants line up with [`EntityKind`].
///
/// The `tag = "type", content = "value"` form mirrors [`EntityKind`]'s
/// shape and disambiguates the wrapper from the field name on the TS
/// side.
///
/// `clippy::large_enum_variant` fires here because [`Sprite`] (the
/// `Sprites` variant inlines a `Vec<NamedSprite>` of full sprites) is
/// substantially larger than the unit variants. The optional
/// [`ReferenceSheet`] in `Sprites` is boxed because it is comparatively
/// large and absent on many sprite entities. `Sprites` itself stays
/// inline because boxing it would force an allocation on every library
/// walk in the hot read path.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
#[allow(clippy::large_enum_variant)]
pub enum EntityContent {
    /// `Custom`-kind entity: named states, each backed by a [`Sprite`].
    /// The first state is the "primary" — what shows in thumbnails by
    /// default. A typical Hero entity has states `[idle, walk, run,
    /// attack]`.
    Sprites {
        /// Ordered states. The first entry is the primary/default state
        /// for thumbnails and the editor open-on-create flow.
        states: Vec<NamedSprite>,
        /// Optional reference sheet shared by every state in this
        /// logical sprite entity. When present, AI generation uses the
        /// canonical sheet as the entity's visual-consistency anchor.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reference_sheet: Option<Box<ReferenceSheet>>,
    },

    /// `Tileset`-kind entity: holds a single tileset (the tile primitives
    /// and autotile rules). Hoisted from inside [`Sprite`] into a
    /// project-level entity so multiple tilemaps can share it.
    Tileset {
        /// The tileset payload.
        tileset: Tileset,
    },

    /// `Tilemap`-kind entity: a level scene that references one or more
    /// `Tileset` entities by id and places tiles on layered grids.
    Tilemap {
        /// The scene payload.
        scene: TilemapScene,
    },
}

impl TS for EntityContent {
    type WithoutGenerics = Self;
    type OptionInnerType = Self;

    const IS_ENUM: bool = true;

    fn name(_: &Config) -> String {
        "EntityContent".into()
    }

    fn inline(_: &Config) -> String {
        r#"{ "type": "Sprites", "value": {
states: Array<NamedSprite>,
reference_sheet?: ReferenceSheet | null,
} } | { "type": "Tileset", "value": {
tileset: Tileset,
} } | { "type": "Tilemap", "value": {
scene: TilemapScene,
} }"#
            .into()
    }

    fn decl(cfg: &Config) -> String {
        format!("type {} = {};", Self::name(cfg), Self::inline(cfg))
    }

    fn decl_concrete(cfg: &Config) -> String {
        Self::decl(cfg)
    }

    fn visit_dependencies(v: &mut impl TypeVisitor)
    where
        Self: 'static,
    {
        v.visit::<NamedSprite>();
        v.visit::<ReferenceSheet>();
        v.visit::<Tileset>();
        v.visit::<TilemapScene>();
    }

    fn output_path() -> Option<PathBuf> {
        Some(PathBuf::from("EntityContent.ts"))
    }
}

/// A named state of a `Custom`-kind entity, e.g. `idle`, `walk`,
/// `attack-1`. Wraps an existing [`Sprite`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct NamedSprite {
    /// State id. Stable across renames so engine handoff and animation
    /// references don't break.
    pub id: StateId,

    /// State name. Conventional set: `idle`, `walk`, `run`, `jump`,
    /// `attack`, `hurt`, `death` — but any string is valid.
    pub state_name: String,

    /// The actual sprite content. The sprite's `name` field becomes the
    /// display name in the editor's title bar; convention:
    /// `EntityName / state_name`.
    pub sprite: Sprite,

    /// Optional engine-side tags (running-state, can-be-cancelled, ...)
    /// for handoff to game engines. No editor semantics.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub engine_tags: Vec<String>,
}

/// Defaults inherited by states/content under the entity.
///
/// Every field is optional so an entity that hasn't expressed any
/// defaults serialises as `{}` and round-trips identically.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct EntityDefaults {
    /// Default canvas size for new states.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canvas_size: Option<Size>,

    /// Default color mode for new states.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color_mode: Option<ColorMode>,

    /// Default palette id (refers to a palette in [`Library::palettes`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_palette_id: Option<PaletteId>,

    /// Default pivot for sprite handoff.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_pivot: Option<Pivot>,

    /// Default playback FPS for new animation states.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_fps: Option<u16>,
}

impl EntityDefaults {
    /// Returns `true` when no default field is set.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.canvas_size.is_none()
            && self.color_mode.is_none()
            && self.default_palette_id.is_none()
            && self.default_pivot.is_none()
            && self.default_fps.is_none()
    }
}

/// A folder-style group within the library tree. Optional, never
/// auto-created.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct EntityGroup {
    /// Stable identifier.
    pub id: GroupId,

    /// Display name in the library tree.
    pub name: String,

    /// Optional parent for nested groups. `None` means top-level.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<GroupId>,

    /// Free-form user data (text + tint).
    #[serde(default, skip_serializing_if = "UserData::is_empty")]
    pub user_data: UserData,
}

/// A level scene that references one or more `Tileset` entities and
/// places tiles on layered grids.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct TilemapScene {
    /// Grid dimensions in tile cells.
    pub size: Size,

    /// References to `Tileset` entities used by this scene. Each
    /// reference gets a `first_gid` for Tiled-compat export (TMX uses
    /// global tile ids that disambiguate across multiple tilesets).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tilesets: Vec<TilesetReference>,

    /// Tile cell layers.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub layers: Vec<TilemapLayer>,

    /// Free-form key/value properties for engine handoff (e.g. `music =
    /// forest-theme.ogg`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub properties: BTreeMap<String, String>,
}

/// One reference from a [`TilemapScene`] to a `Tileset` entity.
#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct TilesetReference {
    /// Stable id of the `Tileset` entity in the project library.
    pub tileset_entity_id: EntityId,
    /// First global tile id for this tileset (TMX-compat). The exporter
    /// uses this to translate per-cell `TileIndex` values into Tiled
    /// global ids.
    pub first_gid: TileIndex,
}

/// One layer within a [`TilemapScene`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct TilemapLayer {
    /// Stable id within the scene.
    pub id: LayerId,
    /// Display name (e.g. `ground`, `decorations`, `collision`).
    pub name: String,
    /// Cell data.
    pub data: TilemapData,
    /// Per-layer alpha multiplier (`0..=255`).
    pub opacity: u8,
    /// Layer visibility toggle.
    pub visible: bool,
}

/// What the user is currently editing inside a project.
///
/// Replaces the old `CanvasState::active_sprite` model where the focus
/// was always a sprite. With the library, the focus can be a state
/// inside a `Custom`-kind entity, a Tileset, or a Tilemap.
/// `None` is valid — empty libraries and the brief window between
/// "create project" and "create first entity" both produce it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum ActiveTarget {
    /// No selection. Empty library or first-launch.
    #[default]
    None,
    /// Editing a state of a `Custom`-kind entity.
    State {
        /// Entity that owns the state.
        entity_id: EntityId,
        /// State within the entity's `EntityContent::Sprites`.
        state_id: StateId,
    },
    /// Editing a `Tileset` entity.
    Tileset {
        /// The targeted entity.
        entity_id: EntityId,
    },
    /// Editing a `Tilemap` entity.
    Tilemap {
        /// The targeted entity.
        entity_id: EntityId,
    },
}

impl ActiveTarget {
    /// Returns `true` if the active target is [`ActiveTarget::None`].
    #[must_use]
    pub const fn is_none(&self) -> bool {
        matches!(self, ActiveTarget::None)
    }
}
