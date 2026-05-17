import { createMemo, createSignal, For, onMount, type Component } from "solid-js";
import { Button } from "../lib/ui/Button";
import { Dialog } from "../lib/ui/Dialog";
import { closePreferences } from "./preferences-state";
import PluginsTab from "./PluginsTab";
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
import {
  aiClearOpenAiApiKey,
  aiGetOpenAiStatus,
  aiSetOpenAiApiKey,
  type OpenAiStatus,
} from "../lib/commands/ai";
import { pushToast } from "../lib/toast/toast-state";
import { reportCommandFailure } from "../lib/utils/errors";

type Tab = "general" | "keybinds" | "ai" | "plugins" | "privacy";

const PreferencesModal: Component = () => {
  const [activeTab, setActiveTab] = createSignal<Tab>("general");

  return (
    <Dialog open={true} title="Preferences" onClose={closePreferences} size="lg">
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
          aria-selected={activeTab() === "plugins"}
          onClick={() => setActiveTab("plugins")}
        >
          Plugins
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

      <Dialog.Body>
        {activeTab() === "general" && <GeneralTab />}
        {activeTab() === "keybinds" && <KeybindsTab />}
        {activeTab() === "ai" && <AiTab />}
        {activeTab() === "plugins" && <PluginsTab />}
        {activeTab() === "privacy" && <PrivacyTab />}
      </Dialog.Body>

      <Dialog.Footer>
        <Button onClick={closePreferences}>Close</Button>
      </Dialog.Footer>
    </Dialog>
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
  const commands = createMemo(() => getAllCommands());
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
            <For each={commands()}>
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
                          <Button
                            variant="ghost"
                            size="sm"
                            class="prefs__kbd-reset"
                            onClick={() => clearCustomKeybind(cmd.id)}
                          >
                            reset
                          </Button>
                        </span>
                      ) : cmd.keybind !== undefined ? (
                        <kbd class="prefs__kbd">{cmd.keybind}</kbd>
                      ) : (
                        <span class="prefs__kbd-empty">—</span>
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
  const [status, setStatus] = createSignal<OpenAiStatus | null>(null);
  const [apiKey, setApiKey] = createSignal("");
  const [saving, setSaving] = createSignal(false);

  onMount(() => {
    aiGetOpenAiStatus()
      .then(setStatus)
      .catch((err: unknown) => reportCommandFailure("ai_get_openai_status", err));
  });

  function handleSave(): void {
    const key = apiKey().trim();
    if (key.length === 0 || saving()) return;
    setSaving(true);
    aiSetOpenAiApiKey(key)
      .then((next) => {
        setStatus(next);
        setApiKey("");
        pushToast({ kind: "success", title: "OpenAI API key saved." });
      })
      .catch((err: unknown) => reportCommandFailure("ai_set_openai_api_key", err))
      .finally(() => setSaving(false));
  }

  function handleClear(): void {
    if (saving()) return;
    setSaving(true);
    aiClearOpenAiApiKey()
      .then((next) => {
        setStatus(next);
        setApiKey("");
        pushToast({ kind: "info", title: "OpenAI API key cleared." });
      })
      .catch((err: unknown) => reportCommandFailure("ai_clear_openai_api_key", err))
      .finally(() => setSaving(false));
  }

  return (
    <div class="prefs__section">
      <p class="prefs__section-title">OpenAI</p>
      <div class="prefs__row">
        <div>
          <div class="prefs__label">
            {status()?.configured ? "API key saved" : "API key missing"}
          </div>
          <div class="prefs__sublabel">
            Image model: {status()?.model ?? "gpt-image-2"}
            {status()?.registered ? " · backend ready" : ""}
          </div>
        </div>
      </div>
      <div class="prefs__row">
        <div>
          <div class="prefs__label">API key</div>
          <input
            class="prefs__select"
            type="password"
            autocomplete="off"
            value={apiKey()}
            placeholder={status()?.configured ? "Saved key is hidden" : "sk-..."}
            onInput={(event) => setApiKey(event.currentTarget.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter") handleSave();
            }}
          />
        </div>
        <div class="prefs__row-actions">
          <Button onClick={handleSave} disabled={saving() || apiKey().trim().length === 0}>
            {saving() ? "Saving…" : "Save"}
          </Button>
          <Button
            variant="ghost"
            onClick={handleClear}
            disabled={saving() || !status()?.configured}
          >
            Clear
          </Button>
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
              class="prefs__link"
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
