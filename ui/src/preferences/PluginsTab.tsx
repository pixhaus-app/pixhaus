// Plugins tab in the Preferences modal.
//
// Lists loaded plugins with per-row reload and unload. The Rescan
// button re-walks the plugin directory and picks up anything new.
//
// Hot-reload via the file watcher still happens in the background;
// this UI is the explicit, audit-friendly path the user reaches for
// when something looks off.

import { createSignal, For, onMount, Show, type Component } from "solid-js";
import { pluginList, pluginReload, pluginScan, pluginUnload } from "../lib/commands/plugins";
import type { PluginInfo } from "../lib/commands/plugins";
import { pushToast } from "../lib/toast/toast-state";
import { reportCommandFailure } from "../lib/utils/errors";

const PluginsTab: Component = () => {
  const [plugins, setPlugins] = createSignal<PluginInfo[]>([]);
  const [loading, setLoading] = createSignal(true);
  const [busy, setBusy] = createSignal<string | null>(null);
  const [confirmUnload, setConfirmUnload] = createSignal<string | null>(null);

  async function refresh(): Promise<void> {
    try {
      const list = await pluginList();
      setPlugins(list);
    } catch (err) {
      reportCommandFailure("List plugins", err);
    } finally {
      setLoading(false);
    }
  }

  onMount(() => {
    void refresh();
  });

  async function handleRescan(): Promise<void> {
    setBusy("__rescan__");
    try {
      const count = await pluginScan();
      pushToast({
        kind: "info",
        title: "Plugins rescanned",
        body: `${count} loaded`,
      });
      await refresh();
    } catch (err) {
      reportCommandFailure("Rescan plugins", err);
    } finally {
      setBusy(null);
    }
  }

  async function handleReload(name: string): Promise<void> {
    setBusy(name);
    try {
      await pluginReload(name);
      pushToast({
        kind: "info",
        title: "Plugin reloaded",
        body: name,
      });
      await refresh();
    } catch (err) {
      reportCommandFailure(`Reload ${name}`, err);
    } finally {
      setBusy(null);
    }
  }

  async function handleUnload(name: string): Promise<void> {
    setBusy(name);
    setConfirmUnload(null);
    try {
      await pluginUnload(name);
      pushToast({
        kind: "info",
        title: "Plugin unloaded",
        body: name,
      });
      await refresh();
    } catch (err) {
      reportCommandFailure(`Unload ${name}`, err);
    } finally {
      setBusy(null);
    }
  }

  return (
    <div class="prefs__section">
      <div class="prefs__row">
        <div>
          <div class="prefs__label">Loaded plugins</div>
          <div class="prefs__sublabel">
            Plugins discovered in the plugin directory at startup. Drop a new plugin folder in and
            click Rescan to pick it up without restarting.
          </div>
        </div>
        <button
          class="prefs__btn prefs__btn--ghost"
          onClick={() => void handleRescan()}
          disabled={busy() !== null}
        >
          Rescan plugins
        </button>
      </div>

      <Show
        when={!loading()}
        fallback={
          <div class="prefs__row">
            <div class="prefs__sublabel">Loading plugins…</div>
          </div>
        }
      >
        <Show
          when={plugins().length > 0}
          fallback={
            <div class="prefs__row">
              <div class="prefs__sublabel">No plugins loaded.</div>
            </div>
          }
        >
          <table class="prefs__plugins-table">
            <tbody>
              <For each={plugins()}>
                {(plugin) => (
                  <tr>
                    <td>
                      <div>
                        <strong>{plugin.name}</strong> <span>v{plugin.version}</span>
                      </div>
                      <div style={{ "font-size": "11px", color: "var(--text-disabled)" }}>
                        {plugin.dir}
                      </div>
                      <div style={{ "font-size": "11px", color: "var(--text-secondary)" }}>
                        {plugin.verb_count} verb{plugin.verb_count === 1 ? "" : "s"}
                        {plugin.description !== undefined ? ` — ${plugin.description}` : ""}
                      </div>
                    </td>
                    <td>
                      <Show
                        when={confirmUnload() === plugin.name}
                        fallback={
                          <span>
                            <button
                              class="prefs__btn prefs__btn--ghost"
                              style={{ "margin-right": "6px" }}
                              onClick={() => void handleReload(plugin.name)}
                              disabled={busy() !== null}
                            >
                              Reload
                            </button>
                            <button
                              class="prefs__btn prefs__btn--ghost"
                              onClick={() => setConfirmUnload(plugin.name)}
                              disabled={busy() !== null}
                            >
                              Unload
                            </button>
                          </span>
                        }
                      >
                        <span>
                          <span
                            style={{
                              "font-size": "11px",
                              color: "var(--text-secondary)",
                              "margin-right": "8px",
                            }}
                          >
                            Confirm unload?
                          </span>
                          <button
                            class="prefs__btn"
                            style={{ "margin-right": "6px" }}
                            onClick={() => void handleUnload(plugin.name)}
                            disabled={busy() !== null}
                          >
                            Yes
                          </button>
                          <button
                            class="prefs__btn prefs__btn--ghost"
                            onClick={() => setConfirmUnload(null)}
                            disabled={busy() !== null}
                          >
                            Cancel
                          </button>
                        </span>
                      </Show>
                    </td>
                  </tr>
                )}
              </For>
            </tbody>
          </table>
        </Show>
      </Show>
    </div>
  );
};

export default PluginsTab;
