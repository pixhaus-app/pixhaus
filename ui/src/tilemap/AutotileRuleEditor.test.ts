import { describe, expect, it } from "vitest";
import { blankRule, conditionLabel, conditionTitle, nextCondition } from "./autotile-helpers";

describe("nextCondition", () => {
  it("any → filled", () => {
    expect(nextCondition("any")).toBe("filled");
  });

  it("filled → empty", () => {
    expect(nextCondition("filled")).toBe("empty");
  });

  it("empty → any", () => {
    expect(nextCondition("empty")).toBe("any");
  });

  it("cycles back to any after two steps from any", () => {
    const step1 = nextCondition("any");
    const step2 = nextCondition(step1);
    const step3 = nextCondition(step2);
    expect(step3).toBe("any");
  });
});

describe("conditionLabel", () => {
  it("filled → F", () => {
    expect(conditionLabel("filled")).toBe("F");
  });

  it("empty → E", () => {
    expect(conditionLabel("empty")).toBe("E");
  });

  it("any → middle dot", () => {
    expect(conditionLabel("any")).toBe("·");
  });
});

describe("conditionTitle", () => {
  it("filled → Filled", () => {
    expect(conditionTitle("filled")).toBe("Filled");
  });

  it("empty → Empty", () => {
    expect(conditionTitle("empty")).toBe("Empty");
  });

  it("any → Any", () => {
    expect(conditionTitle("any")).toBe("Any");
  });
});

describe("blankRule", () => {
  it("creates a rule with exactly 8 conditions", () => {
    expect(blankRule().conditions).toHaveLength(8);
  });

  it("defaults all conditions to any", () => {
    const { conditions } = blankRule();
    expect(conditions.every((c) => c === "any")).toBe(true);
  });

  it("defaults result_tile to 0 (empty tile is a valid result)", () => {
    expect(blankRule().result_tile).toBe(0);
  });

  it("defaults result_flags to 0 (no flip or rotation)", () => {
    expect(blankRule().result_flags).toBe(0);
  });

  it("returns a fresh object on each call", () => {
    const a = blankRule();
    const b = blankRule();
    expect(a).not.toBe(b);
    expect(a.conditions).not.toBe(b.conditions);
  });
});
