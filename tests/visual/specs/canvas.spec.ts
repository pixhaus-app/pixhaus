// Visual regression tests for the canvas viewport.
//
// The Tauri mock provides realistic defaults for sprite_list, canvas_composite,
// layer_list, and other commands issued during canvas startup — see
// helpers/tauri-mock.ts. Tests override project_new so the shell transitions
// from WelcomeScreen to Canvas, then rely on the global defaults to fill in
// the sprite metadata so the renderer draws a 32×32 checkerboard rather than
// a black rectangle.
//
// Pixel-level tile data (canvas:tile-dirty events) requires the Rust backend
// and is not exercised here. Once S01 (pixel buffers) lands these tests can
// fire synthetic tile events to exercise the full compositing path.

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

// Opens the project and waits for the canvas element to be visible.
// "New Project" now opens a canvas-size dialog rather than dispatching
// project_new directly; the helper accepts the default size by clicking
// Create. The ResizeObserver fires synchronously on mount so by the
// time toBeVisible() resolves the canvas.width has already been set.
async function openProjectAndWaitForCanvas(
  page: import("@playwright/test").Page,
) {
  await page.getByRole("button", { name: "New Project" }).click();
  await page.getByTestId("canvas-size-create").click();
  await expect(page.locator(".canvas-container canvas")).toBeVisible();
}

test("canvas mounts and renders checkerboard after project_new resolves", async ({
  page,
}) => {
  await openProjectAndWaitForCanvas(page);
  await expect(page.locator(".welcome")).not.toBeVisible();
  await expect(page).toHaveScreenshot("canvas-with-project.png");
});

test("status bar reflects the open project name", async ({ page }) => {
  await page.getByRole("button", { name: "New Project" }).click();
  await page.getByTestId("canvas-size-create").click();
  await expect(page.locator(".status-bar")).toContainText(
    "Visual Test Project",
  );
  await expect(page.locator(".status-bar")).toHaveScreenshot(
    "status-bar-with-project.png",
  );
});

test("canvas element is present and sized in the DOM", async ({ page }) => {
  await openProjectAndWaitForCanvas(page);
  const canvas = page.locator(".canvas-container canvas");
  // The canvas must have non-zero dimensions after mount.
  const box = await canvas.boundingBox();
  expect(box).not.toBeNull();
  expect(box!.width).toBeGreaterThan(0);
  expect(box!.height).toBeGreaterThan(0);
});
