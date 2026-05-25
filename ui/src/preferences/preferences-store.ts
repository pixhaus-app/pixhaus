import { createStore } from "solid-js/store";
import { crashReportingGetEnabled } from "../lib/commands/crash-reporting";
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

// One store for editor preferences. Reads are prefs.theme, prefs.keybindPreset,
// prefs.crashReportingEnabled, etc.; writes go through the named setters (which
// also persist to localStorage). crashReportingUserTouched flips on the first
// user-initiated toggle so the hydrator can bail if the user changed the
// preference mid-IPC — the user's newer choice wins.
interface PreferencesState {
  theme: Theme;
  keybindPreset: KeybindPreset;
  customKeybinds: Record<string, string>;
  crashReportingEnabled: boolean;
  crashReportingDialogShown: boolean;
  crashReportingUserTouched: boolean;
}

export const [prefs, setPrefs] = createStore<PreferencesState>({
  theme: initialTheme,
  keybindPreset: loadPreset(),
  customKeybinds: loadCustom(),
  crashReportingEnabled: loadCrashEnabled(),
  crashReportingDialogShown: loadCrashDialogShown(),
  crashReportingUserTouched: false,
});

// Apply initial theme to DOM immediately on module load (the literal value,
// not a reactive read, to avoid a reactive-outside-owner warning).
document.documentElement.dataset["theme"] = initialTheme;

/** Anonymous stable identifier for the local installation. */
export const crashReportingUid: string = loadOrCreateUid();

export function setTheme(t: Theme): void {
  setPrefs("theme", t);
  localStorage.setItem(THEME_KEY, t);
  document.documentElement.dataset["theme"] = t;
}

export function setKeybindPreset(p: KeybindPreset): void {
  setPrefs("keybindPreset", p);
  localStorage.setItem(PRESET_KEY, p);
}

export function setCustomKeybind(commandId: string, combo: string): void {
  const next = { ...prefs.customKeybinds, [commandId]: combo };
  localStorage.setItem(CUSTOM_KEY, JSON.stringify(next));
  setPrefs("customKeybinds", next);
}

export function clearCustomKeybind(commandId: string): void {
  const next = { ...prefs.customKeybinds };
  delete next[commandId];
  localStorage.setItem(CUSTOM_KEY, JSON.stringify(next));
  setPrefs("customKeybinds", next);
}

export function setCrashReportingEnabled(enabled: boolean): void {
  // Mark the preference as user-touched so a late-arriving hydrator
  // response from app start can't clobber the choice we're recording here.
  setPrefs("crashReportingUserTouched", true);
  setPrefs("crashReportingEnabled", enabled);
  localStorage.setItem(CRASH_ENABLED_KEY, enabled ? "1" : "0");
}

export function markCrashReportingDialogShown(): void {
  setPrefs("crashReportingDialogShown", true);
  localStorage.setItem(CRASH_DIALOG_SHOWN_KEY, "1");
}

/**
 * Reseed the crash-reporting signal from the backend.
 *
 * The backend's atomic gate is the source of truth — it's persisted to
 * `<config>/pixhaus/crash_reporting.txt` and read at process start. Call
 * this on app start before initialising Sentry so the gate starts at the
 * user's real choice rather than the localStorage cache.
 *
 * If the user has already touched the preference (Preferences toggle or
 * first-launch dialog) between this call's start and its resolution, the
 * user's choice wins and we discard the (now-stale) backend value. On
 * backend failure we keep the localStorage-seeded signal.
 *
 * Returns the value now in the signal.
 */
export async function hydrateCrashReportingFromBackend(): Promise<boolean> {
  try {
    const backendValue = await crashReportingGetEnabled();
    if (prefs.crashReportingUserTouched) {
      // The user flipped the toggle while the IPC was in flight; the
      // newer choice is already in the store and persisted to the
      // backend via setCrashReportingEnabled. Don't apply the stale read.
      return prefs.crashReportingEnabled;
    }
    setPrefs("crashReportingEnabled", backendValue);
    localStorage.setItem(CRASH_ENABLED_KEY, backendValue ? "1" : "0");
    return backendValue;
  } catch (err: unknown) {
    console.error(
      "[pixhaus] crash_reporting_get_enabled failed, falling back to localStorage:",
      err,
    );
    return prefs.crashReportingEnabled;
  }
}
