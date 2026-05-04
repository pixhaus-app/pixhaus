#!/usr/bin/env node
/**
 * Generates synthetic PNG placeholder files for the unity-handoff reference exports.
 *
 * These PNGs are not real pixel art — they are minimal valid RGBA images that
 * allow importers and test harnesses to load the reference JSON/TMX files without
 * a missing-file error. The exporter (S10) will produce real PNGs from actual
 * Pixhaus sprite data; these placeholders exist only to make the reference exports
 * self-contained.
 *
 * Usage:
 *   node generate-pngs.mjs
 *
 * Requires Node.js 18+. No external dependencies.
 */

import { deflateSync } from 'zlib';
import { writeFileSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';

const __dir = dirname(fileURLToPath(import.meta.url));

// --- PNG primitives ----------------------------------------------------------

const CRC_TABLE = (() => {
  const t = new Uint32Array(256);
  for (let i = 0; i < 256; i++) {
    let c = i;
    for (let k = 0; k < 8; k++) c = c & 1 ? (0xedb88320 ^ (c >>> 1)) : (c >>> 1);
    t[i] = c >>> 0;
  }
  return t;
})();

function crc32(buf) {
  let c = 0xffffffff;
  for (const b of buf) c = (CRC_TABLE[(c ^ b) & 0xff] ^ (c >>> 8)) >>> 0;
  return (c ^ 0xffffffff) >>> 0;
}

function u32be(n) {
  const b = Buffer.allocUnsafe(4);
  b.writeUInt32BE(n >>> 0, 0);
  return b;
}

function pngChunk(type, data) {
  const t = Buffer.from(type, 'ascii');
  const crc = crc32(Buffer.concat([t, data]));
  return Buffer.concat([u32be(data.length), t, data, u32be(crc)]);
}

/**
 * Builds a minimal RGBA PNG from a flat pixel buffer.
 *
 * @param {number} width
 * @param {number} height
 * @param {Buffer} pixels  width * height * 4 bytes, RGBA, top-to-bottom
 * @returns {Buffer}
 */
function makePng(width, height, pixels) {
  const PNG_SIGNATURE = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]);

  const ihdrData = Buffer.allocUnsafe(13);
  ihdrData.writeUInt32BE(width,  0);
  ihdrData.writeUInt32BE(height, 4);
  ihdrData[8]  = 8; // bit depth
  ihdrData[9]  = 6; // color type: RGBA
  ihdrData[10] = 0; // compression method: deflate
  ihdrData[11] = 0; // filter method: adaptive
  ihdrData[12] = 0; // interlace: none

  // Build raw scanlines: each row prefixed with filter byte 0 (None).
  const stride = width * 4;
  const raw = Buffer.allocUnsafe(height * (1 + stride));
  for (let y = 0; y < height; y++) {
    raw[y * (1 + stride)] = 0;
    pixels.copy(raw, y * (1 + stride) + 1, y * stride, (y + 1) * stride);
  }

  const idat = deflateSync(raw, { level: 6 });

  return Buffer.concat([
    PNG_SIGNATURE,
    pngChunk('IHDR', ihdrData),
    pngChunk('IDAT', idat),
    pngChunk('IEND', Buffer.alloc(0)),
  ]);
}

/**
 * Fills a pixel buffer with a checkerboard so the image is visually
 * distinguishable from a blank frame while remaining obviously synthetic.
 *
 * @param {number} width
 * @param {number} height
 * @param {number} cellSize  size of each checker square in pixels
 * @param {number[]} colorA  [R, G, B, A]
 * @param {number[]} colorB  [R, G, B, A]
 * @returns {Buffer}
 */
function checkerboard(width, height, cellSize, colorA, colorB) {
  const buf = Buffer.allocUnsafe(width * height * 4);
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      const cell = (Math.floor(x / cellSize) + Math.floor(y / cellSize)) & 1;
      const [r, g, b, a] = cell === 0 ? colorA : colorB;
      const i = (y * width + x) * 4;
      buf[i]     = r;
      buf[i + 1] = g;
      buf[i + 2] = b;
      buf[i + 3] = a;
    }
  }
  return buf;
}

// --- Generate hero.png -------------------------------------------------------
// 64×16: four 16×16 frames laid out horizontally.
// Checkerboard with semi-transparent magenta/transparent cells so it's clearly
// a placeholder when viewed in any PNG viewer or Unity importer.
{
  const w = 64, h = 16;
  const pixels = checkerboard(w, h, 4,
    [220, 60, 180, 200],  // semi-transparent magenta
    [0,   0,  0,   0],    // transparent
  );
  writeFileSync(join(__dir, 'simple-sprite', 'hero.png'), makePng(w, h, pixels));
  console.log('wrote simple-sprite/hero.png  (64×16 RGBA checkerboard placeholder)');
}

// --- Generate dungeon.png ----------------------------------------------------
// 96×16: six 16×16 tiles in a single row (tile 0 transparent, tiles 1–5 tinted).
// Each tile column is a distinct color to make the atlas layout obvious.
{
  const tileW = 16, tileH = 16, tileCount = 6;
  const w = tileW * tileCount, h = tileH;
  const pixels = Buffer.alloc(w * h * 4, 0); // start fully transparent

  // Tile palettes: [R, G, B, A]. Tile 0 stays transparent.
  const tileColors = [
    [0,   0,   0,   0  ],  // 0: empty — fully transparent
    [100, 100, 100, 220],  // 1: floor — mid grey
    [60,  60,  80,  220],  // 2: wall — dark blue-grey
    [80,  80,  100, 220],  // 3: corner — slightly lighter
    [220, 160, 40,  220],  // 4: torch — warm orange
    [120, 80,  40,  220],  // 5: chest — brown
  ];

  for (let tile = 0; tile < tileCount; tile++) {
    const [r, g, b, a] = tileColors[tile];
    for (let y = 0; y < tileH; y++) {
      for (let x = 0; x < tileW; x++) {
        const px = tile * tileW + x;
        const i = (y * w + px) * 4;
        // Darken edges so individual tiles are visually bounded.
        const edge = (x === 0 || x === tileW - 1 || y === 0 || y === tileH - 1) && tile > 0;
        pixels[i]     = edge ? Math.max(0, r - 40) : r;
        pixels[i + 1] = edge ? Math.max(0, g - 40) : g;
        pixels[i + 2] = edge ? Math.max(0, b - 40) : b;
        pixels[i + 3] = a;
      }
    }
  }

  writeFileSync(join(__dir, 'tilemap', 'dungeon.png'), makePng(w, h, pixels));
  console.log('wrote tilemap/dungeon.png     (96×16 RGBA tileset placeholder, 6 tiles)');
}

console.log('done.');
