// Foreground / background color swatch pair.
//
// Renders as two overlapping swatches (foreground on top-left, background
// behind-right) matching the Photoshop / Aseprite convention.  Pressing X
// swaps the two; pressing D resets to index 0 / 1.  Clicking a swatch opens
// the color picker for that index.

import { type Component } from "solid-js";
import { paletteUi, activePalette, swapFgBg, resetFgBg, startEditing } from "./palette-panel-state";
import { rgbaToCss, contrastColor } from "./color-utils";

type Props = {
  /** Called when the user clicks a swatch — parent should open the color picker. */
  onPickerOpen: (index: number) => void;
};

const FgBgSwatches: Component<Props> = (props) => {
  const palette = () => activePalette();

  const fgEntry = () => {
    const p = palette();
    return p?.colors[paletteUi.foregroundIndex] ?? null;
  };

  const bgEntry = () => {
    const p = palette();
    return p?.colors[paletteUi.backgroundIndex] ?? null;
  };

  const fgCss = () => (fgEntry() ? rgbaToCss(fgEntry()!.color) : "transparent");
  const bgCss = () => (bgEntry() ? rgbaToCss(bgEntry()!.color) : "transparent");

  const handleFgClick = () => {
    const entry = fgEntry();
    if (entry) {
      startEditing(paletteUi.foregroundIndex, entry.color);
      props.onPickerOpen(paletteUi.foregroundIndex);
    }
  };

  const handleBgClick = () => {
    const entry = bgEntry();
    if (entry) {
      startEditing(paletteUi.backgroundIndex, entry.color);
      props.onPickerOpen(paletteUi.backgroundIndex);
    }
  };

  return (
    <div class="fgbg" role="group" aria-label="Foreground and background colors">
      {/* Background swatch (behind) */}
      <button
        class="fgbg__swatch fgbg__swatch--bg"
        style={{ background: bgCss() }}
        onClick={handleBgClick}
        title="Background color (click to edit)"
        aria-label="Background color"
      />

      {/* Foreground swatch (in front) */}
      <button
        class="fgbg__swatch fgbg__swatch--fg"
        style={{ background: fgCss() }}
        onClick={handleFgClick}
        title="Foreground color (click to edit)"
        aria-label="Foreground color"
      />

      {/* Swap button */}
      <button
        class="fgbg__action fgbg__action--swap"
        onClick={swapFgBg}
        title="Swap foreground / background (X)"
        aria-label="Swap foreground and background"
      >
        <SwapIcon />
      </button>

      {/* Reset button */}
      <button
        class="fgbg__action fgbg__action--reset"
        onClick={resetFgBg}
        title="Reset to defaults (D)"
        aria-label="Reset foreground and background to defaults"
      >
        <ResetIcon />
      </button>

      {/* Indexed-mode label */}
      <div class="fgbg__index-labels">
        <span
          class="fgbg__index-badge"
          style={{
            background: fgCss(),
            color: fgEntry() ? contrastColor(fgEntry()!.color) : "inherit",
          }}
        >
          {paletteUi.foregroundIndex}
        </span>
        <span
          class="fgbg__index-badge"
          style={{
            background: bgCss(),
            color: bgEntry() ? contrastColor(bgEntry()!.color) : "inherit",
          }}
        >
          {paletteUi.backgroundIndex}
        </span>
      </div>
    </div>
  );
};

// ── Minimal inline SVG icons ──────────────────────────────────────────────────

const SwapIcon: Component = () => (
  <svg width="10" height="10" viewBox="0 0 10 10" fill="currentColor" aria-hidden="true">
    <path
      d="M1 3h6L5 1m4 6H3l2 2"
      stroke="currentColor"
      stroke-width="1.2"
      fill="none"
      stroke-linecap="round"
      stroke-linejoin="round"
    />
  </svg>
);

const ResetIcon: Component = () => (
  <svg width="10" height="10" viewBox="0 0 10 10" fill="currentColor" aria-hidden="true">
    <rect x="1" y="1" width="5" height="5" fill="currentColor" />
    <rect
      x="4"
      y="4"
      width="5"
      height="5"
      fill="var(--bg-panel)"
      stroke="currentColor"
      stroke-width="0.5"
    />
    <rect x="4.5" y="4.5" width="4" height="4" fill="currentColor" opacity="0.3" />
  </svg>
);

export default FgBgSwatches;
