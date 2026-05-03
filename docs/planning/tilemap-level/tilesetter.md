# Tilesetter

## Quick facts
- Vendor / maintainer: LED (solo developer)
- License / pricing model: Commercial (one-time purchase)
- Price point (current): $12.99 USD minimum
- Platforms: Windows, macOS, Linux
- First released: 2019
- Last meaningful update: 2024 (Tilesetter 3 with isometric support)
- Source available: No (proprietary)
- Primary use case: Automatic tileset generation and autotile set composition

## Origin and purpose

Tilesetter solves a specific problem: creating complete autotile sets from a single base tile. Traditional autotile workflows require hand-painting 16, 27, or 47 tiles to cover all neighboring combinations. Tilesetter automates this, generating a full set from one or a few base tiles.

The tool is designed for artists who want to create tilesets quickly without manual tile composition. It bridges the gap between sprite creation (a single base tile) and level editor use (a complete autotile set).

## Tile editing capabilities

Tilesetter is not a tilemap editor. It's a tile generation and composition tool:

- **Import a base tile** — Provide one or more base tile images (PNG).
- **Configure autotile parameters** — Specify tile size, desired set size (16-tile, 3x3, 47-tile, etc.).
- **Generate variations** — Tilesetter composites the base tiles automatically, creating transitions and corners.
- **Visual preview** — See the generated tileset before export.
- **Export autotile set** — Output a complete tileset PNG ready for use in Tiled, LDtk, or game engines.

There is no frame-by-frame animation support; Tilesetter focuses on static tiles.

## Autotile / wang tile / rule tiles

Tilesetter's core function is autotile set generation:

**Supported autotile formats:**

- **16-tile set** — Traditional 2x2 corner autotiles (grass/water boundaries, 4 cardinal corners).
- **3x3 autotiles** — Godot's autotile format (9 tiles covering all neighborhood patterns).
- **47-tile set** — Complete blob tiling set (covers all edge and corner combinations).

**How it works:**

1. Artist provides a base tile and optionally "detail" tiles (edges, corners).
2. Tilesetter analyzes the pixel patterns and identifies edges and corners.
3. Tilesetter composites variations: full tile, edges (top, bottom, left, right, corners), and transitions.
4. Output is a complete tileset PNG in the desired format.

**Smart composition** — Tilesetter "fixes" tiles as the artist draws, updating the tileset automatically. This is different from manual tile painting; it's generative.

## Multi-layer maps

Tilesetter is not a map editor. It has no layer system. It only generates tilesets.

Level designers use Tilesetter's output in Tiled, LDtk, or other tilemap editors, which provide multi-layer support.

## Tile atlas integration

Tilesetter generates a tileset sprite sheet (PNG) ready for use:

- **Output format** — Standard sprite sheet with tiles in a grid.
- **Metadata** — Tilesetter does not generate Tiled .tsx files or other metadata formats. Output is a raw PNG image.
- **Game engine integration** — Artists import the PNG into a tilemap editor (Tiled, LDtk) and configure tilesets normally.

This is straightforward but requires a second step (configuring the tileset in the target editor).

## Export and import

**Import** — Tilesetter imports PNG or JPG images as base tiles.

**Export** — Outputs a PNG sprite sheet (autotile set).

**Export to level editors** — No direct export to Tiled or LDtk. The PNG output is imported as a custom tileset into those editors.

Tilesetter focuses on tile generation, not level design integration.

## Scripting and extensibility

Tilesetter has no scripting or plugin system. Configuration is entirely GUI-based:

- Set tile size, target autotile format, blending options.
- Adjust generation parameters (edge detection sensitivity, etc.).

Extensibility is not a goal; Tilesetter is a focused tool for one task.

## How it fits the asset pipeline

Typical workflow:

1. Artist creates a single "base" tile (or a few variants) in Aseprite or Photoshop.
2. Artist imports the tile into Tilesetter.
3. Artist configures autotile format (16-tile, 3x3, 47-tile).
4. Tilesetter generates a complete autotile set PNG.
5. Artist imports the PNG into Tiled or LDtk as a custom tileset.
6. Level designer uses the tileset in maps.

Tilesetter sits early-to-mid in the pipeline: after individual sprite creation, before level design.

## Workflow strengths

- **Time savings** — Generating a 47-tile set manually takes hours; Tilesetter does it in minutes.
- **Automatic "smart fixes"** — Tilesetter adjusts tiles as the artist paints, preventing manual touch-up.
- **Multiple autotile formats** — Supports 16-tile (traditional), 3x3 (Godot), and 47-tile (comprehensive).
- **Isometric support** — Tilesetter 3 added isometric tile generation.
- **Integration with level editors** — Output PNG works in any editor.
- **Visual feedback** — Real-time preview of the generated tileset.

## Workflow gaps

- **Not a level editor** — Only tile generation; you need Tiled, LDtk, or similar for actual level design.
- **Limited customization** — Generation parameters are basic; cannot fully control tile composition.
- **No metadata export** — Does not generate Tiled .tsx or other metadata; you configure tilesets manually in the editor.
- **No animation support** — Only static tiles; animated tiles must be created separately.
- **Cost** — $12.99 USD minimum; not free like Tiled or LDtk.
- **Single developer** — Smaller support and update cycle than larger projects.

## Notable uses

- **Indie 2D platformers** — Used by developers who want quick tileset generation.
- **Game jams** — Popular for rapid prototyping (if purchased beforehand).
- **Educational projects** — Some game development courses use Tilesetter to teach autotiling concepts.

## Community and ecosystem

Tilesetter has a smaller community than Tiled or LDtk:

- Official documentation and tutorials.
- Community posts on itch.io and game dev forums.
- No large community plugin ecosystem (the tool is focused; plugins are not relevant).

Support is responsive, though smaller in scale.

## Pricing details

- One-time purchase: $12.99 USD minimum (pay-what-you-want above minimum).
- Available on itch.io and Steam (if released to Steam).
- No subscription; purchased once, keep forever (though updates may require repurchase at major versions).

## Version history

Tilesetter 1.0+ released in 2019. Tilesetter 2.0 improved the UI and generation quality. Tilesetter 3 (2024) added isometric support and further refinements.

Current version is stable and actively updated.

## Interaction with related tools

- **Tiled** — Tilesetter's output is used in Tiled as a custom tileset. Manual configuration of terrain sets or Wang tiles required after import.
- **LDtk** — Similarly; import Tilesetter's output as a custom tileset.
- **Aseprite, Photoshop** — Artists create base tiles in these tools, export to Tilesetter.
- **Godot** — Tilesetter's 3x3 format matches Godot's autotile system.

## Comparison with other tileset tools

| Tool | Purpose | Cost | Approach |
|------|---------|------|----------|
| Tilesetter | Autotile generation | $12.99 | Generative (auto-compose from base) |
| TexturePacker | General sprite sheet packing | Free/paid | Manual composition, atlas packing |
| Tiled | Tilemap editing + terrain system | Free | Manual + terrain-based autotiles |
| LDtk | Tilemap editing + autolayers | Free | Manual + visual rule-based autolayers |

Tilesetter is specialized; use it when you need autotile generation, then use Tiled/LDtk for level design.
