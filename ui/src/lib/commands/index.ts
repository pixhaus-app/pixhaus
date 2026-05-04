// Barrel re-export for all IPC command wrappers.
//
// Import from this barrel in UI code so a future refactor (e.g. moving to
// tauri-specta auto-generation) only touches one file.

export * from "./canvas";
export * from "./frames";
export * from "./layers";
export * from "./palette";
export * from "./project";
export * from "./tiles";
export * from "./verbs";
