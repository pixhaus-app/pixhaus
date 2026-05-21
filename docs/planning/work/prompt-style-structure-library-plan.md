# Prompt, Style & Structure library — implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the hardcoded reference-sheet prompt templates with a user-managed, two-tier (built-in + per-project) library of Structures, Styles, and Prompts, consumed by every AI verb through one composition resolver.

**Architecture:** New `composition` types in the `core` crate (plain serializable records). A pure `compose()` resolver in the `ai` crate that turns picked records + a cascading baseline into a positive prompt, a negative prompt, and the panel slice geometry. The four existing `CompositionTemplate` variants migrate into built-in records with output-neutral results, proven by snapshot tests. Project-tier records persist additively on `ProjectAi`; a `.pixstyle` MessagePack+zstd bundle plus copy-from-project enable cross-project reuse. The reference-sheet verb, app IPC layer, and Solid UI are rewired to the new id+overrides shape.

**Tech Stack:** Rust (serde, thiserror, ts-rs, rstest, insta, proptest, rmp-serde, zstd), Tauri 2 + tauri-specta, TypeScript + Solid.js. Build/test via `cargo nextest`, `pnpm test`.

**Spec:** `docs/planning/work/prompt-style-structure-library.md`. Read it before starting; this plan implements it section-for-section.

**Conventions reminder (from CLAUDE.md and skills):**
- `thiserror` in `core`/`io`/`ai`; `anyhow` only in `app`. No `unwrap()`/`panic!()` outside tests.
- Every public function gets at least one test. `insta` for text snapshots, `proptest` for image/parse ops, `rstest` fixtures, `mockall` for trait mocks.
- Conventional Commits. End commit messages with the `Co-Authored-By` trailer required by CLAUDE.md. Commit after every passing step group.
- Run per-crate tests with `cargo nextest run -p <crate>`; clippy with `-D warnings`.

---

## File structure

New files:

- `core/src/project/library/composition/mod.rs` — module root, re-exports, shared `Dimensions`/`PanelRect`.
- `core/src/project/library/composition/structure.rs` — `Structure`, `StructureOutput`, `StructurePanel`, `PanelSlot`, `StructureId`.
- `core/src/project/library/composition/style.rs` — `Style`, `StyleId`.
- `core/src/project/library/composition/prompt.rs` — `PromptTemplate`, `PromptVariable`, `PromptId`.
- `ai/src/compose/mod.rs` — `ComposeRequest`, `ComposedPrompt`, `ComposeError`, `compose()`.
- `ai/src/compose/variables.rs` — token parsing + substitution.
- `ai/src/compose/builtins.rs` — `BuiltinLibrary`, `BUILTIN_DEFAULT_BASELINE`, migrated records.
- `io/src/pixstyle.rs` — `StylePack`, `read_pack`/`write_pack`, `PixstyleError`.
- `app/src/commands/library/composition.rs` — IPC commands.
- `ui/src/sheet/library/LibraryPanel.tsx`, `StructureEditor.tsx`, `StyleEditor.tsx`, `PromptEditor.tsx`, `pickers.tsx`.

Modified files:

- `core/src/project/library/mod.rs` — wire `composition` module + re-exports.
- `core/src/project/library/ai.rs` — three new `ProjectAi` fields; `Default`/`is_empty`.
- `ai/src/lib.rs` — `pub mod compose;`.
- `ai/src/verbs/reference_sheet/mod.rs` — new input shape; resolver call.
- `ai/src/plugin/context.rs` (or wherever `VerbContext` lives) — add `composition_library`.
- `io/src/lib.rs` — `pub mod pixstyle;`.
- `app/src/commands/library/mod.rs` + `reference_sheets.rs` — register commands; shrink `compose_sheet_prompt`.
- `ui/src/sheet/sheet-editor-state.ts`, `ReferenceSheetEditor.tsx`, `ui/src/lib/commands/library.ts`.

Deleted:

- `ai/src/verbs/reference_sheet/templates.rs`.

> Before Task 1, confirm exact paths with `rg`. Two lookups the plan assumes:
> `rg "pub struct VerbContext" ai/src` (to place the `composition_library` field) and
> `rg "fn compose_sheet_prompt" app/src` (to confirm the adapter location).

---

## Task 1: Shared composition value types

**Files:**
- Create: `core/src/project/library/composition/mod.rs`
- Modify: `core/src/project/library/mod.rs`

- [ ] **Step 1: Write the failing test**

In `core/src/project/library/composition/mod.rs`:

```rust
//! User-managed composition library: Structures, Styles, and Prompts that
//! drive AI generation. See docs/planning/work/prompt-style-structure-library.md.

mod prompt;
mod structure;
mod style;

pub use prompt::{PromptId, PromptTemplate, PromptVariable};
pub use structure::{PanelRect, PanelSlot, Structure, StructureId, StructureOutput, StructurePanel};
pub use style::{Style, StyleId};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Pixel canvas size for a paneled structure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Dimensions {
    pub width: u32,
    pub height: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dimensions_round_trip() {
        let d = Dimensions { width: 1024, height: 1536 };
        let json = serde_json::to_string(&d).unwrap();
        let back: Dimensions = serde_json::from_str(&json).unwrap();
        assert_eq!(d, back);
    }
}
```

This file references `prompt`, `structure`, `style` modules created in Tasks 2-4; it will not compile until those exist. Create empty stub files first so the crate builds:

```bash
touch core/src/project/library/composition/prompt.rs \
      core/src/project/library/composition/structure.rs \
      core/src/project/library/composition/style.rs
```

Put a single placeholder line in each stub so `pub use` resolves — but the real content lands in Tasks 2-4. To keep Task 1 self-compiling, temporarily comment the three `pub use` lines and the `mod` lines for `structure`/`style`/`prompt` are added as the tasks land. Simpler: implement Tasks 1-4 as one commit. **Do Tasks 1-4 together, committing once at the end of Task 4.** Steps below still run per-task so tests are written test-first.

- [ ] **Step 2: Wire the module**

In `core/src/project/library/mod.rs`, add near the other `mod` declarations:

```rust
pub mod composition;
```

- [ ] **Step 3: Run (deferred to Task 4 commit)**

Run: `cargo build -p pixhaus-core`
Expected: builds once Tasks 2-4 are in place.

---

## Task 2: Structure types

**Files:**
- Create/replace: `core/src/project/library/composition/structure.rs`
- Test: same file `#[cfg(test)]`

- [ ] **Step 1: Write the failing test**

```rust
//! The layout contract. A Structure defines the canvas and panels; the
//! `ai::compose` resolver derives both layout prose and slice rectangles
//! from it, so prose and geometry cannot desync.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::Dimensions;

/// Stable id for a Structure. Built-ins use reverse-DNS
/// (`pixhaus.builtin.structure.character`); a project record reuses that id
/// to shadow the built-in, or takes a fresh project slug.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct StructureId(pub String);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Structure {
    pub id: StructureId,
    pub name: String,
    pub output: StructureOutput,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub layout_negatives: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum StructureOutput {
    /// One free-composition image; no panels.
    Single,
    /// Structured multi-panel sheet.
    Paneled {
        canvas: Dimensions,
        panels: Vec<StructurePanel>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct StructurePanel {
    pub label: String,
    pub rect: PanelRect,
    /// Prose with `{canvas_w}`, `{canvas_h}`, `{panel_w}`, `{panel_h}`,
    /// `{label}` tokens interpolated by the resolver.
    pub prose_fragment: String,
    pub slot: PanelSlot,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PanelRect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
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
    Generic,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Structure {
        Structure {
            id: StructureId("test.s".into()),
            name: "Test".into(),
            output: StructureOutput::Paneled {
                canvas: Dimensions { width: 100, height: 200 },
                panels: vec![StructurePanel {
                    label: "front".into(),
                    rect: PanelRect { x: 0, y: 0, w: 50, h: 100 },
                    prose_fragment: "front view {panel_w}x{panel_h}".into(),
                    slot: PanelSlot::View,
                }],
            },
            layout_negatives: "overlapping views".into(),
        }
    }

    #[test]
    fn structure_round_trips() {
        let s = sample();
        let json = serde_json::to_string(&s).unwrap();
        let back: Structure = serde_json::from_str(&json).unwrap();
        assert_eq!(s, back);
    }

    #[test]
    fn single_output_serializes_as_snake_case() {
        let json = serde_json::to_string(&StructureOutput::Single).unwrap();
        assert_eq!(json, r#""single""#);
    }

    #[test]
    fn panel_slot_serializes_as_snake_case() {
        assert_eq!(serde_json::to_string(&PanelSlot::PaletteSwatch).unwrap(), r#""palette_swatch""#);
    }
}
```

- [ ] **Step 2: Run test (after Task 4)**

Run: `cargo nextest run -p pixhaus-core composition::structure`
Expected: PASS.

---

## Task 3: Style types

**Files:**
- Create/replace: `core/src/project/library/composition/style.rs`

- [ ] **Step 1: Write the failing test**

```rust
//! Reusable look modifiers — the artist's main library primitive.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::project::library::ai::{ModelId, Quality};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct StyleId(pub String);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Style {
    pub id: StyleId,
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub modifiers: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub look_negatives: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_pref: Option<ModelId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quality: Option<Quality>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn style_round_trips_minimal() {
        let s = Style {
            id: StyleId("test.style".into()),
            name: "SNES".into(),
            modifiers: "16-bit palette".into(),
            look_negatives: "blurry".into(),
            model_pref: None,
            quality: None,
        };
        let json = serde_json::to_string(&s).unwrap();
        let back: Style = serde_json::from_str(&json).unwrap();
        assert_eq!(s, back);
    }

    #[test]
    fn empty_optionals_are_skipped() {
        let s = Style {
            id: StyleId("x".into()),
            name: "x".into(),
            modifiers: String::new(),
            look_negatives: String::new(),
            model_pref: None,
            quality: None,
        };
        let json = serde_json::to_string(&s).unwrap();
        assert_eq!(json, r#"{"id":"x","name":"x"}"#);
    }
}
```

> Confirm `ModelId`/`Quality` are `pub` in `core/src/project/library/ai.rs` (they are, per spec §3.2). Confirm the import path with `rg "pub enum ModelId" core/src`.

- [ ] **Step 2: Run test (after Task 4)**

Run: `cargo nextest run -p pixhaus-core composition::style`
Expected: PASS.

---

## Task 4: Prompt types, then build + commit Tasks 1-4

**Files:**
- Create/replace: `core/src/project/library/composition/prompt.rs`

- [ ] **Step 1: Write the failing test**

```rust
//! Saved request template with variable placeholders.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{StructureId, StyleId};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PromptId(pub String);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PromptTemplate {
    pub id: PromptId,
    pub name: String,
    pub text: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub variables: Vec<PromptVariable>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_style: Option<StyleId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_structure: Option<StructureId>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PromptVariable {
    pub key: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub default: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_round_trips() {
        let p = PromptTemplate {
            id: PromptId("p1".into()),
            name: "Warrior".into(),
            text: "a {species} warrior".into(),
            variables: vec![PromptVariable {
                key: "species".into(),
                label: "Species".into(),
                default: "human".into(),
            }],
            default_style: Some(StyleId("s".into())),
            default_structure: None,
        };
        let json = serde_json::to_string(&p).unwrap();
        let back: PromptTemplate = serde_json::from_str(&json).unwrap();
        assert_eq!(p, back);
    }
}
```

- [ ] **Step 2: Restore the `pub use` lines** in `mod.rs` (uncomment if you commented them in Task 1).

- [ ] **Step 3: Build and run all composition tests**

Run: `cargo nextest run -p pixhaus-core composition`
Expected: all Task 1-4 tests PASS.

- [ ] **Step 4: Verify ts-rs export builds**

Run: `cargo test -p pixhaus-core export_bindings`
Expected: PASS (ts-rs generates `.ts` files for the new `#[ts(export)]` types). If the crate uses a dedicated bindings test, run that instead — confirm with `rg "export_bindings|TS::export" core`.

- [ ] **Step 5: Commit**

```bash
git add core/src/project/library/composition core/src/project/library/mod.rs
git commit -m "$(printf 'feat(core): add composition library value types\n\nStructure, Style, and PromptTemplate records with stable string ids,\nplus Dimensions/PanelRect/PanelSlot. Per spec sections 3.1-3.3.\n\nCo-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>')"
```

---

## Task 5: ProjectAi gains the three library vectors

**Files:**
- Modify: `core/src/project/library/ai.rs` (struct at lines 20-92, `is_empty` at 94-109)

- [ ] **Step 1: Write the failing test**

Add to the `#[cfg(test)]` module in `ai.rs` (create one if absent):

```rust
#[cfg(test)]
mod project_ai_library_tests {
    use super::*;
    use crate::project::library::composition::{Structure, StructureId, StructureOutput};

    #[test]
    fn new_project_ai_has_empty_library() {
        let ai = ProjectAi::default();
        assert!(ai.structures.is_empty());
        assert!(ai.styles.is_empty());
        assert!(ai.prompts.is_empty());
        assert!(ai.is_empty());
    }

    #[test]
    fn project_ai_with_structure_is_not_empty() {
        let mut ai = ProjectAi::default();
        ai.structures.push(Structure {
            id: StructureId("p.s".into()),
            name: "P".into(),
            output: StructureOutput::Single,
            layout_negatives: String::new(),
        });
        assert!(!ai.is_empty());
    }

    #[test]
    fn old_blob_without_library_deserializes() {
        // Simulate an old file: a ProjectAi JSON with none of the new fields.
        let old = r#"{}"#;
        let ai: ProjectAi = serde_json::from_str(old).unwrap();
        assert!(ai.structures.is_empty());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p pixhaus-core project_ai_library_tests`
Expected: FAIL — `no field structures on ProjectAi`.

- [ ] **Step 3: Add the fields**

In `ai.rs`, add to `ProjectAi` after `prompt_history` (line 74):

```rust
    /// Project-tier composition Structures. Shadow built-ins by id.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub structures: Vec<crate::project::library::composition::Structure>,

    /// Project-tier Styles.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub styles: Vec<crate::project::library::composition::Style>,

    /// Project-tier saved Prompts.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub prompts: Vec<crate::project::library::composition::PromptTemplate>,
```

In `Default for ProjectAi` (lines 78-91) add:

```rust
            structures: Vec::new(),
            styles: Vec::new(),
            prompts: Vec::new(),
```

In `is_empty` (lines 97-108) extend the chain:

```rust
            && self.prompt_history.is_empty()
            && self.structures.is_empty()
            && self.styles.is_empty()
            && self.prompts.is_empty()
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo nextest run -p pixhaus-core project_ai_library_tests`
Expected: PASS.

- [ ] **Step 5: Run the existing ProjectAi tests to confirm no regression**

Run: `cargo nextest run -p pixhaus-core ai::`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add core/src/project/library/ai.rs
git commit -m "$(printf 'feat(core): persist composition library on ProjectAi\n\nAdditive structures/styles/prompts vectors, skip-serialized when empty\nso old .pixhaus files load unchanged. Per spec section 10.1.\n\nCo-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>')"
```

---

## Task 6: Variable substitution

**Files:**
- Create: `ai/src/compose/variables.rs`
- Modify: `ai/src/lib.rs` (add `pub mod compose;`), `ai/src/compose/mod.rs` (add `pub mod variables;` — created in Task 7; for Task 6 create `mod.rs` with only the `variables` line and the `VarError` type below)

- [ ] **Step 1: Create `ai/src/compose/mod.rs` minimal**

```rust
//! Composition resolver: turns picked library records plus a cascading
//! baseline into a positive prompt, a negative prompt, and panel slice
//! geometry. See docs/planning/work/prompt-style-structure-library.md.

pub mod variables;
```

Add `pub mod compose;` to `ai/src/lib.rs` (confirm the lib root path with `rg "pub mod" ai/src/lib.rs`).

- [ ] **Step 2: Write the failing test**

In `ai/src/compose/variables.rs`:

```rust
//! `{token}` parsing and substitution for saved Prompts.

use std::collections::BTreeMap;

use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum VarError {
    #[error("unfilled variable `{0}`")]
    Unfilled(String),
    #[error("malformed placeholder near byte {0}")]
    Malformed(usize),
}

/// A token resolver: returns the value for a key, or `None` to fall through.
pub trait VarSource {
    fn get(&self, key: &str) -> Option<String>;
}

impl VarSource for BTreeMap<String, String> {
    fn get(&self, key: &str) -> Option<String> {
        BTreeMap::get(self, key).cloned()
    }
}

/// Returns the distinct `{token}` keys appearing in `text`, in first-seen
/// order. `{{`/`}}` are literal braces and yield no tokens.
#[must_use]
pub fn detect_tokens(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut chars = text.char_indices().peekable();
    while let Some((_, c)) = chars.next() {
        match c {
            '{' => {
                if matches!(chars.peek(), Some((_, '{'))) {
                    chars.next();
                    continue;
                }
                let mut key = String::new();
                for (_, k) in chars.by_ref() {
                    if k == '}' {
                        break;
                    }
                    key.push(k);
                }
                if !key.is_empty() && !out.contains(&key) {
                    out.push(key);
                }
            }
            '}' => {
                if matches!(chars.peek(), Some((_, '}'))) {
                    chars.next();
                }
            }
            _ => {}
        }
    }
    out
}

/// Substitutes every `{token}` in `text` using `sources` in order (first hit
/// wins). `{{`/`}}` collapse to literal braces. Errors on the first token no
/// source can fill.
pub fn substitute(text: &str, sources: &[&dyn VarSource]) -> Result<String, VarError> {
    let mut out = String::with_capacity(text.len());
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i] as char;
        match c {
            '{' if bytes.get(i + 1) == Some(&b'{') => {
                out.push('{');
                i += 2;
            }
            '}' if bytes.get(i + 1) == Some(&b'}') => {
                out.push('}');
                i += 2;
            }
            '{' => {
                let start = i + 1;
                let end = text[start..].find('}').map(|o| start + o).ok_or(VarError::Malformed(i))?;
                let key = &text[start..end];
                let val = sources.iter().find_map(|s| s.get(key)).ok_or_else(|| VarError::Unfilled(key.to_string()))?;
                out.push_str(&val);
                i = end + 1;
            }
            _ => {
                out.push(c);
                i += 1;
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    #[test]
    fn detects_tokens_in_order_without_duplicates() {
        assert_eq!(detect_tokens("a {species} {x} {species}"), vec!["species", "x"]);
    }

    #[test]
    fn detects_no_tokens_in_escaped_braces() {
        assert!(detect_tokens("{{not a token}}").is_empty());
    }

    #[test]
    fn substitutes_from_first_source() {
        let primary = map(&[("species", "orc")]);
        let fallback = map(&[("species", "human")]);
        let out = substitute("a {species}", &[&primary, &fallback]).unwrap();
        assert_eq!(out, "a orc");
    }

    #[test]
    fn falls_through_to_second_source() {
        let primary = map(&[]);
        let fallback = map(&[("species", "human")]);
        assert_eq!(substitute("a {species}", &[&primary, &fallback]).unwrap(), "a human");
    }

    #[test]
    fn errors_on_unfilled() {
        let empty = map(&[]);
        assert_eq!(substitute("a {x}", &[&empty]), Err(VarError::Unfilled("x".into())));
    }

    #[test]
    fn keeps_literal_braces() {
        let empty = map(&[]);
        assert_eq!(substitute("{{x}}", &[&empty]).unwrap(), "{x}");
    }

    #[test]
    fn errors_on_unterminated() {
        let empty = map(&[]);
        assert!(matches!(substitute("a {x", &[&empty]), Err(VarError::Malformed(_))));
    }
}
```

- [ ] **Step 3: Run test to verify it fails, then passes**

Run: `cargo nextest run -p pixhaus-ai compose::variables`
Expected: FAIL first (module not wired) → PASS after Step 1-2 are in place.

- [ ] **Step 4: Add a proptest for substitution stability**

Append to the test module:

```rust
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn plain_text_without_braces_is_unchanged(s in "[a-zA-Z0-9 ,.]{0,64}") {
            let empty = map(&[]);
            prop_assert_eq!(substitute(&s, &[&empty]).unwrap(), s);
        }
    }
```

Run: `cargo nextest run -p pixhaus-ai compose::variables`
Expected: PASS. (Confirm `proptest` is a dev-dependency of `ai` with `rg "proptest" ai/Cargo.toml`; add under `[dev-dependencies]` if missing.)

- [ ] **Step 5: Commit**

```bash
git add ai/src/lib.rs ai/src/compose
git commit -m "$(printf 'feat(ai): add prompt variable substitution\n\nToken detection and {key} substitution with source precedence and\n{{/}} escaping. Per spec section 7.\n\nCo-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>')"
```

---

## Task 7: The compose resolver

**Files:**
- Replace: `ai/src/compose/mod.rs`

- [ ] **Step 1: Write the failing test**

Replace `ai/src/compose/mod.rs` with:

```rust
//! Composition resolver. See docs/planning/work/prompt-style-structure-library.md section 6.

pub mod builtins;
pub mod variables;

use std::collections::BTreeMap;

use pixhaus_core::project::library::composition::{
    PanelSlot, PromptTemplate, Structure, StructureOutput, Style,
};
use pixhaus_core::project::{Rect, SheetComposition, SheetPanel};
use thiserror::Error;

use self::variables::{substitute, VarError, VarSource};

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ComposeError {
    #[error("variable: {0}")]
    Variable(#[from] VarError),
    #[error("paneled structure `{0}` has no panels")]
    EmptyPaneledStructure(String),
}

/// Canvas size returned alongside a composed prompt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Canvas {
    pub width: u32,
    pub height: u32,
}

pub struct ComposeRequest<'a> {
    pub baseline: &'a str,
    pub structure: &'a Structure,
    pub style: Option<&'a Style>,
    pub prompt: Option<&'a PromptTemplate>,
    pub variable_values: &'a BTreeMap<String, String>,
    pub entity_info: &'a BTreeMap<String, String>,
    pub inline_text: &'a str,
    pub inline_negatives: &'a str,
    pub operation_hint: Option<&'a str>,
    pub context_fragments: &'a [String],
}

pub struct ComposedPrompt {
    pub positive: String,
    pub negative: String,
    pub composition: SheetComposition,
    pub canvas: Option<Canvas>,
}

/// Joins non-empty, trimmed segments with `sep`.
fn join_nonempty(sep: &str, segments: &[String]) -> String {
    segments
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(sep)
}

fn structure_prose(structure: &Structure) -> Result<(Vec<String>, Option<Canvas>), ComposeError> {
    match &structure.output {
        StructureOutput::Single => Ok((Vec::new(), None)),
        StructureOutput::Paneled { canvas, panels } => {
            if panels.is_empty() {
                return Err(ComposeError::EmptyPaneledStructure(structure.id.0.clone()));
            }
            let mut prose = Vec::with_capacity(panels.len());
            for p in panels {
                let frag = p
                    .prose_fragment
                    .replace("{canvas_w}", &canvas.width.to_string())
                    .replace("{canvas_h}", &canvas.height.to_string())
                    .replace("{panel_w}", &p.rect.w.to_string())
                    .replace("{panel_h}", &p.rect.h.to_string())
                    .replace("{label}", &p.label);
                prose.push(frag);
            }
            Ok((prose, Some(Canvas { width: canvas.width, height: canvas.height })))
        }
    }
}

fn build_composition(structure: &Structure) -> SheetComposition {
    let StructureOutput::Paneled { panels, .. } = &structure.output else {
        return SheetComposition::default();
    };
    let mut comp = SheetComposition::default();
    for p in panels {
        let rect = Rect::from_xywh(p.rect.x as i32, p.rect.y as i32, p.rect.w, p.rect.h);
        let panel = SheetPanel { region: rect, label: p.label.clone() };
        match p.slot {
            PanelSlot::View | PanelSlot::Generic => comp.views.push(panel),
            PanelSlot::Expression => comp.expressions.push(panel),
            PanelSlot::Callout => comp.callouts.push(panel),
            PanelSlot::Outfit => comp.outfits.push(panel),
            PanelSlot::PaletteSwatch => comp.palette_swatch = Some(rect),
        }
    }
    comp
}

pub fn compose(req: &ComposeRequest) -> Result<ComposedPrompt, ComposeError> {
    // Resolve prompt text with variable substitution (explicit -> entity info -> defaults).
    let prompt_text = match req.prompt {
        Some(p) => {
            let defaults: BTreeMap<String, String> = p
                .variables
                .iter()
                .filter(|v| !v.default.is_empty())
                .map(|v| (v.key.clone(), v.default.clone()))
                .collect();
            let sources: [&dyn VarSource; 3] = [req.variable_values, req.entity_info, &defaults];
            substitute(&p.text, &sources)?
        }
        None => String::new(),
    };

    let (layout_prose, canvas) = structure_prose(req.structure)?;
    let layout_joined = join_nonempty(". ", &layout_prose);

    let positive = join_nonempty(
        ". ",
        &[
            req.baseline.to_string(),
            req.style.map(|s| s.modifiers.clone()).unwrap_or_default(),
            layout_joined,
            prompt_text,
            join_nonempty(". ", req.context_fragments),
            req.operation_hint.unwrap_or("").to_string(),
            req.inline_text.to_string(),
        ],
    );

    let negative = join_nonempty(
        ", ",
        &[
            req.structure.layout_negatives.clone(),
            req.style.map(|s| s.look_negatives.clone()).unwrap_or_default(),
            req.inline_negatives.to_string(),
        ],
    );

    Ok(ComposedPrompt {
        positive,
        negative,
        composition: build_composition(req.structure),
        canvas,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pixhaus_core::project::library::composition::{
        Dimensions, PanelRect, StructureId, StructurePanel, StyleId,
    };

    fn paneled() -> Structure {
        Structure {
            id: StructureId("test.character".into()),
            name: "Character".into(),
            output: StructureOutput::Paneled {
                canvas: Dimensions { width: 1024, height: 480 },
                panels: vec![StructurePanel {
                    label: "front".into(),
                    rect: PanelRect { x: 0, y: 0, w: 200, h: 480 },
                    prose_fragment: "front view, {panel_w} by {panel_h}".into(),
                    slot: PanelSlot::View,
                }],
            },
            layout_negatives: "overlapping views".into(),
        }
    }

    fn empty_vars() -> BTreeMap<String, String> {
        BTreeMap::new()
    }

    #[test]
    fn interpolates_panel_dims_into_prose() {
        let s = paneled();
        let req = ComposeRequest {
            baseline: "pixel art",
            structure: &s,
            style: None,
            prompt: None,
            variable_values: &empty_vars(),
            entity_info: &empty_vars(),
            inline_text: "",
            inline_negatives: "",
            operation_hint: None,
            context_fragments: &[],
        };
        let out = compose(&req).unwrap();
        assert!(out.positive.contains("front view, 200 by 480"));
        assert_eq!(out.canvas, Some(Canvas { width: 1024, height: 480 }));
    }

    #[test]
    fn negatives_merge_structure_and_style() {
        let s = paneled();
        let style = Style {
            id: StyleId("st".into()),
            name: "S".into(),
            modifiers: "16-bit".into(),
            look_negatives: "blurry".into(),
            model_pref: None,
            quality: None,
        };
        let req = ComposeRequest {
            baseline: "",
            structure: &s,
            style: Some(&style),
            prompt: None,
            variable_values: &empty_vars(),
            entity_info: &empty_vars(),
            inline_text: "",
            inline_negatives: "watermark",
            operation_hint: None,
            context_fragments: &[],
        };
        let out = compose(&req).unwrap();
        assert_eq!(out.negative, "overlapping views, blurry, watermark");
        assert!(out.positive.starts_with("16-bit"));
    }

    #[test]
    fn single_output_has_empty_composition_and_no_canvas() {
        let s = Structure {
            id: StructureId("s".into()),
            name: "S".into(),
            output: StructureOutput::Single,
            layout_negatives: String::new(),
        };
        let req = ComposeRequest {
            baseline: "base",
            structure: &s,
            style: None,
            prompt: None,
            variable_values: &empty_vars(),
            entity_info: &empty_vars(),
            inline_text: "a sword",
            inline_negatives: "",
            operation_hint: None,
            context_fragments: &[],
        };
        let out = compose(&req).unwrap();
        assert_eq!(out.positive, "base. a sword");
        assert!(out.composition.views.is_empty());
        assert!(out.canvas.is_none());
    }

    #[test]
    fn maps_panel_slots_to_composition_buckets() {
        let s = paneled();
        let out = build_composition(&s);
        assert_eq!(out.views.len(), 1);
        assert_eq!(out.views[0].label, "front");
    }

    #[test]
    fn empty_paneled_structure_errors() {
        let s = Structure {
            id: StructureId("bad".into()),
            name: "Bad".into(),
            output: StructureOutput::Paneled {
                canvas: Dimensions { width: 10, height: 10 },
                panels: vec![],
            },
            layout_negatives: String::new(),
        };
        let req = ComposeRequest {
            baseline: "",
            structure: &s,
            style: None,
            prompt: None,
            variable_values: &empty_vars(),
            entity_info: &empty_vars(),
            inline_text: "",
            inline_negatives: "",
            operation_hint: None,
            context_fragments: &[],
        };
        assert!(matches!(compose(&req), Err(ComposeError::EmptyPaneledStructure(_))));
    }
}
```

> `mod.rs` now declares `pub mod builtins;` (Task 8) and re-declares `pub mod variables;`. Create an empty `ai/src/compose/builtins.rs` stub (`// filled in Task 8`) so the crate compiles; the stub gets real content next task.

- [ ] **Step 2: Confirm core re-exports**

The test imports `pixhaus_core::project::library::composition::*` and `pixhaus_core::project::{Rect, SheetComposition, SheetPanel}`. Confirm `Rect`, `SheetComposition`, `SheetPanel` are re-exported at `pixhaus_core::project` with `rg "pub use.*SheetComposition" core/src`. If they live deeper (e.g. `project::reference_sheets`), adjust the `use` path. Confirm `SheetComposition: Default` — it is per spec; if not, construct it field-by-field instead of `::default()`.

- [ ] **Step 3: Run tests**

Run: `cargo nextest run -p pixhaus-ai compose::tests`
Expected: PASS.

- [ ] **Step 4: Add an insta snapshot of a full positive prompt**

```rust
    #[test]
    fn positive_prompt_snapshot() {
        let s = paneled();
        let req = ComposeRequest {
            baseline: "pixel art character model sheet",
            structure: &s,
            style: None,
            prompt: None,
            variable_values: &empty_vars(),
            entity_info: &empty_vars(),
            inline_text: "a blue wizard",
            inline_negatives: "",
            operation_hint: Some("Preserve the character identity."),
            context_fragments: &["Background must be flat magenta.".into()],
        };
        let out = compose(&req).unwrap();
        insta::assert_snapshot!(out.positive);
    }
```

Run: `cargo nextest run -p pixhaus-ai positive_prompt_snapshot` then `cargo insta accept` (confirm `insta` is a dev-dep with `rg "insta" ai/Cargo.toml`).
Expected: snapshot created and reviewed; ordering reads baseline → layout → inline → context → operation hint → inline text per spec §6.2.

- [ ] **Step 5: Commit**

```bash
git add ai/src/compose
git commit -m "$(printf 'feat(ai): add the composition resolver\n\nPure compose() turning baseline + Structure + Style + Prompt + inline\ninto positive/negative prompts and SheetComposition slice geometry.\nPer spec section 6.\n\nCo-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>')"
```

---

## Task 8: Built-in registry + migration equivalence

This is the correctness-critical task: the migrated built-ins must reproduce the pre-migration prompts byte-for-byte.

**Files:**
- Replace: `ai/src/compose/builtins.rs`
- Reference (do not delete yet): `ai/src/verbs/reference_sheet/templates.rs`

- [ ] **Step 1: Capture golden strings from the current templates**

Before writing built-ins, snapshot the existing output so migration can be checked against it. Add a temporary test in `ai/src/verbs/reference_sheet/templates.rs` test module:

```rust
    #[test]
    fn golden_character_prompt() {
        insta::assert_snapshot!(
            "golden_character_positive",
            CompositionTemplate::Character.build_prompt("GOLDEN_SUBJECT")
        );
        insta::assert_snapshot!(
            "golden_character_negative",
            CompositionTemplate::Character.build_negative_prompt()
        );
    }
```

Repeat for Item, Tileset, Custom. Run `cargo nextest run -p pixhaus-ai golden_` then `cargo insta accept`. These snapshots are the migration target. Commit them on their own so they are immutable references:

```bash
git add ai/src/verbs/reference_sheet/snapshots
git commit -m "$(printf 'test(ai): capture golden reference-sheet prompts pre-migration\n\nCo-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>')"
```

- [ ] **Step 2: Write the built-in registry**

Replace `ai/src/compose/builtins.rs`. Build each migrated Structure from the **exact panel rectangles** in `templates.rs::*_composition()` and **exact prose clauses** from `build_prompt()`, with literal pixel numbers replaced by `{panel_w}`/`{panel_h}`/`{canvas_w}` tokens. Negatives split per spec §8.1.

```rust
//! Built-in composition records. Source of truth migrated from the former
//! reference_sheet::templates module. Per spec section 8.

use std::collections::BTreeMap;

use pixhaus_core::project::library::composition::{
    Dimensions, PanelRect, PanelSlot, PromptTemplate, Structure, StructureId, StructureOutput,
    StructurePanel, Style, StyleId,
};

/// Default cascading baseline used when a project sets no style_notes.
pub const BUILTIN_DEFAULT_BASELINE: &str = "pixel art reference sheet";

pub const STYLE_DEFAULT_ID: &str = "pixhaus.builtin.style.default";

pub struct BuiltinLibrary {
    pub structures: BTreeMap<StructureId, Structure>,
    pub styles: BTreeMap<StyleId, Style>,
    pub prompts: BTreeMap<PromptId, PromptTemplate>,
}

impl BuiltinLibrary {
    #[must_use]
    pub fn load() -> Self {
        let mut structures = BTreeMap::new();
        for s in [character(), item(), tileset(), custom()] {
            structures.insert(s.id.clone(), s);
        }
        let mut styles = BTreeMap::new();
        let def = default_style();
        styles.insert(def.id.clone(), def);
        Self { structures, styles, prompts: BTreeMap::new() }
    }
}

fn panel(label: &str, x: u32, y: u32, w: u32, h: u32, slot: PanelSlot, prose: &str) -> StructurePanel {
    StructurePanel {
        label: label.into(),
        rect: PanelRect { x, y, w, h },
        prose_fragment: prose.into(),
        slot,
    }
}

fn character() -> Structure {
    // Geometry copied verbatim from templates.rs::character_composition().
    // Views: 200x480 at x=i*200, y=0. Expressions: 256x192 at y=480.
    // Palette swatch: 1024x128 at y=672. Callouts: 512x320 at y=800.
    // Outfit: 256x384 at y=1120.
    let mut panels = Vec::new();
    let views = ["front", "side-left", "three-quarter", "side-right", "back"];
    for (i, label) in views.iter().enumerate() {
        panels.push(panel(label, i as u32 * 200, 0, 200, 480, PanelSlot::View,
            "five turnaround views in a horizontal strip across the top, left-aligned starting at the left edge — front view, left side, three-quarter view, right side, back view, each {panel_w} pixels wide, {panel_h} pixels tall"));
    }
    // Only the first view carries the shared turnaround clause; the rest carry an empty
    // fragment so the prose is not repeated five times. (Compose joins non-empty only.)
    for p in panels.iter_mut().skip(1) {
        p.prose_fragment.clear();
    }
    let exprs = ["neutral", "happy", "angry"];
    for (i, label) in exprs.iter().enumerate() {
        let prose = if i == 0 {
            "three facial expression close-ups side by side, left-aligned starting at the left edge — neutral, happy, angry — each {panel_w} pixels wide, {panel_h} pixels tall"
        } else { "" };
        panels.push(panel(label, i as u32 * 256, 480, 256, 192, PanelSlot::Expression, prose));
    }
    panels.push(panel("palette", 0, 672, 1024, 128, PanelSlot::PaletteSwatch,
        "a horizontal palette swatch row showing all colours used, {panel_w} pixels wide, {panel_h} pixels tall"));
    for (i, label) in ["detail-1", "detail-2"].iter().enumerate() {
        let prose = if i == 0 { "two detail callout panels side by side, each {panel_w} pixels wide, {panel_h} pixels tall" } else { "" };
        panels.push(panel(label, i as u32 * 512, 800, 512, 320, PanelSlot::Callout, prose));
    }
    panels.push(panel("outfit-variant", 0, 1120, 256, 384, PanelSlot::Outfit,
        "one outfit-variant panel, {panel_w} pixels wide, {panel_h} pixels tall, showing an alternate outfit or colour scheme. White background, clean pixel-art lines, consistent scale across all views. Professional sprite sheet format"));
    Structure {
        id: StructureId("pixhaus.builtin.structure.character".into()),
        name: "Character".into(),
        output: StructureOutput::Paneled { canvas: Dimensions { width: 1024, height: 1536 }, panels },
        layout_negatives: "extra limbs, bad anatomy, duplicate characters, overlapping views, inconsistent scale".into(),
    }
}

fn item() -> Structure {
    let mut panels = Vec::new();
    let views = [("front", 0, 0), ("side-left", 512, 0), ("back", 0, 384), ("side-right", 512, 384)];
    for (i, (label, x, y)) in views.iter().enumerate() {
        let prose = if i == 0 { "2×2 grid of orthographic views — top-left is front face, top-right is left side, bottom-left is back face, bottom-right is right side, each {panel_w}×{panel_h}" } else { "" };
        panels.push(panel(label, *x, *y, 512, 384, PanelSlot::View, prose));
    }
    panels.push(panel("palette", 0, 768, 1024, 128, PanelSlot::PaletteSwatch, "a palette swatch row {panel_w}×{panel_h} pixels"));
    for (i, label) in ["detail-1", "detail-2"].iter().enumerate() {
        let prose = if i == 0 { "two detail callout panels {panel_w}×{panel_h} each. White background, consistent scale across all four views" } else { "" };
        panels.push(panel(label, i as u32 * 512, 896, 512, 128, PanelSlot::Callout, prose));
    }
    Structure {
        id: StructureId("pixhaus.builtin.structure.item".into()),
        name: "Item".into(),
        output: StructureOutput::Paneled { canvas: Dimensions { width: 1024, height: 1024 }, panels },
        layout_negatives: "floating elements, inconsistent scale across views".into(),
    }
}

fn tileset() -> Structure {
    let panels = vec![
        panel("tile-primitives", 0, 0, 1024, 256, PanelSlot::View,
            "top row shows the base tile primitives — flat tile, corner variants, edge variants, in a grid"),
        panel("transition-variants", 0, 256, 1024, 384, PanelSlot::View,
            "middle band: transition tile variants and edge blending rules"),
        panel("autotile-preview", 0, 640, 1024, 256, PanelSlot::View,
            "lower block: 3×3 autotile preview demonstrating the autotile rule set"),
        panel("palette", 0, 896, 1024, 128, PanelSlot::PaletteSwatch,
            "bottom strip: palette swatch. White background, grid-aligned, clean pixel art, consistent tile size throughout"),
    ];
    Structure {
        id: StructureId("pixhaus.builtin.structure.tileset".into()),
        name: "Tileset".into(),
        output: StructureOutput::Paneled { canvas: Dimensions { width: 1024, height: 1024 }, panels },
        layout_negatives: "non-grid-aligned tiles, broken patterns, inconsistent tile size".into(),
    }
}

fn custom() -> Structure {
    let panels = vec![
        panel("full-body", 0, 0, 1024, 896, PanelSlot::View,
            "full-body orthographic view centred in a {panel_w}×{panel_h} area"),
        panel("palette", 0, 896, 1024, 128, PanelSlot::PaletteSwatch,
            "palette swatch row at the bottom, {panel_w}×{panel_h} pixels. White background"),
    ];
    Structure {
        id: StructureId("pixhaus.builtin.structure.custom".into()),
        name: "Custom".into(),
        output: StructureOutput::Paneled { canvas: Dimensions { width: 1024, height: 1024 }, panels },
        layout_negatives: String::new(),
    }
}

fn default_style() -> Style {
    Style {
        id: StyleId(STYLE_DEFAULT_ID.into()),
        name: "Default".into(),
        modifiers: String::new(),
        look_negatives: "blurry, low quality, watermark, text label, logo, cropped, photo realistic, 3d render".into(),
        model_pref: None,
        quality: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_four_structures_and_default_style() {
        let lib = BuiltinLibrary::load();
        assert_eq!(lib.structures.len(), 4);
        assert!(lib.styles.contains_key(&StyleId(STYLE_DEFAULT_ID.into())));
    }

    #[test]
    fn character_geometry_matches_legacy() {
        let lib = BuiltinLibrary::load();
        let c = &lib.structures[&StructureId("pixhaus.builtin.structure.character".into())];
        let StructureOutput::Paneled { canvas, panels } = &c.output else { panic!() };
        assert_eq!(*canvas, Dimensions { width: 1024, height: 1536 });
        // 5 views + 3 expressions + 1 palette + 2 callouts + 1 outfit = 12 panels.
        assert_eq!(panels.len(), 12);
        let outfit = panels.iter().find(|p| p.slot == PanelSlot::Outfit).unwrap();
        assert_eq!((outfit.rect.x, outfit.rect.y, outfit.rect.w, outfit.rect.h), (0, 1120, 256, 384));
    }
}
```

> **Important:** the migration is output-neutral only if the joined prose reads like the legacy string. The legacy prompts are single run-on sentences. The plan above puts the whole turnaround clause on the first view panel and clears the rest, so `join_nonempty(". ", …)` reproduces the structure. Step 4 verifies this against the golden snapshots; if the joined text diverges, adjust the per-panel fragments (not the resolver) until the migration test passes. Add `use pixhaus_core::project::library::composition::PromptId;` to the imports (referenced by `BuiltinLibrary`).

- [ ] **Step 3: Run the registry unit tests**

Run: `cargo nextest run -p pixhaus-ai compose::builtins`
Expected: PASS.

- [ ] **Step 4: Write the migration-equivalence test**

In `ai/src/compose/builtins.rs` tests, reconstruct the composed positive/negative for the Character built-in and compare to the golden snapshots from Step 1:

```rust
    use crate::compose::{compose, ComposeRequest};
    use std::collections::BTreeMap;

    #[test]
    fn character_migration_reproduces_legacy_positive() {
        let lib = BuiltinLibrary::load();
        let s = &lib.structures[&StructureId("pixhaus.builtin.structure.character".into())];
        let empty = BTreeMap::new();
        let req = ComposeRequest {
            baseline: "pixel art character model sheet",
            structure: s,
            style: None,
            prompt: None,
            variable_values: &empty,
            entity_info: &empty,
            inline_text: "GOLDEN_SUBJECT",
            inline_negatives: "",
            operation_hint: None,
            context_fragments: &[],
        };
        let out = compose(&req).unwrap();
        // Compare against the golden snapshot captured in Step 1, allowing for
        // the documented reordering (subject now trails the layout prose).
        insta::assert_snapshot!("character_migrated_positive", out.positive);
    }
```

Run: `cargo nextest run -p pixhaus-ai character_migration` then review the snapshot against `golden_character_positive`. They will differ in word order (legacy embeds the subject up front: "pixel art character model sheet, {subject}. Layout: …"; the new resolver appends inline_text last). **This reordering is expected and approved** (spec §6.2). The migration test's job is to prove the *layout instructions and dimensions* are preserved, not the exact subject position. Assert the migrated prompt contains every dimension phrase from the golden ("each 200 pixels wide, 480 pixels tall", "1024 pixels wide, 128 pixels tall", etc.):

```rust
    #[test]
    fn character_migration_preserves_all_layout_phrases() {
        let lib = BuiltinLibrary::load();
        let s = &lib.structures[&StructureId("pixhaus.builtin.structure.character".into())];
        let empty = BTreeMap::new();
        let req = ComposeRequest {
            baseline: "", structure: s, style: None, prompt: None,
            variable_values: &empty, entity_info: &empty,
            inline_text: "", inline_negatives: "", operation_hint: None, context_fragments: &[],
        };
        let out = compose(&req).unwrap();
        for phrase in [
            "each 200 pixels wide, 480 pixels tall",
            "each 256 pixels wide, 192 pixels tall",
            "1024 pixels wide, 128 pixels tall",
            "each 512 pixels wide, 320 pixels tall",
            "256 pixels wide, 384 pixels tall",
            "Professional sprite sheet format",
        ] {
            assert!(out.positive.contains(phrase), "missing: {phrase}");
        }
    }
```

Repeat the phrase-coverage test for Item, Tileset, Custom (each asserting its own dimension phrases and the canvas size). Add a negatives test:

```rust
    #[test]
    fn character_negatives_combine_with_default_style() {
        let lib = BuiltinLibrary::load();
        let s = &lib.structures[&StructureId("pixhaus.builtin.structure.character".into())];
        let style = &lib.styles[&StyleId(STYLE_DEFAULT_ID.into())];
        let empty = BTreeMap::new();
        let req = ComposeRequest {
            baseline: "", structure: s, style: Some(style), prompt: None,
            variable_values: &empty, entity_info: &empty,
            inline_text: "", inline_negatives: "", operation_hint: None, context_fragments: &[],
        };
        let out = compose(&req).unwrap();
        // Legacy character negative, recombined from style.look_negatives + structure.layout_negatives.
        assert!(out.negative.contains("blurry, low quality, watermark"));
        assert!(out.negative.contains("overlapping views, inconsistent scale"));
    }
```

Run: `cargo nextest run -p pixhaus-ai compose::builtins`
Expected: PASS.

- [ ] **Step 5: Remove the temporary golden test** added to `templates.rs` in Step 1 (the migration tests now own coverage). Keep the committed golden snapshots as historical reference, or delete them — either is fine; if deleting, also delete the snapshot files.

- [ ] **Step 6: Commit**

```bash
git add ai/src/compose/builtins.rs ai/src/compose/snapshots ai/src/verbs/reference_sheet
git commit -m "$(printf 'feat(ai): migrate reference-sheet templates to built-in records\n\nFour built-in Structures + a Default Style reproduce the legacy layout\nprose, dimensions, and negatives. Migration-equivalence tests assert\nevery dimension phrase is preserved. Per spec section 8.\n\nCo-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>')"
```

---

## Task 9: VerbContext carries the composition library

**Files:**
- Modify: `ai/src/plugin/context.rs` (confirm path: `rg "pub struct VerbContext" ai/src`)

- [ ] **Step 1: Write the failing test**

Add a borrowed view type and field. First the test (in the context module):

```rust
#[cfg(test)]
mod composition_library_tests {
    use super::*;

    #[test]
    fn library_view_resolves_project_over_builtin() {
        use pixhaus_core::project::library::composition::{Structure, StructureId, StructureOutput};
        let builtins = crate::compose::builtins::BuiltinLibrary::load();
        let project_struct = Structure {
            id: StructureId("pixhaus.builtin.structure.character".into()),
            name: "Shadowed".into(),
            output: StructureOutput::Single,
            layout_negatives: String::new(),
        };
        let project_structs = vec![project_struct];
        let view = CompositionLibraryView::new(&project_structs, &[], &[], &builtins);
        let resolved = view.structure(&StructureId("pixhaus.builtin.structure.character".into())).unwrap();
        assert_eq!(resolved.name, "Shadowed", "project record shadows built-in");
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo nextest run -p pixhaus-ai composition_library_tests`
Expected: FAIL — `CompositionLibraryView` not defined.

- [ ] **Step 3: Implement the view and add it to `VerbContext`**

```rust
use pixhaus_core::project::library::composition::{
    PromptId, PromptTemplate, Structure, StructureId, Style, StyleId,
};
use crate::compose::builtins::BuiltinLibrary;

/// Borrowed, read-only resolution view over project-tier records layered on
/// the built-in registry. Project records shadow built-ins by id.
pub struct CompositionLibraryView<'a> {
    structures: &'a [Structure],
    styles: &'a [Style],
    prompts: &'a [PromptTemplate],
    builtins: &'a BuiltinLibrary,
}

impl<'a> CompositionLibraryView<'a> {
    #[must_use]
    pub fn new(
        structures: &'a [Structure],
        styles: &'a [Style],
        prompts: &'a [PromptTemplate],
        builtins: &'a BuiltinLibrary,
    ) -> Self {
        Self { structures, styles, prompts, builtins }
    }

    #[must_use]
    pub fn structure(&self, id: &StructureId) -> Option<&Structure> {
        self.structures.iter().find(|s| &s.id == id).or_else(|| self.builtins.structures.get(id))
    }

    #[must_use]
    pub fn style(&self, id: &StyleId) -> Option<&Style> {
        self.styles.iter().find(|s| &s.id == id).or_else(|| self.builtins.styles.get(id))
    }

    #[must_use]
    pub fn prompt(&self, id: &PromptId) -> Option<&PromptTemplate> {
        self.prompts.iter().find(|p| &p.id == id).or_else(|| self.builtins.prompts.get(id))
    }
}
```

Add a field to `VerbContext` (match the struct's existing lifetime/ownership style — if `VerbContext` owns its data, store owned `Vec`s and a `BuiltinLibrary` and build the view on demand via a `library_view(&self)` method instead of storing a borrow). Confirm the struct shape first; the spec calls this field `composition_library`.

- [ ] **Step 4: Run to verify it passes**

Run: `cargo nextest run -p pixhaus-ai composition_library_tests`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add ai/src/plugin/context.rs
git commit -m "$(printf 'feat(ai): expose composition library on VerbContext\n\nCompositionLibraryView resolves project records over built-ins by id.\nPer spec section 9.\n\nCo-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>')"
```

---

## Task 10: Rewire the generate-reference-sheet verb

**Files:**
- Modify: `ai/src/verbs/reference_sheet/mod.rs` (inputs at lines 52-93; build/dispatch logic; JSON schema at ~150)
- Delete: `ai/src/verbs/reference_sheet/templates.rs`

- [ ] **Step 1: Update the input struct (test first)**

Add a test asserting the new input deserializes:

```rust
    #[test]
    fn new_inputs_deserialize_with_ids() {
        let json = r#"{
            "entity_id": 1,
            "structure_id": "pixhaus.builtin.structure.character",
            "inline_text": "a blue wizard",
            "num_variants": 2
        }"#;
        let inputs: GenerateReferenceSheetInputs = serde_json::from_str(json).unwrap();
        assert_eq!(inputs.structure_id.0, "pixhaus.builtin.structure.character");
        assert_eq!(inputs.inline_text, "a blue wizard");
        assert!(inputs.style_id.is_none());
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo nextest run -p pixhaus-ai new_inputs_deserialize_with_ids`
Expected: FAIL.

- [ ] **Step 3: Replace the input struct**

Replace `GenerateReferenceSheetInputs` (lines 52-84) with the spec §9 shape:

```rust
use pixhaus_core::project::library::composition::{PromptId, StructureId, StyleId};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GenerateReferenceSheetInputs {
    pub entity_id: EntityId,
    pub structure_id: StructureId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style_id: Option<StyleId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_id: Option<PromptId>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub variable_values: BTreeMap<String, String>,
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

Delete the `template: CompositionTemplate` field and the `prompt`/`negative_prompt` fields.

- [ ] **Step 4: Replace the prompt-building call site**

Find where the verb previously called `inputs.template.build_prompt(&inputs.prompt)` /
`build_negative_prompt` / `inputs.template.composition()` (the helpers at mod.rs lines 395-410 and the `invoke` body). Replace with:

```rust
let lib = context.library_view(); // or context.composition_library, per Task 9
let structure = lib.structure(&inputs.structure_id)
    .ok_or_else(|| VerbError::invalid_input(format!("unknown structure {}", inputs.structure_id.0)))?;
let style = inputs.style_id.as_ref().and_then(|id| lib.style(id));
let prompt = inputs.prompt_id.as_ref().and_then(|id| lib.prompt(id));

let baseline = context.project_style_notes_or_default(); // helper reading style_notes; falls back to BUILTIN_DEFAULT_BASELINE
let req = crate::compose::ComposeRequest {
    baseline,
    structure,
    style,
    prompt,
    variable_values: &inputs.variable_values,
    entity_info: context.entity_info(inputs.entity_id), // existing entity info map
    inline_text: &inputs.inline_text,
    inline_negatives: &inputs.inline_negatives,
    operation_hint: None, // fresh generation; app-side ops set this for refinements
    context_fragments: &[], // app composes these; the verb receives them via inputs if needed
};
let composed = crate::compose::compose(&req).map_err(|e| VerbError::invalid_input(e.to_string()))?;
```

Use `composed.positive`, `composed.negative` for the backend request and `composed.composition` for each `SheetVariantOutput.composition` (replacing `inputs.template.composition()`).

> The legacy code applied background/reference/LoRA fragments at the **app** layer (`compose_sheet_prompt`), not the verb. Keep that boundary: the verb composes the structure/style/prompt; the app passes any extra `context_fragments` and `operation_hint` through new optional input fields if a refinement needs them. For fresh generation they are empty. Confirm `VerbError::invalid_input`, `context.entity_info`, and a style-notes accessor exist; if not, add minimal helpers (entity info is already used by the legacy verb — `rg "entity_info\|info" ai/src/verbs/reference_sheet`).

- [ ] **Step 5: Update the JSON input schema**

Replace the `template` schema property (mod.rs ~158) with the id fields:

```rust
"structure_id": { "type": "string", "description": "Composition Structure id" },
"style_id": { "type": "string" },
"prompt_id": { "type": "string" },
"inline_text": { "type": "string" },
"inline_negatives": { "type": "string" },
"variable_values": { "type": "object", "additionalProperties": { "type": "string" } },
```

Update `required` to `["entity_id", "structure_id"]`.

- [ ] **Step 6: Delete templates.rs and its module declaration**

```bash
git rm ai/src/verbs/reference_sheet/templates.rs
```

Remove `mod templates;` / `pub use templates::*;` from `ai/src/verbs/reference_sheet/mod.rs`. Move any still-needed geometry assertions into the Task 8 migration tests (they already assert geometry).

- [ ] **Step 7: Build and run the verb tests**

Run: `cargo nextest run -p pixhaus-ai reference_sheet`
Expected: PASS. Fix any remaining references to `CompositionTemplate` across the crate (`rg "CompositionTemplate" ai/src` must return nothing).

- [ ] **Step 8: Commit**

```bash
git add ai/src/verbs/reference_sheet
git commit -m "$(printf 'feat(ai): drive reference-sheet generation through the resolver\n\nVerb inputs switch from a fixed template enum to structure/style/prompt\nids plus inline text; templates.rs is deleted. Per spec section 9.\n\nCo-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>')"
```

---

## Task 11: The `.pixstyle` bundle

**Files:**
- Create: `io/src/pixstyle.rs`
- Modify: `io/src/lib.rs` (`pub mod pixstyle;`)

- [ ] **Step 1: Write the failing test**

```rust
//! Portable export/import bundle for composition records. Same MessagePack +
//! zstd stack as the .pixhaus project file. Per spec section 10.2.

use std::io::{Read, Write};

use pixhaus_core::project::library::composition::{PromptTemplate, Structure, Style};
use serde::{Deserialize, Serialize};
use thiserror::Error;

const PIXSTYLE_MAGIC: &[u8; 4] = b"PXST";
const PIXSTYLE_FORMAT_VERSION: u16 = 1;

#[derive(Debug, Error)]
pub enum PixstyleError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("bad magic: not a .pixstyle bundle")]
    BadMagic,
    #[error("unsupported format version {0}")]
    UnsupportedVersion(u16),
    #[error("decode: {0}")]
    Decode(#[from] rmp_serde::decode::Error),
    #[error("encode: {0}")]
    Encode(#[from] rmp_serde::encode::Error),
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct StylePack {
    pub format_version: u16,
    pub structures: Vec<Structure>,
    pub styles: Vec<Style>,
    pub prompts: Vec<PromptTemplate>,
}

pub fn write_pack(pack: &StylePack, mut w: impl Write) -> Result<(), PixstyleError> {
    w.write_all(PIXSTYLE_MAGIC)?;
    w.write_all(&PIXSTYLE_FORMAT_VERSION.to_le_bytes())?;
    let body = rmp_serde::to_vec_named(pack)?;
    let compressed = zstd::encode_all(&body[..], 0)?;
    w.write_all(&compressed)?;
    Ok(())
}

pub fn read_pack(mut r: impl Read) -> Result<StylePack, PixstyleError> {
    let mut magic = [0u8; 4];
    r.read_exact(&mut magic)?;
    if &magic != PIXSTYLE_MAGIC {
        return Err(PixstyleError::BadMagic);
    }
    let mut ver = [0u8; 2];
    r.read_exact(&mut ver)?;
    let version = u16::from_le_bytes(ver);
    if version != PIXSTYLE_FORMAT_VERSION {
        return Err(PixstyleError::UnsupportedVersion(version));
    }
    let mut compressed = Vec::new();
    r.read_to_end(&mut compressed)?;
    let body = zstd::decode_all(&compressed[..])?;
    let pack: StylePack = rmp_serde::from_slice(&body)?;
    Ok(pack)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pixhaus_core::project::library::composition::{StructureId, StructureOutput};

    fn pack() -> StylePack {
        StylePack {
            format_version: PIXSTYLE_FORMAT_VERSION,
            structures: vec![Structure {
                id: StructureId("p.s".into()),
                name: "P".into(),
                output: StructureOutput::Single,
                layout_negatives: String::new(),
            }],
            styles: vec![],
            prompts: vec![],
        }
    }

    #[test]
    fn round_trips_through_bytes() {
        let mut buf = Vec::new();
        write_pack(&pack(), &mut buf).unwrap();
        let back = read_pack(&buf[..]).unwrap();
        assert_eq!(back, pack());
    }

    #[test]
    fn rejects_bad_magic() {
        let err = read_pack(&b"XXXX\x01\x00"[..]).unwrap_err();
        assert!(matches!(err, PixstyleError::BadMagic));
    }

    #[test]
    fn rejects_future_version() {
        let mut buf = Vec::new();
        buf.extend_from_slice(PIXSTYLE_MAGIC);
        buf.extend_from_slice(&99u16.to_le_bytes());
        buf.extend_from_slice(&zstd::encode_all(&rmp_serde::to_vec_named(&pack()).unwrap()[..], 0).unwrap());
        assert!(matches!(read_pack(&buf[..]).unwrap_err(), PixstyleError::UnsupportedVersion(99)));
    }
}
```

- [ ] **Step 2: Run to verify it fails, then passes**

Run: `cargo nextest run -p pixhaus-io pixstyle`
Expected: FAIL (module not wired) → add `pub mod pixstyle;` to `io/src/lib.rs` → PASS. Confirm `rmp-serde` and `zstd` are deps of `io` (`rg "rmp-serde\|zstd" io/Cargo.toml`); the project format already uses both, so they should be present.

- [ ] **Step 3: Commit**

```bash
git add io/src/pixstyle.rs io/src/lib.rs
git commit -m "$(printf 'feat(io): add .pixstyle export/import bundle\n\nMessagePack+zstd StylePack with magic + version header. Per spec 10.2.\n\nCo-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>')"
```

---

## Task 12: Copy-from-project helper

**Files:**
- Modify: `io/src/pixstyle.rs` (or a sibling — confirm where project loading lives with `rg "fn.*open.*project\|read_project" io/src`)

- [ ] **Step 1: Write the failing test**

Add to `io/src/pixstyle.rs`:

```rust
/// Reads only the composition library out of an existing project's ProjectAi.
/// The source project is opened read-only and left untouched.
pub fn read_library_from_project_ai(
    ai: &pixhaus_core::project::library::ProjectAi,
) -> StylePack {
    StylePack {
        format_version: PIXSTYLE_FORMAT_VERSION,
        structures: ai.structures.clone(),
        styles: ai.styles.clone(),
        prompts: ai.prompts.clone(),
    }
}
```

Test:

```rust
    #[test]
    fn extracts_library_from_project_ai() {
        use pixhaus_core::project::library::ProjectAi;
        let mut ai = ProjectAi::default();
        ai.styles.push(crate::pixstyle::tests::sample_style());
        let pack = read_library_from_project_ai(&ai);
        assert_eq!(pack.styles.len(), 1);
    }
```

Add a `sample_style()` helper to the test module. (The full "open another .pixhaus file" path reuses the existing project reader; this helper isolates the library-extraction logic so it is unit-testable without a file. The app command in Task 13 opens the file then calls this.)

- [ ] **Step 2: Run, confirm pass, commit**

Run: `cargo nextest run -p pixhaus-io pixstyle`
Expected: PASS.

```bash
git add io/src/pixstyle.rs
git commit -m "$(printf 'feat(io): extract composition library from a project for copy-from-project\n\nPer spec section 10.3.\n\nCo-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>')"
```

---

## Task 13: App IPC commands

**Files:**
- Create: `app/src/commands/library/composition.rs`
- Modify: `app/src/commands/library/mod.rs` (register), `app/src/lib.rs` or wherever `tauri::generate_handler!` lists commands

- [ ] **Step 1: Write the command module (with conflict policy)**

```rust
//! IPC commands for the composition library. Per spec section 11.

use pixhaus_core::project::library::composition::{
    PromptTemplate, Structure, StructureId, Style, PromptId, StyleId,
};
use pixhaus_io::pixstyle::{read_pack, write_pack, StylePack};
use serde::{Deserialize, Serialize};

use crate::state::AppState; // confirm with `rg "pub struct AppState" app/src`

#[derive(Serialize)]
pub struct CompositionLibrary {
    pub structures: Vec<Structure>,
    pub styles: Vec<Style>,
    pub prompts: Vec<PromptTemplate>,
    pub builtin_structures: Vec<Structure>,
    pub builtin_styles: Vec<Style>,
    pub builtin_prompts: Vec<PromptTemplate>,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictPolicy {
    Skip,
    Overwrite,
    ImportAsCopy,
}

#[tauri::command]
#[specta::specta]
pub async fn library_list_composition(state: tauri::State<'_, AppState>) -> Result<CompositionLibrary, String> {
    // Read project ProjectAi + the built-in registry, return both tiers.
    todo!("implement against AppState project access")
}
```

> The `todo!` here is a placeholder for the **AppState wiring**, which is repo-specific and must be read first. Replace each command body with real access to the loaded project's `ProjectAi` (the legacy reference-sheet commands already mutate `ProjectAi` — pattern-match their locking/borrowing in `reference_sheets.rs`). Do not leave `todo!` in committed code. Commands to implement, each `Result<_, String>`:
>
> - `library_list_composition` → both tiers (project vectors + `BuiltinLibrary::load()`).
> - `library_upsert_structure(structure)` / `_style` / `_prompt` → insert-or-replace by id in the project vectors; mark project dirty.
> - `library_delete_structure(id)` / `_style` / `_prompt` → remove by id (project tier only; reject built-in ids).
> - `library_fork_builtin { kind, builtin_id, as_new }` → clone the built-in record; if `as_new`, mint a fresh id (`format!("project.{kind}.{uuid}")`); else reuse the built-in id to shadow; push to project; return it.
> - `library_export_pack { selection, path }` → build a `StylePack` from selected ids, `write_pack` to a `File`.
> - `library_import_pack { path, policy }` → `read_pack`, merge by policy, return an import report `{ imported, skipped, overwritten }`.
> - `library_copy_from_project { source_path, selection }` → open the source project read-only, `read_library_from_project_ai`, merge selected by policy.
> - `library_resolve_prompt_variables { prompt_id, entity_id }` → resolve the prompt, run `detect_tokens`, return `[{ key, label, default, autofilled }]` where `autofilled` is the entity-info value if present.

- [ ] **Step 2: Write tests for the pure merge logic**

Extract the merge into a pure function so it is testable without Tauri:

```rust
pub fn merge_structures(target: &mut Vec<Structure>, incoming: Vec<Structure>, policy: &ConflictPolicy) -> (u32, u32, u32) {
    let (mut imported, mut skipped, mut overwritten) = (0, 0, 0);
    for s in incoming {
        match target.iter().position(|t| t.id == s.id) {
            Some(_) if matches!(policy, ConflictPolicy::Skip) => skipped += 1,
            Some(i) if matches!(policy, ConflictPolicy::Overwrite) => { target[i] = s; overwritten += 1; }
            Some(_) => { /* ImportAsCopy */ let mut c = s; c.id = StructureId(format!("{}.copy", c.id.0)); target.push(c); imported += 1; }
            None => { target.push(s); imported += 1; }
        }
    }
    (imported, skipped, overwritten)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pixhaus_core::project::library::composition::StructureOutput;

    fn s(id: &str) -> Structure {
        Structure { id: StructureId(id.into()), name: id.into(), output: StructureOutput::Single, layout_negatives: String::new() }
    }

    #[test]
    fn skip_policy_keeps_existing() {
        let mut t = vec![s("a")];
        let (i, sk, o) = merge_structures(&mut t, vec![s("a")], &ConflictPolicy::Skip);
        assert_eq!((i, sk, o), (0, 1, 0));
        assert_eq!(t.len(), 1);
    }

    #[test]
    fn overwrite_replaces() {
        let mut t = vec![s("a")];
        let mut incoming = s("a");
        incoming.name = "new".into();
        merge_structures(&mut t, vec![incoming], &ConflictPolicy::Overwrite);
        assert_eq!(t[0].name, "new");
    }

    #[test]
    fn import_as_copy_adds_suffixed_id() {
        let mut t = vec![s("a")];
        merge_structures(&mut t, vec![s("a")], &ConflictPolicy::ImportAsCopy);
        assert_eq!(t.len(), 2);
        assert!(t.iter().any(|x| x.id.0 == "a.copy"));
    }
}
```

Write equivalent `merge_styles`/`merge_prompts` (same shape) and at least the skip/overwrite/copy tests for one of them.

- [ ] **Step 3: Register commands**

Add the module to `app/src/commands/library/mod.rs` and every `#[tauri::command]` to the `tauri::generate_handler!` / `collect_commands!` list (confirm location with `rg "generate_handler!\|collect_commands!" app/src`). Regenerate tauri-specta bindings per the repo's build step.

- [ ] **Step 4: Build and test**

Run: `cargo nextest run -p pixhaus-app library` and `cargo build -p pixhaus-app`
Expected: PASS, no `todo!` remaining.

- [ ] **Step 5: Commit**

```bash
git add app/src/commands/library
git commit -m "$(printf 'feat(app): composition library IPC commands\n\nCRUD, fork, import/export, copy-from-project, and variable resolution\nwith tested merge policies. Per spec section 11.\n\nCo-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>')"
```

---

## Task 14: Shrink compose_sheet_prompt to an adapter

**Files:**
- Modify: `app/src/commands/library/reference_sheets.rs` (`compose_sheet_prompt` at ~813-873; `SheetProviderRequest` at ~560)

- [ ] **Step 1: Write the failing test**

Add a test asserting the adapter passes context fragments and operation hint through to `ai::compose`:

```rust
    #[test]
    fn adapter_includes_background_and_operation_hint() {
        // Build a SheetProviderRequest for a masked refinement and assert the
        // composed positive contains the background instruction and the
        // preservation hint, sourced from compose() not local string-building.
        let composed = compose_for_request(&sample_masked_request());
        assert!(composed.positive.contains("flat solid chroma key"));
        assert!(composed.positive.contains("Preserve everything outside the edited region"));
    }
```

Add `sample_masked_request()` to the test module mirroring the existing request fixtures.

- [ ] **Step 2: Run to verify it fails**

Run: `cargo nextest run -p pixhaus-app adapter_includes`
Expected: FAIL.

- [ ] **Step 3: Rewrite compose_sheet_prompt as an adapter**

Keep the fragment-building logic (background chroma, per-reference guidance, real-world grounding, LoRA trigger — lines ~815-856) but collect them into a `Vec<String>` `context_fragments` and select the `operation_hint` string (the operation match at ~857-870). Then call `ai::compose::compose()` with the project's resolved structure/style/prompt and return its `positive`/`negative`. Remove the local positive/negative string assembly. Expose a small pure `compose_for_request(req) -> ComposedPrompt` so the test above can call it without Tauri state.

```rust
fn compose_for_request(req: &SheetProviderRequest) -> pixhaus_ai::compose::ComposedPrompt {
    let mut fragments = Vec::new();
    if let Some(notes) = &req.style_notes_extra { fragments.push(format!("Project style notes: {notes}")); }
    fragments.push(format!(
        "Background must be a flat solid chroma key color {} with no shadows, gradients, or background props.",
        req.chroma_hex
    ));
    for (i, r) in req.references.iter().enumerate() {
        fragments.push(format!("Reference {} is {} guidance with weight {:.2}.", i + 1, r.role, r.weight));
    }
    if req.real_world_grounding {
        fragments.push("Use accurate real-world references for named places, objects, and scenes when composing this image.".into());
    }
    if let Some(lora) = &req.applied_lora {
        fragments.push(format!("Apply the Flux LoRA trigger word `{}` at weight {:.2}.", lora.trigger_word, lora.weight));
    }
    let operation_hint = match req.operation {
        OperationKind::MaskedRefinement | OperationKind::RegionalRefinement => Some("Preserve everything outside the edited region."),
        OperationKind::PromptOnlyRefinement | OperationKind::ChatTurn => Some("Preserve the character identity, proportions, palette, and sheet layout unless the user specifically asks to change them."),
        OperationKind::Promotion => Some("Re-render this approved direction as a polished final reference sheet."),
        _ => None,
    };
    let lib = req.library_view();
    let structure = lib.structure(&req.structure_id).expect("validated upstream");
    // ...resolve style/prompt, build ComposeRequest with fragments + operation_hint, call compose()
    pixhaus_ai::compose::compose(&/* ComposeRequest */).expect("validated upstream")
}
```

> Match the real field names on `SheetProviderRequest` (the snippet uses plausible names — confirm with `rg "struct SheetProviderRequest" app/src` and adjust). The operation-hint strings are copied verbatim from the legacy match so wording is unchanged. `expect` on a validated-upstream path is acceptable in `app` per conventions, but prefer mapping to `anyhow::Error` if the surrounding fn returns `Result`.

- [ ] **Step 4: Run to verify it passes**

Run: `cargo nextest run -p pixhaus-app reference_sheets`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add app/src/commands/library/reference_sheets.rs
git commit -m "$(printf 'refactor(app): compose_sheet_prompt delegates to the resolver\n\nThe app now only builds context fragments and the operation hint, then\ncalls ai::compose. Wording unchanged. Per spec section 11.\n\nCo-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>')"
```

---

## Task 15: TypeScript command bindings + state

**Files:**
- Modify: `ui/src/lib/commands/library.ts`, `ui/src/sheet/sheet-editor-state.ts`

- [ ] **Step 1: Confirm generated types**

After Task 13's tauri-specta regen, `Structure`, `Style`, `PromptTemplate`, etc. exist in the generated bindings. Confirm with `rg "export type Structure" ui/src`. If the repo commits generated bindings, regenerate and stage them.

- [ ] **Step 2: Add command wrappers (test first)**

In `ui/src/lib/commands/library.ts`, add typed wrappers calling the new IPC commands. Add a Vitest:

```ts
import { describe, it, expect, vi } from "vitest";
import { listComposition } from "./library";

describe("listComposition", () => {
  it("invokes the library_list_composition command", async () => {
    const invoke = vi.fn().mockResolvedValue({ structures: [], styles: [], prompts: [], builtin_structures: [], builtin_styles: [], builtin_prompts: [] });
    // inject the mock per the repo's invoke-wrapper test pattern (rg "mockIPC\|vi.mock" ui/src)
    const lib = await listComposition(invoke);
    expect(lib.structures).toEqual([]);
  });
});
```

Implement wrappers for: `listComposition`, `upsertStructure/Style/Prompt`, `deleteStructure/Style/Prompt`, `forkBuiltin`, `exportPack`, `importPack`, `copyFromProject`, `resolvePromptVariables`. Match the repo's existing `library.ts` invoke pattern exactly (`rg "invoke<" ui/src/lib/commands/library.ts`).

- [ ] **Step 3: Add editor-state signals**

In `sheet-editor-state.ts`, add signals: `selectedStructureId`, `selectedStyleId` (nullable), `selectedPromptId` (nullable), `variableValues` (record), `inlineText`, `inlineNegatives`. Replace the old `template` signal usage. Keep `prompt`/`refinePrompt` as the inline-text fields or migrate them to `inlineText` — confirm current usage (`rg "setTemplate\|template(" ui/src/sheet`).

- [ ] **Step 4: Run UI tests**

Run: `pnpm test -- library`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add ui/src/lib/commands/library.ts ui/src/sheet/sheet-editor-state.ts
git commit -m "$(printf 'feat(ui): composition library command bindings and editor state\n\nCo-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>')"
```

---

## Task 16: Library panel UI — Styles and Prompts tabs

**Files:**
- Create: `ui/src/sheet/library/LibraryPanel.tsx`, `StyleEditor.tsx`, `PromptEditor.tsx`, `pickers.tsx`

- [ ] **Step 1: Build the Style editor (test first)**

`StyleEditor.tsx` is a controlled form over a `Style`: name, modifiers (textarea), look_negatives (textarea), optional model/quality selects. Add a Solid Testing Library test asserting edits call `onChange` with the updated record. Follow the repo's component-test pattern (`rg "@solidjs/testing-library" ui/src`).

```tsx
// StyleEditor.tsx — controlled, no internal persistence.
import type { Style } from "../../lib/commands/library";
import { Component } from "solid-js";

export const StyleEditor: Component<{ value: Style; onChange: (s: Style) => void; readOnly?: boolean }> = (props) => {
  const set = <K extends keyof Style>(k: K, v: Style[K]) => props.onChange({ ...props.value, [k]: v });
  return (
    <div class="style-editor">
      <input value={props.value.name} disabled={props.readOnly} onInput={(e) => set("name", e.currentTarget.value)} />
      <textarea value={props.value.modifiers} disabled={props.readOnly} onInput={(e) => set("modifiers", e.currentTarget.value)} />
      <textarea value={props.value.look_negatives} disabled={props.readOnly} onInput={(e) => set("look_negatives", e.currentTarget.value)} />
    </div>
  );
};
```

- [ ] **Step 2: Build the Prompt editor with token highlighting**

`PromptEditor.tsx`: text area plus a derived variables table from a `detectTokens(text)` TS helper (port the Rust `detect_tokens` logic — same `{token}`/`{{`/`}}` rules). Test `detectTokens` directly:

```ts
import { detectTokens } from "./PromptEditor";
it("detects tokens without duplicates, ignoring escaped braces", () => {
  expect(detectTokens("a {x} {{y}} {x}")).toEqual(["x"]);
});
```

- [ ] **Step 3: Build `LibraryPanel.tsx` with three tabs**

Tab strip (Structures/Styles/Prompts), list of built-in (badged, read-only) + project records, action buttons (New, Edit, Duplicate, Fork, Delete, Import, Export, Copy from project) wired to the Task 15 command wrappers. Built-in rows pass `readOnly` to the editor and disable Delete.

- [ ] **Step 4: Build `pickers.tsx`**

`StructurePicker` (required), `StylePicker`/`PromptPicker` (nullable) — selects populated from `listComposition`. Emit the selected id to editor state.

- [ ] **Step 5: Run UI tests**

Run: `pnpm test -- library` and `pnpm test -- StyleEditor PromptEditor`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add ui/src/sheet/library
git commit -m "$(printf 'feat(ui): library panel with Styles and Prompts tabs\n\nCo-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>')"
```

---

## Task 17: Structure editor with live layout preview

**Files:**
- Create: `ui/src/sheet/library/StructureEditor.tsx`

- [ ] **Step 1: Build the panel-list editor (test first)**

`StructureEditor.tsx`: canvas width/height inputs, a `For` over panels (each row: label, slot select, x/y/w/h number inputs, prose_fragment textarea), add/remove-panel buttons, layout_negatives textarea. Output is a `Structure`. Test that editing a panel rect calls `onChange` with the new rect and that "add panel" appends a `Generic` panel.

- [ ] **Step 2: Add the live preview**

An SVG (or canvas) drawing each panel rect scaled to fit, labeled. Pure function `previewBoxes(structure, maxW, maxH) -> {x,y,w,h,label}[]` computing scaled rects; unit-test it:

```ts
it("scales panels to fit the preview box", () => {
  const boxes = previewBoxes(structureFixture(1024, 1536), 256, 384);
  expect(boxes[0].w).toBeCloseTo(256 * (200 / 1024));
});
```

- [ ] **Step 3: Run tests**

Run: `pnpm test -- StructureEditor`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add ui/src/sheet/library/StructureEditor.tsx
git commit -m "$(printf 'feat(ui): structure editor with scaled layout preview\n\nCo-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>')"
```

---

## Task 18: Wire pickers + variable panel into the generation form

**Files:**
- Modify: `ui/src/sheet/ReferenceSheetEditor.tsx` (args build ~713; prompt input ~1208)

- [ ] **Step 1: Replace the template control with pickers**

Swap the old template selector for `StructurePicker`/`StylePicker`/`PromptPicker`. Build `GenerateReferenceSheetArgs` from the selected ids + `inlineText` + `variableValues` instead of `template`/`prompt`.

- [ ] **Step 2: Add the variable panel (test first)**

When a Prompt is picked, call `resolvePromptVariables(promptId, entityId)` and render one field per returned variable, pre-filled with `autofilled ?? default`. On generate, send `variableValues`. Test that unfilled (no autofill, no default) variables block the generate button until filled.

- [ ] **Step 3: Run tests + typecheck**

Run: `pnpm test -- ReferenceSheetEditor` then `pnpm tsc --noEmit`
Expected: PASS, no type errors. Resolve any remaining references to the removed `template` field.

- [ ] **Step 4: Commit**

```bash
git add ui/src/sheet/ReferenceSheetEditor.tsx
git commit -m "$(printf 'feat(ui): pickers and variable panel in the generation form\n\nCo-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>')"
```

---

## Task 19: Full-stack verification and PR

- [ ] **Step 1: Workspace gates**

Run:
```bash
cargo fmt --all
cargo clippy --workspace --tests -- -D warnings
cargo nextest run --workspace
pnpm prettier --write ui/ && pnpm eslint ui/ && pnpm test && pnpm tsc --noEmit
./scripts/pre-pr.sh
```
Expected: all green. Confirm `rg "CompositionTemplate" .` returns nothing outside this plan/spec doc, and `rg "todo!" app/src/commands/library` returns nothing.

- [ ] **Step 2: Manual smoke (optional but recommended)**

Run `pnpm dev`, open a project, generate a reference sheet with a built-in Structure (confirm output matches pre-change behavior), fork it, edit a panel, save a Style, export a `.pixstyle`, re-import into a fresh project, and copy-from-project. Confirm the provenance view still shows the composed prompt.

- [ ] **Step 3: Open the PR**

```bash
git push -u origin feat/prompt-style-structure-library
gh pr create --base main --title "feat: user-managed prompt/style/structure library" --body "$(cat <<'EOF'
## What
Replaces the hardcoded reference-sheet templates with a two-tier (built-in + per-project) library of Structures, Styles, and Prompts, consumed by every AI verb through one composition resolver.

## Why
Hardcoded prompts and layouts removed artist control. This makes them editable data — author layouts, save reusable looks, edit the wording sent to backends, and carry it between projects.

## Test plan
- Unit/snapshot: composition resolver, variable substitution, migration equivalence (built-ins reproduce legacy layout phrases), .pixstyle round-trip, merge policies.
- Workspace: cargo nextest, clippy -D warnings, pnpm test, tsc.
- Manual: generate / fork / edit / export / import / copy-from-project.

Spec: docs/planning/work/prompt-style-structure-library.md
Plan: docs/planning/work/prompt-style-structure-library-plan.md
EOF
)"
```

> Implement this feature on its own branch `feat/prompt-style-structure-library` created from `main` (not the current `docs/` or `feat/sheet-candidate-strip-zoom` branch). Create it before Task 1.

---

## Self-review notes

- **Spec coverage:** §3 → Tasks 1-4; §4 tier resolution → Task 9; §5 baseline → Tasks 8 (const) + 10 (accessor); §6 resolver → Task 7; §7 variables → Task 6; §8 built-ins/migration → Task 8; §9 verb integration → Tasks 9-10; §10.1 ProjectAi → Task 5; §10.2 bundle → Task 11; §10.3 copy-from-project → Task 12; §11 IPC + adapter → Tasks 13-14; §12 UI → Tasks 15-18; §13 testing → distributed across every task; §14 backward compat → Tasks 5, 8, 10 tests. All sections covered.
- **Type consistency:** `StructureId`/`StyleId`/`PromptId` are tuple newtypes (`.0` for the string) throughout. `compose()` returns `ComposedPrompt { positive, negative, composition, canvas }`. `CompositionLibraryView` accessors are `structure`/`style`/`prompt`. `merge_structures`/`merge_styles`/`merge_prompts` share one signature shape. `BUILTIN_DEFAULT_BASELINE` and `STYLE_DEFAULT_ID` are the only magic consts.
- **Known repo-specific unknowns flagged for the implementer (verify with `rg` before the task):** `VerbContext` path/ownership (Task 9), `VerbError`/`entity_info`/style-notes accessor names (Task 10), `AppState` access + command registration macro (Task 13), `SheetProviderRequest` field names (Task 14), UI invoke-wrapper and component-test patterns (Tasks 15-18). These are deliberately not hardcoded because the plan cannot see those exact signatures; each is a 1-line `rg` lookup.
