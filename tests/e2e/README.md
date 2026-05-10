# Pixhaus end-to-end tests

WebdriverIO + tauri-driver suite that drives a real Pixhaus binary through the OS WebView. Each spec mirrors a section of `docs/manual-test-guide.md` so the guide stays the source of truth.

## Quickstart

```bash
# 1. Install tauri-driver and the platform WebDriver server.
pnpm e2e:setup

# 2. Build a debug binary of Pixhaus (5-10 min cold, fast on rebuild).
pnpm tauri:build:debug

# 3. Run the suite.
pnpm e2e
```

## Platform support

- **Windows**: works locally. Needs `msedgedriver.exe` matching the local Edge version. `pnpm e2e:setup` checks for it.
- **Linux**: works locally. Needs `webkit2gtk-driver` from your package manager and runs against an X server (use `xvfb-run` if running headless).
- **macOS**: not supported. Tauri's docs are explicit: macOS lacks a WebKit WebDriver tool. Run on Windows or Linux.

CI integration is intentionally deferred. Phase 0 lands locally only; a separate PR will wire a Linux runner.

## How it works

1. `pnpm tauri:build:debug` produces `target/debug/pixhaus(.exe)` by running the UI build (`pnpm --filter pixhaus-ui build`) then `cargo build -p pixhaus-app`. Bypassing `cargo tauri build` avoids dev/build mode confusion in cargo's incremental cache — `cargo build` always produces a production-mode binary that loads bundled assets via custom-protocol.
2. `pnpm e2e` runs `wdio run wdio.conf.ts`. The config:
   - Spawns `tauri-driver` on `127.0.0.1:4444`.
   - Hands tauri-driver the binary path through the `tauri:options.application` capability.
   - tauri-driver shells out to `msedgedriver` (Windows) or `WebKitWebDriver` (Linux), launches the binary, and proxies WebDriver commands.
3. Specs query the DOM through `data-testid` selectors and assert against `window.__pixhaus_debug__` (state) and `window.__pixhaus_ipc_log__` (IPC roundtrips).

## Layout

```
tests/e2e/
├── wdio.conf.ts          # WebdriverIO config + tauri-driver spawn lifecycle
├── helpers/
│   ├── app.ts            # bootApp() — navigate, dismiss dialog, wait for welcome
│   ├── canvas.ts         # clickCanvasAt / dragCanvas (sprite-coord translation)
│   ├── dialog.ts         # mockOpenDialog / mockSaveDialog / mockConfirmDialog
│   ├── ipc.ts            # findIpcByCmd / waitForIpc / expectIpcSequence
│   ├── selectors.ts      # data-testid string registry
│   └── state.ts          # typed accessors for window.__pixhaus_debug__
└── specs/                # one file per manual-test-guide section
    ├── smoke.e2e.ts      # harness sanity check (Phase 0)
    ├── launch.e2e.ts     # T-launch (§1)
    ├── project.e2e.ts    # T-project (§2)
    ├── export.e2e.ts     # T-export (§3)
    ├── canvas.e2e.ts     # T-canvas (§4)
    ├── tools.e2e.ts      # T-tools (§5)
    ├── select.e2e.ts     # T-select (§6)
    ├── transform.e2e.ts  # T-transform (§6)
    ├── layers.e2e.ts     # T-layers (§7)
    ├── palette.e2e.ts    # T-palette (§8)
    ├── timeline.e2e.ts   # T-timeline (§9)
    ├── tilemap.e2e.ts    # T-tilemap (§10)
    ├── cmd.e2e.ts        # T-cmd (§11)
    ├── window.e2e.ts     # T-window (§12)
    ├── help.e2e.ts       # T-help (§13)
    └── keys.e2e.ts       # T-keys (§14)
```

## Adding a spec

1. Pick the section from `docs/manual-test-guide.md` (e.g. T-tools-001..010).
2. Create `specs/<section>.e2e.ts` with one `it('T-section-NNN: ...')` per ID.
3. Reuse helpers — never reach into `__pixhaus_debug__` directly from a spec; add a typed accessor to `helpers/state.ts` instead.
4. Reuse selectors — never hard-code a `data-testid` string in a spec; add it to `helpers/selectors.ts`.
5. Add `data-testid` attributes to the UI as you author the spec; keep the testid name stable across the spec.

## Why not extend tests/visual?

`tests/visual/` runs Playwright against a mocked Tauri (`window.__TAURI_INTERNALS__.invoke` is replaced with a stub). It's fast and exercises rendering, but the IPC layer never round-trips through Rust. The e2e suite drives a real binary, so it catches regressions in the Rust side that the visual tests cannot.

Both are kept. They cover different layers.

## Troubleshooting

- **Suite hangs on session creation (Windows)**: msedgedriver version doesn't match Edge. Re-run `pnpm e2e:setup` or download the matching driver from the Microsoft page linked in the script.
- **`tauri-driver: command not found`**: `cargo install tauri-driver --locked` or run `pnpm e2e:setup`.
- **`binary not found at target/debug/pixhaus`**: run `pnpm tauri:build:debug` first.
- **First-launch dialog blocks the test**: fixed in `helpers/app.ts:bootApp()`. If your spec doesn't call it, the dialog backdrop intercepts pointer events.
- **Flake on canvas-coordinate clicks**: tauri-driver routes pointer events through the OS, so timing varies. Use `browser.waitUntil(...)` against the IPC log rather than fixed `pause()`.
