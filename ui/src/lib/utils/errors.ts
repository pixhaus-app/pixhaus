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
