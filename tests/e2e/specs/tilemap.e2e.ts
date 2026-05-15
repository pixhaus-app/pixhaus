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
    const createBtn = await $$(byTestId(testid.canvasSizeDialog.create));
    await createBtn.waitForDisplayed({ timeout: 5000 });
    await createBtn.click();
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
    // testid="tileset-row-N" is now wired on the tileset management rows
    // and testid="tileset-add-btn" on the create button (TilemapPanel.tsx).
    // The per-tile picker grid renders into a single <canvas>; there is no
    // per-tile DOM element to address. Blocked on tile-picker DOM exposure.
    // TODO(testid): tileset list + per-tile picker
  });

  it.skip("T-tilemap-003: Erase a tile", async () => {
    // Same blocker as T-tilemap-002 — needs a placed tile, which requires
    // a per-tile picker selector. tilemap:tool-erase is palette-dispatchable.
    // TODO(testid): tilemap placement
  });

  it.skip("T-tilemap-004: Autotile mode", async () => {
    // tilemap:toggle-autotile is palette-dispatchable. The assertion needs
    // a `getTilemapCellAt(x, y)` debug accessor to read cell contents, which
    // is not yet on the debug surface (__pixhaus_debug__ in debug/index.ts).
    // TODO(testid): tilemap cell read accessor
  });

  it.skip("T-tilemap-005: Tile property persistence", async () => {
    // testid="tileset-add-btn" and testid="tileset-add-name" are now wired
    // in TilemapPanel.tsx. The tile metadata editor (collision toggle) lives
    // in TilesetPanel.tsx and still has no per-tile testids. Add
    // testids to TilesetPanel's tile property editor before unskipping.
    // TODO(testid): tile metadata editor
  });
});
