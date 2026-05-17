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
  aiClearFalApiKey,
  aiClearGoogleAiApiKey,
  aiGetProviderOverview,
  aiGetOpenAiStatus,
  aiSetFalApiKey,
  aiSetGoogleAiApiKey,
  aiSetOpenAiApiKey,
  type OpenAiStatus,
  type ProviderStatus,
} from "../lib/commands/ai";
import {
  projectClearOperationModelPrefs,
  projectGetStyleNotes,
  projectSetDefaultCandidateCount,
  projectSetDefaultQuality,
  projectSetOperationModelPref,
  projectSetStyleNotes,
} from "../lib/commands/library";
import type { ModelId, OperationKind, Quality } from "../lib/types";
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
  const [providers, setProviders] = createSignal<ProviderStatus[]>([]);
  const [apiKey, setApiKey] = createSignal("");
  const [googleKey, setGoogleKey] = createSignal("");
  const [falKey, setFalKey] = createSignal("");
  const [styleNotes, setStyleNotes] = createSignal("");
  const [prefOperation, setPrefOperation] = createSignal<OperationKind>("fresh_generation");
  const [prefModel, setPrefModel] = createSignal<ModelId>("auto");
  const [defaultQuality, setDefaultQuality] = createSignal<Quality>("medium");
  const [defaultCandidateCount, setDefaultCandidateCount] = createSignal(2);
  const [saving, setSaving] = createSignal(false);

  onMount(() => {
    refreshAiStatus();
    projectGetStyleNotes()
      .then(setStyleNotes)
      .catch((err: unknown) => reportCommandFailure("project_get_style_notes", err));
  });

  function refreshAiStatus(): void {
    aiGetOpenAiStatus()
      .then(setStatus)
      .catch((err: unknown) => reportCommandFailure("ai_get_openai_status", err));
    aiGetProviderOverview()
      .then(setProviders)
      .catch((err: unknown) => reportCommandFailure("ai_get_provider_overview", err));
  }

  function handleSave(): void {
    const key = apiKey().trim();
    if (key.length === 0 || saving()) return;
    setSaving(true);
    aiSetOpenAiApiKey(key)
      .then((next) => {
        setStatus(next);
        setApiKey("");
        refreshAiStatus();
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
        refreshAiStatus();
        pushToast({ kind: "info", title: "OpenAI API key cleared." });
      })
      .catch((err: unknown) => reportCommandFailure("ai_clear_openai_api_key", err))
      .finally(() => setSaving(false));
  }

  function handleProviderSave(provider: "google_ai" | "fal"): void {
    const key = provider === "google_ai" ? googleKey().trim() : falKey().trim();
    if (key.length === 0 || saving()) return;
    setSaving(true);
    const save = provider === "google_ai" ? aiSetGoogleAiApiKey : aiSetFalApiKey;
    save(key)
      .then(() => {
        if (provider === "google_ai") setGoogleKey("");
        else setFalKey("");
        refreshAiStatus();
        pushToast({ kind: "success", title: "API key saved." });
      })
      .catch((err: unknown) => reportCommandFailure("ai_set_provider_api_key", err))
      .finally(() => setSaving(false));
  }

  function handleProviderClear(provider: "google_ai" | "fal"): void {
    if (saving()) return;
    setSaving(true);
    const clear = provider === "google_ai" ? aiClearGoogleAiApiKey : aiClearFalApiKey;
    clear()
      .then(() => {
        refreshAiStatus();
        pushToast({ kind: "info", title: "API key cleared." });
      })
      .catch((err: unknown) => reportCommandFailure("ai_clear_provider_api_key", err))
      .finally(() => setSaving(false));
  }

  function providerStatus(provider: "google_ai" | "fal"): ProviderStatus | null {
    return providers().find((item) => item.provider === provider) ?? null;
  }

  function handleSaveProjectDefaults(): void {
    Promise.all([
      projectSetStyleNotes(styleNotes()),
      projectSetDefaultQuality(defaultQuality()),
      projectSetDefaultCandidateCount(defaultCandidateCount()),
    ])
      .then(() => pushToast({ kind: "success", title: "AI defaults saved." }))
      .catch((err: unknown) => reportCommandFailure("project_set_ai_defaults", err));
  }

  function handleSaveOperationPref(): void {
    projectSetOperationModelPref({ operation: prefOperation(), model: prefModel() })
      .then(() => pushToast({ kind: "success", title: "Operation preference saved." }))
      .catch((err: unknown) => reportCommandFailure("project_set_operation_model_pref", err));
  }

  return (
    <>
      <div class="prefs__section">
        <p class="prefs__section-title">Providers</p>
        <div class="prefs__row">
          <div>
            <div class="prefs__label">OpenAI</div>
            <div class="prefs__sublabel">
              {status()?.configured ? "Configured" : "Not configured"} · Image model:{" "}
              {status()?.model ?? "gpt-image-2"}
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
        <ProviderKeyRow
          label="Google AI Studio"
          placeholder="AIza..."
          status={providerStatus("google_ai")}
          value={googleKey()}
          onInput={setGoogleKey}
          onSave={() => handleProviderSave("google_ai")}
          onClear={() => handleProviderClear("google_ai")}
          saving={saving()}
        />
        <ProviderKeyRow
          label="fal.ai"
          placeholder="fal-key..."
          status={providerStatus("fal")}
          value={falKey()}
          onInput={setFalKey}
          onSave={() => handleProviderSave("fal")}
          onClear={() => handleProviderClear("fal")}
          saving={saving()}
        />
      </div>
      <div class="prefs__section">
        <p class="prefs__section-title">Project AI defaults</p>
        <label class="prefs__row">
          <div>
            <div class="prefs__label">Style notes</div>
            <textarea
              class="prefs__select"
              value={styleNotes()}
              onInput={(event) => setStyleNotes(event.currentTarget.value)}
            />
          </div>
        </label>
        <div class="prefs__row">
          <select
            class="prefs__select"
            value={defaultQuality()}
            onChange={(event) => setDefaultQuality(event.currentTarget.value as Quality)}
          >
            <option value="auto">Auto</option>
            <option value="low">Low</option>
            <option value="medium">Medium</option>
            <option value="high">High</option>
          </select>
          <input
            class="prefs__select"
            type="number"
            min="1"
            max="4"
            value={defaultCandidateCount()}
            onInput={(event) => setDefaultCandidateCount(Number(event.currentTarget.value))}
          />
          <Button onClick={handleSaveProjectDefaults}>Save defaults</Button>
        </div>
      </div>
      <div class="prefs__section">
        <p class="prefs__section-title">Per-operation model preference</p>
        <div class="prefs__row">
          <select
            class="prefs__select"
            value={prefOperation()}
            onChange={(event) => setPrefOperation(event.currentTarget.value as OperationKind)}
          >
            <option value="fresh_generation">Fresh generation</option>
            <option value="masked_refinement">Masked refinement</option>
            <option value="prompt_only_refinement">Prompt-only refinement</option>
            <option value="regional_refinement">Regional refinement</option>
            <option value="chat_turn">Chat turn</option>
            <option value="promotion">Promotion</option>
          </select>
          <select
            class="prefs__select"
            value={prefModel()}
            onChange={(event) => setPrefModel(event.currentTarget.value as ModelId)}
          >
            <option value="auto">Auto</option>
            <option value="open_ai_gpt_image2">OpenAI gpt-image-2</option>
            <option value="google_nano_banana_pro">Nano Banana Pro</option>
            <option value="google_gemini_flash_image">Gemini Flash Image</option>
            <option value="fal_flux_kontext">fal Flux Kontext</option>
            <option value="fal_flux_dev">fal Flux.1 dev</option>
          </select>
          <Button onClick={handleSaveOperationPref}>Save</Button>
          <Button
            variant="ghost"
            onClick={() =>
              projectClearOperationModelPrefs()
                .then(() => pushToast({ kind: "info", title: "Operation preferences cleared." }))
                .catch((err: unknown) =>
                  reportCommandFailure("project_clear_operation_model_prefs", err),
                )
            }
          >
            Clear all
          </Button>
        </div>
      </div>
    </>
  );
};

const ProviderKeyRow: Component<{
  label: string;
  placeholder: string;
  status: ProviderStatus | null;
  value: string;
  onInput: (value: string) => void;
  onSave: () => void;
  onClear: () => void;
  saving: boolean;
}> = (props) => (
  <div class="prefs__row">
    <div>
      <div class="prefs__label">{props.label}</div>
      <div class="prefs__sublabel">
        {props.status?.state ?? "not_configured"}
        {props.status?.registered ? " · backend ready" : ""}
      </div>
      <input
        class="prefs__select"
        type="password"
        autocomplete="off"
        value={props.value}
        placeholder={props.status?.configured ? "Saved key is hidden" : props.placeholder}
        onInput={(event) => props.onInput(event.currentTarget.value)}
      />
    </div>
    <div class="prefs__row-actions">
      <Button onClick={props.onSave} disabled={props.saving || props.value.trim().length === 0}>
        Save
      </Button>
      <Button
        variant="ghost"
        onClick={props.onClear}
        disabled={props.saving || !props.status?.configured}
      >
        Clear
      </Button>
    </div>
  </div>
);

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
