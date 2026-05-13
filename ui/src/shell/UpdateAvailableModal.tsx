import { createEffect, createSignal, onCleanup, Show, type Component } from "solid-js";
import { closeUpdateModal, showUpdateModal, updateInfo } from "./update-modal-state";
import { updaterInstall } from "../lib/commands/updater";
import { reportCommandFailure } from "../lib/utils/errors";
import { pushToast } from "../lib/toast/toast-state";

const UpdateAvailableModal: Component = () => {
  const [installing, setInstalling] = createSignal(false);

  // Document-level Escape handler — bound only while the modal is open.
  // The previous local onKeyDown on the backdrop required focus to land
  // on (or inside) the backdrop element, which doesn't happen when the
  // modal opens from a menu click — focus stays on the editor behind it.
  createEffect(() => {
    if (!showUpdateModal()) return;
    function onKeyDown(e: KeyboardEvent): void {
      if (installing()) return;
      if (e.key === "Escape") {
        e.preventDefault();
        closeUpdateModal();
      }
    }
    document.addEventListener("keydown", onKeyDown);
    onCleanup(() => document.removeEventListener("keydown", onKeyDown));
  });

  function onBackdropClick(e: MouseEvent): void {
    if (installing()) return;
    if (e.target === e.currentTarget) closeUpdateModal();
  }

  function onInstall(): void {
    setInstalling(true);
    updaterInstall()
      .then(() => {
        // On most platforms the Tauri updater quits the process during
        // staging, so this branch never runs (the modal goes with the
        // app). On platforms where the install completes without an
        // auto-restart (Linux AppImage, some Windows MSI flows), reset
        // the button state, close the modal, and inform the user what
        // to do next via a sticky toast — without this the button stays
        // pinned on "Installing..." forever.
        setInstalling(false);
        closeUpdateModal();
        pushToast({
          kind: "success",
          title: "Update staged",
          body: "Restart Pixhaus to finish installing.",
          durationMs: 0,
        });
      })
      .catch((err: unknown) => {
        reportCommandFailure("updater_install", err);
        setInstalling(false);
      });
  }

  return (
    <Show when={showUpdateModal()}>
      <div class="prefs-backdrop" onClick={onBackdropClick} data-testid="update-modal-backdrop">
        <div
          class="prefs"
          role="dialog"
          aria-label="Update available"
          aria-modal="true"
          data-testid="update-modal"
        >
          <div class="prefs__header">
            <h2 class="prefs__title">
              <Show when={updateInfo()} fallback="Update available">
                {(info) => <>Pixhaus {info().version} is available</>}
              </Show>
            </h2>
            <button
              class="prefs__close"
              onClick={closeUpdateModal}
              disabled={installing()}
              aria-label="Close"
            >
              x
            </button>
          </div>

          <div class="prefs__body">
            <Show
              when={updateInfo()?.body}
              fallback={
                <p class="prefs__sublabel">No release notes were included with this update.</p>
              }
            >
              {(body) => (
                <div class="prefs__section">
                  <p class="prefs__section-title">Release notes</p>
                  <pre
                    style={{
                      "white-space": "pre-wrap",
                      "font-family": "inherit",
                      "font-size": "13px",
                      color: "var(--text-primary)",
                      margin: "0",
                    }}
                  >
                    {body()}
                  </pre>
                </div>
              )}
            </Show>
            <Show when={updateInfo()?.date}>
              {(date) => (
                <p class="prefs__sublabel" style={{ "margin-top": "12px" }}>
                  Released {date()}
                </p>
              )}
            </Show>
          </div>

          <div class="prefs__footer" style={{ gap: "8px" }}>
            <button
              class="prefs__btn prefs__btn--ghost"
              onClick={closeUpdateModal}
              disabled={installing()}
            >
              Remind me later
            </button>
            <button
              class="prefs__btn"
              onClick={onInstall}
              disabled={installing()}
              data-testid="update-modal-install"
            >
              {installing() ? "Installing..." : "Install and restart"}
            </button>
          </div>
        </div>
      </div>
    </Show>
  );
};

export default UpdateAvailableModal;
