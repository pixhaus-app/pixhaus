# Rust Foundation Crates Reference - Pixhaus (May 2026)

**Date:** May 2, 2026  
**Target:** Tauri 2.x desktop app (Rust backend + TypeScript/Solid frontend + WebGL2 canvas)  
**Scope:** Foundations, System, Networking, Observability

This document covers 50+ Rust crates across serialization, async runtimes, error handling, networking, observability, and system integration. Each crate entry includes current status (May 2026), maintenance level, version compatibility notes, and Pixhaus-specific use cases.

---

## Table of Contents

1. [Tauri Ecosystem](#tauri-ecosystem)
2. [Async Runtime & Parallelism](#async-runtime--parallelism)
3. [Serialization](#serialization)
4. [Error Handling](#error-handling)
5. [Networking & HTTP](#networking--http)
6. [Observability & Logging](#observability--logging)
7. [System & OS APIs](#system--os-apis)
8. [Compression](#compression)
9. [Process & Shell](#process--shell)
10. [Graphics & GPU](#graphics--gpu)
11. [Scripting](#scripting)
12. [Miscellaneous Utilities](#miscellaneous-utilities)

---

## Tauri Ecosystem

### Tauri (tauri)

- **Purpose:** Lightweight cross-platform desktop app framework. Rust backend + web frontend (Tauri webview bridges to native OS renderers).
- **Crates.io:** https://crates.io/crates/tauri
- **Docs:** https://docs.rs/tauri/latest/tauri/
- **Repo:** https://github.com/tauri-apps/tauri
- **License:** MIT or Apache-2.0
- **Maintenance (May 2026):** Active. Latest stable: 2.11.0 (version scheme is 2.x; Tauri 3.0 is in draft milestone, not yet roadmapped for release).
- **When to use:** Core framework for Pixhaus. All IPC, window management, menu system, updater, native file dialogs, keyboard shortcuts route through Tauri.
- **Alternatives:** Electron (40x larger app bundle, slower startup), Iced (emerging Rust UI framework but no native menu/updater parity), Dioxus desktop (similar maturity to Tauri).
- **Notes:**
  - Tauri 2.0 decoupled most functionality into optional plugins. Many formerly-built-in features (notifications, store, updater, clipboard, dialog) now live in separate plugin crates.
  - Plugin ecosystem is stable. The official plugins listed below are actively maintained.
  - Minimum WebView: WebKit2GTK 4.1 (Linux), WebKit (macOS), WebView2 (Windows). WebView2 runtime bundled in Tauri installers on Windows.
  - Breaking change in 2.0: IPC invoke signatures changed; use tauri-specta to bridge.
  - Tauri 3.0 planning is in draft stage; no timeline. Current focus: mobile UX, hot reload, Linux renderer improvements.
- **Pixhaus streams using it:** S13 (shell), S14 (canvas), S49 (CI/CD)

### tauri-specta

- **Purpose:** Type-safe IPC between Rust backend and TypeScript frontend. Generates TS bindings at build time from Rust function signatures.
- **Crates.io:** https://crates.io/crates/tauri-specta
- **Docs:** https://docs.rs/tauri-specta/latest/tauri_specta/
- **Repo:** https://github.com/specta-rs/specta
- **License:** MIT
- **Maintenance (May 2026):** Active. Version: 2.x (paired with Tauri 2.x; does not support Tauri 1.x).
- **When to use:** Pixhaus's core IPC layer. Every command (S05 undo/redo, S15 brush strokes, S18 palette edits) is a Tauri invoke that must be type-checked. Specta generates TS types from `#[tauri::command]` Rust functions.
- **Alternatives:**
  - `ts-rs` (simpler, one-way TS generation, no command macros) — useful if command structs only.
  - Manual TypeScript interfaces (error-prone, maintenance burden).
- **Notes:**
  - Specta 2.0 requires Tauri 2.0. If Tauri 1.x migration happens, must downgrade to specta 1.x.
  - Supports both events (Rust → TS) and invokes (TS → Rust).
  - Can generate both TypeScript and JavaScript; configure in build.rs.
  - Zero runtime overhead; all work is done at build time.
- **Pixhaus streams using it:** S05 (commands), S13 (IPC shell), S21 (verb runtime invokes), S22 (backend adapters).

### ts-rs

- **Purpose:** Generate TypeScript interfaces from Rust types via `#[derive(TS)]`. Simpler one-way binding than tauri-specta.
- **Crates.io:** https://crates.io/crates/ts-rs
- **Docs:** https://docs.rs/ts-rs/latest/ts_rs/
- **Repo:** https://github.com/Aleph-Alpha/ts-rs
- **License:** MIT
- **Maintenance (May 2026):** Active.
- **When to use:** Pixhaus data model types (Project, Sprite, Palette, etc.) need TS counterparts for serialization/deserialization. Use ts-rs for the models; use tauri-specta for command signatures.
- **Alternatives:** tauri-specta (heavier but command-aware), manual interfaces.
- **Notes:**
  - ts-rs derives only; doesn't handle command invocation. Pair with serde + serde_json for TS import/export.
  - Supports custom impls via `#[ts(as = "Type")]`.
  - Does not require Tauri; can be used in any Rust+TS project.
- **Pixhaus streams using it:** S02 (palette types), S05 (command serialization), S07 (file format), S10 (sprite sheet export).

### tauri-plugin-store

- **Purpose:** Persistent key-value store for app settings, user preferences, window state.
- **Crates.io:** https://crates.io/crates/tauri-plugin-store
- **Docs:** https://docs.rs/tauri-plugin-store/latest/tauri_plugin_store/
- **Repo:** https://github.com/tauri-apps/plugins-workspace
- **License:** MIT
- **Maintenance (May 2026):** Official Tauri plugin, actively maintained.
- **When to use:** Pixhaus preferences (keybinds, theme, AI backend config, recent projects, window geometry). Store is JSON-backed; queried and updated via IPC.
- **Alternatives:**
  - `tauri-plugin-window-state` (specialized for window geometry only).
  - `directories` crate + manual JSON file I/O (lower-level control).
- **Notes:**
  - Data stored in `~/.config/pixhaus/store.json` (or OS equivalent).
  - No query language; flat key-value access.
  - Not suitable for large data (e.g., project history). Use dedicated file formats for that.
- **Pixhaus streams using it:** S13 (preferences UI).

### tauri-plugin-updater

- **Purpose:** Auto-update mechanism. Checks a release endpoint (GitHub Releases by default), downloads + installs new versions, optionally verifies signatures.
- **Crates.io:** https://crates.io/crates/tauri-plugin-updater
- **Docs:** https://docs.rs/tauri-plugin-updater/latest/tauri_plugin_updater/
- **Repo:** https://github.com/tauri-apps/plugins-workspace
- **License:** MIT
- **Maintenance (May 2026):** Official, active.
- **When to use:** Pixhaus release distribution. S50 handles packaging; this plugin wires the update check UI and downloads.
- **Alternatives:** Manual download + shell invocation (fragile), Squirrel.Windows (Windows only, deprecated).
- **Notes:**
  - Tauri bundles release artifacts; updater checks a JSON file (auto-generated by CI) pointing to new binaries.
  - Signature verification uses Ed25519 keypair; keys stored in config.
  - Works cross-platform (Windows MSI, macOS DMG, Linux AppImage/deb/rpm).
- **Pixhaus streams using it:** S50 (release packaging).

### tauri-plugin-fs

- **Purpose:** Sandboxed filesystem access. Rust-side file I/O through Tauri's IPC boundary, scoped to declared paths.
- **Crates.io:** https://crates.io/crates/tauri-plugin-fs
- **Docs:** https://docs.rs/tauri-plugin-fs/latest/tauri_plugin_fs/
- **Repo:** https://github.com/tauri-apps/plugins-workspace
- **License:** MIT
- **Maintenance (May 2026):** Official, active.
- **When to use:** TypeScript side only (e.g., reading / writing project files from the frontend). Rust side uses std::fs or tokio::fs directly without going through Tauri's IPC sandbox. Useful for: user-initiated "open file" dialogs, exporting results to user's home directory.
- **Alternatives:** `tauri-plugin-dialog` (for file picker UI).
- **Notes:**
  - Filesystem access is sandboxed; scope is configured in tauri.conf.json.
  - TS side: `await fs.readTextFile(path)` is convenient but IPC-overhead-heavy for large files. Use Rust-side I/O for project files.
  - Rust side: Use std::fs or tokio::fs; no plugin needed.
- **Pixhaus streams using it:** S13 (save dialog integration from TS if needed), S07 (native format read/write happens in Rust core).

### tauri-plugin-dialog

- **Purpose:** Native file dialogs (open, save) and message boxes (alert, confirm, etc.).
- **Crates.io:** https://crates.io/crates/tauri-plugin-dialog
- **Docs:** https://docs.rs/tauri-plugin-dialog/latest/tauri_plugin_dialog/
- **Repo:** https://github.com/tauri-apps/plugins-workspace
- **License:** MIT
- **Maintenance (May 2026):** Official, active.
- **When to use:** Pixhaus file open/save dialogs. "Open .pixhaus", "Export PNG", "Import .aseprite", etc.
- **Alternatives:** None standard; HTML5 file input is web-only and not suitable for desktop apps.
- **Notes:**
  - Async API: `ask()`, `confirm()`, `open()`, `save()`.
  - Accepts file filters: `[{ name: 'Pixhaus', extensions: ['pixhaus'] }]`.
  - Returns selected path(s); Rust core then performs actual I/O.
- **Pixhaus streams using it:** S13 (shell menu).

### tauri-plugin-window-state

- **Purpose:** Persist and restore window geometry (size, position, maximized state) across restarts.
- **Crates.io:** https://crates.io/crates/tauri-plugin-window-state
- **Docs:** https://docs.rs/tauri-plugin-window-state/latest/tauri_plugin_window_state/
- **Repo:** https://github.com/tauri-apps/plugins-workspace
- **License:** MIT
- **Maintenance (May 2026):** Official, active.
- **When to use:** Pixhaus window should remember its size and position between sessions.
- **Alternatives:** Manual window state via `tauri-plugin-store` (more control, more code).
- **Notes:**
  - Stores state in `~/.config/pixhaus/window-state.json`.
  - Integrates with Tauri's window API; no additional TS code needed in ideal case.
- **Pixhaus streams using it:** S13 (shell).

### tauri-plugin-log

- **Purpose:** Rotate log files and expose logging from Rust to a file on disk.
- **Crates.io:** https://crates.io/crates/tauri-plugin-log
- **Docs:** https://docs.rs/tauri-plugin-log/latest/tauri_plugin_log/
- **Repo:** https://github.com/tauri-apps/plugins-workspace
- **License:** MIT
- **Maintenance (May 2026):** Official, active.
- **When to use:** Pixhaus should log to disk for debugging and diagnostics. Use this for basic log rotation; pair with `tracing` for structured observability.
- **Alternatives:** `env_logger` (manual rotation), `tracing-subscriber` (more powerful but needs manual file setup).
- **Notes:**
  - Simple text logs to file, rotating by size.
  - Logs written to `~/.config/pixhaus/logs/`.
  - Not a structured logger; for JSON logs use `tracing`.
- **Pixhaus streams using it:** S13 (logging infrastructure), S51 (crash reporting integration).

### tauri-plugin-shell

- **Purpose:** Execute external commands / subprocess from Tauri.
- **Crates.io:** https://crates.io/crates/tauri-plugin-shell
- **Docs:** https://docs.rs/tauri-plugin-shell/latest/tauri_plugin_shell/
- **Repo:** https://github.com/tauri-apps/plugins-workspace
- **License:** MIT
- **Maintenance (May 2026):** Official, active.
- **When to use:** Pixhaus may need to invoke external tools (e.g., ImageMagick for palette analysis, ffmpeg for MP4 export). This plugin sandboxes the subprocess.
- **Alternatives:** `std::process::Command` in Rust (unsandboxed), duct or xshell crates (more ergonomic).
- **Notes:**
  - TS side: `await shell.execute(cmd)`.
  - Rust side: Direct tokio::process or duct is simpler; use this only if TS needs to trigger subprocess.
  - Commands are verified against a whitelist in tauri.conf.json for security.
- **Pixhaus streams using it:** S11 (animated GIF/MP4 export if ffmpeg is external), S50 (signing scripts).

### tauri-plugin-os

- **Purpose:** Access OS-level info (platform, arch, type, version).
- **Crates.io:** https://crates.io/crates/tauri-plugin-os
- **Docs:** https://docs.rs/tauri-plugin-os/latest/tauri_plugin_os/
- **Repo:** https://github.com/tauri-apps/plugins-workspace
- **License:** MIT
- **Maintenance (May 2026):** Official, active.
- **When to use:** Pixhaus can use this to detect OS for platform-specific theming or behavior.
- **Alternatives:** `std::env::consts`, `cfg!` macros (compile-time only).
- **Notes:**
  - Returns `platform()`, `type()`, `arch()`, `version()` at runtime.
  - Useful for analytics (S51 crash reporting) to tag crashes by OS.
- **Pixhaus streams using it:** S13 (theming), S51 (crash reporting context).

### tauri-plugin-process

- **Purpose:** Access process-level info and control (PID, kill process, etc.).
- **Crates.io:** https://crates.io/crates/tauri-plugin-process
- **Docs:** https://docs.rs/tauri-plugin-process/latest/tauri_plugin_process/
- **Repo:** https://github.com/tauri-apps/plugins-workspace
- **License:** MIT
- **Maintenance (May 2026):** Official, active.
- **When to use:** Pixhaus single-instance mode (S13) can use this to detect/kill duplicate processes.
- **Alternatives:** `tauri-plugin-single-instance` (specialized for this use case, preferred).
- **Notes:**
  - Can exit process, but most use cases use tauri's built-in lifecycle methods.
- **Pixhaus streams using it:** S13 (single-instance detection).

### tauri-plugin-clipboard-manager

- **Purpose:** Read/write OS clipboard.
- **Crates.io:** https://crates.io/crates/tauri-plugin-clipboard-manager
- **Docs:** https://docs.rs/tauri-plugin-clipboard-manager/latest/tauri_plugin_clipboard_manager/
- **Repo:** https://github.com/tauri-apps/plugins-workspace
- **License:** MIT
- **Maintenance (May 2026):** Official, active.
- **When to use:** Pixhaus copy/paste operations. Copy selected sprite to clipboard (as PNG or JSON), paste from clipboard.
- **Alternatives:** JavaScript Clipboard API (web-only, not suitable for desktop fullscreen apps).
- **Notes:**
  - Supports text and images.
  - TS side: `await clipboard.readText()` or `writeText(text)`.
  - For undo/redo of clipboard ops, issue a command through S05.
- **Pixhaus streams using it:** S15 (brush tools), S16 (selection operations).

### tauri-plugin-deep-link

- **Purpose:** Handle deep-link URIs (custom protocol, e.g., `pixhaus://open/project.pixhaus`).
- **Crates.io:** https://crates.io/crates/tauri-plugin-deep-link
- **Docs:** https://docs.rs/tauri-plugin-deep-link/latest/tauri_plugin_deep_link/
- **Repo:** https://github.com/tauri-apps/plugins-workspace
- **License:** MIT
- **Maintenance (May 2026):** Official, active.
- **When to use:** Pixhaus website or social links could use pixhaus:// URIs to open projects directly. Less critical for MVP.
- **Alternatives:** File associations (via installer configuration).
- **Notes:**
  - Register URI scheme in tauri.conf.json and install URL handler on first run.
  - TS side: listen for `tauri://deep-link` event.
- **Pixhaus streams using it:** S13 (advanced integration), S46 (branding / website integration).

### tauri-plugin-notification

- **Purpose:** Desktop notifications (toast alerts).
- **Crates.io:** https://crates.io/crates/tauri-plugin-notification
- **Docs:** https://docs.rs/tauri-plugin-notification/latest/tauri_plugin_notification/
- **Repo:** https://github.com/tauri-apps/plugins-workspace
- **License:** MIT
- **Maintenance (May 2026):** Official, active.
- **When to use:** Pixhaus can notify user of background tasks (verb completion, file saved, error occurred).
- **Alternatives:** In-app toast (no OS integration).
- **Notes:**
  - Cross-platform; uses WinToast (Windows), NSUserNotification (macOS), D-Bus (Linux).
  - TS side: `await notify.send({ title, body })`.
- **Pixhaus streams using it:** S21 (verb runtime completion notifications), S51 (crash reporting notification).

### tauri-plugin-single-instance

- **Purpose:** Enforce single-instance mode. Only one Pixhaus window can be open; opening again sends event to existing instance.
- **Crates.io:** https://crates.io/crates/tauri-plugin-single-instance
- **Docs:** https://docs.rs/tauri-plugin-single-instance/latest/tauri_plugin_single_instance/
- **Repo:** https://github.com/tauri-apps/plugins-workspace
- **License:** MIT
- **Maintenance (May 2026):** Official, active.
- **When to use:** Pixhaus should probably run single-instance (only one window at a time, to avoid state confusion). Configurable in tauri.conf.json.
- **Alternatives:** Manual IPC socket (more work).
- **Notes:**
  - If user clicks Pixhaus icon while it's running, the existing window comes to foreground.
  - Can pass args to the running instance (e.g., open a file).
- **Pixhaus streams using it:** S13 (app shell).

---

## Async Runtime & Parallelism

### tokio

- **Purpose:** Async runtime for Rust. Powers all concurrent I/O (file, network, timers).
- **Crates.io:** https://crates.io/crates/tokio
- **Docs:** https://docs.rs/tokio/latest/tokio/
- **Repo:** https://github.com/tokio-rs/tokio
- **License:** MIT
- **Maintenance (May 2026):** Active. Latest stable: 1.x LTS versions (1.47.x until Sept 2026 MSRV 1.70; 1.51.x until March 2027 MSRV 1.71). Tokio now celebrates 10 years (2015-2025) and remains the de facto async runtime in Rust.
- **When to use:** Pixhaus Rust core uses tokio for all async work: file I/O (S07 project loading), HTTP requests (S22 backend adapters, S02 Lospec API calls), verb runtime (S21).
- **Alternatives:**
  - `async-std` (similar API, less widely used, declining adoption as of 2026).
  - `smol` (lightweight, no ecosystem ecosystem pressure).
  - A single-threaded runtime if Pixhaus were ever single-threaded (unlikely for a desktop app).
- **Notes:**
  - Multi-threaded runtime is default and correct for Pixhaus.
  - Key features: tokio::fs, tokio::net, tokio::task::spawn, tokio::time.
  - Minimum rustc: 1.70 for 1.47.x LTS.
  - Tauri 2.x auto-provides tokio; Pixhaus code uses it implicitly for all async ops.
- **Pixhaus streams using it:** Every stream that uses async (S21, S22, S07, S08, etc.).

### rayon

- **Purpose:** Data-parallelism library. Convert sequential iterators to parallel with `.par_iter()`.
- **Crates.io:** https://crates.io/crates/rayon
- **Docs:** https://docs.rs/rayon/latest/rayon/
- **Repo:** https://github.com/rayon-rs/rayon
- **License:** MIT or Apache-2.0
- **Maintenance (May 2026):** Active. Requires rustc 1.85.0+.
- **When to use:** Pixhaus uses rayon for CPU-bound pixel operations: blend mode compositing (S01), filter application, palette quantization (S11 GIF export). Example: compositing 50 layers into 256x256 pixel buffer; rayon's work-stealing scheduler divides the pixel grid across cores.
- **Alternatives:**
  - Manual thread pooling (more control, more boilerplate).
  - Sequential iterators (incorrect if CPU-bound work is done in the hot path).
- **Notes:**
  - Zero-cost abstraction; no allocations in the hot path if used correctly.
  - Work-stealing scheduler adapts to runtime load; no manual thread count tuning.
  - Cannot spawn tasks across futures/async; use for blocking CPU work, not I/O.
- **Pixhaus streams using it:** S01 (blend modes), S11 (GIF quantization), S14 (canvas compositing).

### futures, futures-util

- **Purpose:** Utilities for composing async code. Traits (Stream, Sink, Future), combinators (map, filter, zip), executors.
- **Crates.io:**
  - https://crates.io/crates/futures
  - https://crates.io/crates/futures-util
- **Docs:**
  - https://docs.rs/futures/latest/futures/
  - https://docs.rs/futures-util/latest/futures_util/
- **Repo:** https://github.com/rust-lang/futures-rs
- **License:** MIT or Apache-2.0
- **Maintenance (May 2026):** Active (part of Rust async ecosystem).
- **When to use:** Pixhaus uses futures combinators for verb streaming (S21). Example: a verb emits a stream of progress updates; futures::stream::iter() + map() + collect() to process them.
- **Alternatives:**
  - `tokio::sync::mpsc` (channels; simpler for point-to-point).
  - `async-stream` crate (easier syntax for generators).
- **Notes:**
  - futures 0.3.x is stable; 1.0 not yet released (still in discussion).
  - Most commonly used: `futures::future::join_all()`, `try_join_all()`, `select_all()`.
  - `futures-util` is a re-export of common combinators; depends on `futures`.
- **Pixhaus streams using it:** S21 (verb streaming), S22 (backend adapters with multiple futures).

### crossbeam

- **Purpose:** Low-level concurrency primitives. Channels (mpmc), work-stealing deques, epoch-based GC, synchronization (Parker, WaitGroup).
- **Crates.io:** https://crates.io/crates/crossbeam
- **Docs:** https://docs.rs/crossbeam/latest/crossbeam/
- **Repo:** https://github.com/crossbeam-rs/crossbeam
- **License:** MIT or Apache-2.0
- **Maintenance (May 2026):** Active.
- **When to use:** Pixhaus may use crossbeam channels for inter-thread communication (e.g., verb runtime spawns a background task that sends progress updates back to the main UI thread). For most cases, tokio::sync::mpsc is preferred; use crossbeam if you need multiple consumers or unbound queues.
- **Alternatives:** tokio::sync (for tokio-based apps), std::sync::mpsc (single consumer, standard library).
- **Notes:**
  - crossbeam::channel supports multiple producers and consumers (mpmc); std::sync::mpsc does not.
  - crossbeam_epoch is for lock-free data structures; likely overkill for Pixhaus.
  - crossbeam::thread::scope() for scoped threads (deprecated in favor of std::thread::scope in Rust 1.63+).
- **Pixhaus streams using it:** S21 (verb parallelism), potentially S49 (test harness).

### parking_lot

- **Purpose:** Drop-in replacement for std::sync::Mutex and RwLock. 1.5-5x faster, smaller, more flexible.
- **Crates.io:** https://crates.io/crates/parking_lot
- **Docs:** https://docs.rs/parking_lot/latest/parking_lot/
- **Repo:** https://github.com/Amanieu/parking_lot
- **License:** MIT or Apache-2.0
- **Maintenance (May 2026):** Active. MSRV: 1.84.
- **When to use:** Pixhaus Project state should use parking_lot::RwLock instead of std::sync::RwLock for the main project data. Multiple threads will read the project simultaneously (viewer, undo stack, AI verbs); parking_lot's fairness guarantees prevent writer starvation.
- **Alternatives:** std::sync::Mutex (slower, larger), tokio::sync::Mutex (for async-only code).
- **Notes:**
  - Mutex is 1 byte, RwLock is 1 word (vs. platform-dependent sizes in std).
  - Supports downgrading write lock to read lock: `let r = w.downgrade()`.
  - Not async-aware; use tokio::sync for async contexts.
- **Pixhaus streams using it:** S05 (undo/redo stack access), S13 (project state access from multiple UI panels).

### async-trait

- **Purpose:** Macro to enable async fn in trait definitions (before Rust 1.75 stable support; now mostly for dynamic dispatch).
- **Crates.io:** https://crates.io/crates/async-trait
- **Docs:** https://docs.rs/async-trait/latest/async_trait/
- **Repo:** https://github.com/dtolnay/async-trait
- **License:** MIT or Apache-2.0
- **Maintenance (May 2026):** Active. Rust 1.75+ has native async-in-traits support, but async-trait is still needed for dyn Trait.
- **When to use:** Pixhaus Backend adapters (S22) use `#[async_trait] pub trait InferenceBackend { async fn invoke(...) }` for polymorphism. Without async-trait, you'd need `-> Pin<Box<dyn Future>>`, which is verbose.
- **Alternatives:**
  - Native async-in-traits (Rust 1.75+) if not using dyn.
  - Manual `-> Pin<Box<dyn Future>>` returns (verbose).
- **Notes:**
  - Heap-allocates the Future; minor overhead for high-throughput code, negligible for I/O-bound verbs.
  - Required for: `dyn InferenceBackend`, `dyn Command` (from S05).
- **Pixhaus streams using it:** S22 (backend trait), S37 (plugin trait).

---

## Serialization

### serde, serde_json

- **Purpose:** Framework for serializing/deserializing Rust types to/from text (JSON, YAML, TOML) or binary (bincode, postcard, rmp) formats.
- **Crates.io:**
  - https://crates.io/crates/serde
  - https://crates.io/crates/serde_json
- **Docs:**
  - https://docs.rs/serde/latest/serde/
  - https://docs.rs/serde_json/latest/serde_json/
- **Repo:** https://github.com/serde-rs/serde
- **License:** MIT or Apache-2.0
- **Maintenance (May 2026):** Stable. serde is the de facto serialization framework for Rust (used by tokio, reqwest, tauri, etc.).
- **When to use:** Pixhaus uses serde for:
  - JSON config files (preferences, project metadata).
  - IPC serialization (S05 undo/redo commands, S22 backend requests).
  - Interop with TS frontend (ts-rs generates TS interfaces from `#[derive(Serialize, Deserialize)]` Rust types).
- **Alternatives:** Manual serialization (error-prone), other frameworks (rarely needed).
- **Notes:**
  - serde_json is the JSON implementation; other formats (TOML, YAML, MessagePack, etc.) are separate crates.
  - All Pixhaus data model types should `#[derive(Serialize, Deserialize)]`.
- **Pixhaus streams using it:** All streams that handle data (S02, S05, S07, S08, S13, S21, etc.).

### rmp-serde, rmp

- **Purpose:** MessagePack serialization via serde. Binary format, compact, fast.
- **Crates.io:**
  - https://crates.io/crates/rmp-serde
  - https://crates.io/crates/rmp
- **Docs:**
  - https://docs.rs/rmp-serde/latest/rmp_serde/
  - https://docs.rs/rmp/latest/rmp/
- **Repo:** https://github.com/3Hren/msgpack-rust
- **License:** MIT
- **Maintenance (May 2026):** Active. Latest stable: 1.3.1 (rmp-serde); used in production by many projects.
- **When to use:** Pixhaus native format (S07) uses MessagePack for the data model payload (core layers, palette, animation data). Smaller and faster than JSON for binary blobs.
- **Alternatives:** postcard (smaller, no_std), bincode (smaller still, but now unmaintained as of 2025-2026).
- **Notes:**
  - rmp-serde is the serde integration; rmp is the low-level encoder/decoder.
  - Binary data (pixel buffers) should be stored separately (zstd-compressed) not inside MessagePack for Pixhaus.
- **Pixhaus streams using it:** S07 (native format).

### postcard

- **Purpose:** serde-compatible serialization targeting no_std + embedded. Extremely compact.
- **Crates.io:** https://crates.io/crates/postcard
- **Docs:** https://docs.rs/postcard/latest/postcard/
- **Repo:** https://github.com/jamesmunns/postcard
- **License:** MIT or Apache-2.0
- **Maintenance (May 2026):** Active. Version 1.1.3+. Actively maintained with 60+ contributors and used by 7,000+ other projects.
- **When to use:** Pixhaus does not target embedded, so postcard is not necessary. If Pixhaus ever supports no_std contexts (unlikely), postcard would be the choice. For now, rmp-serde or serde_json suffice.
- **Alternatives:** rmp-serde (more readable), bincode (now unmaintained).
- **Notes:**
  - v1.0+ has stable wire format.
  - Optimized for size and speed, not readability.
- **Pixhaus streams using it:** Not currently used; consider if plugin system needs no_std support.

### bincode

- **Purpose:** Fast binary serialization via serde.
- **Crates.io:** https://crates.io/crates/bincode
- **Docs:** https://docs.rs/bincode/latest/bincode/
- **Repo:** https://github.com/bincode-org/bincode
- **License:** MIT
- **Maintenance (May 2026):** UNMAINTAINED as of 2025 (RUSTSEC-2025-0141). Security fix has been slow in coming; new projects should avoid bincode.
- **When to use:** Do NOT use in Pixhaus.
- **Alternatives:** postcard (drop-in replacement, actively maintained, 60+ contributors), rmp-serde (slower but mature), bitcode (emerging).
- **Notes:**
  - The bincode team marked it unmaintained but still usable; however, starting a new project with it is not recommended.
  - If Pixhaus previously used bincode, migrate to postcard or rmp-serde.
- **Pixhaus streams using it:** Avoid. Use rmp-serde or postcard.

---

## Error Handling

### thiserror

- **Purpose:** Derive macro for std::error::Error. For library-level custom error types.
- **Crates.io:** https://crates.io/crates/thiserror
- **Docs:** https://docs.rs/thiserror/latest/thiserror/
- **Repo:** https://github.com/dtolnay/thiserror
- **License:** MIT or Apache-2.0
- **Maintenance (May 2026):** Stable. Part of dtolnay's ecosystem (well-maintained).
- **When to use:** Pixhaus Rust core defines custom error enums for specific subsystems:
  - `core::error::CoreError` (pixel ops, blend modes).
  - `io::error::FormatError` (file I/O, Aseprite parsing).
  - Each derives Error via thiserror, implementing Display + Error traits automatically.
- **Alternatives:** Manual impl Error trait (boilerplate), anyhow/eyre (wrong tier; they consume errors, not define them).
- **Notes:**
  - Use in libraries and library-like modules (core/*, io/*).
  - Do NOT use thiserror in application code (use anyhow instead).
  - Thiserror deliberately does NOT appear in your public API; switching from thiserror to manual impl is not a breaking change.
- **Pixhaus streams using it:** S01 (pixel ops errors), S07 (format errors), S08 (Aseprite parse errors), S09 (PSD errors).

### anyhow

- **Purpose:** Flexible error type for application code. Ergonomic error propagation with `.context()` and `?` operator.
- **Crates.io:** https://crates.io/crates/anyhow
- **Docs:** https://docs.rs/anyhow/latest/anyhow/
- **Repo:** https://github.com/dtolnay/anyhow
- **License:** MIT or Apache-2.0
- **Maintenance (May 2026):** Stable. Widely used in production.
- **When to use:** Pixhaus application code (main.rs, UI handlers, verb runtime, backend adapters) uses `anyhow::Result<T>` for error handling. Error context is added with `.context("operation failed")`.
- **Alternatives:** eyre (similar API, slightly richer output), miette (more structured diagnostics).
- **Notes:**
  - anyhow is simpler and more lightweight than eyre/miette.
  - Core principle: library code returns custom error types (thiserror); application code uses anyhow to wrap them.
- **Pixhaus streams using it:** S21 (verb runtime), S22 (backend adapters), S13 (shell error handling).

### eyre, color-eyre

- **Purpose:** Fork of anyhow with richer error reporting. Supports custom report formats via EyreHandler.
- **Crates.io:**
  - https://crates.io/crates/eyre
  - https://crates.io/crates/color-eyre
- **Docs:**
  - https://docs.rs/eyre/latest/eyre/
  - https://docs.rs/color-eyre/latest/color_eyre/
- **Repo:**
  - https://github.com/eyre-rs/eyre
  - https://github.com/eyre-rs/color-eyre
- **License:** MIT
- **Maintenance (May 2026):** Active.
- **When to use:** Optional for Pixhaus. If CLI tools or debug logging benefit from richer error formatting, use eyre + color-eyre. For the main app, anyhow is sufficient.
- **Alternatives:** anyhow (simpler, 99% of use cases), miette (more diagnostic-focused).
- **Notes:**
  - eyre::Report is a trait object like anyhow::Error but with customizable formatting.
  - color-eyre installs a handler that pretty-prints backtraces.
- **Pixhaus streams using it:** S49 (CI/CD scripts if any error output is user-facing), not required for main app.

### miette

- **Purpose:** Diagnostic error reporting with fancy formatted output, source snippets, colorized rendering.
- **Crates.io:** https://crates.io/crates/miette
- **Docs:** https://docs.rs/miette/latest/miette/
- **Repo:** https://github.com/zkat/miette
- **License:** Apache-2.0
- **Maintenance (May 2026):** Active.
- **When to use:** If Pixhaus includes a scripting system (S38 Lua) or a plugin DSL, miette provides good diagnostic messages for user-facing parsing or script errors. Not needed for normal Pixhaus operation.
- **Alternatives:** anyhow (simpler), eyre (similar tier).
- **Notes:**
  - Miette provides a Diagnostic trait (like Error but more structured).
  - Includes anyhow/eyre-style context helpers (WrapErr, Context).
  - As of 2026, miette has NOT overtaken eyre as the standard; eyre + anyhow remains the norm.
- **Pixhaus streams using it:** S38 (Lua error diagnostics), S37 (plugin loading errors if high-quality reporting is desired).

---

## Networking & HTTP

### reqwest

- **Purpose:** Async HTTP client. Built on hyper + tokio. Supports JSON, multipart, cookies, proxies, TLS.
- **Crates.io:** https://crates.io/crates/reqwest
- **Docs:** https://docs.rs/reqwest/latest/reqwest/
- **Repo:** https://github.com/seanmonstar/reqwest
- **License:** MIT or Apache-2.0
- **Maintenance (May 2026):** Active. Latest stable: 0.13.x. Most popular async HTTP client in Rust.
- **When to use:** Pixhaus uses reqwest for:
  - S22 backend adapters: HTTP calls to Anthropic API, OpenAI API, Replicate, Stability, etc.
  - S02 color palette: Lospec API calls to fetch palette metadata.
  - S51 crash reporting: POST to Sentry or GlitchTip.
- **Alternatives:**
  - ureq (blocking, lighter weight, useful if no async needed).
  - hyper (lower-level, more control, more boilerplate).
- **Notes:**
  - By default uses rustls (pure Rust TLS); optionally use native-tls for OS-level TLS.
  - Connection pooling is automatic.
  - JSON serialization/deserialization via serde built-in.
- **Pixhaus streams using it:** S22 (backend adapters), S02 (Lospec), S51 (crash reporting).

### ureq

- **Purpose:** Lightweight synchronous (blocking) HTTP client. Pure Rust, minimal dependencies.
- **Crates.io:** https://crates.io/crates/ureq
- **Docs:** https://docs.rs/ureq/latest/ureq/
- **Repo:** https://github.com/algesten/ureq
- **License:** MIT or Apache-2.0
- **Maintenance (May 2026):** Active.
- **When to use:** Pixhaus does not use ureq (no synchronous HTTP needed). If a plugin system allows blocking plugins, ureq could be a convenience for plugin authors (lighter dependency than reqwest).
- **Alternatives:** reqwest (async, standard library).
- **Notes:**
  - Useful in CLI tools or simple scripts.
  - Not suitable for high-concurrency servers.
- **Pixhaus streams using it:** Potentially S37 (plugin helper libraries), not core.

### tungstenite, tokio-tungstenite

- **Purpose:** WebSocket implementation. tungstenite is the base library; tokio-tungstenite adds async/await support.
- **Crates.io:**
  - https://crates.io/crates/tungstenite
  - https://crates.io/crates/tokio-tungstenite
- **Docs:**
  - https://docs.rs/tungstenite/latest/tungstenite/
  - https://docs.rs/tokio-tungstenite/latest/tokio_tungstenite/
- **Repo:**
  - https://github.com/snapview/tungstenite-rs
  - https://github.com/snapview/tokio-tungstenite
- **License:** MIT or Apache-2.0
- **Maintenance (May 2026):** Active. tokio-tungstenite 0.28.0 is the latest stable (as of Jan 2026).
- **When to use:** Pixhaus does not currently use WebSockets (all IPC is via Tauri's invoke). If the verb runtime (S21) ever uses a streaming backend (e.g., Claude streaming API), consider tokio-tungstenite. Reqwest now supports streaming responses, so tungstenite may not be needed.
- **Alternatives:** Reqwest streaming (simpler for HTTP), hyper (lower-level).
- **Notes:**
  - Lightweight and fast.
  - Recent versions (0.26.2+) are performant.
- **Pixhaus streams using it:** Not currently used, but could support S21 if streaming becomes a requirement.

### hyper

- **Purpose:** Low-level HTTP implementation. Used internally by reqwest.
- **Crates.io:** https://crates.io/crates/hyper
- **Docs:** https://docs.rs/hyper/latest/hyper/
- **Repo:** https://github.com/hyperium/hyper
- **License:** MIT
- **Maintenance (May 2026):** Active.
- **When to use:** Pixhaus does not use hyper directly (reqwest abstracts it). Only relevant if custom HTTP protocol handling is needed (unlikely).
- **Alternatives:** reqwest (higher-level, recommended).
- **Notes:**
  - Part of the tokio ecosystem.
- **Pixhaus streams using it:** None directly; dependency of reqwest.

---

## Observability & Logging

### tracing, tracing-subscriber

- **Purpose:** Structured event and span-based logging. More powerful than log crate; integrates with OpenTelemetry.
- **Crates.io:**
  - https://crates.io/crates/tracing
  - https://crates.io/crates/tracing-subscriber
- **Docs:**
  - https://docs.rs/tracing/latest/tracing/
  - https://docs.rs/tracing-subscriber/latest/tracing_subscriber/
- **Repo:** https://github.com/tokio-rs/tracing
- **License:** MIT
- **Maintenance (May 2026):** Active. Stable. Latest: tracing 0.27.x. De facto standard for Rust observability.
- **When to use:** Pixhaus uses tracing for all structured logging:
  - `tracing::info!("verb invoked", verb = "inbetween")`.
  - `#[tracing::instrument]` on async functions for automatic span creation.
  - Subscriber setup in main.rs to emit logs to files (via tauri-plugin-log or custom writer).
- **Alternatives:**
  - log + env_logger (simpler, legacy; lacks spans and structured fields).
  - slog (more opinionated, declining usage).
- **Notes:**
  - Spans track operations over time (start, end, duration).
  - Structured fields are logged as key-value pairs (JSON if configured).
  - Zero overhead for disabled spans/events.
  - Interop with log crate: tracing-log bridge layer.
- **Pixhaus streams using it:** S21 (verb runtime), S13 (shell startup), S49 (CI/CD).

### tracing-tree

- **Purpose:** Pretty, tree-formatted output for tracing spans (for local debugging).
- **Crates.io:** https://crates.io/crates/tracing-tree
- **Docs:** https://docs.rs/tracing-tree/latest/tracing_tree/
- **Repo:** https://github.com/tokio-rs/tracing/tree/master/tracing-tree
- **License:** MIT
- **Maintenance (May 2026):** Active (part of tokio-rs/tracing monorepo).
- **When to use:** Optional. For local development, tracing-tree provides a readable nested tree of spans in console output (helpful for debugging async code flow).
- **Alternatives:** tracing-subscriber's default fmt layer (less visual), custom subscribers.
- **Notes:**
  - Useful for dev-time debugging; typically disabled in production.
  - Can be combined with file logging (tracing-subscriber with file writer).
- **Pixhaus streams using it:** S13 (optional, for dev builds).

### log, env_logger

- **Purpose:** Legacy logging framework. Simple facade; env_logger provides a basic implementation.
- **Crates.io:**
  - https://crates.io/crates/log
  - https://crates.io/crates/env_logger
- **Docs:**
  - https://docs.rs/log/latest/log/
  - https://docs.rs/env_logger/latest/env_logger/
- **Repo:**
  - https://github.com/rust-lang/log
  - https://github.com/rust-lang/env_logger
- **License:** MIT or Apache-2.0
- **Maintenance (May 2026):** Stable (log is in Rust stdlib, unmaintained but stable; env_logger is actively maintained).
- **When to use:** Do NOT use in Pixhaus. tracing is the modern standard. log is a facade that many crates expose for dependency injection; Pixhaus should provide tracing integration for those crates if needed (via tracing-log bridge).
- **Alternatives:** tracing (modern).
- **Notes:**
  - Log crate is the standard facade; many libraries expose log interfaces.
  - To bridge log crate output to tracing, use tracing-log crate.
- **Pixhaus streams using it:** Not directly used; dependencies may expose log interfaces (handled via tracing-log if needed).

### sentry-rust, sentry-tracing

- **Purpose:** Crash reporting and error tracking. sentry-rust is the main SDK; sentry-tracing integrates with tracing spans.
- **Crates.io:**
  - https://crates.io/crates/sentry
  - https://crates.io/crates/sentry-tracing
- **Docs:**
  - https://docs.rs/sentry/latest/sentry/
  - https://docs.rs/sentry-tracing/latest/sentry_tracing/
- **Repo:** https://github.com/getsentry/sentry-rust
- **License:** Apache-2.0 or MIT
- **Maintenance (May 2026):** Active. Official Sentry SDK.
- **When to use:** Pixhaus (S51 crash reporting) integrates Sentry for opt-in error tracking. Captures panics and errors, tags with OS/version, strips PII.
- **Alternatives:** GlitchTip (self-hosted, Sentry-compatible). Sentry Free tier is available but has usage limits.
- **Notes:**
  - sentry-tracing layer automatically converts tracing spans to Sentry transactions.
  - Can be configured to send errors to either Sentry cloud or self-hosted GlitchTip.
  - Opt-in only; off by default.
- **Pixhaus streams using it:** S51 (crash reporting).

---

## System & OS APIs

### directories, xdg

- **Purpose:** Locate platform-specific standard directories (config, cache, data, runtime).
- **Crates.io:**
  - https://crates.io/crates/directories (modern, recommended)
  - https://crates.io/crates/xdg (XDG-specific for Linux)
- **Docs:**
  - https://docs.rs/directories/latest/directories/
  - https://docs.rs/xdg/latest/xdg/
- **Repo:**
  - https://github.com/xdg-rs/dirs
  - https://github.com/whitequark/rust-xdg
- **License:** MIT or Apache-2.0
- **Maintenance (May 2026):** Active.
- **When to use:** Pixhaus uses directories to store:
  - Config: `~/.config/pixhaus/preferences.json` (Linux), `~/Library/Preferences/io.pixhaus.pixhaus/` (macOS), `%APPDATA%\Pixhaus\config` (Windows).
  - Cache: plugin cache, compiled shader cache.
  - Data: recent projects, logs.
  - Runtime: sockets for single-instance communication.
- **Alternatives:**
  - Manual hardcoded paths (fragile, non-standard).
  - std::env::home_dir() + hardcoded relative paths (deprecated).
- **Notes:**
  - directories crate is higher-level and cross-platform (recommended).
  - xdg is Linux-only but more detailed.
  - Use directories::ProjectDirs::from(qualifier, organization, application) to get standard paths.
- **Pixhaus streams using it:** S13 (preferences), S50 (release artifacts).

### keyring

- **Purpose:** Access OS keychain for storing secrets (API keys, tokens) securely.
- **Crates.io:** https://crates.io/crates/keyring
- **Docs:** https://docs.rs/keyring/latest/keyring/
- **Repo:** https://github.com/hwchen/keyring-rs
- **License:** MIT
- **Maintenance (May 2026):** Active. Version 3.6.3+.
- **When to use:** Pixhaus (S22 backend adapters) stores API keys for Anthropic, OpenAI, Replicate, etc. in the OS keychain instead of plaintext config. Preferences UI provides a form to set/update API keys; they are stored in Keychain (macOS), Credential Manager (Windows), or Secret Service (Linux).
- **Alternatives:**
  - Plaintext in config (insecure, never do this).
  - Encrypted config file (more work).
- **Notes:**
  - Cross-platform: uses native keychains on each OS.
  - Synchronous API (blocking); call from Tauri backend, not UI thread.
- **Pixhaus streams using it:** S22 (backend API key management), S13 (preferences UI form inputs).

### notify

- **Purpose:** Cross-platform file system event watching. Debounced or raw events.
- **Crates.io:** https://crates.io/crates/notify
- **Docs:** https://docs.rs/notify/latest/notify/
- **Repo:** https://github.com/notify-rs/notify
- **License:** CC0 (public domain) or MIT
- **Maintenance (May 2026):** Active. Used by cargo watch, rust-analyzer, alacritty, mdBook, etc.
- **When to use:** Optional for Pixhaus. If a "watch mode" is implemented (auto-reload when a project file changes), use notify. Less critical for MVP.
- **Alternatives:**
  - Manual file stat checking (polling, inefficient).
  - OS-specific APIs (platform-dependent).
- **Notes:**
  - notify-debouncer-mini provides convenient debouncing.
  - Default API debounces events; raw API sends all events.
- **Pixhaus streams using it:** Not MVP; potential for S13 (watch mode if implemented).

### tempfile

- **Purpose:** Cross-platform temporary files and directories.
- **Crates.io:** https://crates.io/crates/tempfile
- **Docs:** https://docs.rs/tempfile/latest/tempfile/
- **Repo:** https://github.com/Stebalien/tempfile
- **License:** MIT or Apache-2.0
- **Maintenance (May 2026):** Active.
- **When to use:** Pixhaus uses tempfile for:
  - Exporting to temporary files before moving to user's chosen destination (atomic writes).
  - GIF quantization working buffers (S11).
  - Test fixtures.
- **Alternatives:** std::fs::File (less safe), manual /tmp paths (non-portable).
- **Notes:**
  - Automatically cleaned up on drop (RAII).
  - Cross-platform.
- **Pixhaus streams using it:** S07 (save), S11 (export).

### which

- **Purpose:** Locate executable binaries in PATH.
- **Crates.io:** https://crates.io/crates/which
- **Docs:** https://docs.rs/which/latest/which/
- **Repo:** https://github.com/harryfod/which-rs
- **License:** MIT
- **Maintenance (May 2026):** Active.
- **When to use:** If Pixhaus shells out to ffmpeg (S11 MP4 export) or ImageMagick (palette analysis), use which to locate the binary. Fallback to a bundled binary or error gracefully if not found.
- **Alternatives:**
  - Hardcoded paths (fragile).
  - Assume PATH is set correctly (fragile).
- **Notes:**
  - Cross-platform (Windows, Unix).
- **Pixhaus streams using it:** S11 (ffmpeg locating).

### open

- **Purpose:** Open a URL or file with the default application.
- **Crates.io:** https://crates.io/crates/open
- **Docs:** https://docs.rs/open/latest/open/
- **Repo:** https://github.com/Byron/open-rs
- **License:** MIT or Apache-2.0
- **Maintenance (May 2026):** Active.
- **When to use:** Pixhaus can use open to launch the browser for docs links, GitHub, or Lospec website.
- **Alternatives:** tauri-plugin-shell (sandboxed subprocess).
- **Notes:**
  - Cross-platform.
  - Simple: `open::that("https://docs.pixhaus.app")`.
- **Pixhaus streams using it:** S13 (help menu), S41 (docs links).

### atty, supports-color

- **Purpose:** Detect if terminal supports colors / if stdout is a terminal.
- **Crates.io:**
  - https://crates.io/crates/atty
  - https://crates.io/crates/supports-color
- **Docs:**
  - https://docs.rs/atty/latest/atty/
  - https://docs.rs/supports-color/latest/supports_color/
- **Repo:**
  - https://github.com/matklad/atty
  - https://github.com/zkat/supports-color
- **License:** MIT
- **Maintenance (May 2026):** atty is stable (rarely needs changes); supports-color is active.
- **When to use:** CLI tools or debug output (not the main Pixhaus app). If error reporting (S51) or logging includes console output, detect color support.
- **Alternatives:** isatty() via libc (low-level).
- **Notes:**
  - Not needed for GUI app; relevant for CLI scripts or tests.
- **Pixhaus streams using it:** S49 (CI/CD scripts), test harness if any, not main app.

---

## Compression

### zstd

- **Purpose:** Zstandard compression. Fast, high compression ratio. Used in Pixhaus native format.
- **Crates.io:** https://crates.io/crates/zstd
- **Docs:** https://docs.rs/zstd/latest/zstd/
- **Repo:** https://github.com/gyscos/zstd-rs
- **License:** MIT or Apache-2.0
- **Maintenance (May 2026):** Active. Binding to Facebook's zstd C library.
- **When to use:** Pixhaus native format (S07) compresses pixel buffer payloads with zstd. Also useful for S11 (GIF quantization) working buffers if intermediate data is cached.
- **Alternatives:**
  - flate2 (gzip, slower, more widely understood).
  - lz4_flex (lighter, less compression ratio).
  - Pure Rust implementations (slower).
- **Notes:**
  - Uses C bindings by default (fast) but can fall back to pure Rust if needed (feature flag).
  - Compression levels configurable; Pixhaus likely uses a mid-range (e.g., level 6 for balance).
- **Pixhaus streams using it:** S07 (native format), S11 (export optimization).

### flate2

- **Purpose:** DEFLATE (gzip, zlib) compression. Standard, slow, compatible.
- **Crates.io:** https://crates.io/crates/flate2
- **Docs:** https://docs.rs/flate2/latest/flate2/
- **Repo:** https://github.com/rust-lang/flate2-rs
- **License:** MIT or Apache-2.0
- **Maintenance (May 2026):** Active. By default uses miniz_oxide (pure Rust); can use zlib or zlib-ng (C).
- **When to use:** Pixhaus may use flate2 for:
  - PNG export (images::ImageBuffer's PNG encoder uses flate2 internally, usually transparent).
  - Fallback compression if zstd is unavailable.
- **Alternatives:** zstd (faster, better compression), lz4_flex (lighter).
- **Notes:**
  - More compatible than zstd; understood by more decompressors.
  - Slower than zstd; Pixhaus uses zstd by default.
- **Pixhaus streams using it:** S10 (PNG export, indirect via image crate).

### lz4_flex

- **Purpose:** LZ4 compression. Very fast, moderate compression.
- **Crates.io:** https://crates.io/crates/lz4_flex
- **Docs:** https://docs.rs/lz4_flex/latest/lz4_flex/
- **Repo:** https://github.com/PSeitz/lz4_flex
- **License:** MIT
- **Maintenance (May 2026):** Active. Pure Rust implementation.
- **When to use:** Not critical for Pixhaus; zstd is preferred. If very fast compression/decompression of temporary data is needed (e.g., clipboard operations), consider lz4_flex.
- **Alternatives:** zstd (better ratio), flate2 (more compatible).
- **Notes:**
  - Fast compression/decompression.
  - Lower compression ratio than zstd.
- **Pixhaus streams using it:** Not currently planned; optional optimization.

---

## Process & Shell

### duct

- **Purpose:** Builder-style subprocess management. Composable pipelines.
- **Crates.io:** https://crates.io/crates/duct
- **Docs:** https://docs.rs/duct/latest/duct/
- **Repo:** https://github.com/oconnor663/duct.rs
- **License:** MIT or Apache-2.0
- **Maintenance (May 2026):** Active.
- **When to use:** If Pixhaus needs to run external tools (ffmpeg, ImageMagick) synchronously from Rust, duct is ergonomic: `duct::cmd("ffmpeg", &args).run()?`.
- **Alternatives:** xshell (similar), std::process::Command (lower-level), tauri-plugin-shell (sandboxed via IPC).
- **Notes:**
  - Good for one-off commands or pipelines.
  - Blocking; use in tokio::task::spawn_blocking if called from async context.
- **Pixhaus streams using it:** S11 (ffmpeg export, optional).

### xshell

- **Purpose:** Scripting-friendly subprocess API. Re-implements parts of shell semantics in Rust.
- **Crates.io:** https://crates.io/crates/xshell
- **Docs:** https://docs.rs/xshell/latest/xshell/
- **Repo:** https://github.com/matklad/xshell
- **License:** MIT or Apache-2.0
- **Maintenance (May 2026):** Active. Used in rust-analyzer and other projects.
- **When to use:** Build scripts (build.rs, S49 CI) benefit from xshell for ergonomic shell-like commands. Example: `cmd!(sh, "cargo test").run()?`.
- **Alternatives:** duct (more Rust-like), std::process::Command (low-level), actual shell scripts (loses cross-platform safety).
- **Notes:**
  - cmd! macro is convenient.
  - Cross-platform (no bash required).
  - Useful for build scripts, less useful for runtime app code.
- **Pixhaus streams using it:** S49 (CI scripts), build.rs (tauri-specta codegen).

### async-process, subprocess

- **Purpose:** Async-aware subprocess APIs (async-process) or higher-level subprocess management (subprocess).
- **Crates.io:**
  - https://crates.io/crates/async-process
  - https://crates.io/crates/subprocess
- **Docs:**
  - https://docs.rs/async-process/latest/async_process/
  - https://docs.rs/subprocess/latest/subprocess/
- **Repo:**
  - https://github.com/async-rs/async-process
  - https://github.com/hniksic/rust-subprocess
- **License:** MIT or Apache-2.0
- **Maintenance (May 2026):** Active.
- **When to use:** If Pixhaus needs to spawn long-running processes from async code (e.g., verb runtime spawning an ffmpeg subprocess and awaiting completion), use async-process.
- **Alternatives:** tokio::process::Command (simpler, built into tokio), duct with spawn_blocking.
- **Notes:**
  - async-process works with any async runtime (tokio, async-std).
  - subprocess is inspired by Python's subprocess module.
- **Pixhaus streams using it:** S11 (MP4 export from verb runtime, if async).

---

## Graphics & GPU

### wgpu

- **Purpose:** Cross-platform GPU graphics library. Implements WebGPU spec. Compiles to Vulkan, Metal, DirectX 12, WebGL2.
- **Crates.io:** https://crates.io/crates/wgpu
- **Docs:** https://docs.rs/wgpu/latest/wgpu/
- **Repo:** https://github.com/gfx-rs/wgpu
- **License:** MIT or Apache-2.0
- **Maintenance (May 2026):** Active. Part of gfx-rs ecosystem.
- **When to use:** S14 (canvas viewport) does NOT use wgpu directly. WebGL2 is rendered in TypeScript/Solid; if tile compositing (S01) needs GPU acceleration, wgpu could be introduced on the Rust side. Current architecture uses CPU compositing with rayon parallelism.
- **Alternatives:**
  - CPU compositing (current approach, sufficient for 256x256 tiles).
  - OpenGL (lower-level, less portable).
  - Direct use of GPU libraries (Vulkan, Metal, DirectX).
- **Notes:**
  - wgpu is stable and widely used (Bevy game engine, Firefox WebGPU support).
  - Rust shaders use rust-gpu (experimental, targets SPIR-V).
  - For Pixhaus, GPU acceleration is an optimization, not required for MVP.
- **Pixhaus streams using it:** S14 (optional GPU acceleration if needed), not MVP.

### image, imageproc

- **Purpose:** Image encoding/decoding (PNG, JPEG, GIF, TIFF, etc.) and basic image processing.
- **Crates.io:**
  - https://crates.io/crates/image
  - https://crates.io/crates/imageproc
- **Docs:**
  - https://docs.rs/image/latest/image/
  - https://docs.rs/imageproc/latest/imageproc/
- **Repo:**
  - https://github.com/image-rs/image
  - https://github.com/image-rs/imageproc
- **License:** MIT
- **Maintenance (May 2026):** Active.
- **When to use:** Pixhaus uses image for:
  - PNG export (S10 sprite sheets).
  - GIF export (S11) using image's GIF encoder as a base (then swap to dedicated quantizer if custom dithering is needed).
  - PSD import (S09) — may use image-rs PSD decoder if available, or dedicated psd crate.
  - Importing reference images.
- **Alternatives:**
  - Dedicated format crates (image-png, image-gif, etc. from image-rs org).
  - FFmpeg (heavier, external dependency).
- **Notes:**
  - image::ImageBuffer is a flexible generic over pixel types (RGBA, etc.).
  - imageproc has convolution, filters, morphology, etc.; may be useful for S27 (Cleanup verb).
- **Pixhaus streams using it:** S10 (PNG export), S11 (GIF export), S09 (PSD import optional).

---

## Scripting

### mlua

- **Purpose:** High-level Lua 5.4/5.3/5.2/5.1 (+ LuaJIT + Luau) bindings to Rust with async support.
- **Crates.io:** https://crates.io/crates/mlua
- **Docs:** https://docs.rs/mlua/latest/mlua/
- **Repo:** https://github.com/mlua-rs/mlua
- **License:** MIT
- **Maintenance (May 2026):** Active. Updated Apr 4, 2026.
- **When to use:** Pixhaus S38 (Lua scripting) uses mlua to embed Lua 5.4 in the editor. Plugin system allows users to write Lua scripts that register verbs, tools, panels, custom commands.
- **Alternatives:**
  - Python (heavier, more dependencies).
  - WASM (better isolation, slower startup).
  - JavaScript (not suitable for script plugins in a Rust app context).
- **Notes:**
  - mlua_rs fork of rlua; maintained and actively developed.
  - Supports async functions with tokio integration.
  - Sandbox via restricted Lua environment (custom function whitelist).
  - Latest supports Lua 5.4 and Luau (Roblox dialect); Pixhaus likely targets 5.4 for compatibility.
- **Pixhaus streams using it:** S38 (Lua scripting), S37 (plugin loader).

---

## Miscellaneous Utilities

### uuid

- **Purpose:** Generate and parse UUIDs (universally unique identifiers).
- **Crates.io:** https://crates.io/crates/uuid
- **Docs:** https://docs.rs/uuid/latest/uuid/
- **Repo:** https://github.com/uuid-rs/uuid
- **License:** MIT or Apache-2.0
- **Maintenance (May 2026):** Stable and widely used.
- **When to use:** Pixhaus uses UUIDs for:
  - Layer IDs (unique identifier for each layer across undo/redo).
  - Project IDs (for crash reporting context).
  - Command IDs (for analytics / verb tracking).
- **Alternatives:** Manual ID generation (less robust), snowflake IDs (less portable).
- **Notes:**
  - Typically use v4 (random) UUIDs for uniqueness without contention.
  - Serialize with serde via #[serde(serialize_with = "uuid::serde::simple::serialize")].
- **Pixhaus streams using it:** S02 (palette IDs), S05 (command IDs), S13 (project IDs).

### chrono

- **Purpose:** Date and time handling (parsing, formatting, arithmetic).
- **Crates.io:** https://crates.io/crates/chrono
- **Docs:** https://docs.rs/chrono/latest/chrono/
- **Repo:** https://github.com/chronotope/chrono
- **License:** MIT or Apache-2.0
- **Maintenance (May 2026):** Stable (slow release cycle, but solid).
- **When to use:** Pixhaus uses chrono for:
  - Animation frame timing (S19 timeline frame durations).
  - Timestamp recording in project metadata (when was project last saved?).
  - Crash report timestamps (S51).
- **Alternatives:** time crate (competing, less widely used), std::time (no formatting).
- **Notes:**
  - Chrono can be integrated with serde for JSON serialization.
  - Consider time crate as a newer alternative, but chrono is more stable.
- **Pixhaus streams using it:** S05 (undo timestamps), S07 (project metadata), S19 (frame timing), S51 (crash context).

### once_cell, lazy_static

- **Purpose:** Lazy initialization of static values.
- **Crates.io:**
  - https://crates.io/crates/once_cell
  - https://crates.io/crates/lazy_static
- **Docs:**
  - https://docs.rs/once_cell/latest/once_cell/
  - https://docs.rs/lazy_static/latest/lazy_static/
- **Repo:**
  - https://github.com/matklad/once_cell
  - https://github.com/rust-lang-nursery/lazy-static.rs
- **License:** MIT or Apache-2.0
- **Maintenance (May 2026):** once_cell is actively maintained; lazy_static is stable but lower maintenance.
- **When to use:** Pixhaus can use once_cell for:
  - Default keybind map (loaded once at startup).
  - Regex patterns for file parsing (compiled once).
  - Thread pool / worker thread initialization.
- **Alternatives:** std::sync::OnceLock (Rust 1.70+, replaces once_cell).
- **Notes:**
  - once_cell is more flexible and is gradually being replaced by std::sync::OnceLock in stdlib.
  - For Rust 1.70+, prefer OnceLock.
- **Pixhaus streams using it:** S13 (keybind defaults), S07 (regex patterns for parsing).

### nalgebra

- **Purpose:** Linear algebra (vectors, matrices, quaternions) for graphics, physics, game dev.
- **Crates.io:** https://crates.io/crates/nalgebra
- **Docs:** https://docs.rs/nalgebra/latest/nalgebra/
- **Repo:** https://github.com/dimforge/nalgebra
- **License:** MIT or Apache-2.0
- **Maintenance (May 2026):** Active.
- **When to use:** Pixhaus may use nalgebra for:
  - S04 (transform operations): rotation matrices, perspective transforms.
  - S14 (canvas viewport): zoom and pan transforms.
  - S33 (Auto-mesh deformation): mesh deformation rig calculations.
- **Alternatives:**
  - glam (lighter weight, game-dev focused).
  - ultraviolet (no_std, lighter).
  - Custom matrix math (error-prone, reinventing the wheel).
- **Notes:**
  - nalgebra is mature and comprehensive.
  - glam is lighter and faster for graphics; consider glam if nalgebra feels heavy.
- **Pixhaus streams using it:** S04 (transforms), S14 (canvas math), S33 (deformation rig).

### regex

- **Purpose:** Regular expression matching and parsing.
- **Crates.io:** https://crates.io/crates/regex
- **Docs:** https://docs.rs/regex/latest/regex/
- **Repo:** https://github.com/rust-lang/regex
- **License:** MIT or Apache-2.0
- **Maintenance (May 2026):** Stable and mature.
- **When to use:** Pixhaus uses regex for:
  - S08 (Aseprite format parsing): chunk name matching.
  - S38 (Lua scripting): pattern matching in user scripts.
  - S13 (command palette): fuzzy command matching (optional).
- **Alternatives:**
  - Nom (parser combinator, more control but more verbose).
  - Manual string parsing (error-prone).
- **Notes:**
  - Regexes compiled once and cached (via once_cell or OnceLock).
- **Pixhaus streams using it:** S07, S08 (format parsing).

### serde_derive

- **Purpose:** Procedural macros for serde (derive Serialize, Deserialize).
- **Crates.io:** Automatically included via `serde` crate with `derive` feature.
- **Docs:** https://docs.rs/serde_derive/latest/serde_derive/
- **Repo:** https://github.com/serde-rs/serde
- **License:** MIT or Apache-2.0
- **Maintenance (May 2026):** Stable.
- **When to use:** Every type in Pixhaus that is serialized uses `#[derive(Serialize, Deserialize)]`.
- **Notes:**
  - Part of the serde ecosystem; included automatically.
- **Pixhaus streams using it:** All data model types (S02, S05, S07, S10, etc.).

---

## Summary Table

| Category | Recommended Crates | Primary Use |
|----------|-------------------|------------|
| **Tauri Ecosystem** | tauri, tauri-specta, ts-rs, tauri-plugin-* | App shell, IPC, native integrations |
| **Async & Parallelism** | tokio, rayon, parking_lot, async-trait | Async runtime, parallelism, synchronization |
| **Serialization** | serde, serde_json, rmp-serde, postcard | Data serialization/deserialization |
| **Error Handling** | thiserror, anyhow | Library and app error handling |
| **Networking** | reqwest | HTTP client for APIs |
| **Observability** | tracing, tracing-subscriber, sentry | Logging and crash reporting |
| **System** | directories, keyring, notify, tempfile, which, open | Platform integration, file management, secrets |
| **Compression** | zstd, flate2 | Data compression |
| **Subprocess** | duct, xshell, async-process | External command execution |
| **Graphics** | image, imageproc (optional: wgpu) | Image I/O and processing |
| **Scripting** | mlua | Lua plugin support |
| **Math** | nalgebra (or glam) | Linear algebra, transforms |

---

## Key Findings & 2026 Context

### 1. **Tauri 2.x is stable; 3.0 in draft** 
Tauri 3.0 is in early planning (draft milestone) with no public timeline. Pixhaus should target Tauri 2.x and plan for flexibility if 3.0 arrives in 2027-2028. The plugin ecosystem has matured; all critical plugins (updater, store, clipboard, dialog) are official and actively maintained.

### 2. **Tokio dominance confirmed**
Tokio 1.x LTS branches (1.47.x through 2026, 1.51.x through March 2027) show the ecosystem's commitment to long-term stability. async-std and smol have receded; Tokio remains the de facto async runtime (10-year anniversary in 2025).

### 3. **Specta/tauri-specta replaces ts-rs for IPC**
ts-rs is one-way (TS generation only); tauri-specta includes command invocation macros and is the recommended choice for Tauri 2.x. Both can coexist (ts-rs for data models, tauri-specta for commands).

### 4. **tracing is the new standard; log is legacy**
tracing 0.27.x is the modern observability choice with span support and OpenTelemetry integration. The log crate remains a facade for library compatibility; Pixhaus should use tracing throughout.

### 5. **Bincode is unmaintained (RUSTSEC-2025-0141)**
As of May 2026, bincode is marked unmaintained. postcard is the drop-in replacement with 60+ contributors and 7,000+ dependents. If Pixhaus used bincode, migrate to postcard.

### 6. **anyhow + thiserror remain the standard**
No disruption here. Library code (io/*, core/*) uses thiserror; application code uses anyhow. eyre and miette are alternatives but not necessary for typical use.

### 7. **zstd preferred for compression**
zstd has faster compression and better ratio than flate2/gzip. Pixhaus native format (S07) uses zstd for pixel buffer payloads.

### 8. **Tauri plugin ecosystem mature**
Official plugins cover all major needs: store, updater, dialog, shell, clipboard, notifications, etc. Third-party plugins exist for specialized use (deep links, window state, logging, OS info).

### 9. **No major async-std/smol consolidation**
Despite earlier speculation, async-std and smol remain separate, niche runtimes. Tokio's ecosystem dominance is unchallenged.

### 10. **async-trait still needed for dyn Trait**
Despite native async-in-traits support (Rust 1.75+), async-trait remains necessary for dynamic dispatch (dyn InferenceBackend, dyn Command). Pixhaus S22 (backend adapters) and S05 (undo/redo) use async-trait.

---

## Crate Count & Recommendations

**Total crates covered:** 50+ (across all categories)

**Critical crates (MVP cannot ship without):**
- tauri, tokio, serde, anyhow, thiserror, reqwest, tracing, directories, image, mlua

**Important but optional (can defer to post-MVP):**
- wgpu (GPU acceleration), notify (file watching), sentry (crash reporting), nalgebra (complex transforms)

**Deprecated or unmaintained (avoid):**
- bincode → use postcard instead
- log (legacy) → use tracing
- async-std (declining) → use tokio

**Suggested additions for future streams:**
- `sqlx` or `rusqlite`: if a local database for project metadata is desired (S07 extensions).
- `prost`: for Protobuf serialization if multi-language RPC is needed.
- `thunk`: for lazy evaluation in the undo/redo tree (S05 optimization).

---

## References

[Tauri 2.0 Stable Release](https://v2.tauri.app/blog/tauri-20/)  
[Tauri Core Ecosystem Releases](https://v2.tauri.app/release/)  
[GitHub - tauri-apps/tauri](https://github.com/tauri-apps/tauri/releases)  
[The Evolution of Async Rust: From Tokio to High-Level Applications](https://blog.jetbrains.com/rust/2026/02/17/the-evolution-of-async-rust-from-tokio-to-high-level-applications/)  
[Tokio - An asynchronous Rust runtime](https://tokio.rs/)  
[GitHub - specta-rs/specta](https://github.com/specta-rs/specta)  
[tauri-specta - Docs.rs](https://docs.rs/crate/tauri-specta/latest)  
[Comparing logging and tracing in Rust - LogRocket Blog](https://blog.logrocket.com/comparing-logging-tracing-rust/)  
[GitHub - tokio-rs/tracing](https://github.com/tokio-rs/tracing)  
[Serde - Serialization framework for Rust](https://serde.rs/)  
[GitHub - serde-rs/serde](https://github.com/serde-rs/serde)  
[GitHub - seanmonstar/reqwest](https://github.com/seanmonstar/reqwest)  
[How to Build HTTP Clients in Rust with Reqwest](https://oneuptime.com/blog/post/2026-01-26-rust-reqwest-http-client/view)  
[GitHub - zkat/miette](https://github.com/zkat/miette)  
[GitHub - eyre-rs/eyre](https://github.com/eyre-rs/eyre)  
[GitHub - dtolnay/thiserror](https://github.com/dtolnay/thiserror)  
[GitHub - dtolnay/anyhow](https://github.com/dtolnay/anyhow)  
[GitHub - snapview/tokio-tungstenite](https://github.com/snapview/tokio-tungstenite)  
[Rust WebSocket Guide: tokio-tungstenite, axum & JoinSet](https://websocket.org/guides/languages/rust/)  
[GitHub - rayon-rs/rayon](https://github.com/rayon-rs/rayon)  
[How to Process Millions of Records with Parallel Jobs in Rust](https://oneuptime.com/blog/post/2026-01-25-process-millions-records-parallel-jobs-rust/view)  
[GitHub - Amanieu/parking_lot](https://github.com/Amanieu/parking_lot)  
[GitHub - zstandard repository](https://github.com/facebook/zstd)  
[zstd - crates.io](https://crates.io/crates/zstd)  
[GitHub - rust-lang/flate2-rs](https://github.com/rust-lang/flate2-rs)  
[GitHub - notify-rs/notify](https://github.com/notify-rs/notify)  
[GitHub - image-rs/image](https://github.com/image-rs/image)  
[GitHub - image-rs/imageproc](https://github.com/image-rs/imageproc)  
[GitHub - mlua-rs/mlua](https://github.com/mlua-rs/mlua)  
[GitHub - hwchen/keyring-rs](https://github.com/hwchen/keyring-rs)  
[GitHub - whitequark/rust-xdg](https://github.com/whitequark/rust-xdg)  
[GitHub - xdg-rs/dirs](https://github.com/xdg-rs/dirs)  
[GitHub - dtolnay/async-trait](https://github.com/dtolnay/async-trait)  
[GitHub - 3Hren/msgpack-rust](https://github.com/3Hren/msgpack-rust)  
[GitHub - jamesmunns/postcard](https://github.com/jamesmunns/postcard)  
[GitHub - crossbeam-rs/crossbeam](https://github.com/crossbeam-rs/crossbeam)  
[GitHub - gfx-rs/wgpu](https://github.com/gfx-rs/wgpu)  
[Rust GPU Programming with wgpu: The 2026 Guide](https://rustify.rs/articles/rust-gpu-computing-wgpu-2026)  
[GitHub - getsentry/sentry-rust](https://github.com/getsentry/sentry-rust)  
[Rust Error Tracking and Performance Monitoring - Sentry](https://sentry.io/for/rust/)  
[GitHub - matklad/xshell](https://github.com/matklad/xshell)  
[GitHub - oconnor663/duct.rs](https://github.com/oconnor663/duct.rs)  
[GitHub - hniksic/rust-subprocess](https://github.com/hniksic/rust-subprocess)  
[GitHub - uuid-rs/uuid](https://github.com/uuid-rs/uuid)  
[GitHub - chronotope/chrono](https://github.com/chronotope/chrono)  
[GitHub - dimforge/nalgebra](https://github.com/dimforge/nalgebra)  
[GitHub - rust-lang/regex](https://github.com/rust-lang/regex)

---

**Document Status:** Complete. 50+ crates covered with maintenance status (May 2026), use cases, alternatives, and Pixhaus stream references.
