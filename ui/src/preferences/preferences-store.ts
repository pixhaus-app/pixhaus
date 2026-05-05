import { createSignal } from "solid-js";
import { loadStorageJSON } from "../lib/utils/storage";

export type Theme = "dark" | "light" | "pixhaus";
export type KeybindPreset = "aseprite" | "photoshop" | "custom";

const THEME_KEY = "pixhaus:theme";
const PRESET_KEY = "pixhaus:keybind-preset";
const CUSTOM_KEY = "pixhaus:custom-keybinds";

function loadTheme(): Theme {
  const v = localStorage.getItem(THEME_KEY);
  return v === "dark" || v === "light" || v === "pixhaus" ? v : "pixhaus";
}

function loadPreset(): KeybindPreset {
  const v = localStorage.getItem(PRESET_KEY);
  return v === "aseprite" || v === "photoshop" || v === "custom" ? v : "aseprite";
}

function isStringRecord(v: unknown): v is Record<string, string> {
  return (
    v !== null &&
    typeof v === "object" &&
    !Array.isArray(v) &&
    Object.values(v as Record<string, unknown>).every((x) => typeof x === "string")
  );
}

function loadCustom(): Record<string, string> {
  return loadStorageJSON<Record<string, string>>(CUSTOM_KEY, {}, isStringRecord);
}

const [theme, setThemeInternal] = createSignal<Theme>(loadTheme());
const [keybindPreset, setKeybindPresetInternal] = createSignal<KeybindPreset>(loadPreset());
const [customKeybinds, setCustomKeybindsInternal] =
  createSignal<Record<string, string>>(loadCustom());

// Apply initial theme to DOM immediately on module load (read directly from
// storage rather than the signal to avoid a reactive-outside-owner warning)
document.documentElement.dataset["theme"] = loadTheme();

export { theme, keybindPreset, customKeybinds };

export function setTheme(t: Theme): void {
  setThemeInternal(t);
  localStorage.setItem(THEME_KEY, t);
  document.documentElement.dataset["theme"] = t;
}

export function setKeybindPreset(p: KeybindPreset): void {
  setKeybindPresetInternal(p);
  localStorage.setItem(PRESET_KEY, p);
}

export function setCustomKeybind(commandId: string, combo: string): void {
  setCustomKeybindsInternal((prev) => {
    const next = { ...prev, [commandId]: combo };
    localStorage.setItem(CUSTOM_KEY, JSON.stringify(next));
    return next;
  });
}

export function clearCustomKeybind(commandId: string): void {
  setCustomKeybindsInternal((prev) => {
    const next = { ...prev };
    delete next[commandId];
    localStorage.setItem(CUSTOM_KEY, JSON.stringify(next));
    return next;
  });
}
