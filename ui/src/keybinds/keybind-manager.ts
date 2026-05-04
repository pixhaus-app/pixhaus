import { keybindPreset, customKeybinds } from "../preferences/preferences-store";
import { ASEPRITE_DEFAULTS, PHOTOSHOP_DEFAULTS } from "./defaults";
import { dispatchCommand } from "../command-palette/command-registry";

// Builds a canonical combo string from a keyboard event.
// Format: [Ctrl+][Shift+][Alt+]Key
// "Ctrl" is used for both Ctrl and Meta (Cmd on macOS).
function comboFromEvent(e: KeyboardEvent): string {
  const parts: string[] = [];
  if (e.ctrlKey || e.metaKey) parts.push("Ctrl");
  if (e.shiftKey) parts.push("Shift");
  if (e.altKey) parts.push("Alt");

  const { key } = e;
  if (key === " ") {
    parts.push("Space");
  } else if (key === ",") {
    parts.push("Comma");
  } else if (key === "=") {
    parts.push("Equal");
  } else if (key === "-") {
    parts.push("Minus");
  } else if (key === "'") {
    parts.push("Quote");
  } else if (key.length === 1) {
    parts.push(key.toUpperCase());
  } else {
    // Named keys: Escape, Enter, Tab, Backspace, Delete, ArrowUp, etc.
    parts.push(key);
  }

  return parts.join("+");
}

/**
 * Returns true when the event target is an editable input element.
 *
 * Treats `<input>`, `<textarea>`, `<select>`, and any element with the
 * `contenteditable` attribute as editable. Any keyboard handler whose
 * shortcuts could conflict with text entry (the global keybind manager,
 * the canvas spacebar-pan handler, etc.) should funnel through this
 * helper so the rules stay consistent.
 */
export function isEditableTarget(e: KeyboardEvent): boolean {
  const target = e.target;
  if (!(target instanceof Element)) return false;
  const tag = target.tagName.toLowerCase();
  if (tag === "input" || tag === "textarea" || tag === "select") return true;
  return (target as HTMLElement).isContentEditable;
}

// Registers a keydown listener and returns a cleanup function.
export function setupKeybindManager(): () => void {
  const handler = (e: KeyboardEvent): void => {
    if (isEditableTarget(e)) return;

    const combo = comboFromEvent(e);

    // Custom overrides take priority over presets
    const custom = customKeybinds();
    const customCmd = custom[combo];
    if (customCmd !== undefined) {
      e.preventDefault();
      dispatchCommand(customCmd);
      return;
    }

    const preset = keybindPreset();
    const table = preset === "photoshop" ? PHOTOSHOP_DEFAULTS : ASEPRITE_DEFAULTS;
    const cmd = table.get(combo);
    if (cmd !== undefined) {
      e.preventDefault();
      dispatchCommand(cmd);
    }
  };

  window.addEventListener("keydown", handler);
  return () => window.removeEventListener("keydown", handler);
}
