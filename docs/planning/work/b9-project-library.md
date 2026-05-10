# B9 — Project library model

The bedrock spec for organizing many sprites, tilesets, and tilemaps within a single Pixhaus project. Turns the editor from "edit one sprite" into "build a game's worth of art."

This is the next foundational piece. Every AI-native organization feature (auto-tag, semantic search, project style learning at the library level, generate-character-with-states) sits on top of this data model.

## Why now

The current `Project` is `sprites: Vec<Sprite>` — a flat list. Tilesets live inside individual sprites, not at the project level. There's no concept of a "Hero character" with multiple states (idle, walk, attack). There's no "Forest biome" that owns a tileset and several tilemaps. Searching, tagging, AI auto-organization — none of it has a place to live.

Real game projects are a hundred sprites organized by entity (one Hero with eight states) and a dozen tilesets (Forest, Dungeon, City) referenced by a few tilemap scenes (Forest-1, Forest-2, Boss-Arena). The flat list collapses the moment you ship a real game.

Research on how Blender, Spine, Live2D, Unity, Adobe Animate, Aseprite, Pixelorama, Procreate, and Krita organize multi-asset projects is in [`../research/project-library-research.md`](../research/project-library-research.md). The summary: Spine's skin system is the cleanest single pattern, Unity's GUID-based references win on rename safety, and AI-native tools (Scenario, ComfyUI, Midjourney) have weak organization — meaning there's open ground for Pixhaus to lead.

## Decisions locked (2026-05-08)

- **Stated sprites**, not one-sprite-with-frame-tags. Each state is its own Sprite under an entity.
- **No built-in entity-kind taxonomy.** The kind enum is exactly four variants: `Tileset`, `Tilemap`, `Reference`, `Custom(String)`. The Custom string is the user's free-form category — "Hero", "Enemy", "NPC", "Prop", whatever they type. Pixhaus does not bake game-genre assumptions into the data model.
- **No auto-created default groups.** A new project starts empty. We surface optional starter templates instead — picking a template populates a few entities and groups, but skipping it leaves a clean empty library.
- **Aseprite export defaults to per-state files** with merged-file as an opt-in.
- **References are structured sheets, not single images.** Every Reference entity carries a `ReferenceSheet` struct with the canonical sheet, generation history, and structured metadata (name, age, species, personality notes, outfit variations, expressions, detail callouts). B9 ships the type with minimal content (one variant, one image, empty metadata); the full generation/iteration/panelization workflow ships in **B10 — Reference sheets and anchor mechanic**.
- **Custom-kind entities have an optional `anchor_reference_id`** pointing at a Reference entity. When set, AI verbs use the anchored sheet for visual consistency across all subsequent generation for that entity. The pointer is the data-model hook B10 needs.
- **Mood-board References parked.** The user has a separate idea for grouping multiple References into mood boards; explicitly out of scope for B9 and B10. Lives as a follow-up after both ship.
- **Tilemap is multi-tileset** matching the Tiled `firstgid` model that S12 already exports.

## The mental model

A Pixhaus project is a library of named entities. An entity is what an artist refers to in conversation: "the Hero", "the Forest tileset", "the Forest-1 level". Each entity has a kind that determines its content shape:

- **Tileset** — "Forest", "Dungeon", "City" — the tile primitives + autotile rules
- **Tilemap** — "Forest-1", "Boss-Arena" — a level scene built from one or more Tilesets
- **Reference** — style references, photo refs that don't get edited but inform AI generation. One image per Reference.
- **Custom(category)** — everything else. The string is the user's category: `Custom("Character")`, `Custom("Enemy")`, `Custom("Vehicle")`, `Custom("Mech")`. Custom-kind entities hold named **states** — multiple Sprites under one logical thing (a Hero with idle, walk, attack).

A separate **Group** type provides folder-style organization. Groups are optional, never auto-created, and any entity may belong to at most one. Groups can nest.

The organizing principle: an entity is what an artist refers to in conversation. States are what they refer to inside an entity ("the Hero's walk cycle", "the Hero's attack-1"). The kind enum stays small and stable; the user's taxonomy lives in the Custom string and in tags. A Pixhaus project is a flat list of entities with a search-and-tag layer over the top — no game-genre assumptions baked in.

## Data model

The full Rust model. New types are in bold; existing types reused are noted.

### `Library` — the new project-level container

```rust
/// Top-level container of all entities, palettes, tags, and AI metadata
/// in a Pixhaus project. Replaces `Project.sprites: Vec<Sprite>`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Library {
    /// Stable list of entities in this project. Order matters for the
    /// library tree UI (insertion order, manually re-orderable).
    pub entities: Vec<Entity>,

    /// Optional groups for tree-style organization (Characters, Enemies, ...).
    /// Any entity may belong to at most one group via `Entity.group_id`.
    /// Groups can nest via `EntityGroup.parent_id`.
    pub groups: Vec<EntityGroup>,

    /// Project-wide shared palettes. Entities reference by id; sprites
    /// inside an entity may also have local palettes that override these.
    pub palettes: Vec<Palette>,

    /// Defined tags. The user creates tags here; entities reference them
    /// by id. Auto-generated tags (from VLM analysis) are also stored here
    /// with `auto_generated = true`.
    pub tags: Vec<TagDefinition>,

    /// Project-level AI metadata: style learning corpus, project LoRA,
    /// prompt history.
    pub ai: ProjectAi,
}
```

### `Entity` — the named, typed unit of organization

```rust
/// A named entity in the project library — Hero, Goblin, Forest tileset,
/// Forest-1 level. The unit of organization for everything in a project.
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
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub group_id: Option<GroupId>,

    /// Tag ids attached to this entity.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub tags: Vec<TagId>,

    /// Defaults inherited by states/content within this entity.
    pub defaults: EntityDefaults,

    /// The actual content. Shape depends on `kind`.
    pub content: EntityContent,

    /// Per-entity AI metadata: which sprites within this entity are
    /// part of the style reference corpus, prompt history.
    #[serde(skip_serializing_if = "AiMetadata::is_empty", default)]
    pub ai: AiMetadata,

    /// Optional anchor reference. Points at a Reference-kind entity
    /// whose `ReferenceSheet` is used as the consistency anchor for
    /// every AI verb invocation that targets this entity. Set on
    /// Custom-kind entities once the user approves a sheet; left
    /// `None` for Tilesets, Tilemaps, and References themselves.
    /// B10 wires the existing AI verbs to consume this anchor.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub anchor_reference_id: Option<EntityId>,

    /// Free-form user data (text + tint), reusing existing UserData.
    #[serde(skip_serializing_if = "UserData::is_empty", default)]
    pub user_data: UserData,

    /// Created at, updated at — UTC seconds since epoch.
    pub created_at: i64,
    pub updated_at: i64,
}

/// Kinds of entity. Three system kinds + one user-defined kind.
///
/// The data model deliberately does not bake game-genre taxonomy.
/// "Character", "Enemy", "Hero", "Boss" — none of those are kinds.
/// They live in the `Custom(String)` variant where the string is the
/// user's category name.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(tag = "kind", content = "value")]
pub enum EntityKind {
    /// Tile primitives + autotile rules. Project-level so multiple
    /// Tilemap entities can share one.
    Tileset,
    /// A level scene that places tiles drawn from one or more Tileset
    /// entities. Multi-tileset is a first-class case (Tiled `firstgid` model).
    Tilemap,
    /// An input image used by AI verbs (style reference, photo reference).
    /// One image per Reference for B9; mood-board grouping is a follow-up.
    Reference,
    /// User-defined entity. The string is the user's free-form category
    /// — typically "Character", "Enemy", "NPC", "Prop", "Vehicle", "UI",
    /// "Effect", or anything else they type. The autocomplete in the
    /// new-entity modal surfaces common categories as suggestions but
    /// imposes no schema.
    Custom(String),
}

/// Type-specific content. Variants line up with `EntityKind`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(tag = "type", content = "value")]
pub enum EntityContent {
    /// Custom-kind entity: named states, each backed by a Sprite.
    /// The first state is the "primary" — what shows in thumbnails by
    /// default. A typical Hero entity has states `[idle, walk, run, attack]`.
    Sprites { states: Vec<NamedSprite> },

    /// Tileset entity: holds a single tileset (the tile primitives and
    /// autotile rules). Hoisted from inside Sprite into a project-level
    /// entity so multiple tilemaps can share it.
    Tileset { tileset: Tileset },

    /// Tilemap entity: a level scene that references one or more
    /// Tileset entities via id and places tiles on layered grids.
    Tilemap { scene: TilemapScene },

    /// Reference entity: structured asset sheet (character / item / tileset
    /// model sheet) used by AI verbs as the consistency anchor. The full
    /// generation and iteration workflow lives in B10; B9 ships the type
    /// with a minimal canonical variant and empty history/metadata so
    /// B10 can fill it in without a schema migration.
    Reference { sheet: ReferenceSheet },
}

/// A structured asset reference sheet — the anchor for every
/// subsequent AI generation for the linked entity. Modelled after
/// professional character / model sheets used in studio art pipelines.
///
/// In B9 this struct ships in its minimal form: `canonical` is a single
/// `SheetVariant` containing one image and an empty composition; the
/// remaining fields default to empty. B10 implements sheet generation,
/// iterative refinement via prompts, panel layout (turnaround,
/// expressions, callouts, outfit variants), and palette extraction.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ReferenceSheet {
    /// The current canonical variant — the user-approved sheet.
    pub canonical: SheetVariant,

    /// Older or rejected variants the user generated and decided not
    /// to canonicalise. Newest first. Capped by `ProjectAi` settings
    /// when the cap mechanism lands in B10.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub history: Vec<SheetVariant>,

    /// Generation prompts run against this sheet, ordered oldest to
    /// newest. The last entry is what produced `canonical`. Empty in
    /// B9; populated by the sheet generation verbs in B10.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub prompts: Vec<PromptEntry>,

    /// Structured metadata: name, age, species, personality notes,
    /// outfit variations. Free-form keyed map. The new-entity flow
    /// in B9 prompts for a few common fields; B10 extends this with
    /// AI-suggested fields based on the generated sheet.
    pub info: AssetInfo,
}

/// One generated version of a reference sheet. The sheet is one
/// composite image with optional panel rectangles describing the
/// turnaround views, expressions, callouts, and outfit variants
/// inside it. B9 ships the type; B10 populates `composition` with
/// real panel data.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SheetVariant {
    pub id: SheetVariantId,
    pub generated_at: i64,
    /// The composite sheet image (PNG bytes — for B9 stored inline,
    /// B10 may externalise to a project sub-folder if size warrants).
    pub image: ReferenceImage,
    /// What's in the sheet, panel by panel. Empty in B9; B10 fills it.
    #[serde(skip_serializing_if = "SheetComposition::is_empty", default)]
    pub composition: SheetComposition,
    /// Generation provenance. Empty in B9 for user-uploaded references;
    /// B10's generator populates these.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub generation: Option<GenerationProvenance>,
    /// Palette extracted from the sheet image. Empty in B9; B10's
    /// generator runs eyedropper extraction at sheet-creation time.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub extracted_palette: Vec<PaletteEntry>,
}

/// Panel rectangles within a sheet image, labelled by what they show.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SheetComposition {
    /// Full-body view rectangles: front, side, three-quarter, back.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub views: Vec<SheetPanel>,
    /// Facial expression panels: happy, angry, surprised, etc.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub expressions: Vec<SheetPanel>,
    /// Detail close-ups: scars, accessories, tattoos, runes.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub callouts: Vec<SheetPanel>,
    /// Outfit / equipment variations.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub outfits: Vec<SheetPanel>,
    /// Palette swatch rectangle, if the sheet includes one.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub palette_swatch: Option<Rect>,
}

impl SheetComposition {
    pub fn is_empty(&self) -> bool {
        self.views.is_empty()
            && self.expressions.is_empty()
            && self.callouts.is_empty()
            && self.outfits.is_empty()
            && self.palette_swatch.is_none()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SheetPanel {
    /// Rectangle within the sheet image.
    pub region: Rect,
    /// Semantic label: "front", "side-left", "happy", "scar-over-eye".
    pub label: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct GenerationProvenance {
    pub backend: String,                 // "anthropic", "stability", etc.
    pub model: String,                   // model identifier
    pub prompt: String,
    pub seed: Option<u64>,
    pub negative_prompt: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AssetInfo {
    /// Free-form keyed metadata. Common keys: "name", "age", "species",
    /// "era", "faction". Open-ended; artists capture whatever matters.
    #[serde(skip_serializing_if = "BTreeMap::is_empty", default)]
    pub fields: BTreeMap<String, String>,
    /// Personality / behaviour notes — bullets shown in the sheet panel.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub notes: Vec<String>,
}

/// Defaults inherited by states/content under the entity.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct EntityDefaults {
    /// Default canvas size. New states inherit this. Override per-state OK.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub canvas_size: Option<Size>,

    /// Default color mode for new states.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub color_mode: Option<ColorMode>,

    /// Default palette id (refers to a palette in `Library.palettes`).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub default_palette_id: Option<PaletteId>,

    /// Default pivot for sprite handoff.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub default_pivot: Option<Pivot>,

    /// Default playback FPS for new animation states.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub default_fps: Option<u16>,
}
```

### `NamedSprite` — a single state under a stated-sprite entity

```rust
/// A named state of a Character/Enemy/Prop/Ui/Custom entity, e.g.
/// "idle", "walk", "attack-1". Wraps an existing Sprite.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct NamedSprite {
    /// State id. Stable across renames so animation refs don't break.
    pub id: StateId,

    /// State name. Conventional set: idle, walk, run, jump, attack,
    /// hurt, death — but any string is valid.
    pub state_name: String,

    /// The actual sprite content (existing Sprite type, reused). The
    /// sprite's `name` field becomes the display name in the editor's
    /// title bar; convention: "EntityName / state_name".
    pub sprite: Sprite,

    /// Optional engine-side tags (running-state, can-be-cancelled, etc.)
    /// for handoff to game engines. No editor semantics.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub engine_tags: Vec<String>,
}
```

### `TilemapScene` — a level built from Tileset entities

```rust
/// A level scene that references one or more Tileset entities and places
/// tiles on layered grids.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct TilemapScene {
    /// Grid dimensions in tile cells.
    pub size: Size,

    /// References to Tileset entities used by this scene. Each reference
    /// gets a `first_gid` for Tiled-compat export (TMX uses global tile
    /// ids that disambiguate across multiple tilesets).
    pub tilesets: Vec<TilesetReference>,

    /// Tile cell layers (existing TilemapData reused).
    pub layers: Vec<TilemapLayer>,

    /// Free-form key/value properties for engine handoff
    /// (e.g., "music = forest-theme.ogg").
    #[serde(skip_serializing_if = "BTreeMap::is_empty", default)]
    pub properties: BTreeMap<String, String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct TilesetReference {
    /// Stable id of the Tileset entity in the project library.
    pub tileset_entity_id: EntityId,
    /// First global tile id for this tileset (TMX-compat).
    pub first_gid: TileIndex,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct TilemapLayer {
    pub id: LayerId,
    pub name: String,
    pub data: TilemapData, // existing
    pub opacity: u8,
    pub visible: bool,
}
```

### Groups, tags, and AI metadata

```rust
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct EntityGroup {
    pub id: GroupId,
    pub name: String,
    /// Optional parent for nested groups. None = top-level.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub parent_id: Option<GroupId>,
    #[serde(skip_serializing_if = "UserData::is_empty", default)]
    pub user_data: UserData,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct TagDefinition {
    pub id: TagId,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub color: Option<Rgba>,
    /// True if created by VLM auto-tagging. Lets the UI display them
    /// differently and lets the user accept/reject them in batch.
    #[serde(default)]
    pub auto_generated: bool,
}

/// Project-level AI memory. Per-entity AI metadata lives on `Entity.ai`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ProjectAi {
    /// Entity ids included in the project's style reference corpus.
    /// The Project Style Learning verb (S30) trains a LoRA from these.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub style_corpus: Vec<EntityId>,

    /// Path to the trained LoRA file relative to the project directory.
    /// Optional — projects without learned style still work.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub project_lora_path: Option<String>,

    /// Recent prompts for resume-where-you-left-off semantic search.
    /// Capped — implementation details TBD in S21 follow-up.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub prompt_history: Vec<PromptHistoryEntry>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AiMetadata {
    /// Auto-tags suggested by VLM analysis but not yet user-confirmed.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub suggested_tags: Vec<TagId>,
    /// VLM-generated description used for semantic search.
    /// Re-derived when the entity changes.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub vlm_summary: Option<String>,
    /// Embedding for similarity search (optional, computed lazily).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub embedding: Option<Vec<f32>>,
}

impl AiMetadata {
    pub fn is_empty(&self) -> bool {
        self.suggested_tags.is_empty()
            && self.vlm_summary.is_none()
            && self.embedding.is_none()
    }
}
```

### `Project` after migration

```rust
pub struct Project {
    pub schema_version: SchemaVersion,
    pub feature_flags: FeatureFlags,
    pub metadata: ProjectMetadata, // existing
    pub library: Library,           // NEW (replaces sprites: Vec<Sprite>)
    pub canvas: CanvasState,
    pub brush: BrushState,
    pub selection: SelectionState,
    /// What the user is currently editing. Replaces `CanvasState::active_sprite`.
    pub active: ActiveTarget,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum ActiveTarget {
    /// No selection — empty library or first-launch.
    None,
    /// Editing a state of an entity.
    State { entity_id: EntityId, state_id: StateId },
    /// Editing a Tileset entity.
    Tileset { entity_id: EntityId },
    /// Editing a Tilemap entity.
    Tilemap { entity_id: EntityId },
    /// Viewing a Reference entity (not editable).
    Reference { entity_id: EntityId },
}
```

## Migration strategy — none, by design

Pixhaus is pre-launch and has no users. There are no `.pixhaus` files in the wild that need to round-trip. Writing migration code for a project that hasn't shipped is dead code with no payoff. We ship B9 as a clean break.

What this means concretely:

1. The schema version bumps to a new major. Files written by the pre-B9 Pixhaus refuse to load with a clear error: "This file was created with a pre-release Pixhaus and is not supported. Re-create it with the current version."
2. Existing fixture files in `examples/` are regenerated under the new schema as part of B9.1 — load each old fixture, populate a Library with one Character entity per sprite, save under the new schema, replace the old file. One-shot script, throwaway after the rebuild.
3. No `migrate_from_v1_to_v2` function lives in `io/`. The reader has one supported version and rejects everything else.
4. The schema-version field is still load-bearing — it earns its keep the moment we hit Pixhaus 0.1 → 0.2 with real users. But that's a future migration, not this one.

The Aseprite `.ase` reader/writer (S08) is unaffected by this — Aseprite is an external format we always have to handle. The decision about how stated entities round-trip to `.aseprite` is in Open Questions below.

## UX flows

### Creating a new project

Minimal flow. The new-project dialog asks for a name and offers an optional **starter template** picker (see Templates below). The user can skip the template; the project then opens with an empty library and a clear "Create your first entity" prompt in the library panel.

No groups are auto-created. The library starts truly empty. The user creates groups when they want them, names them what they want, organizes how they want.

### Creating a new entity

The library panel has a "+" button. Click → modal with:
- Kind: Tileset / Tilemap / Reference / Custom
- For **Custom**: a "Category" field with autocomplete suggestions (Character, Enemy, NPC, Prop, UI, Vehicle, Effect, Pet, Mount, Weapon, ...). Free-form — the user can type anything and that becomes the Custom string. Previously-used categories in this project rank first.
- Name input
- For Custom: optional initial states (a small input that defaults to one state, with quick-add buttons for the conventional set: idle, walk, run, jump, attack, hurt, death). Skippable.
- For Tileset: tile size (16x16 default)
- For Tilemap: which Tileset entities to reference (multi-select; multi-tileset is supported)
- For Reference: file picker for the source image
- Optional group dropdown (existing groups only — no auto-create)
- Optional starter tags

Submit → entity appears in the library tree. The active target updates to the first state (or the entity itself for Tileset/Tilemap/Reference).

### Adding a state to an existing entity

Right-click a Custom-kind entity → "Add state" → name with autocomplete from the conventional set (idle, walk, run, jump, attack, attack-1, attack-2, hurt, death, victory) plus previously-used state names in this project. Inherits canvas size, palette, pivot, FPS from `EntityDefaults`. The new state opens in the editor.

### Searching the library

Search bar at top. Searches:
- Entity names (substring match)
- Custom kind strings (typing "Character" finds every `Custom("Character")` entity)
- Tag names (any matching tag)
- VLM summaries (semantic — when available)
- Optional kind filter chips (Tileset / Tilemap / Reference / specific Custom strings used in the project)
- Optional tag filter chips

The chip set for Custom kinds is dynamic — it shows only categories that actually exist in the project. A pristine empty project shows no chips; a Project with Hero, Goblin, and Treasure-Chest entities offers chips for whatever Custom strings those use.

### Bulk operations

Multi-select entities → right-click → "Add tag", "Move to group", "Create group from selection", "Delete", "Generate variants...", "Train style LoRA from selection".

### Templates (starter projects)

Template usage is the answer to "I'm new and I don't want to think about structure." Picking a template at new-project time pre-creates a few entities and groups so the user has something to build from. Templates are pure UX — no schema effect, no required structure. They're equivalent to copying a sample project as a starting point.

Suggested templates to ship in B9:
- **Top-down RPG** — groups (Characters, Enemies, Tilesets, Maps), starter Custom("Character") entity Hero with `idle` state, starter Tileset entity Forest, starter Tilemap entity Level-1 referencing Forest.
- **Side-scroller** — groups (Player, Enemies, Tilesets, Levels), starter Custom("Player") entity, starter Tileset entity, starter Tilemap.
- **Top-down shooter** — different starter set.
- **Empty** — the default; no entities, no groups.

The picker shows three to five templates with one-line descriptions. The user can skip and start empty. They can also save the current project structure as a custom template later (a stretch feature, not B9 scope).

### The library tree

The library tree is whatever the user builds. With templates, it has some structure on day one. Without templates, it's a flat list until the user creates groups. Drag-and-drop reorders entities and moves them between groups. Drag a group onto another group to nest. Right-click a group → "Rename", "Delete", "Move to top level". Deleting a group with children prompts "Delete group only (keep entities)" or "Delete group and entities."

## AI integration points

This is where Pixhaus distinguishes itself from every other library system. Everything below uses existing AI verbs from S23-S36 — the library is the data layer they hang off.

### Generate a new character with states (verb chain)

User: "Create a hero, 32x32, fantasy style, with idle, walk, run, attack states."

Flow:
1. Create a `Custom("Character")` entity named "Hero" with `EntityDefaults.canvas_size = 32x32`. The category string comes from the conversational prompt or autocomplete; the user can edit it.
2. Use existing image-gen backend to produce idle (8 frames). This is the seed.
3. Add state "idle" with the generated frames.
4. Use the Continue verb (S24) to predict walk from idle. Add state "walk".
5. Use the Continue verb again to predict run from walk. Add state "run".
6. Use the Variant verb (S26) with motion description "swinging sword" to produce attack. Add state "attack".
7. The seed sprite is auto-added to the project's `style_corpus` so future generations stay consistent.

The user reviews each state in the preview-then-commit flow the verb runtime (S21) already supports.

### Auto-tag the library

The Critique verb (S29) extends to library mode: it reads each entity's primary state, asks a VLM for descriptive tags ("hero", "armored", "blue", "fantasy", "small"), stores them as suggested tags on `AiMetadata`. The user reviews in batch and accepts/rejects.

### Semantic search

When the user types a query that doesn't match a name or existing tag, the search bar offers "Search by description...". The query embeds; we cosine-distance against `AiMetadata.embedding` for each entity. Top matches surface.

### Project style learning

The Project Style Learning verb (S30 — already shipped) trains a per-project LoRA from the entities listed in `style_corpus`. The library panel exposes a "Train style" button that updates the corpus from currently-selected entities and kicks off training. Once trained, every subsequent verb invocation includes the project LoRA as a style reference automatically.

### "Make a goblin like the hero but green and smaller"

Cross-entity transfer. The Variant verb (S26) is parameterized:
- Source entity: Hero (`Custom("Character")`)
- Target entity: new `Custom("Enemy")` entity "Goblin"
- Description: "green skin, smaller, hunched, more savage"
- States to generate: same as Hero's

Output: a new `Custom("Enemy")` entity named Goblin with the same state list, AI-generated to match the description. The user reviews each state. The category string `"Enemy"` is suggested by the conversational layer based on the prompt — the user can override.

## Implementation outline

This is a meaty bedrock spec. Suggested split into sub-tasks:

- **B9.1 — Core types and fixture rebuild** — define `Library`, `Entity`, `EntityContent`, `NamedSprite`, `TilemapScene`, `EntityGroup`, `TagDefinition`, `AiMetadata`, `ProjectAi`, `ActiveTarget`. Round-trip tests. ts-rs export. Bump schema version major. Add the one-shot script that regenerates the fixtures in `examples/` under the new schema (throwaway after B9.1 lands). ~2-3 days.
- **B9.2 — IPC commands** — extend the command catalog (B4 era) with library operations: `library_create_entity`, `library_delete_entity`, `library_rename_entity`, `library_add_state`, `library_move_entity_to_group`, `library_create_group`, `library_search`, `library_get_entity`, `library_set_active_target`, `library_add_tag`, etc. ~3 days.
- **B9.3 — Library panel UI** — Solid component for the library tree, with drag-and-drop, context menus, the entity-creation modal, search bar. ~4 days.
- **B9.4 — AI library hooks** — wire the Critique verb to library auto-tagging, the Variant verb to cross-entity transfer, the Style Learning verb to project corpus management. ~3 days.
- **B9.5 — Aseprite round-trip** — see Open Questions for the strategy decision. ~2-4 days depending on the decision.

Total: roughly 14-17 days of agent work for the full library system. Critical-path: B9.1 + B9.2 + B9.3 — about 10 days. Library is usable without B9.4 and B9.5, which can land later.

## Resolved decisions and parked items

All six initial questions are answered. Logged here for the record:

1. **Stated sprites** — separate Sprite per state. Confirmed.
2. **No auto-created groups.** Project starts empty. Optional starter templates surface in the new-project dialog. Confirmed.
3. **EntityKind is exactly four variants** — `Tileset`, `Tilemap`, `Reference`, `Custom(String)`. No baked-in Character / Enemy / NPC / Prop / UI. The Custom string carries the user's free-form category. Confirmed.
4. **Aseprite export defaults to per-state files** with a merged-with-frame-tags option in the export dialog. Confirmed.
5. **Reference is one image per entity** for B9. The mood-board concept is parked — see "Parked for later" below.
6. **Tilemap is multi-tileset** (Tiled `firstgid` model). Confirmed.

### Parked for later

- **Mood-board References.** The user has a wild idea for this; intentionally deferring until after the rest of B9-and-beyond ships. The B9 data model treats References as single-image entities. When the time comes, a follow-up spec extends `EntityContent::Reference` to a richer shape, with a clean migration since we'll have a versioned schema by then.

## Acceptance criteria for B9 as a whole

- All types defined, documented with rustdoc, round-trip tested
- Schema version bumped; pre-B9 files refuse to load with a clear, user-facing error
- All `examples/` fixtures regenerated under the new schema via the throwaway B9.1 script; the script is deleted after B9.1 lands
- IPC command catalog updated with library operations
- Library panel UI replaces the current sprite tree, supports search and drag-drop
- VLM auto-tagging produces tags on demand (manual trigger; not on every save)
- Project Style Learning trains from `style_corpus` on demand
- The 14 existing AI verbs work with the library as input/output context (no behavior change for the verbs themselves; they get richer context for free)
- Documentation in `docs/library.md` with screenshots of the library panel

## What this enables next

Once B9 lands, the AI features the user actually wants get straightforward:
- "Generate a hero with all standard states" becomes a single verb chain
- "Make all goblins reuse the hero rig but green" becomes a Variant verb
- "Find the angry NPC sprite" becomes semantic search
- "Train a project LoRA from my hero and my forest tiles" becomes a button
- "Generate a forest level using my Forest tileset" becomes a Tileset+Tilemap verb chain

None of these are possible cleanly today because the data model has nowhere to put the relationships. B9 is the data model that lets every interesting AI feature land as a small addition.
