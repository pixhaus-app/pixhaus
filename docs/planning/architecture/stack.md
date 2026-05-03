# Pixhaus stack — locked decisions

These are the technical choices the work organization assumes. They're locked, not provisional. If any of them change, the streams in `work/streams.md` need to be redrawn.

## The lock-in

| Layer | Choice | Reasoning |
|---|---|---|
| Core language | Rust | Image manipulation performance, memory layout, file I/O. See `rust-vs-electron.md`. |
| UI runtime | Tauri 2.x | Native webview, ~10MB binary, idiomatic Rust↔TS bridge |
| UI framework | TypeScript + Solid.js | Reactive, fine-grained updates, small bundle. Svelte is the alternate. React is too heavy. |
| Canvas rendering | WebGL2 (browser side) + Rust compositing | Hot pixel paths in Rust, GPU-accelerated viewport in WebGL2. WebGPU when stable enough. |
| Async runtime (Rust) | Tokio | Standard. Used by Tauri internally. |
| Data parallelism (Rust) | Rayon | For per-frame, per-tile, per-layer parallel work |
| Image library | `image` crate + custom inner core | `image` for format support, custom code for hot paths |
| Project file format | MessagePack-encoded with zstd compression | Schema-evolvable, fast, compact, library support in both Rust and TS |
| Aseprite compat | Custom parser (binary `.aseprite` format is documented) | Standalone Rust crate, contributable upstream |
| Scripting | Lua via `mlua` crate | Aseprite parity. Familiar to existing pixel artists. |
| AI inference | Backend-abstracted via async trait | Anthropic, OpenAI, Replicate, Ollama, ComfyUI as adapters. No vendor lock-in. |
| Engine target | Unity (only) | Per scope decision. One importer package, not five. |
| License | MIT | Maximum adoption, plugin ecosystem friendliness |
| OS support | Windows, macOS, Linux | Tauri targets all three |

## Repo structure

Single git repo. Cargo workspace at the root, npm/pnpm workspace inside `ui/`. Unity package lives in a separate folder that's published independently to OpenUPM.

```
pixhaus/
├── Cargo.toml                  # workspace root
├── package.json                # ui workspace root
├── pnpm-workspace.yaml
├── README.md
├── LICENSE                     # MIT
├── core/                       # Rust core, the main crate
│   ├── Cargo.toml
│   ├── src/
│   │   ├── lib.rs
│   │   ├── canvas/             # pixel buffer ops, blend modes, transforms
│   │   ├── color/              # palette, indexed mode, color math
│   │   ├── frames/             # animation timeline data
│   │   ├── layers/             # layer hierarchy, groups, masks
│   │   ├── tilemap/            # tile layer, autotile rules
│   │   ├── selection/          # selection model
│   │   ├── undo/               # command pattern undo stack
│   │   └── project/            # project file model
│   └── tests/
├── io/                         # Rust crate, file format support
│   ├── Cargo.toml
│   └── src/
│       ├── pixhaus/            # native .pixhaus format
│       ├── aseprite/           # .aseprite read/write
│       ├── psd/                # .psd import
│       ├── png/                # PNG sequence + sprite sheet
│       └── tiled/              # .tmx export
├── ai/                         # Rust crate, AI verb runtime
│   ├── Cargo.toml
│   └── src/
│       ├── runtime/            # verb dispatch, async orchestration
│       ├── backends/           # Anthropic, OpenAI, Replicate, Ollama, ComfyUI
│       ├── verbs/              # built-in verbs (inbetween, continue, etc.)
│       └── plugin/             # plugin protocol implementation
├── scripting/                  # Rust crate, Lua bindings
│   ├── Cargo.toml
│   └── src/
├── app/                        # Tauri shell crate (binary entry point)
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   └── src/
│       └── main.rs             # IPC commands, window setup
├── ui/                         # TypeScript UI (Solid + Vite)
│   ├── package.json
│   ├── vite.config.ts
│   ├── src/
│   │   ├── main.tsx
│   │   ├── shell/              # window chrome, command palette, theming
│   │   ├── canvas/             # WebGL2 viewport, brushes, selection UX
│   │   ├── timeline/           # animation timeline UI
│   │   ├── tilemap/            # tile editor UI
│   │   ├── palette/            # palette panel
│   │   ├── layers/             # layer panel
│   │   ├── ai/                 # AI verb invocation UI
│   │   └── lib/                # shared TS utilities
│   └── public/
├── unity/                      # Unity package (separate publish target)
│   ├── package.json
│   ├── Editor/
│   ├── Runtime/
│   └── Samples~/
├── docs/                       # User docs (mdbook or astro)
│   └── ...
├── plugins/                    # Sample plugins (verbs, scripts)
│   └── ...
├── examples/                   # Sample projects, fixtures
│   └── ...
└── scripts/                    # Build, release, dev tooling
    └── ...
```

## IPC and data flow

UI → Rust: Tauri commands (named async functions). UI calls `await invoke("draw_stroke", { args })`. Rust returns serialized result.

Rust → UI: Tauri events. Rust emits `app.emit("project_changed", payload)`. UI subscribes.

Heavy data (pixel buffers, large layers): never serialized through the IPC bridge. The Rust side keeps pixel data in memory and the UI side gets opaque handles plus rendered tiles for display. The canvas is rendered in Rust to a shared texture and presented in WebGL2 — no per-stroke JSON round-trips.

## AI inference architecture

The AI runtime is its own Rust crate (`ai/`). It exposes a `VerbRuntime` trait with:

- `register_verb(verb_descriptor)` — declarative verb registration
- `invoke(verb_name, context, inputs) -> stream<output>` — async, streaming where backends support it
- `available_backends() -> [BackendDescriptor]` — discovery

Backends implement an `InferenceBackend` trait. Built-in adapters: Anthropic (Claude), OpenAI, Replicate, Ollama, ComfyUI. Plugin-defined verbs can declare their backend requirements; the runtime resolves to whatever the user has configured.

Project context (palette, layers, frames, style references) is serialized into a verb input payload by a context builder that lives in `core/`. Verbs receive this payload plus their specific arguments. Outputs come back as new layers or modifications which the editor applies as non-destructive operations on the undo stack.

## What's deliberately not on the list

- No SQLite, no embedded database. The project file is the source of truth.
- No proprietary cloud services. Pixhaus does not phone home. AI backends are opt-in by user config.
- No telemetry by default. Crash reports if the user opts in, that's it.
- No bundled Chromium. Native webview only.
- No Electron compatibility shim. Tauri only.
- No "Pro" tier. No license server. No phoning home for entitlement.

## Versions

Rust: stable channel, MSRV pinned in `rust-toolchain.toml` (start at 1.82, advance with the project)
Tauri: 2.x (latest stable at start of build)
TypeScript: 5.x
Solid: 1.x
Node/pnpm: pnpm 9, Node 20 LTS
Unity: 2022.3 LTS minimum (and 6.0+ as primary target)

## What needs to be in place before any stream starts

Listed in `../work/bedrock.md`. The TL;DR: this stack lock + repo skeleton + the spec contracts before any feature work fans out.
