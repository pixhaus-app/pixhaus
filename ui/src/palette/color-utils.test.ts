import { describe, expect, it } from "vitest";
import { parseAco } from "./color-utils";

// ── ACO binary builder helpers ────────────────────────────────────────────────

function u16be(n: number): [number, number] {
  return [(n >> 8) & 0xff, n & 0xff];
}

function u32be(n: number): [number, number, number, number] {
  return [(n >> 24) & 0xff, (n >> 16) & 0xff, (n >> 8) & 0xff, n & 0xff];
}

/** Build a version-1 ACO buffer from raw color entries. */
function buildV1(entries: Array<[cs: number, c1: number, c2: number, c3: number]>): ArrayBuffer {
  const bytes: number[] = [
    ...u16be(1), // version
    ...u16be(entries.length), // count
  ];
  for (const [cs, c1, c2, c3] of entries) {
    bytes.push(...u16be(cs), ...u16be(c1), ...u16be(c2), ...u16be(c3), 0, 0); // c4 = 0
  }
  return new Uint8Array(bytes).buffer;
}

/** Build a version-2 ACO buffer. Each entry pairs color data with an optional name string. */
function buildV2(
  entries: Array<[cs: number, c1: number, c2: number, c3: number, name?: string]>,
): ArrayBuffer {
  const bytes: number[] = [
    ...u16be(2), // version
    ...u16be(entries.length), // count
  ];
  for (const [cs, c1, c2, c3, name] of entries) {
    bytes.push(...u16be(cs), ...u16be(c1), ...u16be(c2), ...u16be(c3), 0, 0);
    const chars = name ? [...name].map((c) => c.charCodeAt(0)) : [];
    // Include null terminator in the length (matches Photoshop convention)
    const len = chars.length + 1;
    bytes.push(...u32be(len));
    for (const cp of chars) bytes.push(...u16be(cp));
    bytes.push(0, 0); // null terminator
  }
  return new Uint8Array(bytes).buffer;
}

// ── Color space codes ─────────────────────────────────────────────────────────

const CS_RGB = 0;
const CS_HSB = 1;
const CS_CMYK = 2;
const CS_LAB = 7;
const CS_GRAYSCALE = 8;

// ── v1 tests ──────────────────────────────────────────────────────────────────

describe("parseAco v1", () => {
  it("parses an RGB swatch — full white", () => {
    const buf = buildV1([[CS_RGB, 65535, 65535, 65535]]);
    const colors = parseAco(buf);
    expect(colors).toHaveLength(1);
    expect(colors[0]).toMatchObject({ r: 255, g: 255, b: 255 });
  });

  it("parses an RGB swatch — primary red", () => {
    const buf = buildV1([[CS_RGB, 65535, 0, 0]]);
    const colors = parseAco(buf);
    expect(colors[0]).toMatchObject({ r: 255, g: 0, b: 0 });
  });

  it("parses a grayscale swatch", () => {
    // c1 = 5000 out of 10000 → gray ≈ 127
    const buf = buildV1([[CS_GRAYSCALE, 5000, 0, 0]]);
    const colors = parseAco(buf);
    const c = colors[0];
    expect(c).toBeDefined();
    expect(c!.r).toBe(c!.g);
    expect(c!.g).toBe(c!.b);
    expect(c!.r).toBeGreaterThan(0);
  });

  it("skips HSB entries", () => {
    const buf = buildV1([
      [CS_HSB, 0, 0, 0],
      [CS_RGB, 0, 0, 0],
    ]);
    expect(parseAco(buf)).toHaveLength(1);
  });

  it("skips CMYK entries", () => {
    const buf = buildV1([[CS_CMYK, 0, 0, 0]]);
    expect(parseAco(buf)).toHaveLength(0);
  });

  it("skips Lab entries", () => {
    const buf = buildV1([[CS_LAB, 0, 0, 0]]);
    expect(parseAco(buf)).toHaveLength(0);
  });

  it("throws on unknown color space", () => {
    const buf = buildV1([[999, 0, 0, 0]]);
    expect(() => parseAco(buf)).toThrow();
  });

  it("throws when file is too short", () => {
    expect(() => parseAco(new Uint8Array([0, 1, 0]).buffer)).toThrow();
  });

  it("throws on unsupported version", () => {
    const buf = buildV1([]);
    // Overwrite version bytes with 3
    new DataView(buf).setUint16(0, 3, false);
    expect(() => parseAco(buf)).toThrow();
  });

  it("throws when declared count exceeds payload (no silent truncation)", () => {
    const buf = buildV1([
      [CS_RGB, 0, 0, 0],
      [CS_RGB, 0, 0, 0],
    ]);
    // Bump count to 3 without adding a third entry
    new DataView(buf).setUint16(2, 3, false);
    expect(() => parseAco(buf)).toThrow();
  });
});

// ── v2 tests ──────────────────────────────────────────────────────────────────

describe("parseAco v2", () => {
  it("parses colors and extracts names", () => {
    const buf = buildV2([[CS_RGB, 65535, 0, 0, "Red"]]);
    const colors = parseAco(buf);
    expect(colors[0]).toMatchObject({ r: 255, g: 0, b: 0, name: "Red" });
  });

  it("omits name field when entry has no name content", () => {
    const buf = buildV2([[CS_RGB, 0, 65535, 0, ""]]);
    const colors = parseAco(buf);
    expect(colors[0]?.name).toBeUndefined();
  });

  it("parses multiple named swatches", () => {
    const buf = buildV2([
      [CS_RGB, 65535, 0, 0, "Red"],
      [CS_RGB, 0, 65535, 0, "Green"],
      [CS_RGB, 0, 0, 65535, "Blue"],
    ]);
    const colors = parseAco(buf);
    expect(colors).toHaveLength(3);
    expect(colors[0]?.name).toBe("Red");
    expect(colors[1]?.name).toBe("Green");
    expect(colors[2]?.name).toBe("Blue");
  });

  it("skips non-RGB entries", () => {
    const buf = buildV2([
      [CS_HSB, 0, 0, 0, "hsb"],
      [CS_RGB, 0, 0, 0, "black"],
    ]);
    expect(parseAco(buf)).toHaveLength(1);
  });

  it("throws when file ends before name length field", () => {
    // Build a valid v2 entry but truncate before the name length bytes
    const buf = buildV2([[CS_RGB, 0, 0, 0, "x"]]);
    // Truncate to 4 (header) + 10 (color entry) = 14 bytes — before name len
    const truncated = buf.slice(0, 14);
    expect(() => parseAco(truncated)).toThrow();
  });
});
