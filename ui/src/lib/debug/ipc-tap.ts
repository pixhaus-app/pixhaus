// Records every IPC roundtrip on window.__pixhaus_ipc_log__ so e2e specs
// can assert on `[IPC]` sequences without reaching into the Rust process.
//
// The tap monkey-patches window.__TAURI_INTERNALS__.invoke. It runs once
// at app startup and is idempotent — calling installIpcTap() twice is a
// no-op, so a future module that wants to read the log can safely call it
// again. The tap delegates to the original invoke, so nothing about the
// IPC path itself changes; we only observe.
//
// In environments without a Tauri runtime (vite dev standalone, unit tests
// outside the visual harness) __TAURI_INTERNALS__ is undefined. We return
// quietly in that case — the existing app code already throws on the first
// real invoke, so adding our own error here would only mask that.

export interface IpcLogEntry {
  /** IPC command name, e.g. "project_new". */
  cmd: string;
  /** Args object passed to invoke; structure varies per command. */
  args: unknown;
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

interface TauriInternals {
  invoke: (cmd: string, args?: unknown) => Promise<unknown>;
}

interface TaggedWindow extends Window {
  __TAURI_INTERNALS__?: TauriInternals;
  __pixhaus_ipc_log__?: IpcLogEntry[];
  __pixhaus_ipc_tap_installed__?: boolean;
}

let seq = 0;

/**
 * Installs an IPC tap once. Idempotent. No-op when the Tauri runtime
 * is not present (e.g. standalone vite dev).
 */
export function installIpcTap(): void {
  const w = window as TaggedWindow;
  if (w.__pixhaus_ipc_tap_installed__ === true) return;

  const internals = w.__TAURI_INTERNALS__;
  if (internals === undefined) return;

  if (w.__pixhaus_ipc_log__ === undefined) {
    w.__pixhaus_ipc_log__ = [];
  }
  const log = w.__pixhaus_ipc_log__;

  const original = internals.invoke.bind(internals);
  internals.invoke = async (cmd: string, args?: unknown) => {
    seq += 1;
    const entry: IpcLogEntry = {
      cmd,
      args,
      ok: false,
      durationMs: 0,
      seq,
    };
    const start = performance.now();
    try {
      const result = await original(cmd, args);
      entry.ok = true;
      entry.result = result;
      entry.durationMs = performance.now() - start;
      log.push(entry);
      return result;
    } catch (err) {
      entry.ok = false;
      entry.error = err;
      entry.durationMs = performance.now() - start;
      log.push(entry);
      throw err;
    }
  };

  w.__pixhaus_ipc_tap_installed__ = true;
}
