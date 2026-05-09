// Palette panel e2e — covers manual-test-guide section 8
// (T-palette-001..012).
//
// Status: every test in this file is skipped. The palette panel's flows
// (new palette, add/edit/delete swatch, drag-reorder, harmony, ramp,
// import/export) all require direct UI interaction with elements that
// don't have testids yet — the `+` header button, individual swatches,
// the Harmony / Ramp sub-panels, and the Palette I/O Menu. The command
// registry (ui/src/command-palette/command-registry.ts) exposes no
// `palette:*` entries either, so dispatchViaPalette() can't reach
// palette_add / palette_add_color / palette_set_color / etc.
//
// This file is intentionally a scaffold: it pins the test IDs from the
// manual guide so coverage gaps are visible, and each `it.skip` carries
// a TODO line citing the missing testid, helper, or command. As palette
// testids land, swap the skips for real tests in-place.
//
// References:
//   docs/manual-test-guide.md §8           — manual flows.
//   app/src/lib.rs:157-164                 — palette IPC names.
//   ui/src/command-palette/command-registry.ts — no palette:* entries.
//   ui/src/palette/PalettePanel.tsx        — UI surface, no testids.

import { bootApp } from "../helpers/app.js";

describe("Palette panel (manual-test-guide §8)", () => {
  // Boot once so the suite registers in the test runner; individual
  // tests don't need this since they're all skipped, but a fresh app
  // launch keeps the suite shape consistent with cmd.e2e.ts / tools.e2e.ts.
  before(async () => {
    await bootApp();
  });

  it.skip("T-palette-001: New palette", async () => {
    // TODO(testid): no `palette:new` command in command-registry.ts and
    // the `+` header button in PalettePanel.tsx (handleNewPalette) has
    // no data-testid. Also uses window.prompt() for the name, which
    // WebDriver can't drive without a mock. Add testid="palette-new"
    // and replace prompt() with an in-app modal before unskipping.
  });

  it.skip("T-palette-002: Add a colour", async () => {
    // TODO(testid): needs colour-picker UI interaction (HSV square +
    // hue slider) plus an "Add to palette" button testid. No palette:*
    // command exists; palette_add_color fires only from PalettePanel.tsx.
  });

  it.skip("T-palette-003: Edit a swatch", async () => {
    // TODO(testid): swatches have no per-index testid; editing is a
    // click-then-drag on the picker. Requires testid="palette-swatch-N"
    // and a programmatic-set helper for the picker before this is testable.
  });

  it.skip("T-palette-004: Delete a swatch", async () => {
    // TODO(testid): swatch delete affordance has no testid. Add
    // testid="palette-swatch-delete-N" and surface a deterministic
    // hover state before unskipping.
  });

  it.skip("T-palette-005: Reorder via drag", async () => {
    // TODO(helper): drag-and-drop reorder. WebDriver can synthesise the
    // drag, but there's no swatch testid to anchor source/target on.
    // Needs testid="palette-swatch-N" plus a helpers/canvas.ts-style
    // drag wrapper that targets element rects rather than the canvas.
  });

  it.skip("T-palette-006: Harmony generator", async () => {
    // TODO(testid): Harmony sub-panel and its harmony-type buttons
    // (triadic / tetradic / complementary / analogous) have no testids.
    // Add testid="palette-harmony-toggle" and per-harmony testids.
  });

  it.skip("T-palette-007: Ramp generator", async () => {
    // TODO(testid): Ramp sub-panel toggle, step input, and "Generate"
    // button have no testids. Add testid="palette-ramp-toggle",
    // testid="palette-ramp-steps", testid="palette-ramp-generate".
  });

  it.skip("T-palette-008: Import .gpl (GIMP)", async () => {
    // TODO(testid): Palette I/O Menu has no testid and its Import
    // entries are unaddressable. Also needs mockOpenDialog wiring to
    // a fixture path — fixture lives in examples/, but the menu must
    // be reachable first. Add testid="palette-io-menu",
    // testid="palette-import-gpl".
  });

  it.skip("T-palette-009: Import .hex", async () => {
    // TODO(testid): same as T-palette-008. Add testid="palette-import-hex".
  });

  it.skip("T-palette-010: Import .pal", async () => {
    // TODO(testid): same as T-palette-008. Add testid="palette-import-pal".
  });

  it.skip("T-palette-011: Import .aco", async () => {
    // TODO(testid): same as T-palette-008. No .aco fixture in the repo
    // either — bring-your-own per the manual guide. Add
    // testid="palette-import-aco" and a fixture before unskipping.
  });

  it.skip("T-palette-012: Export", async () => {
    // TODO(testid): Palette I/O Menu Export entries have no testids.
    // Needs testid="palette-export-gpl|hex|pal" plus mockSaveDialog
    // wiring. Round-trip assertion (re-import → colours match) requires
    // a follow-up test that imports the just-written file.
  });
});
