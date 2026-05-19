// Dedicated input form for the `animated_sprite_sheet` verb.
//
// Implements the UX the integrations PRD calls out:
//   - grid picker (2..6)
//   - action chip multi-select with `len(actions) <= grid_size`
//   - prompt textarea
//   - mode selector (procedural / ai / hybrid)
//   - optional reference image (file-input → bytes)
//   - cell size, frame duration, seed, layer name
//
// Excess action picks are *silently refused* and surfaced via a brief
// visual flash on the constraint hint — mirroring FalSprite's friendlier
// alternative to disabling or throwing.

import { type Component, For, Show, createMemo, createSignal } from "solid-js";

import { verbInvoke } from "../../lib/commands/verbs";
import { pushToast } from "../../lib/toast/toast-state";
import { reportCommandFailure } from "../../lib/utils/errors";
import {
  type FormState,
  defaultFormState,
  fitActionsToGrid,
  toggleChip,
  toInputs,
  validate,
} from "./form-logic";
import {
  ACTION_PRESETS,
  ANIMATED_SPRITE_SHEET_VERB_ID,
  DEFAULT_CELL_SIZE_PX,
  DEFAULT_FRAME_DURATION_MS,
  type GenerationMode,
  MAX_CELL_SIZE_PX,
  MAX_GRID_SIZE,
  MIN_CELL_SIZE_PX,
  MIN_GRID_SIZE,
} from "./types";
import { closeAnimatedSpriteSheetForm } from "./state";

const MODE_LABELS: ReadonlyArray<{ mode: GenerationMode; label: string; hint: string }> = [
  {
    mode: "ai",
    label: "AI",
    hint: "Run the full CHARACTER × CHOREOGRAPHY → image pipeline.",
  },
  {
    mode: "hybrid",
    label: "Hybrid",
    hint: "Rewrite-only — review the LLM output before re-running in AI mode.",
  },
  {
    mode: "procedural",
    label: "Procedural",
    hint: "Emit g² empty frames for you to draw by hand. No backend call.",
  },
];

const AnimatedSpriteSheetForm: Component<{ onSubmitted?: () => void }> = (props) => {
  const [state, setState] = createSignal<FormState>(defaultFormState());
  const [flashKey, setFlashKey] = createSignal(0);
  const [submitting, setSubmitting] = createSignal(false);

  const validation = createMemo(() => validate(state()));

  function update<K extends keyof FormState>(key: K, value: FormState[K]): void {
    setState({ ...state(), [key]: value });
  }

  function onGridChange(next: number): void {
    const clamped = Math.max(MIN_GRID_SIZE, Math.min(MAX_GRID_SIZE, next | 0));
    const fitted = fitActionsToGrid(state().actions, clamped);
    setState({ ...state(), gridSize: clamped, actions: fitted });
  }

  function onChipClick(chip: string): void {
    const { actions, outcome } = toggleChip(state().actions, chip, state().gridSize);
    if (outcome === "rejected-capacity") {
      // Bump the key so the constraint-hint element remounts and the CSS
      // `flash` animation replays from scratch on every rejection.
      setFlashKey(flashKey() + 1);
      return;
    }
    setState({ ...state(), actions });
  }

  async function onReferenceFile(event: Event): Promise<void> {
    const input = event.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) return;
    const buf = await file.arrayBuffer();
    update("styleReference", Array.from(new Uint8Array(buf)));
  }

  function onSubmit(e: Event): void {
    e.preventDefault();
    const v = validation();
    if (!v.ok) {
      pushToast({ title: v.reason, kind: "error" });
      return;
    }
    setSubmitting(true);
    const inputs = toInputs(state());

    verbInvoke({ verb_id: ANIMATED_SPRITE_SHEET_VERB_ID, inputs })
      .then(() => {
        pushToast({
          title: "Animated sprite sheet: completed",
          kind: "info",
        });
        closeAnimatedSpriteSheetForm();
        props.onSubmitted?.();
      })
      .catch((err: unknown) => reportCommandFailure("animated_sprite_sheet", err))
      .finally(() => setSubmitting(false));
  }

  return (
    <form class="animated-sprite-sheet-form" onSubmit={onSubmit}>
      <header class="form-header">
        <h2>Animated sprite sheet</h2>
        <p class="form-help">
          Generate a g×g grid of animation frames from a single prompt. Cells read left-to-right,
          top-to-bottom and the last cell loops back to the first.
        </p>
      </header>

      <fieldset>
        <label for="asf-prompt">Concept</label>
        <textarea
          id="asf-prompt"
          rows="3"
          required
          placeholder="baby dragon, pastel pixel art, isometric action RPG"
          value={state().prompt}
          onInput={(e) => update("prompt", e.currentTarget.value)}
        />
      </fieldset>

      <fieldset class="grid-mode-row">
        <label>
          <span>Grid size</span>
          <input
            type="range"
            min={MIN_GRID_SIZE}
            max={MAX_GRID_SIZE}
            step="1"
            value={state().gridSize}
            onInput={(e) => onGridChange(Number(e.currentTarget.value))}
          />
          <output>
            {state().gridSize}×{state().gridSize} = {state().gridSize * state().gridSize} frames
          </output>
        </label>

        <label>
          <span>Mode</span>
          <div class="mode-buttons">
            <For each={MODE_LABELS}>
              {(opt) => (
                <button
                  type="button"
                  class="mode-button"
                  classList={{ active: state().mode === opt.mode }}
                  title={opt.hint}
                  onClick={() => update("mode", opt.mode)}
                >
                  {opt.label}
                </button>
              )}
            </For>
          </div>
        </label>
      </fieldset>

      <fieldset>
        <label>
          <span>
            Actions ({state().actions.length} / {state().gridSize})
          </span>
          {/* keyed so the CSS flash animation replays from scratch when
              a chip click is rejected. */}
          <p class="constraint-hint flash" data-flash={flashKey()} role="status" aria-live="polite">
            Pick up to {state().gridSize} actions — one per row.
          </p>
        </label>
        <div class="chip-grid">
          <For each={ACTION_PRESETS}>
            {(chip) => (
              <button
                type="button"
                class="chip"
                classList={{ selected: state().actions.includes(chip) }}
                onClick={() => onChipClick(chip)}
              >
                {chip}
              </button>
            )}
          </For>
        </div>
        <Show when={state().actions.length > 0}>
          <ol class="action-order">
            <For each={state().actions}>{(a) => <li>{a}</li>}</For>
          </ol>
        </Show>
      </fieldset>

      <details class="advanced">
        <summary>Advanced</summary>

        <fieldset>
          <label>
            <span>Cell size (px)</span>
            <input
              type="number"
              min={MIN_CELL_SIZE_PX}
              max={MAX_CELL_SIZE_PX}
              step="1"
              value={state().cellSizePx}
              onInput={(e) => {
                const n = Number(e.currentTarget.value) || DEFAULT_CELL_SIZE_PX;
                update("cellSizePx", n);
              }}
            />
          </label>
          <label>
            <span>Frame duration (ms)</span>
            <input
              type="number"
              min="1"
              step="1"
              value={state().frameDurationMs}
              onInput={(e) => {
                const n = Number(e.currentTarget.value) || DEFAULT_FRAME_DURATION_MS;
                update("frameDurationMs", n);
              }}
            />
          </label>
          <label>
            <span>Layer name</span>
            <input
              type="text"
              value={state().layerName}
              placeholder="Animated sheet"
              onInput={(e) => update("layerName", e.currentTarget.value)}
            />
          </label>
          <label>
            <span>Seed (optional)</span>
            <input
              type="text"
              inputmode="numeric"
              pattern="[0-9]*"
              value={state().seed}
              onInput={(e) => update("seed", e.currentTarget.value)}
            />
          </label>
          <label>
            <span>Reference image</span>
            <input type="file" accept="image/*" onChange={onReferenceFile} />
          </label>
        </fieldset>

        <fieldset>
          <label>
            <span>Rewritten prompt (CHARACTER × CHOREOGRAPHY)</span>
            <textarea
              rows="4"
              value={state().rewrittenPrompt}
              placeholder="Leave blank to run the LLM rewrite. Paste a hand-edited rewrite to skip it."
              onInput={(e) => update("rewrittenPrompt", e.currentTarget.value)}
            />
          </label>
        </fieldset>
      </details>

      <footer class="form-actions">
        <button type="button" class="ghost" onClick={() => closeAnimatedSpriteSheetForm()}>
          Cancel
        </button>
        <button type="submit" disabled={submitting() || !validation().ok}>
          {submitting() ? "Generating…" : "Generate"}
        </button>
      </footer>
    </form>
  );
};

export default AnimatedSpriteSheetForm;
