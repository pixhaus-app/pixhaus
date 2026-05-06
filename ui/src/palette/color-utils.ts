// Pure color-math utilities for the palette panel.
//
// No external dependencies — all conversions are implemented here so the
// palette panel has zero runtime weight from color libraries.

import type { Rgba } from "../lib/types";

// ── RGB ↔ HSV ─────────────────────────────────────────────────────────────────

export type Hsv = { h: number; s: number; v: number }; // h:0-360, s:0-1, v:0-1

export function rgbToHsv(r: number, g: number, b: number): Hsv {
  const rn = r / 255,
    gn = g / 255,
    bn = b / 255;
  const max = Math.max(rn, gn, bn),
    min = Math.min(rn, gn, bn),
    d = max - min;
  const v = max;
  const s = max === 0 ? 0 : d / max;
  let h = 0;
  if (d !== 0) {
    if (max === rn) h = ((gn - bn) / d) % 6;
    else if (max === gn) h = (bn - rn) / d + 2;
    else h = (rn - gn) / d + 4;
    h = (h * 60 + 360) % 360;
  }
  return { h, s, v };
}

export function hsvToRgb(h: number, s: number, v: number): { r: number; g: number; b: number } {
  const f = (n: number) => {
    const k = (n + h / 60) % 6;
    return Math.round((v - v * s * Math.max(0, Math.min(k, 4 - k, 1))) * 255);
  };
  return { r: f(5), g: f(3), b: f(1) };
}

// ── RGB ↔ HSL ─────────────────────────────────────────────────────────────────

export type Hsl = { h: number; s: number; l: number }; // h:0-360, s:0-1, l:0-1

export function rgbToHsl(r: number, g: number, b: number): Hsl {
  const rn = r / 255,
    gn = g / 255,
    bn = b / 255;
  const max = Math.max(rn, gn, bn),
    min = Math.min(rn, gn, bn);
  const l = (max + min) / 2;
  const d = max - min;
  const s = d === 0 ? 0 : d / (1 - Math.abs(2 * l - 1));
  let h = 0;
  if (d !== 0) {
    if (max === rn) h = ((gn - bn) / d) % 6;
    else if (max === gn) h = (bn - rn) / d + 2;
    else h = (rn - gn) / d + 4;
    h = (h * 60 + 360) % 360;
  }
  return { h, s, l };
}

export function hslToRgb(h: number, s: number, l: number): { r: number; g: number; b: number } {
  const f = (n: number) => {
    const k = (n + h / 30) % 12;
    const a = s * Math.min(l, 1 - l);
    return Math.round((l - a * Math.max(-1, Math.min(k - 3, 9 - k, 1))) * 255);
  };
  return { r: f(0), g: f(8), b: f(4) };
}

// ── Hex ───────────────────────────────────────────────────────────────────────

export function rgbToHex(r: number, g: number, b: number): string {
  return "#" + [r, g, b].map((x) => x.toString(16).padStart(2, "0")).join("");
}

export function hexToRgb(hex: string): { r: number; g: number; b: number } | null {
  let s = hex.replace(/^#/, "");
  if (s.length === 3)
    s = s
      .split("")
      .map((c) => c + c)
      .join("");
  if (!/^[0-9a-fA-F]{6}$/.test(s)) return null;
  const n = parseInt(s, 16);
  return { r: (n >> 16) & 0xff, g: (n >> 8) & 0xff, b: n & 0xff };
}

// ── Oklab / Oklch ─────────────────────────────────────────────────────────────
// Reference: https://bottosson.github.io/posts/oklab/

function lin(c: number): number {
  return c <= 0.04045 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4);
}

function delin(c: number): number {
  return c <= 0.0031308 ? c * 12.92 : 1.055 * Math.pow(c, 1 / 2.4) - 0.055;
}

export type Oklab = { L: number; a: number; b: number };
export type Oklch = { L: number; C: number; H: number }; // H: 0-360

export function rgbToOklab(r: number, g: number, b: number): Oklab {
  const rl = lin(r / 255),
    gl = lin(g / 255),
    bl = lin(b / 255);
  const l = Math.cbrt(0.4122214708 * rl + 0.5363325363 * gl + 0.0514459929 * bl);
  const m = Math.cbrt(0.2119034982 * rl + 0.6806995451 * gl + 0.1073969566 * bl);
  const s = Math.cbrt(0.0883024619 * rl + 0.2817188376 * gl + 0.6299787005 * bl);
  return {
    L: 0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s,
    a: 1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s,
    b: 0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s,
  };
}

export function oklabToRgb(L: number, a: number, b: number): { r: number; g: number; b: number } {
  const l = Math.pow(L + 0.3963377774 * a + 0.2158037573 * b, 3);
  const m = Math.pow(L - 0.1055613458 * a - 0.0638541728 * b, 3);
  const s = Math.pow(L - 0.0894841775 * a - 1.291485548 * b, 3);
  const rl = +4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s;
  const gl = -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s;
  const bl = -0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s;
  const clamp = (x: number) => Math.max(0, Math.min(255, Math.round(delin(x) * 255)));
  return { r: clamp(rl), g: clamp(gl), b: clamp(bl) };
}

export function rgbToOklch(r: number, g: number, b: number): Oklch {
  const lab = rgbToOklab(r, g, b);
  return {
    L: lab.L,
    C: Math.sqrt(lab.a * lab.a + lab.b * lab.b),
    H: ((Math.atan2(lab.b, lab.a) * 180) / Math.PI + 360) % 360,
  };
}

export function oklchToRgb(L: number, C: number, H: number): { r: number; g: number; b: number } {
  const rad = (H * Math.PI) / 180;
  return oklabToRgb(L, C * Math.cos(rad), C * Math.sin(rad));
}

// ── Interpolation ─────────────────────────────────────────────────────────────

/** Perceptually uniform interpolation between two RGBA colors via Oklab. */
export function interpolateOklab(colorA: Rgba, colorB: Rgba, t: number): Rgba {
  const la = rgbToOklab(colorA.r, colorA.g, colorA.b);
  const lb = rgbToOklab(colorB.r, colorB.g, colorB.b);
  const L = la.L + (lb.L - la.L) * t;
  const av = la.a + (lb.a - la.a) * t;
  const bv = la.b + (lb.b - la.b) * t;
  const { r, g, b } = oklabToRgb(L, av, bv);
  return { r, g, b, a: Math.round(colorA.a + (colorB.a - colorA.a) * t) };
}

// ── Harmony ───────────────────────────────────────────────────────────────────

export type HarmonyKind = "complement" | "split-complement" | "triad" | "tetrad" | "analogous";

export function harmonyColors(r: number, g: number, b: number, kind: HarmonyKind): Rgba[] {
  const { h, s, v } = rgbToHsv(r, g, b);
  const hue = (deg: number): Rgba => {
    const { r: hr, g: hg, b: hb } = hsvToRgb((((h + deg) % 360) + 360) % 360, s, v);
    return { r: hr, g: hg, b: hb, a: 255 };
  };
  switch (kind) {
    case "complement":
      return [hue(180)];
    case "split-complement":
      return [hue(150), hue(210)];
    case "triad":
      return [hue(120), hue(240)];
    case "tetrad":
      return [hue(90), hue(180), hue(270)];
    case "analogous":
      return [hue(-30), hue(30)];
  }
}

// ── Palette file format parsers / serializers ─────────────────────────────────

export type ParsedColor = { r: number; g: number; b: number; name?: string };

/** Parse a GIMP Palette (.gpl) text file. */
export function parseGpl(text: string): ParsedColor[] {
  const entries: ParsedColor[] = [];
  let inBody = false;
  for (const raw of text.split("\n")) {
    const line = raw.trim();
    if (line === "GIMP Palette") {
      inBody = true;
      continue;
    }
    if (
      !inBody ||
      !line ||
      line.startsWith("#") ||
      line.startsWith("Name:") ||
      line.startsWith("Columns:")
    )
      continue;
    const parts = line.split(/\s+/);
    if (parts.length < 3) continue;
    const [p0 = "", p1 = "", p2 = ""] = parts;
    const r = parseInt(p0, 10),
      g = parseInt(p1, 10),
      b = parseInt(p2, 10);
    if (isNaN(r) || isNaN(g) || isNaN(b)) continue;
    const nameStr = parts.slice(3).join(" ");
    entries.push(nameStr ? { r, g, b, name: nameStr } : { r, g, b });
  }
  return entries;
}

/** Serialize colors to GIMP Palette (.gpl) format. */
export function toGpl(
  name: string,
  colors: Array<{ r: number; g: number; b: number; name?: string | null }>,
): string {
  return (
    [
      "GIMP Palette",
      `Name: ${name}`,
      "Columns: 16",
      "#",
      ...colors.map(
        (c) =>
          `${String(c.r).padStart(3)} ${String(c.g).padStart(3)} ${String(c.b).padStart(3)}${c.name ? " " + c.name : ""}`,
      ),
    ].join("\n") + "\n"
  );
}

/** Parse a Lospec hex file — one 6-digit hex color per line. */
export function parseHexFile(text: string): ParsedColor[] {
  return text
    .split("\n")
    .map((l) => l.trim().replace(/^#/, ""))
    .filter((l) => /^[0-9a-fA-F]{6}$/.test(l))
    .map((hex) => {
      const n = parseInt(hex, 16);
      return { r: (n >> 16) & 0xff, g: (n >> 8) & 0xff, b: n & 0xff };
    });
}

/** Serialize colors to Lospec hex format. */
export function toHexFile(colors: Array<{ r: number; g: number; b: number }>): string {
  return (
    colors
      .map(
        (c) =>
          c.r.toString(16).padStart(2, "0") +
          c.g.toString(16).padStart(2, "0") +
          c.b.toString(16).padStart(2, "0"),
      )
      .join("\n") + "\n"
  );
}

/** Parse a JASC-PAL text file (the text variant of Windows .pal). */
export function parseJascPal(text: string): ParsedColor[] {
  const lines = text.split("\n").map((l) => l.trim());
  if (lines[0] !== "JASC-PAL") return [];
  const count = parseInt(lines[2] ?? "0", 10);
  const entries: ParsedColor[] = [];
  for (let i = 0; i < count; i++) {
    const parts = (lines[3 + i] ?? "").split(/\s+/);
    if (parts.length < 3) continue;
    const [q0 = "", q1 = "", q2 = ""] = parts;
    const r = parseInt(q0, 10),
      g = parseInt(q1, 10),
      b = parseInt(q2, 10);
    if (!isNaN(r)) entries.push({ r, g, b });
  }
  return entries;
}

/** Serialize colors to JASC-PAL format. */
export function toJascPal(colors: Array<{ r: number; g: number; b: number }>): string {
  return (
    ["JASC-PAL", "0100", String(colors.length), ...colors.map((c) => `${c.r} ${c.g} ${c.b}`)].join(
      "\n",
    ) + "\n"
  );
}

// ACO color space codes
const ACO_CS_RGB = 0;
const ACO_CS_HSB = 1;
const ACO_CS_CMYK = 2;
const ACO_CS_LAB = 7;
const ACO_CS_GRAYSCALE = 8;

function decodeAcoColor(
  view: DataView,
  offset: number,
): { r: number; g: number; b: number } | null {
  const cs = view.getUint16(offset, false);
  const c1 = view.getUint16(offset + 2, false);
  const c2 = view.getUint16(offset + 4, false);
  const c3 = view.getUint16(offset + 6, false);
  switch (cs) {
    case ACO_CS_RGB:
      // Components in [0, 65535]; divide by 257 to get [0, 255]
      return { r: Math.floor(c1 / 257), g: Math.floor(c2 / 257), b: Math.floor(c3 / 257) };
    case ACO_CS_GRAYSCALE: {
      // c1 in [0, 10000]; scale to [0, 255]
      const gray = Math.min(255, Math.floor((c1 * 255) / 10000));
      return { r: gray, g: gray, b: gray };
    }
    case ACO_CS_HSB:
    case ACO_CS_CMYK:
    case ACO_CS_LAB:
      return null;
    default:
      throw new Error(`ACO: unsupported color space ${cs}`);
  }
}

function readAcoV1(view: DataView, start: number, count: number): ParsedColor[] {
  const colors: ParsedColor[] = [];
  let offset = start;
  for (let i = 0; i < count; i++) {
    if (offset + 10 > view.byteLength) {
      throw new Error("ACO v1: file ends mid-entry; declared count is larger than payload");
    }
    const color = decodeAcoColor(view, offset);
    offset += 10;
    if (color !== null) colors.push(color);
  }
  return colors;
}

function readAcoV2(view: DataView, start: number, count: number): ParsedColor[] {
  const colors: ParsedColor[] = [];
  let offset = start;
  for (let i = 0; i < count; i++) {
    if (offset + 10 > view.byteLength) {
      throw new Error("ACO v2: file ends mid-entry; declared count is larger than payload");
    }
    const color = decodeAcoColor(view, offset);
    offset += 10;

    // Name field: 4-byte char count (UTF-16BE chars) + count * 2 bytes
    if (offset + 4 > view.byteLength) {
      throw new Error("ACO v2: file ends before name length field");
    }
    const nameLen = view.getUint32(offset, false);
    const nameBytes = nameLen * 2;
    offset += 4;
    if (offset + nameBytes > view.byteLength) {
      throw new Error("ACO v2: file ends mid-name");
    }

    let name: string | undefined;
    if (nameLen > 0) {
      const chars: number[] = [];
      for (let j = 0; j < nameLen; j++) {
        const cp = view.getUint16(offset + j * 2, false);
        if (cp !== 0) chars.push(cp);
      }
      if (chars.length > 0) name = String.fromCharCode(...chars);
    }
    offset += nameBytes;

    if (color !== null) colors.push(name ? { ...color, name } : color);
  }
  return colors;
}

/** Parse an Adobe Photoshop Color Swatch (.aco) binary file. */
export function parseAco(buffer: ArrayBuffer): ParsedColor[] {
  const view = new DataView(buffer);
  if (buffer.byteLength < 4) throw new Error("ACO: file too short");
  const version = view.getUint16(0, false);
  const count = view.getUint16(2, false);
  if (version === 1) return readAcoV1(view, 4, count);
  if (version === 2) return readAcoV2(view, 4, count);
  throw new Error(`ACO: unsupported version ${version}`);
}

// ── CSS helpers ───────────────────────────────────────────────────────────────

export function rgbaToCss(c: Rgba): string {
  return `rgba(${c.r}, ${c.g}, ${c.b}, ${(c.a / 255).toFixed(3)})`;
}

/** Returns white or black depending on which has better contrast with `c`. */
export function contrastColor(c: Rgba): string {
  // Perceived luminance via ITU-R BT.601
  const lum = (0.299 * c.r + 0.587 * c.g + 0.114 * c.b) / 255;
  return lum > 0.55 ? "#000" : "#fff";
}
