// Visual regression tests for the canvas viewport.
//
// Depends on: S14 (canvas renderer). Tests here validate the canvas container
// structure and overlay elements. Pixel-level WebGL rendering requires the
// Rust backend, which is not available in the Vite-only test environment —
// the canvas element renders but tile data is never received.
//
// When S14 is complete, extend these tests with:
//   - canvas at 100% zoom with a 32x32 sprite
//   - canvas at 800% zoom showing the pixel grid overlay
//   - onion skin overlay rendering
//
// Each UI stream should add its own spec file; this file ships the harness
// pattern for canvas-level visual tests.

import { test, expect } from "@playwright/test";
import { injectTauriMock, mockInvokeResponse } from "../helpers/tauri-mock";

// A minimal ProjectStatus that passes the activeProject !== null check in
// Shell, causing the Canvas component to mount instead of WelcomeScreen.
const MOCK_PROJECT = {
  metadata: {
    name: "Visual Test Project",
    description: "",
    author: "",
    created_at: 0,
    updated_at: 0,
    editor_version: "0.1.0",
  },
  path: null,
  dirty: false,
  sprite_count: 1,
};

test.beforeEach(async ({ page }) => {
  await injectTauriMock(page);
  // Override project_new so clicking "New Project" sets an active project and
  // switches the shell from WelcomeScreen to Canvas.
  await mockInvokeResponse(page, "project_new", MOCK_PROJECT);
  await page.goto("/");
  await page.waitForSelector(".shell");
});

test("canvas mounts after project_new resolves", async ({ page }) => {
  await page.getByRole("button", { name: "New Project" }).click();
  // Canvas container should replace the welcome screen.
  await expect(page.locator(".canvas-container")).toBeVisible();
  await expect(page.locator(".welcome")).not.toBeVisible();
  await expect(page).toHaveScreenshot("canvas-with-project.png");
});

test("status bar reflects the open project name", async ({ page }) => {
  await page.getByRole("button", { name: "New Project" }).click();
  await expect(page.locator(".status-bar")).toContainText("Visual Test Project");
  await expect(page.locator(".status-bar")).toHaveScreenshot("status-bar-with-project.png");
});

test("canvas element is present and sized in the DOM", async ({ page }) => {
  await page.getByRole("button", { name: "New Project" }).click();
  const canvas = page.locator(".canvas-container canvas");
  await expect(canvas).toBeVisible();
  // The canvas must have non-zero dimensions after mount.
  const box = await canvas.boundingBox();
  expect(box).not.toBeNull();
  expect(box!.width).toBeGreaterThan(0);
  expect(box!.height).toBeGreaterThan(0);
});
