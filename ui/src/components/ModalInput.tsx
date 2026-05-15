// Reusable single-field input dialog.
//
// Used wherever the UI needs the user to type a short string (rename a
// tag, name a new asset, etc.) without dragging in form-library scope.

import { type Component, Show, createEffect, createSignal, createUniqueId } from "solid-js";
import { Button } from "../lib/ui/Button";
import { Dialog } from "../lib/ui/Dialog";

type Props = {
  readonly open: boolean;
  readonly title: string;
  readonly label: string;
  readonly initialValue: string;
  readonly placeholder?: string;
  readonly submitLabel?: string;
  readonly validate?: (value: string) => string | null;
  readonly onSubmit: (value: string) => void;
  readonly onClose: () => void;
};

const ModalInput: Component<Props> = (props) => {
  const [value, setValue] = createSignal("");
  const [error, setError] = createSignal<string | null>(null);
  const inputId = createUniqueId();

  createEffect(() => {
    if (props.open) {
      setValue(props.initialValue);
      setError(null);
    }
  });

  function submit(): void {
    const v = value();
    const err = props.validate?.(v) ?? null;
    if (err !== null) {
      setError(err);
      return;
    }
    props.onSubmit(v);
  }

  function onInputKeyDown(e: KeyboardEvent): void {
    // Don't let Backspace/Delete bubble to panel-level shortcut handlers
    // that would otherwise treat them as "delete frame".
    e.stopPropagation();
    if (e.key === "Enter") {
      e.preventDefault();
      submit();
    }
  }

  return (
    <Dialog open={props.open} title={props.title} onClose={props.onClose} size="sm">
      <Dialog.Body>
        <div class="modal-input__row">
          <label class="prefs__label" for={inputId}>
            {props.label}
          </label>
          <input
            id={inputId}
            class="modal-input__field"
            value={value()}
            placeholder={props.placeholder}
            onInput={(e) => {
              setValue(e.currentTarget.value);
              setError(null);
            }}
            onKeyDown={onInputKeyDown}
          />
        </div>
        <Show when={error() !== null}>
          <p class="form-field__error" role="alert">
            {error()}
          </p>
        </Show>
      </Dialog.Body>
      <Dialog.Footer>
        <Button variant="ghost" onClick={props.onClose}>
          Cancel
        </Button>
        <Button onClick={submit}>{props.submitLabel ?? "OK"}</Button>
      </Dialog.Footer>
    </Dialog>
  );
};

export default ModalInput;
