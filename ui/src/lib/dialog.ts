// Wrapper around @tauri-apps/plugin-dialog that supports test-time
// mocking of OS-native dialogs.
//
// In production WebView2 / WebKitGTK, opening a native file picker
// suspends the renderer thread until the user clicks. WebDriver can't
// drive native dialogs, so any spec that triggers `open()` or `save()`
// would hang. The mock queue here lets specs prime responses before the
// triggering action and the wrapper drains the queue first; if empty,
// it falls through to the real plugin so dev sessions still get the
// native picker.
//
// All UI code that needs file/save dialogs imports from this module
// instead of `@tauri-apps/plugin-dialog` directly. Signatures mirror
// the real plugin so call sites don't need changes beyond the import
// path.

import * as plugin from "@tauri-apps/plugin-dialog";

type OpenReturn = string | string[] | null;

interface DialogQueue {
  open: OpenReturn[];
  save: (string | null)[];
  confirm: boolean[];
  message: plugin.MessageDialogResult[];
}

const queue: DialogQueue = {
  open: [],
  save: [],
  confirm: [],
  message: [],
};

export async function open<T extends plugin.OpenDialogOptions>(
  options?: T,
): Promise<plugin.OpenDialogReturn<T>> {
  if (queue.open.length > 0) {
    const value = queue.open.shift() ?? null;
    return value as plugin.OpenDialogReturn<T>;
  }
  return plugin.open(options);
}

export async function save(options?: plugin.SaveDialogOptions): Promise<string | null> {
  if (queue.save.length > 0) {
    return queue.save.shift() ?? null;
  }
  return plugin.save(options);
}

export async function confirm(
  msg: string,
  options?: string | plugin.ConfirmDialogOptions,
): Promise<boolean> {
  if (queue.confirm.length > 0) {
    return queue.confirm.shift() ?? false;
  }
  return plugin.confirm(msg, options);
}

export async function message(
  msg: string,
  options?: string | plugin.MessageDialogOptions,
): Promise<plugin.MessageDialogResult> {
  if (queue.message.length > 0) {
    return queue.message.shift() ?? "Ok";
  }
  return plugin.message(msg, options);
}

// Test-only API. Specs call these via window.__pixhaus_debug__.dialog
// before triggering the action that would open the native dialog.

export function enqueueOpen(value: OpenReturn): void {
  queue.open.push(value);
}

export function enqueueSave(value: string | null): void {
  queue.save.push(value);
}

export function enqueueConfirm(value: boolean): void {
  queue.confirm.push(value);
}

export function enqueueMessage(value: plugin.MessageDialogResult): void {
  queue.message.push(value);
}

export function clearDialogQueue(): void {
  queue.open.length = 0;
  queue.save.length = 0;
  queue.confirm.length = 0;
  queue.message.length = 0;
}
