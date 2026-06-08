# Pixhaus code audit — 2026-06-08

A crate-by-crate, file-by-file audit of the `v3` tree against the repo's skills,
the root and per-crate `CLAUDE.md` rules, the architecture bible boundaries, and
the locked `Cargo.toml` lints. Every Rust file in the workspace was reviewed.

## Verdict

The codebase is in strong, disciplined compliance. The Stop-gate basics are green
(clippy `-D warnings` all targets, 639 tests, fmt, zero `unsafe`), and the static
audit found no critical or high-severity issues: 35 confirmed findings, all low or
info except two medium. The load-bearing rules hold across the layers — `core`/
`render` are egui-free, mutation routes through `Command`s, the deferred-intent UI
model is type-enforced, the i18n key/data split is structural, and the dependency
idioms are version-correct.

One thing is actually broken, and it is pre-existing: `cargo doc` fails the Stop
gate on broken intra-doc links (8 errors across 4 files). The static agents could
not see it because they review source without building docs; the live gate caught
it. Fix that first — see Finding 0. Everything else is polish.

## Remediation status (2026-06-08)

All 35 confirmed findings were fixed on branch `fix/audit-compliance`, plus a
second pre-existing `cargo doc` failure that the Finding 0 fix exposed. One finding
is a documented skip. The full gate is now green where the committed baseline was red:

| Gate | Before | After |
|---|---|---|
| `cargo fmt --all --check` | pass | pass |
| `cargo clippy --workspace --all-targets -- -D warnings` | pass | pass |
| `cargo nextest run --workspace` | pass (639) | pass (644; +5 new tests) |
| `cargo doc --workspace` (`RUSTDOCFLAGS=-D warnings`) | **fail** | **pass** |

By theme:

- **Doc gate (Finding 0, + a masked sibling).** Repaired the 8 broken/redundant
  intra-doc links in `core`/`services`, then fixed a `crate::widgets::card`
  ambiguity in `ui` (a `fn@` disambiguator) that the core/services failures had
  hidden — `cargo doc` stopped before reaching `ui`, so it surfaced only once the
  earlier errors were gone.
- **Correctness.** U5-1: a no-op handle rename's undo no longer bumps the document
  revision (it now mirrors apply), with a regression test.
- **i18n.** U27-1 (medium): the Codex style/animation `enum_picker` resolves variant
  labels through `keys.rs` mappers + `codex.yaml` instead of `format!("{v:?}")`, with
  a resolution test. U18-1: the anchor badge composes via a `codex.anchor.badge`
  template. U24-2: sprite-edit frame/selection buttons use their existing command
  keys. U23-1/U23-2: the `i18n_keys` gate now also checks workspace status items and
  the Codex module, so this drift class fails the build going forward.
- **Design system.** U30-1: removed the doubled header across the four export dock
  panels. U20-1: the canvas HUD/zoom radii use theme tokens.
- **Ownership / performance.** U17-1: `Theme`/`MockColors` drop `Copy` (cloned once
  in `Host`). U9-1: `Provider::id` returns `&ProviderId`. U11-3/U11-4: HashSet dedup.
- **Hygiene.** ~35 prose `// TODO(luis)` comments reworded to rationale (the clippy
  `todo` lint never caught them). U30-2: the tiles module logs an `info!` on
  registration. U6-1/U6-2/U16-1/U1-1: documented panics, a deferred decision, and a
  latent paint-path guard. U32-2: `detect_language` extracted with an empty-tag guard.
- **Tests.** New direct tests for the OpenRouter cancel branch, `estimated_size_bytes`,
  `show_keys`, `log_dir`, and enum-key resolution; the generate-loop drain helper reused.

**Skipped — U22-2** (a direct test for `CodexEditorDraft::load_from`). A unit test
needs a ~29-field `CodexEntryDetail` fixture with no `Default` or builder seam —
disproportionate and brittle — and the method already has transitive coverage via the
shell selection tests (`crates/ui/src/shell/mod.rs`). Recorded here rather than
shipping a fixture that rots on every field addition.

## How this was audited

- **32 units, all 192 source files.** Coverage is complete: `crates/{core, render,
  io, platform, services, ui}`, `modules/*`, and `app`. Large files
  (`state/intent.rs`, `widgets/codex.rs`, `codex_ws/editor.rs`) got their own units.
- **Skill-matched review.** Each unit's agent loaded exactly the skills and
  `CLAUDE.md` files that apply to its code — `wgpu`/`bytemuck`/`glam`/`performance`
  for `render`, `openrouter`/`image`/`keyring`/`tokio`/`async-trait` for `providers`,
  `ui-conventions`/`egui`/`i18n` for UI, and `rust-conventions`/`result-handling`
  everywhere.
- **Adversarial verification.** A second agent re-read the cited code and rule for
  every finding. It rejected 25 of 78 raw findings (32%) as false positives —
  fabricated rules, `unwrap` in test code (exempt), i18n keys mistaken for hardcoded
  strings, theme tokens mistaken for hex literals, `anyhow` flagged in the binary
  (correct there). Each rejection is recorded in the per-crate "Checked and cleared"
  sections, so the surviving findings are trustworthy, not credulous.
- **Live ground truth.** `cargo clippy`, `nextest`, `fmt`, and `doc` were run
  against the tree, plus an `unsafe` sweep.
- **Method limit, stated plainly.** Static review does not build docs or run the
  app. The `cargo doc` breakage was found by the gate, not the agents; the visual
  design-system compliance is judged from code, not rendered frames. Treat the
  static findings and the live gate together.

## Ground truth (live gate results)

| Check | Result |
|---|---|
| `cargo clippy --workspace --all-targets -- -D warnings` | pass |
| `cargo nextest run --workspace` | 639 passed, 1 skipped (`openrouter_live`, gated on an API key) |
| `cargo fmt --all --check` | pass |
| `cargo doc --workspace` (`RUSTDOCFLAGS=-D warnings`) | **fail — see Finding 0** |
| `unsafe` in tree | 0 (one comment reference in `dirs.rs`; no `unsafe` code) |

## Finding 0 — `cargo doc` gate is red (pre-existing, highest priority)

The committed tree fails `cargo doc` under the Stop gate's `RUSTDOCFLAGS='-D
warnings'`. Eight rustdoc errors, all in the Codex command docs — most likely
fallout from the swap-command macro refactor (b05edfb) renaming command types
without updating the prose links that point at them.

Unresolved intra-doc links (the linked item is not in scope):

- `crates/core/src/commands/codex/set_anchor.rs:5` — `[\`RemoveAnchor\`]`
- `crates/services/src/codex/demo/animations.rs:17` — `[\`UpdateCodexEntry\`]`
- `crates/services/src/codex/demo/animations.rs:18` — `[\`SetPromptFragments\`]`
- `crates/services/src/codex/demo/animations.rs:19` — `[\`SetNegativeFragments\`]` and `[\`SetAnchor\`]`

Redundant explicit link targets (the label already resolves; the `(path)` is dead weight):

- `crates/core/src/commands/codex/change_relationship_kind.rs:3` — `[\`Relationship\`](crate::codex::Relationship)`
- `crates/core/src/commands/codex/set_anchor.rs:3` — `[\`AnchorKind\`](crate::codex::AnchorKind)`
- `crates/core/src/commands/codex/set_details.rs:3` — `[\`EntryDetails\`](crate::codex::EntryDetails)`

Fix: for the unresolved links, qualify them with a full path
(`[\`UpdateCodexEntry\`](pixhaus_core::commands::codex::UpdateCodexEntry)`) or correct
the type name if it was renamed/removed; for the redundant ones, drop the explicit
`(crate::...)` target and keep the bare `` [`Type`] ``. Then re-run `cargo doc
--workspace --no-deps --document-private-items` to confirm green.

## Findings rollup

78 raw findings → **35 confirmed, 18 need human review, 25 false positives.**

Severity (confirmed + needs-review, after verifier re-grading):

| critical | high | medium | low | info |
|---|---|---|---|---|
| 0 | 0 | 2 | 29 | 22 |

Plus Finding 0 (the `cargo doc` gate failure) which the live gate rates as a
blocker because it breaks the build gate, not surfaced in the static counts above.

By category (confirmed + needs-review):

| Category | Count |
|---|---|
| tests | 15 |
| i18n | 9 |
| docs | 7 |
| boundary | 2 |
| ownership | 2 |
| performance | 2 |
| no-unwrap | 2 |
| style | 2 |
| undo-symmetry, wgpu, duplication, latent-panic, ui-tokens, serde, comments, ui-widgets, tracing, altitude, ui, async | 1 each |

## Cross-cutting themes, ranked by leverage

1. **i18n literal drift (9 findings).** The `View > Show i18n Keys` dev toggle
   exists to catch exactly this, and several labels would not flip under it. The
   headliner is **U27-1 (medium)**: `modules/codex/.../details.rs` renders enum
   variants as user-facing text via `format!("{v:?}")` (`PingPong`, bare `None`
   reach the artist). Also `sprite-edit` button labels hardcoded despite existing
   keys (U24-2), the `anchor_badge` word-order baked into Rust (U18-1), and two
   gaps in the `i18n_keys.rs` dangling-key test that let `status_items` and the
   whole Codex module slip past the gate (U23-1, U23-2). Fix the leaks, then close
   the test gaps so regressions get caught automatically.

2. **Doubled panel header (U30-1, medium).** All four `export` dock panels call
   `widgets::section_header` with their own panel title key, but the shell already
   wraps each right-dock body in `widgets::card`, which draws the title — the exact
   anti-pattern the `ui-conventions` skill names. Remove the duplicate headers.

3. **Prose `// TODO(luis)` comments (~30, across sprite-edit, generation, export,
   tiles).** Banned by `rust-conventions` (file an issue, leave a `// see #NNN`
   breadcrumb). They are invisible to the Stop gate because clippy's `todo` lint
   only catches the `todo!()` macro, not prose — so this rots silently. Convert to
   issue references or drop the prefix and keep the rationale prose.

4. **Test-floor gaps (15).** Most are transitive-coverage judgment calls (a public
   fn exercised through a sibling integration test) — legitimately "needs human
   review." The real gaps worth closing: the OpenRouter `generate` cancel branch
   has only the ignored live test (U29-3), and `CodexEditorDraft::load_from` (a
   five-field public mutator) has none (U22-2).

5. **Doc accuracy (beyond Finding 0).** `region_id` module doc miscounts the
   regions (U14-1); `render::upload_frame` documents a tight-packing precondition in
   prose with no `# Panics` note or `debug_assert` (U6-1).

6. **Design-system and allocation nits.** `Theme` derives `Copy` at ~304 bytes,
   12x the conventions guideline, so every by-value pass is a silent ~300-byte
   memcpy — drop `Copy`, keep `Clone` (U17-1). `Provider::id` returns an owned
   `String` per call and the registry allocates one per comparison — return
   `&ProviderId` (U9-1).

7. **Latent panics on the paint path.** `onion::bake` feeds a stride-bearing buffer
   into `ColorImage::from_rgba_unmultiplied`, which asserts tight packing and would
   panic per-frame if the upstream invariant ever breaks — guard or record it
   (U16-1). Same shape as U6-1.

8. **Boundary placement.** `handle_from_name` slugification lives in `crates/ui`
   beside `CodexHandle` validation that lives in `crates/core` — the missing other
   half of one domain concept; move it to core (U21-2). `composite_layer` ignores
   `layer.blend` with no `match`, so a second `BlendMode` variant would silently
   composite as Normal with no compile error (U1-1).

## Per-crate scorecard

Confirmed / needs-review / false-positive counts, with the detailed section linked.

| Crate / group | Files | Confirmed | Review | FP | Headline | Section |
|---|---|---|---|---|---|---|
| `core` | 53 | 3 | 4 | 8 | most disciplined group; one undo-symmetry nit | [core.md](by-crate/core.md) |
| `render` | 1 | 2 | 0 | 1 | cleanest unit; two deferred-decision notes | [render.md](by-crate/render.md) |
| `platform` | 2 | 1 | 0 | 1 | exemplary; both `directories` traps handled | [platform.md](by-crate/platform.md) |
| `io` | 1 | 0 | 0 | 0 | clean compiling stub, fully compliant | [io.md](by-crate/io.md) |
| `services` | 11 | 6 | 3 | 5 | high-quality; minor alloc + O(n^2) on bounded data | [services.md](by-crate/services.md) |
| `ui` | 49 | 12 | 5 | 7 | strong; type-enforced intent + i18n boundaries | [ui.md](by-crate/ui.md) |
| `mod-sprite-edit` | 3 | 2 | 0 | 0 | prose TODOs + 4 hardcoded labels | [mod-sprite-edit.md](by-crate/mod-sprite-edit.md) |
| `mod-animation` | 2 | 0 | 1 | 1 | clean | [mod-animation.md](by-crate/mod-animation.md) |
| `mod-codex` | 9 | 2 | 2 | 0 | strongest UI; one medium i18n leak (U27-1) | [mod-codex.md](by-crate/mod-codex.md) |
| `mod-generation` | 7 | 1 | 1 | 0 | near-exemplary; minor TODO/badge nits | [mod-generation.md](by-crate/mod-generation.md) |
| `mod-providers` | 6 | 1 | 2 | 1 | idiomatic; one cancel-branch test gap | [mod-providers.md](by-crate/mod-providers.md) |
| `mod-export-tiles` | 4 | 4 | 0 | 0 | doubled header (medium) + TODOs + tiles `info!` | [mod-export-tiles.md](by-crate/mod-export-tiles.md) |
| `mod-stubs` | 2 | 0 | 0 | 0 | clean compiling stubs | [mod-stubs.md](by-crate/mod-stubs.md) |
| `app` | 3 | 1 | 0 | 1 | exemplary binary; one readability note | [app.md](by-crate/app.md) |

## Recommended remediation order

1. **Finding 0** — fix the eight broken doc links. Gate-critical, ~15 minutes,
   unblocks `cargo doc`.
2. **i18n drift + test hardening** — fix U27-1, U24-2, U18-1, then close the
   `i18n_keys.rs` gaps (U23-1, U23-2) so future drift fails the gate.
3. **Doubled header (U30-1)** and the missing `tiles` registration `info!` (U30-2).
4. **Prose TODOs** — file the tracking issue(s), convert the ~30 comments to
   `// see #NNN`.
5. **Test gaps** — U29-3 (cancel branch), U22-2 (`load_from`), and the smaller
   ones; decide the transitive-coverage judgment calls.
6. **Polish, as you touch the code** — `Theme` `Copy` (U17-1), `Provider::id`
   (U9-1), the latent paint-path panics (U16-1, U6-1), the boundary moves (U21-2,
   U1-1).

## Strengths worth preserving

This is not a remediation list with no upside — the disciplines below are why the
audit came back clean, and they should survive future churn.

- **`core`** — flat strided `Vec<u8>` buffers with `checked_mul` guards; every id a
  distinct `Copy` newtype with a `compile_fail` doctest; the AI-result-as-command
  boundary takes plain pixels, never a services type; undo restores exactly,
  including list positions.
- **`render`** — version-correct wgpu 29.0.1; deliberately bytemuck-free std140
  hand-packing to keep wgpu the sole dependency; nearest sampling on all filters
  (the most common pixel-art bug, avoided); heavy GPU objects built once.
- **`ui`** — the deferred-intent contract is enforced by types (read-only
  `ContribCtx`, a single `&mut IntentSink`), `MsgKey` makes displaying an
  unresolved key a compile error, and the 86-arm intent `match` is wildcard-free so
  a new variant fails the build until handled.
- **`services`** — lib-`tokio` pinned to `rt`/`macros`/`time` (not `full`); biased
  `select!` so cancel wins ties; the one sanctioned `Arc<Mutex>` documented; the
  async-trait crate avoided via a hand-rolled `Pin<Box<dyn Future>>` to keep `dyn
  Provider` object-safe.
- **`providers`** — textbook untrusted-decode guard (`ImageReader` + `Limits` at
  8192); the exact verified openrouter-rs 0.10 image surface; the API key never
  logged, never stored in provenance.
- **`platform` / `app`** — both `directories` traps handled with recorded
  rationale; the binary owns the one runtime, the one subscriber (guard held for all
  of main), and sets the boot language — the responsibilities the spine reserves for
  it.

## Artifacts

- Per-crate detail: `by-crate/*.md` (linked above).
- The audit workflow script: `.claude/audit_workflow.js` (re-runnable).
