# Native Create mode — prompt management

Status: proposed. Companion to `native-ui-create-mode.md` and `native-ui-vertical-slice.md`.

Prompt management today is two text boxes and a four-item dropdown. The reference-sheet tab takes a
free-typed subject and a template name; the animation wizard takes a free-typed motion. Behind both
sits a full composition model — saved prompt templates with variables, layout structures, look
styles — that already persists in the project file and already assembles the exact string sent to
the backend. The shell just doesn't expose it, doesn't let you edit it, and doesn't even send the
project's own records to the verb. This doc gives prompt management a real home: a central view in
Create mode to build and customize templates, typed controls for their variables, an advanced mode
that shows and can override the full prompt, and a Create section rewired to actually consume all of
it.

Everything here is in the `v2` branch (`pixhaus-worktrees/v2`). The files that own the flow:

- `core/src/project/library/composition/{prompt,structure,style}.rs` — the `PromptTemplate`,
  `PromptVariable`, `Structure`, and `Style` types.
- `core/src/project/library/ai.rs` — `ProjectAi`, which persists `prompts`, `structures`, `styles`.
- `core/src/project/schema.rs` — the project schema version.
- `ai/src/compose/{mod,variables,builtins}.rs` — `compose`, `substitute`, and the built-in registry.
- `ai/src/plugin/context.rs` — `CompositionLibraryView`, `ProjectCompositionLibrary`, `VerbContext`.
- `ai/src/verbs/reference_sheet/mod.rs` — `GenerateReferenceSheetInputs` and the compose-then-send body.
- `shell/src/ai.rs` — `TEMPLATES`, `SheetJob`, `AnimJob`, and how the verb context is built.
- `shell/src/app.rs` — the Create workspace, `inspector_panel`, `reference_sheet_tab`, the wizard.
- `shell/src/commands.rs` — `push_sprite_edit` / `SpriteEdit`, the undo model to copy.

## What already exists, and why it's wasted today

The model is built and serializes. A `PromptTemplate` (`prompt.rs`) carries an id, a name, `text`
with `{key}` placeholders, and a `Vec<PromptVariable>`; a `PromptVariable` is `{ key, label, default }`.
A `Structure` (`structure.rs`) carries the canvas and panel layout plus `layout_negatives`; a `Style`
(`style.rs`) carries `modifiers` and `look_negatives`. All three live in `ProjectAi.{prompts,structures,styles}`
(`ai/src/project/library/ai.rs`) inside the `Library`, so they round-trip in the `.pixhaus`
MessagePack file. Built-in seeds (four structures, a default style, four example prompts) load from
`ai/src/compose/builtins.rs`; project records shadow built-ins by id through `CompositionLibraryView`
(`ai/src/plugin/context.rs`).

`ai::compose` (`ai/src/compose/mod.rs`) is the single place the final prompt is assembled. It is pure
and deterministic: baseline, style modifiers, structure prose, the variable-substituted template
text, context fragments, an operation hint, and the inline subject — joined with `". "` for the
positive; structure plus style plus inline negatives joined with `", "` for the negative.
`substitute` (`ai/src/compose/variables.rs`) fills `{key}` tokens from explicit values, then entity
info, then variable defaults.

Now the gaps.

**There is no UI to manage any of it.** Templates, structures, and styles can only be created in code
(the built-ins). A user can't write a template, name a variable, save a style, or organize them. The
v2 settings window (`shell/src/settings.rs`) has General, Keybinds, and AI backends — no prompt
surface at all. (The "prompt settings" memory is from the old Tauri/TS UI; v2 never ported it.)

**The shell throws its own records away before the verb sees them.** `reference_sheet_tab`
(`shell/src/app.rs`) only fills `inline_text` and a hardcoded `structure_id` from `ai::TEMPLATES`
(`shell/src/ai.rs`). The `GenerateReferenceSheetInputs` construction in `shell/src/ai.rs` pins
`style_id`, `prompt_id` to `None`, `variable_values` to an empty map, `inline_negatives` to empty,
`quality` to `None`. Worse, the verb context is built with `VerbContext::empty(meta)` — so even if a
saved `prompt_id` were set, the verb's `library_view` (`reference_sheet/mod.rs:291-337`) would resolve
only built-ins, never the project's own prompts/styles/structures. Project-tier records are
unreachable from generation.

**The composed prompt is invisible and uneditable.** `compose` runs inside the verb, after the
shell hands off; the user never sees the string until it lands as provenance on a finished variant.
There is no way to preview it before paying for a generation, and no way to tweak it.

## The Prompt Library — a central Create-mode view

Create mode gets a third surface alongside Reference sheet and Animation: a **Prompt Library** that
takes over the central area (where `canvas_ui` normally paints). It is a main view, not a dock tab,
because building a template with several variables and reading a multi-line composed prompt needs
room. The canvas and timeline stay one click away — nothing goes full-screen, per the slice doc.

Add a `CreateView { ReferenceSheet, Animation, Library }` selector to the Create-mode top strip. When
`Library` is active, the central panel renders the library instead of the canvas; the right inspector
shows the selected record's editor — for a template, its variable form and the live composed-prompt
preview (see Advanced mode). Reference sheet and Animation remain the *consumers* of what the library
holds.

The library view is three lists — templates, structures, styles — and an editor for the selection:

- **Templates.** Name, the `text` body with `{key}` placeholders, the variable list (add / remove /
  reorder), and optional default structure and style. Built-ins show read-only with a "Duplicate to
  customize" button; duplicating writes a project-tier copy under a new id.
- **Structures.** Name, output mode (Single or Paneled), and for paneled output the canvas size and
  panels. Built-in structures are read-only; most users will only ever pick one, so the structure
  editor is the advanced corner of the library, not its front door.
- **Styles.** Name, `modifiers`, `look_negatives`, optional model and quality preference.

CRUD operates on `Project.library.ai.{prompts,structures,styles}`. Project records shadow built-ins by
id; built-ins are never mutated — editing one means duplicating it into the project. Every edit routes
through a new `push_library_edit`, an undo command that mirrors `push_sprite_edit` / `SpriteEdit`
(`shell/src/commands.rs`) but snapshots the `ProjectAi` composition vectors instead of a `Sprite`. So
creating, renaming, and deleting templates all sit on the same undo history as pixel edits, and they
persist in the project file with no new format.

## Typed variable controls

A variable today is `{ key, label, default }` and renders as a text field. That is too blunt for the
things templates actually vary — a pose is one of a few choices, a count is a number in a range, a
tint is a colour. Extend `PromptVariable` (`prompt.rs`) with an optional, back-compatible control:

```rust
pub struct PromptVariable {
    pub key: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub default: String,
    #[serde(default)]
    pub control: VarControl,
}

#[derive(Default)]
pub enum VarControl {
    #[default]
    Text,
    Select { choices: Vec<String> },
    Number { min: f64, max: f64, step: f64 },
    Color,
}
```

`#[serde(default)]` means existing projects and the built-in prompts load unchanged — an absent
`control` is `Text`. This is a MINOR schema bump (4.2 -> 4.3 in `core/src/project/schema.rs`), additive
and back-compatible, like the composition types before it.

The control governs only the editor widget and how the chosen value renders to a string: `Select`
draws a dropdown of `choices`, `Number` a slider or drag value formatted to a plain number, `Color` a
swatch rendered as `#RRGGBB`. Substitution stays string-valued — `substitute` and `compose`
(`ai/src/compose/variables.rs`, `mod.rs`) are untouched, because by the time a value reaches them it is
already a string. Typed controls are a UI and authoring nicety, not a change to how prompts compose.

A worked example. A "creature" template with text `a {pose} {species}, {count} of them, {tint} accents`
exposes `pose` as a `Select { ["idle", "walk", "attack"] }`, `species` as `Text` (default "slime"),
`count` as a `Number { 1, 8, 1 }`, and `tint` as a `Color`. The variable form draws a dropdown, a text
box, a slider, and a swatch; the composed text reads `a walk slime, 3 of them, #66ccff accents`.

## Advanced mode — preview and override

The user should see the exact prompt before spending a generation, and be able to tweak it. Both come
from the same pure function the verb uses.

**Preview is free and synchronous.** The shell builds a `CompositionLibraryView::new(structures,
styles, prompts, BuiltinLibrary::load())` from `doc.project.library.ai` and calls `ai::compose` on the
UI thread — no backend, no await. Both `compose` and `CompositionLibraryView::new` are already `pub`.
The Advanced section (collapsed by default, under the reference-sheet and animation controls) shows the
composed positive and negative, updating live as the user edits the template, fills variables, or
changes the inline subject.

**Override is opt-in.** The two preview fields are editable. A "Send my edits verbatim" toggle, when
on, sends the edited text instead of recomposing. Add two optional fields to
`GenerateReferenceSheetInputs` (`reference_sheet/mod.rs`):

```rust
#[serde(default, skip_serializing_if = "Option::is_none")]
pub prompt_override: Option<String>,
#[serde(default, skip_serializing_if = "Option::is_none")]
pub negative_override: Option<String>,
```

In the verb body, when `prompt_override` is `Some`, use it as the positive and skip composing the
positive; same for the negative. The structure is still resolved either way — it sets the canvas size
and the panel geometry the output is sliced into (`reference_sheet/mod.rs:291-337`). So a user can
hand-write the prose and still get a correctly paneled sheet. With the toggle off, the overrides are
`None` and the verb composes as before.

## Rewiring the Create section to consume the library

The library is useless if generation ignores it. Four changes connect them.

- **Send the project's records to the verb.** Replace `VerbContext::empty(meta)` (`shell/src/ai.rs`)
  with a context carrying a `ProjectCompositionLibrary` built from `doc.project.library.ai`
  (`ai/src/plugin/context.rs`). Now `library_view` resolves saved and customized prompts, styles, and
  structures, not just built-ins.
- **Widen `SheetJob`.** Add `style_id: Option<StyleId>`, `prompt_id: Option<PromptId>`,
  `variable_values: BTreeMap<String, String>`, `inline_negatives: String`, `quality:
  Option<ImageQuality>`, and the two overrides (`shell/src/ai.rs`). Populate the
  `GenerateReferenceSheetInputs` from them instead of the current `None` / empty defaults.
- **Rebuild the reference-sheet tab.** Replace the hardcoded `ai::TEMPLATES` combo
  (`reference_sheet_tab`, `shell/src/app.rs`) with library-backed pickers — structure, template, style —
  drawn from the resolved view, plus the selected template's variable form, the inline subject and
  negatives, the quality control, and the Advanced section. Approve still only sets the anchor; the
  preview path is unchanged.
- **Manage motion prompts the same way.** The animation wizard's motion text becomes a managed preset
  too, reusing `PromptTemplate` with variables and a `Single` (no-panel) structure. The animation
  consumer substitutes the variables and feeds the resulting string into `AnimJob.motion_prompt`
  (`shell/src/ai.rs`), with the same Advanced preview and override. One library, two consumers — the
  reference sheet and the animation — rather than a second parallel system.

## What this is not, and open risks

- **Not removing the built-ins.** They stay as read-only seeds; customizing means duplicating into the
  project. A fresh project still generates in one click with no library work.
- **Not a new file format.** Prompts, structures, and styles already persist in `ProjectAi`; this only
  adds the `VarControl` field (serde-default) and the override fields (serde-default).
- **Substitution stays string-based.** Typed controls shape the editor and the rendered string, not the
  compose pipeline. Don't push types into `substitute`.
- **Motion structures are out of scope.** The animation path reuses `Single`/no-structure templates;
  paneled layouts remain a reference-sheet concept.
- **Risk: structure editing is deep.** Most users never need it. Lead the library with templates and
  styles; keep the structure editor a labelled advanced corner so the front door stays simple.

## Verification

Once built:

- **Automated.** Unit tests for the `PromptVariable` / `VarControl` serde round-trip including the
  defaulted (absent) control, so old projects load; `compose` honoring the override fields (override
  wins for the prompt text, canvas and panel geometry still come from the structure); and a
  `push_library_edit` undo round-trip (add a template, undo, assert it's gone; redo, assert it's back).
  `cargo nextest run -p pixhaus-shell` and `--workspace` stay green.
- **By hand**, from the `v2` worktree: open Create mode, switch to the Prompt Library, duplicate a
  built-in template, add a Select, a Number, and a Color variable, and fill them; watch the live
  composed prompt update in the Advanced preview; flip "Send my edits verbatim", tweak the text, and
  generate a reference sheet; confirm the prompt recorded on the finished variant matches what the
  preview showed, and that a saved-then-reopened project keeps the customized template.
