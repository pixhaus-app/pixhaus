# OGMO Editor 3

## Quick facts
- Vendor / maintainer: OGMO community (open-source)
- License / pricing model: MIT (free)
- Price point (current): Free
- Platforms: Windows, macOS, Linux
- First released: 2011 (OGMO Editor, various iterations)
- Last meaningful update: 2024 (3.4.0+)
- Source available: Yes (GitHub, full source code)
- Primary use case: Flexible, lightweight tilemap and level editor for indie games

## Origin and purpose

OGMO Editor evolved as a community-maintained project, designed to be simple yet flexible. Unlike Tiled's focus on autotiling or LDtk's entity-first design, OGMO prioritizes straightforward layer composition: tiles, decals (placed images), entities, and grid-based metadata.

OGMO is particularly popular in game jam communities where quick iteration and simplicity are valued over feature completeness.

## Tile editing capabilities

OGMO provides basic tile painting:

- **Stamp brush** — Click to place tiles.
- **Fill tools** — Fill rectangular or irregular regions.
- **Eraser** — Remove tiles.
- **Selection tools** — Select and operate on regions.

The tile selection UI is simple: a panel showing the tileset sprite sheet, with individual tiles clickable.

OGMO assumes pre-made tilesets; no pixel-level editing.

## Autotile / wang tile / rule tiles

OGMO does not have a built-in autotile system. Instead:

- Use individual tiles and paint manually.
- Or, layer multiple tile layers (one for base terrain, one for decorative details).
- Or, use decal layers to place pre-drawn patterns.

Some projects use external tools (Tiled) to generate maps with autotiling, then export and import into OGMO. This is not ideal but functional.

**Custom grid layers** — OGMO supports integer grid layers (metadata grids) with custom properties, which some teams use for collision or trigger definition.

This is a notable gap compared to Tiled's terrain system or LDtk's autolayers.

## Multi-layer maps

OGMO supports multiple layer types:

**Tile Layers** — Standard tilemap grids.

**Decal Layers** — Free-form image placement with scale and rotation. Useful for decorative elements, parallax, or static props.

**Entity Layers** — Place named game objects with custom properties.

**Grid Layers** — Integer or tile-based grid metadata (for collision, hazards, etc.).

Layers have visibility toggles, opacity control, and locking.

Layer organization is hierarchical; you can group and nest layers.

## Tile atlas integration

**Tileset definition** — A Tileset specifies:

- Sprite sheet image and tile size.
- OGMO auto-detects boundaries (with manual offset if needed).

**Simple configuration** — No terrain types or complex metadata. Tilesets are straightforward references to sprite sheets.

Custom properties per tile are not supported natively; all per-tile metadata must go elsewhere (entity properties or grid layer values).

## Export and import

**Export formats** — OGMO exports in two formats:

- **XML** (.oel format, OGMO's native format).
- **JSON** (.json, increasingly common).

Both formats include tileset data, layers, entities, and custom properties.

**Import** — OGMO reads its own .ogmo project files. It does not import from other editors.

**Game engine integration** — Community exporters and importers exist for:

- **Godot** — Community importers available.
- **GameMaker Studio 2** — Importers available.
- **Custom engines** — XML/JSON are straightforward to parse.

Less mature than Tiled's export ecosystem, but sufficient for most use cases.

## Scripting and extensibility

OGMO does not have a built-in scripting system. Customization is through:

- Custom properties on entities (game developers parse these in code).
- Grid layer metadata (collision types, etc.).

No plugins, no Lua scripts, no visual rule editor. Simplicity is the design philosophy.

## How it fits the asset pipeline

Typical workflow:

1. Artist creates tileset sprite sheet.
2. Artist exports as PNG.
3. Level designer imports tileset into OGMO.
4. Level designer paints tile layers and places decals, entities.
5. Level designer exports as XML or JSON.
6. Game developer parses and integrates into the engine.

OGMO is lightweight and fast; well-suited to rapid prototyping.

## Workflow strengths

- **Simple and lightweight** — Fast to learn and use; minimal UI overhead.
- **Flexible layer types** — Tiles, decals, entities, grids; adapts to various workflows.
- **Multiple export formats** — XML and JSON both supported.
- **Free and open-source** — MIT license; no costs.
- **Active community** — Well-maintained despite being community-driven.
- **Game jam friendly** — Minimal setup; fast iteration.

## Workflow gaps

- **No autotile system** — Manual tile painting only; must use external tools for terrain autotiling.
- **No visual entity composition** — Entities are placed, but not visually composed; no hierarchical entity building.
- **Limited metadata** — No per-tile properties; all custom data on entities or grid layers.
- **Smaller ecosystem** — Fewer community tools and importers than Tiled.
- **No collaborative editing** — Single-user only.
- **No scripting** — No Lua or visual rules; logic entirely in game code.

## Notable uses

- **Game jam entries** — Popular in Ludum Dare and other jams.
- **Indie platformers** — Some indie 2D games use OGMO.
- **Educational projects** — Used in game development courses for simplicity.

## Community and ecosystem

OGMO has a small but dedicated community:

- Official documentation.
- Community GitHub repos with importers.
- Forum and Discord support.
- Asset packs with pre-made tilesets.

The community is responsive but smaller than Tiled or LDtk.

## Pricing details

Free. MIT license; no donations model, but support is appreciated.

## Version history

OGMO has evolved through multiple versions. OGMO Editor 3 (version 3.4.0+) is the current stable release, with XML and JSON export, and improved UI.

Version 3 represents a significant modernization from earlier OGMO versions.

## Interaction with related tools

- **Tiled** — Some users export from Tiled (with autotiles), then import into OGMO for further refinement (not ideal).
- **Custom game engines** — Many indie teams have custom OGMO importers.
- **Game jams** — Popular for quick prototyping due to simplicity.

## Technical details

**Performance** — Lightweight; handles large maps smoothly.

**File format** — XML (.oel) or JSON (.json), both text-based and version-control friendly.

**Layer limits** — No hard limits on layers or map size; performance depends on hardware.

**Undo/redo** — Full history support.

## Comparison with Tiled and LDtk

| Feature | Tiled | LDtk | OGMO |
|---------|-------|------|------|
| Autotiling | Terrain + Wang | Autolayers | None (manual) |
| UI complexity | Moderate | Simple | Very simple |
| Export formats | Multiple | JSON (primary) | XML, JSON |
| Scripting | Lua | Visual rules | None |
| Community | Large | Growing | Small |
| Learning curve | Moderate | Shallow | Very shallow |
| Best for | Established pipelines | Modern workflows | Game jams, rapid prototyping |
