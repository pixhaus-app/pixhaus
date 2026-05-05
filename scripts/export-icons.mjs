/**
 * export-icons.mjs
 *
 * Generates app/icons/ PNG rasters from the pixel-house symbol. The
 * geometry lives in the GRID constant below; brand/logo-symbol.svg
 * mirrors it for vector consumers (web, docs, README). The two are
 * kept in sync by hand — if you change one, change the other.
 *
 * All sizes are rendered from the same integer cell grid so scaling
 * never introduces sub-pixel blur.
 *
 * Run via:  node scripts/run.mjs export-icons
 * Or directly: node scripts/export-icons.mjs
 *
 * Outputs:
 *   app/icons/32x32.png        — 32x32   (toolbar, taskbar)
 *   app/icons/128x128.png      — 128x128 (desktop, macOS Dock)
 *   app/icons/128x128@2x.png   — 256x256 (Retina at 128 logical)
 *   app/icons/icon.png         — 512x512 (Linux, source for ICO/ICNS)
 *
 * After running this script, regenerate app/icons/icon.ico and icon.icns
 * with a separate tool (ImageMagick, electron-icon-maker, or tauri-cli):
 *   convert app/icons/icon.png -define icon:auto-resize app/icons/icon.ico
 */

import { deflateSync } from "node:zlib";
import { writeFileSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const REPO_ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
const ICONS_DIR = join(REPO_ROOT, "app", "icons");

// Pixhaus Indigo #7c6cef — brand primary, opaque.
const BRAND = [0x7c, 0x6c, 0xef, 0xff];
const CLEAR = [0x00, 0x00, 0x00, 0x00];

// 5x5 occupancy map: 1 = filled pixel cell.
// Matches the rect layout in brand/logo-symbol.svg.
const GRID = [
  [0, 0, 1, 0, 0], // peak
  [0, 1, 1, 1, 0], // upper roof
  [1, 1, 1, 1, 1], // eave
  [1, 1, 0, 1, 1], // walls (door gap at col 2)
  [1, 1, 0, 1, 1], // walls (door gap at col 2)
];

/** Render the 5x5 grid into a flat RGBA Uint8ClampedArray of size×size pixels. */
function rasterize(size) {
  // Largest integer cell size so 5 cells fit. When (size - used) is odd
  // we can't split the leftover evenly; bias the extra pixel to the
  // right/bottom so the visual offset is consistent across sizes.
  const cellSize = Math.floor(size / 5);
  const used = 5 * cellSize;
  const slack = size - used;
  const padLeft = Math.floor(slack / 2);
  // padRight = slack - padLeft (unused but kept for clarity)

  const buf = new Uint8ClampedArray(size * size * 4);

  for (let row = 0; row < 5; row++) {
    for (let col = 0; col < 5; col++) {
      const color = GRID[row][col] ? BRAND : CLEAR;
      const ox = padLeft + col * cellSize;
      const oy = padLeft + row * cellSize;
      for (let dy = 0; dy < cellSize; dy++) {
        for (let dx = 0; dx < cellSize; dx++) {
          const x = ox + dx;
          const y = oy + dy;
          if (x >= size || y >= size) continue;
          const i = (y * size + x) * 4;
          buf[i]     = color[0];
          buf[i + 1] = color[1];
          buf[i + 2] = color[2];
          buf[i + 3] = color[3];
        }
      }
    }
  }

  return buf;
}

function crc32(buf) {
  let crc = 0xffffffff;
  for (const byte of buf) {
    crc ^= byte;
    for (let k = 0; k < 8; k++) {
      crc = crc & 1 ? (0xedb88320 ^ (crc >>> 1)) : crc >>> 1;
    }
  }
  return (crc ^ 0xffffffff) >>> 0;
}

function pngChunk(type, data) {
  const typeBuf = Buffer.from(type, "ascii");
  const lenBuf = Buffer.alloc(4);
  lenBuf.writeUInt32BE(data.length);
  const crcInput = Buffer.concat([typeBuf, data]);
  const crcBuf = Buffer.alloc(4);
  crcBuf.writeUInt32BE(crc32(crcInput));
  return Buffer.concat([lenBuf, typeBuf, data, crcBuf]);
}

/** Encode RGBA pixel data into a valid PNG buffer. */
function encodePng(size, pixels) {
  // Scanline data: filter-byte 0 (None) + raw RGBA row.
  const rows = [];
  for (let y = 0; y < size; y++) {
    const row = Buffer.alloc(1 + size * 4);
    row[0] = 0; // filter: None
    for (let x = 0; x < size; x++) {
      const s = (y * size + x) * 4;
      row[1 + x * 4]     = pixels[s];
      row[1 + x * 4 + 1] = pixels[s + 1];
      row[1 + x * 4 + 2] = pixels[s + 2];
      row[1 + x * 4 + 3] = pixels[s + 3];
    }
    rows.push(row);
  }
  const raw = Buffer.concat(rows);
  const compressed = deflateSync(raw, { level: 9 });

  const sig = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]);

  const ihdrData = Buffer.alloc(13);
  ihdrData.writeUInt32BE(size, 0);
  ihdrData.writeUInt32BE(size, 4);
  ihdrData[8]  = 8; // bit depth
  ihdrData[9]  = 6; // color type: RGBA
  ihdrData[10] = 0; // compression: deflate
  ihdrData[11] = 0; // filter: adaptive
  ihdrData[12] = 0; // interlace: none
  const ihdr = pngChunk("IHDR", ihdrData);

  const idat = pngChunk("IDAT", compressed);
  const iend = pngChunk("IEND", Buffer.alloc(0));

  return Buffer.concat([sig, ihdr, idat, iend]);
}

const OUTPUTS = [
  { file: "32x32.png",       size: 32  },
  { file: "128x128.png",     size: 128 },
  { file: "128x128@2x.png",  size: 256 },
  { file: "icon.png",        size: 512 },
];

console.log("export-icons: rendering pixel-house symbol");
for (const { file, size } of OUTPUTS) {
  const pixels = rasterize(size);
  const png = encodePng(size, pixels);
  const dest = join(ICONS_DIR, file);
  writeFileSync(dest, png);
  console.log(`  ${file} (${size}x${size}, ${png.length} bytes)`);
}
console.log(
  "\nexport-icons: done — regenerate icon.ico / icon.icns from icon.png separately:\n" +
  "  convert app/icons/icon.png -define icon:auto-resize=256,128,64,48,32,16 app/icons/icon.ico"
);
