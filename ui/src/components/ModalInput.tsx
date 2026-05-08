// Reusable single-field input dialog.
//
// Used wherever the UI needs the user to type a short string (rename a
// tag, name a new asset, etc.) without dragging in form-library scope.
// Mirrors the portal+backdrop styling of TilesetPickerDialog so the look
// stays consistent.

import {
  type Component,
  Show,
  createEffect,
  createSignal,
  createUniqueId,
  onCleanup,
} from "solid-js";

type Props = {
  readonly open: boolean;
  readonly title: string;
  readonly label: string;
  readonly initialValue: string;
  readonly placeholder?: string;
  readonly submitLabel?: string;
  // Returns null when the value is acceptable, otherwise an error message.
  readonly validate?: (value: string) => string | null;
  readonly onSubmit: (value: string) => void;
  readonly onClose: () => void;
};

const ModalInput: Component<Props> = (props) => {
  const [value, setValue] = createSignal("");
  const [error, setError] = createSignal<string | null>(null);
  // Per-instance id so two ModalInputs mounted simultaneously don't collide
  // on `for`/`id` and break label-input association.
  const inputId = createUniqueId();
  let inputRef: HTMLInputElement | undefined;

  // Reset internal state and focus the input each time the dialog opens.
  // Without the reset, reopening with a different `initialValue` would keep
  // the user's last edits from the previous session.
  createEffect(() => {
    if (props.open) {
      setValue(props.initialValue);
      setError(null);
      // Solid runs this effect synchronously during render; defer focus to
      // the next tick so the element is mounted.
      queueMicrotask(() => {
        inputRef?.focus();
        inputRef?.select();
      });
    }
  });

  // Document-level Escape handler — bound only while open.
  createEffect(() => {
    if (!props.open) return;
    function onKeyDown(e: KeyboardEvent): void {
      if (e.key === "Escape") {
        e.preventDefault();
        props.onClose();
      }
    }
    document.addEventListener("keydown", onKeyDown);
    onCleanup(() => document.removeEventListener("keydown", onKeyDown));
  });

  function onBackdropClick(e: MouseEvent): void {
    if (e.target === e.currentTarget) props.onClose();
  }

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
    <Show when={props.open}>
      <div class="prefs-backdrop" onClick={onBackdropClick}>
        <div class="prefs modal-input" role="dialog" aria-modal="true" aria-label={props.title}>
          <div class="prefs__header">
            <h2 class="prefs__title">{props.title}</h2>
            <button class="prefs__close" onClick={() => props.onClose()} aria-label="Close">
              ✕
            </button>
          </div>

          <div class="prefs__body">
            <div class="modal-input__row">
              <label class="prefs__label" for={inputId}>
                {props.label}
              </label>
              <input
                id={inputId}
                ref={inputRef}
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
              <p class="modal-input__error">{error()}</p>
            </Show>
          </div>

          <div class="prefs__footer">
            <button class="prefs__btn prefs__btn--ghost" onClick={() => props.onClose()}>
              Cancel
            </button>
            <button class="prefs__btn" onClick={submit}>
              {props.submitLabel ?? "OK"}
            </button>
          </div>
        </div>
      </div>
    </Show>
  );
};

export default ModalInput;
