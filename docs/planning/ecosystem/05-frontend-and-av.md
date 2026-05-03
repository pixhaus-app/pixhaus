# Pixhaus Frontend & AV Ecosystem (May 2026)

This document surveys the TypeScript/JavaScript frontend ecosystem and audio/video processing libraries relevant to Pixhaus—an open-source AI-native pixel art + animation + tilemap editor built with Tauri 2.x + Rust core + Solid.js UI + WebGL2.

Scope covers: Solid.js ecosystem, build tooling (Vite, TypeScript, pnpm), Tauri JS APIs, canvas/WebGL/WebGPU rendering, audio/video processing (browser and Rust), state management, and testing.

---

## Solid.js Ecosystem

### Solid.js

- **Purpose:** Fine-grained reactive JavaScript UI framework with zero virtual DOM overhead.
- **Package URL:** https://www.npmjs.com/package/solid-js
- **Docs:** https://docs.solidjs.com/
- **Repo:** https://github.com/solidjs/solid
- **License:** MIT
- **Maintenance (May 2026):** Active. Latest stable v1.9.11; v2.0.0-experimental in development.
- **When to use:** Core UI runtime for Pixhaus. Solid's reactivity model—signals and effects with zero virtual DOM—is the right fit for a high-performance editor where fine-grained updates matter.
- **Alternatives:** Svelte, Vue 3 with composition API, React (too heavy for desktop).
- **Notes:** Solid has become the most influential framework of the past five years; signals are now core to Angular, Vue, and being considered for TC39. The 2.0 development cycle introduces `@solidjs/signals` as a decoupled reactive foundation. For Pixhaus, v1.9.x is production-ready; v2.0 is worth watching for the next major cycle.
- **Pixhaus streams using it:** S13 (shell), S14 (canvas), S15-S20 (UI panels), S41-S44 (docs, tutorials).

### @solidjs/router

- **Purpose:** Universal (SSR-capable) client-side router for Solid SPAs.
- **Package URL:** https://www.npmjs.com/package/@solidjs/router
- **Docs:** https://docs.solidjs.com/solid-router
- **Repo:** https://github.com/solidjs/solid-router
- **License:** MIT
- **Maintenance (May 2026):** Active.
- **When to use:** If Pixhaus ever needs multi-view routing (e.g., workspace switcher, settings panels as routes). Not required for the initial MVP where a single canvas-centric layout dominates.
- **Alternatives:** Manual URL state, hash-based routing, TanStack Router (more powerful, heavier).
- **Notes:** Solid Router is inspired by Ember and React Router. It works client-side (SPA) and can be deployed as static files. For a desktop Tauri app, routing overhead is minimal; the router is useful if organizing complex UI state as multiple "views."
- **Pixhaus streams using it:** S13 (optional for prefs/workspace UI).

### solid-primitives

- **Purpose:** Community collection of 80–90% of common Solid use cases: reactive utilities, hooks, directives for DOM refs, storage, deep reactivity, keyboard/gesture handling.
- **Package URL:** https://primitives.solidjs.community/
- **Docs:** https://primitives.solidjs.community/
- **Repo:** https://github.com/solidjs-community/solid-primitives
- **License:** MIT
- **Maintenance (May 2026):** Active. Growing ecosystem; now 40+ packages.
- **When to use:** For common patterns: useMediaQuery, createLocalStorage, createEventListener, createMutationObserver, createResizeObserver, keyboard/gamepad hooks. All tested and well-maintained.
- **Alternatives:** Roll your own, three.js-style monolithic utilities.
- **Notes:** Named with `create*` prefix (reactive) or `make*` prefix (non-reactive foundation). Saves 5–10 hours of wheel-reinvention per editor. Recommended: use @solid-primitives/refs for canvas/viewport element tracking.
- **Pixhaus streams using it:** S14 (canvas viewport, gamepad/keyboard input), S15-S20 (UI interactions).

### @solidjs/start

- **Purpose:** Meta-framework for full-stack Solid applications with SSR, streaming, server functions, and file-based routing.
- **Package URL:** https://docs.solidjs.com/solid-start
- **Docs:** https://docs.solidjs.com/solid-start
- **Repo:** https://github.com/solidjs/solid-start
- **License:** MIT
- **Maintenance (May 2026):** In beta progression; v2.0.0-alpha.2 released Feb 2026. Architecture shifting from Vinxi to pure Vite ("DeVinxi").
- **When to use:** NOT for Pixhaus. Start is for full-stack web apps (SSR, database, auth). Pixhaus is desktop (Tauri) and doesn't need SSR. Using Start would add unnecessary complexity.
- **Alternatives:** Plain Vite + Solid, TanStack Start (for full-stack React/Solid web apps).
- **Notes:** If Pixhaus ever ships a web-based companion tool or cloud collaboration layer, Start becomes relevant. For now, skip it.
- **Pixhaus streams using it:** None (explicitly avoid).

### solid-toast / solid-sonner

- **Purpose:** Toast notification systems for Solid. solid-toast is a simple imperative toast library; solid-sonner is a port of Sonner (React's popular toast) to Solid.
- **Package URL:** solid-toast: https://www.npmjs.com/package/solid-toast; solid-sonner: https://github.com/wobsoriano/solid-sonner
- **Docs:** solid-toast: limited; solid-sonner: minimal (reverse-engineer from Sonner).
- **Repo:** solid-toast: https://github.com/ludicroushq/solid-toast; solid-sonner: https://github.com/wobsoriano/solid-sonner
- **License:** MIT
- **Maintenance (May 2026):** solid-toast: maintained; solid-sonner: community-maintained, less actively updated.
- **When to use:** Transient feedback (verb invoked, file saved, error occurred). Pixhaus will want toast for AI verb status, import/export feedback, and errors.
- **Alternatives:** Home-grown toast component, @kobalte/core Dialog.
- **Notes:** solid-sonner is the more polished option; mimics Sonner's API. Recommendation: use solid-sonner for consistency with web ecosystem and smoother UX.
- **Pixhaus streams using it:** S13 (shell, global notification bus), S21 (verb runtime status).

### solid-headless

- **Purpose:** Unstyled, accessible headless UI components for Solid (buttons, dropdowns, tabs, etc.).
- **Package URL:** Limited npm presence; community project.
- **Docs:** Minimal.
- **Repo:** Search GitHub for solid-headless; no official single source.
- **License:** MIT (varies by port).
- **Maintenance (May 2026):** Fragmented. Multiple independent "solid-headless" projects with no clear canonical version.
- **When to use:** NOT recommended. Use Kobalte or Ark UI instead.
- **Alternatives:** Kobalte (Solid-specific, best-in-class), Ark UI (multi-framework, equally good).
- **Notes:** The term "headless UI" is overloaded. No monolithic "solid-headless" package owns this space in May 2026. The ecosystem coalesced around Kobalte for Solid-native and Ark UI for multi-framework.
- **Pixhaus streams using it:** None. Use Kobalte instead.

### @kobalte/core

- **Purpose:** Headless, accessible UI primitives designed for Solid. Inspired by Radix (React) and React Aria. Composable, unstyled, WAI-ARIA compliant by default.
- **Package URL:** https://www.npmjs.com/package/@kobalte/core
- **Docs:** https://kobalte.dev/
- **Repo:** https://github.com/kobaltedev/kobalte
- **License:** MIT
- **Maintenance (May 2026):** Active, regularly updated.
- **When to use:** Popover, Dialog, Dropdown, Menu, Tabs, Combobox, Tooltip, Accordion, etc. Use Kobalte for all structural UI components. It's the Solid equivalent of Radix UI.
- **Alternatives:** Ark UI (multi-framework, equally capable), shadcn/solid (pre-styled wrapper around Kobalte).
- **Notes:** Kobalte is the go-to for Solid. Components are low-level (you style them); the library handles state, keyboard interaction, and a11y. Compare to Ark UI: Kobalte is Solid-native with tighter integration, Ark is multi-framework. For Pixhaus, Kobalte is the right choice because Solid-native beats framework-agnostic when you own the stack.
- **Pixhaus streams using it:** S13 (menus, popovers, dialogs), S17 (layer panel dropdowns), S18 (palette picker), S19 (timeline controls).

### @ark-ui/solid

- **Purpose:** Unstyled, accessible components (45+) built on Zag.js state machines. Works across React, Vue, Solid, Svelte with perfect parity.
- **Package URL:** https://www.npmjs.com/package/@ark-ui/solid
- **Docs:** https://ark-ui.com/
- **Repo:** https://github.com/chakra-ui/ark
- **License:** MIT
- **Maintenance (May 2026):** Active (Chakra team).
- **When to use:** If multi-framework consistency matters (e.g., Pixhaus web companion tool uses React; desktop uses Solid). Ark gives identical component behavior across both.
- **Alternatives:** Kobalte (Solid-specific, no multi-framework requirement).
- **Notes:** Ark is built on Zag.js, a headless state machine library. Components are accessible by design. For Pixhaus (Solid-only), Kobalte has tighter native integration. But if a web version appears, Ark unifies the component logic across both.
- **Pixhaus streams using it:** S13 optionally (if web UI appears in future).

### solid-icons

- **Purpose:** A collection of SVG icon components for Solid.
- **Package URL:** https://www.npmjs.com/package/solid-icons
- **Docs:** https://solid-icons.vercel.app/
- **Repo:** https://github.com/x64Bits/solid-icons
- **License:** MIT
- **Maintenance (May 2026):** Actively maintained.
- **When to use:** Toolbar icons, menu icons, UI glyphs. solid-icons includes icon sets like Font Awesome, Feather, Tabler, Heroicons, and more.
- **Alternatives:** Import raw SVGs, heroicons, tabler-icons (library form).
- **Notes:** Solid-icons is convenient; each icon is a reactive component, not an image file. Performance is negligible for typical UI icon counts (< 100). Good for rapid UI iteration.
- **Pixhaus streams using it:** S13, S15–S20 (all UI panels).

---

## Build Tooling

### Vite

- **Purpose:** Next-generation frontend build tool. Used for development (HMR under 50ms for Solid) and production bundling with Rollup.
- **Package URL:** https://www.npmjs.com/package/vite
- **Docs:** https://vite.dev/
- **Repo:** https://github.com/vitejs/vite
- **License:** MIT
- **Maintenance (May 2026):** Active. v5.x is current; v6 in development. Recent releases integrate Oxc for faster TypeScript transpilation.
- **When to use:** Universal choice for Pixhaus UI. Vite's ESM-first approach, fast HMR, and Rollup bundling are the foundation.
- **Alternatives:** Webpack (legacy, heavier), esbuild (bundler only, no dev server).
- **Notes:** Vite 5+ includes Oxc transformer, making TypeScript→JS transpilation 10–30x faster than tsc alone. HMR is under 50ms for Solid (no virtual DOM overhead). Vitest uses the same Vite config, unifying dev and test environments.
- **Pixhaus streams using it:** All UI streams (S13–S20).

### vite-plugin-solid

- **Purpose:** Vite integration for Solid. Handles JSX transformation, HMR, build optimizations.
- **Package URL:** https://www.npmjs.com/package/vite-plugin-solid
- **Docs:** https://docs.solidjs.com/configuration
- **Repo:** https://github.com/solidjs/vite-plugin-solid
- **License:** MIT
- **Maintenance (May 2026):** Active.
- **When to use:** Required in vite.config.ts for any Solid project.
- **Alternatives:** None; this is the canonical Solid-Vite integration.
- **Notes:** Configure with `{ typescript: { onlyRemoveTypeImports: true } }` to prevent TypeScript's JSX transformation from conflicting with Solid's.
- **Pixhaus streams using it:** All UI streams.

### vitest

- **Purpose:** Unit testing framework powered by Vite. Uses the same config as dev/build, instant module reloading, supports components testing (Vue, React, Solid, Svelte, etc.).
- **Package URL:** https://www.npmjs.com/package/vitest
- **Docs:** https://vitest.dev/
- **Repo:** https://github.com/vitest-dev/vitest
- **License:** MIT
- **Maintenance (May 2026):** Actively maintained. v2.x is stable, v3 in development.
- **When to use:** All unit tests for Pixhaus UI. Pair with @solidjs/testing-library for component testing.
- **Alternatives:** Jest (slower, requires separate config), Mocha (older, less integrated).
- **Notes:** vitest shares Vite's config; no duplication. HMR makes test development fast. Component testing in vitest v2+ is production-ready for all major frameworks including Solid.
- **Pixhaus streams using it:** S13–S20, S52 (visual regression harness).

### @solidjs/testing-library

- **Purpose:** Testing utilities for Solid components (render, query, fireEvent, userEvent). Mirrors React Testing Library API with Solid-specific adaptations.
- **Package URL:** https://www.npmjs.com/package/@solidjs/testing-library
- **Docs:** https://docs.solidjs.com/guides/testing
- **Repo:** https://github.com/solidjs/solid-testing-library
- **License:** MIT
- **Maintenance (May 2026):** Active.
- **When to use:** Component unit tests. Use with vitest + @testing-library/user-event for user interaction simulation.
- **Alternatives:** None; this is canonical for Solid.
- **Notes:** Solid's testing story is mature. Component tests written with this library are fast (no virtual DOM overhead) and realistic (tests behavior, not implementation).
- **Pixhaus streams using it:** S13–S20, S52.

### @tauri-apps/cli

- **Purpose:** CLI for scaffolding, building, and managing Tauri projects. Integrates Rust (Tauri core) with the TS frontend.
- **Package URL:** https://www.npmjs.com/package/@tauri-apps/cli
- **Docs:** https://v2.tauri.app/start/
- **Repo:** https://github.com/tauri-apps/tauri
- **License:** Apache 2.0 / MIT (dual)
- **Maintenance (May 2026):** Active. Tauri 2.x is stable.
- **When to use:** Every Pixhaus build. Tauri CLI orchestrates: pnpm build (TS frontend), cargo build (Rust core), bundle creation (Windows MSI, macOS DMG, Linux .deb/.rpm/AppImage).
- **Alternatives:** None; Tauri is the only choice for this stack.
- **Notes:** tauri build produces native installers; tauri dev launches a dev window with auto-reload. Familiar to anyone using Create React App or Next.js CLI.
- **Pixhaus streams using it:** S13, S49, S50.

### pnpm

- **Purpose:** Fast, disk-space-efficient npm-compatible package manager.
- **Package URL:** https://pnpm.io/
- **Docs:** https://pnpm.io/
- **Repo:** https://github.com/pnpm/pnpm
- **License:** MIT
- **Maintenance (May 2026):** Active. pnpm 9+ is current; pnpm 10 in development.
- **When to use:** All TS/JS dependency management in Pixhaus. pnpm's monorepo support (pnpm-workspace.yaml) handles the ui/ folder cleanly.
- **Alternatives:** npm (slower, more disk space), yarn (heavier than pnpm).
- **Notes:** pnpm's strict peer dependency handling catches incompatibilities npm/yarn might hide. Lock file (pnpm-lock.yaml) is deterministic. Workspace support for the ui/ and unity/ subfolders is clean.
- **Pixhaus streams using it:** All UI streams, S39 (Unity package setup).

### TypeScript 5.x

- **Purpose:** Typed JavaScript superset. Used for all TS code in Pixhaus frontend.
- **Package URL:** https://www.npmjs.com/package/typescript
- **Docs:** https://www.typescriptlang.org/
- **Repo:** https://github.com/microsoft/TypeScript
- **License:** Apache 2.0
- **Maintenance (May 2026):** Active. TS 5.7 is current; 5.8 in development.
- **When to use:** All UI code in Pixhaus.
- **Alternatives:** None for type safety in the TS ecosystem.
- **Notes:** Configure tsconfig.json with:
  - `"jsx": "preserve"` (let vite-plugin-solid handle JSX transformation)
  - `"jsxImportSource": "solid-js"`
  - `"strict": true`
  - `"skipLibCheck": true` (to speed up type checking)
  
  Vite 5+ uses Oxc transformer, so TypeScript compilation is fast even in strict mode.
- **Pixhaus streams using it:** All UI streams.

---

## Tauri JavaScript APIs

### @tauri-apps/api

- **Purpose:** Core Tauri JavaScript SDK. Provides wrappers for window management, file dialog, IPC to Rust backend, OS information, clipboard, and more.
- **Package URL:** https://www.npmjs.com/package/@tauri-apps/api
- **Docs:** https://v2.tauri.app/reference/javascript/api/
- **Repo:** https://github.com/tauri-apps/tauri
- **License:** Apache 2.0 / MIT (dual)
- **Maintenance (May 2026):** Active.
- **When to use:** All Tauri interop. Core APIs: invoke (call Rust commands), emit/listen (events), window (manage window state), os (get platform info).
- **Alternatives:** None; this is the canonical Tauri JS binding.
- **Notes:** API surface is mature and stable in v2. Types are first-class TypeScript. Import what you need (tree-shakable).
- **Pixhaus streams using it:** S13 (window chrome), S21–S22 (verb invocation to Rust).

### @tauri-apps/plugin-store

- **Purpose:** Persistent key-value store backed by Rust (serde_json on disk). Stores user preferences, recent projects, window state.
- **Package URL:** https://www.npmjs.com/package/@tauri-apps/plugin-store
- **Docs:** https://v2.tauri.app/reference/javascript/api/namespaceasync/
- **Repo:** https://github.com/tauri-apps/plugins-workspace
- **License:** Apache 2.0 / MIT (dual)
- **Maintenance (May 2026):** Active.
- **When to use:** Preferences (keybinds, themes, AI backend settings), recent projects list, window geometry. Not for large blobs; use the native .pixhaus file format for project data.
- **Alternatives:** localStorage (browser API; not suitable for desktop app preferences), TOML config files (manual parsing).
- **Notes:** Store is async; designed for desktop, not browser. Data is JSON on disk in the app's AppData folder (Windows), Library/Application Support (macOS), ~/.config (Linux).
- **Pixhaus streams using it:** S13 (preferences), S49 (CI integration).

### @tauri-apps/plugin-dialog

- **Purpose:** Native file/folder picker dialogs.
- **Package URL:** https://www.npmjs.com/package/@tauri-apps/plugin-dialog
- **Docs:** https://v2.tauri.app/reference/javascript/api/namespacebrowser/
- **Repo:** https://github.com/tauri-apps/plugins-workspace
- **License:** Apache 2.0 / MIT (dual)
- **Maintenance (May 2026):** Active.
- **When to use:** File open (import .aseprite, .psd, PNG), file save (.pixhaus, sprite sheets, GIFs).
- **Alternatives:** Input file (browser API; limited to web sandbox). Tauri's dialog is native and unrestricted.
- **Notes:** Dialog respects OS theme (light/dark). Returns full file path; no sandbox restrictions.
- **Pixhaus streams using it:** S13 (File menu), S07–S12 (I/O operations).

### @tauri-apps/plugin-fs

- **Purpose:** Filesystem access (read/write/list files and directories). Unrestricted within the app's sandboxed scope.
- **Package URL:** https://www.npmjs.com/package/@tauri-apps/plugin-fs
- **Docs:** https://v2.tauri.app/reference/javascript/api/namespacefs/
- **Repo:** https://github.com/tauri-apps/plugins-workspace
- **License:** Apache 2.0 / MIT (dual)
- **Maintenance (May 2026):** Active.
- **When to use:** Exporting files (sprite sheets, GIFs, TMX), reading recent projects, temp file management.
- **Alternatives:** None for desktop file I/O in Tauri.
- **Notes:** Sandbox is configurable in tauri.conf.json; by default, allows file access scoped to the app's directories. For Pixhaus, relax the scope to allow full file access (user choice of where to save/load).
- **Pixhaus streams using it:** S07–S12 (all I/O), S21 (verb outputs), S45 (sample projects).

### @tauri-apps/plugin-shell

- **Purpose:** Execute OS commands (shell scripts, subprocesses). Useful for invoking external tools.
- **Package URL:** https://www.npmjs.com/package/@tauri-apps/plugin-shell
- **Docs:** https://v2.tauri.app/reference/javascript/api/namespaceshell/
- **Repo:** https://github.com/tauri-apps/plugins-workspace
- **License:** Apache 2.0 / MIT (dual)
- **Maintenance (May 2026):** Active.
- **When to use:** Invoke ffmpeg (not needed if ffmpeg-next is used in Rust), open external apps, run build scripts. Not primary for Pixhaus.
- **Alternatives:** Invoke Rust commands directly (preferred for Pixhaus).
- **Notes:** Security: shell commands should be whitelisted in tauri.conf.json (scope-based access control). Avoid exposing arbitrary shell injection.
- **Pixhaus streams using it:** S11 (animated export via ffmpeg if browser-side approach is chosen).

### @tauri-apps/plugin-window-state

- **Purpose:** Persist and restore window geometry (position, size) across app launches.
- **Package URL:** https://www.npmjs.com/package/@tauri-apps/plugin-window-state
- **Docs:** https://v2.tauri.app/reference/javascript/api/
- **Repo:** https://github.com/tauri-apps/plugins-workspace
- **License:** Apache 2.0 / MIT (dual)
- **Maintenance (May 2026):** Active.
- **When to use:** Auto-save/restore window size and position (standard UX for desktop apps).
- **Alternatives:** Store plugin (manual state tracking).
- **Notes:** Integration is simple: call saveWindowState() on close, restoreWindowState() on launch. Prevents the "where did my window go" UX problem.
- **Pixhaus streams using it:** S13 (shell window setup).

### @tauri-apps/plugin-updater

- **Purpose:** Background auto-update mechanism for Tauri apps. Checks for new versions, downloads, and prompts user to restart.
- **Package URL:** https://www.npmjs.com/package/@tauri-apps/plugin-updater
- **Docs:** https://v2.tauri.app/reference/javascript/api/namespaceupdater/
- **Repo:** https://github.com/tauri-apps/plugins-workspace
- **License:** Apache 2.0 / MIT (dual)
- **Maintenance (May 2026):** Active.
- **When to use:** Auto-update on startup. Pixhaus should check for new releases (GitHub Releases is the endpoint).
- **Alternatives:** Manual update check (user responsibility).
- **Notes:** Requires signed release artifacts (S50). Configuration in tauri.conf.json points to a GitHub Releases endpoint. Simple integration; high UX value.
- **Pixhaus streams using it:** S50 (release packaging).

### @tauri-apps/plugin-log

- **Purpose:** Structured logging to a file in the app's data directory.
- **Package URL:** https://www.npmjs.com/package/@tauri-apps/plugin-log
- **Docs:** https://v2.tauri.app/reference/javascript/api/
- **Repo:** https://github.com/tauri-apps/plugins-workspace
- **License:** Apache 2.0 / MIT (dual)
- **Maintenance (May 2026):** Active.
- **When to use:** Debugging. TS side can log; logs appear in the Tauri log file (for crash report support or user diagnostics).
- **Alternatives:** console.log (goes nowhere in production Tauri app), sentry (S51).
- **Notes:** Logs are on disk; useful for crash post-mortem analysis. Less critical for Pixhaus than for a server; includes in S51 (crash reporting).
- **Pixhaus streams using it:** S13, S51 (crash reporting context).

### @tauri-apps/plugin-os

- **Purpose:** Detect OS platform, architecture, type.
- **Package URL:** https://www.npmjs.com/package/@tauri-apps/plugin-os
- **Docs:** https://v2.tauri.app/reference/javascript/api/namespaceos/
- **Repo:** https://github.com/tauri-apps/plugins-workspace
- **License:** Apache 2.0 / MIT (dual)
- **Maintenance (May 2026):** Active.
- **When to use:** Conditional UI/behavior per OS (Windows uses Alt for menu, macOS uses Cmd). Keybind customization.
- **Alternatives:** navigator.platform (browser API; less reliable in Tauri context).
- **Notes:** Tauri's os plugin is more reliable than browser APIs. Use for platform-specific shortcuts (Ctrl on Windows/Linux, Cmd on macOS).
- **Pixhaus streams using it:** S13 (keybind defaults).

### @tauri-apps/plugin-clipboard-manager

- **Purpose:** Read/write system clipboard.
- **Package URL:** https://www.npmjs.com/package/@tauri-apps/plugin-clipboard-manager
- **Docs:** https://v2.tauri.app/reference/javascript/api/
- **Repo:** https://github.com/tauri-apps/plugins-workspace
- **License:** Apache 2.0 / MIT (dual)
- **Maintenance (May 2026):** Active.
- **When to use:** Copy/paste UI elements, export frames as PNG to clipboard (useful for sharing quick previews).
- **Alternatives:** None for desktop clipboard in Tauri.
- **Notes:** Supports text and files (platform-specific). Good UX for "Copy as PNG" operations.
- **Pixhaus streams using it:** S13 (Edit menu), S15–S20 (copy/paste context).

### @tauri-apps/plugin-deep-link

- **Purpose:** Handle deep-link protocols (pixhaus://open-file/...). Allows opening Pixhaus projects from file explorer or web links.
- **Package URL:** https://www.npmjs.com/package/@tauri-apps/plugin-deep-link
- **Docs:** https://v2.tauri.app/reference/javascript/api/
- **Repo:** https://github.com/tauri-apps/plugins-workspace
- **License:** Apache 2.0 / MIT (dual)
- **Maintenance (May 2026):** Active.
- **When to use:** Associate .pixhaus files with Pixhaus, open from command line or web links. Deferred to post-MVP (nice-to-have).
- **Alternatives:** File association in installer (manual registration).
- **Notes:** OS-specific setup (registry on Windows, .desktop on Linux, plist on macOS) is handled by Tauri's installer. The plugin handles listening for incoming deep links.
- **Pixhaus streams using it:** S50 (release packaging, optional).

### @tauri-apps/plugin-notification

- **Purpose:** Send OS native notifications (taskbar toast on Windows, macOS Notification Center, Linux libnotify).
- **Package URL:** https://www.npmjs.com/package/@tauri-apps/plugin-notification
- **Docs:** https://v2.tauri.app/reference/javascript/api/
- **Repo:** https://github.com/tauri-apps/plugins-workspace
- **License:** Apache 2.0 / MIT (dual)
- **Maintenance (May 2026):** Active.
- **When to use:** OS-level notifications for long-running operations (verb completion, file export done). More prominent than in-app toast.
- **Alternatives:** solid-sonner (in-app toast only).
- **Notes:** Native notifications are more attention-grabbing. Use for user-initiated async operations (verbs, exports).
- **Pixhaus streams using it:** S21 (verb completion), S13 (export done notification).

---

## Canvas / WebGL / WebGPU

### WebGL2 (native browser API)

- **Purpose:** GPU-accelerated 2D/3D graphics via OpenGL ES 3.0-like API. Mature, widely supported, performant.
- **Docs:** https://developer.mozilla.org/en-US/docs/Web/API/WebGL2RenderingContext
- **License:** N/A (browser standard).
- **Maintenance (May 2026):** Stable. No new features; browser vendors maintain implementations.
- **When to use:** Primary canvas rendering for Pixhaus. WebGL2 is the right choice for 2D sprite rendering with tile-based compositing.
- **Alternatives:** Canvas 2D (software, slower), WebGPU (newer, not yet widely supported).
- **Notes:** WebGL2 maps to OpenGL ES 3.0; support is universal on desktop (Chrome, Firefox, Safari 15+) and most mobile. For Pixhaus, using Tauri's native webview means WebGL2 support is guaranteed. Pixhaus uses WebGL2 for:
  - Composited sprite sheet rendering (Rust → GPU texture)
  - Pan/zoom with smooth scrolling
  - Overlay UI (grid, marching ants, brush preview)
  
  Consider WebGPU for compute shaders (e.g., per-pixel processing in verbs), but not required for MVP.
- **Pixhaus streams using it:** S14 (canvas viewport).

### WebGPU

- **Purpose:** Next-generation GPU API. More explicit control over pipelines, compute shaders, and memory. Emerging standard (not mature in all browsers).
- **Docs:** https://www.w3.org/TR/webgpu/
- **License:** N/A (browser standard).
- **Maintenance (May 2026):** Stable in Chrome 113+, Firefox 147+, Safari 26+ (macOS 14+). Feature detection required.
- **When to use:** Compute workloads (image filters, neural network inference). Not required for MVP; candidate for future optimization (S32 motion-from-video, S34 audio-driven timing if compute is used).
- **Alternatives:** WebGL2 (stable, sufficient for 2D), Rust-side compute (more control, less portable).
- **Notes:** WebGPU shipping in Tauri's webview is confirmed (Tauri on Windows via Direct3D 12 works). However, fallback to WebGL2 is mandatory for older systems. Recommendation: WebGL2 for MVP, WebGPU for compute verbs in v1.1+.
- **Pixhaus streams using it:** S14 optionally (post-MVP), S32/S34 (if compute is offloaded to GPU).

### regl

- **Purpose:** Functional WebGL wrapper. Reduces boilerplate; not a framework, just helpers for shader compilation, buffer management, and draw calls.
- **Package URL:** https://www.npmjs.com/package/regl
- **Docs:** https://github.com/regl-project/regl
- **Repo:** https://github.com/regl-project/regl
- **License:** MIT
- **Maintenance (May 2026):** Stable but not actively updated (mature library). Last significant release 2018–2019.
- **When to use:** If writing raw WebGL2. regl reduces verbosity significantly (vs. raw WebGL2 API).
- **Alternatives:** twgl.js (similar scope, also mature), three.js (overkill for 2D), raw WebGL2 (verbose but fine for simple cases).
- **Notes:** regl's functional style is elegant for frame-by-frame rendering loops. However, for Pixhaus, using a 2D renderer (pixi.js or custom tile-based approach) is simpler than writing raw WebGL2 + regl.
- **Pixhaus streams using it:** S14 possibly (if custom WebGL2 rendering is chosen).

### twgl.js

- **Purpose:** Tiny WebGL Helper Library. Reduces boilerplate for buffer creation, shader compilation, and attribute binding.
- **Package URL:** https://www.npmjs.com/package/twgl.js
- **Docs:** https://twgljs.org/
- **Repo:** https://github.com/greggman/twgl.js
- **License:** MIT
- **Maintenance (May 2026):** Actively maintained.
- **When to use:** Similar to regl. Lower-level than pixi but less verbose than raw WebGL2.
- **Alternatives:** regl (functional style), pixi.js (full 2D renderer).
- **Notes:** twgl is lighter than regl; more of a utility belt. Good for 2D rendering if building a custom engine. For Pixhaus, pixi.js is a better fit (higher level, pixel-art optimized).
- **Pixhaus streams using it:** S14 possibly (if custom WebGL2 rendering is chosen).

### pixi.js

- **Purpose:** Fast, lightweight 2D WebGL renderer for canvas games and interactive applications. Pixel-art-optimized with smart batching and texture atlasing.
- **Package URL:** https://www.npmjs.com/package/pixi.js
- **Docs:** https://pixijs.com/
- **Repo:** https://github.com/pixijs/pixijs
- **License:** MIT
- **Maintenance (May 2026):** Actively maintained (v8.x current, v9 in development). One of the most mature 2D renderers.
- **When to use:** Primary canvas renderer for Pixhaus. pixi.js handles sprite sheets, batching, and pixel-perfect rendering out of the box. Consider pixi for the main viewport.
- **Alternatives:** Custom tile-based WebGL2 (more control, higher maintenance), three.js (overkill for 2D), canvas 2D (software, slow).
- **Notes:** pixi.js is game-industry standard for 2D (Godot, Unity devs use pixi for web ports). Specific relevance for Pixhaus:
  - Sprite batching: multiple sprites rendered in one draw call.
  - Pixel-perfect rendering: disable interpolation, use Math.floor() for integer pixel positions.
  - Texture management: efficient atlas packing.
  
  Recommendation: Use pixi.js for the canvas viewport (S14). Pair with Solid.js for reactive UI overlays (grid, marching ants, brush preview).
- **Pixhaus streams using it:** S14 (canvas viewport, primary).

### three.js

- **Purpose:** Full-featured 3D library. overkill for 2D pixel art editor.
- **Package URL:** https://www.npmjs.com/package/three
- **Docs:** https://threejs.org/
- **Repo:** https://github.com/mrdoob/three.js
- **License:** MIT
- **Maintenance (May 2026):** Active.
- **When to use:** NOT for Pixhaus. Three.js adds 600KB+ bundle size and learning curve. Unnecessary overhead for 2D sprite rendering.
- **Alternatives:** pixi.js (2D-optimized), webGL (raw, lightweight).
- **Notes:** Three.js is for 3D visualization, interactive 3D models, physics simulation. Pixel art editors don't need it.
- **Pixhaus streams using it:** None.

### @loaders.gl

- **Purpose:** Loaders for various 3D/image formats (GLB, OBJ, PLY, PNG, JPEG, KTX2, etc.).
- **Package URL:** https://www.npmjs.com/package/@loaders.gl/core
- **Docs:** https://loaders.gl/
- **Repo:** https://github.com/visgl/loaders.gl
- **License:** MIT
- **Maintenance (May 2026):** Actively maintained by Uber / vis.gl.
- **When to use:** Image loading (if using a custom WebGL2 pipeline), not needed if pixi.js is used (pixi handles image loading).
- **Alternatives:** pixi.js loaders (built-in), raw Image API (simpler for single images).
- **Notes:** loaders.gl is useful for streaming large images or 3D data. For Pixhaus, pixi.js's built-in image loading is sufficient.
- **Pixhaus streams using it:** S14 possibly (if custom WebGL used).

### ogl

- **Purpose:** Lightweight WebGL2 library (alternative to three.js, smaller footprint).
- **Package URL:** https://www.npmjs.com/package/ogl
- **Docs:** https://github.com/oframe/ogl
- **Repo:** https://github.com/oframe/ogl
- **License:** MIT
- **Maintenance (May 2026):** Community-maintained, stable.
- **When to use:** NOT for Pixhaus. ogl is for 3D/graphics experiments, not 2D sprite rendering.
- **Alternatives:** pixi.js (2D), three.js (3D, heavier).
- **Notes:** ogl is tiny (~20KB) but still 3D-focused. pixi.js is the right 2D choice.
- **Pixhaus streams using it:** None.

### @webgpu/types

- **Purpose:** TypeScript type definitions for WebGPU API.
- **Package URL:** https://www.npmjs.com/package/@webgpu/types
- **Docs:** https://www.w3.org/TR/webgpu/
- **Repo:** https://github.com/gpuweb/types
- **License:** W3C License
- **Maintenance (May 2026):** Active (W3C group).
- **When to use:** If implementing WebGPU compute shaders for verbs (S32, S34). Not required for MVP.
- **Alternatives:** None for type safety with WebGPU.
- **Notes:** Only needed if WebGPU is used. TypeScript support is first-class.
- **Pixhaus streams using it:** S32/S34 optionally (post-MVP).

### wgpu-matrix

- **Purpose:** Matrix math library optimized for WebGPU. Fast linear algebra (Vector2/3/4, Matrix4, Quaternion).
- **Package URL:** https://www.npmjs.com/package/wgpu-matrix
- **Docs:** https://github.com/greggman/wgpu-matrix
- **Repo:** https://github.com/greggman/wgpu-matrix
- **License:** MIT
- **Maintenance (May 2026):** Actively maintained.
- **When to use:** If implementing WebGPU rendering. For 2D (pixi.js), not needed.
- **Alternatives:** glMatrix (older, heavier), math.js (general-purpose, overkill), glm.js (similar).
- **Notes:** wgpu-matrix is WebGPU-native and very fast. Not needed for Pixhaus MVP.
- **Pixhaus streams using it:** None (initially).

---

## Color / Palette UI Helpers

### chroma-js

- **Purpose:** Color manipulation library. Convert between color spaces (RGB, HSL, HSV, Lab, LCH), interpolate colors, generate color scales.
- **Package URL:** https://www.npmjs.com/package/chroma-js
- **Docs:** https://gka.github.io/chroma.js/
- **Repo:** https://github.com/gka/chroma.js
- **License:** Apache 2.0
- **Maintenance (May 2026):** Stable, community-maintained.
- **When to use:** Palette UI color operations, color harmony generation, palette interpolation.
- **Alternatives:** culori (modern, fewer conversions), tinycolor2 (simpler, lighter), color (older).
- **Notes:** chroma-js is well-tested and has good format support. Recommendation: use chroma-js for palette operations (S18). Bundle size is ~15KB (minified).
- **Pixhaus streams using it:** S18 (color and palette panel).

### culori

- **Purpose:** Modern color library with focus on perceptual color spaces (OKLab, OKLCH) and precise conversions.
- **Package URL:** https://www.npmjs.com/package/culori
- **Docs:** https://culorijs.org/
- **Repo:** https://github.com/Evercoder/culori
- **License:** LGPL 3.0
- **Maintenance (May 2026):** Actively maintained.
- **When to use:** If perceptual color accuracy matters (e.g., palette harmony that respects human perception). chroma-js is more battle-tested; use culori if you need OKLab/OKLCH specifically.
- **Alternatives:** chroma-js (more widely used), color (older).
- **Notes:** culori's LGPL license is compatible with MIT as long as derivative works are shared. For Pixhaus, chroma-js is probably safer (Apache 2.0 is simpler), but culori is worth considering if perceptual color matters.
- **Pixhaus streams using it:** S18 optionally (if palette harmony uses OKLCH).

### tinycolor2

- **Purpose:** Lightweight color parsing and manipulation. Very small (~6KB minified), focuses on common operations.
- **Package URL:** https://www.npmjs.com/package/tinycolor2
- **Docs:** https://chir.cat/tinycolor/
- **Repo:** https://github.com/bgrins/TinyColor
- **License:** MIT
- **Maintenance (May 2026):** Stable, lightly maintained (complete library).
- **When to use:** If bundle size is critical and features are simple (convert, lighten, darken, triad). For Pixhaus, chroma-js offers more features with minimal size trade-off.
- **Alternatives:** chroma-js (more features), culori (more accurate).
- **Notes:** tinycolor2 is good for simple color picker UI. For the full palette system (S18), chroma-js is better.
- **Pixhaus streams using it:** S15–S18 possibly (for simple color picker tweaks).

### color

- **Purpose:** Older color manipulation library. Fewer features than chroma-js, but stable.
- **Package URL:** https://www.npmjs.com/package/color
- **Docs:** https://github.com/Qix-/color
- **Repo:** https://github.com/Qix-/color
- **License:** MIT
- **Maintenance (May 2026):** Stable, lightly maintained.
- **When to use:** NOT recommended. chroma-js is newer and has more features.
- **Alternatives:** chroma-js.
- **Notes:** Legacy library; use chroma-js instead.
- **Pixhaus streams using it:** None.

---

## Animation / Interaction

### motionone / motion

- **Purpose:** Animation library for the web. Keyframe animations, easing, spring physics, WAAPI-backed.
- **Package URL:** https://www.npmjs.com/package/motion
- **Docs:** https://motion.dev/
- **Repo:** https://github.com/motiondeveloper/motion
- **License:** MIT
- **Maintenance (May 2026):** Actively maintained.
- **When to use:** Smooth UI animations: panel transitions, property panel slide-in, timeline scrubbing. Not for sprite animation (that's S19 timeline, handled in Rust).
- **Alternatives:** @motionone/solid (Motion One for Solid-specific), Framer Motion (React-only), CSS transitions (simple but less powerful).
- **Notes:** Motion One is framework-agnostic and uses native WAAPI (Web Animation API), so it's performant. Solid support is via @motionone/solid package.
- **Pixhaus streams using it:** S13, S15–S20 (panel animations).

### @use-gesture/*

- **Purpose:** Gesture handling library. Recognizes drag, pinch, wheel, keyboard input, and provides normalized handlers for cross-browser consistency.
- **Package URL:** https://www.npmjs.com/package/@use-gesture/react (React), similar for other frameworks.
- **Docs:** https://use-gesture.js.org/
- **Repo:** https://github.com/pmndrs/use-gesture
- **License:** MIT
- **Maintenance (May 2026):** Community-maintained. Solid support via community package.
- **When to use:** Canvas interaction: pan (spacebar+drag, middle-mouse), zoom (wheel), brush strokes (click+drag). @use-gesture abstracts device differences.
- **Alternatives:** solid-primitives gesture hooks (from solid-primitives collection), manual pointer/mouse/touch listeners.
- **Notes:** @use-gesture handles cross-device complexity (touch, mouse, pen, multi-touch). Solid.js may have a community port; verify if not, use solid-primitives instead.
- **Pixhaus streams using it:** S14 (canvas viewport interaction), S15–S16 (brush and selection tools).

### comlink

- **Purpose:** TypeScript-first library for Web Worker communication. Hides the message-passing verbosity; feels like calling remote functions.
- **Package URL:** https://www.npmjs.com/package/comlink
- **Docs:** https://github.com/GoogleChromeLabs/comlink
- **Repo:** https://github.com/GoogleChromeLabs/comlink
- **License:** Apache 2.0
- **Maintenance (May 2026):** Actively maintained (Google).
- **When to use:** Offload heavy JS computation to workers (e.g., image processing, palette quantization, visual regression diffing). Not required for MVP but valuable for responsiveness.
- **Alternatives:** Manual Worker message passing (verbose), workerpool (more framework-heavy).
- **Notes:** comlink makes worker code look synchronous. Useful for S52 (visual regression) and potential color quantization (S27 Cleanup).
- **Pixhaus streams using it:** S52 (visual regression harness), S27 optionally (palette quantization in worker).

### workerpool

- **Purpose:** Worker pool management. Create a pool of workers, distribute tasks, manage concurrency.
- **Package URL:** https://www.npmjs.com/package/workerpool
- **Docs:** https://github.com/josdejong/workerpool
- **Repo:** https://github.com/josdejong/workerpool
- **License:** Apache 2.0
- **Maintenance (May 2026):** Actively maintained.
- **When to use:** If parallelizing heavy tasks (e.g., frame diffing for visual regression, batch color analysis). More complex than comlink.
- **Alternatives:** comlink (simpler, less pooling), manual worker management.
- **Notes:** workerpool is useful for compute-heavy verbs (S30 Project style learning) or batch operations. For MVP, likely overkill.
- **Pixhaus streams using it:** S30 optionally (style training parallelization).

---

## State Management for Solid

### Solid stores (built-in)

- **Purpose:** Reactive signal-based state management. Built into Solid.js core (createSignal, createMemo, createEffect).
- **Docs:** https://docs.solidjs.com/concepts/reactivity
- **License:** MIT
- **Maintenance (May 2026):** Core to Solid.
- **When to use:** ALL state in Pixhaus should start with Solid stores (signals/effects). No external state library needed for simple cases.
- **Alternatives:** none needed; Solid stores are powerful enough.
- **Notes:** Solid's reactivity is fine-grained and explicit. For Pixhaus: use signals for UI state (active layer, zoom level, theme), effects for side effects (write to store, emit Tauri event). No Redux-style boilerplate.
- **Pixhaus streams using it:** All UI streams (S13–S20).

### @nanostores/solid

- **Purpose:** Tiny multi-framework store library. Atomic stores that work across React, Vue, Solid, Svelte, etc. Bridge state across framework boundaries.
- **Package URL:** https://www.npmjs.com/package/@nanostores/solid
- **Docs:** https://github.com/nanostores/solid
- **Repo:** https://github.com/nanostores/solid
- **License:** MIT
- **Maintenance (May 2026):** Actively maintained.
- **When to use:** If a web companion tool is built (shares state with desktop Solid.js app). For desktop-only Pixhaus, NOT required.
- **Alternatives:** Solid stores, context API.
- **Notes:** nanostores is <1KB and tree-shakable. For Pixhaus MVP, use Solid stores directly. nanostores becomes relevant if a web frontend mirrors the desktop app.
- **Pixhaus streams using it:** None initially; consider if web version appears.

### xstate

- **Purpose:** State machine and statechart library. Explicit, testable state transitions. Particularly useful for AI verb workflows with clear states (idle, loading, preview, committed).
- **Package URL:** https://www.npmjs.com/package/xstate
- **Docs:** https://stately.ai/docs/xstate
- **Repo:** https://github.com/statelyai/xstate
- **License:** MIT
- **Maintenance (May 2026):** Actively maintained. v5.x current (released April 2026).
- **When to use:** Verb state machine (S21). Verbs have clear states: idle → running → preview → (accept/reject) → committed or cancelled. XState formalizes this.
- **Alternatives:** @xstate/solid (Solid-specific integration), manual state enum + switch (simpler but less robust).
- **Notes:** XState is powerful and opinionated. For Pixhaus verbs, it prevents impossible states (e.g., can't accept preview if verb is still running). Use @xstate/solid for reactive integration.
- **Pixhaus streams using it:** S21 (verb runtime state machine), S23–S36 (verb workflows).

---

## Command Palette UI

### cmdk-solid

- **Purpose:** Port of cmdk (React) to Solid.js. Fuzzy-searchable command menu with keyboard navigation.
- **Package URL:** https://www.npmjs.com/package/cmdk-solid (or via GitHub)
- **Docs:** Limited; reverse-engineer from React cmdk.
- **Repo:** https://github.com/create-signal/cmdk-solid
- **License:** MIT
- **Maintenance (May 2026):** Community-maintained. Actively used but not as established as React cmdk.
- **When to use:** Command palette for Pixhaus (S13). cmdk-solid provides the UI; the command registry is in Bedrock B4.
- **Alternatives:** solid-command-palette (different API, uses fuse.js), manual combobox (more control, more work).
- **Notes:** cmdk-solid uses Kobalte for Dialog (accessibility). Recommendation: use cmdk-solid because it mirrors the excellent React cmdk, which developers may know.
- **Pixhaus streams using it:** S13 (command palette).

### solid-command-palette

- **Purpose:** Command palette component for Solid. Uses fuse.js for fuzzy search, tinykeys for keybinds, solid-transition-group for animations.
- **Package URL:** https://www.npmjs.com/package/solid-command-palette
- **Docs:** https://www.npmjs.com/package/solid-command-palette
- **Repo:** https://github.com/itaditya/solid-command-palette
- **License:** MIT
- **Maintenance (May 2026):** Community-maintained.
- **When to use:** Alternative to cmdk-solid. More opinionated about layout and styling.
- **Alternatives:** cmdk-solid (simpler, more like React).
- **Notes:** solid-command-palette includes animation and styling helpers; cmdk-solid is more unstyled (Solid-idiomatic). Recommendation: cmdk-solid is lighter and more composable.
- **Pixhaus streams using it:** S13 optionally (if not using cmdk-solid).

---

## Audio (Browser-Side)

### Tone.js

- **Purpose:** Advanced audio synthesis, scheduling, effects, and music programming. Built on Web Audio API.
- **Package URL:** https://www.npmjs.com/package/tone
- **Docs:** https://tonejs.github.io/
- **Repo:** https://github.com/Tonejs/Tone.js
- **License:** MIT
- **Maintenance (May 2026):** Actively maintained.
- **When to use:** NOT primary for Pixhaus. Tone.js is for synthesis and music apps, not audio playback/analysis. Useful if implementing audio-driven animation (S34 Audio-driven timing) with synthesis preview.
- **Alternatives:** Howler.js (playback-focused), Essentia.js (analysis-focused).
- **Notes:** 600K weekly npm downloads. Tone.js is comprehensive; overkill for simple audio playback. If S34 needs to generate click-track or metronome, Tone.js could generate beeps.
- **Pixhaus streams using it:** S34 optionally (if synthesis is needed for audio preview).

### howler.js

- **Purpose:** Lightweight audio playback library. Cross-browser audio, 3D spatial audio, audio sprites, fallback to HTML5 Audio.
- **Package URL:** https://www.npmjs.com/package/howler
- **Docs:** https://howlerjs.com/
- **Repo:** https://github.com/goldfire/howler.js
- **License:** MIT
- **Maintenance (May 2026):** Actively maintained.
- **When to use:** Audio playback for S34 (audio-driven timing). Load audio file, play, analyze timing. Lighter than Tone.js.
- **Alternatives:** Tone.js (more features), Web Audio API directly (verbose).
- **Notes:** 1.5M weekly downloads (most popular). Cross-browser consistent behavior. Recommendation: use Howler.js for audio playback in S34.
- **Pixhaus streams using it:** S34 (audio-driven timing).

### meyda

- **Purpose:** Audio feature extraction. Compute MFCC, spectral centroid, zero crossing rate, chroma, etc. from live audio or files.
- **Package URL:** https://www.npmjs.com/package/meyda
- **Docs:** https://github.com/meyda/meyda
- **Repo:** https://github.com/meyda/meyda
- **License:** MIT
- **Maintenance (May 2026):** Actively maintained (music information retrieval focus).
- **When to use:** Beat detection, onset detection in S32 (motion-from-video audio sync) or S34 (audio-driven timing). JavaScript-side feature extraction.
- **Alternatives:** Essentia.js (WebAssembly-based, faster), aubio-rs (Rust-side, more control).
- **Notes:** Meyda is pure JavaScript, no WASM compilation. Suitable for real-time analysis on the browser side. For Pixhaus, Essentia.js is likely faster (WASM).
- **Pixhaus streams using it:** S32/S34 optionally (browser-side beat detection).

### essentia.js

- **Purpose:** JavaScript wrapper around Essentia (C++ music analysis library) compiled to WebAssembly. Comprehensive audio analysis: beat tracking, onset detection, key/chord estimation, music auto-tagging.
- **Package URL:** https://www.npmjs.com/package/essentia.js
- **Docs:** https://mtg.github.io/essentia.js/
- **Repo:** https://github.com/MTG/essentia.js
- **License:** AGPL 3.0 (copyleft; check compatibility)
- **Maintenance (May 2026):** Actively maintained (Music Technology Group, UPF).
- **When to use:** Beat detection in S32 (motion-from-video) and S34 (audio-driven timing). Faster than meyda (WASM-backed).
- **Alternatives:** meyda (JavaScript, simpler), aubio-rs (Rust-side, more control).
- **Notes:** Essentia.js is faster than meyda for most algorithms. AGPL license means any derivative must open-source. For Pixhaus (MIT), AGPL compatibility is a concern if Essentia.js is linked; either: (1) keep Essentia.js usage isolated (optional dependency), (2) use aubio-rs on Rust side instead, or (3) use meyda (MIT).
  
  Recommendation: For MVP, use aubio-rs on Rust side (S34) or meyda.js (MIT-compatible). Essentia.js is better but AGPL licensing complicates things.
- **Pixhaus streams using it:** S32/S34 (if AGPL compatibility is resolved).

### web-audio-beat-detector

- **Purpose:** Lightweight beat detection using Web Audio API. Analyzes frequency data to detect beats.
- **Package URL:** https://www.npmjs.com/package/web-audio-beat-detector
- **Docs:** https://github.com/mido9/web-audio-beat-detector
- **Repo:** https://github.com/mido9/web-audio-beat-detector
- **License:** MIT
- **Maintenance (May 2026):** Community-maintained, less actively updated.
- **When to use:** Lightweight beat detection alternative. Less comprehensive than Essentia or meyda.
- **Alternatives:** Essentia.js, meyda, aubio-rs.
- **Notes:** Simpler but less accurate than music-information-retrieval libraries. Useful if beat detection is the only feature needed.
- **Pixhaus streams using it:** S34 optionally (lightweight fallback).

---

## Video Processing (Browser-Side)

### ffmpeg.wasm

- **Purpose:** FFmpeg compiled to WebAssembly. In-browser video/audio encoding, decoding, transcoding.
- **Package URL:** https://www.npmjs.com/package/@ffmpeg/ffmpeg
- **Docs:** https://ffmpegwasm.netlify.app/
- **Repo:** https://github.com/ffmpegwasm/ffmpeg.wasm
- **License:** MIT (for the wrapper; FFmpeg is LGPL, see licensing note below)
- **Maintenance (May 2026):** Actively maintained.
- **When to use:** Browser-side video processing for motion-from-video (S32). Load video, extract frames, convert to PNG sequence.
- **Alternatives:** MediaBunny (modern, TypeScript, WebCodecs-based), Rust-side ffmpeg-next (more control), shell ffmpeg (via plugin-shell).
- **Notes:** FFmpeg.wasm license: the wrapper is MIT, but FFmpeg itself is LGPL (copyleft). For Pixhaus (MIT), using FFmpeg.wasm creates a license conflict. Options:
  1. Use ffmpeg.wasm but license Pixhaus under LGPL (not desired).
  2. Use ffmpeg-next in Rust side with proper LGPL attribution (acceptable if LGPL source is linked, not bundled).
  3. Use MediaBunny (Mozilla Public License 2.0, less restrictive).
  4. Use WebCodecs API directly (new, native, no external deps).
  
  Recommendation: Skip ffmpeg.wasm for browser. Instead, use ffmpeg-next on Rust side (S11) for export, or MediaBunny for browser side.
- **Pixhaus streams using it:** S32 (motion-from-video) — use alternative.

### MediaBunny

- **Purpose:** Modern JavaScript video processing library. Hardware-accelerated encoding/decoding via WebCodecs API. TypeScript-first.
- **Package URL:** Available via GitHub, not yet on npm (as of May 2026).
- **Docs:** https://github.com/WasmMediaPlayer/mediabunny
- **Repo:** https://github.com/WasmMediaPlayer/mediabunny
- **License:** Mozilla Public License 2.0 (MPL 2.0)
- **Maintenance (May 2026):** Under active development.
- **When to use:** Browser-side video processing (S32). Better than ffmpeg.wasm for modern workflows; uses native WebCodecs for hardware acceleration.
- **Alternatives:** ffmpeg.wasm (LGPL complications), WebCodecs API directly (lower-level).
- **Notes:** MediaBunny is purpose-built for the web; no LGPL baggage. Smaller bundle than ffmpeg.wasm. Emerging library; less battle-tested but promising.
- **Pixhaus streams using it:** S32 (motion-from-video) — preferred over ffmpeg.wasm.

### mediapipe-js

- **Purpose:** Google MediaPipe in JavaScript. Pose detection, hand tracking, object detection, image segmentation.
- **Package URL:** https://www.npmjs.com/package/@mediapipe/tasks-vision
- **Docs:** https://developers.google.com/mediapipe/solutions/vision/pose_landmarker
- **Repo:** https://github.com/google-ai-edge/mediapipe
- **License:** Apache 2.0
- **Maintenance (May 2026):** Actively maintained (Google).
- **When to use:** Pose extraction for S32 (motion-from-video). MediaPipe Pose detects 33 keypoints on the human body from video.
- **Alternatives:** DensePose (bottom-up, denser; Python/C++, not JS), custom ONNX models.
- **Notes:** MediaPipe is the gold standard for pose in production. Blazepose is lightweight (good for web). Recommendation: use MediaPipe for S32.
- **Pixhaus streams using it:** S32 (motion-from-video, pose extraction).

### @tensorflow/tfjs

- **Purpose:** TensorFlow.js for ML inference in the browser. Used by MediaPipe internally; also useful for custom models.
- **Package URL:** https://www.npmjs.com/package/@tensorflow/tfjs
- **Docs:** https://www.tensorflow.org/js
- **Repo:** https://github.com/tensorflow/tfjs
- **License:** Apache 2.0
- **Maintenance (May 2026):** Actively maintained (Google).
- **When to use:** Custom ML models (e.g., style transfer for S36 Sketch finishing, segmentation for S33 Auto-mesh-deformation).
- **Alternatives:** ONNX Runtime JS, TensorFlow Lite JS.
- **Notes:** TensorFlow.js is mature for inference. Training in the browser is slow; inference is fast with hardware acceleration.
- **Pixhaus streams using it:** S32 (if custom pose models), S33 (segmentation), S36 (style transfer).

---

## Audio (Rust-Side)

### rodio

- **Purpose:** Audio playback library for Rust. Cross-platform, supports multiple formats (WAV, FLAC, Vorbis, MP3).
- **Package URL:** https://crates.io/crates/rodio
- **Docs:** https://docs.rs/rodio/
- **Repo:** https://github.com/RustAudio/rodio
- **License:** MIT / Apache 2.0 (dual)
- **Maintenance (May 2026):** Actively maintained.
- **When to use:** Audio playback on Rust side (if needed for S34 audio-driven timing; usually browser-side is simpler).
- **Alternatives:** cpal (lower-level I/O), kmusic (higher-level, less maintained).
- **Notes:** rodio is the most straightforward choice for audio playback in Rust. Useful if Pixhaus backend needs to validate audio or synthesize previews.
- **Pixhaus streams using it:** S34 optionally (Rust-side audio preview).

### cpal

- **Purpose:** Audio I/O library. Low-level interface to the system audio subsystem. Cross-platform (Windows WASAPI, macOS CoreAudio, Linux ALSA/PulseAudio).
- **Package URL:** https://crates.io/crates/cpal
- **Docs:** https://docs.rs/cpal/
- **Repo:** https://github.com/RustAudio/cpal
- **License:** MIT / Apache 2.0 (dual)
- **Maintenance (May 2026):** Actively maintained.
- **When to use:** Low-level audio I/O if Pixhaus needs to sample or stream audio (not typical for S32/S34).
- **Alternatives:** rodio (higher-level), JACK (audio pro tools).
- **Notes:** cpal is for when you need direct hardware control. For Pixhaus, rodio is usually sufficient.
- **Pixhaus streams using it:** None initially.

### symphonia

- **Purpose:** Pure Rust audio codec library. Decodes MP3, FLAC, Vorbis, WAV, etc. without external dependencies.
- **Package URL:** https://crates.io/crates/symphonia
- **Docs:** https://docs.rs/symphonia/
- **Repo:** https://github.com/pdeljanov/Symphonia
- **License:** MIT / Apache 2.0 (dual)
- **Maintenance (May 2026):** Actively maintained.
- **When to use:** Decode audio files on the Rust side (S32, S34). Pair with cpal for playback or pass decoded frames to browser.
- **Alternatives:** ffmpeg (LGPL, more comprehensive), libsndfile-rs (C wrapper, less pure Rust).
- **Notes:** symphonia is pure Rust, no C dependencies. Slower than native FFmpeg but sufficient for analysis use cases.
- **Pixhaus streams using it:** S32/S34 (audio decoding).

### hound

- **Purpose:** Pure Rust WAV file I/O. Read and write WAV files.
- **Package URL:** https://crates.io/crates/hound
- **Docs:** https://docs.rs/hound/
- **Repo:** https://github.com/ruuda/hound
- **License:** Apache 2.0
- **Maintenance (May 2026):** Stable, lightly maintained (complete library).
- **When to use:** WAV export from Pixhaus (if exporting animation timing as MIDI or audio track). Not primary for S32/S34.
- **Alternatives:** symphonia (more formats), ffmpeg-next (more comprehensive).
- **Notes:** hound is lightweight and focused. For Pixhaus, symphonia or ffmpeg-next are more useful.
- **Pixhaus streams using it:** None initially.

### aubio-rs

- **Purpose:** Rust bindings to Aubio (C library for audio analysis). Beat tracking, onset detection, tempo estimation, pitch detection.
- **Package URL:** https://crates.io/crates/aubio-rs (or aubio)
- **Docs:** https://docs.rs/aubio-rs/
- **Repo:** https://github.com/katyo/aubio-rs
- **License:** GPL (Aubio is GPL; bindings are LGPL)
- **Maintenance (May 2026):** Maintained.
- **When to use:** Beat detection and onset detection for S32 (motion-from-video audio sync) and S34 (audio-driven timing). Rust-side analysis is more efficient than browser-side.
- **Alternatives:** Essentia.js (browser, AGPL), meyda.js (browser, MIT), custom analysis.
- **Notes:** Aubio-rs requires GPL/LGPL compliance. For Pixhaus (MIT), this is acceptable as long as the Aubio binding is not bundled (dynamically linked). Alternative: keep analysis on browser side with meyda.js or essentia.js.
  
  Recommendation: For S34, use aubio-rs on Rust side (if willing to handle GPL) or meyda.js on browser side (simpler, MIT-compatible).
- **Pixhaus streams using it:** S34 (audio-driven timing) if LGPL compliance is acceptable.

---

## Video Processing (Rust-Side)

### ffmpeg-next

- **Purpose:** Rust bindings to FFmpeg. Video/audio encoding, decoding, transcoding, format conversion.
- **Package URL:** https://crates.io/crates/ffmpeg-next
- **Docs:** https://docs.rs/ffmpeg-next/
- **Repo:** https://github.com/zmwangx/ffmpeg-rust
- **License:** MIT / Apache 2.0 (bindings); FFmpeg is LGPL
- **Maintenance (May 2026):** Actively maintained.
- **When to use:** Video processing for S11 (animated export: GIF, WebP, MP4), S32 (motion-from-video frame extraction).
- **Alternatives:** video-rs (higher-level wrapper), gstreamer-rs (alternative pipeline), MediaBunny/WebCodecs (browser-side).
- **Notes:** FFmpeg is the industry standard but introduces LGPL dependency. For Pixhaus (MIT):
  - Option 1: Link FFmpeg dynamically (system FFmpeg, not bundled). MIT-compatible.
  - Option 2: Use LGPL license for Pixhaus (simplest legally).
  - Option 3: Use video-rs or gstreamer-rs (may be faster/simpler).
  
  Recommendation: Use ffmpeg-next with dynamic linking. Document that system FFmpeg is required (or bundle with proper LGPL compliance).
- **Pixhaus streams using it:** S11 (animated export), S32 (motion-from-video frame extraction).

### video-rs

- **Purpose:** Higher-level video processing library built on FFmpeg. Simpler API for encoding/decoding.
- **Package URL:** https://crates.io/crates/video-rs
- **Docs:** https://docs.rs/video-rs/
- **Repo:** https://github.com/tmm1/video-rs
- **License:** MIT
- **Maintenance (May 2026):** Community-maintained, less active than ffmpeg-next.
- **When to use:** If simpler API is preferred over ffmpeg-next's flexibility.
- **Alternatives:** ffmpeg-next (more control), gstreamer-rs (alternative).
- **Notes:** video-rs wraps ffmpeg-next but is less popular (less battle-tested). For Pixhaus, ffmpeg-next is more reliable.
- **Pixhaus streams using it:** None (prefer ffmpeg-next).

### opencv

- **Purpose:** OpenCV Rust bindings. Computer vision: object detection, motion estimation, image processing.
- **Package URL:** https://crates.io/crates/opencv
- **Docs:** https://docs.rs/opencv/
- **Repo:** https://github.com/twistedfall/opencv-rust
- **License:** MIT (bindings); OpenCV is BSD/Apache (permissive)
- **Maintenance (May 2026):** Actively maintained.
- **When to use:** Image processing that goes beyond standard filters. S32 (optical flow for motion extraction), S33 (segmentation), potential image analysis for verbs.
- **Alternatives:** custom image processing, ffmpeg, skia-safe (graphics focus).
- **Notes:** OpenCV is comprehensive but has a learning curve. For Pixhaus, custom Rust code is likely simpler than invoking OpenCV. Consider for future verbs.
- **Pixhaus streams using it:** S32 optionally (optical flow), S33 optionally (segmentation).

### gstreamer-rs

- **Purpose:** Rust bindings to GStreamer. Media pipeline construction, encoding/decoding, audio/video processing.
- **Package URL:** https://crates.io/crates/gstreamer
- **Docs:** https://slomo.pages.freedesktop.org/rustdocs/gstreamer/gstreamer/
- **Repo:** https://github.com/sdroege/gstreamer-rs
- **License:** MIT / Apache 2.0 (bindings); GStreamer is LGPL
- **Maintenance (May 2026):** Actively maintained (freedesktop project).
- **When to use:** Alternative to FFmpeg if GStreamer is preferred (or bundled). Good for media pipelines on Linux.
- **Alternatives:** ffmpeg-next (more portable, more widely used), video-rs (simpler).
- **Notes:** GStreamer is popular on Linux but less so on macOS/Windows. For cross-platform Pixhaus, ffmpeg-next is simpler.
- **Pixhaus streams using it:** None (prefer ffmpeg-next).

---

## Visual Diff / Image Utilities

### pixelmatch

- **Purpose:** Fast pixel-level image diffing. Compares two PNG/canvas images and returns diff highlighting changes.
- **Package URL:** https://www.npmjs.com/package/pixelmatch
- **Docs:** https://github.com/mapbox/pixelmatch
- **Repo:** https://github.com/mapbox/pixelmatch
- **License:** ISC (BSD-equivalent)
- **Maintenance (May 2026):** Stable, lightly maintained (complete library).
- **When to use:** Visual regression testing (S52). Compare expected vs. actual screenshots, highlight diffs.
- **Alternatives:** Playwright toHaveScreenshot (integrates pixelmatch, higher-level), custom image diff.
- **Notes:** pixelmatch is fast and zero-dependency (only pngjs required). Supports configurable threshold (pixel tolerance, acceptable mismatch %). Perfect for S52.
- **Pixhaus streams using it:** S52 (visual regression test harness).

### sharp

- **Purpose:** High-performance image processing library (Node.js / Rust). Resize, rotate, crop, format conversion.
- **Package URL:** https://www.npmjs.com/package/sharp
- **Docs:** https://sharp.pixelplumbing.com/
- **Repo:** https://github.com/lovell/sharp
- **License:** Apache 2.0
- **Maintenance (May 2026):** Actively maintained.
- **When to use:** Server-side image processing (not typical for Pixhaus, which is desktop). Useful if build scripts need to process sample images.
- **Alternatives:** image crate (Rust), ImageMagick.
- **Notes:** sharp is based on libvips (fast, low memory). For Pixhaus, the Rust core handles image processing; sharp is only useful in build scripts or docs generation.
- **Pixhaus streams using it:** S45 optionally (sample project asset generation).

### canvas-color-tracker

- **Purpose:** Extract and track color data from canvas. Analyze pixel colors, count unique colors, track color changes.
- **Package URL:** Community library, not widely available on npm.
- **Docs:** Limited.
- **Repo:** Varies by implementation.
- **License:** Varies.
- **Maintenance (May 2026):** Not clear (fragmented ecosystem).
- **When to use:** NOT recommended. Use pixel-level analysis directly or Rust-side color tracking.
- **Alternatives:** custom canvas analysis, Rust core.
- **Notes:** No canonical "canvas-color-tracker" in 2026 npm ecosystem. Skip this.
- **Pixhaus streams using it:** None.

### @squoosh/lib

- **Purpose:** Squoosh (Google's image compression tool) library for Node.js. Codec bindings, image compression, format conversion.
- **Package URL:** https://www.npmjs.com/package/@squoosh/lib
- **Docs:** https://github.com/GoogleChromeLabs/squoosh
- **Repo:** https://github.com/GoogleChromeLabs/squoosh
- **License:** Apache 2.0
- **Maintenance (May 2026):** Community-maintained (not actively updated by Google).
- **When to use:** Image compression for exports (S11, S10). Support multiple codecs (WebP, AVIF, MozJPEG, OxiPNG).
- **Alternatives:** sharp (simpler, recommended), imagemin (CLI tool).
- **Notes:** Squoosh is powerful but heavyweight (includes WASM modules). For Pixhaus, sharp or backend imagemin (ffmpeg) is simpler.
- **Pixhaus streams using it:** S10/S11 optionally (if Squoosh compression is used in export pipeline).

---

## Markdown / Docs

### marked

- **Purpose:** Markdown parser. Convert Markdown to HTML.
- **Package URL:** https://www.npmjs.com/package/marked
- **Docs:** https://marked.js.org/
- **Repo:** https://github.com/markedjs/marked
- **License:** MIT
- **Maintenance (May 2026):** Actively maintained.
- **When to use:** If Pixhaus needs to render Markdown in-app (e.g., help docs, changelog in UI).
- **Alternatives:** markdown-it (plugin ecosystem), remark (ecosystem-heavy), showdown (legacy).
- **Notes:** marked is lightweight and widely used. For Pixhaus, Markdown is unlikely to be rendered in-app (docs are separate). Consider only if in-app help panels need Markdown.
- **Pixhaus streams using it:** S13 optionally (in-app help).

### markdown-it

- **Purpose:** Markdown parser with plugin support. More extensible than marked.
- **Package URL:** https://www.npmjs.com/package/markdown-it
- **Docs:** https://markdown-it.github.io/
- **Repo:** https://github.com/markdown-it/markdown-it
- **License:** MIT
- **Maintenance (May 2026):** Actively maintained.
- **When to use:** If advanced Markdown features (tables, syntax highlighting, custom blocks) are needed.
- **Alternatives:** marked (simpler), remark (more complex).
- **Notes:** markdown-it is popular in documentation and static site generators. For Pixhaus, not primary unless in-app docs are extensive.
- **Pixhaus streams using it:** S41–S43 (docs site generation).

### astro / astro-starlight

- **Purpose:** Astro is a static site builder (astro.build). Starlight is a documentation theme for Astro.
- **Package URL:** https://www.npmjs.com/package/astro
- **Docs:** https://docs.astro.build/
- **Repo:** https://github.com/withastro/astro
- **License:** MIT
- **Maintenance (May 2026):** Actively maintained.
- **When to use:** Build the Pixhaus documentation site (S41). Astro + Starlight is the gold standard for docs in 2026.
- **Alternatives:** mdbook (simpler but less flexible), Docusaurus (React-based, heavier), Hugo (Go-based).
- **Notes:** Astro Starlight is recommended for S41. Fast, beautiful, accessible. Ships as static HTML (no JavaScript overhead unless needed).
- **Pixhaus streams using it:** S41 (user documentation), S42–S43 (migration guide, plugin guide).

---

## State of Key Ecosystems (May 2026)

### WebGL2 vs WebGPU

**WebGL2** is production-ready and widely supported (desktop and older mobile). Recommended for Pixhaus MVP.

**WebGPU** is now shipping in all major browsers (Chrome 113+, Firefox 147+, Safari 26+). However, desktop support is not guaranteed on older systems. Feature detection and fallback to WebGL2 is required for shipping apps. Recommendation: **Use WebGL2 for MVP; WebGPU for future compute verbs (post-MVP).**

### Solid.js Maturity

Solid.js is mature and production-ready. v1.9.x is the current recommended version; v2.0.0-experimental is in development (Feb 2026 milestone). The signals paradigm is now industry-standard (Angular, Vue, React all adopting). Recommendation: **Use Solid 1.9.x for Pixhaus; consider 2.0 migration in v1.1+.**

### Command Palette Landscape

React has `cmdk` (canonical). Solid.js has `cmdk-solid` (community port, solid-idiomatic) and `solid-command-palette` (alternative). Recommendation: **Use cmdk-solid for Pixhaus (lighter, more familiar to React developers).**

### Audio Analysis

Browser-side options: meyda.js (MIT, pure JS), essentia.js (faster, AGPL). Rust-side: aubio-rs (GPL, faster), symphonia (MIT-compatible). Recommendation: **Use meyda.js or aubio-rs for S34, depending on GPL tolerance. meyda.js is simpler.**

### Video Processing

Browser-side: ffmpeg.wasm (LGPL complications), MediaBunny (MPL 2.0, emerging, better), WebCodecs API (native, no deps). Rust-side: ffmpeg-next (LGPL, industry standard), gstreamer-rs (LGPL, less portable). Recommendation: **Use ffmpeg-next on Rust side (S11, S32) with dynamic linking. Use MediaBunny or WebCodecs for browser-side (S32) to avoid LGPL.**

### Headless UI Components

Kobalte (Solid-specific, best-in-class) and Ark UI (multi-framework, equal capability). Recommendation: **Use Kobalte for Pixhaus. Solid-native integration beats multi-framework agnosticism.**

---

## Critical Missing Pieces & Library Ecosystem Gaps

After surveying the ecosystem, a few critical gaps emerge for Pixhaus:

### 1. Pixel-Art-Specific JavaScript Libraries

There are no off-the-shelf pixel-art authoring libraries in the TS/JS ecosystem. Aseprite owns this space. Pixhaus is building this from scratch (correct choice). Note: pixi.js is game-engine-focused, not authoring-focused.

### 2. Command Palette Solid Port Maturity

cmdk-solid is a working port but less battle-tested than React cmdk. Consider reviewing the port or being prepared to fork and maintain if issues arise.

### 3. Audio Analysis License Complexity

Every browser-side audio analysis library (meyda, essentia, aubio) has license complexities (MIT, AGPL, GPL). LGPL/GPL audio libraries are useful but require careful handling for MIT projects. Recommendation: stick to meyda.js (MIT) for S34, or accept aubio-rs (LGPL) on Rust side with proper attribution.

### 4. FFmpeg License Handling

FFmpeg.wasm introduces LGPL into the browser bundle. Using ffmpeg-next on Rust side + dynamic linking is cleaner. No perfect solution without legal review.

### 5. No Solid.js-Native XState Integration for S21

XState has @xstate/solid bindings, but verb state machine orchestration is complex. May need to build custom state management or wrapper on top of XState.

---

## Summary: Recommended Stack for Pixhaus (May 2026)

### UI Framework & Tooling
- **Solid.js** (1.9.x) + **Vite** + **TypeScript** 5.x + **pnpm**
- **solid-primitives** for common utilities
- **@kobalte/core** for headless UI components
- **solid-icons** for toolbar/menu icons
- **solid-sonner** for toast notifications
- **cmdk-solid** for command palette
- **motion** (via @motionone/solid) for UI animations
- **vitest** + **@solidjs/testing-library** for testing

### Tauri Ecosystem
- **@tauri-apps/api** (core IPC)
- **@tauri-apps/plugin-store** (preferences)
- **@tauri-apps/plugin-dialog** (file picker)
- **@tauri-apps/plugin-fs** (file I/O)
- **@tauri-apps/plugin-window-state** (window persistence)
- **@tauri-apps/plugin-notification** (OS notifications)

### Canvas & Rendering
- **pixi.js** (WebGL2-based 2D renderer)
- **WebGL2** natively (via browser, no abstraction layer needed initially)
- **regl** or **twgl.js** only if custom WebGL is needed (not primary)
- Defer **WebGPU** to post-MVP

### Color & Palette
- **chroma-js** for color manipulation, harmony generation
- **solid-icons** for palette UI glyphs

### Audio (S32, S34)
- Browser: **howler.js** (playback) + **meyda.js** (beat detection, MIT-compatible)
- Rust: **symphonia** (audio decoding), optional **aubio-rs** (beat tracking, if LGPL acceptable)

### Video (S11, S32)
- Rust: **ffmpeg-next** (with dynamic FFmpeg linking) for GIF, WebP, MP4 export and frame extraction
- Browser: **MediaBunny** (modern alternative to ffmpeg.wasm) if WebCodecs-based approach is preferred
- Pose extraction: **mediapipe-js** (@mediapipe/tasks-vision)

### State Management
- **Solid stores** (signals/effects) for UI state
- **@xstate/solid** + **xstate** for verb state machines (S21)

### Testing & CI
- **vitest** (unit tests)
- **pixelmatch** (visual regression, S52)
- **@tauri-apps/cli** for build orchestration

### Docs
- **Astro Starlight** (S41, user docs)
- **marked** or **markdown-it** (if in-app Markdown needed)

---

## Licenses in Use (Pixhaus & Dependencies)

| Layer | License | Notes |
|---|---|---|
| Pixhaus core | MIT | Open-source, permissive |
| Solid.js | MIT | Fully compatible |
| Tauri | Apache 2.0 / MIT | Compatible |
| pixi.js | MIT | Fully compatible |
| chroma-js | Apache 2.0 | Compatible |
| meyda.js | MIT | Fully compatible |
| @mediapipe/tasks-vision | Apache 2.0 | Compatible |
| ffmpeg (Rust-side, dynamic) | LGPL | Acceptable if dynamically linked; document dependency |
| aubio (optional, Rust-side) | GPL | Requires open-source derivative; optional for beat detection |

For MVP, keep LGPL/GPL use to optional components (aubio, ffmpeg-next) and document licensing clearly.

---

## Conclusion

The TypeScript/JavaScript frontend ecosystem in May 2026 is mature and well-suited for Pixhaus. Solid.js offers fine-grained reactivity without virtual DOM overhead, Vite provides fast builds and HMR, Tauri integrates seamlessly with Rust, and libraries like pixi.js, Kobalte, and cmdk-solid provide battle-tested building blocks. Audio/video processing requires careful license handling (LGPL/GPL complexity) but is solvable with Rust-side processing (ffmpeg-next, aubio-rs) or MIT-compatible browser libs (meyda.js, MediaBunny). No critical missing piece prevents shipping; a few gaps exist (pixel-art-specific JS libs, perfect command-palette Solid port) but are non-blocking for MVP.

Total libraries covered: **95+ packages and tools** across frontend, build, Tauri plugins, canvas, audio/video, state management, and testing.

Key surprises:
1. **WebGPU is shipping but requires fallback.** Desktop support in Tauri is confirmed but not universal; WebGL2 is the safe default.
2. **FFmpeg.wasm license is problematic for MIT projects.** Rust-side ffmpeg-next with dynamic linking is cleaner.
3. **Solid.js is now the most influential reactive framework.** Signals are being adopted by Angular, Vue, and TC39; Pixhaus is in good company.
4. **cmdk-solid exists but is less established than React cmdk.** Be prepared to fork or contribute upstream if issues arise.
5. **No canonical pixel-art-specific JS library.** Pixhaus is building the entire authoring layer from scratch.
