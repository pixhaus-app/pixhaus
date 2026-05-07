// Keybind defaults for the two built-in presets.
//
// Combo format: modifiers joined with "+" in order Ctrl > Shift > Alt, then key.
// Keys: single uppercase letter, digit, or named key (Space, Escape, Enter,
// Comma, Equal, Minus, Quote, Backspace, Delete, ArrowUp, ArrowDown, ...).
//
// These are display and lookup tables only. The native menu accelerators
// (CmdOrCtrl+N, CmdOrCtrl+S, etc.) are handled at the OS layer by Tauri.
// The keybind manager reads these tables for tool keys and custom overrides.

export type Combo = string;
export type CommandId = string;

// Keybind table: combo → command id
export type KeybindTable = ReadonlyMap<Combo, CommandId>;

export const ASEPRITE_DEFAULTS: KeybindTable = new Map<Combo, CommandId>([
  // File
  ["Ctrl+N", "file:new"],
  ["Ctrl+O", "file:open"],
  ["Ctrl+S", "file:save"],
  ["Ctrl+Shift+S", "file:save-as"],
  ["Ctrl+W", "file:close"],
  // Edit
  ["Ctrl+Z", "edit:undo"],
  ["Ctrl+Shift+Z", "edit:redo"],
  ["Ctrl+X", "edit:cut"],
  ["Ctrl+C", "edit:copy"],
  ["Ctrl+V", "edit:paste"],
  ["Ctrl+A", "edit:select-all"],
  ["Ctrl+D", "edit:deselect"],
  // Select
  ["Ctrl+Shift+I", "select:invert"],
  // View
  ["Ctrl+Equal", "view:zoom-in"],
  ["Ctrl+Minus", "view:zoom-out"],
  ["Ctrl+0", "view:zoom-fit"],
  ["Ctrl+1", "view:zoom-100"],
  ["Ctrl+G", "view:toggle-grid"],
  // Tools — Aseprite-style single-letter switches
  ["B", "tool:pencil"],
  ["P", "tool:pencil"],
  ["E", "tool:eraser"],
  ["G", "tool:fill"],
  ["L", "tool:line"],
  ["U", "tool:rect"],
  ["O", "tool:ellipse"],
  // Window
  ["Ctrl+K", "window:command-palette"],
  ["Ctrl+Comma", "window:preferences"],
  ["Ctrl+Shift+L", "window:toggle-layers"],
  ["Ctrl+Shift+T", "window:toggle-timeline"],
]);

export const PHOTOSHOP_DEFAULTS: KeybindTable = new Map<Combo, CommandId>([
  // File
  ["Ctrl+N", "file:new"],
  ["Ctrl+O", "file:open"],
  ["Ctrl+S", "file:save"],
  ["Ctrl+Shift+S", "file:save-as"],
  ["Ctrl+W", "file:close"],
  // Edit
  ["Ctrl+Z", "edit:undo"],
  ["Ctrl+Shift+Z", "edit:redo"],
  ["Ctrl+X", "edit:cut"],
  ["Ctrl+C", "edit:copy"],
  ["Ctrl+V", "edit:paste"],
  ["Ctrl+A", "edit:select-all"],
  ["Ctrl+D", "edit:deselect"],
  // Select
  ["Ctrl+Shift+I", "select:invert"],
  // View — PS uses Ctrl+'+' and Ctrl+'-'
  ["Ctrl+Equal", "view:zoom-in"],
  ["Ctrl+Minus", "view:zoom-out"],
  ["Ctrl+0", "view:zoom-fit"],
  ["Ctrl+1", "view:zoom-100"],
  ["Ctrl+Quote", "view:toggle-grid"], // PS: Ctrl+' for Show Grid
  // Tools — Photoshop-style single-letter switches
  ["B", "tool:pencil"], // PS: B = Brush; mapped to pencil since we don't have brush yet
  ["P", "tool:pencil"], // alias for Aseprite muscle memory
  ["E", "tool:eraser"],
  ["G", "tool:fill"], // PS: G = Gradient/Paint Bucket; we use as fill
  ["U", "tool:rect"], // PS: U = Shape tool
  ["L", "tool:line"], // not standard PS; convenience
  ["O", "tool:ellipse"], // not standard PS; convenience
  // Window
  ["Ctrl+K", "window:command-palette"],
  ["Ctrl+Comma", "window:preferences"],
  ["Ctrl+Shift+L", "window:toggle-layers"],
  ["Ctrl+Shift+T", "window:toggle-timeline"],
]);

// Returns the first combo mapped to commandId in table, or undefined.
export function defaultCombo(table: KeybindTable, commandId: CommandId): Combo | undefined {
  for (const [combo, id] of table) {
    if (id === commandId) return combo;
  }
  return undefined;
}
