// AI verb invocation commands.
// Invoke and cancel are stubbed until B5 (verb plugin protocol) lands.
// verb_list returns an empty array until B5 populates the registry.

import { invoke } from "@tauri-apps/api/core";

// ── types ─────────────────────────────────────────────────────────────────────

export type VerbInvokeArgs = {
  name: string;
  /** Free-form JSON context passed to the verb. Schema defined per-verb in docs/verb-protocol.md. */
  context: unknown;
};

export type VerbStatus =
  | { kind: "pending" }
  | { kind: "done" }
  | { kind: "error"; message: string };

export type VerbResult = {
  verb_id: string;
  status: VerbStatus;
};

export type VerbInfo = {
  name: string;
  description: string;
  required_backends: string[];
};

// ── commands ──────────────────────────────────────────────────────────────────

/**
 * Invokes a registered AI verb with the given context.
 * Requires B5 (verb plugin protocol) — returns an error until B5 lands.
 */
export function verbInvoke(args: VerbInvokeArgs): Promise<VerbResult> {
  return invoke<VerbResult>("verb_invoke", { args });
}

/** Lists all registered verbs. Returns an empty array until B5 lands. */
export function verbList(): Promise<VerbInfo[]> {
  return invoke<VerbInfo[]>("verb_list");
}

/**
 * Cancels an in-progress verb invocation by its opaque ID.
 * Requires B5 (verb plugin protocol) — returns an error until B5 lands.
 */
export function verbCancel(verb_id: string): Promise<void> {
  return invoke<void>("verb_cancel", { verb_id });
}
