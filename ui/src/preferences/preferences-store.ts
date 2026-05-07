import { createSignal } from "solid-js";
import { loadStorageJSON } from "../lib/utils/storage";

export type Theme = "dark" | "light" | "pixhaus";
export type KeybindPreset = "aseprite" | "photoshop" | "custom";

const THEME_KEY = "pixhaus:theme";
const PRESET_KEY = "pixhaus:keybind-preset";
const CUSTOM_KEY = "pixhaus:custom-keybinds";

// Crash-reporting keys
const CRASH_ENABLED_KEY = "pixhaus:crash-reporting-enabled";
const CRASH_DIALOG_SHOWN_KEY = "pixhaus:crash-reporting-dialog-shown";
const CRASH_UID_KEY = "pixhaus:crash-reporting-uid";

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

function loadCrashEnabled(): boolean {
  return localStorage.getItem(CRASH_ENABLED_KEY) === "1";
}

function loadCrashDialogShown(): boolean {
  return localStorage.getItem(CRASH_DIALOG_SHOWN_KEY) === "1";
}

function loadOrCreateUid(): string {
  const stored = localStorage.getItem(CRASH_UID_KEY);
  if (stored) return stored;
  // Tauri's webview is modern Chromium (>= 92) so crypto.randomUUID is
  // always available; no fallback needed. Math.random()-based UUIDs are
  // not cryptographically random and easily collide across reinstalls.
  const uid = crypto.randomUUID();
  localStorage.setItem(CRASH_UID_KEY, uid);
  return uid;
}

const initialTheme = loadTheme();
const [theme, setThemeInternal] = createSignal<Theme>(initialTheme);
const [keybindPreset, setKeybindPresetInternal] = createSignal<KeybindPreset>(loadPreset());
const [customKeybinds, setCustomKeybindsInternal] =
  createSignal<Record<string, string>>(loadCustom());
const [crashReportingEnabled, setCrashReportingEnabledInternal] =
  createSignal<boolean>(loadCrashEnabled());
const [crashReportingDialogShown, setCrashReportingDialogShownInternal] =
  createSignal<boolean>(loadCrashDialogShown());

// Apply initial theme to DOM immediately on module load (the literal value,
// not the signal, to avoid a reactive-outside-owner warning).
document.documentElement.dataset["theme"] = initialTheme;

export { theme, keybindPreset, customKeybinds, crashReportingEnabled, crashReportingDialogShown };

/** Anonymous stable identifier for the local installation. */
export const crashReportingUid: string = loadOrCreateUid();

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

export function setCrashReportingEnabled(enabled: boolean): void {
  setCrashReportingEnabledInternal(enabled);
  localStorage.setItem(CRASH_ENABLED_KEY, enabled ? "1" : "0");
}

export function markCrashReportingDialogShown(): void {
  setCrashReportingDialogShownInternal(true);
  localStorage.setItem(CRASH_DIALOG_SHOWN_KEY, "1");
}
