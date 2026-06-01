# pixhaus-mod-pixel-art

The pixel-art module — a deep dedicated art mode (architecture bible sections
7.3, 6.8, 10).

- **Registers:** indexed-palette mode, pixel-perfect tools, palette reduction and
  dithering, pixel grid overlays, palette validation, and pixel-art generation
  constraints.
- **Status:** stub.

## Boundaries

- Pixel art is a mode layered on the shared core, NOT the whole product. Don't
  make the rest of the app pixel-only.
- Enforce pixel-art constraints through the surface types and the active art mode,
  not by forcing indexed color or grid snapping on every sprite.
- Pixel-art tools still produce commands and reuse the shared canvas; this module
  adds constraints and specialized tools, it doesn't replace the editing core.

Shared module rules: `modules/CLAUDE.md`. Global rules: root `CLAUDE.md`.
