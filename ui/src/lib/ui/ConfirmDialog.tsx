import type { JSX } from "solid-js";
import { Button } from "./Button";
import { Dialog } from "./Dialog";

type ConfirmDialogProps = {
  open: boolean;
  title: string;
  message: JSX.Element;
  confirmLabel: string;
  cancelLabel?: string;
  onConfirm: () => void;
  onCancel: () => void;
  testId?: string;
};

export function ConfirmDialog(props: ConfirmDialogProps) {
  return (
    <Dialog
      open={props.open}
      title={props.title}
      onClose={props.onCancel}
      size="sm"
      initialFocus="none"
      {...(props.testId !== undefined ? { testId: props.testId } : {})}
    >
      <Dialog.Body>
        <p class="dialog__message">{props.message}</p>
      </Dialog.Body>
      <Dialog.Footer>
        <Button
          variant="ghost"
          onClick={props.onCancel}
          data-testid={props.testId ? `${props.testId}-cancel` : undefined}
        >
          {props.cancelLabel ?? "Cancel"}
        </Button>
        <Button
          variant="danger"
          onClick={props.onConfirm}
          data-testid={props.testId ? `${props.testId}-confirm` : undefined}
        >
          {props.confirmLabel}
        </Button>
      </Dialog.Footer>
    </Dialog>
  );
}
