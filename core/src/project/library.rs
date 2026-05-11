//! Project library: entities, groups, tags, and AI metadata.
//!
//! A Pixhaus project is a [`Library`] of named [`Entity`] values. The
//! kind of an entity determines its content shape: a `Tileset` entity
//! holds a single tileset, a `Tilemap` entity holds a level scene that
//! references one or more tilesets, a `Reference` entity holds a
//! structured asset sheet that AI verbs use as a consistency anchor,
//! and a `Custom` entity is the user's free-form kind (Hero, Goblin,
//! Treasure-Chest, Vehicle, ...) and holds named states each backed by
//! a [`Sprite`].
//!
//! # Design notes
//!
//! - The kind enum is exactly four variants. The data model deliberately
//!   does not bake game-genre taxonomy — "Character", "Enemy", "Hero" all
//!   live in the user-typed string carried by `EntityKind::Custom`.
//! - Groups are optional and never auto-created. A pristine project has
//!   no entities and no groups.
//! - Tilesets are project-level so a single Forest tileset can back
//!   multiple Forest-1, Forest-2, Boss-Arena tilemap scenes (the Tiled
//!   `firstgid` model).
//! - References ship in B9 as a single image per entity wrapped in a
//!   [`ReferenceSheet`]; the structured generation/iteration workflow
//!   lands in B10.
//! - Custom-kind entities may carry an optional [`Entity::anchor_reference_id`]
//!   pointing at a `Reference`-kind entity. B10 wires AI verbs to consume
//!   the anchored sheet for visual consistency.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::color::Rgba;
use super::geometry::{Rect, Size};
use super::id::{EntityId, GroupId, LayerId, PaletteId, SheetVariantId, StateId, TagId, TileIndex};
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
/// The unit of organization for everything in a project: a Hero, a
/// Goblin, a Forest tileset, a Forest-1 level. Stable ids let renames
/// happen without breaking cross-entity references (anchor links,
/// tilemap-to-tileset, group membership).
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

    /// Optional anchor reference. Points at a `Reference`-kind entity
    /// whose [`ReferenceSheet`] is used as the consistency anchor for
    /// every AI verb invocation that targets this entity. Set on
    /// `Custom`-kind entities once the user approves a sheet; left
    /// `None` for Tilesets, Tilemaps, and References themselves. B10
    /// wires the existing AI verbs to consume this anchor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anchor_reference_id: Option<EntityId>,

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

/// Kinds of entity. Three system kinds plus one user-defined kind.
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
    /// An input image used by AI verbs (style reference, photo
    /// reference). One image per `Reference` for B9; mood-board grouping
    /// is a follow-up.
    Reference,
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
/// substantially larger than the unit variants. The B9.1-cleanup
/// regression test (`entity_content_size_is_bounded`) measured
/// `size_of::<ReferenceSheet>() > size_of::<Sprite>()`, so the
/// `Reference` variant is boxed: `Reference { sheet: Box<ReferenceSheet> }`.
/// `Sprites` stays inline because boxing it would force an allocation
/// on every library walk in the hot read path. The regression test
/// pins both invariants — flip the boxing decision if `Sprite` ever
/// outgrows [`ReferenceSheet`], and update both the rustdoc here and
/// the test in lockstep. The TS mirror unwraps `Box` automatically so
/// the generated type stays
/// `{ type: "Reference"; value: { sheet: ReferenceSheet } }`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
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

    /// `Reference`-kind entity: structured asset sheet (character /
    /// item / tileset model sheet) used by AI verbs as the consistency
    /// anchor. The full generation and iteration workflow lives in B10;
    /// B9 ships the type with a minimal canonical variant and empty
    /// history/metadata so B10 can fill it in without a schema
    /// migration.
    ///
    /// `sheet` is boxed because `ReferenceSheet` is the largest variant
    /// payload by stack size (`entity_content_size_is_bounded` pins
    /// the measurement). Boxing keeps `size_of::<EntityContent>()` from
    /// being dragged up by this comparatively rare variant. `Box<T>` is
    /// transparent to serde, so the on-disk wire format is unchanged.
    Reference {
        /// The structured reference sheet payload.
        sheet: Box<ReferenceSheet>,
    },
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

/// Project-level AI memory.
///
/// Per-entity AI metadata lives on [`Entity::ai`]; this struct collects
/// the project-wide pieces — the style learning corpus, the trained
/// `LoRA`, recent prompt history.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ProjectAi {
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

impl ProjectAi {
    /// Returns `true` when no project-level AI memory is set.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.style_corpus.is_empty()
            && self.project_lora_path.is_none()
            && self.prompt_history.is_empty()
    }
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
}

impl AiMetadata {
    /// Returns `true` when no AI metadata is attached.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.suggested_tags.is_empty() && self.vlm_summary.is_none() && self.embedding.is_none()
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

/// A structured asset reference sheet — the anchor for every subsequent
/// AI generation for the linked entity.
///
/// In B9 this struct ships in its minimal form: `canonical` is a single
/// [`SheetVariant`] containing one image and an empty composition; the
/// remaining fields default to empty. B10 implements sheet generation,
/// iterative refinement via prompts, panel layout (turnaround,
/// expressions, callouts, outfit variants), and palette extraction.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ReferenceSheet {
    /// The current canonical variant — the user-approved sheet.
    pub canonical: SheetVariant,

    /// Older or rejected variants the user generated and decided not to
    /// canonicalise. Newest first. Capped by [`ProjectAi`] settings
    /// when the cap mechanism lands in B10.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub history: Vec<SheetVariant>,

    /// Generation prompts run against this sheet, ordered oldest to
    /// newest. The last entry is what produced `canonical`. Empty in
    /// B9; populated by the sheet generation verbs in B10.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub prompts: Vec<PromptEntry>,

    /// Structured metadata: name, age, species, personality notes,
    /// outfit variations. Free-form keyed map. The new-entity flow in
    /// B9 prompts for a few common fields; B10 extends this with
    /// AI-suggested fields based on the generated sheet.
    #[serde(default, skip_serializing_if = "AssetInfo::is_empty")]
    pub info: AssetInfo,
}

/// One generated version of a reference sheet.
///
/// The sheet is one composite image with optional panel rectangles
/// describing the turnaround views, expressions, callouts, and outfit
/// variants inside it. B9 ships the type; B10 populates `composition`
/// with real panel data.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SheetVariant {
    /// Stable identifier within the parent [`ReferenceSheet`].
    pub id: SheetVariantId,
    /// UTC seconds-since-epoch at which this variant was generated or
    /// imported.
    #[ts(type = "number")]
    pub generated_at: i64,
    /// The composite sheet image.
    pub image: ReferenceImage,
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
    /// [`ReferenceSheet::history`].
    Variant(SheetVariantId),
    /// The prompt failed; the string is the user-facing error message.
    Error(String),
}

/// What the user is currently editing inside a project.
///
/// Replaces the old `CanvasState::active_sprite` model where the focus
/// was always a sprite. With the library, the focus can be a state
/// inside a `Custom`-kind entity, a Tileset, a Tilemap, or a Reference.
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
    /// Viewing a `Reference` entity (not editable in B9).
    Reference {
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

    /// Pins the boxing decision for [`EntityContent`].
    ///
    /// Measured sizes on the B9.1-cleanup branch (Rust 1.85, no
    /// platform-specific layout assumptions):
    /// - `size_of::<Sprite>()` ≈ 288 bytes
    /// - `size_of::<ReferenceSheet>()` ≈ 416 bytes (largest variant
    ///   payload by stack size; boxed inside `EntityContent::Reference`)
    /// - `size_of::<EntityContent>()` ≈ 1.2× `size_of::<Sprite>()` once
    ///   the inlined `Vec<NamedSprite>` header and tag are accounted for
    ///
    /// The decision rule from the cleanup brief: if
    /// `size_of::<ReferenceSheet>() > size_of::<Sprite>()`, box the
    /// Reference variant. That ordering holds today, so `Reference`
    /// carries `Box<ReferenceSheet>` and the enum's stack footprint is
    /// driven by `Sprite` plus the `Vec<NamedSprite>` header.
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

        // ReferenceSheet larger than Sprite → Reference variant boxed.
        // Once boxed, the variant no longer drives the enum size; the
        // assertion captures the *measured* ordering so a future shrink
        // of ReferenceSheet that flips the inequality forces us to
        // re-examine whether the Box is still earning its allocation.
        assert!(
            sheet > sprite,
            "ReferenceSheet ({sheet} bytes) shrank below Sprite ({sprite} bytes); \
             consider un-boxing the Reference variant"
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
