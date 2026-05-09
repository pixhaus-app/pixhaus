// Tilemap panel e2e — covers manual-test-guide section 10
// (T-tilemap-001..005).
//
// Every test in this section is skipped at the time of writing. The
// command palette exposes tilemap tool toggles (tilemap:tool-pencil,
// tilemap:tool-erase, tilemap:toggle-autotile) but no entries for
// creating tilesets or placing/erasing tiles — those flows live in the
// Tilemap panel form, which has no addressable testids yet.
//
// Place / erase / autotile tests additionally need:
//   - a tile selected from the tileset grid (no testid),
//   - the active layer converted to a tilemap layer (T-layers-009),
//   - a click on a specific tilemap-layer cell on the canvas at
//     tile-grid coordinates.
//
// describe.skip the whole spec until the panel is wired with testids.
// Keep the file in tree so the section-10 sweep stays auditable and
// each test ID has a place to land when its UI affordance arrives.

describe.skip("Tilemap panel (manual-test-guide §10)", () => {
  it("T-tilemap-001: Add a tileset", async () => {
    // Pre: a sprite is active.
    // Palette has no "New Tileset" command (command-registry.ts only
    // exposes tilemap:tool-pencil, tilemap:tool-erase,
    // tilemap:toggle-autotile, tilemap:toggle-tool — none of which fire
    // tileset_add). Creation goes through the Tilemap panel → Tilesets
    // tab → New Tileset form, whose name + tile-size inputs and submit
    // button lack testids.
    //
    // To unblock: either add a `tilemap:new-tileset` palette command
    // that opens a form with default values and dispatches tileset_add,
    // OR wire `tilemap-new-tileset-name`, `tilemap-new-tileset-size`,
    // and `tilemap-new-tileset-submit` testids on the panel form.
    // TODO(testid): tilemap panel new-tileset form
  });

  it("T-tilemap-002: Place a tile", async () => {
    // Pre: T-tilemap-001 done; layer is a tilemap layer; tilemap pencil
    // active; a tile is selected from the tileset grid.
    //
    // Multi-step UI dependency:
    //   1. Tileset selection (no testid on the tileset list rows).
    //   2. Tile-grid cell click to pick the source tile (no per-tile
    //      testid; the grid renders into a single canvas).
    //   3. Layer conversion to tilemap (T-layers-009 — covered there).
    //   4. Canvas click on a tilemap-layer cell at tile-grid coords.
    //
    // Step 4 is feasible with helpers/canvas.ts but the upstream picks
    // need testids first. Unskip once T-tilemap-001 lands and the
    // tile-picker exposes per-tile selectors.
    // TODO(testid): tileset list + per-tile picker
  });

  it("T-tilemap-003: Erase a tile", async () => {
    // Pre: at least one placed tile; tilemap erase tool active.
    // Same blockers as T-tilemap-002 — needs a placed tile to erase, and
    // placement isn't addressable yet. tilemap:tool-erase IS in the
    // palette, so step "switch to erase" works; the rest doesn't.
    // TODO(testid): tilemap placement
  });

  it("T-tilemap-004: Autotile mode", async () => {
    // Pre: a tileset with autotile rules; a source tile selected.
    // tilemap:toggle-autotile is dispatchable via the palette
    // ("toggle autotile mode"), but the assertion ([VISUAL] neighbour
    // tiles re-fit when the autotile bit flips) needs a way to read the
    // tilemap layer's cell contents, which is not exposed on the debug
    // surface. Add a `getTilemapCellAt(x, y)` accessor before unskipping.
    // TODO(testid): tilemap cell read accessor
  });

  it("T-tilemap-005: Tile property persistence", async () => {
    // Pre: a tileset with at least one tile.
    // Tile metadata (collision, custom kv pairs) persists through
    // tileset_set_tile_metadata. The metadata editor lives in the
    // Tilemap panel without testids and isn't reachable from the
    // palette. Unskip alongside T-tilemap-001's panel-testid work.
    // TODO(testid): tile metadata editor
  });
});
