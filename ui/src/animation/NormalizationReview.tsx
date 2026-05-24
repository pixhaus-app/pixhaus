// Normalization review: contact sheet + GIF-style loop check before landing.
//
// Surfaces the seven-step normalization result so the user sees baseline lock
// and scale correction (pipeline doc). Integrate commits the normalized frames
// to the timeline.

import { type Component, For, Show } from "solid-js";

import type { LoopDirection } from "../lib/types/LoopDirection";
import type { NormalizeReport } from "../lib/types/NormalizeReport";
import type { RgbaFrame } from "../lib/commands/animation";
import FramePreview from "./FramePreview";

type Props = {
  frames: RgbaFrame[];
  report: NormalizeReport | null;
  loopDirection: LoopDirection;
  fps: number;
  onReNormalize: () => void;
  onIntegrate: () => void;
};

/** A small static thumbnail of one frame, drawn to a canvas. */
const ContactCell: Component<{ frame: RgbaFrame }> = (props) => {
  let canvas: HTMLCanvasElement | undefined;
  const draw = (el: HTMLCanvasElement): void => {
    canvas = el;
    el.width = props.frame.width;
    el.height = props.frame.height;
    const ctx = el.getContext("2d");
    if (ctx === null) return;
    const data = new Uint8ClampedArray(props.frame.pixels);
    if (data.length === props.frame.width * props.frame.height * 4) {
      ctx.putImageData(new ImageData(data, props.frame.width, props.frame.height), 0, 0);
    }
  };
  return (
    <canvas
      ref={(el) => draw(el)}
      class="animation-studio__contact-cell"
      style={{ "image-rendering": "pixelated" }}
      data-canvas={canvas !== undefined}
    />
  );
};

const NormalizationReview: Component<Props> = (props) => {
  return (
    <div class="animation-studio__normalize" data-testid="normalization-review">
      <header class="animation-studio__normalize-head">Normalize</header>

      <div class="animation-studio__contact-sheet">
        <For each={props.frames}>{(frame) => <ContactCell frame={frame} />}</For>
      </div>

      <div class="animation-studio__normalize-preview">
        <FramePreview
          frames={props.frames}
          fps={props.fps}
          playing={true}
          loopDirection={props.loopDirection}
          scale={3}
        />
      </div>

      <Show when={props.report}>
        {(report) => (
          <div class="animation-studio__normalize-meta">
            <span>
              baseline:{" "}
              {report().baseline_drift_px === 0
                ? "locked"
                : `${report().baseline_drift_px}px drift`}
            </span>
            <span>
              scale: {report().scale_match_pct >= 90 ? "matched" : `${report().scale_match_pct}%`}
            </span>
            <span>seam: {report().seam}</span>
            <Show when={(report().warnings ?? []).length > 0}>
              <ul class="animation-studio__warnings">
                <For each={report().warnings ?? []}>{(w) => <li>{w}</li>}</For>
              </ul>
            </Show>
          </div>
        )}
      </Show>

      <div class="animation-studio__normalize-actions">
        <button class="animation-studio__btn" onClick={() => props.onReNormalize()}>
          Re-normalize
        </button>
        <button
          class="animation-studio__btn animation-studio__btn--accept"
          onClick={() => props.onIntegrate()}
          data-testid="normalize-integrate"
        >
          Integrate
        </button>
      </div>
    </div>
  );
};

export default NormalizationReview;
