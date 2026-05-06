// Canvas viewport component.
//
// Hosts a single <canvas> element that WebGL2 renders into, with two SVG
// overlays (brush cursor, transform handles) layered on top.  Wires the
// renderer, input handlers, Solid state signals, and IPC event listeners.
//
// Lifecycle:
//   mount  → create renderer, attach input, subscribe to tile-dirty events
//   update → ResizeObserver keeps the GL viewport in sync
//   unmount → destroy renderer, remove all listeners

import { onMount, onCleanup, createEffect, createSignal, untrack, type Component } from "solid-js";
import { listen } from "@tauri-apps/api/event";
import { CanvasRenderer } from "./renderer";
import { attachCanvasInput } from "./input";
import { BrushCursor, TransformHandles } from "./overlays";
import {
  scrollX,
  scrollY,
  zoom,
  showPixelGrid,
  showTileGrid,
  gridSpacing,
  onionSkin,
  onionSkinPrev,
  onionSkinNext,
  onionSkinOpacity,
  brushSize,
  brushShape,
  cursorCanvas,
  transformBounds,
  activeSpriteId,
  activeFrameIndex,
  selectionRect,
  resetViewport,
} from "./canvas-state";
import { activeProject } from "../project-state";
import { canvasComposite } from "../lib/commands/canvas";
import { spriteList } from "../lib/commands/project";

// ── Tile-dirty event payload ────────────────────────────────────────────────

interface TileDirtyPayload {
  sprite_id: number;
  frame_index: number;
  tile_x: number;
  tile_y: number;
  /** Base64-encoded RGBA bytes (width * height * 4). */
  data: string;
  width: number;
  height: number;
}

// ── Component ──────────────────────────────────────────────────────────────

const Canvas: Component = () => {
  let canvasEl!: HTMLCanvasElement;
  let containerEl!: HTMLDivElement;

  // Viewport CSS dimensions, kept in a signal so SVG overlays re-render in
  // sync with the WebGL canvas when the container resizes.
  const [vpW, setVpW] = createSignal(1);
  const [vpH, setVpH] = createSignal(1);

  onMount(() => {
    let renderer: CanvasRenderer;
    try {
      renderer = new CanvasRenderer(canvasEl);
    } catch (err: unknown) {
      console.error("[pixhaus] canvas renderer init failed:", err);
      return;
    }

    const detachInput = attachCanvasInput(containerEl);

    const ro = new ResizeObserver((entries) => {
      const entry = entries[0];
      if (!entry) return;
      // Local-binding guard so the static analyzer can prove the
      // canvas is non-null for every property access in this block.
      // In practice the ref is always populated by the time
      // ResizeObserver fires (it's bound on mount), but a future
      // unmount-during-callback path would otherwise NPE.
      const canvas = canvasEl;
      if (!canvas) return;
      const { width, height } = entry.contentRect;
      const w = Math.round(width);
      const h = Math.round(height);
      canvas.width = w;
      canvas.height = h;
      setVpW(w);
      setVpH(h);
      renderer.setViewport({
        scrollX: scrollX(),
        scrollY: scrollY(),
        zoom: zoom(),
        width: w,
        height: h,
      });
    });
    ro.observe(containerEl);

    // Hold the listen() promise rather than the resolved UnlistenFn so
    // an unmount that happens before listen() resolves still detaches
    // the listener (the cleanup awaits the promise and calls fn() on
    // resolve). Mirrors the pattern used by Shell.tsx::menuListenerPromise.
    const tileDirtyPromise = listen<TileDirtyPayload>("canvas:tile-dirty", (event) => {
      const p = event.payload;
      const raw = atob(p.data);
      const bytes = new Uint8Array(raw.length);
      for (let i = 0; i < raw.length; i++) bytes[i] = raw.charCodeAt(i);
      renderer.uploadTile(String(p.sprite_id), p.frame_index, p.tile_x, p.tile_y, {
        bytes,
        width: p.width,
        height: p.height,
      });
    });

    // ── Auto-select first sprite when a project loads ─────────────────────
    //
    // Nothing else sets activeSpriteId on project open; without this the
    // canvas compositor and layer panel never fire their first IPC calls.
    // Runs once per Canvas mount (the Show in Shell unmounts it on close).
    let projectSeenNonNull = false;
    createEffect(() => {
      const proj = activeProject();
      if (!proj) {
        projectSeenNonNull = false;
        return;
      }
      if (projectSeenNonNull) return;
      projectSeenNonNull = true;

      // Don't overwrite a sprite that was already selected (e.g. restored
      // from a saved viewport) — only auto-select on a fresh open.
      if (untrack(activeSpriteId) !== null) return;

      spriteList()
        .then((sprites) => {
          const first = sprites[0];
          if (!first) return;
          // vpW/vpH are untracked: by the time the async result arrives the
          // ResizeObserver has already fired its first measurement.
          resetViewport(
            first.canvas.width,
            first.canvas.height,
            Math.max(untrack(vpW), 1),
            Math.max(untrack(vpH), 1),
            first.id,
          );
        })
        .catch((err: unknown) => {
          console.warn("[pixhaus] sprite_list failed:", err);
        });
    });

    // ── Reactive bridge: push signal changes to the renderer ─────────────
    //
    // Split into two effects so toggling the pixel grid, onion skin, or
    // scrubbing frames re-renders without re-running canvas_composite,
    // and so a later canvas_composite reply doesn't clobber state from
    // an in-flight sprite switch.
    //
    // Effect A: fetch composite metadata when (project, sprite) changes.
    // Stale-result protection: each invocation increments
    // `compositeRequestId`; only the latest response writes the size
    // signals.
    const [spriteSize, setSpriteSize] = createSignal<{ w: number; h: number } | null>(null);
    let compositeRequestId = 0;

    createEffect(() => {
      const spriteId = activeSpriteId();
      const proj = activeProject();
      if (!proj || spriteId === null) {
        setSpriteSize(null);
        return;
      }
      const requestId = ++compositeRequestId;
      canvasComposite(spriteId)
        .then((info) => {
          if (requestId !== compositeRequestId) return; // stale
          setSpriteSize({ w: info.sprite_width, h: info.sprite_height });
        })
        .catch((err: unknown) => {
          console.warn("[pixhaus] canvas_composite failed:", err);
          if (requestId !== compositeRequestId) return;
          // Fall back to a small placeholder so the renderer still has
          // dimensions to draw the checkerboard against.
          setSpriteSize({ w: 32, h: 32 });
        });
    });

    // Effect B: synchronously push the latest sprite/frame/grid/onion-skin
    // tuple to the renderer. Re-fires on every signal change — Solid
    // tracks every read at the top of the closure.
    createEffect(() => {
      const spriteId = activeSpriteId();
      const size = spriteSize();
      if (spriteId === null || size === null) {
        renderer.setSprite(null);
        return;
      }
      renderer.setSprite({
        spriteId: String(spriteId),
        frameIndex: activeFrameIndex(),
        spriteWidth: size.w,
        spriteHeight: size.h,
        showPixelGrid: showPixelGrid(),
        onionSkin: onionSkin(),
      });
    });

    createEffect(() => {
      // Local-binding guard for the same null-safety reasons as the
      // ResizeObserver callback above.
      const canvas = canvasEl;
      if (!canvas) return;
      renderer.setViewport({
        scrollX: scrollX(),
        scrollY: scrollY(),
        zoom: zoom(),
        width: canvas.width,
        height: canvas.height,
      });
    });

    createEffect(() => {
      renderer.setSelection({ rect: selectionRect() });
    });

    createEffect(() => {
      renderer.setOnionSkin({
        prev: onionSkinPrev(),
        next: onionSkinNext(),
        opacity: onionSkinOpacity(),
      });
    });

    createEffect(() => {
      renderer.setMajorGrid({ enabled: showTileGrid(), spacing: gridSpacing() });
    });

    onCleanup(() => {
      ro.disconnect();
      detachInput();
      tileDirtyPromise
        .then((unlisten) => unlisten())
        .catch((err: unknown) => {
          console.warn("[pixhaus] failed to unlisten canvas:tile-dirty:", err);
        });
      renderer.destroy();
    });
  });

  return (
    <div ref={containerEl} class="canvas-container" tabIndex={-1}>
      <canvas ref={canvasEl} class="canvas-viewport" />
      <BrushCursor
        scrollX={scrollX()}
        scrollY={scrollY()}
        zoom={zoom()}
        vpW={vpW()}
        vpH={vpH()}
        cursor={cursorCanvas()}
        size={brushSize()}
        shape={brushShape()}
      />
      <TransformHandles
        scrollX={scrollX()}
        scrollY={scrollY()}
        zoom={zoom()}
        vpW={vpW()}
        vpH={vpH()}
        bounds={transformBounds()}
      />
    </div>
  );
};

export default Canvas;
