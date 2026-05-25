import { afterEach, describe, expect, it, vi } from "vitest";
import { createSignal } from "solid-js";
import { createBackendQuery } from "./query";
import { __resetRegistry, invalidate } from "./invalidation";
import { clearToasts, toastState } from "../toast/toast-state";

afterEach(() => {
  __resetRegistry();
  clearToasts();
});

const tick = () => new Promise((r) => setTimeout(r, 0));

interface Deferred<T> {
  promise: Promise<T>;
  resolve: (v: T) => void;
  reject: (e: unknown) => void;
}
function deferred<T>(): Deferred<T> {
  let resolve!: (v: T) => void;
  let reject!: (e: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

describe("createBackendQuery", () => {
  it("stays at the initial value while the source is idle (null)", async () => {
    const fetch = vi.fn(() => Promise.resolve(["x"]));
    const q = createBackendQuery<number, string[]>({
      key: "k",
      source: () => null,
      fetch,
      initial: [],
    });
    await tick();
    expect(q.data()).toEqual([]);
    expect(fetch).not.toHaveBeenCalled();
    q.dispose();
  });

  it("commits the latest fetch and discards a stale out-of-order response", async () => {
    const [param, setParam] = createSignal<number>(1);
    const calls = new Map<number, Deferred<string>>();
    const q = createBackendQuery<number, string>({
      key: "k",
      source: param,
      fetch: (p) => {
        const d = deferred<string>();
        calls.set(p, d);
        return d.promise;
      },
      initial: "initial",
    });
    await tick();

    // Switch the source before the first fetch resolves.
    setParam(2);
    await tick();

    // The first (now stale) request resolves AFTER the second is issued.
    calls.get(1)!.resolve("stale-1");
    calls.get(2)!.resolve("fresh-2");
    await tick();

    // createResource keeps only the latest — the stale response is dropped.
    expect(q.data()).toBe("fresh-2");
    q.dispose();
  });

  it("refetches when its key is invalidated", async () => {
    let n = 0;
    const q = createBackendQuery<number, number>({
      key: "counter",
      source: () => 1,
      fetch: () => Promise.resolve(++n),
      initial: 0,
    });
    await tick();
    expect(q.data()).toBe(1);
    invalidate("counter");
    await tick();
    expect(q.data()).toBe(2);
    q.dispose();
  });

  it("runs onLoaded with each committed value", async () => {
    const seen: string[] = [];
    const q = createBackendQuery<number, string>({
      key: "k",
      source: () => 1,
      fetch: () => Promise.resolve("loaded"),
      initial: "initial",
      onLoaded: (v) => seen.push(v),
    });
    await tick();
    // Fires for the initial value and the first committed fetch.
    expect(seen).toContain("loaded");
    q.dispose();
  });

  it("toasts when a fetch rejects", async () => {
    const q = createBackendQuery<number, string>({
      key: "k",
      source: () => 1,
      fetch: () => Promise.reject(new Error("network down")),
      initial: "",
      errorTitle: "Could not load layers",
    });
    await tick();
    expect(toastState.toasts).toHaveLength(1);
    expect(toastState.toasts[0]?.title).toBe("Could not load layers");
    q.dispose();
  });
});
