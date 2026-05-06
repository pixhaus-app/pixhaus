// DOM/SVG overlays drawn on top of the WebGL2 canvas.
//
// Two reasons these don't live in a shader pass:
//   - they're hit-testable (handles will accept pointer events in S16);
//   - they're cheap, infrequently redrawn UI chrome whose crispness benefits
//     from native vector rendering.
//
// Both overlays receive viewport state as props and convert canvas coords to
// CSS pixels via canvasToScreen.

import { For, Show, type Component } from "solid-js";
import { canvasToScreen } from "./viewport";

interface ViewportProps {
  scrollX: number;
  scrollY: number;
  zoom: number;
  /** Viewport CSS dimensions. */
  vpW: number;
  vpH: number;
}

// ── Brush cursor overlay ───────────────────────────────────────────────────

interface BrushCursorProps extends ViewportProps {
  /** Cursor in canvas coordinates. */
  cursor: { x: number; y: number } | null;
  /** Brush diameter in canvas pixels. */
  size: number;
  shape: "pixel" | "circle" | "square";
}

/**
 * Outlines the brush footprint at the pointer, snapped to the canvas pixel
 * grid.  S15 will own brush state; this overlay is the visualisation half so
 * the renderer feels responsive ahead of that stream.
 */
export const BrushCursor: Component<BrushCursorProps> = (props) => {
  return (
    <Show when={props.cursor}>
      {(cursor) => {
        // Snap to canvas pixel grid.  The brush is centred on the cursor and
        // odd sizes need a half-pixel offset so the centre lands on a pixel.
        const half = props.size / 2;
        const cx = Math.floor(cursor().x);
        const cy = Math.floor(cursor().y);
        const x0 = cx - Math.floor(half);
        const y0 = cy - Math.floor(half);
        const x1 = x0 + props.size;
        const y1 = y0 + props.size;

        const [sx0, sy0] = canvasToScreen(
          x0,
          y0,
          props.scrollX,
          props.scrollY,
          props.zoom,
          props.vpW,
          props.vpH,
        );
        const [sx1, sy1] = canvasToScreen(
          x1,
          y1,
          props.scrollX,
          props.scrollY,
          props.zoom,
          props.vpW,
          props.vpH,
        );
        const w = sx1 - sx0;
        const h = sy1 - sy0;

        return (
          <svg
            class="canvas-overlay brush-cursor-overlay"
            width={props.vpW}
            height={props.vpH}
            viewBox={`0 0 ${props.vpW} ${props.vpH}`}
            aria-hidden="true"
          >
            {props.shape === "circle" ? (
              <ellipse
                cx={sx0 + w / 2}
                cy={sy0 + h / 2}
                rx={w / 2}
                ry={h / 2}
                fill="none"
                stroke="white"
                stroke-width="1"
                stroke-dasharray="2 2"
              />
            ) : (
              <rect
                x={sx0}
                y={sy0}
                width={Math.max(w, 1)}
                height={Math.max(h, 1)}
                fill="none"
                stroke="white"
                stroke-width="1"
                stroke-dasharray="2 2"
              />
            )}
          </svg>
        );
      }}
    </Show>
  );
};

// ── Transform handles overlay ──────────────────────────────────────────────

interface TransformHandlesProps extends ViewportProps {
  /** Selection or transform-target bounding box in canvas coordinates. */
  bounds: { x: number; y: number; width: number; height: number } | null;
}

/**
 * Eight resize handles plus one rotation handle, drawn at the corners and
 * edge midpoints of `bounds`.  Hit-testing logic belongs to S16; for now the
 * overlay is purely visual so the rest of the viewport can be exercised.
 */
export const TransformHandles: Component<TransformHandlesProps> = (props) => {
  return (
    <Show when={props.bounds}>
      {(bounds) => {
        const [sx0, sy0] = canvasToScreen(
          bounds().x,
          bounds().y,
          props.scrollX,
          props.scrollY,
          props.zoom,
          props.vpW,
          props.vpH,
        );
        const [sx1, sy1] = canvasToScreen(
          bounds().x + bounds().width,
          bounds().y + bounds().height,
          props.scrollX,
          props.scrollY,
          props.zoom,
          props.vpW,
          props.vpH,
        );
        const cxMid = (sx0 + sx1) / 2;
        const cyMid = (sy0 + sy1) / 2;
        const handleSize = 8;
        const rotateOffset = 24; // CSS px above the bounding box

        const handles: Array<{ cx: number; cy: number; cursor: string }> = [
          { cx: sx0, cy: sy0, cursor: "nwse-resize" },
          { cx: cxMid, cy: sy0, cursor: "ns-resize" },
          { cx: sx1, cy: sy0, cursor: "nesw-resize" },
          { cx: sx1, cy: cyMid, cursor: "ew-resize" },
          { cx: sx1, cy: sy1, cursor: "nwse-resize" },
          { cx: cxMid, cy: sy1, cursor: "ns-resize" },
          { cx: sx0, cy: sy1, cursor: "nesw-resize" },
          { cx: sx0, cy: cyMid, cursor: "ew-resize" },
        ];

        return (
          <svg
            class="canvas-overlay transform-handles-overlay"
            width={props.vpW}
            height={props.vpH}
            viewBox={`0 0 ${props.vpW} ${props.vpH}`}
            aria-hidden="true"
          >
            <rect
              x={sx0}
              y={sy0}
              width={sx1 - sx0}
              height={sy1 - sy0}
              fill="none"
              stroke="#7aa2ff"
              stroke-width="1"
              stroke-dasharray="3 3"
            />
            {/* Tether between rotation handle and bounding box. */}
            <line
              x1={cxMid}
              y1={sy0}
              x2={cxMid}
              y2={sy0 - rotateOffset}
              stroke="#7aa2ff"
              stroke-width="1"
            />
            <circle
              cx={cxMid}
              cy={sy0 - rotateOffset}
              r={handleSize / 2}
              fill="#0f0f13"
              stroke="#7aa2ff"
              stroke-width="1.5"
              data-handle="rotate"
              style={{ cursor: "alias" }}
            />
            <For each={handles}>
              {(h) => (
                <rect
                  x={h.cx - handleSize / 2}
                  y={h.cy - handleSize / 2}
                  width={handleSize}
                  height={handleSize}
                  fill="#0f0f13"
                  stroke="#7aa2ff"
                  stroke-width="1.5"
                  data-handle="resize"
                  style={{ cursor: h.cursor }}
                />
              )}
            </For>
          </svg>
        );
      }}
    </Show>
  );
};
