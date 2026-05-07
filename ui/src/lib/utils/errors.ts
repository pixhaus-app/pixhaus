// Surfaces a failed IPC command to the user. Routes through the toast
// host so the editor stays interactive while the message is on screen
// (alert() blocks the event loop). Keeps the catch-block at every
// call-site to one line and keeps the message format consistent.
import { pushToast } from "../toast/toast-state";

export function reportCommandFailure(operation: string, err: unknown): void {
  console.error(`[pixhaus] ${operation}:`, err);
  const detail =
    err !== null && typeof err === "object" && "message" in err
      ? String((err as { message: unknown }).message)
      : String(err);
  pushToast({
    kind: "error",
    title: `${operation} failed`,
    body: detail,
  });
}

export type UnimplementedError = { kind: "unimplemented"; stream?: string };

export function isUnimplementedError(err: unknown): err is UnimplementedError {
  return (
    err !== null && typeof err === "object" && (err as { kind?: unknown }).kind === "unimplemented"
  );
}

// Routes a stub-command rejection to an info toast keyed to the stream
// that will implement it. `label` names the user-facing feature
// (e.g. "Magic wand"); `defaultStream` is used when the IPC error did
// not include one.
export function toastUnimplemented(
  label: string,
  err: UnimplementedError,
  defaultStream: string,
): void {
  pushToast({
    kind: "info",
    title: `${label} requires ${err.stream ?? defaultStream} — not yet available.`,
  });
}
