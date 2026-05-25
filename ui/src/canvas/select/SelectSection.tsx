// Selection-tool options as a right-rail section body.
//
// Renders controls for the currently active select tool. Today only the
// magic wand surfaces options — tolerance, connectivity, and the S57
// gap-close pre-pass. Auto-expanded by the rail when select.selectTool ===
// "wand" (see rail-state).

import { Show, type Component } from "solid-js";
import {
  select,
  setWandTolerance,
  setWandConnectivity,
  setWandGapClose,
  setWandGapDistance,
} from "./select-state";

const SelectSection: Component = () => (
  <div class="tool-options-group" data-testid="tool-options-select">
    <Show
      when={select.selectTool === "wand"}
      fallback={
        <p class="tool-options-empty" data-testid="select-section-empty">
          Choose the magic wand to configure selection options.
        </p>
      }
    >
      <label class="tool-option-label">
        Tolerance
        <input
          type="range"
          min="0"
          max="255"
          value={select.wandTolerance}
          onInput={(e) => setWandTolerance(Number(e.currentTarget.value))}
          class="tool-option-range"
          data-testid="wand-option-tolerance"
        />
        <span class="tool-option-value">{select.wandTolerance}</span>
      </label>
      <fieldset class="tool-option-fieldset">
        <legend>Connectivity</legend>
        <label class="tool-option-radio">
          <input
            type="radio"
            name="wand-connectivity"
            value="four"
            checked={select.wandConnectivity === "four"}
            onChange={() => setWandConnectivity("four")}
            data-testid="wand-option-connectivity-four"
          />
          4-connected
        </label>
        <label class="tool-option-radio">
          <input
            type="radio"
            name="wand-connectivity"
            value="eight"
            checked={select.wandConnectivity === "eight"}
            onChange={() => setWandConnectivity("eight")}
            data-testid="wand-option-connectivity-eight"
          />
          8-connected
        </label>
      </fieldset>
      <label class="tool-option-checkbox">
        <input
          type="checkbox"
          checked={select.wandGapClose}
          onChange={(e) => setWandGapClose(e.currentTarget.checked)}
          data-testid="wand-option-gap-close"
        />
        Auto-close gaps
      </label>
      <Show when={select.wandGapClose}>
        <label class="tool-option-label">
          Max gap
          <input
            type="range"
            min="1"
            max="64"
            value={select.wandGapDistance}
            onInput={(e) => setWandGapDistance(Number(e.currentTarget.value))}
            class="tool-option-range"
            data-testid="wand-option-gap-distance"
          />
          <span class="tool-option-value">{select.wandGapDistance}</span>
        </label>
      </Show>
    </Show>
  </div>
);

export default SelectSection;
