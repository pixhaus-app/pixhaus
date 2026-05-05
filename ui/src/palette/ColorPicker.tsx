// Slider-based color picker supporting HSV, HSL, RGB, HEX, and OKLCH modes.
//
// `onChange` fires on every slider tick (for live preview).
// `onCommit` fires when the user releases the slider or blurs an input
// (the caller uses this to send the final value to the Rust side).

import { createSignal, createMemo, For, Show, type Component } from "solid-js";
import type { Rgba } from "../lib/types";
import {
  rgbToHsv,
  hsvToRgb,
  rgbToHsl,
  hslToRgb,
  rgbToHex,
  hexToRgb,
  rgbToOklch,
  oklchToRgb,
  rgbaToCss,
} from "./color-utils";

type Mode = "hsv" | "hsl" | "rgb" | "hex" | "oklch";

const MODES: Mode[] = ["hsv", "hsl", "rgb", "hex", "oklch"];

type Props = {
  color: Rgba;
  onChange: (color: Rgba) => void;
  onCommit: (color: Rgba) => void;
};

const ColorPicker: Component<Props> = (props) => {
  const [mode, setMode] = createSignal<Mode>("hsv");
  const [hexDraft, setHexDraft] = createSignal("");
  const [hexEditing, setHexEditing] = createSignal(false);

  const hsv = createMemo(() => rgbToHsv(props.color.r, props.color.g, props.color.b));
  const hsl = createMemo(() => rgbToHsl(props.color.r, props.color.g, props.color.b));
  const lch = createMemo(() => rgbToOklch(props.color.r, props.color.g, props.color.b));
  const hexStr = createMemo(() => rgbToHex(props.color.r, props.color.g, props.color.b));

  const mergeAlpha = (rgb: { r: number; g: number; b: number }): Rgba => ({
    ...rgb,
    a: props.color.a,
  });

  const handleHsvChange = (key: "h" | "s" | "v", val: number) => {
    const { h, s, v } = hsv();
    props.onChange(
      mergeAlpha(hsvToRgb(key === "h" ? val : h, key === "s" ? val : s, key === "v" ? val : v)),
    );
  };

  const handleHslChange = (key: "h" | "s" | "l", val: number) => {
    const { h, s, l } = hsl();
    props.onChange(
      mergeAlpha(hslToRgb(key === "h" ? val : h, key === "s" ? val : s, key === "l" ? val : l)),
    );
  };

  const handleLchChange = (key: "L" | "C" | "H", val: number) => {
    const { L, C, H } = lch();
    props.onChange(
      mergeAlpha(oklchToRgb(key === "L" ? val : L, key === "C" ? val : C, key === "H" ? val : H)),
    );
  };

  const handleHexCommit = () => {
    const parsed = hexToRgb(hexDraft());
    if (parsed) {
      const next: Rgba = { ...parsed, a: props.color.a };
      props.onChange(next);
      props.onCommit(next);
    }
    setHexEditing(false);
  };

  // Static hue gradient shared by all hue sliders.
  const hueGrad = "linear-gradient(to right,#f00,#ff0,#0f0,#0ff,#00f,#f0f,#f00)";

  return (
    <div class="cpk">
      {/* Mode tabs */}
      <div class="cpk__tabs" role="tablist" aria-label="Color mode">
        <For each={MODES}>
          {(m) => (
            <button
              role="tab"
              aria-selected={mode() === m}
              class="cpk__tab"
              onClick={() => setMode(m)}
            >
              {m.toUpperCase()}
            </button>
          )}
        </For>
      </div>

      {/* Preview swatch */}
      <div class="cpk__preview-row">
        <div class="cpk__preview-checker" aria-hidden="true" />
        <div class="cpk__preview" style={{ background: rgbaToCss(props.color) }} title={hexStr()} />
        <span class="cpk__hex-badge">{hexStr()}</span>
      </div>

      {/* HSV sliders */}
      <Show when={mode() === "hsv"}>
        <div class="cpk__sliders">
          <CpkSlider
            label="H"
            value={Math.round(hsv().h)}
            min={0}
            max={360}
            step={1}
            trackStyle={hueGrad}
            onChange={(v) => handleHsvChange("h", v)}
            onCommit={() => props.onCommit(props.color)}
          />
          <CpkSlider
            label="S"
            value={Math.round(hsv().s * 100)}
            min={0}
            max={100}
            step={1}
            onChange={(v) => handleHsvChange("s", v / 100)}
            onCommit={() => props.onCommit(props.color)}
          />
          <CpkSlider
            label="V"
            value={Math.round(hsv().v * 100)}
            min={0}
            max={100}
            step={1}
            onChange={(v) => handleHsvChange("v", v / 100)}
            onCommit={() => props.onCommit(props.color)}
          />
        </div>
      </Show>

      {/* HSL sliders */}
      <Show when={mode() === "hsl"}>
        <div class="cpk__sliders">
          <CpkSlider
            label="H"
            value={Math.round(hsl().h)}
            min={0}
            max={360}
            step={1}
            trackStyle={hueGrad}
            onChange={(v) => handleHslChange("h", v)}
            onCommit={() => props.onCommit(props.color)}
          />
          <CpkSlider
            label="S"
            value={Math.round(hsl().s * 100)}
            min={0}
            max={100}
            step={1}
            onChange={(v) => handleHslChange("s", v / 100)}
            onCommit={() => props.onCommit(props.color)}
          />
          <CpkSlider
            label="L"
            value={Math.round(hsl().l * 100)}
            min={0}
            max={100}
            step={1}
            onChange={(v) => handleHslChange("l", v / 100)}
            onCommit={() => props.onCommit(props.color)}
          />
        </div>
      </Show>

      {/* RGB sliders */}
      <Show when={mode() === "rgb"}>
        <div class="cpk__sliders">
          <CpkSlider
            label="R"
            value={props.color.r}
            min={0}
            max={255}
            step={1}
            onChange={(v) => props.onChange({ ...props.color, r: v })}
            onCommit={() => props.onCommit(props.color)}
          />
          <CpkSlider
            label="G"
            value={props.color.g}
            min={0}
            max={255}
            step={1}
            onChange={(v) => props.onChange({ ...props.color, g: v })}
            onCommit={() => props.onCommit(props.color)}
          />
          <CpkSlider
            label="B"
            value={props.color.b}
            min={0}
            max={255}
            step={1}
            onChange={(v) => props.onChange({ ...props.color, b: v })}
            onCommit={() => props.onCommit(props.color)}
          />
        </div>
      </Show>

      {/* HEX input */}
      <Show when={mode() === "hex"}>
        <div class="cpk__hex-row">
          <input
            class="cpk__hex-input"
            type="text"
            value={hexEditing() ? hexDraft() : hexStr()}
            onFocus={() => {
              setHexEditing(true);
              setHexDraft(hexStr());
            }}
            onInput={(e) => setHexDraft(e.currentTarget.value)}
            onBlur={handleHexCommit}
            onKeyDown={(e) => {
              if (e.key === "Enter") handleHexCommit();
            }}
            spellcheck={false}
            maxlength={7}
          />
        </div>
      </Show>

      {/* OKLCH sliders */}
      <Show when={mode() === "oklch"}>
        <div class="cpk__sliders">
          <CpkSlider
            label="L"
            value={Math.round(lch().L * 100)}
            min={0}
            max={100}
            step={1}
            onChange={(v) => handleLchChange("L", v / 100)}
            onCommit={() => props.onCommit(props.color)}
          />
          <CpkSlider
            label="C"
            value={parseFloat((lch().C * 100).toFixed(1))}
            min={0}
            max={40}
            step={0.1}
            onChange={(v) => handleLchChange("C", v / 100)}
            onCommit={() => props.onCommit(props.color)}
          />
          <CpkSlider
            label="H"
            value={Math.round(lch().H)}
            min={0}
            max={360}
            step={1}
            trackStyle={hueGrad}
            onChange={(v) => handleLchChange("H", v)}
            onCommit={() => props.onCommit(props.color)}
          />
        </div>
      </Show>

      {/* Alpha slider — always visible */}
      <div class="cpk__alpha-row">
        <CpkSlider
          label="A"
          value={props.color.a}
          min={0}
          max={255}
          step={1}
          onChange={(v) => props.onChange({ ...props.color, a: v })}
          onCommit={() => props.onCommit(props.color)}
        />
      </div>
    </div>
  );
};

// ── CpkSlider ─────────────────────────────────────────────────────────────────

type SliderProps = {
  label: string;
  value: number;
  min: number;
  max: number;
  step: number;
  trackStyle?: string;
  onChange: (value: number) => void;
  onCommit: () => void;
};

const CpkSlider: Component<SliderProps> = (props) => {
  // Local draft while the number input is focused; null otherwise.
  const [numDraft, setNumDraft] = createSignal<string | null>(null);

  const commitNum = (raw: string) => {
    const v = parseFloat(raw);
    if (!isNaN(v)) {
      props.onChange(Math.max(props.min, Math.min(props.max, v)));
      props.onCommit();
    }
    setNumDraft(null);
  };

  return (
    <div class="cpk__slider-row">
      <span class="cpk__slider-label">{props.label}</span>
      <input
        class="cpk__slider"
        type="range"
        min={props.min}
        max={props.max}
        step={props.step}
        value={props.value}
        style={props.trackStyle ? { background: props.trackStyle } : undefined}
        onInput={(e) => props.onChange(parseFloat(e.currentTarget.value))}
        onChange={props.onCommit}
      />
      <input
        class="cpk__slider-num"
        type="number"
        min={props.min}
        max={props.max}
        step={props.step}
        value={numDraft() ?? String(props.value)}
        onFocus={() => setNumDraft(String(props.value))}
        onInput={(e) => setNumDraft(e.currentTarget.value)}
        onBlur={(e) => commitNum(e.currentTarget.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter") commitNum((e.currentTarget as HTMLInputElement).value);
        }}
      />
    </div>
  );
};

export default ColorPicker;
