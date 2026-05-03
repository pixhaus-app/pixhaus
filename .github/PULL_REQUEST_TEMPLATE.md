<!--
Thank you for the PR. Fill the sections below before requesting review.
The streams.md / bedrock.md references let reviewers find context fast.
-->

## What changed

<!-- One or two sentences. The "what", not the "why". -->

## Why

<!-- The "why". Link the stream or bedrock spec this implements:
     docs/planning/work/streams.md#sNN  or  docs/planning/work/bedrock.md#bN
     If this is unscoped, link the issue. -->

Implements:

## Test plan

<!-- What ran, what passed, what's still untested. Be concrete:
- `cargo test --workspace`: green
- `pnpm test`: green
- New tests: `core/tests/blend_modes.rs` covers the four new modes
- Not covered yet: visual regression for the new transform pipeline
-->

- [ ] `cargo build --workspace` green
- [ ] `cargo test --workspace` green
- [ ] `cargo clippy --workspace -- -D warnings` green
- [ ] `cargo fmt --check` green
- [ ] `pnpm typecheck` green
- [ ] `pnpm lint` green
- [ ] `pnpm test` green
- [ ] `pnpm build` green
- [ ] Documentation updated where the change touches public surface

## Screenshots / clips

<!-- For UI changes. Drop images or short MP4/GIF clips. Delete this section
     if the PR has no UI surface. -->

## Open questions

<!-- Anything you want the reviewer to weigh in on. -->

## Checklist

- [ ] Branch follows `feat/sNN-<slug>` (or `fix/`, `chore/`, `docs/`) naming
- [ ] Commits follow Conventional Commits
- [ ] CI is green
- [ ] No `unwrap()` / `expect()` / `panic!` in non-test Rust code
- [ ] No `console.log` left in TypeScript code
- [ ] No new dependencies outside the approved set in `docs/planning/ecosystem/`
      (or, if adding one, called out in "Open questions")
