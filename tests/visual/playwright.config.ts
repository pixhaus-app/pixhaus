import { defineConfig, devices } from "@playwright/test";

// Baselines are committed for linux/chromium (the CI environment). Running
// locally on macOS or Windows will produce minor rendering differences (font
// hinting, DPI). Update baselines with `pnpm visual:update` inside a Linux
// environment or the CI container.

export default defineConfig({
  testDir: "./specs",
  snapshotDir: "./baselines",
  // Drop {platform} and {projectName} from the path so baselines are
  // OS-agnostic by intent — we only commit one set from linux CI.
  snapshotPathTemplate: "{snapshotDir}/{testFilePath}/{arg}{ext}",
  timeout: 30_000,
  expect: {
    toHaveScreenshot: {
      // 2% pixel-ratio tolerance absorbs sub-pixel font rendering variance
      // across different WebKit / Chromium minor versions in CI vs local.
      maxDiffPixelRatio: 0.02,
    },
  },
  retries: process.env["CI"] ? 1 : 0,
  reporter: [
    ["html", { open: "never", outputFolder: "../../.playwright-report" }],
    process.env["CI"] ? ["github"] : ["list"],
  ],
  use: {
    baseURL: "http://localhost:1420",
    screenshot: "only-on-failure",
    trace: "retain-on-failure",
    video: "off",
  },
  projects: [
    {
      name: "chromium",
      use: {
        ...devices["Desktop Chrome"],
        // Fixed viewport keeps baseline screenshots deterministic.
        viewport: { width: 1280, height: 800 },
      },
    },
  ],
  // Starts the Vite dev server before tests. pnpm -w resolves to the
  // workspace root, which has the ui:dev script.
  webServer: {
    command: "pnpm -w ui:dev",
    url: "http://localhost:1420",
    reuseExistingServer: !process.env["CI"],
    timeout: 60_000,
  },
  outputDir: "./test-results",
});
