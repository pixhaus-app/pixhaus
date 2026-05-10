// Module-level wrapper around @tauri-apps/api/core's invoke.
//
// All UI code calls this function to dispatch IPC. We delegate to the
// real SDK after pushing an entry on window.__pixhaus_ipc_log__ so
// e2e specs can assert on `[IPC]` sequences.
//
// We wrap at the module layer rather than monkey-patching
// __TAURI_INTERNALS__.invoke at runtime: the runtime patch is fragile
// (the property is non-writable in production WebView2 and silently
// no-ops in some configurations), while a module export is just a
// regular function we control end to end.
//
// The signature mirrors the SDK's exactly so command wrappers only
// change the import path, not the call shape.

import { invoke as tauriInvoke, type InvokeArgs, type InvokeOptions } from "@tauri-apps/api/core";

export type { InvokeArgs, InvokeOptions };

export interface IpcLogEntry {
  /** IPC command name, e.g. "project_new". */
  cmd: string;
  /** Args object passed to invoke; structure varies per command. */
  args: InvokeArgs | undefined;
  /** Optional invoke options (headers, etc) — third positional arg. */
  options: InvokeOptions | undefined;
  /** Whether the command resolved (true) or rejected (false). */
  ok: boolean;
  /** Resolved value when ok === true; `undefined` for void commands. */
  result?: unknown;
  /** Rejection value when ok === false. */
  error?: unknown;
  /** Wall-clock duration of the call in milliseconds. */
  durationMs: number;
  /** Monotonic counter, 1-based, allocated at call time. */
  seq: number;
}

interface TaggedWindow extends Window {
  __pixhaus_ipc_log__?: IpcLogEntry[];
}

let seq = 0;

function getLog(): IpcLogEntry[] {
  const w = window as TaggedWindow;
  if (w.__pixhaus_ipc_log__ === undefined) {
    w.__pixhaus_ipc_log__ = [];
  }
  return w.__pixhaus_ipc_log__;
}

/**
 * Sends a Tauri IPC call and records it on window.__pixhaus_ipc_log__.
 * Mirrors `@tauri-apps/api/core`'s `invoke` signature exactly.
 */
export async function invoke<T>(
  cmd: string,
  args?: InvokeArgs,
  options?: InvokeOptions,
): Promise<T> {
  seq += 1;
  const entry: IpcLogEntry = {
    cmd,
    args,
    options,
    ok: false,
    durationMs: 0,
    seq,
  };
  const start = performance.now();
  try {
    const result = await tauriInvoke<T>(cmd, args, options);
    entry.ok = true;
    entry.result = result;
    entry.durationMs = performance.now() - start;
    getLog().push(entry);
    return result;
  } catch (err) {
    entry.ok = false;
    entry.error = err;
    entry.durationMs = performance.now() - start;
    getLog().push(entry);
    throw err;
  }
}
