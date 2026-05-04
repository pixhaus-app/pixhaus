# Bedrock specs — what gets written before any stream fans out

The streams in `streams.md` can run in parallel only because everything they need has already been agreed on. The bedrock is that agreement. It's a small, high-leverage set of documents and scaffolding that locks shared contracts so 15+ agents can ship without colliding.

The list is intentionally short. Each item has an owner brief ready to dispatch.

## The bedrock list

| # | Spec | Depends on | Estimated agent-time | Status |
|---|---|---|---|---|
| B1 | Repo scaffold | nothing | 1 day | not started |
| B2 | Core data model (Rust + TS) | B1 | 2-3 days | not started |
| B3 | Project file format (`.pixhaus`) | B2 | 2 days | not started |
| B4 | IPC command catalog (Tauri commands) | B2 | 2 days | not started |
| B5 | AI verb plugin protocol | B2 | 3 days | not started |
| B6 | Unity handoff format | B2 | 2 days | not started |
| B7 | Aseprite format compat spec | nothing (parallel with B2) | 3 days | not started |
| B8 | Agent handbook (style, tests, contributions) | B1 | 2 days | not started |

Total wall-clock if dispatched correctly: about a week. B1 is the only true sequential bottleneck — once the repo scaffold exists, B2-B8 mostly fan out in parallel with cross-checking.

## Dependency graph

```
B1 (scaffold)
  ├─→ B2 (data model)         ←──── starts the cascade
  │     ├─→ B3 (file format)
  │     ├─→ B4 (IPC commands)
  │     ├─→ B5 (verb protocol)
  │     └─→ B6 (Unity handoff)
  ├─→ B8 (agent handbook)
  └─→ B7 (Aseprite compat spec) ← parallel with B2, doesn't block streams
```

Once B1-B6 exist, every parallel stream in `streams.md` can start. B7 informs S3 (file I/O stream) but doesn't block anyone else. B8 should ship with B1 so agents have conventions from day one.

---

## B1. Repo scaffold

**Output:** A working repo with the layout described in `../architecture/stack.md`, a Tauri app that opens an empty window, a Cargo workspace that builds, a pnpm workspace that builds, CI pipelines that lint and test on push, and a working hot-reload dev loop (`pnpm dev` should bring up the editor with HMR for the UI and `cargo run` for the Rust core).

**Acceptance criteria:**
- `cargo build --workspace` succeeds
- `pnpm install && pnpm dev` opens an empty Pixhaus window
- `cargo test --workspace` passes (with placeholder tests)
- `pnpm typecheck` and `pnpm lint` pass
- GitHub Actions CI configured for: cargo check, cargo test, cargo clippy, pnpm lint, pnpm typecheck, pnpm build
- LICENSE (MIT), README, CONTRIBUTING stubs in place

**Agent brief:**
> Create the initial Pixhaus repo scaffold. Stack is locked: Tauri 2.x, Rust workspace with crates `core`, `io`, `ai`, `scripting`, `app`; TypeScript UI in `ui/` using Solid.js + Vite; Unity package in `unity/`. Target structure is in `architecture/stack.md`. Deliverable: a clean PR-ready commit that produces a working `pnpm dev` loop opening an empty Pixhaus window, with all linting/testing/CI infrastructure in place. Use the latest stable Tauri 2 templates as starting points. No feature code — scaffold only. License MIT.

---

## B2. Core data model

**Output:** Rust types in `core/src/project/mod.rs` and TypeScript mirrors in `ui/src/lib/types/`. The types describe everything a Pixhaus project contains: project metadata, sprites, frames, layers (raster, group, tilemap), palettes, tilesets, tilemap data, frame tags, slices, animations, references. Types are serde-derive-able on the Rust side and serializable to MessagePack with a stable schema. TS types are generated from Rust via `ts-rs` or hand-mirrored.

**Acceptance criteria:**
- All types defined and documented with rustdoc
- Serde serialize/deserialize implemented
- `ts-rs` (or equivalent) generates matching TS types
- A round-trip test (Rust → MessagePack → Rust) passes for every type
- A TS-side parse test on a known fixture passes

**Agent brief:**
> Define the Pixhaus core data model in Rust under `core/src/project/`, derived in TypeScript under `ui/src/lib/types/`. The model needs to express: Project (metadata, version), Sprite (canvas size, dimensions, color mode), Layer (variants: raster, group, tilemap, reference), Frame, FrameTag (named range with loop direction), Cel (per-layer-per-frame pixel data, stored as opaque handle to a pixel buffer), Palette (indexed or RGB, with named colors), Tileset (tile size, source, tiles), TilemapData (per-cell tile index + flags), Slice (rectangular region with name, used for nine-slice and pivots), Animation (named animation, references frame range). Also: SelectionState, BrushState, CanvasState. All types must be serde-serializable to MessagePack with a versioned schema. TS mirror types must be generated automatically via ts-rs. Document each type with rustdoc. Include unit tests for round-trip serialization. The reference for what's needed is the Aseprite file format spec (`docs/ase-file-specs.md` in the Aseprite repo). Bias toward minimal-but-complete — we can extend the model later, but breaking changes hurt.

---

## B3. Project file format (`.pixhaus`)

**Output:** A spec document at `docs/file-format.md` describing the `.pixhaus` binary format. Implementation in `io/src/pixhaus/`. The format is MessagePack-encoded core data model + zstd compression for pixel buffer payloads + a small header for magic bytes, version, and feature flags.

**Acceptance criteria:**
- Spec document defines: magic bytes, version field, header structure, body encoding, compression strategy, schema evolution rules
- Read and write implementations in Rust pass round-trip tests
- Files produced are reasonable in size compared to equivalent .aseprite files
- Forward compatibility: old readers refuse to load files with unknown required features, gracefully ignore optional ones

**Agent brief:**
> Design the `.pixhaus` project file format. Constraints: MessagePack-encoded payload using the core data model (B2), zstd compression for pixel buffer data, magic byte header, version field with documented evolution rules (additive changes don't break readers, breaking changes bump major version), feature-flag bitfield for optional features. Implementation in `io/src/pixhaus/{read,write,schema}.rs`. Document the format in `docs/file-format.md` with byte-level layout, schema versioning policy, and migration strategy for breaking changes. Include round-trip tests with at least three non-trivial fixtures. Compare file sizes against equivalent .aseprite files — target within 1.5x for similar content.

---

## B4. IPC command catalog

**Output:** A Rust module at `app/src/commands/` exposing every Tauri command the UI can invoke, with input and output types pulled from the core data model. A reference document at `docs/ipc-commands.md` listing every command, its arguments, its return type, its possible errors, and its expected latency. TypeScript wrapper functions auto-generated or hand-mirrored in `ui/src/lib/commands/`.

**Acceptance criteria:**
- Every command has typed input and output
- Errors are typed (no string errors crossing the boundary)
- Latency contracts documented per command
- TS wrapper functions provide the same type safety as Rust
- A test harness can invoke every command and assert types

**Agent brief:**
> Define the Tauri IPC command catalog for Pixhaus. Commands fall into categories: project (open, save, new, close), canvas (draw_stroke, fill, transform, select), layers (add, delete, reorder, blend_mode_set, opacity_set), frames (add, delete, duplicate, reorder, tag_create), tiles (place, erase, autotile_apply), palette (add_color, remove_color, swap_palette), and verbs (invoke_verb, list_verbs, cancel_verb). Each command takes a typed input from the core data model, returns a typed result, and uses a typed error enum. Implementation lives in `app/src/commands/`. TS wrappers in `ui/src/lib/commands/`. Reference doc in `docs/ipc-commands.md` listing every command with signature, latency expectation, and side effects. Aim for completeness — adding commands later is fine, but the basic verb set should be there from day one so streams can build against it.

---

## B5. AI verb plugin protocol

**Output:** A Rust trait `Verb` in `ai/src/plugin/`, a registration mechanism, an invocation runtime, and a documented protocol at `docs/verb-protocol.md`. The protocol covers: verb declaration (name, description, input schema, output schema, required backends), context injection (project palette, layers, references), invocation (sync or streaming), preview model (verb produces a preview before commit), cancellation, cost/latency contracts.

**Acceptance criteria:**
- Trait definition with default implementations where reasonable
- Registration API: `runtime.register(MyVerb::new())`
- Invocation API: `runtime.invoke("verb_name", context, inputs).await`
- Streaming output via async streams
- Cancellation via tokio cancellation tokens
- A reference plugin (`echo` verb that returns its input as a layer) implementing the trait end-to-end
- Documentation in `docs/verb-protocol.md`

**Agent brief:**
> Design the AI verb plugin protocol for Pixhaus. The protocol must support: verb declaration with name, description, input schema (JSON Schema or equivalent), output schema, required backend capabilities (e.g., "needs vision-language", "needs image-gen"); context injection (the runtime gives the verb the active palette, layer stack, frame history, project style references); preview-then-commit flow (verb produces a preview, user accepts or rejects, accept commits to undo stack); streaming outputs (some verbs may emit progressive results); cancellation tokens; cost/latency declarations the UI can show. Implementation in `ai/src/plugin/{trait,runtime,context}.rs`. Reference doc in `docs/verb-protocol.md` with a worked example. Ship a reference plugin: an `echo` verb that takes an image input and produces an output layer with the same image, demonstrating the full lifecycle. Built-in Pixhaus verbs (Inbetween, Continue, Extend, Variant, Cleanup, Tile, Critique) will be implemented against this protocol in their own streams; the protocol must be expressive enough to support all of them.

---

## B6. Unity handoff format

**Output:** A spec document at `docs/unity-handoff.md` describing what Pixhaus exports for Unity consumption. Two artifacts: a sprite sheet PNG + JSON metadata file, and (for tilemap projects) a Tiled-compatible `.tmx` + tileset PNG. JSON schema documented and version-stamped. The Unity importer (separate stream) reads exactly this format.

**Acceptance criteria:**
- JSON sprite sheet schema documented with examples
- Schema is Aseprite-JSON-compatible where possible (so existing Unity Aseprite importers work)
- TMX export tested against Tiled and Unity's SuperTiled2Unity importer
- Reference exports in `examples/unity-handoff/` showing valid output

**Agent brief:**
> Define the Unity handoff format for Pixhaus. Two outputs: (a) sprite sheet PNG + JSON metadata describing frame rectangles, frame durations, frame tags, slices, and pivots; (b) tilemap export as Tiled-compatible TMX plus tileset PNG. Schema for (a) should be compatible with the Aseprite JSON sprite sheet format so existing Unity Aseprite importer packages can consume it without modification — borrow the schema from Aseprite's documentation, extending only as needed. Schema for (b) follows the Tiled TMX format spec. Document both in `docs/unity-handoff.md` with byte-level / line-level examples. Place reference exports in `examples/unity-handoff/`. Include a checklist of edge cases: missing frames, indexed vs RGB, multiple animations in one file, animated tiles. Test by importing the reference exports into Unity 2022.3 LTS using common importer packages — call out any gaps where our format doesn't round-trip cleanly.

---

## B7. Aseprite format compatibility spec

**Output:** A spec document at `docs/aseprite-compat.md` describing exactly which Aseprite file format features Pixhaus reads and writes. Implementation lives in `io/src/aseprite/` and is driven by a separate stream (S08); this bedrock spec only fixes the contract.

**Acceptance criteria:**
- Document lists every Aseprite file format chunk type and Pixhaus's support level: read+write, read-only, ignored
- Reference test files in `examples/aseprite-roundtrip/` demonstrate what's supported
- Known gaps documented explicitly (e.g., "tileset chunks introduced in Aseprite 1.3 are read but not written in v1")

**Agent brief:**
> Write the Aseprite file format compatibility spec for Pixhaus at `docs/aseprite-compat.md`. The Aseprite binary format is documented at https://github.com/aseprite/aseprite/blob/main/docs/ase-file-specs.md. Walk through every chunk type and decide Pixhaus's support level: full (read+write with feature parity), read-only (we read but do not write back), ignored (we read but discard, with a warning). The minimum bar for "Pixhaus opens an Aseprite file" is: layers (raster + group), frames, frame tags, palette, slices, blend modes, opacity, tileset chunks. Things we may not support immediately: linked cels (workaround acceptable), color profile chunks, user data on every entity. The goal is that 90% of indie pixel artists' .aseprite files open in Pixhaus without warnings. Document write-side compatibility separately — what does a Pixhaus-saved file look like when Aseprite opens it? Implementation lives in stream S08 of streams.md, but the contract is fixed here.

---

## B8. Agent handbook

**Output:** A document at `CONTRIBUTING.md` plus `docs/agent-handbook.md` defining the conventions every agent (and human) follows. Coding standards for Rust and TypeScript, test conventions, commit format, PR template, branch naming, code review expectations.

**Acceptance criteria:**
- Rust style follows `rustfmt` defaults plus a `clippy.toml` with project lints
- TypeScript style follows Prettier + ESLint with a documented config
- Test conventions: every public function has at least one test; integration tests live in `tests/` directories; visual regression tests use a documented harness
- Commit format: Conventional Commits
- PR template requires: what changed, why, test plan, screenshots if UI

**Agent brief:**
> Write the Pixhaus agent handbook. Two documents: (a) `CONTRIBUTING.md` for outside contributors, (b) `docs/agent-handbook.md` for AI agents working on the codebase. Cover: Rust style (rustfmt defaults, clippy lint set, error handling philosophy with `thiserror` + `anyhow`, async patterns with tokio, no unwrap in production code paths), TypeScript style (Prettier config, ESLint rules, no any, strict null checks, Solid.js idioms), test conventions (unit tests inline, integration tests in `tests/`, visual regression via a screenshot diff harness to be defined), commit format (Conventional Commits), branch naming (`feat/`, `fix/`, `chore/`), PR template, code review expectations. The agent handbook section should additionally cover: how to read existing code before writing new code, how to verify changes with the test suite, how to surface uncertainty rather than guess, and the priority order for resolving conflicts (correctness > readability > performance > novelty).

---

## What happens after bedrock

When B1-B6 are merged and B7-B8 are at draft quality, dispatch the streams in `streams.md`. The streams are designed to consume the bedrock outputs as their stable inputs, so they don't need to coordinate with each other beyond minor cross-references.
