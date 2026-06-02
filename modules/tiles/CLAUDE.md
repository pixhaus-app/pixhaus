# pixhaus-mod-tiles

The tiles module — the Tiles workspace and terrain workflows (architecture bible
sections 7.3, 6.6).

- **Registers:** the Tiles workspace, the tile document type, the tile preview
  panel, autotile rules and seam validation, the tile-stamp tools, and tileset
  export targets.
- **Status:** stub.

## Boundaries

- May lean on pixel-art tooling heavily, but must not be limited to pixel art —
  tiles exist in other art styles too.
- Reuse the shared canvas, tools, and commands; tile editing is the editing core
  with tile-aware overlays and validation, not a separate editor — it acts through
  the shared editing context (architecture bible sections 5.9 and 22.7).
- Tileset export targets register with the export pipeline; the actual codecs live
  in `io`.
- Instrument autotile and seam-validation passes and the tileset-export jobs
  (`#[instrument]` on the bodies) plus the module registration. See the
  `pixhaus-tracing` skill.

Shared module rules: `modules/CLAUDE.md`. Global rules: root `CLAUDE.md`.
