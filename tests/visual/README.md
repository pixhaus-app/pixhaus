# Visual regression tests

Playwright harness for the editor UI. Runs in CI on Linux Chromium with a fixed 1280×800 viewport. Uses `expect(page).toHaveScreenshot()` for diffing.

## Baselines

Baselines (`tests/visual/baselines/*.png`) are committed and were generated on Linux (Ubuntu 22.04, Playwright Chromium) to match CI rendering.

To regenerate after an intentional UI change:

```bash
# Inside a Linux environment that matches CI (Ubuntu 22.04, Chromium):
pnpm visual:update    # runs `playwright test --update-snapshots`
git add tests/visual/baselines
git commit -m "test(visual): update baseline screenshots"
```

Do not generate baselines on Windows or macOS — Chromium font hinting and antialiasing differ across platforms by enough pixels to break the 2% tolerance once the run lands on Linux CI.

## Updating baselines after intentional UI changes

Same flow: a PR that changes the UI must include the regenerated baselines. Reviewers should look at the diff PNGs in `tests/visual/baselines/` to confirm the change is intentional.

## Limitations

The Tauri mock returns `null` for any IPC command the test doesn't explicitly override. That means most tests today screenshot the welcome screen or an empty editor; they catch chrome regressions but not pixel-rendering issues. A follow-up stream will add realistic IPC defaults so canvas tests can exercise actual sprite content.
