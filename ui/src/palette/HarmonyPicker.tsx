// Harmony color suggestions for the active foreground swatch.
//
// Computes complementary, split-complement, triad, tetrad, and analogous
// relationships via HSV rotation.  Clicking a suggestion adds it to the
// palette.

import { createSignal, For, type Component } from "solid-js";
import type { Rgba } from "../lib/types";
import { activePalette, foregroundIndex } from "./palette-panel-state";
import { harmonyColors, rgbaToCss, rgbToHex, contrastColor, type HarmonyKind } from "./color-utils";

type Props = {
  /** Called when the user clicks a suggested color to append it to the palette. */
  onAddColor: (color: Rgba) => void;
};

const KINDS: Array<{ id: HarmonyKind; label: string }> = [
  { id: "complement", label: "Complement" },
  { id: "split-complement", label: "Split" },
  { id: "triad", label: "Triad" },
  { id: "tetrad", label: "Tetrad" },
  { id: "analogous", label: "Analogous" },
];

const HarmonyPicker: Component<Props> = (props) => {
  const [kind, setKind] = createSignal<HarmonyKind>("complement");

  const fgColor = (): Rgba | null => {
    const p = activePalette();
    return p?.colors[foregroundIndex()]?.color ?? null;
  };

  const suggestions = (): Rgba[] => {
    const c = fgColor();
    if (!c) return [];
    return harmonyColors(c.r, c.g, c.b, kind());
  };

  return (
    <div class="harmony">
      {/* Kind selector */}
      <div class="harmony__tabs" role="tablist" aria-label="Harmony type">
        <For each={KINDS}>
          {({ id, label }) => (
            <button
              role="tab"
              class="harmony__tab"
              aria-selected={kind() === id}
              onClick={() => setKind(id)}
            >
              {label}
            </button>
          )}
        </For>
      </div>

      {/* Source swatch + suggestions */}
      <div class="harmony__swatches">
        {/* Source color reference */}
        {fgColor() && (
          <div
            class="harmony__swatch harmony__swatch--source"
            style={{ background: rgbaToCss(fgColor()!) }}
            title="Active foreground color (source)"
            aria-label="Source color"
          />
        )}

        {/* Harmony suggestions */}
        <For each={suggestions()}>
          {(color) => (
            <button
              class="harmony__swatch harmony__swatch--suggestion"
              style={{ background: rgbaToCss(color) }}
              title={`Add to palette: ${rgbToHex(color.r, color.g, color.b)}`}
              aria-label={`Add harmony color ${rgbToHex(color.r, color.g, color.b)}`}
              onClick={() => props.onAddColor(color)}
            >
              <span
                class="harmony__add-icon"
                style={{ color: contrastColor(color) }}
                aria-hidden="true"
              >
                +
              </span>
            </button>
          )}
        </For>
      </div>
    </div>
  );
};

export default HarmonyPicker;
