import { describe, expect, it } from "vitest";
import { flattenLayers, isLayerWritable, nextAutoName, type FlatEntry } from "./layer-state";
import type { Layer } from "../lib/types";

function at(result: FlatEntry[], i: number): FlatEntry {
  const entry = result[i];
  if (entry === undefined) throw new Error(`result[${i}] is undefined (length=${result.length})`);
  return entry;
}

// ── Fixtures ─────────────────────────────────────────────────────────────────

function raster(id: number, name: string, parent?: number): Layer {
  return {
    id,
    name,
    kind: { kind: "raster" },
    blend_mode: "normal",
    opacity: 255,
    visible: true,
    locked: false,
    parent: parent ?? null,
  };
}

function group(id: number, name: string, parent?: number): Layer {
  return {
    id,
    name,
    kind: { kind: "group", collapsed: false },
    blend_mode: "normal",
    opacity: 255,
    visible: true,
    locked: false,
    parent: parent ?? null,
  };
}

// All groups expanded by default in tests.
const allExpanded = () => true;
// No groups expanded.
const noneExpanded = () => false;

// ── flattenLayers ─────────────────────────────────────────────────────────────

describe("flattenLayers", () => {
  it("returns an empty list for an empty layer array", () => {
    expect(flattenLayers([], allExpanded)).toEqual([]);
  });

  it("returns a single raster layer at depth 0", () => {
    const layer = raster(1, "BG");
    const result = flattenLayers([layer], allExpanded);
    expect(result).toHaveLength(1);
    expect(at(result, 0).layer).toBe(layer);
    expect(at(result, 0).depth).toBe(0);
    expect(at(result, 0).index).toBe(0);
  });

  it("reverses Rust bottom-to-top order (topmost layer first in result)", () => {
    // Rust stores index 0 = bottom, index n-1 = top.
    const bottom = raster(1, "Bottom");
    const top = raster(2, "Top");
    // Rust order: [bottom, top]
    const result = flattenLayers([bottom, top], allExpanded);
    expect(at(result, 0).layer).toBe(top);
    expect(at(result, 1).layer).toBe(bottom);
  });

  it("assigns correct flat-list indices matching the original array", () => {
    const a = raster(1, "A");
    const b = raster(2, "B");
    const c = raster(3, "C");
    const result = flattenLayers([a, b, c], allExpanded);
    // Result is reversed (c first), but index should match the array position.
    const byName = Object.fromEntries(result.map((e) => [e.layer.name, e.index]));
    expect(byName["A"]).toBe(0);
    expect(byName["B"]).toBe(1);
    expect(byName["C"]).toBe(2);
  });

  it("nests children under their parent group and increments depth", () => {
    const g = group(1, "Group");
    const child = raster(2, "Child", 1);
    // Rust order: group at 0, child at 1 (but child has parent = group id)
    const result = flattenLayers([g, child], allExpanded);
    // group first (top level), then child (nested)
    expect(result).toHaveLength(2);
    expect(at(result, 0).layer).toBe(g);
    expect(at(result, 0).depth).toBe(0);
    expect(at(result, 1).layer).toBe(child);
    expect(at(result, 1).depth).toBe(1);
  });

  it("excludes group children when the group is collapsed", () => {
    const g = group(1, "Group");
    const child = raster(2, "Child", 1);
    const result = flattenLayers([g, child], noneExpanded);
    // Only the group itself is visible; children are hidden.
    expect(result).toHaveLength(1);
    expect(at(result, 0).layer).toBe(g);
  });

  it("handles nested groups correctly when all expanded", () => {
    const outer = group(1, "Outer");
    const inner = group(2, "Inner", 1);
    const leaf = raster(3, "Leaf", 2);
    const result = flattenLayers([outer, inner, leaf], allExpanded);
    expect(result).toHaveLength(3);
    expect(at(result, 0).layer).toBe(outer);
    expect(at(result, 0).depth).toBe(0);
    expect(at(result, 1).layer).toBe(inner);
    expect(at(result, 1).depth).toBe(1);
    expect(at(result, 2).layer).toBe(leaf);
    expect(at(result, 2).depth).toBe(2);
  });

  it("only collapses the specific group that is collapsed", () => {
    const groupA = group(1, "A");
    const groupB = group(2, "B");
    const childA = raster(3, "ChildA", 1);
    const childB = raster(4, "ChildB", 2);
    // Collapse only groupA.
    const expandFn = (id: number) => id !== 1;
    const result = flattenLayers([groupA, groupB, childA, childB], expandFn);
    const names = result.map((e) => e.layer.name);
    // groupB and childB visible; groupA visible but childA hidden.
    expect(names).toContain("A");
    expect(names).toContain("B");
    expect(names).toContain("ChildB");
    expect(names).not.toContain("ChildA");
  });

  it("renders multiple children of a group in reverse order within the group", () => {
    const g = group(1, "Group");
    const bottom = raster(2, "Bottom", 1);
    const top = raster(3, "Top", 1);
    // Rust order: g at 0, bottom at 1, top at 2.
    const result = flattenLayers([g, bottom, top], allExpanded);
    // group first, then children reversed: top before bottom
    expect(result).toHaveLength(3);
    expect(at(result, 0).layer).toBe(g);
    expect(at(result, 0).depth).toBe(0);
    expect(at(result, 1).layer).toBe(top);
    expect(at(result, 1).depth).toBe(1);
    expect(at(result, 2).layer).toBe(bottom);
    expect(at(result, 2).depth).toBe(1);
  });

  it("preserves top-level order even when groups have children", () => {
    // Two top-level layers: layer1 (bottom) and layer2 (top), no groups.
    const layer1 = raster(1, "Layer 1");
    const layer2 = raster(2, "Layer 2");
    const result = flattenLayers([layer1, layer2], allExpanded);
    // Top layer first.
    expect(at(result, 0).layer.name).toBe("Layer 2");
    expect(at(result, 1).layer.name).toBe("Layer 1");
  });
});

// ── nextAutoName ─────────────────────────────────────────────────────────────

describe("nextAutoName", () => {
  it("returns `<prefix> 1` when no layers match", () => {
    expect(nextAutoName([], "Layer")).toBe("Layer 1");
    expect(nextAutoName([raster(1, "Untitled")], "Layer")).toBe("Layer 1");
  });

  it("picks max + 1 for matching layers", () => {
    const list = [raster(1, "Layer 1"), raster(2, "Layer 2"), raster(3, "Layer 3")];
    expect(nextAutoName(list, "Layer")).toBe("Layer 4");
  });

  it("does not gap-fill after a delete", () => {
    // Simulating "Layer 2" was deleted from [1, 2, 3].
    const list = [raster(1, "Layer 1"), raster(3, "Layer 3")];
    expect(nextAutoName(list, "Layer")).toBe("Layer 4");
  });

  it("ignores names that don't match the prefix exactly", () => {
    const list = [
      raster(1, "Layer"), // no number
      raster(2, "Layerfoo 1"), // wrong prefix
      raster(3, "Layer 1 extra"), // suffix after number
      raster(4, "Layer 7"),
    ];
    expect(nextAutoName(list, "Layer")).toBe("Layer 8");
  });

  it("works for arbitrary prefixes (e.g. Group)", () => {
    const list = [group(1, "Group 1"), group(2, "Group 5"), raster(3, "Layer 99")];
    expect(nextAutoName(list, "Group")).toBe("Group 6");
  });

  it("ignores non-numeric trailing tokens", () => {
    const list = [raster(1, "Layer abc"), raster(2, "Layer NaN")];
    expect(nextAutoName(list, "Layer")).toBe("Layer 1");
  });

  it("treats the prefix as a literal (regex metacharacters are not specials)", () => {
    // `(RGB)` would explode if we built a RegExp from the prefix without
    // escaping; the literal-prefix implementation handles it cleanly.
    const list = [raster(1, "Image (RGB) 1"), raster(2, "Image (RGB) 4")];
    expect(nextAutoName(list, "Image (RGB)")).toBe("Image (RGB) 5");
  });

  it("rejects negative numbers and signs after the prefix", () => {
    const list = [raster(1, "Layer -3"), raster(2, "Layer +2")];
    // Neither matches `<prefix> <digits>` strictly.
    expect(nextAutoName(list, "Layer")).toBe("Layer 1");
  });
});

// ── isLayerWritable ──────────────────────────────────────────────────────────

describe("isLayerWritable", () => {
  function locked(l: Layer): Layer {
    return { ...l, locked: true };
  }

  it("returns true for an unlocked, parentless layer", () => {
    expect(isLayerWritable([raster(1, "BG")], 1)).toBe(true);
  });

  it("returns false for a directly locked layer", () => {
    expect(isLayerWritable([locked(raster(1, "BG"))], 1)).toBe(false);
  });

  it("returns false when an ancestor group is locked", () => {
    const g = locked(group(10, "fx"));
    const child = raster(11, "leaf", 10);
    expect(isLayerWritable([g, child], 11)).toBe(false);
  });

  it("returns false even when only a deep ancestor is locked", () => {
    const outer = locked(group(10, "outer"));
    const inner = group(20, "inner", 10);
    const leaf = raster(21, "leaf", 20);
    expect(isLayerWritable([outer, inner, leaf], 21)).toBe(false);
  });

  it("returns true when an unrelated layer is locked", () => {
    const target = raster(1, "target");
    const decoration = locked(raster(2, "decoration"));
    expect(isLayerWritable([target, decoration], 1)).toBe(true);
  });

  it("treats a missing layer id as writable (the IPC will surface the real error)", () => {
    expect(isLayerWritable([raster(1, "BG")], 99)).toBe(true);
  });
});
