// Query invalidation registry.
//
// Every backend-backed query (see query.ts) registers its refetch under a
// stable string key. Mutations (mutation.ts) and the event router
// (events.ts) call invalidate() with the keys they touched, and the matching
// queries refetch. This replaces the per-domain `refreshX()` functions that
// each mutation used to call by hand.
//
// Keys are plain strings rather than a union so a new domain can register
// without editing a central enum. Collisions are a programming error; the
// registry warns if two live queries claim the same key.

/** Stable identifiers for the backend-backed query caches. */
export type QueryKey = "layers" | "frames" | "palettes" | "tilesets" | "library" | "sheets";

type Refetch = () => void;

const registry = new Map<string, Refetch>();

/**
 * Registers a query's refetch under `key`. Returns an unregister function;
 * call it from the owning reactive root's cleanup. Registering a key that is
 * already live warns and overwrites — two live caches under one key is a bug.
 */
export function registerQuery(key: string, refetch: Refetch): () => void {
  if (registry.has(key)) {
    console.warn(`[pixhaus] query key "${key}" registered twice; overwriting`);
  }
  registry.set(key, refetch);
  return () => {
    if (registry.get(key) === refetch) registry.delete(key);
  };
}

/**
 * Refetches every named query that is currently registered. Unknown keys are
 * ignored (the owning domain may not be mounted yet). Pass the keys a mutation
 * or event affected; order is not significant.
 */
export function invalidate(...keys: string[]): void {
  for (const key of keys) {
    const refetch = registry.get(key);
    if (refetch !== undefined) refetch();
  }
}

/** Refetches every registered query. Use sparingly — e.g. after project open. */
export function invalidateAll(): void {
  for (const refetch of registry.values()) refetch();
}

/** Test-only: drop all registrations so a fresh test starts clean. */
export function __resetRegistry(): void {
  registry.clear();
}
