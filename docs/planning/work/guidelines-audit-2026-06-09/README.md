# Guidelines audit — 2026-06-09 (the unaudited dimensions)

Audit of the workspace against the guideline surface the prior two audits never
covered: testing conventions, tracing, i18n, UI conventions, async/concurrency,
call-site idioms (result-handling / pointers / dispatch / type-state), the
egui/wgpu/eframe and per-dependency API skills, the architecture bible's
runtime rules, comments and recorded decisions, voice, and repo hygiene.

Complements `../code-audit-2026-06-08/` and `../skill-audit-2026-06-09/`.
Deliberately excluded: `pixhaus-rust-conventions`, `pixhaus-thiserror`, and
`pixhaus-rust-modern` (audited and fixed this morning) and the clippy-enforced
classes (no unwrap/expect/panic, -D warnings, no unsafe — clean by
construction). `pixhaus-performance` was swept during gap closure: its only
static surface (unmeasured performance claims in rationale comments) is clean —
every perf comment in the tree either cites a measured bound or scopes itself
off the pixel hot path.

## Method

An ultracode workflow, the same shape as the morning audit but wider: 49 finder
agents across 11 dimensions, each reading the governing skill and doc files
before sweeping its crate group (~38.7K lines split six ways) and citing the
written rule for every proposed finding. 223 raw findings deduped to 205. Every
finding then went to two adversarial verifiers — one auditing the rule citation,
one the code claim, both default-reject — with a third as tiebreaker on splits:
106 confirmed, 99 rejected. A completeness critic audited the finders' own
coverage claims afterwards; its eight gaps were closed by the orchestrator by
hand (see Gap closure). 481 agents total.

Severity calibration: the verifiers also judged severity, and where both argued
a downgrade it is applied here and marked `*`. The single finder-rated high
(the sprite-edit AI button literals) was downgraded to medium by both
verifiers. After calibration plus the gap-closure additions: 0 high, 21 medium,
93 low.

## Result at a glance

- **Architecture is fully clean.** Both bible dimensions (dependency direction
  and the commands/jobs runtime rules) produced six proposals; the verifiers
  refuted all six. The crate graph is acyclic and layered as documented, core
  and render see no egui, all project-state mutation routes through commands,
  and AI/provider work runs as jobs whose results apply as commands.
- **No finding survived at high severity.** The dominant signal is drift, not
  defects: doc comments and per-crate CLAUDE.md files trailing the code they
  describe (the generation module's docs still call it a stub), test-shape
  conventions applied unevenly (rstest tables, assert messages), and dead
  configuration (a post-edit hook that cannot run on macOS, a stale advisory
  ignore, pre-v3 root-tree leftovers).
- **A handful of correctness-adjacent catches** justify the sweep: cross-wired
  field labels in the codex style editor, index-keyed edit buffers that attach
  stale text to the wrong row after a remove, a derived `Deserialize` that
  bypasses `CodexHandle`'s validation, a test asserting the opposite of its
  name, and silently swallowed undo/redo failures.

## Act first — the calibrated mediums

Correctness-adjacent:

| Location | Finding | Fix direction |
|---|---|---|
| `modules/codex/src/codex_ws/details.rs:482` | Style editor cross-wires field labels: detail-level picker reads "Outline", anti-aliasing picker reads "Dithering" | Mint `codex.style.detail_level` / `.anti_aliasing` keys and use them; keep the option-value keys as they are |
| `modules/codex/src/codex_ws/details.rs:574` | Per-row edit buffers keyed by list index attach stale text to the wrong row after a remove | Key buffers and `push_id` by stable row identity, or clear the buffers on remove |
| `crates/core/src/codex/handle.rs:18` | Derived `Deserialize` bypasses `CodexHandle::new` validation, breaking the always-valid invariant on load | `impl TryFrom<String>` delegating to `new`, then `#[serde(try_from = "String")]` |
| `crates/services/src/codex/reference.rs:411` | Test named "rejects unknown namespace" asserts the opposite; `UnknownNamespace` is pinned by no test | Rename to what it proves; add a real `UnknownNamespace` test |
| `crates/ui/src/state/intent.rs:632` | Undo/redo failures swallowed with `.is_ok()` — no log line | `warn!` on real errors; stay quiet (or `trace!`) on `NothingToUndo`/`NothingToRedo` |

Dev tooling and hygiene (the first is an orchestrator finding from gap closure):

| Location | Finding | Fix direction |
|---|---|---|
| `.claude/settings.json` (PostToolUse) | Hook runs `powershell … post-edit.ps1` only; neither `powershell` nor `pwsh` exists on this macOS machine, so the per-edit format+clippy guard CLAUDE.md describes has been dead here — `post-edit.sh` exists but is wired nowhere | Dispatch by platform (a one-line wrapper that picks `.sh` on unix, `.ps1` on Windows) |
| `.cargo/deny.toml:26` | RUSTSEC-2026-0009 ignore rationale cites "MSRV 1.85" and a re-evaluation trigger (>= 1.88) that has already fired at MSRV 1.95 | `cargo update -p time`, delete the ignore, re-run `cargo deny check` |
| repo root | Pre-v3 leftovers at top level violate the repo-layout rule: `ui/`, `node_modules/`, `tests/`, `website/`, `docs/site/`, `.playwright-report/`, `app/gen/` — none gitignored (see Gap closure) | Delete them (all date to the Tauri era; none contains source the v3 tree uses); gitignore the artifact patterns |

Docs trailing the code (stale text misleads both readers and future agent sessions):

| Location | Finding | Fix direction |
|---|---|---|
| `modules/generation/CLAUDE.md:9` | Still declares the generation module a stub; it registers a workspace, panels, and real jobs | Rewrite the status line to the wired reality |
| `modules/generation/src/lib.rs:6` | Crate doc says panels are mock and provider dispatch is future; both partly false | Describe the live/mock split as it stands |
| `modules/generation/src/generate.rs:423` | ResultsPanel doc describes the retired mock panel | Describe the store-driven grid and intent flow |
| `app/examples/render_workspaces.rs:42` | `build_host` doc claims it mirrors `main.rs`; the bodies have diverged (no provider registration, no codex seed) | Scope the mirror claim or re-sync the body |
| `crates/ui/src/widgets/tool_button.rs:6` | Doc comment describes the old active style (accent.muted, 2px); code paints `tool_active_bg` with 3px | Update the doc comment |
| `crates/render/CLAUDE.md:9` | Claims "Depends on: core"; the manifest depends only on `wgpu` (orchestrator finding) | Correct the dependency line (or add the dep when render gains core types) |
| `crates/services/CLAUDE.md:8` | Claims a dependency on `io` that does not exist; external list omits thiserror, serde, parking_lot, rust-i18n, tracing (orchestrator finding) | Re-derive the block from the manifest |

Convention deviations worth a deliberate pass:

| Location | Finding | Fix direction |
|---|---|---|
| `modules/sprite-edit/src/draw.rs:315` (was high) | AI sub-row buttons are permanent English literals ("Fill", "Clean up", "Make seamless") — real action buttons, not mock rows, and the comment declares them permanent | Mint short-form keys in `sprite_edit.yaml`, resolve via `MsgKey::tr()`, rewrite the stale comment |
| `crates/ui/src/contrib_api/context.rs:64` | `PanelScope` carries a second mutable carve-out (`draft`) that the skill, `crates/ui/CLAUDE.md`, and the Panel trait doc all say cannot exist | Reconcile rule and code in one change — name both sanctioned carve-outs everywhere or remove one |
| `crates/services/src/transaction.rs:75` | `Transaction::estimated_size_bytes` has no test | Pin the sum-of-children-plus-self contract |
| `crates/ui/src/shell/mod.rs:107` | `sync_playback_mirror` has no test despite being headless-testable | Mirror the playback/onion test pattern |
| `modules/export/src/export_ws.rs:345` | Four panels' ids/regions packed into one unlabeled-loop test | rstest `#[case]` table, one labeled case per panel |
| `modules/providers/src/postprocess.rs:51` | Image ops `chroma_key_magenta` / `slice_sheet` have no proptest (the skill names image ops as proptest ground) | Add round-trip and invariant properties |
| `modules/providers/src/openrouter.rs:118` | No span captures provider-response duration on the one external AI call | Coarse `info_span!`/elapsed-ms field around the await |

## All confirmed findings by dimension

Severity is verifier-calibrated; `*` marks a downgrade both verifiers argued
for (`medium*` = filed high, `low*` = filed medium). Full evidence, rule
citations, verifier reasoning, and suggested fixes for every row live in the
workflow output (see Method); locations are exact.

### i18n (4)

The locale bundles are consistent and the key architecture holds; the catches are two real label defects (one of them the cross-wired style editor above), one key-shape deviation, and one key reuse.

| Location | Severity | Finding |
|---|---|---|
| `modules/sprite-edit/src/draw.rs:315` | medium* | Selection Actions AI sub-row buttons are permanent English literals ("Fill", "Clean up", "Make seamless") |
| `modules/codex/src/codex_ws/details.rs:482` | medium | Style editor cross-wires field-label keys: detail-level picker labeled 'Outline', anti-aliasing picker labeled 'Dithering' |
| `crates/services/locales/providers.yaml:16` | low | Provider failure keys are named provider.error.<case> instead of the documented error.<domain>.<case> shape |
| `modules/codex/src/codex_ws/inspector.rs:75` | low | Quick-actions section header reuses codex.editor.section.identity ('Identity') |

### UI conventions (13)

Theme-token discipline is good but not airtight: a few raw `Color32`/radius/size literals sit where tokens exist, and three spots use accent color against the recorded one-violet-handle decision. The `PanelScope` second carve-out is the one structural rule/code conflict.

| Location | Severity | Finding |
|---|---|---|
| `app/examples/render_workspaces.rs:4` | low* | Harness doc comment says 'the five capability modules' but six are registered |
| `app/examples/render_workspaces.rs:38` | low* | Harness build_host claims to mirror main.rs but has drifted: no provider registration, no boot-time codex seed |
| `crates/ui/src/contrib_api/context.rs:64` | medium | PanelScope carries a second mutable carve-out (draft: Option<&mut CodexEditorDraft>) that both rule sources and the Panel trait doc say cannot exist |
| `crates/ui/src/shell/regions/canvas_stage.rs:92` | low* | Color32 literals for the artboard drop shadow in region code |
| `modules/codex/src/codex_ws/editor.rs:155` | low* | Corner-radius literals duplicate theme.radius tokens in painter calls |
| `modules/codex/src/codex_ws/inspector.rs:31` | low* | Bare #[allow(clippy::too_many_lines)] with no recorded reason |
| `modules/codex/src/codex_ws/navigator.rs:106` | low* | Bare #[allow(clippy::too_many_lines)] with no recorded reason |
| `crates/ui/src/shell/regions/canvas_stage.rs:358` | low | Hardcoded padding/offset values in paint_hud instead of theme.spacing tokens |
| `modules/animation/src/animate.rs:425` | low | #[allow(clippy::cast_precision_loss)] on clip_span_rect carries no recorded reason |
| `modules/codex/src/codex_ws/editor.rs:46` | low | Empty-state glyph sized with a 48.0 literal instead of a type-scale token |
| `modules/codex/src/codex_ws/editor.rs:1095` | low | Relations rows color the entry @handle in accent, contradicting the design system's recorded one-violet-handle decision |
| `modules/codex/src/codex_ws/navigator.rs:127` | low | Comment claims a 'filled accent button' but no accent fill is applied; on_accent text used without an accent background |
| `modules/generation/src/codex_context.rs:130` | low | Inert @-suggestion chips colored accent.base |

### egui / wgpu API (3)

The 0.34/29.0.1 API usage is correct throughout — the callback contract, texture registration, and eframe lifecycle all check out. The catches are one retained-state bug (the index-keyed buffers above), one unused manifest dependency, and one unvirtualized list.

| Location | Severity | Finding |
|---|---|---|
| `app/Cargo.toml:15` | low* | Binary declares egui-wgpu directly despite never using it |
| `modules/codex/src/codex_ws/details.rs:574` | medium | Per-row edit buffers keyed by list index attach stale text to the wrong row after a remove |
| `modules/codex/src/codex_ws/navigator.rs:165` | low | Navigator entry list renders a widget per row inside a plain ScrollArea::show |

### Testing (40)

The largest dimension, and almost entirely shape rather than coverage: multi-input cases packed into single tests or unlabeled loops where the skill prescribes rstest `#[case]` tables (~12 rows), bare `is_ok()` asserts without the error attached, prefix-named flat tests where nested modules are prescribed, and a few genuinely untested public functions. One real race: a test flips the process-global show-keys flag without `#[serial]` under parallel nextest. Doc tests are effectively absent workspace-wide (core 2, services 0, ui 0) — filed once against services, and the same observation holds for ui and core (gap-closure normalization).

| Location | Severity | Finding |
|---|---|---|
| `crates/core/src/buffer_store.rs:69` | low* | PixelBufferStore::is_empty has no test anywhere in the workspace |
| `crates/core/src/pixel.rs:212` | low* | prop_assert!(buf.is_ok()) carries no failure message with the error |
| `crates/services/src/codex/reference.rs:411` | medium | Test name claims 'rejects unknown namespace' but the body asserts the opposite |
| `crates/services/src/transaction.rs:75` | medium | Transaction::estimated_size_bytes has no test anywhere |
| `crates/ui/src/shell/mod.rs:107` | medium | pub fn sync_playback_mirror has no test anywhere despite being headless-testable |
| `crates/ui/src/state/intent.rs:1394` | low* | toggle_i18n_keys test flips the process-global SHOW_KEYS flag without #[serial], racing a tr()-dependent test in the same binary |
| `crates/ui/tests/generate_loop.rs:7` | low* | Blanket file-level allow of disallowed_methods/unwrap_used/expect_used with no recorded reason, then uses .expect() on its own code |
| `modules/animation/src/animate.rs:464` | low* | animation_panel_ids_and_regions packs three panels' ids and regions into one test |
| `modules/export/src/export_ws.rs:345` | medium | dock_panel_ids_and_regions packs four panels' ids and regions into one test with an unlabeled loop |
| `modules/providers/src/mock.rs:249` | low* | Bare assert!(...is_ok()) with no failure message in mock-provider tests |
| `modules/providers/src/openrouter.rs:291` | low* | Bare assert!(...is_ok()) with no failure message in request-shaping tests |
| `modules/providers/src/postprocess.rs:51` | medium | Image ops chroma_key_magenta and slice_sheet have no proptest |
| `modules/sprite-edit/src/draw.rs:608` | low* | shared_dock_panel_ids_and_regions and shared_tray_panel_ids_and_regions pack multi-input cases into single tests with unlabeled loops |
| `modules/tiles/src/tiles_ws.rs:396` | low* | tile_panel_ids_and_regions packs six panels' ids and regions into one test with an unlabeled loop |
| `crates/core/src/codex/details.rs:252` | low | EntryDetails::default_for tests stack/loop four inputs per body instead of an rstest table |
| `crates/core/src/codex/handle.rs:67` | low | Multi-input handle-validation tests loop/stack inputs instead of using an rstest table |
| `crates/core/src/codex/handle.rs:62` | low | Five tests for CodexHandle::new sit flat instead of in a nested `new` module |
| `crates/core/src/pixel.rs:174` | low | Unlabeled #[case(...)] in the only rstest case table in core |
| `crates/core/src/pixel.rs:113` | low | PixelBuffer::into_bytes has no test that pins its own contract |
| `crates/render/src/lib.rs:439` | low | frame_upload_is_skippable_only_on_zero_area inlines five input cases; render has no rstest dev-dependency |
| `crates/services/src/codex/health.rs:714` | low | health_tier_bands packs three score-band cases plus a label_key assertion in one test |
| `crates/services/src/codex/reference.rs:414` | low | Bare assert!(...is_ok()) without a failure message |
| `crates/services/src/codex/validation.rs:360` | low | worst_and_severity_order packs worst(), count(), and Severity ordering into one test |
| `crates/services/src/history.rs:289` | low | undo_on_empty_history_errors also tests redo, which its name does not commit to |
| `crates/services/src/job.rs:444` | low | grid_cell_count_multiplies_cols_by_rows packs four input cases inline instead of an rstest table |
| `crates/services/src/job.rs:344` | low | drain_until_terminal poll-loops with tokio::time::sleep(1ms) up to 5000 iterations |
| `crates/services/src/lib.rs` | low | Zero doc tests across a crate with ~100 public functions, several with non-obvious usage |
| `crates/services/src/provider.rs:205` | low | message_key_is_stable_per_variant is a three-case input table without rstest |
| `crates/services/src/provider.rs:212` | low | detail_carries_context_only_when_present is a three-case input table without rstest |
| `crates/services/src/provider.rs:196` | low | by_id_round_trips tests three behaviors: found, not-found, and all()'s length |
| `crates/ui/src/canvas/overlay.rs:20` | low | paint_selection and paint_tool_preview (canvas/overlay.rs:20, :34) have no tests |
| `crates/ui/src/canvas/view.rs:184` | low | Units with 3+ prefix-named tests are flat instead of grouped in nested modules |
| `crates/ui/src/lib.rs:50` | low | pub fn install_canvas_renderer has no test |
| `crates/ui/src/widgets/card.rs:16` | low | All Ui-taking widget pub fns are untested: 9 across card/section_header/tool_button/tray_tab/workspace_tab/busy/placeholder plus ~28 in widgets/codex.rs |
| `crates/ui/src/widgets/layout.rs:79` | low | Multi-input case tables written as repeated assert_eq lines instead of rstest #[case] tables; rstest is a declared dev-dependency but never used |
| `modules/codex/src/codex_ws/mod.rs:217` | low | Four next_free_slot_key tests vary only input but are flat copy-pasted bodies, not an rstest table or nested module |
| `modules/generation/src/prompt/types.rs:107` | low | frame_count test packs three input cases into one body instead of an rstest table |
| `modules/providers/src/lib.rs:97` | low | now_ms_is_nonzero_and_monotonic asserts ordering of two wall-clock reads |
| `modules/sprite-edit/src/draw.rs:599` | low | layers_panel_meta is named for the function, not a behavior |
| `modules/sprite-edit/src/tools.rs:516` | low | pencil_meta is named for the function, not a behavior |

### Tracing (6)

No subscriber leaks, no println, no secrets in logs — the API-key paths are clean. The findings are observability gaps (no span on the provider response, none on texture upload), one privacy-shaped span field (the raw autocomplete query), the swallowed undo/redo errors above, and a default EnvFilter that never gives modules their documented debug level.

| Location | Severity | Finding |
|---|---|---|
| `app/src/diagnostics.rs:27` | low* | DEBUG_DEFAULT omits every pixhaus_mod_* module crate, so modules never get the documented debug level in debug builds |
| `crates/services/src/codex/coverage.rs:122` | low* | Custom target "pixhaus::coverage" renames the module and silently filters the event out under the default EnvFilter |
| `crates/services/src/codex/search.rs:55` | low* | suggest() records the raw user-typed autocomplete query into its #[instrument] span |
| `crates/ui/src/state/intent.rs:632` | medium | Undo/Redo failures are silently swallowed with .is_ok() — no log line |
| `modules/providers/src/openrouter.rs:118` | medium | OpenRouter AI request has no span capturing the provider-response duration |
| `crates/render/src/lib.rs:233` | low | ViewportRenderer::upload_frame (a texture upload) has no span or tracing event; pixhaus-render has no tracing dependency at all |

### Async / concurrency (5)

No lock crosses an await anywhere — the load-bearing rule holds. The findings: two per-pixel/encode paths running on the reactor instead of `spawn_blocking`, two feature-trim opportunities (workspace `futures` default features, an unused `rt` feature), and the app's missing exit wind-down (jobs die by runtime drop, no root `CancellationToken`).

| Location | Severity | Finding |
|---|---|---|
| `Cargo.toml:32` | low* | Workspace pins futures with default features, keeping the skill-banned bundled executor available |
| `modules/providers/src/mock.rs:74` | low* | MockProvider draws per-pixel sprite/animation buffers directly on the reactor |
| `modules/providers/src/openrouter.rs:120` | low* | Reference-image PNG encode runs on the tokio reactor inside the generate future |
| `app/src/main.rs:198` | low | App holds no root CancellationToken and has no exit wind-down; in-flight jobs die by Runtime drop |
| `crates/services/Cargo.toml:26` | low | tokio-util inherits the workspace `rt` feature that services does not use, silently re-enabling tokio/sync |

### Call-site idioms (7)

One premature `Arc<Vec<u8>>` with a single owner end-to-end, an `AtomicU64` behind `&mut self`, two method-call `.clone()` on Arc handles where `Arc::clone` is the convention, one bare allow, and two error paths no test drives.

| Location | Severity | Finding |
|---|---|---|
| `crates/ui/src/lib.rs:61` | low* | Premature Arc: CanvasFrame.rgba is Arc<Vec<u8>> with a single owner end-to-end |
| `crates/core/src/composite.rs:60` | low | composite_frame's Err paths are never driven by any test |
| `crates/core/src/pixel.rs:63` | low | PixelBuffer::new's only Err variant (PixelError::Overflow) is never reached by a test |
| `crates/services/src/job.rs:154` | low | JobManager uses AtomicU64 for its job counter though every mutation path is behind &mut self |
| `crates/ui/src/state/intent.rs:965` | low | Method-call .clone() on an Arc field instead of explicit Arc::clone |
| `crates/ui/src/state/intent.rs:973` | low | Bare #[allow(clippy::too_many_arguments)] with no recorded reason |
| `modules/providers/tests/mock_generation_loop.rs:50` | low | Arc handle cloned via method-call .clone() instead of explicit Arc::clone |

### Comments and recorded decisions (20)

The Recording-decisions rule is mostly honored — the misses are stale rationale (comments describing code that changed: the no-op re-sort, the fuzzy-score doc, two stale counts of regions/modules), narrate-the-code comments, bare `#[allow]`s without a recorded reason, and a few uncommented magic numbers. One banned word ("robust").

| Location | Severity | Finding |
|---|---|---|
| `app/examples/render_workspaces.rs:42` | medium | build_host doc claims it is kept identical to main.rs, but main.rs's build_host has diverged |
| `crates/services/src/codex/compiler.rs:267` | low* | Comment claims a re-sort 'back into author order' that the code does not (and cannot) perform |
| `crates/services/src/codex/search.rs:131` | low* | fuzzy_score doc comment promises a 'digit boundary' word-start bonus the code does not implement |
| `crates/ui/src/region.rs:9` | low* | 'The seven window regions' doc sits directly atop an eight-variant Region enum |
| `crates/ui/src/shell/regions/mod.rs:1` | low* | regions/mod.rs repeats the stale 'seven window regions' count |
| `crates/ui/src/widgets/tool_button.rs:6` | medium | Doc comment describes the old active style (accent.muted background, 2px line); code paints tool_active_bg with a 3px line |
| `modules/generation/CLAUDE.md:9` | medium | Module CLAUDE.md still declares generation a stub |
| `modules/generation/src/generate.rs:423` | medium | ResultsPanel doc comment describes the retired mock version of the panel |
| `modules/generation/src/generate.rs:309` | low* | recipe_row's allow rationale defends casts that do not exist in the function |
| `modules/generation/src/lib.rs:6` | medium | Crate doc says panels render mock content and provider dispatch is future; both are now partly false |
| `crates/ui/src/shell/about.rs:29` | low | Narrate-the-code comment '// Escape closes.' |
| `crates/ui/src/shell/regions/bottom_tray.rs:49` | low | Narrate-the-code comment '// Tab row.' |
| `crates/ui/src/shell/regions/canvas_stage.rs:26` | low | Banned word 'robust' in a doc comment |
| `crates/ui/src/shell/regions/canvas_stage.rs:370` | low | Magic placement offset vec2(-176.0, -38.0) for the floating zoom control has no recorded reason |
| `crates/ui/src/theme/tokens.rs:116` | low | AccentTokens doc cites default seed '~#7c6cef' but DEFAULT_ACCENT_SEED is #7b68f0 |
| `modules/codex/src/codex_ws/editor.rs:393` | low | #[allow(clippy::too_many_lines)] on render_overview_card without an explicit allow rationale |
| `modules/codex/src/codex_ws/editor.rs:202` | low | 'Test generate' buttons do not generate, with no stand-in marker at either spot |
| `modules/codex/src/codex_ws/inspector.rs:77` | low | Quick-actions cluster narrates each button with restate-the-code comments |
| `modules/codex/src/codex_ws/inspector.rs:47` | low | Health-tier thresholds 0.8/0.4 are uncommented magic numbers |
| `modules/tiles/src/tiles_ws.rs:304` | low | Narrate-the-code comments restating the called function names |

### Dependency APIs (3)

serde, glam, bytemuck, directories, rfd, parking_lot, and openrouter usage all conform to their skills. The catches: the `Deserialize`-bypasses-validation invariant hole above, a workspace-level unused tokio-util feature, and one `to_rgba8()` where `into_rgba8()` avoids a copy.

| Location | Severity | Finding |
|---|---|---|
| `crates/core/src/codex/handle.rs:18` | medium | Derived Deserialize on validated CodexHandle bypasses its validation, breaking the documented always-valid invariant |
| `Cargo.toml:34` | low | Workspace tokio-util pin enables the rt feature though only the feature-free CancellationToken is used anywhere |
| `modules/providers/src/openrouter.rs:233` | low | to_rgba8() used where the DynamicImage is dropped immediately — into_rgba8() is the documented idiom |

### Hygiene (5)

The workspace dependency catalog is fully consumed and correctly inherited. The findings are the stale advisory ignore above, the root-tree leftovers (extended during gap closure — the finder filed three of eight), and `rustfmt.toml` setting `max_width = 160` while CLAUDE.md claims "rustfmt defaults".

| Location | Severity | Finding |
|---|---|---|
| `.cargo/deny.toml:26` | medium | RUSTSEC-2026-0009 ignore rationale cites 'workspace MSRV of 1.85' and a re-evaluation trigger (>= 1.88) that has already fired at MSRV 1.95 |
| `.playwright-report` | low* | Untracked top-level .playwright-report/ is a stale test-run artifact, not gitignored |
| `ui` | low* | Untracked top-level ui/ is a dead v1 frontend tree (dist + node_modules only) |
| `app/gen` | low | app/gen/schemas/ contains Tauri 2 generated schema JSONs inside the eframe binary crate |
| `rustfmt.toml:2` | low | rustfmt.toml overrides defaults (max_width = 160) while CLAUDE.md claims 'rustfmt defaults' |

### Architecture (0 findings)

Both dimensions came back clean. All six proposals (three per dimension) were
refuted on evidence: the suspected layering and mutation-bypass sites turned
out to be the documented carve-outs or misreadings. Dependency direction,
egui-free core/render, commands-own-mutation, and the jobs model all hold as
the bible draws them.

## Gap closure — the completeness critic's eight gaps

The critic audited the finders' coverage claims and spot-checked the weakest
three; unit coverage of the Rust tree itself was complete (every crate, module,
manifest, locale bundle, and the WGSL shader appears in at least two finders'
read lists), but eight gaps needed closing. Orchestrator disposition:

1. **Root-tree artifacts under-filed.** The hygiene finder ls'd all the
   offenders but filed only `ui/`, `.playwright-report/`, and `app/gen/`.
   Verified and filed here: `node_modules/` (dead pre-v3 pnpm tree with
   `@tauri-apps`, no root `package.json`), `tests/` (e2e/visual husks,
   node_modules only), `website/` and `docs/site/` (Astro skeletons,
   node_modules only) — all from the Tauri era, none gitignored, all violating
   the same repo-layout / `preventRootAdditions` ground. A stray root file
   named `-` (a rustfmt default-config dump created by this audit's own finder
   at 15:01) was deleted during gap closure.
2. **Per-crate CLAUDE.md accuracy had no owner.** Verified against the
   manifests and filed: the render and services dependency-claim drifts
   (tables above) and `crates/ui/CLAUDE.md:11`'s external list omitting
   egui-phosphor, egui_extras, image, serde, thiserror, tracing (low).
3. **Stale skill text had no filing channel.** Recorded below as the skill
   maintenance lane.
4. **`pixhaus-claude-code-workflow` was nobody's ground.** Commit subjects all
   conform to Conventional Commits, but the history contains zero merge
   commits — every change lands directly on `v3` while CLAUDE.md mandates
   branch -> PR -> merge. Surfaced as a maintainer question below.
5. **The visual verification loop was unexercised.** Every UI finder was
   read-only, so `cargo run -p pixhaus-app --example render_workspaces` +
   comparison against `docs/ui_visual_example/` never ran. Static greps catch
   token violations, not layout drift. Left as the one open follow-up.
6. **Doc-test adjudication normalized.** Counted: core 2 doc-test fences,
   services 0, ui 0. The services finding stands at low and now reads as the
   workspace-wide observation it is.
7. **`scripts/` and `.claude/settings.json` were in no unit list.** Read during
   gap closure: the sh/ps1 pair mirror each other and match CLAUDE.md's hook
   description — but the settings wiring only invokes the ps1 via
   `powershell`, which does not exist on this machine. Filed as the dead-hook
   medium above.
8. **`pixhaus-performance` had no owner.** Swept by grep during gap closure:
   clean (see Scope).

## Skill and doc maintenance lane

The audit grounded every dimension in the skill files — and found the ground
itself stale in four places. Stale skills miscalibrate every future session
that loads them:

1. **`pixhaus-serde` (and the rmp-serde/zstd cross-references)** present
   MessagePack + zstd as the `.pixhaus` on-disk format, while
   `docs/pixhaus_save_file_format_architecture.md` settles V1 on JSON.
   Reconcile before `crates/io` gains a body — an agent implementing io today
   would follow the skills into the wrong format.
2. **`pixhaus-result-handling`** says tests unwrap freely via the
   `#![cfg_attr(test, allow(...))]` pattern — but `clippy.toml`'s
   `disallowed-methods` bans `unwrap`/`expect` through a lint that allowlist
   does not cover, which is why test files end up adding their own file-level
   allows (`crates/ui/tests/generate_loop.rs`). Either document
   `clippy::disallowed_methods` in the sanctioned test-allow pattern or amend
   the skill's claim.
3. **`pixhaus-claude-code-workflow:54`** scopes commits to a crate list
   containing `shell`, which does not exist in the v3 layout (the crates are
   core, io, render, services, platform, ui, plus modules and app).
4. **`pixhaus-rust-conventions`** carries the same v2-era naming: its crate
   ownership table is core/io/render/shell and its error-handling section
   says "the `shell` binary". The rules are right; the names predate the v3
   graph.

## Questions for the maintainer

- **The PR rule vs reality.** CLAUDE.md says "never merge directly" and every
  skill repeats branch-per-change, yet the entire history is direct commits on
  `v3` with zero merges. Either record the v3 clean-slate exception where the
  rule lives, or start enforcing it now that the scaffold is runnable. Dead
  process text trains every reader to ignore the live rules next to it.
- **The post-edit hook fix.** Wiring `bash scripts/post-edit.sh` on this
  machine is one line in `.claude/settings.json`, but the file is checked in
  and shared — a platform dispatcher keeps it working on both OSes.

## Open follow-up

- Run the visual verification loop (`cargo run -p pixhaus-app --example
  render_workspaces`, compare `target/ui-snapshots/` against
  `docs/ui_visual_example/`) in a write-enabled session. It is the one
  verification rule CLAUDE.md names for UI work that this audit could not
  exercise.

## Rejected findings — 99

The adversarial pass earned its cost: 48% of deduped findings died under
verification. The largest killed class was "Panel::ui / shell-region functions
have no test" (~15 findings) — refuted because the egui_kittest
`render_workspaces` harness executes `Shell::run` across all workspaces plus
the splash, exercising every cited function, and the boot-smoke decision is
recorded in the ui-shell-foundation spec. Other recurring kill grounds: the
rule cited did not actually say that (the i18n dimension proposed 21, lost 17,
mostly to the mock-placeholder carve-out and key-shape readings); the code was
test-only; or a nearby comment recorded the choice as deliberate. The morning
audit's lesson repeated: verifiers were instructed to ground every claim in
the files, not training data, after one stale-memory rejection yesterday.

## Verification

This audit changed no source code. The only write was deleting the stray root
file `-` (fallout from this audit's own finder). The Stop-gate state is
unchanged from this morning's audit session: fmt clean, clippy clean at
-D warnings, 644 tests passing, deny clean on the recorded ignore (whose
removal is itself a finding above).
