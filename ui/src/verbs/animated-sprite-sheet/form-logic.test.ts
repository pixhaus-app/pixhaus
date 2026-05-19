import { describe, expect, it } from "vitest";

import {
  defaultFormState,
  fitActionsToGrid,
  toggleChip,
  toInputs,
  validate,
  type FormState,
} from "./form-logic";
import {
  DEFAULT_CELL_SIZE_PX,
  DEFAULT_GRID_SIZE,
  MAX_CELL_SIZE_PX,
  MIN_CELL_SIZE_PX,
} from "./types";

describe("toggleChip — grid-row constraint", () => {
  it("adds a chip up to the capacity", () => {
    const result = toggleChip([], "idle", 3);
    expect(result.outcome).toBe("added");
    expect(result.actions).toEqual(["idle"]);
  });

  it("removes a chip that is already selected", () => {
    const result = toggleChip(["idle", "walk"], "idle", 3);
    expect(result.outcome).toBe("removed");
    expect(result.actions).toEqual(["walk"]);
  });

  it("silently rejects when grid capacity is reached", () => {
    const result = toggleChip(["idle", "walk", "run"], "attack", 3);
    expect(result.outcome).toBe("rejected-capacity");
    expect(result.actions).toEqual(["idle", "walk", "run"]);
  });

  it("rejects empty / whitespace chip labels", () => {
    expect(toggleChip([], "   ", 4).outcome).toBe("rejected-capacity");
    expect(toggleChip([], "", 4).outcome).toBe("rejected-capacity");
  });

  it("trims whitespace before comparing for membership", () => {
    const result = toggleChip(["idle"], "  idle  ", 4);
    expect(result.outcome).toBe("removed");
    expect(result.actions).toEqual([]);
  });

  it("does not mutate the input array", () => {
    const input: readonly string[] = ["idle"];
    toggleChip(input, "walk", 4);
    expect(input).toEqual(["idle"]);
  });
});

describe("fitActionsToGrid", () => {
  it("returns the same array when grid is large enough", () => {
    expect(fitActionsToGrid(["a", "b"], 4)).toEqual(["a", "b"]);
  });

  it("truncates the trailing actions when grid shrinks", () => {
    expect(fitActionsToGrid(["a", "b", "c", "d"], 2)).toEqual(["a", "b"]);
  });

  it("returns an empty array for grid size 0", () => {
    expect(fitActionsToGrid(["a", "b"], 0)).toEqual([]);
  });

  it("never returns negative-length slices", () => {
    expect(fitActionsToGrid(["a"], -3)).toEqual([]);
  });
});

describe("validate", () => {
  function state(over: Partial<FormState>): FormState {
    return { ...defaultFormState(), ...over };
  }

  it("rejects empty / whitespace prompts", () => {
    expect(validate(state({ prompt: "" })).ok).toBe(false);
    expect(validate(state({ prompt: "   " })).ok).toBe(false);
  });

  it("rejects grid sizes outside the spec range", () => {
    for (const bad of [0, 1, 7, 100]) {
      const out = validate(state({ prompt: "x", gridSize: bad }));
      expect(out.ok).toBe(false);
    }
  });

  it("rejects more actions than the grid permits", () => {
    const out = validate(
      state({
        prompt: "x",
        gridSize: 2,
        actions: ["a", "b", "c"],
      }),
    );
    expect(out.ok).toBe(false);
  });

  it("rejects empty action labels", () => {
    const out = validate(
      state({
        prompt: "x",
        gridSize: 3,
        actions: ["idle", " "],
      }),
    );
    expect(out.ok).toBe(false);
  });

  it("rejects out-of-range cell sizes", () => {
    expect(validate(state({ prompt: "x", cellSizePx: MIN_CELL_SIZE_PX - 1 })).ok).toBe(false);
    expect(validate(state({ prompt: "x", cellSizePx: MAX_CELL_SIZE_PX + 1 })).ok).toBe(false);
  });

  it("rejects zero frame duration", () => {
    expect(validate(state({ prompt: "x", frameDurationMs: 0 })).ok).toBe(false);
  });

  it("rejects non-integer / negative seeds", () => {
    expect(validate(state({ prompt: "x", seed: "abc" })).ok).toBe(false);
    expect(validate(state({ prompt: "x", seed: "-1" })).ok).toBe(false);
    expect(validate(state({ prompt: "x", seed: "1.5" })).ok).toBe(false);
  });

  it("accepts a valid minimal form", () => {
    expect(validate(state({ prompt: "baby dragon" })).ok).toBe(true);
  });

  it("accepts an empty seed (means random)", () => {
    expect(validate(state({ prompt: "baby dragon", seed: "" })).ok).toBe(true);
  });
});

describe("toInputs", () => {
  it("omits optional fields when blank", () => {
    const s = defaultFormState();
    s.prompt = "baby dragon";
    const payload = toInputs(s);
    expect(payload.prompt).toBe("baby dragon");
    expect(payload.grid_size).toBe(DEFAULT_GRID_SIZE);
    expect(payload.cell_size_px).toBe(DEFAULT_CELL_SIZE_PX);
    expect(payload.mode).toBe("ai");
    expect(payload.actions).toBeUndefined();
    expect(payload.rewritten_prompt).toBeUndefined();
    expect(payload.layer_name).toBeUndefined();
    expect(payload.seed).toBeUndefined();
  });

  it("includes optional fields when set", () => {
    const s = defaultFormState();
    s.prompt = "  baby dragon  ";
    s.actions = ["idle", "walk"];
    s.rewrittenPrompt = "CHARACTER: pre-edited\nCHOREOGRAPHY: pre-edited";
    s.layerName = "Hero";
    s.seed = "42";
    s.mode = "hybrid";
    const payload = toInputs(s);
    expect(payload.prompt).toBe("baby dragon");
    expect(payload.actions).toEqual(["idle", "walk"]);
    expect(payload.rewritten_prompt).toBe("CHARACTER: pre-edited\nCHOREOGRAPHY: pre-edited");
    expect(payload.layer_name).toBe("Hero");
    expect(payload.seed).toBe(42);
    expect(payload.mode).toBe("hybrid");
  });
});
