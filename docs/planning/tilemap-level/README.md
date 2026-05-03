# Tilemap and Level Editor Tools

This directory documents standalone and integrated tilemap/level editors. These tools sit adjacent to sprite workflows in the game development pipeline: artists create sprite assets, level designers compose them into maps using tilesets.

## Coverage

Tilemap editors fall into three categories:

**Dominant toolchain** — Tiled (open-source, industry standard).

**Modern alternatives** — LDtk (JSON-first, designed by Dead Cells creator).

**Secondary options** — OGMO Editor (lightweight, open-source), Tilesetter (tile generator for autotile automation).

## Key strategic questions for SpriteMaster

- **Tileset preparation** — How do artists export sprite sheets as tilesets? Do editors require specific metadata or handle raw images?
- **Autotile systems** — How do Wang tiles and rule-based autotiling work? What's the artist workflow for creating a 2x2, 3x3, or 16-tile autotile set?
- **Export and integration** — Which game engines does each editor export to? How portable is level data?
- **Custom rules and plugins** — Can level designers extend autotile rules? Do editors support scripting?

## Tools documented

- **Tiled** — The industry standard, with extensive autotile (wang tile) support, multi-format export, and a large plugin ecosystem.
- **LDtk** — Modern, JSON-first editor with visual autolayering and strong indie game adoption.
- **OGMO Editor 3** — Lightweight open-source alternative with flexible layer types and XML/JSON export.
- **Tilesetter** — Specialized tile generator for autotile workflow automation (16-tile sets, 3x3, etc.).

Each tool has different strengths and constraints. The choice depends on game type, team size, and export target.
