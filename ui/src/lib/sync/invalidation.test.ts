import { afterEach, describe, expect, it, vi } from "vitest";
import { __resetRegistry, invalidate, invalidateAll, registerQuery } from "./invalidation";

afterEach(() => __resetRegistry());

describe("invalidation registry", () => {
  it("refetches a registered query by key", () => {
    const refetch = vi.fn();
    registerQuery("layers", refetch);
    invalidate("layers");
    expect(refetch).toHaveBeenCalledTimes(1);
  });

  it("ignores unknown keys without throwing", () => {
    expect(() => invalidate("nope")).not.toThrow();
  });

  it("refetches only the named keys", () => {
    const layers = vi.fn();
    const frames = vi.fn();
    registerQuery("layers", layers);
    registerQuery("frames", frames);
    invalidate("frames");
    expect(layers).not.toHaveBeenCalled();
    expect(frames).toHaveBeenCalledTimes(1);
  });

  it("invalidateAll refetches every registered query", () => {
    const a = vi.fn();
    const b = vi.fn();
    registerQuery("layers", a);
    registerQuery("frames", b);
    invalidateAll();
    expect(a).toHaveBeenCalledTimes(1);
    expect(b).toHaveBeenCalledTimes(1);
  });

  it("unregister drops the key so later invalidation is a no-op", () => {
    const refetch = vi.fn();
    const off = registerQuery("layers", refetch);
    off();
    invalidate("layers");
    expect(refetch).not.toHaveBeenCalled();
  });

  it("warns and overwrites when a key is registered twice", () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    const first = vi.fn();
    const second = vi.fn();
    registerQuery("layers", first);
    registerQuery("layers", second);
    invalidate("layers");
    expect(first).not.toHaveBeenCalled();
    expect(second).toHaveBeenCalledTimes(1);
    expect(warn).toHaveBeenCalled();
    warn.mockRestore();
  });
});
