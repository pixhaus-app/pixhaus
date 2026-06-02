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
- Wrap the palette-reduction and dither jobs in a coarse span (the whole job is the
  perf signal); no per-pixel spans. See the `pixhaus-tracing` skill.
- Register the indexed-mode tools and the palette/dither controls with keys in its
  namespace; ship its bundle when it gains UI. Validation findings shown to the user
  are keyed, not literal English. See the `pixhaus-i18n` skill.
- Palette analysis and previews are derived cache — recomputable, never the source
  of truth (bible section 22.6).

Shared module rules: `modules/CLAUDE.md`. Global rules: root `CLAUDE.md`.
