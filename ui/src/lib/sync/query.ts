// Backend-backed query primitive.
//
// Wraps Solid's createResource, which already does the thing every domain
// used to hand-roll with a `refreshToken` counter: when the reactive source
// changes mid-flight, the resource commits only the latest fetch and discards
// stale ones. So createBackendQuery deletes that boilerplate outright.
//
// Each query registers its refetch in the invalidation registry under a
// stable key, so mutations and backend events can refresh it without the
// query exposing a bespoke `refreshX()` function.
//
// Queries live at module scope (one cache per domain, single-document app),
// so we own the reactive root explicitly via createRoot. The root is never
// disposed in production — these caches live for the app's lifetime — but the
// returned `dispose` lets tests tear one down.

import { createEffect, createResource, createRoot, on, type Accessor } from "solid-js";
import { registerQuery } from "./invalidation";
import { pushToast } from "../toast/toast-state";

export interface BackendQuery<T> {
  /** Latest committed value, or `initial` before the first fetch settles. */
  data: Accessor<T>;
  /** True while a fetch is in flight. */
  loading: Accessor<boolean>;
  /** Force a refetch with the current source value. */
  refetch: () => void;
  /**
   * Optimistically overwrite the cached value without a fetch. The next
   * refetch replaces it with authoritative data. Mirrors createResource's
   * mutate.
   */
  mutate: (value: T) => void;
  /** Tear down the reactive root and unregister. Test/teardown use only. */
  dispose: () => void;
}

export interface BackendQueryOptions<P, T> {
  /** Stable key for invalidation (mutations/events refetch by key). */
  key: string;
  /**
   * Reactive parameter. When it returns null/false/undefined the fetcher is
   * not called and `data()` stays at `initial` — the idle state (no project,
   * no active sprite).
   */
  source: () => P | null | false | undefined;
  /** Async fetch for a given source value. */
  fetch: (param: P) => Promise<T>;
  /** Value before the first successful fetch and while idle. */
  initial: T;
  /**
   * Runs whenever a fresh value commits (and on the initial value). Use for
   * derived bookkeeping like picking a default selection. Tracks only the
   * data — reads inside it do not re-subscribe the effect.
   */
  onLoaded?: (data: T) => void;
  /** Toast title shown when a fetch rejects. Defaults to "Failed to load". */
  errorTitle?: string;
}

/**
 * Creates a module-scoped cache of backend-owned data keyed on a reactive
 * source. Replaces the manual refresh-token + refreshX() pattern.
 */
export function createBackendQuery<P, T>(opts: BackendQueryOptions<P, T>): BackendQuery<T> {
  return createRoot((dispose) => {
    const [resource, { refetch, mutate }] = createResource(
      opts.source,
      (param) => opts.fetch(param as P),
      { initialValue: opts.initial },
    );

    // `resource.latest` returns the last good value (or `initial`) without
    // throwing on error — exactly the cache semantics we want. Reading
    // `resource()` directly re-throws the error to drive ErrorBoundary, which
    // a module-level cache must not do.
    const data: Accessor<T> = () => resource.latest;
    const loading: Accessor<boolean> = () => resource.loading;

    // Surface fetch failures as a toast instead of a silent console.error.
    createEffect(
      on(
        () => resource.error as unknown,
        (err) => {
          if (err === undefined) return;
          pushToast({
            kind: "error",
            title: opts.errorTitle ?? "Failed to load",
            body: humanizeFetchError(err),
          });
        },
        { defer: true },
      ),
    );

    if (opts.onLoaded !== undefined) {
      const onLoaded = opts.onLoaded;
      // `on(data, ...)` so the callback re-runs only when the committed value
      // changes — not when signals it reads internally (e.g. the active id)
      // change, which would otherwise loop.
      createEffect(on(data, (value) => onLoaded(value)));
    }

    const unregister = registerQuery(opts.key, () => void refetch());

    return {
      data,
      loading,
      refetch: () => void refetch(),
      mutate: (value: T) => mutate(() => value),
      dispose: () => {
        unregister();
        dispose();
      },
    };
  });
}

/** Best-effort one-line message from a rejected fetch. */
function humanizeFetchError(err: unknown): string {
  if (typeof err === "string") return err;
  if (err instanceof Error) return err.message;
  if (err !== null && typeof err === "object" && "kind" in err) {
    return String((err as { kind: unknown }).kind);
  }
  return "Unknown error";
}
