// Toast queue state. Module-level so any non-component code (catch
// blocks, IPC error helpers) can surface a message without threading
// a context through. ToastHost reads this signal to render the stack.

import { createSignal } from "solid-js";

export type ToastKind = "error" | "info" | "success";

export interface Toast {
  id: number;
  kind: ToastKind;
  title: string;
  /** Optional second line — shown smaller, wraps. */
  body?: string;
}

export interface PushToastInput {
  kind?: ToastKind;
  title: string;
  body?: string;
  /** Auto-dismiss delay in ms. Set to 0 to require manual dismiss. */
  durationMs?: number;
}

const DEFAULT_DURATION_MS = 6000;

let nextId = 1;

export const [toasts, setToasts] = createSignal<Toast[]>([]);

export function pushToast(input: PushToastInput): number {
  const toast: Toast = {
    id: nextId++,
    kind: input.kind ?? "info",
    title: input.title,
    ...(input.body !== undefined ? { body: input.body } : {}),
  };
  setToasts((prev) => [...prev, toast]);
  const duration = input.durationMs ?? DEFAULT_DURATION_MS;
  if (duration > 0) {
    setTimeout(() => dismissToast(toast.id), duration);
  }
  return toast.id;
}

export function dismissToast(id: number): void {
  setToasts((prev) => prev.filter((t) => t.id !== id));
}

export function clearToasts(): void {
  setToasts([]);
}
