// Helpers for asserting against window.__pixhaus_ipc_log__.
//
// Specs use these to verify the manual-test-guide's `[IPC]` assertions:
// "after action X, command Y fired with args Z". The log captures every
// IPC roundtrip, so a typical spec calls clearIpcLog() before the act
// phase and then expects the named commands.

import { browser } from "@wdio/globals";

export interface IpcLogEntry {
  cmd: string;
  args: unknown;
  ok: boolean;
  result?: unknown;
  error?: unknown;
  durationMs: number;
  seq: number;
}

/** Returns a copy of the entire IPC log. */
export async function getIpcLog(): Promise<IpcLogEntry[]> {
  return browser.execute(() => {
    const w = window as unknown as {
      __pixhaus_debug__: { ipc: { log(): IpcLogEntry[] } };
    };
    return w.__pixhaus_debug__.ipc.log();
  }) as Promise<IpcLogEntry[]>;
}

/** Empties the IPC log. Call before the act phase of a test. */
export async function clearIpcLog(): Promise<void> {
  await browser.execute(() => {
    const w = window as unknown as {
      __pixhaus_debug__: { ipc: { clear(): void } };
    };
    w.__pixhaus_debug__.ipc.clear();
  });
}

/** Returns log entries matching `cmd`, in original order. */
export async function findIpcByCmd(cmd: string): Promise<IpcLogEntry[]> {
  return browser.execute((c: string) => {
    const w = window as unknown as {
      __pixhaus_debug__: { ipc: { findByCmd(cmd: string): IpcLogEntry[] } };
    };
    return w.__pixhaus_debug__.ipc.findByCmd(c);
  }, cmd) as Promise<IpcLogEntry[]>;
}

/**
 * Polls the IPC log until at least `count` entries with the given cmd
 * exist. Useful when the action is async and the IPC fires after a
 * RAF tick or microtask.
 */
export async function waitForIpc(
  cmd: string,
  count = 1,
  timeoutMs = 5000,
): Promise<IpcLogEntry[]> {
  let entries: IpcLogEntry[] = [];
  await browser.waitUntil(
    async () => {
      entries = await findIpcByCmd(cmd);
      return entries.length >= count;
    },
    {
      timeout: timeoutMs,
      timeoutMsg: `expected >= ${count} '${cmd}' IPC, saw ${entries.length}`,
    },
  );
  return entries;
}

/**
 * Asserts the next N entries (from current log state) match the given
 * cmds in order. Use when the action triggers a deterministic chain
 * (e.g. "New Project" fires project_new → sprite_add → project_get).
 */
export async function expectIpcSequence(
  expected: readonly string[],
): Promise<void> {
  const log = await getIpcLog();
  const observed = log.map((e) => e.cmd);
  // Find the first index where the expected sequence starts.
  for (let i = 0; i + expected.length <= observed.length; i++) {
    let match = true;
    for (let j = 0; j < expected.length; j++) {
      if (observed[i + j] !== expected[j]) {
        match = false;
        break;
      }
    }
    if (match) return;
  }
  throw new Error(
    `IPC sequence ${JSON.stringify(expected)} not found in log:\n  ${observed.join(" -> ")}`,
  );
}
