# UI state management migration — Solid-native stores + central IPC-sync layer

## Context

The Pixhaus UI (`ui/`, Solid.js 1.9.13) has grown to ~23 module-level "state" files and ~329 components. State is reactive already — the problem is **not** the absence of a state library. The recurring "state issues" trace to four structural gaps, all visible in the current code:

1. **IPC race conditions, patched by hand.** A `refreshToken` counter that drops out-of-order responses is copy-pasted across `layer-state.ts:108`, `timeline-state.ts`, `library-state.ts`, and `tilemap-ctx-sync.ts`. Each domain reinvents it; each is a place to get it subtly wrong.
2. **No uniform mutate→refetch pattern.** Every mutation is the same shape — `someCmd(...).then(() => refreshX()).catch((e) => console.error(...))` — repeated ~20× in `layer-state.ts` alone. Failures only log; there is no rollback and no user-facing error.
3. **Cross-domain sync is ad-hoc.** One-off reactive bridges (`palette-color-sync.ts`, `tilemap-ctx-sync.ts`) and circular-import workarounds (`canvas-state.ts:34-39` owns `activeLayerId` only to avoid a cycle with the layer panel).
4. **Events are scattered.** `listen()` for Tauri events lives directly in 4 components (`Shell.tsx`, `Canvas.tsx`, `AnimationStudio.tsx`, `ReferenceSheetEditor.tsx`) with hand-rolled `onCleanup`.

**Decision (confirmed with user):** Do **not** introduce Zustand — it is React-first and would require bridging its vanilla store to Solid's reactivity on every consumer, discarding Solid's fine-grained tracking. Use Solid's native `solid-js/store` `createStore` plus `createResource`, and build one **central IPC-sync layer** that internalizes staleness, refetch, and error handling. Migrate **all domains in one branch (big-bang)**, ordered foundation-first so each commit builds green.

Intended outcome: one consistent way to read backend-owned state, one way to mutate it, one event router, and zero hand-rolled token guards. The backend (Rust `DocumentStore`) remains the single source of truth; the UI holds a disciplined cache.

> Big-bang risk note: ~23 domains in one branch is large to review. Mitigation — land the foundation (Phase 0) as the first commit, migrate one domain per commit on the same branch (Phase 1+), keep `pnpm test` + `tsc --noEmit` green at every commit, and open the PR only once all domains are migrated. If review proves unwieldy, the foundation commit is independently mergeable as a fallback.

## Architecture

### New layout
```text
ui/src/lib/sync/         # the central IPC-sync layer (new)
  query.ts               # createBackendQuery — wraps createResource, kills refreshToken
  mutation.ts            # runMutation — invoke + invalidate + toast-on-error + optional rollback
  events.ts              # registerEventRouter — single listen() registration site
  invalidation.ts        # query registry + invalidate(key) used by mutations & events
ui/src/stores/           # one store module per domain (new home for *-state.ts)
  canvas-store.ts
  layers-store.ts
  timeline-store.ts
  ... (one per existing *-state.ts)
```

State stays at **module scope** (no Context) — this is a single-window, single-document app, so module-level Solid stores under `createRoot` are simpler than a provider tree and match the existing mental model. Resolve the circular-import issue by letting cross-domain reads flow through the store modules directly (the dependency that forced `activeLayerId` into `canvas-state` disappears once layers and canvas are both stores importing a shared `active-store.ts` that owns `{ activeSpriteId, activeFrameIndex, activeLayerId }`).

### Building block 1 — `createBackendQuery` (`lib/sync/query.ts`)
Replaces every hand-rolled `refreshToken`. Wrap Solid's `createResource`, which **natively ignores stale responses** (it tracks only the latest in-flight fetch keyed on its reactive source). Register each query in the invalidation registry so mutations/events can refetch it.

```ts
// Sketch — final API tuned during implementation.
export function createBackendQuery<P, T>(opts: {
  key: string;                      // for invalidation
  source: () => P | null | false;   // reactive param; null/false => idle, returns `initial`
  fetch: (param: P) => Promise<T>;
  initial: T;
  onLoaded?: (data: T) => void;     // e.g. ensureActiveLayer
}): {
  data: Accessor<T>;
  loading: Accessor<boolean>;
  refetch: () => void;
};
```
Built on `createResource(source, fetcher)` inside a module-level `createRoot`. `refetch` delegates to the resource's `refetch`. Errors route to the toast layer, not `console.error`.

Migration: `refreshLayers()` + `refreshToken` + `ensureActiveLayer` collapse into one `createBackendQuery({ key: "layers", source: () => activeSpriteId(), fetch: layerList, initial: [], onLoaded: ensureActiveLayer })`. The exported `layers()` accessor stays, so most consumers are untouched.

### Building block 2 — `runMutation` (`lib/sync/mutation.ts`)
Collapses the repeated `.then(refresh).catch(console.error)`.

```ts
export async function runMutation<T>(opts: {
  run: () => Promise<T>;
  invalidate?: string[];             // query keys to refetch on success
  optimistic?: { apply: () => void; rollback: () => void };
  errorToast?: string | ((e: unknown) => string);  // default: humanized AppCommandError
}): Promise<T | undefined>;
```
On success: invalidate listed queries. On error: rollback if optimistic, push a toast via existing `pushToast()` (`lib/toast/toast-state.ts`), and surface the `AppCommandError.kind/message` shape returned by Rust commands. This is where issue #2's silent failures get fixed.

Migration: `setLayerVisibility` becomes `runMutation({ run: () => layerSetVisibility(...), invalidate: ["layers"], optimistic: {...} })` and also triggers the existing `refreshViewport` recomposite.

### Building block 3 — `registerEventRouter` (`lib/sync/events.ts`)
One module that registers all Tauri `listen()` subscriptions at app startup (called once from `App.tsx`/`Shell.tsx` mount) and dispatches each event into the owning store or a query invalidation. Returns a single cleanup. Folds in `canvas:tile-dirty`, `shell:menu`, `tilemap:cell-changed`, `tilemap:bulk-changed`, `updater:available`, `updater:ready`, plus the per-job streams (`AnimationJobUpdate`, `SheetRequest*`). Component-local listeners are removed in favor of store subscriptions.

> Keep `canvas:tile-dirty` handling hot-path-friendly: it routes straight to the renderer tile cache as today (no store round-trip for pixel data). The router only owns *registration*, not added indirection on the per-tile decode path.

### Building block 4 — domain stores (`ui/src/stores/*`)
Each existing `*-state.ts` becomes a `*-store.ts` that:
- holds UI-only state in a single `createStore` object (replacing scattered `createSignal`s where they form one logical record — e.g. viewport, selection),
- exposes backend-owned data through a `createBackendQuery`,
- exposes actions implemented with `runMutation`.

Keep existing public accessor names (`layers()`, `zoom()`, `activeSpriteId()`, etc.) as thin re-exports so the ~329 consuming components need minimal churn. The migration is structural underneath, not a rename storm.

## Work plan (single branch `feat/ui-state-migration`, ordered commits)

**Phase 0 — foundation (commit 1).** Add `lib/sync/{query,mutation,events,invalidation}.ts` with unit tests. Add `stores/active-store.ts` owning `{ activeSpriteId, activeFrameIndex, activeLayerId }` and re-export from `canvas-state.ts` to keep imports working. No behavior change yet.

**Phase 1 — high-traffic domains (commits 2-5).** Migrate in dependency order: `active` → `canvas` → `layers` → `timeline`. These exercise queries (`layers`, `frames/tags/cels`), mutations, and the active-id sharing that removes the circular-import workaround.

**Phase 2 — remaining backend-backed domains (commits 6-10).** `palette-panel`, `library`, `tilemap` (+ retire `tilemap-ctx-sync.ts`/`palette-color-sync.ts` into store-level derivations), `sheet`/`sheet-editor`, `animation-studio`.

**Phase 3 — UI-only domains (commit 11).** `tool`, `select`, `transform`, `rail`, `preferences`, `toast`, `project`, and the modal toggles (`palette`, `preferences`, `canvas-size-dialog`, `update-modal`, `composition-library`, `entity-create`, `verb-invoke`). Mostly mechanical moves into `createStore`.

**Phase 4 — events + cleanup (commit 12).** Wire `registerEventRouter`, delete the four component-local `listen()` blocks, delete every `refreshToken`/manual stale-guard, delete the two sync-bridge files. Update `pixhaus-testing-conventions`-style tests.

## Critical files

Modify / move:
- `ui/src/canvas/canvas-state.ts`, `ui/src/layers/layer-state.ts`, `ui/src/timeline/timeline-state.ts` — first and most representative migrations.
- `ui/src/palette/palette-panel-state.ts`, `ui/src/library/library-state.ts`, `ui/src/tilemap/tilemap-state.ts`, `ui/src/animation/animation-studio-state.ts` — query/mutation conversions.
- `ui/src/canvas/Canvas.tsx`, `ui/src/shell/Shell.tsx`, `ui/src/animation/AnimationStudio.tsx`, `ui/src/sheet/ReferenceSheetEditor.tsx` — remove local `listen()`.

Reuse (do not reinvent):
- `ui/src/lib/ipc.ts` — keep as the single `invoke` wrapper; `runMutation` and queries call command modules that already route through it.
- `ui/src/lib/commands/*` — the command facades stay as-is; stores call them.
- `ui/src/lib/toast/toast-state.ts` `pushToast()` — the error surface for `runMutation`.
- Solid's `createResource` / `createStore` / `createRoot` — the staleness + fine-grained reactivity engine. Pull current API details via context7 (`solid-js`) before writing `query.ts`.

Delete at the end:
- `ui/src/canvas/tilemap-ctx-sync.ts`-equivalent and `ui/src/palette/palette-color-sync.ts` (folded into store derivations).
- All `refreshToken` / manual stale-guard blocks.

## Verification

- `pnpm test` (UI unit tests) and `pnpm tsc --noEmit` green at **every commit** — strict mode, no `any`, no unchecked nulls per CLAUDE.md.
- Unit tests for the foundation: `query.ts` drops a stale response when `source` changes mid-flight; `runMutation` invalidates on success and rolls back + toasts on rejection; `events.ts` dispatches a sample payload to the right store.
- `pnpm dev` smoke test of the real app for each migrated domain:
  - Layers: add/delete/reorder/rename/visibility/opacity; rapidly switch sprites to confirm no stale layer list (the race the token guard existed for).
  - Canvas: paint, fill, transform; confirm `tile-dirty` repaint still real-time.
  - Timeline: frame add/delete, playback, tag edits.
  - Trigger a mutation failure path (e.g. rename to empty / locked-layer paint) and confirm a toast now appears instead of a silent console error.
- `./scripts/pre-pr.sh` before opening the PR.
- Follow `pixhaus-claude-code-workflow` for branch/commit/PR; reference the relevant UI stream in `docs/planning/work/streams.md`.
- Per project memory, copy this plan into `docs/planning/work/` as the durable design doc in commit 1.
