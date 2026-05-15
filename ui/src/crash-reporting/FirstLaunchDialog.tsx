// Consent dialog shown once on first launch to ask whether the user wants
// to opt into anonymous crash reporting.

import { type Component } from "solid-js";
import { Button } from "../lib/ui/Button";
import { Dialog } from "../lib/ui/Dialog";

interface Props {
  onAccept: () => void;
  onDecline: () => void;
}

const FirstLaunchDialog: Component<Props> = (props) => {
  // onClose handles both backdrop click and Escape; both dismiss as decline
  // (the conservative path for a consent dialog). Changing this contract
  // would change the consent semantics for crash-reporting opt-in.
  return (
    <Dialog
      open={true}
      title="Help improve Pixhaus?"
      onClose={props.onDecline}
      size="md"
      initialFocus="primary"
      testId="first-launch-dialog"
    >
      <Dialog.Body>
        <p class="first-launch-dialog__body">
          Automatically send anonymous crash reports when Pixhaus encounters an unexpected error. No
          project content, file names, or personal information is included. You can change this
          setting at any time in Preferences &rsaquo; Privacy.
        </p>
      </Dialog.Body>
      <Dialog.Footer>
        <Button variant="ghost" onClick={props.onDecline} data-testid="first-launch-dialog-decline">
          No thanks
        </Button>
        <Button onClick={props.onAccept} data-testid="first-launch-dialog-accept">
          Yes, send crash reports
        </Button>
      </Dialog.Footer>
    </Dialog>
  );
};

export default FirstLaunchDialog;
