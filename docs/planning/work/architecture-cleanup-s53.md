# Architecture cleanup — post-S53 audit

Status: planned, not yet executed. Open in the
`feat/s53-animated-sprite-sheet` worktree.

## Context

After landing S53 (animated sprite sheet from prompt) and the subsequent
`/simplify` pass on its diff, a wider architectural review was run against
the whole repo. Three Explore agents covered (a) verb architecture
consistency, (b) Rust dead / half-done code, and (c) TypeScript dead /
half-done code. The raw agent reports were dense; this brief keeps only
the verified findings and turns them into an ordered, executable plan.

Three commits land in sequence. Each is self-reviewable.

- Commit 1 — **fix:** the iterate-reference-sheet verb is implemented but
  never registered, so it cannot be invoked at runtime today.
- Commit 2 — **refactor:** four verbs open-code their own backend
  downcast chains where shared helpers should exist; one test-only
  function is incorrectly marked `pub(crate)` with `#[allow(dead_code)]`;
  one verb's ID drift is undocumented.
- Commit 3 — **chore:** two pre-existing hygiene items the previous
  `pre-pr` run surfaced (missing docs script aliases, missing typos
  allow-list entry).

CI gates (clippy `-D warnings`, nextest workspace-wide, pnpm test, tsc,
fmt-check) must remain green at every step.

---

## Commit 1 — `fix(verbs): register iterate-reference-sheet verb in AppState`

### What's wrong

`IterateReferenceSheetVerb` (B10.2, PR #165) is fully implemented under
`ai/src/verbs/iterate_reference_sheet/mod.rs`:

- Constant: `ITERATE_REFERENCE_SHEET_VERB_ID = "pixhaus.builtin.iterate_reference_sheet"` (line 43)
- Custom effect: `ITERATE_SHEET_EFFECT_NAME` (line 53)
- Input schema: `IterateReferenceSheetInputs` (line 59)
- Verb impl with lifecycle tests at line 615+

It is also `pub use`-d from `ai/src/verbs/mod.rs:63` and re-exported from
`ai/src/plugin/mod.rs`.

But `app/src/state.rs::AppState::new` (lines 303–319) does not call
`register_builtin(&runtime, IterateReferenceSheetVerb::new())`. A
project-wide search confirms zero references to
`ITERATE_REFERENCE_SHEET_VERB_ID` or `IterateReferenceSheetVerb` anywhere
in `app/src/` or `ui/src/`.

Effect: the verb is invisible to the running app. The reference-sheet UI
panel (B10.4) has no way to call the panel-scoped inpainting iteration
B10.2 was supposed to enable. The B10.2 PR description claims it landed,
but the wiring step was missed.

### Files to change

1. `app/src/state.rs:19–24` — extend the existing
   `use pixhaus_ai::verbs::{ ... }` block to include
   `IterateReferenceSheetVerb`. Keep alphabetical order: it goes between
   `InbetweenVerb` and `MotionFromVideoVerb`.

2. `app/src/state.rs:312` — add one line *between* the existing
   `register_builtin(&runtime, InbetweenVerb::new());` (line 312) and
   `register_builtin(&runtime, MotionFromVideoVerb::new());` (line 313):

   ```rust
   register_builtin(&runtime, IterateReferenceSheetVerb::new());
   ```

3. `app/src/state.rs:413` — extend the `expected` array in
   `new_registers_every_built_in_verb`. Insert
   `"pixhaus.builtin.iterate_reference_sheet"` in alphabetical position
   between the `inbetween` and `motion_from_video` rows.

### Verification

```bash
cargo nextest run -p pixhaus-app state::tests::new_registers_every_built_in_verb
cargo nextest run --workspace
```

The all-verbs test will fail until both the runtime registration and the
expected list are updated together — this is exactly what the test exists
to catch.

---

## Commit 2 — `refactor(ai): consolidate backend-downcast helpers + cleanup`

This commit groups four related changes. They touch the same file
(`ai/src/verbs/mod.rs`) plus the four verbs that consume from it, plus an
inline-into-test cleanup of a dead production function. Splitting further
would mean reviewing the same module-docs change twice; keeping them
together gives one clean refactor diff.

### 2a. Add three shared backend-call helpers

`ai/src/verbs/mod.rs` already provides:

- `call_text_vlm` — for `TextGenRequest`, downcasts Anthropic / OpenAI /
  `BackendProxy`
- `ctx_fat_backend` — resolves the operational backend without a request
  type, downcasts Anthropic / OpenAI / Replicate / Stability /
  `BackendProxy`

Four verbs still open-code their own downcast chains:

| Verb | File:line | Concrete adapters downcast |
|---|---|---|
| Sketch finishing | `ai/src/verbs/sketch_finishing/mod.rs:384–388` | Stability, OpenAI, Replicate |
| Continue (frame-interp) | `ai/src/verbs/continue_verb/mod.rs:415–435` | Replicate, Stability, OpenAI, Anthropic, Ollama, ComfyUi, plus its own `TestFrameBackend` (line 602) |
| Project style learning | `ai/src/verbs/project_style_learning/mod.rs:312` | Replicate-only |
| Train entity LoRA | `ai/src/verbs/train_entity_lora/mod.rs:311` | Replicate-only |

Adding a fifth concrete adapter forces edits at four sites. Add three
helpers next to `call_text_vlm`:

```rust
/// Sends an `ImageEditRequest` (used by Variant, Sketch finishing, and
/// any inpaint verb) through whichever concrete image-edit adapter is
/// attached. Tries Stability → OpenAI → Replicate, then `BackendProxy`.
pub(crate) async fn call_image_edit(
    backend: &dyn PluginBackend,
    request: ImageEditRequest,
    progress: VerbProgress,
    cancel: CancellationToken,
) -> Result<Vec<Vec<u8>>>;

/// Sends a `FrameInterpolationRequest` through whichever concrete
/// frame-interpolation adapter is attached. Used by Continue and
/// Inbetween. `BackendProxy` is the canonical test path; verbs should
/// stop ship­ping their own test stubs and use `BackendProxy::new(...)`.
pub(crate) async fn call_frame_interpolation(
    backend: &dyn PluginBackend,
    request: FrameInterpolationRequest,
    progress: VerbProgress,
    cancel: CancellationToken,
) -> Result<FrameInterpolationResponse>;

/// Sends a Replicate-style training request through the style-training
/// backend (Replicate today; LoRA training is currently single-backend).
pub(crate) async fn call_style_training(
    backend: &dyn PluginBackend,
    request: ReplicateRequest,
    progress: VerbProgress,
    cancel: CancellationToken,
) -> Result<TrainedModelRef>;
```

Match the shape and error mapping of `call_text_vlm` line-for-line — the
goal is consistency.

### 2b. Migrate the four affected verbs to the helpers

For each verb below, the change is *delete the inline downcast block,
replace with `crate::verbs::call_<helper>(backend, request, progress, cancel).await?`*.

**Sketch finishing** — `ai/src/verbs/sketch_finishing/mod.rs:376–408`
(the whole `call_image_edit` private fn). Delete the function. Update
the only call site (around line 243 inside `invoke`) to call
`crate::verbs::call_image_edit(...)` directly. Drop the now-unused
`StabilityBackend`, `OpenAiBackend`, `ReplicateBackend`, `BackendError`,
`InferenceBackend as BackendInvoker`, `InferenceResponse` imports at the
top of the file if they're no longer needed.

**Continue** — `ai/src/verbs/continue_verb/mod.rs:391–445` (the bespoke
`call_frame_interpolation` private fn). Replace with the shared helper.
The verb's own `TestFrameBackend` (line 602+) duplicates the
`BackendProxy` pattern; convert its tests to use
`BackendProxy::new(...)` and delete `TestFrameBackend`. This is the
biggest of the four migrations — budget extra time for it. The
inbetween-lifecycle test (`ai/tests/inbetween_lifecycle.rs`) is the
working reference for the `BackendProxy` test pattern.

**Project style learning** — `ai/src/verbs/project_style_learning/mod.rs:312`.
The downcast is single-target (Replicate) so the migration is a one-line
replacement.

**Train entity LoRA** — `ai/src/verbs/train_entity_lora/mod.rs:311`.
Same shape as project style learning; one-line replacement.

### 2c. Document the namespace drift in `ai/src/verbs/mod.rs`

The "Known drift" comment block at lines 25–28 currently only mentions
`SketchFinishingVerb`. Add a second entry:

```text
Known drift:
- [`SketchFinishingVerb`] advertises `pixhaus.ai.sketch_finishing` instead
  of the `pixhaus.builtin.*` convention.
- [`AutoMeshDeformationVerb`] advertises
  `pixhaus.builtin.auto-mesh-deformation` (kebab-case) instead of the
  snake_case convention every other built-in follows.

Both are public surface (logged, scriptable, baked into stored projects),
so neither is renamed. Future built-ins should use the
`pixhaus.builtin.<snake_case>` form.
```

Also extend the `# Shared helpers` section (lines 30–35) so it enumerates
all four helpers, not just `call_text_vlm`:

```text
# Shared helpers

Verbs that need an inference call go through one of four helpers that
each centralise the downcast from `Arc<dyn PluginBackend>` to a concrete
adapter:

- `call_text_vlm`             — `TextGenRequest`
- `call_image_edit`           — `ImageEditRequest`
- `call_frame_interpolation`  — `FrameInterpolationRequest`
- `call_style_training`       — `ReplicateRequest` (LoRA training)

Adding a new concrete adapter means updating the helper(s) that accept
it, not the verbs. `ctx_fat_backend` remains the no-request-type escape
hatch for verbs that need the fat trait without sending a specific
request (today: tileset-from-description).
```

### 2d. Delete `apply_generated_reference_sheet_payload` from production

`app/src/commands/library.rs:4635–4636` declares:

```rust
#[allow(dead_code)]
pub(crate) fn apply_generated_reference_sheet_payload(...)
```

Its sole caller is the test at line 6357 of the same file. Per user
decision: inline the helper into that test (its body is a thin wrapper
over field assignments — call it inline or extract to a `#[cfg(test)]`
helper inside the test mod) and delete the production function plus its
`#[allow(dead_code)]` annotation.

### Verification

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run --workspace
```

Specifically check:

- The existing lifecycle tests for the affected verbs still pass:
  - `ai/tests/inbetween_lifecycle.rs` (frame-interp pattern)
  - `ai/src/verbs/sketch_finishing/mod.rs` test module
  - `ai/src/verbs/project_style_learning/mod.rs` test module
  - `ai/src/verbs/train_entity_lora/mod.rs` test module
  - `ai/src/verbs/continue_verb/mod.rs` test module — pay extra attention
    here because `TestFrameBackend` is being deleted and its tests must
    be ported to `BackendProxy`.
- `cargo doc -D warnings` covers the doc-comment updates.

---

## Commit 3 — `chore: docs script aliases + typos allow-list`

### 3a. Add `docs:dev` and `docs:build` aliases

Root `package.json` lines 23–24 have `website:*` aliases but no parallel
`docs:*` aliases for the Starlight docs site at `docs/site/`. Two-line
addition:

```json
"docs:dev":   "pnpm --filter pixhaus-docs dev",
"docs:build": "pnpm --filter pixhaus-docs build",
```

Insert right after the `website:build` entry (line 24) so all per-site
scripts cluster.

### 3b. Allow-list `laf` in `_typos.toml`

The S53 pre-PR run flagged two pre-existing occurrences of `laf`
(Aseprite's UI submodule, a proper noun) in `LICENSES/NOTICE.txt:20`
and `LICENSES/aseprite-MIT.txt:25`. They predate this branch and were
left alone in S53. Add to the `[default.extend-words]` section of
`_typos.toml`:

```toml
# "laf" is the name of Aseprite's UI submodule (https://github.com/aseprite/laf),
# referenced in the attribution notes.
laf = "laf"
```

### Verification

```bash
pnpm docs:dev  # smoke test the alias
typos          # should now pass
./scripts/pre-pr.sh
```

---

## False positives — DO NOT change

The agent sweep surfaced several items that looked wrong but are
intentional. Listing them here so a future audit doesn't re-discover and
reverse them.

- **`continue_verb` and `extend` "missing anchor"**: both verbs document
  intentional skipping of `ctx.anchor` with explicit code comments
  (`ai/src/verbs/continue_verb/mod.rs:194`, `ai/src/verbs/extend.rs:331`).
  `FrameInterpolationRequest` has no style slot; `DirectionalViewRequest`
  is its own request type. Leave alone.
- **`reference_sheet` "missing anchor"**: this verb *generates* the
  anchor sheet; consuming an anchor while creating one would be circular.
  Leave alone.
- **`PreferencesModal` `providers` signal flagged as "never set"**:
  populated via an async `.then(setProviders)` chain in a `createEffect`.
  Working as intended.
- **`#[allow(dead_code)]` on `_ensure_actual_cost_is_visible`,
  `_imports_used`, `_ensure_imports_visible`**: these are intentional
  visibility-only references that keep types alive for downstream
  integration tests. Documented at their definition sites. Leave alone.
- **`FalQueueSubmitResponse` and `AnthropicRequest` with
  `#[allow(dead_code)]`**: forward-compat shapes for streaming flows not
  yet wired. Marked with rationale comments. Leave alone.

---

## Critical files

| Path | Touched in |
|---|---|
| `app/src/state.rs` | Commit 1 |
| `ai/src/verbs/mod.rs` | Commit 2 (helpers + drift docs) |
| `ai/src/verbs/sketch_finishing/mod.rs` | Commit 2 |
| `ai/src/verbs/continue_verb/mod.rs` | Commit 2 (largest migration) |
| `ai/src/verbs/project_style_learning/mod.rs` | Commit 2 |
| `ai/src/verbs/train_entity_lora/mod.rs` | Commit 2 |
| `app/src/commands/library.rs` | Commit 2 (inline + delete) |
| `package.json` (root) | Commit 3 |
| `_typos.toml` | Commit 3 |

---

## Full verification matrix (run before opening PR)

```bash
cargo fmt --all -- --check                     # formatting
cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run --workspace                  # 1762+ tests must pass
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --document-private-items
pnpm typecheck                                 # tsc across workspaces
pnpm test                                      # vitest, 265+ tests
pnpm exec tsc --noEmit                         # ui-side check
pnpm exec --filter @pixhaus/e2e-tests typecheck
typos                                          # should now pass with allow-list
./scripts/pre-pr.sh                            # umbrella gate
```

E2E (`tests/e2e/`, WebDriver-based) cannot run on macOS — tauri-driver
is Linux/Windows-only. Rely on CI for that pass.

Visual tests (`tests/visual/`, Playwright) are local-runnable; confirm
baselines unchanged after Commit 1 (it touches the verb registration but
no UI surface).

For Commit 2 specifically, the lifecycle-test green status is the
strongest signal that the helper refactor preserved behaviour. If any of
the four verbs' tests fail, the helper signature or downcast order is
likely off — compare to `call_text_vlm` for the canonical pattern.

---

## Out of scope (explicitly deferred)

The agent sweep also surfaced these. They're real but better addressed
when their parent feature lands, not as part of this cleanup:

- PSD blend-mode helper `blend_mode_from_psd_debug` is gated for the
  B9.5 PSD-import revival — keep until that ships.
- CSS `--color-*` token aliases possibly dead after S47 — verify against
  the website crate before pruning.
- `tests/visual/` baselines cover only `shell.spec.ts` and
  `canvas.spec.ts` — coverage expansion is a stream of its own, not a
  cleanup.
- Documentation-site (`docs/site/`) vs. marketing-site (`website/`)
  split is intentional; mention in `README.md` is nice-to-have, not
  blocking.
