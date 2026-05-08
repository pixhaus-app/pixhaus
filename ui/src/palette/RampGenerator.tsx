// Color ramp generator.
//
// Picks two existing swatches from the active palette and interpolates
// N steps in Oklab (perceptually uniform).  On confirm, each intermediate
// color is appended to the palette via the palette_add_color command.

import { createSignal, For, type Component } from "solid-js";
import type { Rgba } from "../lib/types";
import { activePalette } from "./palette-panel-state";
import { interpolateOklab, rgbaToCss } from "./color-utils";

type Props = {
  /** Called once per generated swatch so the parent can call palette_add_color. */
  onAddColor: (color: Rgba) => Promise<unknown>;
};

const RampGenerator: Component<Props> = (props) => {
  const [fromIndex, setFromIndex] = createSignal(0);
  const [toIndex, setToIndex] = createSignal(1);
  const [steps, setSteps] = createSignal(5);
  const [busy, setBusy] = createSignal(false);

  const palette = () => activePalette();

  const colorAt = (index: number): Rgba | null => palette()?.colors[index]?.color ?? null;

  const preview = (): Rgba[] => {
    const a = colorAt(fromIndex());
    const b = colorAt(toIndex());
    if (!a || !b) return [];
    const n = steps();
    return Array.from({ length: n }, (_, i) => interpolateOklab(a, b, i / (n - 1)));
  };

  const handleGenerate = async () => {
    const colors = preview();
    if (colors.length === 0) return;
    setBusy(true);
    try {
      // Skip the first and last — they're the existing from/to swatches.
      for (const color of colors.slice(1, -1)) {
        await props.onAddColor(color);
      }
    } finally {
      setBusy(false);
    }
  };

  const maxIndex = () => (palette()?.colors.length ?? 1) - 1;

  return (
    <div class="ramp">
      {/* From / to pickers */}
      <div class="ramp__controls">
        <label class="ramp__label">
          From
          <input
            class="ramp__index"
            type="number"
            min={0}
            max={maxIndex()}
            value={fromIndex()}
            onInput={(e) =>
              setFromIndex(
                Math.max(0, Math.min(maxIndex(), parseInt(e.currentTarget.value, 10) || 0)),
              )
            }
          />
          {colorAt(fromIndex()) && (
            <span
              class="ramp__dot"
              style={{ background: rgbaToCss(colorAt(fromIndex())!) }}
              aria-hidden="true"
            />
          )}
        </label>

        <label class="ramp__label">
          To
          <input
            class="ramp__index"
            type="number"
            min={0}
            max={maxIndex()}
            value={toIndex()}
            onInput={(e) =>
              setToIndex(
                Math.max(0, Math.min(maxIndex(), parseInt(e.currentTarget.value, 10) || 0)),
              )
            }
          />
          {colorAt(toIndex()) && (
            <span
              class="ramp__dot"
              style={{ background: rgbaToCss(colorAt(toIndex())!) }}
              aria-hidden="true"
            />
          )}
        </label>

        <label class="ramp__label">
          Steps
          <input
            class="ramp__index"
            type="number"
            min={3}
            max={32}
            value={steps()}
            onInput={(e) =>
              setSteps(Math.max(3, Math.min(32, parseInt(e.currentTarget.value, 10) || 3)))
            }
          />
        </label>
      </div>

      {/* Preview strip */}
      <div class="ramp__preview" aria-label="Ramp preview" role="img">
        <For each={preview()}>
          {(c) => <div class="ramp__preview-swatch" style={{ background: rgbaToCss(c) }} />}
        </For>
      </div>

      <p class="ramp__note">
        Adds {Math.max(0, steps() - 2)} intermediate swatches to the palette. Interpolation is
        perceptual (Oklab).
      </p>

      <button class="ramp__btn" disabled={busy() || preview().length < 3} onClick={handleGenerate}>
        {busy() ? "Adding…" : "Add to palette"}
      </button>
    </div>
  );
};

export default RampGenerator;
