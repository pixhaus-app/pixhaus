// Central Tauri event router.
//
// Backend → UI events used to be wired with a `listen()` call inside each
// component that cared (Canvas, Shell, AnimationStudio, ReferenceSheetEditor),
// each with its own onCleanup. This module is the single place those
// subscriptions are registered, at app startup, so the set of events the UI
// reacts to is readable in one file and torn down with one call.
//
// `listen` is injected so the router is unit-testable in a node environment
// without the Tauri runtime; production passes @tauri-apps/api/event's listen.

export interface EventPayload<T> {
  event: string;
  payload: T;
}

/** Subset of @tauri-apps/api/event's listen signature the router needs. */
export type ListenFn = <T>(
  event: string,
  handler: (e: EventPayload<T>) => void,
) => Promise<() => void>;

/** A single event subscription: the event name and what to do with payloads. */
export interface EventHandler<T = unknown> {
  event: string;
  handle: (payload: T) => void;
}

/**
 * Registers every handler against `listen` and returns one unlisten that
 * detaches all of them. Subscriptions resolve asynchronously (Tauri's listen
 * is async); the returned cleanup awaits and detaches whatever has resolved.
 */
export function registerEventRouter(listen: ListenFn, handlers: EventHandler[]): () => void {
  const pending = handlers.map((h) =>
    listen(h.event, (e) => h.handle(e.payload)).catch((err: unknown) => {
      console.error(`[pixhaus] failed to listen for "${h.event}":`, err);
      return () => {};
    }),
  );

  let disposed = false;
  const unlistens: Array<() => void> = [];
  void Promise.all(pending).then((fns) => {
    if (disposed) {
      for (const fn of fns) fn();
      return;
    }
    unlistens.push(...fns);
  });

  return () => {
    disposed = true;
    for (const fn of unlistens) fn();
    unlistens.length = 0;
  };
}
