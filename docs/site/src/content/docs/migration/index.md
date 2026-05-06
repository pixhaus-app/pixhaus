---
title: Migrating from Aseprite
description: What transfers directly, what works differently, and how to feel at home in the first hour.
sidebar:
  order: 1
---

import { Aside, LinkCard, CardGrid } from "@astrojs/starlight/components";

You've used Aseprite. Most of what you know transfers. This guide focuses on
what's different, what's better, and where to look when something doesn't
behave the way you expect.

<CardGrid>
  <LinkCard title="Keybind comparison" href="/migration/keybinds/" description="Side-by-side table of Aseprite vs. Pixhaus shortcuts." />
  <LinkCard title="Porting Aseprite scripts" href="/migration/scripting/" description="Walk through Color Reduction, Outline, and Sprite Sheet Generator diffs." />
</CardGrid>

## What's the same

The core editing model is intentionally familiar.

**Layer system.** Layer groups, blend modes (all 18 — Normal through Divide),
opacity per layer and per cel, visibility toggle, lock toggle, linked cels.
The layer panel looks and works like Aseprite's.

**Frame timeline.** Frames left-to-right, layers top-to-bottom. Frame tags
with all four loop directions: Forward, Reverse, Ping-pong, Ping-pong
Reverse. Repeat count per tag. Frame duration in milliseconds per frame.

**Onion skin.** Previous and next frames at configurable opacity, tinted red
(past) and blue (future). Toggle with Shift+F1 (or via the timeline panel).

**Palette workflow.** 256-color indexed mode with a named palette. Palette
entries have names. Transparent color index is preserved. The palette panel
lets you click-to-pick, reorder, and lock entries. Color cycling is
supported. Indexed-mode files load with the exact same palette.

**File format.** Open `.aseprite` files directly — no import step. What
round-trips cleanly is listed in [File compatibility](#file-compatibility)
below. Save back to `.aseprite` or to the native `.pixhaus` format.

**Keyboard shortcuts.** The Aseprite keybind preset is the default when
you first launch Pixhaus. `Ctrl+Z`, `Ctrl+S`, `Ctrl+N`, zoom, pan — all
where you expect them. See [keybind comparison](/migration/keybinds/) for
the full table, including the two places Pixhaus intentionally diverges.

**Slices.** Named regions with nine-slice and pivot round-trip exactly.

**Scripting.** The Lua API mirrors Aseprite's `app` global. Common scripts
port with under 20 lines of changes. See [porting Aseprite scripts](/migration/scripting/).

## What's different

### Tilemaps are a layer type, not a separate tool

In Aseprite 1.3+ you can have tilemap layers, but the autotile tooling is
limited. In Pixhaus, tilemap layers are a first-class layer type with a
full autotile rule engine: 16-tile Wang corner-blob, 47-tile Wang edge-blob
(the blob set that matches how most autotile tilesets are distributed), and
user-defined rule sets.

You do not need Tiled. The tilemap workflow, tileset editor, and autotile
rule editor are all in Pixhaus. Unity import understands the tilemap chunks
directly.

### AI verbs are first-class commands

Pixhaus ships AI verbs as named commands in the AI menu and command palette.
These are not a side panel or an afterthought — they are the same class of
operation as Undo or Flatten Visible.

The verbs in the first release:

| Verb | What it does |
|---|---|
| Inbetween | Generates intermediate frames between two key frames |
| Continue | Predicts the next 1–3 frames from the last N |
| Extend | Generates multi-direction views from a single sprite |
| Variant | Palette swaps, equipment overlays, expression sets |
| Cleanup | Snaps to palette, removes AA artifacts, fixes pivot drift |
| Tile | Generates a 47-tile autotile set from example transitions |
| Critique | VLM analysis: pose continuity, palette violations, pivot drift |
| Sketch finishing | Refines rough silhouettes into finished sprites |

Each verb has a preview-before-commit flow. Nothing applies until you
accept. The commit goes through the undo stack, so Ctrl+Z reverses it.

Verbs work with BYO API keys (Anthropic, OpenAI, Replicate, Ollama,
ComfyUI, Stability). Configure them at `Edit > Preferences > AI backends`.
You can use local backends exclusively; nothing is sent to the cloud if you
prefer not to.

### Plugin system extends the UI, not just data

Aseprite plugins are Lua scripts that operate on the data model. Pixhaus
plugins can also register custom verbs, custom tools (new brush types,
selection algorithms), custom panels, and custom file format readers.

The plugin format is a folder with a `plugin.toml` manifest plus either a
Lua entry point or a compiled WASM module. WASM plugins can be written in
any language that compiles to WASM.

See [Plugin developer guide](/plugins/developing/) for how this works.

### Undo is a tree

Pixhaus tracks branching history: if you undo several steps and then make a
new edit, the previous future is preserved as a tree branch. You can
navigate back to it. The history panel shows the full tree.

This matches how you probably already think about exploration — it just
never throws work away.

### Unity integration is built in

Aseprite users typically export sprite sheets and import them into Unity
manually, or via a third-party Aseprite importer. Pixhaus ships a Unity UPM
package (OpenUPM-compatible) that understands `.pixhaus` exports natively:
auto-slicing, AnimationClip generation per frame tag, pivot from slice data,
and tilemap import into Unity's Tilemap system.

The export path is: `File > Export > Sprite sheet (PNG + JSON)` in Pixhaus,
then the Unity importer picks it up automatically on the Unity side.

## File compatibility

Opening an `.aseprite` file in Pixhaus and saving it back:

**Preserved exactly:**
- All raster and tilemap layer content
- Layer groups, blend modes, opacity
- Frame tags, loop directions, repeat counts
- Full palette (RGBA + per-entry names)
- Slices (bounds, nine-slice, pivot)
- Linked cels
- User data (text + color per layer/cel/tag/slice)

**Preserved with a warning:**
- ICC color profile — stripped on save (`[warn] ICC color profile
  discarded; Pixhaus operates display-referred`). If your workflow is
  display-referred sRGB (almost all sprite art is), you will see no visual
  difference.
- External tileset references — inlined on save (`[warn] External tileset
  "<name>" has been inlined`). The tile data is intact; the external file
  path is lost.
- User data properties map — the text and color fields round-trip; the
  custom properties extension used by some Aseprite plugins is dropped.

**Silently dropped:**
- Z-index on individual cels (extremely rare; only used for layer-ordering
  tricks within a single frame)
- Cel Extra float bounds (sub-pixel transform data from Aseprite's transform
  tools)
- Grid settings (editor state, not project data)
- Layer UUIDs

**Not supported in v1:**
- 16-bit and 32-bit per-channel sprites (downsampled to 8-bit on load)
- Non-square pixel ratio (warn; rendered as square)

If you need the full technical detail, see [Aseprite compatibility](/reference/aseprite-compat/).

## Tips for the first hour

**1. Load the Aseprite keybind preset — it already is.**
Pixhaus ships the Aseprite preset as the default. Check `Edit > Keybinds`
to confirm. If someone else set up the machine and changed it, select
"Aseprite" from the Presets dropdown.

**2. Open your existing `.aseprite` files directly.**
`File > Open`, select the file. No import step. The file opens in Pixhaus's
editor with all layers, tags, and palette intact. Save it back to `.aseprite`
or use `File > Save as` to convert to `.pixhaus`.

**3. Redo is Ctrl+Shift+Z, not Ctrl+Y.**
This is one of two intentional divergences from the Aseprite preset.
Pixhaus uses the cross-app standard (`Ctrl+Shift+Z`) rather than
Aseprite's `Ctrl+Y`. Your muscle memory will catch up within the first
session.

**4. The command palette (Ctrl+K) is the fastest path to any command.**
Type a fragment of any command name and it shows up. Useful while you are
relearning where things live in the menu.

**5. Tilemap layers belong to sprites, not to a separate file.**
If you have been using Tiled alongside Aseprite, your tilesets and tilemap
layouts can now live inside the same Pixhaus project. Open a tileset sprite,
create a tilemap layer, and paint — the autotile rules handle transitions.

**6. AI verbs need backend credentials to do anything.**
The AI menu is visible on first launch, but verbs show a configuration
prompt until you add at least one backend. Go to `Edit > Preferences > AI
backends`. You can add an Anthropic or OpenAI API key, point Pixhaus at a
local Ollama instance, or configure a ComfyUI server. Verbs route to the
cheapest capable backend by default.

**7. The layer panel toggle is Ctrl+Shift+L, not F7.**
The second intentional divergence. Pixhaus groups all panel toggles under
`Ctrl+Shift+*` so they form a consistent family. F7 no-ops with a hint
message pointing you to the new binding.

**8. Your palette opens as-is.**
Indexed mode palettes load from `.aseprite` files with full fidelity. Named
entries, RGBA values, transparent index — all there. The palette panel shows
the same grid layout you are used to.
