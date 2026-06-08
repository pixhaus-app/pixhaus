## mod-animation

The animation module is one of the cleaner units in the codebase: every user-facing string flows through a MsgKey/i18n key that resolves in animation.yaml, all colors, spacing, and radii come from theme tokens, panels are strictly &self with mutation routed through Intents, icons are phosphor glyphs, and there is no unwrap/expect/panic/println in non-test code. Boundary discipline is exact (no pixhaus-core dependency, shared panels referenced by id not re-registered, the anim.* namespace avoids collisions), the egui 0.34 API is used correctly, and the single tracing line is a boot-time registration info! with no per-frame logging. Compliance is high. The only real gap is test coverage: the private geometry helper clip_span_rect — which exists to fix a drawn-vs-clickable span drift bug — has no direct test pinning its returned rect.

### Strengths

- Exemplary i18n: every chrome label is a MsgKey resolved at render time (animate.rs:116-130, 228-235, 157-163), every key resolves in crates/services/locales/animation.yaml, and user data (clip.name) is passed as a tr_args interpolation argument rather than baked into a key (line 116) — exactly the rule in pixhaus-i18n §12.
- Strict deferred-intent discipline: all three panels are &self, read only through scope.ctx, and push every mutation as an Intent; TimelinePanel collects at most one Intent into a local Option and pushes it after render so it never holds a borrow of scope mid-draw (lines 211, 358-360), with the why recorded in a comment.
- Token-only styling: no hex or Color32 literals — clip fills, accent outlines, ruler ink, and the decorative track bands all come from theme.mock.clips / theme.accent.base / theme.roles / theme.surfaces, with the tokens verified to exist in crates/ui/src/theme/tokens.rs and palettes.rs.
- Boundary correctness: Cargo.toml depends only on egui, pixhaus-services, pixhaus-ui, and tracing (no pixhaus-core); shared Layers/Sprites/Frames/Console panels are referenced by id and never re-registered (lines 30-34, 71-72); the anim.* action ids are namespaced to avoid colliding with sprite-edit's ai.* — each decision recorded in a comment.
- Recorded-decision comments at the spot each non-obvious choice shaped: the clip_span_rect single-source-of-truth note citing the prior 2px/4px drift bug (lines 421-423), the inert loop checkbox being a durable property deferred to a Command (lines 88, 126-127), and the status-bar onion-skin-only change replacing the old hardcoded placeholders (lines 76-80).
- tracing: the only event is a single boot-time info!(module="animation") on registration, consistent with every sibling module and the pixhaus-tracing "info! when a module registers capabilities" rule; no per-frame logging in any panel ui body.

### Findings

| ID | File:Lines | Severity | Category | Issue -> Fix |
|----|-----------|----------|----------|--------------|
| U25-1 | modules/animation/src/animate.rs:424-429 | low (review) | tests | clip_span_rect is the documented single-source-of-truth geometry helper added to fix a real drawn-vs-clickable span drift (2px/4px inset, lines 421-423), but it is a private fn so the cited "every public function has at least one test" rule does not strictly apply as worded. It is pure and deterministic, and the drift it guards could silently regress untested. -> Add an rstest/unit test in the existing #[cfg(test)] module pinning clip_span_rect for a couple of known (rect, start, end, frames) inputs (e.g. a full-width single-frame clip and a multi-frame span), asserting the returned min/size so the draw-vs-hit-test invariant is enforced by the suite. |

### Checked and cleared (false positives)

- U25-2 (register() fn has no test): rejected. The claim that animation "contributes nothing to such a check itself" is false — crates/ui/tests/support/mod.rs:33 registers AnimationModule into fully_registered_host(), which routes through animate::register(), and that host is exercised by crates/ui/tests/i18n_keys.rs::every_registered_key_resolves (walks every registered workspace, panel, action, and menu item asserting each MsgKey resolves) and crates/ui/tests/resolve_layout_snapshot.rs::animate_layout_is_stable (snapshots the resolved Animate layout, catching registration drift). register() is already covered by build_host-level integration tests.
