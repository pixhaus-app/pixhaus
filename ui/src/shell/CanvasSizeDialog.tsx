// Canvas-size dialog used by "New Project" (Welcome screen) and "New
// Sprite" (command palette). Presets keep the common cases one click
// away; the width/height inputs cover everything else. Last-used size
// is persisted so a user working at 64x64 doesn't get bounced back to
// 32x32 every time.

import { type Component, For, Show, createEffect, createSignal } from "solid-js";
import { Button } from "../lib/ui/Button";
import { Dialog } from "../lib/ui/Dialog";
import {
  canvasSizeRequest,
  closeCanvasSizeDialog,
  loadLastCanvasSize,
  saveLastCanvasSize,
  parseCanvasDim,
  validateCanvasDim,
  MIN_CANVAS_DIM,
  MAX_CANVAS_DIM,
} from "./canvas-size-dialog-state";

const PRESETS: readonly number[] = [16, 32, 64, 128, 256];
const DEFAULT_PROJECT_NAME = "Untitled";

const CanvasSizeDialog: Component = () => {
  const [widthInput, setWidthInput] = createSignal("32");
  const [heightInput, setHeightInput] = createSignal("32");
  const [name, setName] = createSignal(DEFAULT_PROJECT_NAME);
  const [error, setError] = createSignal<string | null>(null);

  createEffect(() => {
    const req = canvasSizeRequest();
    if (req === null) return;
    const last = loadLastCanvasSize();
    setWidthInput(String(last.width));
    setHeightInput(String(last.height));
    setName(DEFAULT_PROJECT_NAME);
    setError(null);
  });

  function applyPreset(n: number): void {
    setWidthInput(String(n));
    setHeightInput(String(n));
    setError(null);
  }

  function currentSelectedPreset(): number | null {
    const w = parseCanvasDim(widthInput());
    const h = parseCanvasDim(heightInput());
    if (w === null || h === null || w !== h) return null;
    return PRESETS.includes(w) ? w : null;
  }

  function submit(e: SubmitEvent): void {
    e.preventDefault();
    const req = canvasSizeRequest();
    if (req === null) return;
    const w = parseCanvasDim(widthInput());
    const h = parseCanvasDim(heightInput());
    const widthErr = validateCanvasDim(w);
    const heightErr = validateCanvasDim(h);
    if (widthErr !== null) {
      setError(`Width: ${widthErr}`);
      return;
    }
    if (heightErr !== null) {
      setError(`Height: ${heightErr}`);
      return;
    }
    const width = w as number;
    const height = h as number;
    saveLastCanvasSize({ width, height });
    const result =
      req.mode === "project"
        ? { name: name().trim() || DEFAULT_PROJECT_NAME, width, height }
        : { width, height };
    closeCanvasSizeDialog();
    req.onConfirm(result);
  }

  function submitDisabled(): boolean {
    const w = parseCanvasDim(widthInput());
    const h = parseCanvasDim(heightInput());
    return validateCanvasDim(w) !== null || validateCanvasDim(h) !== null;
  }

  const title = () => (canvasSizeRequest()?.mode === "project" ? "New project" : "New sprite");

  return (
    <Dialog
      open={canvasSizeRequest() !== null}
      title={title()}
      onClose={closeCanvasSizeDialog}
      size="md"
    >
      <form onSubmit={submit}>
        <Dialog.Body>
          <Show when={canvasSizeRequest()?.mode === "project"}>
            <div class="prefs__row">
              <div>
                <div class="prefs__label">Name</div>
              </div>
              <input
                class="ts-picker__input"
                data-testid="canvas-size-name"
                value={name()}
                onInput={(e) => setName(e.currentTarget.value)}
              />
            </div>
          </Show>

          <p class="prefs__section-title">Presets</p>
          <div class="canvas-size-dialog__presets" role="radiogroup">
            <For each={PRESETS}>
              {(size) => (
                <button
                  type="button"
                  class="btn btn--ghost btn--sm canvas-size-dialog__preset"
                  classList={{
                    "canvas-size-dialog__preset--active": currentSelectedPreset() === size,
                  }}
                  data-testid={`canvas-size-preset-${size}`}
                  aria-checked={currentSelectedPreset() === size}
                  role="radio"
                  onClick={() => applyPreset(size)}
                >
                  {size}×{size}
                </button>
              )}
            </For>
          </div>

          <div class="prefs__row">
            <div>
              <div class="prefs__label">Canvas size</div>
              <div class="prefs__sublabel">
                Width × height in pixels ({MIN_CANVAS_DIM}–{MAX_CANVAS_DIM})
              </div>
            </div>
            <div class="ts-picker__size">
              <input
                class="ts-picker__input ts-picker__input--num"
                type="number"
                min={MIN_CANVAS_DIM}
                max={MAX_CANVAS_DIM}
                step="1"
                data-testid="canvas-size-width"
                value={widthInput()}
                onInput={(e) => {
                  setWidthInput(e.currentTarget.value);
                  setError(null);
                }}
              />
              <span aria-hidden="true">×</span>
              <input
                class="ts-picker__input ts-picker__input--num"
                type="number"
                min={MIN_CANVAS_DIM}
                max={MAX_CANVAS_DIM}
                step="1"
                data-testid="canvas-size-height"
                value={heightInput()}
                onInput={(e) => {
                  setHeightInput(e.currentTarget.value);
                  setError(null);
                }}
              />
            </div>
          </div>

          <Show when={error() !== null}>
            <p class="form-field__error" data-testid="canvas-size-error" role="alert">
              {error()}
            </p>
          </Show>
        </Dialog.Body>

        <Dialog.Footer>
          <Button variant="ghost" onClick={closeCanvasSizeDialog} data-testid="canvas-size-cancel">
            Cancel
          </Button>
          <Button type="submit" data-testid="canvas-size-create" disabled={submitDisabled()}>
            Create
          </Button>
        </Dialog.Footer>
      </form>
    </Dialog>
  );
};

export default CanvasSizeDialog;
