// Helpers for priming the native-dialog mock queue from a spec.
//
// The UI's `ui/src/lib/dialog.ts` wraps `@tauri-apps/plugin-dialog`. When
// the queue holds a value, the wrapper returns it instead of opening a
// real OS picker. WebDriver can't drive native dialogs, so every spec
// that triggers `open()` / `save()` / `confirm()` / `message()` must
// pre-queue its response.
//
// Specs call these helpers BEFORE the action that triggers the dialog.

import { browser } from "@wdio/globals";

/** Queues the next `open()` dialog to return `value`. */
export async function mockOpenDialog(
  value: string | string[] | null,
): Promise<void> {
  await browser.execute((v) => {
    const w = window as unknown as {
      __pixhaus_debug__: {
        dialog: { enqueueOpen(v: string | string[] | null): void };
      };
    };
    w.__pixhaus_debug__.dialog.enqueueOpen(v);
  }, value);
}

/** Queues the next `save()` dialog to return `value`. */
export async function mockSaveDialog(value: string | null): Promise<void> {
  await browser.execute((v) => {
    const w = window as unknown as {
      __pixhaus_debug__: { dialog: { enqueueSave(v: string | null): void } };
    };
    w.__pixhaus_debug__.dialog.enqueueSave(v);
  }, value);
}

/** Queues the next `confirm()` dialog to return `value`. */
export async function mockConfirmDialog(value: boolean): Promise<void> {
  await browser.execute((v) => {
    const w = window as unknown as {
      __pixhaus_debug__: { dialog: { enqueueConfirm(v: boolean): void } };
    };
    w.__pixhaus_debug__.dialog.enqueueConfirm(v);
  }, value);
}

/** Empties the dialog queue between tests. Idempotent. */
export async function clearDialogQueue(): Promise<void> {
  await browser.execute(() => {
    const w = window as unknown as {
      __pixhaus_debug__: { dialog: { clear(): void } };
    };
    w.__pixhaus_debug__.dialog.clear();
  });
}
