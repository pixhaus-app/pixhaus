import { describe, it, expect } from "vitest";
import {
  applyHandleDelta,
  syncNumericFromBounds,
  numericX,
  numericY,
  numericW,
  numericH,
} from "./transform-state";

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
