// Consent dialog shown once on first launch to ask whether the user wants
// to opt into anonymous crash reporting.

import { type Component } from "solid-js";

interface Props {
  onAccept: () => void;
  onDecline: () => void;
}

const FirstLaunchDialog: Component<Props> = (props) => {
  return (
    <div class="first-launch-backdrop">
      <div
        class="first-launch-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="first-launch-title"
      >
        <h2 id="first-launch-title" class="first-launch-dialog__title">
          Help improve Pixhaus?
        </h2>
        <p class="first-launch-dialog__body">
          Automatically send anonymous crash reports when Pixhaus encounters an unexpected error. No
          project content, file names, or personal information is included. You can change this
          setting at any time in Preferences &rsaquo; Privacy.
        </p>
        <div class="first-launch-dialog__actions">
          <button
            class="first-launch-dialog__btn first-launch-dialog__btn--secondary"
            onClick={() => props.onDecline()}
          >
            No thanks
          </button>
          <button
            class="first-launch-dialog__btn first-launch-dialog__btn--primary"
            onClick={() => props.onAccept()}
          >
            Yes, send crash reports
          </button>
        </div>
      </div>
    </div>
  );
};

export default FirstLaunchDialog;
