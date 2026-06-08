## mod-generation

This unit is near-exemplary and the highest-compliance group in the audit. Architecture boundaries are honored cleanly: panels are `&self`, read through the read-only `ContribCtx`, and route every mutation through typed `Intent`s — generation is modeled as job submission with the apply step owned elsewhere as a command, and the workspace asks for a capability rather than a provider. The i18n data-vs-key split is handled with rare care, theme tokens are used throughout, and the pure prompt builders are well-tested. The only defects are minor hygiene items: four prose `// TODO(luis):` comments and one borderline set of mock-row badge literals, both already acknowledged in-tree.

### Strengths

- Clean deferred-intent compliance: every panel is `&self`, reads `scope.ctx.session` through read-only accessors (`is_generating`, `selected_is_anchor`, `result_frame_count`), and pushes `Intent`s; the one `&mut` is `scope.scratch` for the TextEdit, with the carve-out documented at both prompt panels.
- Correct job/command boundary: the module never executes a job or touches the canvas — it submits via `Intent::SubmitAnchorJob`/`SubmitIdleAnimationJob` and applies results via `Intent::InsertSelectedResultAsSprite`/`InsertSelectedAsAnimatedSprite`, matching the bible's "AI proposes, applying a result is a command" rule.
- Exemplary i18n data-vs-key discipline: UI labels are `MsgKey` keys present in `generation.yaml`/`codex.yaml`, while the seed string, prompt text, and `compiled.negative` (passed as a `tr_args` interpolation argument) are kept as DATA, never folded into a key — exactly as i18n rule 12 demands; the `kb.rs`/`types.rs`/`defaults.rs` headers each restate this.
- Theme-token-only styling throughout: every color, spacing, and radius comes from `theme.surfaces/roles/accent/spacing/radius/type_scale` and `theme.mock.thumbnails`; no hex or `Color32::from_rgb` literal in any panel or paint code. The violet accent is reserved for AI affordances.
- Shared widgets and phosphor icons used correctly (`busy_indicator`, `section_header`, `mock_thumbnail_grid`, `mock_log`, `reference_chip`, `strength_selector`, `icons::SPARKLE/CODEX/REFERENCE`) — no bespoke chrome, no emoji.
- Static dispatch preserved at the right seam: `strength_selector` takes `impl Fn(AnchorStrength) -> String` rather than a boxed closure; `Box<dyn HostRegistrar>`/`Box::new(panel)` appear only at the genuine registry heterogeneous-collection boundary.
- Strong, intent-revealing tests on the pure logic: `frame_count` saturation, anchor/idle prompt assembly, row-major frame ordering, the breathing-phase generalization beyond 8 frames, a guard that the anchor spec and idle const key on the same magenta, and the anchor-strength-key uniqueness test.
- Decisions are recorded where they live: the numeric-cast `#[allow]`s on `recipe_row`/`result_card` carry a why-comment ("small bounded constants; the casts cannot truncate"), and both the foreign-panel-id composition and the registered-but-not-yet-dispatched `gen.*` actions carry the rationale the "record the why" rule asks for.
- Tracing matches the skill: `info!(module = "generation", ...)` on registration with structured fields, no per-frame tracing in UI code, no `println!` or secrets.

### Findings

| ID | File:Lines | Severity | Category | Issue -> Fix |
|----|------------|----------|----------|--------------|
| U28-2 | modules/generation/src/generate.rs:326-331 | info (review) | i18n | `recipe_row` renders hardcoded `"Built-in"`/`"User"` badge phrases, which sit on the borderline of i18n rule 3 (a real phrase is keyed; a mock placeholder is left a literal). The rows are mock with no `core` data yet, so the placeholder carve-out plausibly covers them. -> No change this round; when the Recipe panel leaves mock, key the badge phrases (e.g. `command.gen.badge.built-in`/`.user`) even if the recipe names stay user data. |
| U28-1 | modules/generation/src/generate.rs:245, 356, 393, 615 | low | comments | Four prose `// TODO(luis): i18n these rows when the panel leaves mock` comments violate the rust-conventions rule against prose `// TODO`s in the tree (the clippy `todo` lint only catches the `todo!()` macro, so the Stop gate stays green and the violation is invisible to tooling). -> Replace each with an issue reference (e.g. `// Mock rows; keyed when the panel leaves mock — see #NNN`) or drop the prose and track the work in the issue tracker. |

### Checked and cleared (false positives)

None.
