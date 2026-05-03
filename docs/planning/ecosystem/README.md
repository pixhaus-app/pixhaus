# Pixhaus ecosystem reference

Six documents covering every Rust crate, JavaScript library, and Rust convention the Pixhaus build will lean on. Researched in May 2026 against current crates.io, docs.rs, npm, and GitHub state. Not a snapshot of training-data assumptions.

This folder is the input the build streams in `../work/streams.md` consume when picking dependencies, and the input to any per-domain skills we create later.

## The documents

| # | Document | Scope | Words |
|---|---|---|---|
| 01 | [Foundations](01-foundations.md) | Tauri 2.x ecosystem, async (tokio, rayon), serde, error handling, networking, observability, system APIs, compression | ~8,200 |
| 02 | [Graphics and formats](02-graphics-and-formats.md) | image processing, GPU (wgpu, vello), color, fonts, image formats, sprite-art parsers (Aseprite, PSD, Tiled), geometry math | ~7,000 |
| 03 | [AI and ML](03-ai-ml.md) | Local inference (Candle, Burn, ORT, Mistral.rs), LLM API SDKs, image generation, audio analysis, pose estimation | ~2,300 |
| 04 | [Scripting and testing](04-scripting-and-testing.md) | mlua, wasmtime/extism, plugin systems, testing (nextest, rstest, proptest, insta), benchmarks, fuzzing, CI tooling | ~6,500 |
| 05 | [Frontend and AV](05-frontend-and-av.md) | Solid.js ecosystem, Tauri JS plugins, WebGL2/WebGPU, canvas libraries, audio/video processing (Rust + JS sides) | ~9,400 |
| 06 | [Rust + AI best practices](06-rust-best-practices-2026.md) | Patterns and anti-patterns for AI-agent-driven Rust development. Manifesto, not a crate reference. | ~8,400 |

Total: ~41,800 words across six documents.

## How to use this folder

When dispatching a stream from `../work/streams.md`, the agent brief should reference the relevant ecosystem doc(s). Example:

> "Implement S07 (`.pixhaus` native format). Crate choices are documented in `ecosystem/01-foundations.md` (compression, serialization) and `ecosystem/02-graphics-and-formats.md` (geometry types). Read both before starting. Conventions live in `ecosystem/06-rust-best-practices-2026.md`."

That single sentence gives the agent the entire dependency context it needs without the brief carrying the weight.

## Findings worth flagging

A handful of things in this research changed the build plan or surfaced as risks.

### Crates that need to be written

The research found four meaningful gaps where Pixhaus needs to write its own crate or wrapper, since the open-source ecosystem doesn't have a maintained option. Each is small (a few hundred lines) but each is now an explicit work item:

1. **Anthropic Rust SDK** — Anthropic publishes Python and TypeScript SDKs but no Rust SDK as of May 2026. We write a lightweight client over `reqwest` against their REST API. Add as stream item or sub-task of S22 (backend adapters).
2. **ComfyUI Rust client** — submitting workflows to a local ComfyUI server via HTTP. No maintained Rust client exists. Add as sub-task of S22.
3. **Aseprite parser** — the existing `aseprite` and `aseprite-loader` crates are either JSON-only or Bevy-tied. We need a maintained binary `.aseprite` reader/writer crate as stream S08. Consider extracting it as a standalone open-source crate the community can use.
4. **Tool-use orchestration** — Rust has no equivalent of LangChain/LangGraph in production state. The Pixhaus verb runtime (S21) is itself this layer. Build it for our own use; consider open-sourcing once stable.

### Crates that need watching

- **wgpu** is pre-1.0 and breaking changes still happen. Pin versions carefully. The W3C WebGPU spec ships in browsers in 2026 but Tauri's webview support depends on the OS WebView version.
- **vello** (Linebender's GPU 2D vector renderer) is production-ready in 2026 but still evolving. Useful for vector path rendering if we ever need it; not on the critical path.
- **bincode** was marked unmaintained in early 2025 (RUSTSEC-2025-0141). Migrate to `postcard` if we used it (we don't — we picked `rmp-serde`, but worth knowing for any agent reaching for it).
- **rlua** is effectively dead — archived and now a thin wrapper around `mlua`. Always reach for `mlua`.
- **The Tauri 3.0 horizon is vapor.** The team is focused on 2.x mobile/UX work. Pixhaus can commit to 2.x through 2026 without migration anxiety.

### Tooling decisions locked

The ecosystem research surfaced clear winners for several tooling questions left open in the architecture doc:

| Question | 2026 answer |
|---|---|
| TS bindings from Rust | `tauri-specta` for typed IPC commands; `ts-rs` for plain data types. Both can coexist. |
| Rust→TS error types | `thiserror` for library errors (in `core/`, `io/`, `ai/`), `anyhow` for application code (in `app/`). |
| Logging | `tracing` everywhere, with `tracing-subscriber` for output. `log` is legacy; bridge old libs via `tracing-log`. |
| Testing runner | `cargo-nextest` is the de facto default for new projects. Wire it into CI from day one. |
| Snapshot testing | `insta` for text snapshots, `image-compare` for image diffs (better than `pixelmatch-rs` for anti-alias tolerance). |
| Property tests | `proptest` (quickcheck is unmaintained). |
| Code coverage | `cargo-llvm-cov` (cross-platform; tarpaulin is Linux-only ptrace-based). |
| Plugin system | `extism` for the cross-language WASM plugin standard; `wasmtime` if we need lower-level control. |
| Async traits | Native `async fn` in traits is stable (1.75+); use it for static dispatch. `async-trait` is still required for dynamic dispatch (`dyn Trait`). |
| MessagePack lib | `rmp-serde`. |
| Compression | `zstd` for project file payloads. `flate2` only where PNG/GZIP compatibility forces it. |

### Concerning patterns

The Rust AI/ML ecosystem is meaningfully behind Python. There's no production-grade equivalent of LangChain, LangGraph, or LlamaIndex in 2026. The Pixhaus verb runtime (S21) and the verb plugin protocol (B5) are effectively building the orchestration layer Rust doesn't have. That's a strategic reality to plan around — we can't import what doesn't exist, but we also can't sit on closed-source work indefinitely. Open-sourcing the verb runtime as a standalone crate once it's stable is worth thinking about.

### License risk audit

Open-source licensing matters for an MIT project. Crates and libraries that introduce non-MIT-compatible licenses surface here:

| Library | License | Pixhaus impact |
|---|---|---|
| ffmpeg.wasm | LGPL | Avoid for browser-side video; use WebCodecs API or push to Rust side. |
| ffmpeg-next (Rust) | LGPL via libffmpeg | Use dynamic linking only; document compliance steps. |
| OpenCV | Apache-2.0 (since 4.5) | OK. |
| aubio-rs | GPL | Avoid. Push beat detection to JS-side `meyda` (MIT) or Rust-side `dasp + rustfft`. |
| essentia.js | AGPL | Avoid. Same workaround. |
| Skia | BSD | OK. |
| SDL2 | zlib | Not used. |

The two real workarounds: ffmpeg (link dynamically and document) and audio analysis (use `meyda.js` MIT browser-side or `dasp + rustfft` Rust-side instead of LGPL/GPL alternatives).

## What needs skills

The user flagged that "we may need to create a lot of skills for this work." Skills (per the Claude convention — folder with a `SKILL.md` describing patterns and conventions) are how we encode domain knowledge that agents reach for repeatedly.

Suggested skills to author once the bedrock is in place:

1. **`pixhaus-rust-conventions`** — distillation of `06-rust-best-practices-2026.md` into agent-actionable patterns. The handbook is the human-readable version; the skill is the agent-actionable version.
2. **`pixhaus-tauri-patterns`** — specific to Tauri command shapes, IPC patterns, state management, event emission. Heavily referenced from `01-foundations.md`.
3. **`pixhaus-image-processing`** — image, imageproc, fast_image_resize, blend modes, palette ops. The Rust core editor's daily bread.
4. **`pixhaus-aseprite-format`** — the `.aseprite` binary format details, chunk types, our compatibility level. Used by the S08 stream and any future format-related work.
5. **`pixhaus-verb-protocol`** — how to author an AI verb plugin. Used by streams S23-S36 and any future verb work.
6. **`pixhaus-testing-conventions`** — distilled from `04-scripting-and-testing.md`. Test patterns, snapshot conventions, visual regression workflow.
7. **`pixhaus-solid-ui`** — Solid.js conventions for our UI stream. State management, command palette, panel patterns.
8. **`pixhaus-ai-backend-adapter`** — how to add a new AI inference backend (Anthropic, OpenAI, Replicate, Ollama, ComfyUI, Stability). Used when extending S22.

Each skill should be 200-500 lines, focus on actionable patterns the agent can apply directly, and reference the source ecosystem doc for context. Authoring them is its own work stream — call it S53 if we want it explicit in `../work/streams.md`.

The order to author them: conventions and Tauri patterns first (every other stream uses these), then the domain-specific ones in the order their streams come online.

## Cross-references

- Architecture decisions: [`../architecture/`](../architecture/)
- Product scope: [`../product/scope.md`](../product/scope.md)
- Work streams: [`../work/streams.md`](../work/streams.md)
- Bedrock specs: [`../work/bedrock.md`](../work/bedrock.md)
- AI verb capability map: [`../product/ai-2026.md`](../product/ai-2026.md)

## When to refresh this research

The Rust ecosystem moves fast. Significant churn happens in 6-12 month cycles. Plan to refresh this folder at:

- Project kickoff (lock the dependency graph)
- 6 months in (catch ecosystem changes before they bite)
- Pre-1.0 release (final dependency audit)

Each refresh should re-run the same six research streams against the current state. The structure can stay; the content gets updated.
