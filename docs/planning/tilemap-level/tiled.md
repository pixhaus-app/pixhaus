# Tiled Map Editor

## Quick facts
- Vendor / maintainer: Thorbjørn Lindeijer (open-source community)
- License / pricing model: GPL 2.0 (free, with optional donation model on itch.io)
- Price point (current): Free (donations accepted)
- Platforms: Windows, macOS, Linux
- First released: 2008
- Last meaningful update: 2024 (1.12+)
- Source available: Yes (GitHub, full source code)
- Primary use case: Orthogonal, isometric, and hexagonal tilemap creation with extensive autotile support

## Origin and purpose

Tiled emerged as the open-source standard for tilemap editing, designed to be engine-agnostic and extensible. It handles orthogonal (2D grid), isometric, and hexagonal maps with support for multiple layer types, tilesets, and custom properties.

The tool is built by a single maintainer with community contributions, making it sustainable and transparent. Tiled has become the de-facto industry standard for indie game development and is widely integrated into game engines.

## Tile editing capabilities

Tiled provides a visual tile painter with several brush types:

- **Stamp (single-tile) brush** — Click individual tiles to place them.
- **Bucket fill** — Fill contiguous regions with a single tile.
- **Rectangle and polygon selection** — Select an area and fill or clear it.
- **Eraser** — Remove tiles.
- **Random tiles** — Place random tiles from a weighted set (useful for variation).

The UI is straightforward; no pixel-level editing (tiling assumes pre-made tiles in a spritesheet).

Tiles are selected from a Tileset panel showing a sprite sheet. You can select individual tiles or "stamped" patterns (pre-composed tile arrangements).

## Autotile / wang tile / rule tiles

Tiled's autotile system has evolved significantly:

**Terrain-based autotiling** — Define terrain types (grass, water, sand) and Tiled automatically generates transitions at boundaries.

- A terrain set specifies which tiles match which terrain type.
- When you paint grass next to water, Tiled chooses the corner/edge transition tile automatically.
- Supports 16-tile sets (traditional 2x2 corners), edge-based sets, and custom configurations.

**Wang tiles** — A named tiling system where each tile is labeled with colors at its edges and corners. Tiled matches tiles based on these labels.

- Wang sets can be "Edge" (tiles match at edges), "Corner" (match at corners), or "Mixed".
- The Wang Brush visually shows overlays matching each configuration.
- Recent updates (1.10+) unified terrain and Wang into a single flexible system.

**Rule tiles** — Custom rule-based tile selection. You define conditions (e.g., "if grass is above and water to the left, use this tile") and Tiled applies them.

- Powerful for complex transitions but requires manual setup.
- Less common in indie games but essential for AAA-level tilemap polish.

**Stamp patterns** — Pre-drawn tile compositions (e.g., a 3x3 tree or a 2x2 water pond) that can be placed as a single unit.

## Multi-layer maps

Tiled supports multiple layer types:

**Tile Layers** — Standard tile grids for terrain, collision, decoration.

**Object Layers** — Free-form placement of rectangles, circles, polygons, and text for level design metadata (spawn points, doors, hazards).

**Image Layers** — Background parallax, decoration images placed at arbitrary positions.

**Group Layers** — Organize layers hierarchically.

Layers have visibility toggles, opacity control, and blend modes. You can lock layers to prevent accidental edits.

## Tile atlas integration

**Tileset definition** — A Tileset is a Tiled asset referencing a sprite sheet:

- Specify the image, tile size, spacing, and margin.
- Tiled automatically detects tile boundaries (with configurable offset).
- Assign tiles to terrain types or Wang sets.
- Define custom properties per tile (collision, animation, etc.).

**Tileset sources** — Tiled supports multiple tileset sources:

- Embedded (defined in the map file).
- External (separate tileset file, reusable across maps).
- Collection of images (each tile is a separate image file).

**Custom properties** — Attach key-value data to tiles for game engine integration (tile type, animation frame, physics layer).

## Export and import

**Export formats** — Tiled supports multiple output formats:

- **TMX** (Tiled's native XML format, de-facto standard).
- **JSON** (increasingly popular, easier for some engines to parse).
- **Lua** (Lua table format, popular with Corona and LÖVE).
- **CSV** (basic export for simple tools).

Each format includes tileset data, layers, objects, and custom properties.

**Plugin exporters** — The community has written hundreds of custom exporters:

- **Godot exporter** — Exports to Godot's native tilemap format.
- **GameMaker Studio exporter** — Converts to GML rooms.
- **Defold exporter** — Converts to Defold tilemap format.
- **Custom game engine exporters** — If no built-in exporter exists, developers write one.

**Import** — Tiled reads TMX and JSON (external tilesets in various formats). Most game engines provide an importer or plugin to read Tiled's output.

## Scripting and extensibility

**Lua scripting** — Tiled supports Lua scripting for tile placement automation:

- Write scripts that generate terrain procedurally or apply rules.
- Examples: generate forests randomly, auto-fill collision layers based on visible tiles.

**Plugins** — Custom exporters are Lua scripts that generate output in any format. The plugin system is straightforward and widely used.

**Scripting limitations** — Limited to Tiled's internal data model; you cannot extend the UI or brush types significantly through scripts.

## How it fits the asset pipeline

Typical workflow:

1. Artist creates a tileset sprite sheet in Aseprite or Photoshop (individual tiles in a grid).
2. Artist exports as PNG (standard tile grid).
3. Level designer imports the tileset into Tiled.
4. Level designer configures terrain types and autotiles (or uses pre-configured tilesets).
5. Level designer paints maps using Tiled's brushes.
6. Level designer exports maps as TMX or JSON.
7. Game engine imports and renders the maps.

Tiled is positioned late in the pipeline: after sprite creation, before game integration.

## Workflow strengths

- **Industry standard** — Tiled is the most widely supported tilemap tool; most engines have built-in or community importers.
- **Extensive autotile support** — Terrain and Wang tiles cover most autotiling needs.
- **Multi-format export** — Flexibility in export target; not locked into one engine.
- **Large community** — Thousands of tutorials, assets, and plugins.
- **Free and open-source** — No licensing costs; full source code available.
- **Lightweight** — Fast to run even on older hardware; responsive UI.

## Workflow gaps

- **No pixel-level editing** — Tiled expects pre-made tilesets; cannot paint within tiles.
- **Basic object placement** — Object layers support simple shapes; no complex entity composition.
- **Limited visual feedback** — Autotile rules can be opaque; sometimes unclear which tile will be placed.
- **Plugin ecosystem fragmentation** — Custom exporters vary in quality; not all maintained.
- **No collaborative editing** — Single-user only; no built-in multiplayer or version control integration.

## Notable uses

- **Celeste** — Used Tiled (with custom pipeline) for level design.
- **Stardew Valley** — Level design in Tiled.
- **Countless indie games** — Most indie tilemap games use Tiled or have a Tiled exporter.

## Community and ecosystem

Tiled has one of the strongest communities in indie game development:

- Official documentation and asset packs.
- Hundreds of open-source plugins on GitHub.
- Community forums, Discord, and Reddit presence.
- Asset marketplaces with pre-made tilesets configured for Tiled.

Third-party support is extensive; nearly every game engine has a Tiled importer.

## Pricing details

Free. Available on itch.io with a "name your price" model (donation accepted, not required).

## Version history

Tiled has been actively developed since 2008. Recent versions (1.10+) have refined Wang tiles and merged the terrain/autotile system into a cohesive model.

Version 1.12+ (2024) includes further improvements to Wang sets and performance.

## Related tools and integrations

- **Godot** — Built-in Tilemap support; community importers for Tiled formats.
- **GameMaker Studio 2** — Room editor integrates Tiled workflows; exporters available.
- **Defold** — Community exporters available.
- **LÖVE 2D** — Lua-based engine; Tiled exporters integrate seamlessly.
- **Custom engines** — Many indie teams write their own Tiled importers.

## Technical details

**Performance** — Tiled handles large maps (thousands of tiles) smoothly. Memory usage is low.

**Undo/redo** — Full history support.

**Grid settings** — Supports arbitrary tile sizes, spacing, and offsets.

**Shortcuts** — Extensive keyboard shortcuts for power users.
