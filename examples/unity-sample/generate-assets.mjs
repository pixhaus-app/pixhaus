#!/usr/bin/env node
/**
 * Generates synthetic PNG placeholder files for the Pixhaus Unity sample project.
 *
 * These PNGs are not real pixel art. They use solid-color tiles with a 1px
 * darker border so individual tiles are visually distinguishable in any
 * PNG viewer or Unity importer. The structure — frame layout, frame tags,
 * pivot slices — is the deliverable; replace the pixel data with final art
 * when the editor is ready.
 *
 * Usage:
 *   node generate-assets.mjs
 *
 * Requires Node.js 18+. No external dependencies.
 *
 * Outputs:
 *   Assets/Pixhaus/knight.png   — 256x160 (8 cols x 5 rows, 32x32 tiles, 33 frames)
 *   Assets/Pixhaus/slime.png    — 112x48  (7 cols x 3 rows, 16x16 tiles, 21 frames)
 *   Assets/Pixhaus/tileset.png  — 272x16  (17 tiles x 16px wide, 1 row)
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
 * @param {number} width
 * @param {number} height
 * @param {Buffer} pixels  width * height * 4 bytes, RGBA, top-to-bottom
 */
function makePng(width, height, pixels) {
  const PNG_SIGNATURE = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]);
  const ihdrData = Buffer.allocUnsafe(13);
  ihdrData.writeUInt32BE(width,  0);
  ihdrData.writeUInt32BE(height, 4);
  ihdrData[8]  = 8;  // bit depth
  ihdrData[9]  = 6;  // color type: RGBA
  ihdrData[10] = 0;  // compression: deflate
  ihdrData[11] = 0;  // filter: adaptive
  ihdrData[12] = 0;  // interlace: none

  const stride = width * 4;
  const raw = Buffer.allocUnsafe(height * (1 + stride));
  for (let y = 0; y < height; y++) {
    raw[y * (1 + stride)] = 0; // filter byte: None
    pixels.copy(raw, y * (1 + stride) + 1, y * stride, (y + 1) * stride);
  }

  return Buffer.concat([
    PNG_SIGNATURE,
    pngChunk('IHDR', ihdrData),
    pngChunk('IDAT', deflateSync(raw, { level: 6 })),
    pngChunk('IEND', Buffer.alloc(0)),
  ]);
}

/**
 * Fills a rectangular region of a pixel buffer with a solid color.
 * The outer 1px border of each tile is darkened so tiles are distinguishable.
 *
 * @param {Buffer} buf      destination pixel buffer (width * height * 4 bytes)
 * @param {number} bufW     buffer width in pixels
 * @param {number} tx       tile top-left x
 * @param {number} ty       tile top-left y
 * @param {number} tw       tile width
 * @param {number} th       tile height
 * @param {number[]} color  [R, G, B, A]
 */
function fillTile(buf, bufW, tx, ty, tw, th, color) {
  const [r, g, b, a] = color;
  for (let y = 0; y < th; y++) {
    for (let x = 0; x < tw; x++) {
      const edge = x === 0 || x === tw - 1 || y === 0 || y === th - 1;
      const i = ((ty + y) * bufW + (tx + x)) * 4;
      buf[i]     = edge ? Math.max(0, r - 40) : r;
      buf[i + 1] = edge ? Math.max(0, g - 40) : g;
      buf[i + 2] = edge ? Math.max(0, b - 40) : b;
      buf[i + 3] = a;
    }
  }
}

// --- Knight sprite sheet -------------------------------------------------------
// 256x160: 8 columns x 5 rows, 32x32 tiles, 33 frames.
//
// Animation layout (matches knight.pixhaussprite frameTags):
//   idle   (0-3):    warm white — resting knight
//   walk   (4-11):   steel blue — moving
//   run    (12-17):  cyan       — fast movement
//   attack (18-23):  red-orange — combat swing
//   hurt   (24-26):  deep red   — taking damage
//   death  (27-32):  dark grey  — falling
{
  const COLS = 8, ROWS = 5, TW = 32, TH = 32;
  const W = COLS * TW, H = ROWS * TH;
  const buf = Buffer.alloc(W * H * 4, 0);

  // Per-animation color palette (by frame index ranges).
  const animColors = [
    { from: 0,  to: 3,  color: [220, 210, 180, 240] }, // idle    — warm white
    { from: 4,  to: 11, color: [100, 140, 200, 240] }, // walk    — steel blue
    { from: 12, to: 17, color: [80,  200, 220, 240] }, // run     — cyan
    { from: 18, to: 23, color: [220, 100,  60, 240] }, // attack  — red-orange
    { from: 24, to: 26, color: [180,  50,  50, 240] }, // hurt    — deep red
    { from: 27, to: 32, color: [ 80,  80,  80, 240] }, // death   — dark grey
  ];

  function colorFor(frame) {
    for (const a of animColors) {
      if (frame >= a.from && frame <= a.to) return a.color;
    }
    return [60, 60, 60, 200];
  }

  for (let i = 0; i < 33; i++) {
    const col = i % COLS;
    const row = Math.floor(i / COLS);
    fillTile(buf, W, col * TW, row * TH, TW, TH, colorFor(i));
  }

  writeFileSync(join(__dir, 'Assets', 'Pixhaus', 'knight.png'), makePng(W, H, buf));
  console.log(`wrote Assets/Pixhaus/knight.png  (${W}x${H} placeholder, 33 frames at 32x32)`);
}

// --- Slime sprite sheet --------------------------------------------------------
// 112x48: 7 columns x 3 rows, 16x16 tiles, 21 frames.
//
// Animation layout (matches slime.pixhaussprite frameTags):
//   idle  (0-3):   lime green — resting slime
//   hop   (4-9):   teal       — hopping
//   hit   (10-12): orange-red — taking a hit
//   split (13-20): purple     — splitting animation
{
  const COLS = 7, ROWS = 3, TW = 16, TH = 16;
  const W = COLS * TW, H = ROWS * TH;
  const buf = Buffer.alloc(W * H * 4, 0);

  const animColors = [
    { from: 0,  to: 3,  color: [80,  200, 80,  240] }, // idle  — lime green
    { from: 4,  to: 9,  color: [60,  180, 180, 240] }, // hop   — teal
    { from: 10, to: 12, color: [220, 120, 60,  240] }, // hit   — orange-red
    { from: 13, to: 20, color: [140, 80,  200, 240] }, // split — purple
  ];

  function colorFor(frame) {
    for (const a of animColors) {
      if (frame >= a.from && frame <= a.to) return a.color;
    }
    return [60, 60, 60, 200];
  }

  for (let i = 0; i < 21; i++) {
    const col = i % COLS;
    const row = Math.floor(i / COLS);
    fillTile(buf, W, col * TW, row * TH, TW, TH, colorFor(i));
  }

  writeFileSync(join(__dir, 'Assets', 'Pixhaus', 'slime.png'), makePng(W, H, buf));
  console.log(`wrote Assets/Pixhaus/slime.png   (${W}x${H} placeholder, 21 frames at 16x16)`);
}

// --- Forest tileset ------------------------------------------------------------
// 272x16: 17 tiles x 16px wide, 1 row.
//
// Tile local ids 0-16 map to GIDs 1-17 (forest.tsx firstgid=1).
// Colors give each tile a distinct, recognizable hue. Real art replaces these.
{
  const TILE_COUNT = 17, TW = 16, TH = 16;
  const W = TILE_COUNT * TW, H = TH;
  const buf = Buffer.alloc(W * H * 4, 0);

  // One color per tile id (0-16).
  const tileColors = [
    [100, 180,  60, 220], //  0 grass          — bright green
    [160, 110,  60, 220], //  1 dirt            — warm brown
    [160, 160, 160, 220], //  2 stone floor     — light grey
    [ 60, 120, 220, 220], //  3 water-1         — vivid blue
    [ 80, 140, 230, 220], //  4 water-2         — lighter blue
    [100, 160, 240, 220], //  5 water-3         — lightest blue
    [ 40, 100,  40, 220], //  6 tree top        — dark green
    [ 80,  50,  30, 220], //  7 tree trunk      — dark brown
    [120, 200,  80, 220], //  8 tall grass      — medium green
    [130, 130, 130, 220], //  9 rock            — medium grey
    [240, 200, 100, 220], // 10 flowers         — yellow-white
    [120,  80,  40, 220], // 11 fence-h         — dark brown rail
    [100,  70,  35, 220], // 12 fence-v         — dark brown post
    [200, 160,  40, 220], // 13 chest-closed    — gold
    [200, 160,  40, 200], // 14 chest-open      — gold, slightly transparent
    [ 80,  80, 100, 220], // 15 wall stone      — dark grey-blue
    [140, 130, 120, 220], // 16 path stone      — medium stone
  ];

  for (let i = 0; i < TILE_COUNT; i++) {
    fillTile(buf, W, i * TW, 0, TW, TH, tileColors[i]);
  }

  writeFileSync(join(__dir, 'Assets', 'Pixhaus', 'tileset.png'), makePng(W, H, buf));
  console.log(`wrote Assets/Pixhaus/tileset.png (${W}x${H} placeholder, ${TILE_COUNT} tiles at 16x16)`);
}

console.log('done.');
