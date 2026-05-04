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

import { onMount, onCleanup, createEffect, createSignal, type Component } from "solid-js";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
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
} from "./canvas-state";
import { activeProject } from "../project-state";
import { canvasComposite } from "../lib/commands/canvas";

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
      const { width, height } = entry.contentRect;
      const w = Math.round(width);
      const h = Math.round(height);
      canvasEl.width = w;
      canvasEl.height = h;
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

    let unlisten: UnlistenFn | undefined;
    listen<TileDirtyPayload>("canvas:tile-dirty", (event) => {
      const p = event.payload;
      const raw = atob(p.data);
      const bytes = new Uint8Array(raw.length);
      for (let i = 0; i < raw.length; i++) bytes[i] = raw.charCodeAt(i);
      renderer.uploadTile(String(p.sprite_id), p.frame_index, p.tile_x, p.tile_y, {
        bytes,
        width: p.width,
        height: p.height,
      });
    })
      .then((fn) => {
        unlisten = fn;
      })
      .catch((err: unknown) => {
        console.warn("[pixhaus] listen canvas:tile-dirty failed:", err);
      });

    // ── Reactive bridge: push signal changes to the renderer ─────────────
    createEffect(() => {
      const spriteId = activeSpriteId();
      const proj = activeProject();
      if (!proj || spriteId === null) {
        renderer.setSprite(null);
        return;
      }

      canvasComposite(spriteId)
        .then((info) => {
          renderer.setSprite({
            spriteId: String(spriteId),
            frameIndex: activeFrameIndex(),
            spriteWidth: info.sprite_width,
            spriteHeight: info.sprite_height,
            showPixelGrid: showPixelGrid(),
            onionSkin: onionSkin(),
          });
        })
        .catch((err: unknown) => {
          console.warn("[pixhaus] canvas_composite failed:", err);
          renderer.setSprite({
            spriteId: String(spriteId),
            frameIndex: activeFrameIndex(),
            spriteWidth: 32,
            spriteHeight: 32,
            showPixelGrid: showPixelGrid(),
            onionSkin: onionSkin(),
          });
        });
    });

    createEffect(() => {
      renderer.setViewport({
        scrollX: scrollX(),
        scrollY: scrollY(),
        zoom: zoom(),
        width: canvasEl.width,
        height: canvasEl.height,
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
      unlisten?.();
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
