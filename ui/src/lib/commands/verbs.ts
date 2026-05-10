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

export type VerbInfo = {
  /** Stable verb ID. */
  id: string;
  description: string;
  /** True if the verb supports cancellation mid-run. */
  cancellable: boolean;
  /** Bitfield of `BackendCapabilities` required to invoke. */
  required_capabilities: number;
};

// ── commands ──────────────────────────────────────────────────────────────────

/** Invokes a registered AI verb. Errors propagate as Promise rejections. */
export function verbInvoke(args: VerbInvokeArgs): Promise<VerbOutput> {
  return invoke<VerbOutput>("verb_invoke", { args });
}

/** Lists all registered verbs, sorted by ID. */
export function verbList(): Promise<VerbInfo[]> {
  return invoke<VerbInfo[]>("verb_list");
}

/**
 * Cancels an in-progress verb invocation by its opaque invocation id
 * (not the verb id — concurrent invocations of the same verb each get
 * their own handle). The Rust handler is a stub pending an in-flight
 * invocation map; calling this rejects with
 * AppCommandError::Unimplemented for now.
 */
export function verbCancel(invocation_id: string): Promise<void> {
  return invoke<void>("verb_cancel", { invocation_id });
}
