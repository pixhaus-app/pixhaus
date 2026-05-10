// Tilemap panel e2e — covers manual-test-guide section 10
// (T-tilemap-001..005).
//
// T-tilemap-001 (Add a tileset) runs via the tilemap:new-tileset
// palette command added in this PR. T-tilemap-002..005 stay skipped
// — they require:
//   - a tile selected from the tileset grid (no per-tile testid;
//     the grid renders into a single canvas),
//   - the active layer converted to a tilemap layer (T-layers-009),
//   - a click on a specific tilemap-layer cell at tile-grid coords,
//   - a debug accessor to read tilemap cell contents (for autotile
//     verification),
//   - testids on the tile-metadata editor (collision toggle).
//
// Each skipped test carries an inline TODO citing the specific
// blocker. Unskip in-place as the underlying UI affordances arrive.

describe("Tilemap panel (manual-test-guide §10)", () => {
  it("T-tilemap-001: Add a tileset", async () => {
    const { $: $$, browser: br } = await import("@wdio/globals");
    const { bootApp } = await import("../helpers/app.js");
    const { byTestId, testid } = await import("../helpers/selectors.js");
    const { getActiveProject } = await import("../helpers/state.js");
    const { clearIpcLog, waitForIpc } = await import("../helpers/ipc.js");

    await bootApp();
    const newProject = await $$(byTestId(testid.welcome.newProject));
    await newProject.click();
    await br.waitUntil(async () => (await getActiveProject()) !== null, {
      timeout: 10000,
      timeoutMsg: "active project never registered",
    });
    await clearIpcLog();
    await br.execute(() => {
      const w = window as unknown as {
        __pixhaus_debug__: { command: { dispatch(id: string): void } };
      };
      w.__pixhaus_debug__.command.dispatch("tilemap:new-tileset");
    });
    const entries = await waitForIpc("tileset_add", 1, 5000);
    if (entries.length === 0) throw new Error("tileset_add never fired");
  });

  it.skip("T-tilemap-002: Place a tile", async () => {
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

  it.skip("T-tilemap-003: Erase a tile", async () => {
    // Pre: at least one placed tile; tilemap erase tool active.
    // Same blockers as T-tilemap-002 — needs a placed tile to erase, and
    // placement isn't addressable yet. tilemap:tool-erase IS in the
    // palette, so step "switch to erase" works; the rest doesn't.
    // TODO(testid): tilemap placement
  });

  it.skip("T-tilemap-004: Autotile mode", async () => {
    // Pre: a tileset with autotile rules; a source tile selected.
    // tilemap:toggle-autotile is dispatchable via the palette
    // ("toggle autotile mode"), but the assertion ([VISUAL] neighbour
    // tiles re-fit when the autotile bit flips) needs a way to read the
    // tilemap layer's cell contents, which is not exposed on the debug
    // surface. Add a `getTilemapCellAt(x, y)` accessor before unskipping.
    // TODO(testid): tilemap cell read accessor
  });

  it.skip("T-tilemap-005: Tile property persistence", async () => {
    // Pre: a tileset with at least one tile.
    // Tile metadata (collision, custom kv pairs) persists through
    // tileset_set_tile_metadata. The metadata editor lives in the
    // Tilemap panel without testids and isn't reachable from the
    // palette. Unskip alongside T-tilemap-001's panel-testid work.
    // TODO(testid): tile metadata editor
  });
});
