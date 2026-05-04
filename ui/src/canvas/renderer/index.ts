// WebGL2 canvas viewport renderer.
//
// Manages five shader programs:
//   SPRITE      — checkerboard background + tile textures
//   ONION       — neighbour-frame tiles tinted red/blue, blended over SPRITE
//   GRID        — per-pixel grid overlay (zoom >= 4)
//   MAJOR_GRID  — configurable major-grid overlay (every N canvas pixels)
//   ANTS        — animated selection outline
//
// Call setViewport() / setSprite() / setSelection() / setOnionSkin() /
// setMajorGrid() to push state, then call markDirty() (or let an animated
// selection drive it) to schedule a frame.  Static frames stop redrawing
// once the dirty flag is consumed.  Call destroy() when the canvas element
// is removed.

import {
  COMMON_VERT,
  SPRITE_FRAG,
  GRID_FRAG,
  MAJOR_GRID_FRAG,
  ONION_FRAG,
  ANTS_VERT,
  ANTS_FRAG,
} from "./shaders";
import { TileCache, TILE_SIZE, tileKey, type TileData } from "./tile-cache";
import { PIXEL_GRID_ZOOM_THRESHOLD } from "../viewport";

// ── WebGL helpers ──────────────────────────────────────────────────────────

function compileShader(gl: WebGL2RenderingContext, type: number, src: string): WebGLShader {
  const shader = gl.createShader(type);
  if (!shader) throw new Error("createShader failed");
  gl.shaderSource(shader, src);
  gl.compileShader(shader);
  if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
    const log = gl.getShaderInfoLog(shader) ?? "unknown";
    gl.deleteShader(shader);
    throw new Error(`shader compile error: ${log}`);
  }
  return shader;
}

function linkProgram(gl: WebGL2RenderingContext, vertSrc: string, fragSrc: string): WebGLProgram {
  const vert = compileShader(gl, gl.VERTEX_SHADER, vertSrc);
  const frag = compileShader(gl, gl.FRAGMENT_SHADER, fragSrc);
  const prog = gl.createProgram();
  if (!prog) throw new Error("createProgram failed");
  gl.attachShader(prog, vert);
  gl.attachShader(prog, frag);
  gl.linkProgram(prog);
  gl.deleteShader(vert);
  gl.deleteShader(frag);
  if (!gl.getProgramParameter(prog, gl.LINK_STATUS)) {
    const log = gl.getProgramInfoLog(prog) ?? "unknown";
    gl.deleteProgram(prog);
    throw new Error(`program link error: ${log}`);
  }
  return prog;
}

// Upload a 6-vertex (2 triangles) quad as a Float32Array to a buffer.
// Positions are in whatever coordinate space the caller uses (canvas px).
function uploadQuad(
  gl: WebGL2RenderingContext,
  buf: WebGLBuffer,
  x0: number,
  y0: number,
  x1: number,
  y1: number,
): void {
  const data = new Float32Array([x0, y0, x1, y0, x0, y1, x0, y1, x1, y0, x1, y1]);
  gl.bindBuffer(gl.ARRAY_BUFFER, buf);
  gl.bufferData(gl.ARRAY_BUFFER, data, gl.DYNAMIC_DRAW);
}

// ── Renderer ───────────────────────────────────────────────────────────────

export interface ViewportConfig {
  scrollX: number;
  scrollY: number;
  zoom: number;
  /** Canvas element dimensions in CSS pixels (not device pixels). */
  width: number;
  height: number;
}

export interface SpriteConfig {
  spriteId: string;
  frameIndex: number;
  spriteWidth: number;
  spriteHeight: number;
  showPixelGrid: boolean;
  onionSkin: boolean;
}

export interface SelectionConfig {
  /** null = no selection. */
  rect: { x: number; y: number; width: number; height: number } | null;
}

export interface OnionSkinConfig {
  /** How many earlier frames to show, tinted red. */
  prev: number;
  /** How many later frames to show, tinted blue. */
  next: number;
  /** Per-frame opacity multiplier in `[0, 1]`. Earlier/later frames fade further. */
  opacity: number;
}

export interface MajorGridConfig {
  enabled: boolean;
  /** Distance between grid lines in canvas pixels. */
  spacing: number;
}

interface SpriteProgram {
  prog: WebGLProgram;
  vao: WebGLVertexArrayObject;
  buf: WebGLBuffer;
  loc: {
    aPos: number;
    uResolution: WebGLUniformLocation;
    uScroll: WebGLUniformLocation;
    uZoom: WebGLUniformLocation;
    uTile: WebGLUniformLocation;
    uHasTile: WebGLUniformLocation;
    uTileOriginCanvas: WebGLUniformLocation;
    uTileSizeCanvas: WebGLUniformLocation;
  };
}

interface GridProgram {
  prog: WebGLProgram;
  vao: WebGLVertexArrayObject;
  buf: WebGLBuffer;
  loc: {
    aPos: number;
    uResolution: WebGLUniformLocation;
    uScroll: WebGLUniformLocation;
    uZoom: WebGLUniformLocation;
  };
}

interface MajorGridProgram {
  prog: WebGLProgram;
  vao: WebGLVertexArrayObject;
  buf: WebGLBuffer;
  loc: {
    aPos: number;
    uResolution: WebGLUniformLocation;
    uScroll: WebGLUniformLocation;
    uZoom: WebGLUniformLocation;
    uSpacing: WebGLUniformLocation;
  };
}

interface OnionProgram {
  prog: WebGLProgram;
  vao: WebGLVertexArrayObject;
  buf: WebGLBuffer;
  loc: {
    aPos: number;
    uResolution: WebGLUniformLocation;
    uScroll: WebGLUniformLocation;
    uZoom: WebGLUniformLocation;
    uTile: WebGLUniformLocation;
    uTileOriginCanvas: WebGLUniformLocation;
    uTileSizeCanvas: WebGLUniformLocation;
    uTint: WebGLUniformLocation;
    uOpacity: WebGLUniformLocation;
  };
}

interface AntsProgram {
  prog: WebGLProgram;
  vao: WebGLVertexArrayObject;
  buf: WebGLBuffer;
  loc: {
    aPos: number;
    uResolution: WebGLUniformLocation;
    uScroll: WebGLUniformLocation;
    uZoom: WebGLUniformLocation;
    uSelMin: WebGLUniformLocation;
    uSelMax: WebGLUniformLocation;
    uTime: WebGLUniformLocation;
  };
}

function buildSpriteProgram(gl: WebGL2RenderingContext): SpriteProgram {
  const prog = linkProgram(gl, COMMON_VERT, SPRITE_FRAG);
  const vao = gl.createVertexArray()!;
  const buf = gl.createBuffer()!;

  gl.bindVertexArray(vao);
  gl.bindBuffer(gl.ARRAY_BUFFER, buf);
  const aPos = gl.getAttribLocation(prog, "a_pos");
  gl.enableVertexAttribArray(aPos);
  gl.vertexAttribPointer(aPos, 2, gl.FLOAT, false, 0, 0);
  gl.bindVertexArray(null);

  return {
    prog,
    vao,
    buf,
    loc: {
      aPos,
      uResolution: gl.getUniformLocation(prog, "u_resolution")!,
      uScroll: gl.getUniformLocation(prog, "u_scroll")!,
      uZoom: gl.getUniformLocation(prog, "u_zoom")!,
      uTile: gl.getUniformLocation(prog, "u_tile")!,
      uHasTile: gl.getUniformLocation(prog, "u_has_tile")!,
      uTileOriginCanvas: gl.getUniformLocation(prog, "u_tile_origin_canvas")!,
      uTileSizeCanvas: gl.getUniformLocation(prog, "u_tile_size_canvas")!,
    },
  };
}

function buildGridProgram(gl: WebGL2RenderingContext): GridProgram {
  const prog = linkProgram(gl, COMMON_VERT, GRID_FRAG);
  const vao = gl.createVertexArray()!;
  const buf = gl.createBuffer()!;

  gl.bindVertexArray(vao);
  gl.bindBuffer(gl.ARRAY_BUFFER, buf);
  const aPos = gl.getAttribLocation(prog, "a_pos");
  gl.enableVertexAttribArray(aPos);
  gl.vertexAttribPointer(aPos, 2, gl.FLOAT, false, 0, 0);
  gl.bindVertexArray(null);

  return {
    prog,
    vao,
    buf,
    loc: {
      aPos,
      uResolution: gl.getUniformLocation(prog, "u_resolution")!,
      uScroll: gl.getUniformLocation(prog, "u_scroll")!,
      uZoom: gl.getUniformLocation(prog, "u_zoom")!,
    },
  };
}

function buildMajorGridProgram(gl: WebGL2RenderingContext): MajorGridProgram {
  const prog = linkProgram(gl, COMMON_VERT, MAJOR_GRID_FRAG);
  const vao = gl.createVertexArray()!;
  const buf = gl.createBuffer()!;

  gl.bindVertexArray(vao);
  gl.bindBuffer(gl.ARRAY_BUFFER, buf);
  const aPos = gl.getAttribLocation(prog, "a_pos");
  gl.enableVertexAttribArray(aPos);
  gl.vertexAttribPointer(aPos, 2, gl.FLOAT, false, 0, 0);
  gl.bindVertexArray(null);

  return {
    prog,
    vao,
    buf,
    loc: {
      aPos,
      uResolution: gl.getUniformLocation(prog, "u_resolution")!,
      uScroll: gl.getUniformLocation(prog, "u_scroll")!,
      uZoom: gl.getUniformLocation(prog, "u_zoom")!,
      uSpacing: gl.getUniformLocation(prog, "u_spacing")!,
    },
  };
}

function buildOnionProgram(gl: WebGL2RenderingContext): OnionProgram {
  const prog = linkProgram(gl, COMMON_VERT, ONION_FRAG);
  const vao = gl.createVertexArray()!;
  const buf = gl.createBuffer()!;

  gl.bindVertexArray(vao);
  gl.bindBuffer(gl.ARRAY_BUFFER, buf);
  const aPos = gl.getAttribLocation(prog, "a_pos");
  gl.enableVertexAttribArray(aPos);
  gl.vertexAttribPointer(aPos, 2, gl.FLOAT, false, 0, 0);
  gl.bindVertexArray(null);

  return {
    prog,
    vao,
    buf,
    loc: {
      aPos,
      uResolution: gl.getUniformLocation(prog, "u_resolution")!,
      uScroll: gl.getUniformLocation(prog, "u_scroll")!,
      uZoom: gl.getUniformLocation(prog, "u_zoom")!,
      uTile: gl.getUniformLocation(prog, "u_tile")!,
      uTileOriginCanvas: gl.getUniformLocation(prog, "u_tile_origin_canvas")!,
      uTileSizeCanvas: gl.getUniformLocation(prog, "u_tile_size_canvas")!,
      uTint: gl.getUniformLocation(prog, "u_tint")!,
      uOpacity: gl.getUniformLocation(prog, "u_opacity")!,
    },
  };
}

function buildAntsProgram(gl: WebGL2RenderingContext): AntsProgram {
  const prog = linkProgram(gl, ANTS_VERT, ANTS_FRAG);
  const vao = gl.createVertexArray()!;
  const buf = gl.createBuffer()!;

  gl.bindVertexArray(vao);
  gl.bindBuffer(gl.ARRAY_BUFFER, buf);
  const aPos = gl.getAttribLocation(prog, "a_pos");
  gl.enableVertexAttribArray(aPos);
  gl.vertexAttribPointer(aPos, 2, gl.FLOAT, false, 0, 0);
  gl.bindVertexArray(null);

  return {
    prog,
    vao,
    buf,
    loc: {
      aPos,
      uResolution: gl.getUniformLocation(prog, "u_resolution")!,
      uScroll: gl.getUniformLocation(prog, "u_scroll")!,
      uZoom: gl.getUniformLocation(prog, "u_zoom")!,
      uSelMin: gl.getUniformLocation(prog, "u_sel_min")!,
      uSelMax: gl.getUniformLocation(prog, "u_sel_max")!,
      uTime: gl.getUniformLocation(prog, "u_time")!,
    },
  };
}

// Expands a selection rect outward by `margin` canvas pixels for the marching-
// ants border quad so the border straddles the selection edge.
function antsQuadCoords(
  x: number,
  y: number,
  w: number,
  h: number,
  margin: number,
): [number, number, number, number] {
  return [x - margin, y - margin, x + w + margin, y + h + margin];
}

export class CanvasRenderer {
  private canvas: HTMLCanvasElement;
  private gl: WebGL2RenderingContext;
  private spriteP!: SpriteProgram;
  private gridP!: GridProgram;
  private majorGridP!: MajorGridProgram;
  private onionP!: OnionProgram;
  private antsP!: AntsProgram;
  private tileCache!: TileCache;
  private rafId = 0;
  private startTime: number;

  private vp: ViewportConfig = { scrollX: 0, scrollY: 0, zoom: 1, width: 1, height: 1 };
  private sprite: SpriteConfig | null = null;
  private selection: SelectionConfig = { rect: null };
  private onion: OnionSkinConfig = { prev: 1, next: 1, opacity: 0.4 };
  private majorGrid: MajorGridConfig = { enabled: false, spacing: 8 };

  // Re-render gating.  When false, the RAF loop skips drawing — the previous
  // frame stays on screen.  Selection animation forces continuous redraw.
  private dirty = true;
  // Tile coordinates that became dirty since the last render.  Currently used
  // only as a stat; future passes can scope re-uploads to this set.
  private dirtyTiles = new Set<string>();
  private contextLost = false;

  // Stable handlers retained so we can detach them in destroy().
  private onContextLost: (e: Event) => void;
  private onContextRestored: () => void;

  constructor(canvas: HTMLCanvasElement) {
    const gl = canvas.getContext("webgl2", { alpha: false, antialias: false });
    if (!gl) throw new Error("[pixhaus] WebGL2 is not available in this browser");

    this.canvas = canvas;
    this.gl = gl;
    this.startTime = performance.now();

    this.buildPrograms();

    // ── Context-loss handlers ───────────────────────────────────────────
    // GPU resets, driver crashes, and tab backgrounding can drop the GL
    // context.  Without these handlers the renderer dies silently.
    this.onContextLost = (e: Event) => {
      e.preventDefault();
      this.contextLost = true;
      cancelAnimationFrame(this.rafId);
      this.rafId = 0;
      // Mark every cached tile dirty so they get re-uploaded after restore.
      this.dirtyTiles.clear();
      this.tileCache.markAllDirty(this.dirtyTiles);
    };

    this.onContextRestored = () => {
      this.contextLost = false;
      // Build new GL programs/textures on the restored context.
      this.tileCache = new TileCache(this.gl);
      this.buildPrograms();
      this.gl.enable(this.gl.BLEND);
      this.gl.blendFunc(this.gl.SRC_ALPHA, this.gl.ONE_MINUS_SRC_ALPHA);
      this.dirty = true;
      this.scheduleFrame();
    };

    canvas.addEventListener("webglcontextlost", this.onContextLost, false);
    canvas.addEventListener("webglcontextrestored", this.onContextRestored, false);

    gl.enable(gl.BLEND);
    gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA);

    this.scheduleFrame();
  }

  private buildPrograms(): void {
    const { gl } = this;
    this.spriteP = buildSpriteProgram(gl);
    this.gridP = buildGridProgram(gl);
    this.majorGridP = buildMajorGridProgram(gl);
    this.onionP = buildOnionProgram(gl);
    this.antsP = buildAntsProgram(gl);
    if (!this.tileCache) {
      this.tileCache = new TileCache(gl);
    }
  }

  // ── Public API ─────────────────────────────────────────────────────────

  setViewport(vp: ViewportConfig): void {
    this.vp = vp;
    const { gl } = this;
    if (gl.canvas.width !== vp.width || gl.canvas.height !== vp.height) {
      (gl.canvas as HTMLCanvasElement).width = vp.width;
      (gl.canvas as HTMLCanvasElement).height = vp.height;
    }
    this.markDirty();
  }

  setSprite(sprite: SpriteConfig | null): void {
    if (this.sprite && sprite?.spriteId !== this.sprite.spriteId) {
      this.tileCache.evict(this.sprite.spriteId);
    }
    this.sprite = sprite;
    this.markDirty();
  }

  setSelection(sel: SelectionConfig): void {
    this.selection = sel;
    this.markDirty();
  }

  setOnionSkin(onion: OnionSkinConfig): void {
    this.onion = onion;
    this.markDirty();
  }

  setMajorGrid(grid: MajorGridConfig): void {
    this.majorGrid = grid;
    this.markDirty();
  }

  /** Upload a composited tile returned from canvas_composite. */
  uploadTile(
    spriteId: string,
    frameIndex: number,
    tileX: number,
    tileY: number,
    data: TileData,
  ): void {
    const key = tileKey(spriteId, frameIndex, tileX, tileY);
    this.tileCache.update(key, spriteId, data);
    this.markDirty(tileX, tileY);
  }

  /**
   * Schedules a frame.  When called with tile coordinates, also records the
   * tile so future passes can scope re-uploads or partial repaints.  The IPC
   * `canvas:tile-dirty` handler routes through here via `uploadTile`.
   */
  markDirty(tileX?: number, tileY?: number): void {
    this.dirty = true;
    if (tileX !== undefined && tileY !== undefined) {
      this.dirtyTiles.add(`${tileX}:${tileY}`);
    }
    if (this.rafId === 0 && !this.contextLost) this.scheduleFrame();
  }

  destroy(): void {
    cancelAnimationFrame(this.rafId);
    this.canvas.removeEventListener("webglcontextlost", this.onContextLost);
    this.canvas.removeEventListener("webglcontextrestored", this.onContextRestored);
    this.tileCache.clear();
    const { gl } = this;
    gl.deleteProgram(this.spriteP.prog);
    gl.deleteVertexArray(this.spriteP.vao);
    gl.deleteBuffer(this.spriteP.buf);
    gl.deleteProgram(this.gridP.prog);
    gl.deleteVertexArray(this.gridP.vao);
    gl.deleteBuffer(this.gridP.buf);
    gl.deleteProgram(this.majorGridP.prog);
    gl.deleteVertexArray(this.majorGridP.vao);
    gl.deleteBuffer(this.majorGridP.buf);
    gl.deleteProgram(this.onionP.prog);
    gl.deleteVertexArray(this.onionP.vao);
    gl.deleteBuffer(this.onionP.buf);
    gl.deleteProgram(this.antsP.prog);
    gl.deleteVertexArray(this.antsP.vao);
    gl.deleteBuffer(this.antsP.buf);
  }

  // ── Render loop ────────────────────────────────────────────────────────

  private scheduleFrame(): void {
    this.rafId = requestAnimationFrame((t) => this.render(t));
  }

  private render(t: DOMHighResTimeStamp): void {
    if (this.contextLost) {
      this.rafId = 0;
      return;
    }

    // Selection animation needs continuous redraw for the marching ants.
    const animating = this.selection.rect !== null;
    if (!this.dirty && !animating) {
      // Nothing to do this frame.  Park the RAF loop until something marks
      // dirty or selection becomes active.
      this.rafId = 0;
      return;
    }

    this.scheduleFrame();
    this.dirty = false;
    this.dirtyTiles.clear();

    const { gl, vp } = this;
    gl.viewport(0, 0, vp.width, vp.height);
    gl.clearColor(0.059, 0.059, 0.075, 1.0); // matches --bg-app #0f0f13
    gl.clear(gl.COLOR_BUFFER_BIT);

    if (this.sprite) {
      this.renderSpriteTiles();

      if (this.sprite.onionSkin) {
        this.renderOnionSkin();
      }

      if (vp.zoom >= PIXEL_GRID_ZOOM_THRESHOLD && this.sprite.showPixelGrid) {
        this.renderGrid();
      }

      if (this.majorGrid.enabled && this.majorGrid.spacing > 0) {
        this.renderMajorGrid();
      }
    }

    if (this.selection.rect) {
      this.renderAnts(t);
    }
  }

  // ── Sprite tile pass ────────────────────────────────────────────────────

  private renderSpriteTiles(): void {
    const { gl, vp, spriteP: p, sprite } = this;
    if (!sprite) return;

    gl.useProgram(p.prog);
    gl.bindVertexArray(p.vao);
    gl.uniform2f(p.loc.uResolution, vp.width, vp.height);
    gl.uniform2f(p.loc.uScroll, vp.scrollX, vp.scrollY);
    gl.uniform1f(p.loc.uZoom, vp.zoom);
    gl.uniform1i(p.loc.uTile, 0);

    const tilesX = Math.ceil(sprite.spriteWidth / TILE_SIZE);
    const tilesY = Math.ceil(sprite.spriteHeight / TILE_SIZE);

    for (let ty = 0; ty < tilesY; ty++) {
      for (let tx = 0; tx < tilesX; tx++) {
        const x0 = tx * TILE_SIZE;
        const y0 = ty * TILE_SIZE;
        const x1 = Math.min(x0 + TILE_SIZE, sprite.spriteWidth);
        const y1 = Math.min(y0 + TILE_SIZE, sprite.spriteHeight);
        const tileW = x1 - x0;
        const tileH = y1 - y0;

        uploadQuad(gl, p.buf, x0, y0, x1, y1);

        const key = tileKey(sprite.spriteId, sprite.frameIndex, tx, ty);
        const bound = this.tileCache.bind(key, 0);

        gl.uniform1i(p.loc.uHasTile, bound ? 1 : 0);
        gl.uniform2f(p.loc.uTileOriginCanvas, x0, y0);
        gl.uniform2f(p.loc.uTileSizeCanvas, tileW, tileH);

        gl.drawArrays(gl.TRIANGLES, 0, 6);
      }
    }

    gl.bindVertexArray(null);
  }

  // ── Onion-skin pass ─────────────────────────────────────────────────────

  // For each neighbour frame in [-prev, +next] (excluding current), iterate
  // the tile grid and additively blend the tinted tile over the current
  // frame.  Per-frame opacity falls off with distance.
  private renderOnionSkin(): void {
    const { gl, vp, onionP: p, sprite, onion } = this;
    if (!sprite) return;
    if (onion.prev <= 0 && onion.next <= 0) return;

    gl.useProgram(p.prog);
    gl.bindVertexArray(p.vao);
    gl.uniform2f(p.loc.uResolution, vp.width, vp.height);
    gl.uniform2f(p.loc.uScroll, vp.scrollX, vp.scrollY);
    gl.uniform1f(p.loc.uZoom, vp.zoom);
    gl.uniform1i(p.loc.uTile, 0);

    const tilesX = Math.ceil(sprite.spriteWidth / TILE_SIZE);
    const tilesY = Math.ceil(sprite.spriteHeight / TILE_SIZE);

    // Walk neighbour frames, drawing each as one full pass.
    const passes: Array<{ delta: number; tint: [number, number, number] }> = [];
    for (let d = onion.prev; d >= 1; d--) passes.push({ delta: -d, tint: [1.0, 0.4, 0.4] });
    for (let d = 1; d <= onion.next; d++) passes.push({ delta: d, tint: [0.4, 0.5, 1.0] });

    for (const pass of passes) {
      const target = sprite.frameIndex + pass.delta;
      if (target < 0) continue; // no past frame -1; skip
      const distance = Math.abs(pass.delta);
      const opacity = onion.opacity / distance; // earlier/later fade further

      gl.uniform3f(p.loc.uTint, pass.tint[0], pass.tint[1], pass.tint[2]);
      gl.uniform1f(p.loc.uOpacity, opacity);

      for (let ty = 0; ty < tilesY; ty++) {
        for (let tx = 0; tx < tilesX; tx++) {
          const x0 = tx * TILE_SIZE;
          const y0 = ty * TILE_SIZE;
          const x1 = Math.min(x0 + TILE_SIZE, sprite.spriteWidth);
          const y1 = Math.min(y0 + TILE_SIZE, sprite.spriteHeight);
          const tileW = x1 - x0;
          const tileH = y1 - y0;

          const key = tileKey(sprite.spriteId, target, tx, ty);
          const bound = this.tileCache.bind(key, 0);
          if (!bound) continue; // neighbour tile not cached — skip this quad

          uploadQuad(gl, p.buf, x0, y0, x1, y1);
          gl.uniform2f(p.loc.uTileOriginCanvas, x0, y0);
          gl.uniform2f(p.loc.uTileSizeCanvas, tileW, tileH);
          gl.drawArrays(gl.TRIANGLES, 0, 6);
        }
      }
    }

    gl.bindVertexArray(null);
  }

  // ── Pixel grid pass ─────────────────────────────────────────────────────

  private renderGrid(): void {
    const { gl, vp, gridP: p, sprite } = this;
    if (!sprite) return;

    gl.useProgram(p.prog);
    gl.bindVertexArray(p.vao);
    gl.uniform2f(p.loc.uResolution, vp.width, vp.height);
    gl.uniform2f(p.loc.uScroll, vp.scrollX, vp.scrollY);
    gl.uniform1f(p.loc.uZoom, vp.zoom);

    uploadQuad(gl, p.buf, 0, 0, sprite.spriteWidth, sprite.spriteHeight);
    gl.drawArrays(gl.TRIANGLES, 0, 6);

    gl.bindVertexArray(null);
  }

  // ── Major-grid pass ─────────────────────────────────────────────────────

  private renderMajorGrid(): void {
    const { gl, vp, majorGridP: p, sprite, majorGrid } = this;
    if (!sprite) return;

    gl.useProgram(p.prog);
    gl.bindVertexArray(p.vao);
    gl.uniform2f(p.loc.uResolution, vp.width, vp.height);
    gl.uniform2f(p.loc.uScroll, vp.scrollX, vp.scrollY);
    gl.uniform1f(p.loc.uZoom, vp.zoom);
    gl.uniform1f(p.loc.uSpacing, majorGrid.spacing);

    uploadQuad(gl, p.buf, 0, 0, sprite.spriteWidth, sprite.spriteHeight);
    gl.drawArrays(gl.TRIANGLES, 0, 6);

    gl.bindVertexArray(null);
  }

  // ── Marching ants pass ──────────────────────────────────────────────────

  private renderAnts(t: DOMHighResTimeStamp): void {
    const { gl, vp, antsP: p, selection } = this;
    const rect = selection.rect;
    if (!rect) return;

    const elapsed = (t - this.startTime) / 1000;

    gl.useProgram(p.prog);
    gl.bindVertexArray(p.vao);
    gl.uniform2f(p.loc.uResolution, vp.width, vp.height);
    gl.uniform2f(p.loc.uScroll, vp.scrollX, vp.scrollY);
    gl.uniform1f(p.loc.uZoom, vp.zoom);
    gl.uniform2f(p.loc.uSelMin, rect.x, rect.y);
    gl.uniform2f(p.loc.uSelMax, rect.x + rect.width, rect.y + rect.height);
    gl.uniform1f(p.loc.uTime, elapsed);

    const [qx0, qy0, qx1, qy1] = antsQuadCoords(rect.x, rect.y, rect.width, rect.height, 2);
    uploadQuad(gl, p.buf, qx0, qy0, qx1, qy1);
    gl.drawArrays(gl.TRIANGLES, 0, 6);

    gl.bindVertexArray(null);
  }
}
