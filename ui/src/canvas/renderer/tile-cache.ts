// WebGL2 tile texture cache.
//
// Tiles are 256×256-canvas-pixel regions of the composited sprite.  The cache
// owns one WebGL texture per tile; a tile's key is `${spriteId}:${frame}:${x}:${y}`.
//
// When the Rust side emits a `canvas:tile-dirty` event, callers upload new
// RGBA bytes via `update`.  When a sprite is unloaded, call `evict` to free
// all GPU textures for that sprite.

export const TILE_SIZE = 256; // canvas pixels per tile side

export type TileKey = string; // "${spriteId}:${frameIndex}:${tileX}:${tileY}"

export interface TileData {
  /** RGBA bytes, length = width * height * 4. */
  bytes: Uint8Array;
  /** Tile width in canvas pixels (may be < TILE_SIZE at right/bottom edges). */
  width: number;
  /** Tile height in canvas pixels. */
  height: number;
}

interface CacheEntry {
  texture: WebGLTexture;
  tileW: number;
  tileH: number;
  spriteId: string;
}

export function tileKey(
  spriteId: string,
  frameIndex: number,
  tileX: number,
  tileY: number,
): TileKey {
  return `${spriteId}:${frameIndex}:${tileX}:${tileY}`;
}

export class TileCache {
  private gl: WebGL2RenderingContext;
  private entries = new Map<TileKey, CacheEntry>();

  constructor(gl: WebGL2RenderingContext) {
    this.gl = gl;
  }

  /** Uploads or replaces the GPU texture for a tile. */
  update(key: TileKey, spriteId: string, data: TileData): void {
    const { gl } = this;
    let entry = this.entries.get(key);

    if (!entry) {
      const texture = gl.createTexture();
      if (!texture) {
        console.error("[pixhaus] tile-cache: failed to create WebGL texture");
        return;
      }
      entry = { texture, tileW: data.width, tileH: data.height, spriteId };
      this.entries.set(key, entry);
    }

    gl.bindTexture(gl.TEXTURE_2D, entry.texture);
    gl.texImage2D(
      gl.TEXTURE_2D,
      0,
      gl.RGBA,
      data.width,
      data.height,
      0,
      gl.RGBA,
      gl.UNSIGNED_BYTE,
      data.bytes,
    );
    // No mipmaps; nearest-neighbour filtering preserves pixel crispness.
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
    gl.bindTexture(gl.TEXTURE_2D, null);

    entry.tileW = data.width;
    entry.tileH = data.height;
  }

  /** Binds the tile's texture and returns its dimensions, or null if absent. */
  bind(key: TileKey, unit = 0): { tileW: number; tileH: number } | null {
    const entry = this.entries.get(key);
    if (!entry) return null;
    this.gl.activeTexture(this.gl.TEXTURE0 + unit);
    this.gl.bindTexture(this.gl.TEXTURE_2D, entry.texture);
    return { tileW: entry.tileW, tileH: entry.tileH };
  }

  /** Deletes all GPU textures for the given sprite. */
  evict(spriteId: string): void {
    for (const [key, entry] of this.entries) {
      if (entry.spriteId === spriteId) {
        this.gl.deleteTexture(entry.texture);
        this.entries.delete(key);
      }
    }
  }

  /** Deletes all GPU textures. Call before destroying the renderer. */
  clear(): void {
    for (const entry of this.entries.values()) {
      this.gl.deleteTexture(entry.texture);
    }
    this.entries.clear();
  }

  /**
   * Records every cached tile's coordinates into `out` and clears the cache
   * map without touching GL.  Used by the renderer's context-loss handler:
   * after the GL context resets all textures are gone, so the cache must be
   * emptied — and the renderer needs to know which tiles to request again.
   */
  markAllDirty(out: Set<string>): void {
    for (const key of this.entries.keys()) {
      // Strip the spriteId/frame prefix; renderer wants `tx:ty`.
      const parts = key.split(":");
      if (parts.length === 4) out.add(`${parts[2]}:${parts[3]}`);
    }
    // The GL textures the entries point at are already invalid after a
    // context loss event, so just drop the references.
    this.entries.clear();
  }
}
