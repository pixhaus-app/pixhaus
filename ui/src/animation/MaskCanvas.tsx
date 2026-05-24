// Minimal inpaint mask painter.
//
// Shows the frame with a paint-over overlay; the painted region is what
// gpt-image-2 will edit. Exports an OpenAI-compatible mask the size of the
// frame: opaque everywhere, transparent where painted (transparent = edit).

import { type Component, createSignal, onMount } from "solid-js";

type Props = {
  /** Frame to paint over, as a base64 PNG. */
  src: string;
  /** Native frame width/height — the exported mask matches these. */
  width: number;
  height: number;
  /** Receives the mask exporter; returns base64 PNG, or null if nothing painted. */
  onReady: (exporter: () => string | null) => void;
};

const MaskCanvas: Component<Props> = (props) => {
  let canvas: HTMLCanvasElement | undefined;
  const [brush, setBrush] = createSignal(48);
  const [painted, setPainted] = createSignal(false);
  let drawing = false;

  function ctx(): CanvasRenderingContext2D | null {
    return canvas?.getContext("2d", { willReadFrequently: true }) ?? null;
  }

  // Maps a pointer event to native canvas coordinates (the canvas is CSS-fit).
  function toNative(e: PointerEvent): { x: number; y: number } | null {
    if (canvas === undefined) return null;
    const rect = canvas.getBoundingClientRect();
    if (rect.width === 0 || rect.height === 0) return null;
    return {
      x: ((e.clientX - rect.left) / rect.width) * props.width,
      y: ((e.clientY - rect.top) / rect.height) * props.height,
    };
  }

  function paintAt(e: PointerEvent): void {
    const c = ctx();
    const p = toNative(e);
    if (c === null || p === null) return;
    c.fillStyle = "rgba(255, 60, 60, 0.55)";
    c.beginPath();
    c.arc(p.x, p.y, brush(), 0, Math.PI * 2);
    c.fill();
    setPainted(true);
  }

  onMount(() => {
    const c = ctx();
    if (c !== null) c.clearRect(0, 0, props.width, props.height);
    // Hand the parent an exporter that converts the painted overlay into a
    // mask: opaque (kept) everywhere, transparent (edited) where painted. It
    // is called imperatively from a button handler, so reading signals at call
    // time is intentional — not a missed reactive subscription.
    // eslint-disable-next-line solid/reactivity
    props.onReady(() => {
      if (canvas === undefined || !painted()) return null;
      const src = ctx();
      if (src === null) return null;
      const data = src.getImageData(0, 0, props.width, props.height);
      const out = new ImageData(props.width, props.height);
      for (let i = 0; i < data.data.length; i += 4) {
        const paintedHere = (data.data[i + 3] ?? 0) > 0;
        out.data[i] = 0;
        out.data[i + 1] = 0;
        out.data[i + 2] = 0;
        out.data[i + 3] = paintedHere ? 0 : 255;
      }
      const off = document.createElement("canvas");
      off.width = props.width;
      off.height = props.height;
      const octx = off.getContext("2d");
      if (octx === null) return null;
      octx.putImageData(out, 0, 0);
      return off.toDataURL("image/png").replace(/^data:image\/png;base64,/, "");
    });
  });

  return (
    <div class="animation-studio__mask-editor" data-testid="mask-canvas">
      <div class="animation-studio__mask-stage">
        <img
          class="animation-studio__fit-image"
          src={`data:image/png;base64,${props.src}`}
          alt="frame to refine"
        />
        <canvas
          ref={canvas}
          class="animation-studio__mask-overlay"
          width={props.width}
          height={props.height}
          onPointerDown={(e) => {
            drawing = true;
            e.currentTarget.setPointerCapture(e.pointerId);
            paintAt(e);
          }}
          onPointerMove={(e) => drawing && paintAt(e)}
          onPointerUp={() => {
            drawing = false;
          }}
        />
      </div>
      <div class="animation-studio__mask-tools">
        <label class="animation-studio__field">
          brush: {brush()}
          <input
            type="range"
            min={8}
            max={128}
            value={brush()}
            onInput={(e) => setBrush(Number(e.currentTarget.value))}
          />
        </label>
        <button
          class="animation-studio__btn"
          onClick={() => {
            const c = ctx();
            if (c !== null) c.clearRect(0, 0, props.width, props.height);
            setPainted(false);
          }}
        >
          Clear mask
        </button>
      </div>
    </div>
  );
};

export default MaskCanvas;
