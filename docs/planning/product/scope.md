# Pixhaus — product scope

## The thesis

The pixel-art world has two converged tools and one unsolved frontier. Aseprite owns sprite editing and frame animation. Tiled owns tilemap design. Both are excellent. Both end where the other begins, and both predate the AI capabilities that have arrived in the last 24 months. Pixhaus is the open-source, AI-native unification of those two domains, with AI verbs as first-class commands instead of bolted-on side panels.

The hand stays on the canvas. The artist is still the artist. The AI is the apprentice that handles the toil.

## One-liner

Pixhaus is the open-source AI-native pixel art editor for sprites, animations, and tilemaps.

## Audience

Indie game artists currently working across Aseprite + Tiled + AI generators (Scenario, PixelLab, Retro Diffusion, ComfyUI). Solo developers who can't afford a five-tool pipeline. Pixel artists who want AI leverage without losing pixel-perfect discipline. Studios that need a tool they can host, fork, and extend.

Engine target: Unity. Other engines come later or via community plugins; the in-scope build only ships Unity tooling.

## What Pixhaus is

### Pixel-perfect editing core

Frame-based timeline with onion skin, frame tags, layer groups with working blend modes (the Aseprite gap), indexed-color discipline, palette swap workflow, brush engine with pixel-perfect mode, selection, transforms, symmetry, references. Read and write `.aseprite` / `.ase` files for direct compatibility with existing artist libraries. Read Photoshop `.psd` for import. Sprite sheet export with JSON metadata in the Aseprite-compatible format that Unity importer packages already consume.

If an Aseprite user opens Pixhaus and feels lost, the editing layer has failed. The pixel-art editing surface is ground-truth-equivalent before anything AI-native ships on top.

### Tilemap as a first-class layer type

Tilemaps live inside the same project file as the sprites that compose them. Tilesets are layers that paint tile indices instead of pixels. Autotile rules — Wang corner-blob, Wang edge-blob (47-tile), and rule-based custom sets — are configured in-tool, not in a separate Tiled session. The tilemap renders adjacent to the sprite the player will walk through. Tile editing, sprite editing, and animation share one selection model, one undo stack, one project.

Closing the Aseprite-Tiled split is the second-largest non-AI bet of the project.

### Animation timeline that includes tiles

Animated tiles (water, lava, conveyor belts) are first-class. A tile in a tilemap layer can hold an animation tag and play independently of the sprite timeline. Tile animation is currently a workaround in every tool surveyed. Pixhaus treats it as a built-in.

### AI verbs, in canvas, with project context

Generation never happens in a side panel that ignores the rest of the project. AI commands run with the active palette, the existing layer stack, the chosen reference frames, and the project's style examples as constraints. The full verb set:

- **Inbetween** — generate intermediate frames between two key frames, palette-locked.
- **Continue** — given the last N frames, generate the next 1-3 frames consistent with motion and style.
- **Extend** — generate alternate views (4-direction, 8-direction) of the active sprite from a single drawing.
- **Variant** — generate palette swaps, equipment overlays, expression sets as derived layers from a base.
- **Cleanup** — snap a generated or imported sprite to the active palette, remove sub-pixel anti-aliasing, fix pivot drift.
- **Tile** — generate a 47-blob autotile set from 1-3 example transitions.
- **Critique** — vision-language analysis of a sprite or animation: pose continuity, palette violations, missing frames, pivot drift, style inconsistency.
- **Project style learning** — train a per-project LoRA from existing layers; subsequent verbs use it as a baseline style reference.
- **Conversational editing** — natural language driving multi-step editor commands ("make this enemy look angrier, add a scar over the left eye, slow the walk to 8fps").
- **Motion-from-video** — extract pose timing from a reference video into the timeline.
- **Auto-mesh-deformation** — derive a Live2D-style deformation rig from a single sprite without explicit bones.
- **Audio-driven timing** — beat detection and lip-sync to drive animation timing.
- **Tileset-from-description** — generate a complete autotile-compatible tileset from a prompt.
- **Sketch finishing** — refine rough silhouettes / gesture sketches into finished sprites in project style.

Each verb is a command palette entry. Each runs against project context. Each produces a non-destructive layer the artist can accept, edit, or reject. None of them autocomplete while the artist is drawing.

The architecture-level treatment of these verbs lives in the AI runtime stream (S21) and the verb plugin protocol (B5). Each verb is implemented as a stream in `../work/streams.md` (S23-S36).

### Scripting

Lua, matching Aseprite's surface so existing scripts have a migration path. Hot-reload. The plugin system reaches further than Aseprite's: custom UI panels, custom tools, custom AI verbs. Any AI capability beyond the built-in set should be implementable as a plugin without forking the editor.

### Engine handoff

JSON sprite sheet export in the Aseprite-compatible schema (frame rectangles, tags, durations, slices) — Unity's existing Aseprite importer packages consume this directly. Tilemap export to Tiled `.tmx` for projects that route tilemaps through SuperTiled2Unity. A first-party Pixhaus Unity package importer (S39) reads these formats with a cleaner API than the third-party Aseprite importers and provides Pixhaus-specific helpers.

## What Pixhaus is not

- **No skeletal rigging.** Out of scope. Bones are a different mental model. The Spine / Live2D / DragonBones territory is well-served, and a unified pixel + skeletal tool is a different product. Mesh deformation arrives via the auto-mesh-deformation verb (S33) — that's the closest no-bones path to skeletal-class results.
- **No vector tools.** Pixhaus is raster-first. Vector workflows belong to Adobe Animate, Synfig, Moho.
- **No 3D layer support.** Voxel and pseudo-3D pixel work routes to specialized tools.
- **No real-time multiplayer editing.** Single-user. Collaboration is a future engineering effort, not a scope item now.
- **No proprietary file format.** Pixhaus reads and writes existing formats. The native `.pixhaus` project file is open and documented from day one (B3).
- **No subscription tier.** Open source means open source. No "Pro" features held back. No license server.
- **No engine support beyond Unity** in the in-scope build. Godot, Unreal, GameMaker can be community plugins; the team's Unity importer is the only first-party engine integration.
- **No mobile target.** Desktop-only — Windows, macOS, Linux. iPad and web are deferred indefinitely.

## Positioning against the field

| Versus | What Pixhaus does differently |
|---|---|
| Aseprite | Open source. Tilemap as first-class. AI verbs in canvas. Layer group blend modes that actually work. |
| Pixelorama | More opinionated AI surface. Aseprite-compatible file format. Real plugin ecosystem with custom tools and panels. |
| Tiled / LDtk | Tilemap lives inside the sprite project, not in a separate tool. Tileset and tilemap edit in the same window. |
| Scenario / Layer / PixelLab | AI is a verb on the artist's canvas, not a generator with a refine-then-export workflow. Project context is automatic. |
| Retro Diffusion | Deeper editor; AI is one capability in a full tool, not the whole product. |
| ComfyUI | UX is for artists, not for ML practitioners. Workflow graphs are a power-user mode, not the main interface. |

## What "AI-native" actually means here

Not: an AI button in every menu, generation as canvas autocomplete, text-to-finished-sprite as the primary workflow.

Yes: every AI command receives project context (palette, existing layers, style references, animation history) by default. AI output is always a non-destructive layer the artist accepts or rejects. The artist's brush is sacred — AI doesn't paint while they paint. Generation produces starting points; refinement is the artist's job and the editor makes it fast.

The architecture supports multiple inference backends: bring-your-own API key (Anthropic, OpenAI, Replicate, Stability), self-hosted (Ollama, ComfyUI, vLLM). None of them are the only option. Local-first is the design preference; cloud-first is the configurable default for users who don't have a GPU.

## Tech stack (locked)

Tauri 2.x with a Rust workspace and a TypeScript + Solid.js UI. WebGL2 viewport. MessagePack + zstd for the project file. Lua scripting via `mlua`. AI inference behind a backend abstraction with adapters for Anthropic, OpenAI, Replicate, Ollama, ComfyUI, and Stability. MIT license. Unity 2022.3 LTS minimum target for the importer.

Full detail: `../architecture/stack.md`. Why Rust over Electron: `../architecture/rust-vs-electron.md`.

## Open source posture

License: MIT. Pixelorama is MIT. LibreSprite is GPLv2. MIT maximizes adoption — it lets studios use the tool internally without copyleft obligations and lets companies ship plugins without the GPL viral concern. The cost is that someone could build a closed-source commercial fork. The benefit is that the open ecosystem grows faster.

Governance: BDFL while the contributor count is small. Move toward an open governance model once that's no longer the right shape.

Sustainability: Open Collective or GitHub Sponsors for funding. No paid tier. No "Pro" features held back. Sustainability comes from sponsorships and grant funding (the Krita and Blender models).

## How the work happens

The build is structured for parallel agent execution. The bedrock specs (`../work/bedrock.md`) lock cross-stream contracts. The 52 work streams (`../work/streams.md`) fan out from there, each with an agent brief ready to dispatch. There's no v1/v2/v1.5 phasing — features land as their streams complete. The first usable internal build comes when the critical-path streams complete.

If you want a starting point for "what to dispatch first," the order is in `../work/streams.md` under "How to dispatch."
