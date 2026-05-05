// Barrel re-export for all IPC command wrappers.
//
// Import from this barrel in UI code so a future refactor (e.g. moving to
// tauri-specta auto-generation) only touches one file.
//
// Every command rejects with `AppCommandError` (`{ kind, message? }`); the TS
// type is generated from the Rust enum by ts-rs. Re-exported here so callers
// can `import type { AppCommandError } from "../lib/commands"` to type-narrow
// on rejected promises (the Tauri `invoke` runtime throws the error object,
// not a string, so a `try { await ... } catch (e) { ... }` block sees the
// typed shape).

export type { AppCommandError } from "../types/AppCommandError";

export * from "./canvas";
export * from "./frames";
export * from "./layers";
export * from "./palette";
export * from "./project";
export * from "./tiles";
export * from "./tilesets";
export * from "./undo";
export * from "./verbs";
