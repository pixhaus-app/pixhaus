import { describe, expect, it, vi } from "vitest";
import { reportCommandFailure } from "./errors";
import { clearToasts, toasts } from "../toast/toast-state";

describe("reportCommandFailure body extraction", () => {
  it("uses message.detail when present", () => {
    clearToasts();
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    reportCommandFailure("test_op", { kind: "validation", message: { detail: "bad input" } });
    expect(toasts()[0]?.body).toBe("bad input");
  });

  it("formats layer_locked with the layer id instead of the bare kind", () => {
    clearToasts();
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    reportCommandFailure("canvas_begin_stroke", { kind: "layer_locked", message: { layer_id: 7 } });
    expect(toasts()[0]?.body).toBe("layer 7 is locked");
  });

  it("falls back to kind when no recognised field is present", () => {
    clearToasts();
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    reportCommandFailure("test_op", { kind: "no_active_project" });
    expect(toasts()[0]?.body).toBe("no_active_project");
  });
});
