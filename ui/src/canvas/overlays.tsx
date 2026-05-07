// DOM/SVG overlays drawn on top of the WebGL2 canvas.
//
// Two reasons these don't live in a shader pass:
//   - they're hit-testable (handles accept pointer events for S16 transform);
//   - they're cheap, infrequently redrawn UI chrome whose crispness benefits
//     from native vector rendering.
//
// Both overlays receive viewport state as props and convert canvas coords to
// CSS pixels via canvasToScreen.

import { For, Show, createMemo, type Component } from "solid-js";
import { canvasToScreen } from "./viewport";
import type { ShapePreviewState } from "./canvas-state";
import type { TransformHandle } from "./transform/transform-state";

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
 * grid.
 *
 * Solid `<Show>` (non-keyed) runs its child function exactly once — so any
 * derived values computed inside the function body get frozen at first
 * render. The previous version captured `cx`/`cy`/`x0`/etc. as `const`s
 * inside the child function and the SVG attributes used those frozen
 * values, which is why the cursor preview appeared at the first mouse
 * position and never moved. Wrap every signal-derived value in
 * `createMemo` so each one tracks its own deps and the JSX re-reads them.
 */
export const BrushCursor: Component<BrushCursorProps> = (props) => {
  // Pixel-shape always paints a single canvas pixel, regardless of the size
  // slider. The preview should reflect that — a fat outlined rect at size
  // 11 with shape="pixel" would be a lie about what gets painted.
  const effectiveSize = createMemo(() => (props.shape === "pixel" ? 1 : props.size));

  // Canvas-space top-left of the brush rect, snapped to integer pixels.
  const x0 = createMemo(() => {
    const c = props.cursor;
    if (!c) return 0;
    const half = effectiveSize() / 2;
    return Math.floor(c.x) - Math.floor(half);
  });
  const y0 = createMemo(() => {
    const c = props.cursor;
    if (!c) return 0;
    const half = effectiveSize() / 2;
    return Math.floor(c.y) - Math.floor(half);
  });
  const x1 = createMemo(() => x0() + effectiveSize());
  const y1 = createMemo(() => y0() + effectiveSize());

  // Screen-space corners. Each is its own memo so changes to scroll/zoom
  // re-flow the overlay without forcing the entire SVG to re-render.
  const sx0 = createMemo(
    () =>
      canvasToScreen(x0(), y0(), props.scrollX, props.scrollY, props.zoom, props.vpW, props.vpH)[0],
  );
  const sy0 = createMemo(
    () =>
      canvasToScreen(x0(), y0(), props.scrollX, props.scrollY, props.zoom, props.vpW, props.vpH)[1],
  );
  const sx1 = createMemo(
    () =>
      canvasToScreen(x1(), y1(), props.scrollX, props.scrollY, props.zoom, props.vpW, props.vpH)[0],
  );
  const sy1 = createMemo(
    () =>
      canvasToScreen(x1(), y1(), props.scrollX, props.scrollY, props.zoom, props.vpW, props.vpH)[1],
  );
  const w = createMemo(() => sx1() - sx0());
  const h = createMemo(() => sy1() - sy0());

  return (
    <Show when={props.cursor}>
      <svg
        class="canvas-overlay brush-cursor-overlay"
        width={props.vpW}
        height={props.vpH}
        viewBox={`0 0 ${props.vpW} ${props.vpH}`}
        aria-hidden="true"
      >
        {props.shape === "circle" ? (
          <ellipse
            cx={sx0() + w() / 2}
            cy={sy0() + h() / 2}
            rx={w() / 2}
            ry={h() / 2}
            fill="none"
            stroke="white"
            stroke-width="1"
            stroke-dasharray="2 2"
          />
        ) : (
          <rect
            x={sx0()}
            y={sy0()}
            width={Math.max(w(), 1)}
            height={Math.max(h(), 1)}
            fill="none"
            stroke="white"
            stroke-width="1"
            stroke-dasharray="2 2"
          />
        )}
      </svg>
    </Show>
  );
};

// ── Shape-drag preview overlay ─────────────────────────────────────────────

interface ShapePreviewProps extends ViewportProps {
  /** Active shape drag, or null when no shape tool is dragging. */
  preview: ShapePreviewState | null;
}

/**
 * Outlines the line / rect / ellipse the user is currently dragging.
 *
 * Renders nothing while the shape tool is idle. While a drag is active,
 * the line / rect / ellipse is drawn against the visible canvas using
 * the same screen-pixel conversion as the brush cursor so the preview
 * lines up with the bytes the dispatch will eventually paint.
 */
export const ShapePreview: Component<ShapePreviewProps> = (props) => {
  // Convert a canvas-pixel corner to its top-left screen position.
  const corner = (x: number, y: number): [number, number] =>
    canvasToScreen(x, y, props.scrollX, props.scrollY, props.zoom, props.vpW, props.vpH);

  return (
    <Show when={props.preview}>
      {(preview) => {
        // Re-derive the box on every preview change so each move re-renders.
        const startCorner = createMemo(() => corner(preview().start[0], preview().start[1]));
        const endCorner = createMemo(() => corner(preview().end[0], preview().end[1]));

        // Inclusive box: a 1x1 pixel at (5, 5) covers screen rect (5..6, 5..6).
        const x0 = createMemo(() => Math.min(startCorner()[0], endCorner()[0]));
        const y0 = createMemo(() => Math.min(startCorner()[1], endCorner()[1]));
        const x1 = createMemo(() => {
          const px = Math.max(preview().start[0], preview().end[0]);
          return corner(px + 1, 0)[0];
        });
        const y1 = createMemo(() => {
          const py = Math.max(preview().start[1], preview().end[1]);
          return corner(0, py + 1)[1];
        });
        const w = createMemo(() => x1() - x0());
        const h = createMemo(() => y1() - y0());

        return (
          <svg
            class="canvas-overlay shape-preview-overlay"
            width={props.vpW}
            height={props.vpH}
            viewBox={`0 0 ${props.vpW} ${props.vpH}`}
            aria-hidden="true"
          >
            <Show when={preview().kind === "line"}>
              {/* Anchor and current cursor in screen-pixel centres of their cells. */}
              <line
                x1={corner(preview().start[0], 0)[0] + cellCentreOffset(props.zoom)}
                y1={corner(0, preview().start[1])[1] + cellCentreOffset(props.zoom)}
                x2={corner(preview().end[0], 0)[0] + cellCentreOffset(props.zoom)}
                y2={corner(0, preview().end[1])[1] + cellCentreOffset(props.zoom)}
                stroke="white"
                stroke-width="1"
                stroke-dasharray="3 3"
              />
            </Show>
            <Show when={preview().kind === "rect"}>
              <rect
                x={x0()}
                y={y0()}
                width={Math.max(w(), 1)}
                height={Math.max(h(), 1)}
                fill="none"
                stroke="white"
                stroke-width="1"
                stroke-dasharray="3 3"
              />
            </Show>
            <Show when={preview().kind === "ellipse"}>
              <ellipse
                cx={x0() + w() / 2}
                cy={y0() + h() / 2}
                rx={Math.max(w() / 2, 0.5)}
                ry={Math.max(h() / 2, 0.5)}
                fill="none"
                stroke="white"
                stroke-width="1"
                stroke-dasharray="3 3"
              />
            </Show>
          </svg>
        );
      }}
    </Show>
  );
};

/** Half-cell offset in screen pixels at the given zoom. */
function cellCentreOffset(zoom: number): number {
  return zoom / 2;
}

// ── Transform handles overlay ──────────────────────────────────────────────

interface TransformHandlesProps extends ViewportProps {
  /** Selection or transform-target bounding box in canvas coordinates. */
  bounds: { x: number; y: number; width: number; height: number } | null;
  /**
   * Called when the user presses down on a handle or the interior of the
   * bounding box.  S16 uses this to start a transform drag.
   */
  onHandleDown?: (handle: TransformHandle, e: PointerEvent) => void;
}

/**
 * Eight resize handles plus one rotation handle, drawn at the corners and
 * edge midpoints of `bounds`.
 *
 * Every handle element has a `data-handle` attribute identifying it
 * ("nw", "n", "ne", "e", "se", "s", "sw", "w", "rotate").  The body of the
 * bounding-box border is also pointer-event-enabled as the "body" handle for
 * drag-to-translate.
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

        const handles: Array<{ id: TransformHandle; cx: number; cy: number; cursor: string }> = [
          { id: "nw", cx: sx0, cy: sy0, cursor: "nwse-resize" },
          { id: "n", cx: cxMid, cy: sy0, cursor: "ns-resize" },
          { id: "ne", cx: sx1, cy: sy0, cursor: "nesw-resize" },
          { id: "e", cx: sx1, cy: cyMid, cursor: "ew-resize" },
          { id: "se", cx: sx1, cy: sy1, cursor: "nwse-resize" },
          { id: "s", cx: cxMid, cy: sy1, cursor: "ns-resize" },
          { id: "sw", cx: sx0, cy: sy1, cursor: "nesw-resize" },
          { id: "w", cx: sx0, cy: cyMid, cursor: "ew-resize" },
        ];

        function fireHandle(id: TransformHandle, e: PointerEvent): void {
          e.stopPropagation();
          props.onHandleDown?.(id, e);
        }

        return (
          <svg
            class="canvas-overlay transform-handles-overlay"
            width={props.vpW}
            height={props.vpH}
            viewBox={`0 0 ${props.vpW} ${props.vpH}`}
            aria-hidden="true"
          >
            {/* Interior hit zone: drag the body to translate the selection. */}
            <rect
              x={sx0}
              y={sy0}
              width={sx1 - sx0}
              height={sy1 - sy0}
              fill="transparent"
              stroke="#7aa2ff"
              stroke-width="1"
              stroke-dasharray="3 3"
              style={{ cursor: "move" }}
              onPointerDown={(e) => fireHandle("body", e)}
            />
            {/* Tether between rotation handle and bounding box. */}
            <line
              x1={cxMid}
              y1={sy0}
              x2={cxMid}
              y2={sy0 - rotateOffset}
              stroke="#7aa2ff"
              stroke-width="1"
              pointer-events="none"
            />
            <circle
              cx={cxMid}
              cy={sy0 - rotateOffset}
              r={handleSize / 2 + 2}
              fill="#0f0f13"
              stroke="#7aa2ff"
              stroke-width="1.5"
              data-handle="rotate"
              style={{ cursor: "alias" }}
              onPointerDown={(e) => fireHandle("rotate", e)}
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
                  data-handle={h.id}
                  style={{ cursor: h.cursor }}
                  onPointerDown={(e) => fireHandle(h.id, e)}
                />
              )}
            </For>
          </svg>
        );
      }}
    </Show>
  );
};
