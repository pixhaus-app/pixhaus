import { createSignal, Show, type Component } from "solid-js";
import { Button } from "../lib/ui/Button";
import { Dialog } from "../lib/ui/Dialog";
import { closeUpdateModal, updateModal } from "./update-modal-state";
import { updaterInstall } from "../lib/commands/updater";
import { reportCommandFailure } from "../lib/utils/errors";
import { pushToast } from "../lib/toast/toast-state";

const UpdateAvailableModal: Component = () => {
  const [installing, setInstalling] = createSignal(false);

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

  const title = () => {
    const info = updateModal.updateInfo;
    return info ? `Pixhaus ${info.version} is available` : "Update available";
  };

  return (
    <Dialog
      open={updateModal.showUpdateModal}
      title={title()}
      onClose={closeUpdateModal}
      size="lg"
      closeOnBackdrop={!installing()}
      closeOnEscape={!installing()}
    >
      <Dialog.Body>
        <Show
          when={updateModal.updateInfo?.body}
          fallback={<p class="prefs__sublabel">No release notes were included with this update.</p>}
        >
          {(body) => (
            <div class="prefs__section">
              <p class="prefs__section-title">Release notes</p>
              <pre class="update-modal__release-notes">{body()}</pre>
            </div>
          )}
        </Show>
        <Show when={updateModal.updateInfo?.date}>
          {(date) => <p class="prefs__sublabel update-modal__date">Released {date()}</p>}
        </Show>
      </Dialog.Body>

      <Dialog.Footer>
        <Button variant="ghost" onClick={closeUpdateModal} disabled={installing()}>
          Remind me later
        </Button>
        <Button onClick={onInstall} loading={installing()} data-testid="update-modal-install">
          {installing() ? "Installing" : "Install and restart"}
        </Button>
      </Dialog.Footer>
    </Dialog>
  );
};

export default UpdateAvailableModal;
