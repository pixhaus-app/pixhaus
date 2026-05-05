// Visual regression tests for the application shell.
//
// These tests cover the welcome screen and status bar — the UI state when no
// project is open. They run against the Vite dev server with Tauri IPC mocked.
//
// Add tests here for shell-level chrome: menus, preferences modal, command
// palette. Canvas, layer panel, and timeline tests live in their own spec
// files alongside the streams that implement those panels.

import { test, expect } from "@playwright/test";
import { injectTauriMock } from "../helpers/tauri-mock";

test.beforeEach(async ({ page }) => {
  await injectTauriMock(page);
  await page.goto("/");
  // Wait for the app to hydrate before taking any measurements.
  await page.waitForSelector(".shell");
});

test("welcome screen: title, subtitle, and action buttons are visible", async ({ page }) => {
  await expect(page.locator(".welcome__title")).toBeVisible();
  await expect(page.locator(".welcome__title")).toContainText("Pixhaus");
  await expect(page.getByRole("button", { name: "New Project" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Open Project..." })).toBeVisible();
  await expect(page).toHaveScreenshot("welcome-screen.png");
});

test("status bar: shows no-project state", async ({ page }) => {
  await expect(page.locator(".status-bar")).toBeVisible();
  await expect(page.locator(".status-bar")).toContainText("No project open");
  await expect(page.locator(".status-bar")).toHaveScreenshot("status-bar-no-project.png");
});

test("command palette: opens on Ctrl+K and closes on Escape", async ({ page }) => {
  // Palette should not be visible initially.
  await expect(page.locator(".palette-backdrop")).not.toBeVisible();

  await page.keyboard.press("Control+k");
  await expect(page.locator(".palette-backdrop")).toBeVisible();
  // The input inside the palette should have focus.
  await expect(page.locator(".palette__input")).toBeFocused();
  await expect(page).toHaveScreenshot("command-palette-open.png");

  await page.keyboard.press("Escape");
  await expect(page.locator(".palette-backdrop")).not.toBeVisible();
});

test("command palette: filters commands by fuzzy query", async ({ page }) => {
  await page.keyboard.press("Control+k");
  await page.locator(".palette__input").fill("new");
  // At least one result should match "new" (e.g. "New Project", "New Sprite").
  await expect(page.locator(".palette__item").first()).toBeVisible();
  await expect(page).toHaveScreenshot("command-palette-filtered.png");
});
