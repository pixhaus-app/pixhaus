// Module-scoped state for the verb-invoke modal host.
//
// `command-registry.ts` handlers can't render UI directly — they're
// plain functions. They open the modal by writing to this state; the
// `<VerbInvokeHost>` component (mounted once in the shell) subscribes
// and renders `ModalForm` accordingly.

import { createSignal } from "solid-js";
import type { VerbInfo } from "../commands/verbs";

/** Verb the modal is currently open for, or null when closed. */
const [activeVerb, setActiveVerb] = createSignal<VerbInfo | null>(null);

/** In-flight invocation handle while a verb is running, else null. */
const [activeInvocationId, setActiveInvocationId] = createSignal<string | null>(null);

/**
 * Caller-supplied prefill for the next modal open. Read by `VerbInvokeHost`
 * and threaded into `ModalForm`'s `initialValues`. Cleared on modal close.
 * Set this before calling `setActiveVerb()` so the modal opens with seeded
 * inputs (used by sheet-panel Refine to pass `panel_label` and by Re-run
 * to pass `prompt`).
 */
const [pendingPrefill, setPendingPrefill] = createSignal<Record<string, unknown> | null>(null);

/**
 * Memoised verb list — populated lazily on first open. The runtime's
 * verb set is fixed at startup, so a single fetch is enough; if a
 * future plugin-hot-reload feature changes the set, call
 * `clearVerbCache()` to refetch.
 */
let verbsCache: Promise<VerbInfo[]> | null = null;

export {
  activeVerb,
  setActiveVerb,
  activeInvocationId,
  setActiveInvocationId,
  pendingPrefill,
  setPendingPrefill,
};

export function getCachedVerbList(load: () => Promise<VerbInfo[]>): Promise<VerbInfo[]> {
  if (verbsCache === null) {
    verbsCache = load().catch((err: unknown) => {
      // Don't memoise the failure — the next call should retry rather
      // than re-surface a stale error.
      verbsCache = null;
      throw err;
    });
  }
  return verbsCache;
}

export function clearVerbCache(): void {
  verbsCache = null;
}
