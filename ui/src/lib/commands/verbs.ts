// AI verb invocation commands.
//
// The Rust handler signatures live in app/src/commands/verbs.rs. Keep these
// types in sync — any drift surfaces only at the IPC boundary as a serde
// error, which is hard to debug from the UI side.
//
// Once tauri-specta wires up generated bindings for verbs, delete the
// hand-rolled types here and import the generated ones.

import { invoke } from "../ipc";

// ── types ─────────────────────────────────────────────────────────────────────

export type VerbInvokeArgs = {
  /** Stable verb ID, e.g. "pixhaus.builtin.critique". */
  verb_id: string;
  /** Per-verb input payload. Schema defined by the verb's descriptor. */
  inputs: unknown;
};

/**
 * Output of a successful verb invocation. The shape is per-verb; for now
 * the UI just surfaces it as opaque JSON. A typed VerbOutput will land
 * with tauri-specta bindings.
 */
export type VerbOutput = unknown;

/**
 * Result of a verb invocation: the verb's output plus the opaque
 * handle the UI hands back to `verbCancel` if the user interrupts a
 * still-running invocation.
 */
export type VerbInvocationResult = {
  /** Per-invocation handle, valid until `finish` returns. */
  invocation_id: string;
  output: VerbOutput;
};

export type VerbInfo = {
  /** Stable verb ID. */
  id: string;
  /** Display name for menus and the command palette. */
  display_name: string;
  description: string;
  /** True if the verb supports cancellation mid-run. */
  cancellable: boolean;
  /** Bitfield of `BackendCapabilities` required to invoke. */
  required_capabilities: number;
  /**
   * JSON Schema for the verb's input payload. The UI uses this to
   * render an input form without baking per-verb knowledge.
   */
  input_schema: unknown;
};

// ── commands ──────────────────────────────────────────────────────────────────

/** Invokes a registered AI verb. Errors propagate as Promise rejections. */
export function verbInvoke(args: VerbInvokeArgs): Promise<VerbInvocationResult> {
  return invoke<VerbInvocationResult>("verb_invoke", { args });
}

/** Lists all registered verbs, sorted by ID. */
export function verbList(): Promise<VerbInfo[]> {
  return invoke<VerbInfo[]>("verb_list");
}

/**
 * Cancels an in-progress verb invocation by its opaque invocation id
 * (the value returned by `verbInvoke`). Cancellation is cooperative —
 * the verb observes the token between expensive operations.
 * Idempotent — already-completed ids are not an error.
 */
export function verbCancel(invocation_id: string): Promise<void> {
  return invoke<void>("verb_cancel", { invocation_id });
}
