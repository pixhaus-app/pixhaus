## app

The app binary is exemplary. The single audited unit covers `main.rs`, `diagnostics.rs`, and the `render_workspaces.rs` example, and every app-binary responsibility the rubric singles out is discharged correctly: anyhow with `.context()`, the single owned tokio runtime, the one tracing subscriber with the `WorkerGuard` held for all of main, `sys-locale` boot-language detection defaulting to `en`, the eframe 0.34 `ui`/`logic` methods, and no unwrap/expect/panic outside test blocks. The only confirmed finding is a single info-level readability note; there are no defects.

### Strengths

- Boot ordering in `main.rs:147-167` is correct (resolve log dir, init tracing, log startup, set language, build runtime) and each non-obvious step is commented inline, including the `WorkerGuard`-must-outlive-main caveat and why the env-var API key read is an interim seam pending the keyring path.
- `diagnostics.rs` is textbook `pixhaus-tracing` compliance: one Registry with a single shared `EnvFilter` gating both sinks, console ANSI on and file ANSI off (with the why), the log-to-tracing bridge via `LogTracer::init()` installed manually to avoid the double-init panic, `RUST_LOG` honored, and `build_subscriber` split out so it is testable without the once-per-process global init.
- Combinator policy is followed over `unwrap()`/`expect()`: `main.rs:166` uses `map_or_else` plus `unwrap_or("en")`, with the eager `unwrap_or` correct for the cheap `&'static str` default.
- `window_icon()` (`main.rs:102-110`) decodes the trusted baked-in brand PNG, so it needs no `Limits` guard, and uses the move-only `into_rgba8().into_raw()` interop path the `pixhaus-image` skill names; the infallible-boot fallback to eframe's default icon via `.ok()?` is the right shape.
- i18n is honored: the binary sets the active language at boot exactly where `pixhaus-i18n` requires, and the `render_workspaces.rs` string literals (`"draw"`, `"animate"`, ...) are `WorkspaceId` ids, not display text, so they are correctly not keyed.
- `tracing::info!` lines use structured fields (`backend`, `adapter`, `modules`, version/os/arch) instead of interpolated strings, and `OPENROUTER_API_KEY` is read but explicitly never logged, satisfying the no-secrets rule.
- Import groups are separated std / external / workspace with blank lines across all three files, matching the hand-maintained grouping convention.
- eframe 0.34 API is used correctly throughout: `App::ui` and `App::logic` (not deprecated `update`), `run_native` with `Ok(Box::new(...))`, explicit `Renderer::Wgpu`, and a distinct app id `"pixhaus"` versus window title `"Pixhaus"`.

### Findings

| ID | File:Lines | Severity | Category | Issue -> Fix |
|----|-----------|----------|----------|--------------|
| U32-2 | app/src/main.rs:166 | info | style | The locale-detection statement packs `map_or_else`, `split`, `next`, `unwrap_or`, and `to_ascii_lowercase` into one long single-line `let`, against the rust-conventions "one adapter per line; a chain you can't read at a glance wants to be a loop" rule. It passes fmt and clippy pedantic, so it is a readability note, not a lint failure. Edge note: an empty OS locale tag splits to `[""]` and yields `""`, which the `unwrap_or("en")` guard does not catch (it is harmless only because the i18n service falls back to `en`). -> Extract a small `detect_language() -> String` (or break the chain across lines) and add `.filter(|s| !s.is_empty())` before the fallback if a non-empty code is wanted. |

### Checked and cleared (false positives)

- U32-1 (missing `#[instrument]` on `build_host`): rejected. The cited tracing rule scopes `#[instrument]` to public job/command/encode/load functions; `build_host` is a private binary boot function, and it already emits explicit `info!`/`warn!` landmarks for the startup flow. The auditor itself conceded no change is required for compliance.
