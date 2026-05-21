# Prompt, Style & Structure library — full implementation spec

Status: design approved, ready for implementation planning
Date: 2026-05-21
Owner: Luis Morales
Scope: replaces the hardcoded reference-sheet prompt/template system with a
user-managed, multi-tier library of Structures, Styles, and Prompts, consumed
by every AI verb through a single composition resolver.

This document specifies the complete implementation. There is no phasing —
everything described here is in scope for the feature. Reference sheets are the
first wired consumer because they exist today; the resolver is verb-agnostic so
the future verb streams (S23–S36) adopt it without further design.

---

## 1. Why

Today the prompts and layouts that drive AI generation are hardcoded Rust:

- `ai/src/verbs/reference_sheet/templates.rs` holds four `CompositionTemplate`
  enum variants (Character, Item, Tileset, Custom). Each variant hardcodes the
  positive prompt prose, the negative prompt, the sheet dimensions, and the
  panel geometry.
- `app/src/commands/library/reference_sheets.rs::compose_sheet_prompt()`
  hardcodes the layering of project style notes, background instructions,
  reference-image guidance, and operation hints.
- The only artist-facing knob is `ProjectAi.style_notes` — a single freeform
  string prepended to every prompt.

The result: the artist cannot author a layout, cannot save a reusable look,
cannot edit the wording that gets sent to the backend, and cannot carry any of
this between projects. The tool dictates style; the artist obeys. This feature
inverts that — the built-in prompts become editable data, and the artist owns a
library.

## 2. Goals and non-goals

Goals:

- Three reusable, named primitives — **Structure**, **Style**, **Prompt** —
  each existing at a built-in tier and a per-project tier.
- A single composition resolver in the `ai` crate that every verb calls; it
  produces the positive prompt, the negative prompt, and the panel slice
  rectangles from the same source of truth, so layout prose and slice geometry
  can never desync.
- Full CRUD on project-tier entries; fork-from-built-in; import/export as
  portable `.pixstyle` bundles; copy-from-another-project.
- Variable substitution in saved Prompts, auto-filled from entity metadata.
- Migration of the four existing templates into built-in data with byte-for-
  byte-equivalent output, and a backward-compatible `ProjectAi` schema bump.

Non-goals (explicitly out of scope, not "later"):

- No app-level cross-project user library tier. Reuse across projects happens
  through `.pixstyle` bundles and copy-from-project, not a hidden global store.
- No human-readable bundle format. Bundles use the same MessagePack+zstd stack
  as `.pixhaus`.
- No change to backend adapters, model routing, or the verb plugin protocol
  (B5) wire contract beyond the verb input shape described in §9.

## 3. The three primitives

All three are plain serializable records carrying a stable id, a display name,
and an implicit tier (built-in records live in the binary registry; project
records live in `ProjectAi`). Ids are string newtypes so a project record can
**shadow** a built-in by reusing its id.

### 3.1 Structure — the layout contract

A Structure is the only primitive the *code* depends on. It defines the canvas
and the panels; the resolver derives both the layout prose and the slice
rectangles from it.

```rust
// new: core/src/project/library/composition/structure.rs

/// Stable id for a Structure. Built-ins use reverse-DNS
/// (`pixhaus.builtin.structure.character`); project forks reuse that id to
/// shadow the built-in, or take a fresh project slug.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
pub struct StructureId(pub String);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Structure {
    pub id: StructureId,
    pub name: String,
    /// `Single` for verbs that produce one image (inpaint, recolor, upscale).
    /// `Paneled` for structured sheets (reference sheets, tilesets).
    pub output: StructureOutput,
    /// Layout-level negative clauses, e.g. "overlapping views, inconsistent
    /// scale". Merged with the picked Style's look negatives at compose time.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub layout_negatives: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum StructureOutput {
    /// One free-composition image; no panels, empty slice rects.
    Single,
    Paneled {
        canvas: Dimensions,           // { width, height }
        panels: Vec<StructurePanel>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct StructurePanel {
    /// Human label, also written into the generated variant's composition.
    pub label: String,
    /// Pixel rectangle within the canvas. Source of truth for slicing.
    pub rect: PanelRect,              // { x, y, w, h }
    /// Prose fragment describing this panel, with the canvas dims and the
    /// panel's own size interpolated by the resolver (see §6.2).
    pub prose_fragment: String,
    /// Which `SheetComposition` bucket this panel maps to. Drives backward
    /// compatibility with panel-scoped iteration (B10.2).
    pub slot: PanelSlot,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum PanelSlot {
    View,
    Expression,
    Callout,
    Outfit,
    PaletteSwatch,
    /// Verb-generic panel with no reference-sheet semantics.
    Generic,
}
```

`PanelSlot` exists because `SheetComposition` (the type stored on every
generated variant) buckets panels into `views`, `expressions`, `callouts`,
`outfits`, and `palette_swatch`, and B10.2's panel-scoped iteration reads those
buckets. The resolver maps `StructurePanel`s into the correct buckets by slot,
so authoring a Structure keeps the existing iteration workflow intact.

`Dimensions`, `PanelRect` are thin serializable structs; the resolver converts
`PanelRect` to the existing `pixhaus_core::project::Rect` via `Rect::from_xywh`.

### 3.2 Style — reusable look modifiers

```rust
// new: core/src/project/library/composition/style.rs

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
pub struct StyleId(pub String);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Style {
    pub id: StyleId,
    pub name: String,
    /// Positive look modifiers, e.g. "SNES 16-bit palette, 1px black
    /// outlines, dithered shading".
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub modifiers: String,
    /// Look-level negative clauses, e.g. "blurry, photorealistic, 3d render".
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub look_negatives: String,
    /// Optional generation defaults this style implies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_pref: Option<ModelId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quality: Option<Quality>,
}
```

`ModelId` and `Quality` are the existing enums in `core/src/project/library/ai.rs`.

### 3.3 Prompt — saved request template with variables

```rust
// new: core/src/project/library/composition/prompt.rs

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
pub struct PromptId(pub String);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PromptTemplate {
    pub id: PromptId,
    pub name: String,
    /// Request text with `{placeholder}` tokens, e.g.
    /// "a {species} warrior, idle pose, {extra}".
    pub text: String,
    /// Declared variables. Auto-detected tokens not listed here are treated
    /// as variables with an empty default and a label equal to the key.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub variables: Vec<PromptVariable>,
    /// Style applied by default when this prompt is picked (user can override).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_style: Option<StyleId>,
    /// Structure applied by default when this prompt is picked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_structure: Option<StructureId>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PromptVariable {
    pub key: String,        // matches `{key}` in `text`
    pub label: String,      // shown in the generation form
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub default: String,
}
```

> Naming note: the type is `PromptTemplate` to avoid colliding with the many
> existing uses of the word "prompt" as a plain `String` across the verb and
> app layers. The user-facing label is "Prompt".

## 4. Tiers

Each primitive exists at two tiers:

- **Built-in** — compiled into the binary, read-only. Loaded once at startup
  into a `BuiltinLibrary` (see §8.2). The four current templates migrate here
  (§8). Forking copies a built-in record into the project tier under the same
  id (shadowing) or a new id (a distinct entry), where it becomes editable.
- **Project** — stored in `ProjectAi`, full CRUD, serialized into the
  `.pixhaus` file so it travels with the project.

Resolution of a single id (used everywhere a record is looked up):

```
fn resolve_structure(id, project, builtins) -> &Structure:
    project.structures.get(id)          // project shadows built-in
        .or_else(|| builtins.structures.get(id))
        .ok_or(MissingStructure)
```

Same shape for `Style` and `PromptTemplate`. A project entry with the same id
as a built-in always wins.

## 5. The cascading baseline

Separate from the explicitly-picked primitives, there is one always-applied
baseline layer — the evolution of today's `ProjectAi.style_notes`.

```
fn baseline(project: &ProjectAi) -> &str:
    if project.style_notes is non-empty { project.style_notes }
    else { BUILTIN_DEFAULT_BASELINE }     // a const &str shipped in `ai`
```

Most-specific wins: a non-empty project baseline replaces the built-in default
outright (no concatenation, no surprise doubling). `style_notes` keeps its
existing field, type, and serde attributes — existing projects are unaffected.

## 6. Composition pipeline

The heart of the feature is one pure function that replaces both
`templates.rs::{build_prompt, build_negative_prompt, composition}` and the
app-level `compose_sheet_prompt()`.

### 6.1 Signature and location

```rust
// new crate module: ai/src/compose/mod.rs

pub struct ComposeRequest<'a> {
    pub baseline: &'a str,
    pub structure: &'a Structure,
    pub style: Option<&'a Style>,
    pub prompt: Option<&'a PromptTemplate>,
    pub variable_values: &'a BTreeMap<String, String>,
    pub inline_text: &'a str,            // free-typed additions
    pub inline_negatives: &'a str,
    /// Operation-specific trailing instruction (masked-edit preservation,
    /// promotion re-render, etc.) — moved verbatim out of compose_sheet_prompt.
    pub operation_hint: Option<&'a str>,
    /// Background / chroma, reference-image guidance, LoRA trigger, real-world
    /// grounding — the existing app-level fragments, passed through unchanged.
    pub context_fragments: &'a [String],
}

pub struct ComposedPrompt {
    pub positive: String,
    pub negative: String,
    pub composition: SheetComposition,   // empty buckets for Single output
    pub canvas: Dimensions,
}

pub fn compose(req: &ComposeRequest) -> Result<ComposedPrompt, ComposeError>;
```

`ComposeError` is a `thiserror` enum in the `ai` crate (unfilled required
variable, malformed placeholder, empty structure with paneled output, etc.).

### 6.2 Algorithm

Positive prompt, assembled in this fixed order. The picked-primitive sequence
(baseline → style → structure → prompt → inline) is the order approved during
brainstorming; the context fragments and operation hint are the existing
app-level pieces, slotted in just before the inline text where
`compose_sheet_prompt()` already places them:

1. **baseline** — §5.
2. **style modifiers** — `style.modifiers` if a Style is picked.
3. **structure layout prose** — for `Paneled`, concatenate each panel's
   `prose_fragment` with `{canvas_w}`, `{canvas_h}`, `{panel_w}`, `{panel_h}`,
   `{label}` interpolated, then append the canvas-level framing line. For
   `Single`, the structure contributes no layout prose.
4. **resolved prompt text** — `prompt.text` with `{placeholders}` substituted
   from `variable_values` (§7).
5. **context fragments** — joined in array order (background, references,
   grounding, LoRA — produced by the app exactly as today).
6. **operation hint** — appended last if present.
7. **inline text** — the user's free-typed additions.

Each non-empty segment is trimmed and joined with ". " (period-space),
collapsing duplicate separators, so the output reads as one clean prompt.

Negative prompt:

```
negative = join_nonempty(", ", [
    structure.layout_negatives,
    style.look_negatives (if style),
    inline_negatives,
])
```

`SheetComposition`: bucket each `StructurePanel` by `slot` into the matching
field (`View → views`, `Expression → expressions`, `Callout → callouts`,
`Outfit → outfits`, `PaletteSwatch → palette_swatch` (last one wins, it is a
single `Option<Rect>`), `Generic → views` as a fallback so generic panels still
slice). Convert each `PanelRect` via `Rect::from_xywh`. For `Single`, return
`SheetComposition::default()` (all empty).

The function is pure and deterministic: same inputs, same bytes out. That makes
it trivially snapshot-testable (§13).

## 7. Variable substitution

- Tokens are `{key}` where `key` matches `[a-z0-9_]+`. A literal brace is `{{`
  / `}}`.
- Resolution order for each token: explicit `variable_values[key]` →
  the entity's `info` map (the existing `BTreeMap<String,String>` on reference
  sheets, e.g. `species`, `age`) → the `PromptVariable.default` → error
  `UnfilledVariable(key)`.
- The app layer is responsible for collecting unfilled variables from the user
  before dispatch (see §11). The resolver itself errors on an unfilled
  variable rather than emitting a literal `{key}`.
- Auto-detection: any `{token}` in `prompt.text` not declared in `variables` is
  treated as a variable with `label == key` and empty default, so a user can
  type placeholders without a separate declaration step.

## 8. Built-in registry and migration

### 8.1 What migrates

The four `CompositionTemplate` variants become built-in records. The exact
current strings in `templates.rs` are the source of truth; we split each into a
Structure (geometry + layout prose + layout negatives) and contribute one
shared built-in "Default" Style for the look negatives common to all four.

Mapping per template:

| Old variant | Built-in Structure id | Output | Panels (slot) | Built-in Style |
|---|---|---|---|---|
| Character | `pixhaus.builtin.structure.character` | Paneled 1024×1536 | 5 View, 3 Expression, 2 Callout, 1 Outfit, 1 PaletteSwatch | `pixhaus.builtin.style.default` |
| Item | `pixhaus.builtin.structure.item` | Paneled 1024×1024 | 4 View, 2 Callout, 1 PaletteSwatch | default |
| Tileset | `pixhaus.builtin.structure.tileset` | Paneled 1024×1024 | 3 View, 1 PaletteSwatch | default |
| Custom | `pixhaus.builtin.structure.custom` | Paneled 1024×1024 | 1 View, 1 PaletteSwatch | default |

Panel rectangles are copied verbatim from the `*_composition()` functions in
`templates.rs` (e.g. Character views are `200×480` at `x = i*200, y = 0`; the
outfit slot is `256×384` at `0, 1120`; the palette swatch is `1024×128` at
`0, 672`). The `prose_fragment` per panel is the corresponding clause of the
old `build_prompt()` text, with the literal pixel numbers replaced by the
`{panel_w}`/`{panel_h}`/`{canvas_w}` tokens so prose and geometry derive from
the same `rect`.

Negative-prompt split:

- The clause shared by all four ("blurry, low quality, watermark, text label,
  logo, photo realistic, 3d render") and the look-specific tails ("extra
  limbs, bad anatomy") move to the built-in Default `Style.look_negatives`.
- The layout-specific tails ("overlapping views, inconsistent scale",
  "non-grid-aligned tiles, broken patterns", "floating elements") move to each
  Structure's `layout_negatives`.

A migration snapshot test (§13) asserts that `compose()` over the migrated
built-ins reproduces the pre-migration positive and negative strings for each
template, given the same user prompt — so the change is provably output-neutral.

### 8.2 Registry

```rust
// new: ai/src/compose/builtins.rs
pub struct BuiltinLibrary {
    pub structures: BTreeMap<StructureId, Structure>,
    pub styles: BTreeMap<StyleId, Style>,
    pub prompts: BTreeMap<PromptId, PromptTemplate>,
}

impl BuiltinLibrary {
    pub fn load() -> Self;   // constructs the records above, no I/O
}
```

`BUILTIN_DEFAULT_BASELINE` (§5) is a `const &str` here. Built-ins are pure
constructors — no file reads — so they cannot fail at runtime.

## 9. Verb integration

The composition layer is verb-agnostic. Reference sheets is the first consumer;
the design imposes no reference-sheet assumptions on the resolver (a `Single`
structure yields empty composition).

Changes to the generate-reference-sheet verb (`ai/src/verbs/reference_sheet/`):

- Delete the `CompositionTemplate` enum and `templates.rs` entirely (its data
  is now built-in records; its tests move to migration snapshot tests).
- `GenerateReferenceSheetInputs` changes from carrying `template:
  CompositionTemplate` to carrying ids + overrides:

```rust
pub struct GenerateReferenceSheetInputs {
    pub entity_id: EntityId,
    pub structure_id: StructureId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style_id: Option<StyleId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_id: Option<PromptId>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub variable_values: BTreeMap<String, String>,
    /// Free-typed prompt text (replaces the old `prompt` field for the
    /// inline case; a saved Prompt is referenced by `prompt_id`).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub inline_text: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub inline_negatives: String,
    #[serde(default = "default_num_variants")]
    pub num_variants: u32,
    #[serde(default = "default_quality", skip_serializing_if = "Option::is_none")]
    pub quality: Option<ImageQuality>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<u64>,
}
```

- The verb resolves ids against `project_library ⊕ builtins`, builds a
  `ComposeRequest`, calls `ai::compose::compose()`, and uses the returned
  `positive`/`negative`/`canvas`/`composition`. The resolved library and
  built-ins are threaded through `VerbContext` (a new
  `VerbContext::composition_library` field carrying a borrowed view of the
  project records plus the `BuiltinLibrary`).
- The verb's JSON-Schema `input_schema` is updated to the new shape (string ids
  instead of the template enum).
- Per-quality / per-model defaults implied by a picked Style (§3.2) are applied
  by the verb before backend selection, overridable by explicit input fields.

The iterate-reference-sheet verb is unchanged in behavior: it still operates on
the stored `SheetComposition` buckets, which the resolver now produces from the
Structure. No change to its inputs.

## 10. Storage and serialization

### 10.1 ProjectAi additions

Three additive fields on `ProjectAi` (`core/src/project/library/ai.rs`),
guarded with `#[serde(default, skip_serializing_if = ...)]` so old files load
and clean projects serialize nothing:

```rust
#[serde(default, skip_serializing_if = "Vec::is_empty")]
pub structures: Vec<Structure>,
#[serde(default, skip_serializing_if = "Vec::is_empty")]
pub styles: Vec<Style>,
#[serde(default, skip_serializing_if = "Vec::is_empty")]
pub prompts: Vec<PromptTemplate>,
```

`ProjectAi::is_empty` and `Default` are updated to include them. This is an
additive minor schema bump per `docs/file-format.md`; no migration code needed,
and `style_notes` is untouched.

### 10.2 The `.pixstyle` bundle

Import/export uses the same stack as `.pixhaus`: a header + MessagePack body +
zstd. New module `io/src/pixstyle.rs`:

```rust
pub struct StylePack {
    pub format_version: u16,
    pub structures: Vec<Structure>,
    pub styles: Vec<Style>,
    pub prompts: Vec<PromptTemplate>,
}

pub fn write_pack(pack: &StylePack, w: impl Write) -> Result<(), PixstyleError>;
pub fn read_pack(r: impl Read) -> Result<StylePack, PixstyleError>;
```

Export writes the user-selected subset. Import merges into `ProjectAi`: an
incoming record whose id collides with an existing project record prompts the
user to skip, overwrite, or import-as-copy (new minted id). Bundles are binary
(not git-diffable) — an accepted trade-off per the design decision.

### 10.3 Copy-from-project

`io` exposes a read-only helper that opens another `.pixhaus`, deserializes its
`ProjectAi`, and returns its three vectors for the app to present in a picker.
No write to the source project.

## 11. App / IPC layer

`compose_sheet_prompt()` in
`app/src/commands/library/reference_sheets.rs` shrinks to a thin adapter: it
builds the `context_fragments` (background chroma, reference-image guidance,
real-world grounding, LoRA trigger — unchanged logic) and the `operation_hint`
(masked/prompt-only/promotion strings — unchanged), then delegates to
`ai::compose::compose()`. It no longer assembles the positive/negative strings
itself.

New Tauri commands (tauri-specta, snake_case, `Result<_, String>` at the IPC
boundary per repo conventions):

```
library_list_composition(project) -> { structures, styles, prompts, builtins }
library_upsert_structure(structure) / _style(style) / _prompt(prompt)
library_delete_structure(id) / _style(id) / _prompt(id)
library_fork_builtin(kind, builtin_id, as_new: bool) -> new record
library_export_pack(selection, path)
library_import_pack(path, conflict_policy) -> import report
library_copy_from_project(source_path, selection) -> import report
library_resolve_prompt_variables(prompt_id, entity_id)
    -> [{ key, label, default, autofilled_value? }]   // drives the form
```

Generation commands accept the new id+overrides input shape and, before
dispatch, ensure every required variable is filled (using
`library_resolve_prompt_variables`); the UI collects any gaps.

## 12. UI

A "Prompt & Style Library" surface in the reference-sheet editor
(`ui/src/sheet/`), with room to promote it to a global panel later. Three tabs:
**Structures**, **Styles**, **Prompts**.

Per tab:

- List shows built-in (badged, read-only) and project records together.
- Actions: New, Edit, Duplicate, Fork from built-in, Delete (project only),
  Import `.pixstyle`, Export selection, Copy from project.
- **Structure editor**: a panel-list editor — each row is `label`, `slot`
  (dropdown of `PanelSlot`), `rect` (x/y/w/h), `prose_fragment` — plus a
  canvas-size field and a live preview that draws the panel rectangles to scale
  so the artist sees the layout. `layout_negatives` is a text area.
- **Style editor**: `modifiers` and `look_negatives` text areas, optional
  model/quality defaults.
- **Prompt editor**: `text` area with live `{token}` highlighting, a variables
  table (key/label/default, auto-populated from detected tokens), and optional
  default Style/Structure pickers.

Generation form changes: a Structure picker (required), a Style picker
(optional), a Prompt picker (optional), an inline-text box, and a variable
panel that appears with one field per unfilled `{placeholder}` (auto-filled
values shown pre-populated and editable). The existing quality / candidate-count
controls stay.

TypeScript types mirror the Rust records via `ts-rs` (`#[ts(export)]` already on
the new structs); `ui/src/lib/commands/library.ts` gains the new command
signatures and arg types. The provenance view continues to show the stored
`composed_prompt` per variant for audit.

## 13. Testing

- **Composition resolver (unit + snapshot)**: `insta` snapshots of `compose()`
  positive/negative output across structure/style/prompt/inline combinations,
  including `Single` output and empty optionals.
- **Migration equivalence (snapshot)**: for each of the four old templates,
  assert `compose()` over the migrated built-ins reproduces the exact
  pre-migration positive and negative strings for a fixed user prompt. This is
  the proof the migration is output-neutral. Port the existing `templates.rs`
  layout-geometry assertions to assert the built-in Structures yield the same
  `SheetComposition` rectangles.
- **Variable substitution (unit + proptest)**: token detection, `{{`/`}}`
  escaping, fill precedence (explicit → entity info → default → error),
  auto-detected undeclared tokens.
- **Tier resolution (unit)**: project record shadows built-in by id; missing id
  errors.
- **Serialization (unit)**: `ProjectAi` round-trips with and without the new
  vectors; an old `ProjectAi` MessagePack blob (no new fields) deserializes;
  `.pixstyle` write/read round-trip; `StylePack` `format_version` honored.
- **Import conflict policies (unit)**: skip / overwrite / import-as-copy each
  produce the expected merged `ProjectAi`.
- **IPC (integration)**: each new command via the existing app command test
  harness; `mockall` for any backend boundary already mocked in the
  reference-sheet tests.
- **UI**: structure-editor rect/preview logic and prompt-token highlighting
  unit-tested; generation-form variable collection tested.

Every new public function gets at least one test, per repo convention.

## 14. Backward compatibility

- `ProjectAi` schema bump is additive; old `.pixhaus` files load unchanged and
  gain empty library vectors.
- `style_notes` keeps working as the cascading baseline.
- Migration equivalence tests guarantee identical generation output for the
  four built-in Structures, so existing projects' results do not shift.
- The verb input shape changes (template enum → ids). Because this is a v1,
  pre-launch codebase with no persisted verb-invocation payloads, no input
  migration is required; the app always constructs the new shape. (If any
  fixture or saved request references the old `template` field, it is updated to
  `structure_id` in the same change.)

## 15. File-by-file change list

New:

- `core/src/project/library/composition/mod.rs` — re-exports.
- `core/src/project/library/composition/structure.rs` — `Structure`,
  `StructureOutput`, `StructurePanel`, `PanelSlot`, `StructureId`, `Dimensions`,
  `PanelRect`.
- `core/src/project/library/composition/style.rs` — `Style`, `StyleId`.
- `core/src/project/library/composition/prompt.rs` — `PromptTemplate`,
  `PromptVariable`, `PromptId`.
- `ai/src/compose/mod.rs` — `ComposeRequest`, `ComposedPrompt`, `ComposeError`,
  `compose()`.
- `ai/src/compose/builtins.rs` — `BuiltinLibrary`, `BUILTIN_DEFAULT_BASELINE`,
  the four migrated Structures and the Default Style.
- `ai/src/compose/variables.rs` — token parsing and substitution.
- `io/src/pixstyle.rs` — `StylePack`, `read_pack`/`write_pack`.
- `app/src/commands/library/composition.rs` — the new IPC commands.
- `ui/src/sheet/library/` — library panel, three tab editors, pickers,
  structure preview.

Modified:

- `core/src/project/library/ai.rs` — three new `ProjectAi` fields,
  `Default`/`is_empty` updates.
- `core/src/project/library/mod.rs` — module wiring, re-exports.
- `ai/src/verbs/reference_sheet/mod.rs` — new input shape, resolver call;
  delete `templates.rs`.
- `ai/src/plugin/verb.rs` (or `context.rs`) — `VerbContext::composition_library`.
- `app/src/commands/library/reference_sheets.rs` — `compose_sheet_prompt()`
  becomes a thin adapter.
- `ui/src/sheet/sheet-editor-state.ts`, `ReferenceSheetEditor.tsx`,
  `ui/src/lib/commands/library.ts` — pickers, variable panel, command bindings.

Deleted:

- `ai/src/verbs/reference_sheet/templates.rs` — replaced by built-in records;
  tests become migration-equivalence tests.

## 16. Decisions log (from brainstorming)

- Scope: general layer across all AI verbs; reference sheets wired first.
- Concept model: three primitives — Structure, Style, Prompt.
- Resolution: explicit pick of named records on top of one cascading baseline.
- Tiers: built-in (read-only, forkable) + per-project; project shadows built-in
  by id. No app-level user library tier.
- Cross-project reuse: both `.pixstyle` import/export and copy-from-project.
- Structure editability: structured data (panels with label/rect/slot/prose),
  not raw prose, so prose and geometry cannot desync.
- Variables: supported, auto-filled from entity info, prompt for the rest.
- Bundle format: binary MessagePack+zstd (not human-readable), accepted.
- Negative prompts: layout negatives on Structure, look negatives on Style,
  merged at compose time with inline negatives.
