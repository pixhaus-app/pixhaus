// Module-scoped state for the verb-invoke modal host.
//
// `command-registry.ts` handlers can't render UI directly — they're
// plain functions. They open the modal by writing to this state; the
// `<VerbInvokeHost>` component (mounted once in the shell) subscribes
// and renders `ModalForm` accordingly.

import { createStore } from "solid-js/store";
import type { VerbInfo } from "../commands/verbs";

// One store for the verb-invoke modal host. Reads are verbModal.activeVerb,
// verbModal.pendingPrefill, etc.; writes go through the setters below.
interface VerbInvokeState {
  /** Verb the modal is currently open for, or null when closed. */
  activeVerb: VerbInfo | null;
  /** In-flight invocation handle while a verb is running, else null. */
  activeInvocationId: string | null;
  /**
   * Caller-supplied prefill for the next modal open, threaded into ModalForm's
   * initialValues and cleared on close. Set before setActiveVerb() so the
   * modal opens with seeded inputs (sheet-panel Refine passes panel_label,
   * Re-run passes prompt).
   */
  pendingPrefill: Record<string, unknown> | null;
}

export const [verbModal, setVerbModal] = createStore<VerbInvokeState>({
  activeVerb: null,
  activeInvocationId: null,
  pendingPrefill: null,
});

export const setActiveVerb = (v: VerbInfo | null): void => setVerbModal("activeVerb", v);
export const setActiveInvocationId = (v: string | null): void =>
  setVerbModal("activeInvocationId", v);
export const setPendingPrefill = (v: Record<string, unknown> | null): void =>
  setVerbModal("pendingPrefill", v);

/**
 * Memoised verb list — populated lazily on first open. The runtime's
 * verb set is fixed at startup, so a single fetch is enough; if a
 * future plugin-hot-reload feature changes the set, call
 * `clearVerbCache()` to refetch.
 */
let verbsCache: Promise<VerbInfo[]> | null = null;

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
