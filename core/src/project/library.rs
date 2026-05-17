//! Project library: entities, groups, tags, and AI metadata.
//!
//! A Pixhaus project is a [`Library`] of named [`Entity`] values. The
//! kind of an entity determines its content shape: a `Tileset` entity
//! holds a single tileset, a `Tilemap` entity holds a level scene that
//! references one or more tilesets, and a `Custom` entity is the user's
//! free-form kind (Hero, Goblin, Treasure-Chest, Vehicle, ...) and holds
//! named states each backed by a [`Sprite`]. Custom entities may also
//! carry a structured reference sheet that AI verbs use as the sprite's
//! consistency anchor.
//!
//! # Design notes
//!
//! - The kind enum is exactly three variants. The data model deliberately
//!   does not bake game-genre taxonomy — "Character", "Enemy", "Hero" all
//!   live in the user-typed string carried by `EntityKind::Custom`.
//! - Groups are optional and never auto-created. A pristine project has
//!   no entities and no groups.
//! - Tilesets are project-level so a single Forest tileset can back
//!   multiple Forest-1, Forest-2, Boss-Arena tilemap scenes (the Tiled
//!   `firstgid` model).
//! - Custom-kind entities carry an optional [`ReferenceSheet`] inside
//!   [`EntityContent::Sprites`]. When present, that sheet is the AI
//!   generation anchor for every state in the entity.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use ts_rs::{Config, TS, TypeVisitor};

use super::color::Rgba;
use super::geometry::{IVec2, Rect, Size};
use super::id::{
    AssetId, EntityId, GroupId, LayerId, PaletteId, SheetVariantId, StateId, TagId, TileIndex,
    TrainingJobId,
};
use super::palette::{Palette, PaletteEntry};
use super::slice::Pivot;
use super::sprite::Sprite;
use super::tilemap::TilemapData;
use super::tileset::Tileset;
use super::user_data::UserData;
use crate::project::color::ColorMode;

/// Top-level container of all entities, palettes, tags, and AI metadata
/// in a Pixhaus project.
///
/// Lives on [`super::Project::library`] alongside the legacy `sprites`
/// field; new code targets the library, the `sprites` field is the
/// transitional path until the B9 migration completes across consumers.
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

/// Definition of a tag the user (or VLM auto-tagging) created.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct TagDefinition {
    /// Stable identifier.
    pub id: TagId,

    /// Tag name. User-facing; must be non-empty in the editor but the
    /// data model itself does not enforce that.
    pub name: String,

    /// Optional accent color, used by the library tree to tint the chip.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<Rgba>,

    /// `true` if created by VLM auto-tagging. Lets the UI display them
    /// differently and lets the user accept/reject them in batch.
    #[serde(default)]
    pub auto_generated: bool,
}

/// Project-level AI memory and defaults.
///
/// Per-entity AI metadata lives on [`Entity::ai`]; this struct collects
/// the project-wide pieces used by the v1 reference-sheet workflow:
/// prompt style notes, reusable assets, provider-routing preferences,
/// project defaults, and asynchronous `LoRA` training jobs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ProjectAi {
    /// Project-level notes prepended to every AI prompt composition.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub style_notes: String,

    /// Browseable project-scoped AI asset library.
    #[serde(default, skip_serializing_if = "AssetLibrary::is_empty")]
    pub asset_library: AssetLibrary,

    /// Per-operation model overrides. If absent, the router uses its
    /// operation defaults and configured-provider fallback.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub per_operation_model_prefs: BTreeMap<OperationKind, ModelId>,

    /// Default chroma-key color for new sheet generations.
    #[serde(
        default = "default_reference_chroma",
        skip_serializing_if = "is_default_reference_chroma"
    )]
    pub default_chroma: Rgba,

    /// Default quality tier for new generation forms.
    #[serde(
        default = "default_project_quality",
        skip_serializing_if = "Quality::is_medium"
    )]
    pub default_quality: Quality,

    /// Default candidate count for new generation forms.
    #[serde(
        default = "default_candidate_count",
        skip_serializing_if = "is_default_candidate_count"
    )]
    pub default_candidate_count: u8,

    /// `LoRA` training jobs, including completed and failed jobs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub training_jobs: Vec<TrainingJob>,

    /// Entity ids included in the project's style reference corpus. The
    /// Project Style Learning verb (S30) trains a `LoRA` from these.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub style_corpus: Vec<EntityId>,

    /// Path to the trained `LoRA` file relative to the project directory.
    /// Optional — projects without a learned style still work.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_lora_path: Option<String>,

    /// Recent prompts for resume-where-you-left-off semantic search.
    /// Capped — the implementation details land with S21's follow-up.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub prompt_history: Vec<PromptHistoryEntry>,
}

impl Default for ProjectAi {
    fn default() -> Self {
        Self {
            style_notes: String::new(),
            asset_library: AssetLibrary::default(),
            per_operation_model_prefs: BTreeMap::new(),
            default_chroma: default_reference_chroma(),
            default_quality: Quality::Medium,
            default_candidate_count: default_candidate_count(),
            training_jobs: Vec::new(),
            style_corpus: Vec::new(),
            project_lora_path: None,
            prompt_history: Vec::new(),
        }
    }
}

impl ProjectAi {
    /// Returns `true` when no project-level AI memory is set.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.style_notes.is_empty()
            && self.asset_library.is_empty()
            && self.per_operation_model_prefs.is_empty()
            && is_default_reference_chroma(&self.default_chroma)
            && self.default_quality.is_medium()
            && is_default_candidate_count(&self.default_candidate_count)
            && self.training_jobs.is_empty()
            && self.style_corpus.is_empty()
            && self.project_lora_path.is_none()
            && self.prompt_history.is_empty()
    }
}

/// Default chroma-key background for AI-generated reference sheets.
#[must_use]
pub const fn default_reference_chroma() -> Rgba {
    Rgba::opaque(255, 0, 255)
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_default_reference_chroma(color: &Rgba) -> bool {
    *color == default_reference_chroma()
}

fn default_project_quality() -> Quality {
    Quality::Medium
}

fn default_candidate_count() -> u8 {
    2
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_default_candidate_count(n: &u8) -> bool {
    *n == default_candidate_count()
}

/// Per-entity AI metadata.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AiMetadata {
    /// Auto-tags suggested by VLM analysis but not yet user-confirmed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub suggested_tags: Vec<TagId>,

    /// VLM-generated description used for semantic search. Re-derived
    /// when the entity changes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vlm_summary: Option<String>,

    /// Embedding for similarity search. Optional and computed lazily —
    /// the entity is fully usable without one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub embedding: Option<Vec<f32>>,

    /// Per-entity `LoRA` reference. Populated by the B10.5
    /// train-entity-lora verb after a successful training run against
    /// this entity's canonical reference sheet. **Currently the Replicate
    /// weights URL written verbatim by the IPC layer; a future host-side
    /// download will replace it with a project-relative path.** When
    /// present, anchor payloads built for this entity carry it through to
    /// backends, overriding any project-wide `LoRA` for the duration of
    /// generations against this entity. `None` means "fall back to the
    /// project-wide `LoRA`, if any."
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lora_path: Option<String>,
}

impl AiMetadata {
    /// Returns `true` when no AI metadata is attached.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.suggested_tags.is_empty()
            && self.vlm_summary.is_none()
            && self.embedding.is_none()
            && self.lora_path.is_none()
    }
}

/// One entry in the per-project prompt history.
///
/// Defined minimally for B9; the verb runtime (S21 / B10) extends the
/// shape as the streaming-prompt workflow lands.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PromptHistoryEntry {
    /// Verb that consumed the prompt.
    pub verb_name: String,
    /// The user-typed prompt text.
    pub prompt: String,
    /// UTC seconds-since-epoch at which the prompt was issued.
    #[ts(type = "number")]
    pub timestamp: i64,
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

/// Opaque holder for an image referenced inside the data model.
///
/// Pixhaus does not interpret the bytes here; it just round-trips them.
/// The `mime` hint helps the UI decide how to render — typically
/// `image/png` for B9, with B10 widening to other formats as the sheet
/// generator lands.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ReferenceImage {
    /// Source bytes. Stored inline in B9; B10 may externalise large
    /// images to a project sub-folder.
    pub bytes: Vec<u8>,
    /// MIME type hint. `image/png` is the common case.
    pub mime: String,
}

/// High-level quality tier exposed by the reference-sheet editor.
#[derive(
    Copy, Clone, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, TS,
)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum Quality {
    /// Let the router/provider choose the quality tier.
    Auto,
    /// Cheapest / fastest tier.
    Low,
    /// Balanced default tier.
    #[default]
    Medium,
    /// Final/promoted output tier.
    High,
}

impl Quality {
    #[allow(clippy::trivially_copy_pass_by_ref)]
    fn is_medium(&self) -> bool {
        matches!(self, Self::Medium)
    }
}

/// Stable Pixhaus model labels. Provider-specific endpoint IDs live in
/// `pixhaus-ai`; the project file stores these product-facing identifiers.
#[derive(
    Copy, Clone, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, TS,
)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ModelId {
    /// Let the router pick by operation type and provider availability.
    #[default]
    Auto,
    /// `OpenAI` image model label for `gpt-image-2`.
    OpenAiGptImage2,
    /// Google AI Studio Nano Banana Pro image model label.
    GoogleNanoBananaPro,
    /// Google AI Studio Flash image model label.
    GoogleGeminiFlashImage,
    /// fal Flux Kontext image/edit model.
    FalFluxKontext,
    /// fal Flux.1 dev with extensions such as `LoRA`/IP-Adapter.
    FalFluxDev,
    /// fal Recraft vectorize endpoint.
    FalRecraftVectorize,
    /// fal Real-ESRGAN upscaler.
    FalRealEsrgan,
}

/// Reference-sheet operation used by model routing and project prefs.
#[allow(missing_docs)]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum OperationKind {
    FreshGeneration,
    MaskedRefinement,
    PromptOnlyRefinement,
    RegionalRefinement,
    ChatTurn,
    Promotion,
    CrossModelGrid,
    VectorExport,
    Upscale,
    LoraTraining,
}

/// Built-in reference-sheet templates exposed by core/UI.
#[derive(
    Copy, Clone, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, TS,
)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ReferenceSheetTemplateId {
    /// Four-view turnaround for character sheets.
    #[default]
    Turnaround4View,
    /// Eight-direction top-down turnaround.
    Turnaround8Direction,
    /// Facial expression sheet.
    ExpressionSheet,
    /// Action pose sheet.
    ActionPoses,
    /// Labeled turnaround for text-heavy outputs.
    TypographicTurnaround,
    /// Legacy character template identifier.
    Character,
    /// Legacy item template identifier.
    Item,
    /// Legacy tileset template identifier.
    Tileset,
    /// User-authored/custom template.
    Custom,
}

/// Width/height pair for template-controlled sheet output sizes.
#[allow(missing_docs)]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SheetDimensions {
    pub width: u32,
    pub height: u32,
}

/// Core/UI-facing template definition. Backend-only prompt fragments stay
/// in `pixhaus-ai`.
#[allow(missing_docs)]
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ReferenceSheetTemplateDefinition {
    pub id: ReferenceSheetTemplateId,
    pub label: String,
    pub allowed_dimensions: Vec<SheetDimensions>,
    pub default_dimensions: SheetDimensions,
    pub default_chroma: Rgba,
    pub benefits_from_text_labels: bool,
}

/// Returns the built-in reference-sheet template definitions.
#[must_use]
pub fn built_in_reference_sheet_templates() -> Vec<ReferenceSheetTemplateDefinition> {
    use ReferenceSheetTemplateId::{
        ActionPoses, ExpressionSheet, Turnaround4View, Turnaround8Direction, TypographicTurnaround,
    };

    vec![
        template_definition(
            Turnaround4View,
            "Turnaround (4-view)",
            &[(2048, 1024), (1024, 512), (2560, 1280), (1536, 1024)],
            0,
            false,
        ),
        template_definition(
            Turnaround8Direction,
            "Turnaround (8-direction)",
            &[(2048, 1024), (1024, 512), (3072, 1024)],
            0,
            false,
        ),
        template_definition(
            ExpressionSheet,
            "Expression sheet",
            &[(1024, 1024), (1536, 1024), (2048, 2048)],
            0,
            false,
        ),
        template_definition(
            ActionPoses,
            "Action poses",
            &[(2048, 1024), (1024, 512), (3072, 1024)],
            0,
            false,
        ),
        template_definition(
            TypographicTurnaround,
            "Typographic turnaround",
            &[(2048, 1024), (1536, 1024), (2560, 1280)],
            0,
            true,
        ),
    ]
}

fn template_definition(
    id: ReferenceSheetTemplateId,
    label: &str,
    dims: &[(u32, u32)],
    default_index: usize,
    benefits_from_text_labels: bool,
) -> ReferenceSheetTemplateDefinition {
    let allowed_dimensions = dims
        .iter()
        .map(|&(width, height)| SheetDimensions { width, height })
        .collect::<Vec<_>>();
    let default_dimensions =
        allowed_dimensions
            .get(default_index)
            .copied()
            .unwrap_or(SheetDimensions {
                width: 2048,
                height: 1024,
            });
    ReferenceSheetTemplateDefinition {
        id,
        label: label.into(),
        allowed_dimensions,
        default_dimensions,
        default_chroma: default_reference_chroma(),
        benefits_from_text_labels,
    }
}

fn default_reference_template() -> ReferenceSheetTemplateId {
    ReferenceSheetTemplateId::Turnaround4View
}

fn default_sheet_width() -> u32 {
    2048
}

fn default_sheet_height() -> u32 {
    1024
}

fn default_lora_weight() -> f32 {
    1.0
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_default_lora_weight(weight: &f32) -> bool {
    (*weight - 1.0).abs() < f32::EPSILON
}

/// Role hint for one reference image slot.
#[allow(missing_docs)]
#[derive(
    Copy, Clone, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, TS,
)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ReferenceRole {
    Subject,
    Style,
    Pose,
    Outfit,
    Context,
    #[default]
    Generic,
}

/// One ordered reference image supplied to a generation/refinement request.
#[allow(missing_docs)]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ReferenceSlot {
    pub image: ReferenceImage,
    #[serde(default)]
    pub role: ReferenceRole,
    #[serde(default = "default_lora_weight")]
    pub weight: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_asset_id: Option<AssetId>,
}

/// Source operation that produced a sheet variant.
#[allow(missing_docs)]
#[derive(
    Copy, Clone, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, TS,
)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum VariantOrigin {
    #[default]
    FreshGeneration,
    Refinement,
    ChatTurn,
    Promotion,
    CrossModelGrid,
    ManualImport,
}

/// Region used by regional reference refinement.
#[allow(missing_docs)]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct RegionDefinition {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub polygon: Vec<IVec2>,
    pub prompt: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub region_references: Vec<ReferenceSlot>,
}

/// Refinement metadata stored on variants produced by refinement.
#[allow(missing_docs)]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
#[ts(export)]
pub enum RefinementKind {
    Masked { mask_png: ReferenceImage },
    PromptOnly,
    Regional { regions: Vec<RegionDefinition> },
}

/// Full chat transcript stored for conversational variant provenance.
#[allow(missing_docs)]
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ChatTranscript {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub turns: Vec<ChatTurn>,
}

/// One conversational editing turn and the variant it produced.
#[allow(missing_docs)]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ChatTurn {
    #[ts(type = "number")]
    pub timestamp: i64,
    pub user_message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mask: Option<ReferenceImage>,
    pub resulting_variant_id: SheetVariantId,
}

/// Project-scoped reusable assets for reference-sheet workflows.
#[allow(missing_docs)]
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AssetLibrary {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub references: Vec<ReferenceAsset>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub character_cards: Vec<CharacterCard>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub style_swatches: Vec<StyleSwatch>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub loras: Vec<LoraAsset>,
}

impl AssetLibrary {
    #[allow(missing_docs)]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.references.is_empty()
            && self.character_cards.is_empty()
            && self.style_swatches.is_empty()
            && self.loras.is_empty()
    }
}

/// Single image saved to the project asset library.
#[allow(missing_docs)]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ReferenceAsset {
    pub id: AssetId,
    pub image: ReferenceImage,
    #[serde(default)]
    pub default_role: ReferenceRole,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_variant_id: Option<SheetVariantId>,
    #[ts(type = "number")]
    pub created_at: i64,
}

/// Bundle of references and style notes representing one character.
#[allow(missing_docs)]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CharacterCard {
    pub id: AssetId,
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub references: Vec<AssetId>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub style_notes: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub associated_lora: Option<AssetId>,
    #[ts(type = "number")]
    pub created_at: i64,
}

/// Bundle of references and style notes representing a visual style.
#[allow(missing_docs)]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct StyleSwatch {
    pub id: AssetId,
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub references: Vec<AssetId>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub style_notes: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub associated_lora: Option<AssetId>,
    #[ts(type = "number")]
    pub created_at: i64,
}

/// Kind of `LoRA` training asset.
#[allow(missing_docs)]
#[derive(
    Copy, Clone, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, TS,
)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum LoraKind {
    Style,
    #[default]
    Character,
    KontextPair,
}

/// Trained `LoRA` registered in the project asset library.
#[allow(missing_docs)]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct LoraAsset {
    pub id: AssetId,
    pub name: String,
    #[serde(default)]
    pub kind: LoraKind,
    pub trigger_word: String,
    pub target_model: ModelId,
    pub fal_lora_url: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub training_data_thumbnails: Vec<ReferenceImage>,
    #[ts(type = "number")]
    pub created_at: i64,
}

/// State of a fal `LoRA` training job.
#[allow(missing_docs)]
#[derive(
    Copy, Clone, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, TS,
)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum TrainingStatus {
    #[default]
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
}

/// Async `LoRA` training job record.
#[allow(missing_docs)]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct TrainingJob {
    pub id: TrainingJobId,
    pub asset_name: String,
    #[serde(default)]
    pub kind: LoraKind,
    pub target_model: ModelId,
    pub trigger_word: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub training_data: Vec<AssetId>,
    pub fal_job_id: String,
    #[serde(default)]
    pub status: TrainingStatus,
    #[ts(type = "number")]
    pub created_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(type = "number | null")]
    pub completed_at: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result_lora_id: Option<AssetId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// A structured asset reference sheet: the canonical AI anchor and all
/// drafts/history for one sprite entity.
///
/// Draft-only sheets are valid: `canonical` is `None` until the user
/// approves a generated or imported variant. AI verbs only use approved
/// canonical sheets as anchors; variants in `variants` are retained as
/// candidates/history and never anchor generation on their own.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReferenceSheet {
    /// The current canonical variant — the user-approved sheet. `None`
    /// means candidates exist, but none has been approved as the AI
    /// consistency anchor.
    #[serde(default)]
    pub canonical: Option<SheetVariant>,

    /// Drafts and previous canonicals, newest first. The canonical
    /// variant is stored only in `canonical`, not duplicated here.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub variants: Vec<SheetVariant>,

    /// Legacy prompt log retained for pre-v1 command compatibility. New
    /// provenance lives directly on [`SheetVariant`].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub prompts: Vec<PromptEntry>,

    /// Legacy structured metadata retained for existing project commands.
    /// New reusable metadata belongs in [`AssetLibrary`].
    #[serde(default, skip_serializing_if = "AssetInfo::is_empty")]
    pub info: AssetInfo,
}

impl TS for ReferenceSheet {
    type WithoutGenerics = Self;
    type OptionInnerType = Self;

    fn name(_: &Config) -> String {
        "ReferenceSheet".into()
    }

    fn inline(_: &Config) -> String {
        r"{
canonical: SheetVariant | null,
variants?: Array<SheetVariant>,
}"
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
        v.visit::<SheetVariant>();
    }

    fn output_path() -> Option<PathBuf> {
        Some(PathBuf::from("ReferenceSheet.ts"))
    }
}

/// One generated/imported/refined version of a reference sheet.
///
/// The raster image is always embedded. Optional vector output is stored
/// as another [`ReferenceImage`] with `mime = "image/svg+xml"`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SheetVariant {
    /// Stable identifier within the parent [`ReferenceSheet`].
    pub id: SheetVariantId,
    /// UTC seconds-since-epoch at which this variant was generated or
    /// imported.
    #[ts(type = "number")]
    pub created_at: i64,
    /// Layout template used to generate this sheet.
    #[serde(default = "default_reference_template")]
    pub template: ReferenceSheetTemplateId,
    /// Output raster width.
    #[serde(default = "default_sheet_width")]
    pub width: u32,
    /// Output raster height.
    #[serde(default = "default_sheet_height")]
    pub height: u32,
    /// Flat chroma-key background color requested for the model.
    #[serde(default = "default_reference_chroma")]
    pub chroma_color: Rgba,
    /// User-typed prompt for this operation.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub user_prompt: String,
    /// Full composed prompt after style notes/template/reference hints.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub composed_prompt: String,
    /// Ordered generation references with role hints.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub references: Vec<ReferenceSlot>,
    /// Whether Google Search grounding was requested.
    #[serde(default)]
    pub real_world_grounding: bool,
    /// Applied `LoRA` asset, Flux-only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub applied_lora: Option<AssetId>,
    /// Applied `LoRA` strength for Flux requests.
    #[serde(
        default = "default_lora_weight",
        skip_serializing_if = "is_default_lora_weight"
    )]
    pub lora_weight: f32,
    /// The composite sheet image.
    pub image: ReferenceImage,
    /// Optional vectorized SVG output.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vector_image: Option<ReferenceImage>,
    /// Model that produced this variant.
    #[serde(default)]
    pub model: ModelId,
    /// Quality tier used for the run.
    #[serde(default)]
    pub quality: Quality,
    /// Parent/source variant for refinements, chat turns, promotions, and
    /// cross-model comparisons.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_variant_id: Option<SheetVariantId>,
    /// Operation origin.
    #[serde(default)]
    pub origin: VariantOrigin,
    /// Refinement-specific metadata.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refinement: Option<RefinementKind>,
    /// Full chat transcript for chat-generated variants.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chat_transcript: Option<ChatTranscript>,
    /// True when produced by "Promote to final".
    #[serde(default)]
    pub promotion: bool,
    /// Actual cost recorded after completion, if known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_usd: Option<f64>,
    /// What's in the sheet, panel by panel. Empty in B9; B10 fills it.
    #[serde(default, skip_serializing_if = "SheetComposition::is_empty")]
    pub composition: SheetComposition,
    /// Generation provenance. `None` in B9 for user-uploaded references;
    /// B10's generator populates these.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generation: Option<GenerationProvenance>,
    /// Palette extracted from the sheet image. Empty in B9; B10's
    /// generator runs eyedropper extraction at sheet-creation time.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extracted_palette: Vec<PaletteEntry>,
}

impl SheetVariant {
    /// Builds a variant with PRD v1 defaults from an embedded image.
    #[must_use]
    pub fn from_image(id: SheetVariantId, created_at: i64, image: ReferenceImage) -> Self {
        Self {
            id,
            created_at,
            template: ReferenceSheetTemplateId::Custom,
            width: default_sheet_width(),
            height: default_sheet_height(),
            chroma_color: default_reference_chroma(),
            user_prompt: String::new(),
            composed_prompt: String::new(),
            references: Vec::new(),
            real_world_grounding: false,
            applied_lora: None,
            lora_weight: default_lora_weight(),
            image,
            vector_image: None,
            model: ModelId::Auto,
            quality: Quality::Medium,
            parent_variant_id: None,
            origin: VariantOrigin::ManualImport,
            refinement: None,
            chat_transcript: None,
            promotion: false,
            cost_usd: None,
            composition: SheetComposition::default(),
            generation: None,
            extracted_palette: Vec::new(),
        }
    }
}

/// Panel rectangles within a sheet image, labelled by what they show.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SheetComposition {
    /// Full-body view rectangles: front, side, three-quarter, back.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub views: Vec<SheetPanel>,
    /// Facial expression panels: happy, angry, surprised, ...
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub expressions: Vec<SheetPanel>,
    /// Detail close-ups: scars, accessories, tattoos, runes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub callouts: Vec<SheetPanel>,
    /// Outfit / equipment variations.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub outfits: Vec<SheetPanel>,
    /// Palette swatch rectangle, if the sheet includes one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub palette_swatch: Option<Rect>,
}

impl SheetComposition {
    /// Returns `true` when no panel rectangles or palette swatch are
    /// recorded.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.views.is_empty()
            && self.expressions.is_empty()
            && self.callouts.is_empty()
            && self.outfits.is_empty()
            && self.palette_swatch.is_none()
    }
}

/// One labelled panel within a sheet.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SheetPanel {
    /// Rectangle within the sheet image.
    pub region: Rect,
    /// Semantic label: `front`, `side-left`, `happy`, `scar-over-eye`.
    pub label: String,
}

/// Provenance for a generated sheet variant.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct GenerationProvenance {
    /// Backend that produced the sheet (e.g. `anthropic`, `stability`,
    /// `replicate`).
    pub backend: String,
    /// Model identifier used for the run.
    pub model: String,
    /// The prompt that produced this variant.
    pub prompt: String,
    /// Seed for reproducible regeneration. `None` if the backend does
    /// not expose seeds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<u64>,
    /// Negative prompt, if used.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub negative_prompt: Option<String>,
}

/// Free-form structured metadata for an asset.
///
/// Common keys: `name`, `age`, `species`, `era`, `faction`. Open-ended;
/// artists capture whatever matters.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AssetInfo {
    /// Free-form keyed metadata.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub fields: BTreeMap<String, String>,
    /// Personality / behaviour notes — bullets shown in the sheet panel.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
}

impl AssetInfo {
    /// Returns `true` when no fields and no notes are recorded.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.fields.is_empty() && self.notes.is_empty()
    }
}

/// A prompt issued against a reference sheet.
///
/// Defined minimally for B9 — `prompt` plus optional `negative_prompt`
/// and `seed`, with the result the prompt produced. B10's sheet
/// generator extends this with backend-specific knobs (steps, CFG
/// scale, sampler) as the verb wiring lands.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PromptEntry {
    /// The prompt text.
    pub prompt: String,
    /// Optional negative prompt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub negative_prompt: Option<String>,
    /// Optional seed for reproducible regeneration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<u64>,
    /// What the prompt produced. `None` if the prompt is queued but not
    /// yet run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<PromptResult>,
    /// UTC seconds-since-epoch at which the prompt was issued.
    #[ts(type = "number")]
    pub issued_at: i64,
}

/// The outcome of a [`PromptEntry`].
///
/// Defined minimally for B9; B10 extends this as the streaming verb
/// runtime grows new shape (intermediate previews, multiple samples,
/// failure detail).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum PromptResult {
    /// The prompt produced a sheet variant; the id refers to a variant
    /// stored in the parent [`ReferenceSheet::canonical`] or
    /// [`ReferenceSheet::variants`].
    Variant(SheetVariantId),
    /// The prompt failed; the string is the user-facing error message.
    Error(String),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip<T>(value: &T) -> T
    where
        T: Serialize + for<'de> Deserialize<'de>,
    {
        let bytes = rmp_serde::to_vec_named(value).unwrap();
        rmp_serde::from_slice(&bytes).unwrap()
    }

    #[test]
    fn empty_library_round_trips() {
        let l = Library::default();
        assert!(l.is_empty());
        let back: Library = round_trip(&l);
        assert_eq!(l, back);
    }

    #[test]
    fn entity_kind_custom_carries_string() {
        let k = EntityKind::Custom("Character".into());
        let json = serde_json::to_string(&k).unwrap();
        assert_eq!(json, r#"{"kind":"Custom","value":"Character"}"#);
        let back: EntityKind = serde_json::from_str(&json).unwrap();
        assert_eq!(k, back);
    }

    #[test]
    fn export_bindings_entitycontent() {
        EntityContent::export_all(&Config::from_env()).expect("export EntityContent binding");
    }

    #[test]
    fn export_bindings_referencesheet() {
        ReferenceSheet::export_all(&Config::from_env()).expect("export ReferenceSheet binding");
    }

    #[test]
    fn entity_kind_unit_variants_serialise_compactly() {
        let k = EntityKind::Tileset;
        let json = serde_json::to_string(&k).unwrap();
        assert_eq!(json, r#"{"kind":"Tileset"}"#);
    }

    #[test]
    fn active_target_none_is_default() {
        assert!(ActiveTarget::default().is_none());
    }

    #[test]
    fn active_target_state_round_trips() {
        let t = ActiveTarget::State {
            entity_id: EntityId::new(7),
            state_id: StateId::new(3),
        };
        let back: ActiveTarget = round_trip(&t);
        assert_eq!(t, back);
    }

    #[test]
    fn empty_helpers_match_default() {
        assert!(EntityDefaults::default().is_empty());
        assert!(AiMetadata::default().is_empty());
        assert!(ProjectAi::default().is_empty());
        assert!(SheetComposition::default().is_empty());
        assert!(AssetInfo::default().is_empty());
    }

    #[test]
    fn default_reference_chroma_is_magenta() {
        assert_eq!(default_reference_chroma(), Rgba::opaque(255, 0, 255));
    }

    #[test]
    fn built_in_templates_include_expected_turnaround_defaults() {
        let templates = built_in_reference_sheet_templates();
        let turnaround = templates
            .iter()
            .find(|template| template.id == ReferenceSheetTemplateId::Turnaround4View)
            .expect("turnaround template");

        assert_eq!(
            turnaround.default_dimensions,
            SheetDimensions {
                width: 2048,
                height: 1024,
            }
        );
        assert_eq!(turnaround.default_chroma, default_reference_chroma());
        assert!(!turnaround.benefits_from_text_labels);
        assert!(
            turnaround
                .allowed_dimensions
                .contains(&turnaround.default_dimensions)
        );
    }

    #[test]
    fn sheet_variant_from_image_uses_manual_import_defaults() {
        let image = ReferenceImage {
            bytes: vec![1, 2, 3],
            mime: "image/png".into(),
        };
        let variant = SheetVariant::from_image(SheetVariantId::new(7), 123, image.clone());

        assert_eq!(variant.id, SheetVariantId::new(7));
        assert_eq!(variant.created_at, 123);
        assert_eq!(variant.image, image);
        assert_eq!(variant.template, ReferenceSheetTemplateId::Custom);
        assert_eq!(variant.chroma_color, default_reference_chroma());
        assert_eq!(variant.model, ModelId::Auto);
        assert_eq!(variant.quality, Quality::Medium);
        assert_eq!(variant.origin, VariantOrigin::ManualImport);
        assert!(variant.references.is_empty());
        assert!(!variant.promotion);
    }

    /// Pins the boxing decision for [`EntityContent`].
    ///
    /// `ReferenceSheet` is boxed inside `EntityContent::Sprites` so
    /// embedding optional reference sheets does not drag the enum's
    /// stack footprint up to the full sheet size.
    ///
    /// The cap is `1.5 * size_of::<Sprite>()` — generous enough to
    /// absorb small additions without churning the boxing decision,
    /// tight enough to fire if a variant payload starts dominating the
    /// enum again. When the test fires: either box the new offender or
    /// update both the cap and the rustdoc above `EntityContent` with
    /// the new measurement.
    #[test]
    fn entity_content_size_is_bounded() {
        use std::mem::size_of;

        let sprite = size_of::<Sprite>();
        let sheet = size_of::<ReferenceSheet>();
        let content = size_of::<EntityContent>();

        // ReferenceSheet larger than Sprite -> keep it boxed inside
        // the Sprites variant's optional reference slot.
        assert!(
            sheet > sprite,
            "ReferenceSheet ({sheet} bytes) shrank below Sprite ({sprite} bytes); \
             consider un-boxing the embedded reference_sheet"
        );

        let cap = sprite + sprite / 2;
        assert!(
            content <= cap,
            "EntityContent grew to {content} bytes; cap is {cap} \
             (1.5 * size_of::<Sprite>() = 1.5 * {sprite}). Box the \
             outgrown variant or update the cap with a recorded \
             measurement."
        );
    }

    /// Pins the `MessagePack` wire shape for [`EntityKind::Custom`].
    ///
    /// The JSON-side counterpart at `entity_kind_custom_carries_string`
    /// already covers the human-readable form. `rmp-serde` with
    /// `to_vec_named` writes a map of string keys to string values;
    /// decoding back into `BTreeMap<String, String>` and asserting both
    /// the `"kind"` and `"value"` entries pin the wire format against
    /// silent serde-attribute drift (`tag` / `content` rename).
    #[test]
    fn entity_kind_custom_messagepack_shape() {
        let k = EntityKind::Custom("Character".into());
        let bytes = rmp_serde::to_vec_named(&k).expect("encode");
        let decoded: BTreeMap<String, String> =
            rmp_serde::from_slice(&bytes).expect("decode generic");

        assert_eq!(decoded.len(), 2);
        assert_eq!(decoded.get("kind").map(String::as_str), Some("Custom"));
        assert_eq!(decoded.get("value").map(String::as_str), Some("Character"));
    }
}
