import { createSignal, For, type Component } from "solid-js";
import { closePreferences } from "./preferences-state";
import {
  theme,
  setTheme,
  keybindPreset,
  setKeybindPreset,
  customKeybinds,
  clearCustomKeybind,
  crashReportingEnabled,
  setCrashReportingEnabled,
  type Theme,
  type KeybindPreset,
} from "./preferences-store";
import { setCrashReportingEnabled as setSentryEnabled } from "../crash-reporting/crash-reporting";
import { getAllCommands } from "../command-palette/command-registry";

type Tab = "general" | "keybinds" | "ai" | "privacy";

const PreferencesModal: Component = () => {
  const [activeTab, setActiveTab] = createSignal<Tab>("general");

  let dialogRef: HTMLDivElement | undefined;

  function onBackdropClick(e: MouseEvent): void {
    if (e.target === e.currentTarget) closePreferences();
  }

  function onKeyDown(e: KeyboardEvent): void {
    if (e.key === "Escape") {
      e.preventDefault();
      closePreferences();
    }
  }

  return (
    <div class="prefs-backdrop" onClick={onBackdropClick} onKeyDown={onKeyDown}>
      <div ref={dialogRef} class="prefs" role="dialog" aria-label="Preferences" aria-modal="true">
        <div class="prefs__header">
          <h2 class="prefs__title">Preferences</h2>
          <button class="prefs__close" onClick={closePreferences} aria-label="Close Preferences">
            ✕
          </button>
        </div>

        <div class="prefs__tabs" role="tablist">
          <button
            class="prefs__tab"
            role="tab"
            aria-selected={activeTab() === "general"}
            onClick={() => setActiveTab("general")}
          >
            General
          </button>
          <button
            class="prefs__tab"
            role="tab"
            aria-selected={activeTab() === "keybinds"}
            onClick={() => setActiveTab("keybinds")}
          >
            Keybinds
          </button>
          <button
            class="prefs__tab"
            role="tab"
            aria-selected={activeTab() === "ai"}
            onClick={() => setActiveTab("ai")}
          >
            AI Backend
          </button>
          <button
            class="prefs__tab"
            role="tab"
            aria-selected={activeTab() === "privacy"}
            onClick={() => setActiveTab("privacy")}
          >
            Privacy
          </button>
        </div>

        <div class="prefs__body">
          {activeTab() === "general" && <GeneralTab />}
          {activeTab() === "keybinds" && <KeybindsTab />}
          {activeTab() === "ai" && <AiTab />}
          {activeTab() === "privacy" && <PrivacyTab />}
        </div>

        <div class="prefs__footer">
          <button class="prefs__btn" onClick={closePreferences}>
            Close
          </button>
        </div>
      </div>
    </div>
  );
};

const GeneralTab: Component = () => {
  return (
    <div class="prefs__section">
      <p class="prefs__section-title">Appearance</p>
      <div class="prefs__row">
        <div>
          <div class="prefs__label">Theme</div>
          <div class="prefs__sublabel">Controls the color scheme of the editor</div>
        </div>
        <select
          class="prefs__select"
          value={theme()}
          onChange={(e) => setTheme(e.currentTarget.value as Theme)}
        >
          <option value="pixhaus">Pixhaus (default)</option>
          <option value="dark">Dark</option>
          <option value="light">Light</option>
        </select>
      </div>
    </div>
  );
};

const KeybindsTab: Component = () => {
  const commands = getAllCommands();
  const custom = customKeybinds;

  return (
    <>
      <div class="prefs__section">
        <p class="prefs__section-title">Preset</p>
        <div class="prefs__row">
          <div>
            <div class="prefs__label">Keybind preset</div>
            <div class="prefs__sublabel">
              Aseprite-compatible defaults or Photoshop-compatible defaults
            </div>
          </div>
          <select
            class="prefs__select"
            value={keybindPreset()}
            onChange={(e) => setKeybindPreset(e.currentTarget.value as KeybindPreset)}
          >
            <option value="aseprite">Aseprite</option>
            <option value="photoshop">Photoshop</option>
            <option value="custom">Custom</option>
          </select>
        </div>
      </div>

      <div class="prefs__section">
        <p class="prefs__section-title">Shortcuts</p>
        <table class="prefs__kbd-table">
          <tbody>
            <For each={commands}>
              {(cmd) => {
                const override = () => custom()[cmd.id];
                return (
                  <tr>
                    <td>
                      {cmd.category} — {cmd.label}
                    </td>
                    <td>
                      {override() !== undefined ? (
                        <span>
                          <kbd class="prefs__kbd">{override()}</kbd>{" "}
                          <button
                            style={{
                              "font-size": "10px",
                              color: "var(--text-disabled)",
                              cursor: "pointer",
                            }}
                            onClick={() => clearCustomKeybind(cmd.id)}
                          >
                            reset
                          </button>
                        </span>
                      ) : cmd.keybind !== undefined ? (
                        <kbd class="prefs__kbd">{cmd.keybind}</kbd>
                      ) : (
                        <span style={{ color: "var(--text-disabled)" }}>—</span>
                      )}
                    </td>
                  </tr>
                );
              }}
            </For>
          </tbody>
        </table>
      </div>
    </>
  );
};

const AiTab: Component = () => {
  return (
    <div class="prefs__section">
      <p class="prefs__section-title">AI Backend</p>
      <div class="prefs__row">
        <div class="prefs__label">
          AI backend configuration will be available once the AI verb runtime (B5) lands.
        </div>
      </div>
    </div>
  );
};

const PrivacyTab: Component = () => {
  function handleToggle(e: Event): void {
    const enabled = (e.currentTarget as HTMLInputElement).checked;
    setCrashReportingEnabled(enabled);
    setSentryEnabled(enabled);
  }

  return (
    <div class="prefs__section">
      <p class="prefs__section-title">Crash reporting</p>
      <div class="prefs__row">
        <div>
          <div class="prefs__label">Send anonymous crash reports</div>
          <div class="prefs__sublabel">
            Sends stack traces and OS info when Pixhaus crashes. No project content, file names, or
            personal information is included. See{" "}
            <a
              href="https://pixhaus.app/privacy"
              target="_blank"
              rel="noopener noreferrer"
              style={{ color: "var(--accent)" }}
            >
              privacy policy
            </a>{" "}
            for details.
          </div>
        </div>
        <input
          type="checkbox"
          checked={crashReportingEnabled()}
          onChange={handleToggle}
          aria-label="Enable anonymous crash reporting"
        />
      </div>
    </div>
  );
};

export default PreferencesModal;
