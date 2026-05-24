// Animated loop preview.
//
// Draws a sequence of raw RGBA frames to a canvas and advances them at the
// target fps, honouring the loop direction's play order (ping-pong for idle,
// forward for walk). Optionally overlays the foot-baseline line. Used by the
// candidate review, the frame picker, and the centre preview.

import { type Component, createEffect, createSignal, onCleanup } from "solid-js";

import type { LoopDirection } from "../lib/types/LoopDirection";
import type { RgbaFrame } from "../lib/commands/animation";

type Props = {
  frames: RgbaFrame[];
  fps: number;
  playing: boolean;
  loopDirection: LoopDirection;
  /** Draw a horizontal baseline guide at this fraction of height (0..1). */
  baselineFraction?: number;
  /** Integer upscale factor for the preview. Defaults to fit-ish 4x. */
  scale?: number;
  /**
   * Fit the frame to its container (preserving aspect) instead of a fixed
   * upscale. Use for raw i2v frames, which are hundreds of pixels wide and
   * overflow a fixed scale.
   */
  fit?: boolean;
};

/** Expands `[0, n)` into the order frames play for `dir` (no duplicate ends). */
function playOrder(n: number, dir: LoopDirection): number[] {
  if (n <= 1) return n === 1 ? [0] : [];
  const fwd = Array.from({ length: n }, (_, i) => i);
  switch (dir) {
    case "forward":
      return fwd;
    case "reverse":
      return [...fwd].reverse();
    case "ping_pong": {
      const tail = [];
      for (let i = n - 2; i >= 1; i -= 1) tail.push(i);
      return [...fwd, ...tail];
    }
    case "ping_pong_reverse": {
      const rev = [...fwd].reverse();
      const tail = [];
      for (let i = 1; i <= n - 2; i += 1) tail.push(i);
      return [...rev, ...tail];
    }
  }
}

const FramePreview: Component<Props> = (props) => {
  let canvas: HTMLCanvasElement | undefined;
  const [step, setStep] = createSignal(0);

  // Advance the play head at the target fps while playing.
  createEffect(() => {
    if (!props.playing || props.frames.length <= 1) return;
    const order = playOrder(props.frames.length, props.loopDirection);
    const interval = Math.max(16, Math.round(1000 / Math.max(1, props.fps)));
    const timer = setInterval(() => {
      setStep((s) => (s + 1) % order.length);
    }, interval);
    onCleanup(() => clearInterval(timer));
  });

  // Draw the current frame whenever the step, frames, or scale change.
  createEffect(() => {
    const frames = props.frames;
    if (canvas === undefined || frames.length === 0) return;
    const order = playOrder(frames.length, props.loopDirection);
    const idx = order[step() % order.length] ?? 0;
    const frame = frames[idx];
    if (frame === undefined) return;
    canvas.width = frame.width;
    canvas.height = frame.height;
    const ctx = canvas.getContext("2d");
    if (ctx === null) return;
    ctx.clearRect(0, 0, frame.width, frame.height);
    const data = new Uint8ClampedArray(frame.pixels);
    if (data.length === frame.width * frame.height * 4) {
      ctx.putImageData(new ImageData(data, frame.width, frame.height), 0, 0);
    }
    const bf = props.baselineFraction;
    if (bf !== undefined) {
      ctx.strokeStyle = "rgba(120, 200, 255, 0.7)";
      ctx.lineWidth = 1;
      const y = Math.round(frame.height * bf) + 0.5;
      ctx.beginPath();
      ctx.moveTo(0, y);
      ctx.lineTo(frame.width, y);
      ctx.stroke();
    }
  });

  return (
    <canvas
      ref={canvas}
      class="animation-studio__preview-canvas"
      style={
        props.fit === true
          ? {
              "image-rendering": "pixelated",
              "max-width": "100%",
              "max-height": "100%",
              width: "auto",
              height: "auto",
              "object-fit": "contain",
            }
          : {
              "image-rendering": "pixelated",
              transform: `scale(${props.scale ?? 4})`,
              "transform-origin": "center",
            }
      }
      data-testid="animation-preview-canvas"
    />
  );
};

export default FramePreview;
