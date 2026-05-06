// Consent dialog shown once on first launch to ask whether the user wants
// to opt into anonymous crash reporting.

import { onCleanup, onMount, type Component } from "solid-js";

interface Props {
  onAccept: () => void;
  onDecline: () => void;
}

const FirstLaunchDialog: Component<Props> = (props) => {
  let primaryBtn!: HTMLButtonElement;

  onMount(() => {
    // Move keyboard focus into the dialog so screen-reader users land
    // on an actionable control immediately and Escape gets routed via
    // the dialog's onKeyDown handler.
    primaryBtn.focus();

    const onDocKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        props.onDecline();
      }
    };
    document.addEventListener("keydown", onDocKey);
    onCleanup(() => document.removeEventListener("keydown", onDocKey));
  });

  return (
    <div
      class="first-launch-backdrop"
      onClick={(e) => {
        // Backdrop click dismisses with the conservative (decline) path.
        // Clicks bubbling out of the inner dialog are stopped below so
        // the user can still interact with the buttons normally.
        if (e.target === e.currentTarget) props.onDecline();
      }}
    >
      <div
        class="first-launch-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="first-launch-title"
        onClick={(e) => e.stopPropagation()}
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
            ref={primaryBtn}
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
