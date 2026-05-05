import { afterEach, describe, expect, it, vi } from "vitest";
import { clearToasts, dismissToast, pushToast, toasts } from "./toast-state";

afterEach(() => {
  clearToasts();
  vi.useRealTimers();
});

describe("toast-state", () => {
  it("pushToast appends a toast with a fresh id", () => {
    const id = pushToast({ kind: "error", title: "boom", body: "details", durationMs: 0 });
    const list = toasts();
    expect(list).toHaveLength(1);
    expect(list[0]?.id).toBe(id);
    expect(list[0]?.kind).toBe("error");
    expect(list[0]?.title).toBe("boom");
    expect(list[0]?.body).toBe("details");
  });

  it("pushToast defaults kind to info when omitted", () => {
    pushToast({ title: "hi", durationMs: 0 });
    expect(toasts()[0]?.kind).toBe("info");
  });

  it("dismissToast removes the matching toast", () => {
    const a = pushToast({ title: "a", durationMs: 0 });
    const b = pushToast({ title: "b", durationMs: 0 });
    dismissToast(a);
    expect(toasts().map((t) => t.id)).toEqual([b]);
  });

  it("auto-dismisses after the configured duration", () => {
    vi.useFakeTimers();
    pushToast({ title: "fades", durationMs: 1000 });
    expect(toasts()).toHaveLength(1);
    vi.advanceTimersByTime(999);
    expect(toasts()).toHaveLength(1);
    vi.advanceTimersByTime(1);
    expect(toasts()).toHaveLength(0);
  });

  it("durationMs of 0 keeps the toast until manually dismissed", () => {
    vi.useFakeTimers();
    const id = pushToast({ title: "sticky", durationMs: 0 });
    vi.advanceTimersByTime(60_000);
    expect(toasts()).toHaveLength(1);
    dismissToast(id);
    expect(toasts()).toHaveLength(0);
  });
});
