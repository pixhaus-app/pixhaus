# Work journal — the anchor + idle-animation pipeline for Generate

> **Why this file is in `docs/`.** `docs/CLAUDE.md` reserves this tree for durable,
> read-only design references and says transient task notes don't belong here. This
> file is a deliberate, user-requested exception: a running build journal, kept here
> (not at the repo root) because the conclaude `preventRootAdditions` hook blocks new
> top-level files. Same exception the foundation journal (`work_foundation.md`)
> documents. It records what landed, the decisions, and deviations from the plan.

Plan: `~/.claude/plans/lets-start-working-on-proud-lightning.md`. Branch: `feat/generate-anchor-idle`.

## Goal

Turn the Generate foundation into a real two-pass sprite pipeline: generate an
**anchor** (one neutral character on a flat magenta key), then generate an **idle
animation** from it (an 8-frame breathing loop, sliced from a 4x2 sheet, applied as
an animated sprite with an `idle` clip). The prompting fuses three sources — the
proven anchor-then-sheet scaffold from `image-extender`, the v2 Bit mascot identity,
and the animation knowledge base (`docs/animation-principles-knowledge-base/`). Bit +
idle ship as the hardcoded default until the prompt-library system lands. The real
backend is **OpenRouter** (via `openrouter-rs`); the offline mock keeps the whole
loop testable without a key.

## The pipeline (two passes)

1. **Anchor** — `build_anchor_prompt(Bit identity, AnchorSpec)` → `SubmitAnchorJob`
   → a provider with the `GenerateAnchor` capability → a still `GeneratedResult::Sprite`
   on a flat `#FF00FF` key, chroma-keyed to transparent.
2. **Idle** — `build_idle_prompt(Bit, IdleSpec, StylePreset, AnimationPrinciples)` →
   `SubmitIdleAnimationJob { from_result }` (the selected anchor is the reference
   image) → a provider with `GenerateIdleAnimation` → a 4x2 sheet, chroma-keyed and
   sliced into 8 `GeneratedFrame`s → `GeneratedResult::Animation`.
3. **Apply** — `InsertSelectedAsAnimatedSprite` → `ApplyGeneratedAnimation` through
   the history → an animated sprite (8 frames + one `idle` clip). Undo removes it.

Both passes are separate jobs; the idle pass carries the anchor as owned immutable
RGBA bytes (`ReferenceImage`), never a live document handle (bible 13.6). The
two-pass chaining is orchestrated in the intent layer, not inside one provider call.

## Where the pieces live

- **Core model** (`crates/core`): frames-first, cel-ready. A `Sprite` holds
  `frames: Vec<Frame>`, `clips: Vec<AnimationClip>`, and `active_frame`; a `Frame`
  owns the layer stack a sprite used to own directly, so a still sprite is a
  one-frame sprite and existing behavior is unchanged. `ApplyGeneratedAnimation`
  inserts a whole clip as one undoable step. `composite_frame` flattens a chosen
  frame. `DEFAULT_CANVAS_SIZE = (512, 512)`.
- **Services** (`crates/services`): `GeneratedResult { Sprite, Animation }`,
  `GeneratedAnimation`/`GeneratedFrame`, `ResultKind` (the UI mirror summary). The
  `Provider` future returns a `GeneratedResult`; capabilities gain `GenerateAnchor`
  and `GenerateIdleAnimation`. `GenerationJobInput` carries a `GenerationKind`
  (`Anchor` or `IdleAnimation { reference, animation_id, grid, fps }`). The
  `ResultStore` holds both kinds with kind-specific accessors.
- **Providers** (`modules/providers`): `postprocess::{chroma_key_magenta,
  slice_sheet}`; the `MockProvider` (deterministic anchor diamond + bobbing idle, the
  offline floor); the `OpenRouterProvider` (the real backend).
- **Prompt builder** (`modules/generation/src/prompt/`): the composable types
  (`CharacterIdentity`, `AnchorSpec`, `IdleSpec`, `StylePreset`,
  `AnimationPrinciples`), the cited knowledge-base consts (`kb.rs`), the shipped Bit
  defaults (`defaults.rs`), and `build_anchor_prompt` / `build_idle_prompt`.
- **UI** (`crates/ui` + `modules/generation`): the two-step Prompt panel (pre-filled
  with the Bit subject; Generate Anchor always, Generate Idle Animation once an
  anchor is selected), the kind-aware Results panel (frame-count badge, kind-aware
  apply), the new intents + helpers, and the read-only `result_kinds` mirror.
- **App** (`app/src/main.rs`): reads `OPENROUTER_API_KEY`, registers OpenRouter
  ahead of the mock so capability lookups prefer the real backend.

## The prompt structure and defaults

`build_*` assemble the final prompt string from data. The knowledge base is baked as
cited `&'static str` consts in `prompt/kb.rs` (not read at runtime — it is
non-runtime reference per `docs/CLAUDE.md`). The Bit defaults live in
`prompt/defaults.rs`; the prompt box is pre-seeded with `BIT_DEFAULT_PROMPT` and the
edited text becomes the identity description. The full Bit anchor and idle prompts
are pinned by the builder tests.

## OpenRouter wiring

`openrouter-rs` 0.10: `modalities=[Image,Text]` + an `image_config` aspect ratio make
the model return an image; the result comes back on `choices[*].message.images` as
base64 data URLs. The anchor is a text request; the idle pass attaches the anchor via
`Message::with_parts` + `ContentPart::image_url`. The returned PNG is base64- and
PNG-decoded, chroma-keyed, and (for animation) sliced — all in `spawn_blocking`. The
model slug defaults to a Gemini image model and is overridable via
`PIXHAUS_OPENROUTER_MODEL`. Full details: the `pixhaus-openrouter` skill.

## i18n boundary

Prompt content (identity, specs, the assembled strings, provenance) is data, stored
verbatim, never i18n keys. Panel/button labels are keyed (or literal mock content
matching the existing generate panel). The command label
`command.apply_generated_animation` and the provider label
`provider.openrouter.label` are keys in the locale bundles.

## Decisions and deviations from the plan

- **Frames-first, not full sparse cels.** The bible's layer×frame cel matrix (9.6) is
  the eventual target; a `Frame` owning the layer stack fills the boundary without
  reshaping it, and keeps still sprites byte-identical.
- **`ModelParams` dropped from the job input.** The provider owns its own
  per-kind model/temperature/aspect-ratio rather than threading them through
  `services` — keeps `services` provider-agnostic. Revisit when a model picker lands.
- **The reference image rides in `GenerationKind::IdleAnimation`, not
  `GenerationContext`.** It is owned decoded bytes (worker contract), not a live
  sprite snapshot; `GenerationContext` stays `NewAsset`. The `CurrentSprite` context
  is the documented seam for live-sprite references later.
- **No `serde` on the prompt types yet.** Deferred until the prompt-library actually
  deserializes recipes, to avoid a speculative dependency.
- **Builder tests use content assertions, not `insta` snapshots.** Lighter, no new
  snapshot files; they pin the load-bearing parts of each prompt.
- **Button labels are literal strings**, matching the existing generate panel's mock
  literals; a full i18n pass of the panel bodies is a separate cleanup.

## Done vs TODO

- [x] Canvas default 512x512
- [x] Core frames-first model + `AnimationClip` + `ApplyGeneratedAnimation`
- [x] Services result kinds + capabilities + `GenerationKind`
- [x] Providers chroma-key + slice + mock multi-frame idle
- [x] Prompt builder + Bit defaults + knowledge-base consts
- [x] Two-step Generate UI + read-only result-kind mirror
- [x] Headless two-step test (offline pipeline verified end to end)
- [x] OpenRouter provider + deps (cargo deny green) + app wiring
- [x] `pixhaus-openrouter` skill + this journal
- [ ] Live OpenRouter run (needs a key + network; the ignored integration test)
- [x] In-app playback — the Animate workspace plays the inserted animation on the
  canvas (transient playhead in `UiState`, the canvas composites the playhead frame,
  the timeline drives transport/scrub); pure frame math in `crates/ui/src/playback.rs`
- [ ] Prompt-library / recipe system (Bit is the hardcoded default until then)
- [ ] Real body plans beyond biped; walk/run/etc. choreography
- [ ] Per-layer cel data on the timeline (Band 4 tracks are still decorative); a
  `SetClipLoopMode` command to wire the Clip Properties loop checkbox

## How to run and test

- Offline, no key:
  `cargo nextest run -p pixhaus-core -p pixhaus-services -p pixhaus-mod-providers -p pixhaus-mod-generation -p pixhaus-ui`
  — the unit tests plus the two-step headless loop
  (`pixhaus-ui::generate_loop`).
- In-app (mock): `cargo run -p pixhaus-app`, Cmd+4, the prompt is pre-filled with
  Bit; Generate Anchor → select the result → Generate Idle Animation → select it →
  Insert as animated sprite.
- Real OpenRouter (manual): set `OPENROUTER_API_KEY` (and optionally
  `PIXHAUS_OPENROUTER_MODEL`) and run the ignored integration test:
  `cargo nextest run -p pixhaus-mod-providers -- --ignored`.
- Session gate: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D
  warnings`, `cargo nextest run --workspace`, `cargo test --doc --workspace`,
  `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps --document-private-items`,
  `cargo deny check`.
