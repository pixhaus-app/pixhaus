import { afterEach, describe, expect, it, vi } from "vitest";
import { humanizeCommandError, runMutation } from "./mutation";
import { __resetRegistry, registerQuery } from "./invalidation";
import { clearToasts, toastState } from "../toast/toast-state";
import type { AppCommandError } from "../types/AppCommandError";

afterEach(() => {
  __resetRegistry();
  clearToasts();
});

describe("runMutation", () => {
  it("returns the command value and invalidates listed queries on success", async () => {
    const refetch = vi.fn();
    registerQuery("layers", refetch);
    const value = await runMutation({
      run: () => Promise.resolve(42),
      invalidate: ["layers"],
    });
    expect(value).toBe(42);
    expect(refetch).toHaveBeenCalledTimes(1);
  });

  it("runs onSuccess after a successful call", async () => {
    const onSuccess = vi.fn();
    await runMutation({ run: () => Promise.resolve("ok"), onSuccess });
    expect(onSuccess).toHaveBeenCalledWith("ok");
  });

  it("rolls back the optimistic update and toasts on failure", async () => {
    const rollback = vi.fn();
    const apply = vi.fn();
    const refetch = vi.fn();
    registerQuery("layers", refetch);
    const result = await runMutation({
      run: () => Promise.reject<number>({ kind: "layer_locked", message: { layer_id: 3 } }),
      invalidate: ["layers"],
      optimistic: { apply, rollback },
    });
    expect(result).toBeUndefined();
    expect(apply).toHaveBeenCalledTimes(1);
    expect(rollback).toHaveBeenCalledTimes(1);
    // No invalidation on failure.
    expect(refetch).not.toHaveBeenCalled();
    // A toast surfaced the failure (the old code only console.error'd).
    expect(toastState.toasts).toHaveLength(1);
    expect(toastState.toasts[0]?.kind).toBe("error");
  });

  it("suppresses the toast when errorToast is false", async () => {
    await runMutation({
      run: () => Promise.reject<number>({ kind: "no_active_project" }),
      errorToast: false,
    });
    expect(toastState.toasts).toHaveLength(0);
  });

  it("applies the optimistic update before the call resolves", async () => {
    const order: string[] = [];
    await runMutation({
      run: () => {
        order.push("run");
        return Promise.resolve(1);
      },
      optimistic: { apply: () => order.push("apply"), rollback: () => {} },
    });
    expect(order).toEqual(["apply", "run"]);
  });
});

describe("humanizeCommandError", () => {
  it("renders not_found with entity and id", () => {
    const err: AppCommandError = { kind: "not_found", message: { entity: "layer", id: 7n } };
    expect(humanizeCommandError(err)).toBe("Layer #7 no longer exists.");
  });

  it("renders layer_locked with the layer id", () => {
    const err: AppCommandError = { kind: "layer_locked", message: { layer_id: 4 } };
    expect(humanizeCommandError(err)).toContain("#4");
  });

  it("passes through validation detail", () => {
    const err: AppCommandError = { kind: "validation", message: { detail: "name is empty" } };
    expect(humanizeCommandError(err)).toBe("name is empty");
  });

  it("falls back for plain string and Error values", () => {
    expect(humanizeCommandError("boom")).toBe("boom");
    expect(humanizeCommandError(new Error("kaboom"))).toBe("kaboom");
  });
});
