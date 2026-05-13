// Race-guard tests for hydrateCrashReportingFromBackend.
//
// The hydrator runs at app start and pulls the authoritative value from
// the Rust-side persisted gate (see app/src/crash_reporting.rs). If the
// user flips the toggle during the IPC round-trip, the user's choice
// must win — the late-arriving backend value should not overwrite it.

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

// preferences-store touches localStorage and document at module load.
// Vitest's node environment provides neither, so polyfill minimal shims
// BEFORE any module imports resolve.
vi.hoisted(() => {
  const g = globalThis as unknown as {
    localStorage?: Storage;
    document?: { documentElement: { dataset: Record<string, string> } };
  };
  const store = new Map<string, string>();
  g.localStorage = {
    getItem: (k: string) => store.get(k) ?? null,
    setItem: (k: string, v: string) => {
      store.set(k, v);
    },
    removeItem: (k: string) => {
      store.delete(k);
    },
    clear: () => store.clear(),
    key: (i: number) => Array.from(store.keys())[i] ?? null,
    get length() {
      return store.size;
    },
  };
  g.document = { documentElement: { dataset: {} } };
  // Node 18+ provides globalThis.crypto natively (read-only) with the
  // same randomUUID surface preferences-store needs at module load.
});

const invokeMock = vi.fn();

// Mock the IPC wrapper rather than @tauri-apps/api/core so the call
// shape matches what crashReportingGetEnabled actually invokes.
vi.mock("../lib/ipc", () => ({
  invoke: (cmd: string, args?: unknown) => invokeMock(cmd, args),
}));

import {
  crashReportingEnabled,
  hydrateCrashReportingFromBackend,
  setCrashReportingEnabled,
} from "./preferences-store";

beforeEach(() => {
  localStorage.clear();
  invokeMock.mockReset();
  // Reset the user-touched flag by setting the signal back through the
  // public setter... but that flips the flag too. There's no public
  // reset; rely on test ordering and one-shot setCrashReportingEnabled
  // calls. Each test explicitly seeds the state it needs.
});

afterEach(() => {
  localStorage.clear();
});

describe("hydrateCrashReportingFromBackend", () => {
  it("applies the backend value when the user has not touched the preference", async () => {
    // The user-touched flag is module-scoped and initialised to false on
    // module load. The other tests in this file flip it via
    // setCrashReportingEnabled, so this test must run first to observe
    // the untouched state. Vitest runs tests in declaration order within
    // a file, which is stable enough for this single-file suite.
    invokeMock.mockResolvedValueOnce(true);

    const result = await hydrateCrashReportingFromBackend();

    expect(result).toBe(true);
    expect(crashReportingEnabled()).toBe(true);
    expect(localStorage.getItem("pixhaus:crash-reporting-enabled")).toBe("1");
    expect(invokeMock).toHaveBeenCalledWith("crash_reporting_get_enabled", undefined);
  });

  it("preserves the user's choice if they toggle while the IPC is in flight", async () => {
    // Simulate a slow backend: the IPC resolves only after the user has
    // flipped the toggle. The user's value (false) must survive.
    let resolve: (value: boolean) => void = () => {};
    const pending = new Promise<boolean>((r) => {
      resolve = r;
    });
    invokeMock.mockReturnValueOnce(pending);

    const hydratePromise = hydrateCrashReportingFromBackend();
    // User clicks the toggle to false while the IPC is still pending.
    setCrashReportingEnabled(false);
    // Now the backend responds with a stale true.
    resolve(true);

    const result = await hydratePromise;

    // The user's false wins; the stale backend true is discarded.
    expect(result).toBe(false);
    expect(crashReportingEnabled()).toBe(false);
    expect(localStorage.getItem("pixhaus:crash-reporting-enabled")).toBe("0");
  });

  it("falls back to the cached signal when the backend call rejects", async () => {
    // Seed the cache (and signal) to a known value via the touched
    // setter so localStorage matches.
    setCrashReportingEnabled(true);
    invokeMock.mockRejectedValueOnce(new Error("ipc unreachable"));

    const result = await hydrateCrashReportingFromBackend();

    expect(result).toBe(true);
    expect(crashReportingEnabled()).toBe(true);
  });
});
