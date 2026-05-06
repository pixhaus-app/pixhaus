import { describe, expect, it } from "vitest";
import { buildCelPresence, buildSwapPairs, genUniqueTagName } from "./timeline-state";
import type { Cel } from "../lib/types";

// ── Fixtures ──────────────────────────────────────────────────────────────────

function cel(layerId: number, frameIndex: number): Cel {
  return {
    layer_id: layerId,
    frame_index: frameIndex,
    position: { x: 0, y: 0 },
    opacity: 255,
    data: { kind: "linked", source_frame: 0 },
  };
}

// ── genUniqueTagName ──────────────────────────────────────────────────────────

describe("genUniqueTagName", () => {
  it("returns Tag 1 for an empty name set", () => {
    expect(genUniqueTagName(new Set())).toBe("Tag 1");
  });

  it("skips names already in the set", () => {
    expect(genUniqueTagName(new Set(["Tag 1"]))).toBe("Tag 2");
    expect(genUniqueTagName(new Set(["Tag 1", "Tag 2"]))).toBe("Tag 3");
  });

  it("fills the first gap rather than always appending", () => {
    expect(genUniqueTagName(new Set(["Tag 1", "Tag 3"]))).toBe("Tag 2");
  });

  it("ignores non-sequential names", () => {
    expect(genUniqueTagName(new Set(["Walk", "Run"]))).toBe("Tag 1");
  });
});

// ── buildCelPresence ──────────────────────────────────────────────────────────

describe("buildCelPresence", () => {
  it("returns an empty map for an empty cel list", () => {
    expect(buildCelPresence([])).toEqual(new Map());
  });

  it("maps a single cel to its layer and frame", () => {
    const m = buildCelPresence([cel(1, 0)]);
    expect(m.get(1)?.has(0)).toBe(true);
  });

  it("groups multiple cels from the same layer together", () => {
    const m = buildCelPresence([cel(1, 0), cel(1, 2), cel(1, 5)]);
    const set = m.get(1);
    expect(set?.has(0)).toBe(true);
    expect(set?.has(2)).toBe(true);
    expect(set?.has(5)).toBe(true);
    expect(m.size).toBe(1);
  });

  it("creates separate sets for different layers", () => {
    const m = buildCelPresence([cel(1, 0), cel(2, 0), cel(3, 1)]);
    expect(m.size).toBe(3);
    expect(m.get(1)?.has(0)).toBe(true);
    expect(m.get(2)?.has(0)).toBe(true);
    expect(m.get(3)?.has(1)).toBe(true);
  });

  it("does not mark absent (layer, frame) pairs as present", () => {
    const m = buildCelPresence([cel(1, 0)]);
    expect(m.get(1)?.has(1)).toBe(false);
    expect(m.get(2)).toBeUndefined();
  });
});

// ── buildSwapPairs ────────────────────────────────────────────────────────────

describe("buildSwapPairs", () => {
  it("returns no pairs for an empty selection", () => {
    expect(buildSwapPairs(new Set())).toEqual([]);
  });

  it("returns no pairs for a single-frame selection", () => {
    expect(buildSwapPairs(new Set([3]))).toEqual([]);
  });

  it("returns one swap pair for two frames", () => {
    expect(buildSwapPairs(new Set([1, 4]))).toEqual([[1, 4]]);
  });

  it("reverses three frames with one swap pair (middle stays in place)", () => {
    // Frames [0,1,2]: outer pair [0,2] swaps; middle is left untouched.
    expect(buildSwapPairs(new Set([0, 1, 2]))).toEqual([[0, 2]]);
  });

  it("reverses four frames with two swap pairs", () => {
    expect(buildSwapPairs(new Set([0, 1, 2, 3]))).toEqual([
      [0, 3],
      [1, 2],
    ]);
  });

  it("works on a non-contiguous selection", () => {
    // Selection [0, 2, 5, 7] reverses to [7, 5, 2, 0]: pairs (0,7) and (2,5).
    expect(buildSwapPairs(new Set([0, 2, 5, 7]))).toEqual([
      [0, 7],
      [2, 5],
    ]);
  });

  it("is order-independent (unsorted input produces the same pairs)", () => {
    const sorted = buildSwapPairs(new Set([0, 1, 2, 3]));
    const unsorted = buildSwapPairs(new Set([3, 1, 0, 2]));
    expect(unsorted).toEqual(sorted);
  });
});
