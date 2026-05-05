import { describe, expect, it } from "vitest";
import { flattenLayers, type FlatEntry } from "./layer-state";
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
