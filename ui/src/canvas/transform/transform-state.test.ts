import { describe, it, expect } from "vitest";
import {
  applyHandleDelta,
  syncNumericFromBounds,
  numericX,
  numericY,
  numericW,
  numericH,
} from "./transform-state";
import type { TransformArgs, TransformOp } from "../../lib/commands/canvas";

const BASE = { x: 10, y: 20, width: 100, height: 80 };

describe("applyHandleDelta — body (translate)", () => {
  it("translates by dx, dy", () => {
    const r = applyHandleDelta("body", BASE, 5, -3);
    expect(r).toEqual({ x: 15, y: 17, width: 100, height: 80 });
  });
});

describe("applyHandleDelta — corner handles (scale)", () => {
  it("nw: moves origin and shrinks size", () => {
    const r = applyHandleDelta("nw", BASE, 10, 5);
    expect(r).toEqual({ x: 20, y: 25, width: 90, height: 75 });
  });

  it("se: keeps origin and grows size", () => {
    const r = applyHandleDelta("se", BASE, 10, 5);
    expect(r).toEqual({ x: 10, y: 20, width: 110, height: 85 });
  });

  it("ne: moves y, grows width, shrinks height", () => {
    const r = applyHandleDelta("ne", BASE, 10, 5);
    expect(r).toEqual({ x: 10, y: 25, width: 110, height: 75 });
  });

  it("sw: moves x, shrinks width, grows height", () => {
    const r = applyHandleDelta("sw", BASE, 10, 5);
    expect(r).toEqual({ x: 20, y: 20, width: 90, height: 85 });
  });
});

describe("applyHandleDelta — edge handles", () => {
  it("n: moves y, shrinks height", () => {
    const r = applyHandleDelta("n", BASE, 0, 5);
    expect(r).toEqual({ x: 10, y: 25, width: 100, height: 75 });
  });

  it("s: grows height", () => {
    const r = applyHandleDelta("s", BASE, 0, 5);
    expect(r).toEqual({ x: 10, y: 20, width: 100, height: 85 });
  });

  it("w: moves x, shrinks width", () => {
    const r = applyHandleDelta("w", BASE, 10, 0);
    expect(r).toEqual({ x: 20, y: 20, width: 90, height: 80 });
  });

  it("e: grows width", () => {
    const r = applyHandleDelta("e", BASE, 10, 0);
    expect(r).toEqual({ x: 10, y: 20, width: 110, height: 80 });
  });
});

describe("applyHandleDelta — clamps to minimum size", () => {
  it("nw: width never drops below 1", () => {
    const r = applyHandleDelta("nw", { x: 0, y: 0, width: 5, height: 5 }, 200, 200);
    expect(r.width).toBeGreaterThanOrEqual(1);
    expect(r.height).toBeGreaterThanOrEqual(1);
  });
});

describe("applyHandleDelta — rotate stub", () => {
  it("returns original bounds unchanged", () => {
    const r = applyHandleDelta("rotate", BASE, 10, 10);
    expect(r).toEqual(BASE);
  });
});

describe("syncNumericFromBounds", () => {
  it("rounds and writes to signals", () => {
    syncNumericFromBounds({ x: 1.6, y: 2.3, width: 99.9, height: 40.1 });
    expect(numericX()).toBe(2);
    expect(numericY()).toBe(2);
    expect(numericW()).toBe(100);
    expect(numericH()).toBe(40);
  });
});

// The Rust TransformArgs uses #[serde(tag = "kind", rename_all = "snake_case")]
// for its op enum. The JSON produced from the TS shape must therefore carry a
// `kind` discriminator with snake_case variant names, and shape-specific fields
// at the same nesting level.
describe("TransformArgs serialisation", () => {
  function serialise(op: TransformOp): Record<string, unknown> {
    const args: TransformArgs = {
      sprite_id: 1,
      layer_id: 2,
      frame_index: 0,
      ops: [op],
    };
    return JSON.parse(JSON.stringify(args)) as Record<string, unknown>;
  }

  it("serialises translate with kind + dx + dy", () => {
    const out = serialise({ kind: "translate", dx: 5, dy: -3 });
    expect(out.ops).toEqual([{ kind: "translate", dx: 5, dy: -3 }]);
  });

  it("serialises flip variants with no extra fields", () => {
    expect(serialise({ kind: "flip_horizontal" }).ops).toEqual([{ kind: "flip_horizontal" }]);
    expect(serialise({ kind: "flip_vertical" }).ops).toEqual([{ kind: "flip_vertical" }]);
  });

  it("serialises rotate90 variants with no extra fields", () => {
    expect(serialise({ kind: "rotate90_cw" }).ops).toEqual([{ kind: "rotate90_cw" }]);
    expect(serialise({ kind: "rotate90_ccw" }).ops).toEqual([{ kind: "rotate90_ccw" }]);
    expect(serialise({ kind: "rotate180" }).ops).toEqual([{ kind: "rotate180" }]);
  });

  it("serialises scale_nearest with new dimensions", () => {
    const out = serialise({ kind: "scale_nearest", new_width: 64, new_height: 32 });
    expect(out.ops).toEqual([{ kind: "scale_nearest", new_width: 64, new_height: 32 }]);
  });

  it("preserves the top-level fields the Rust handler reads", () => {
    const args: TransformArgs = {
      sprite_id: 7,
      layer_id: 9,
      frame_index: 4,
      ops: [{ kind: "translate", dx: 1, dy: 2 }],
    };
    const json = JSON.parse(JSON.stringify(args)) as Record<string, unknown>;
    expect(Object.keys(json).sort()).toEqual(["frame_index", "layer_id", "ops", "sprite_id"]);
  });
});
