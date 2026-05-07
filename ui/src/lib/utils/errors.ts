// Surfaces a failed IPC command to the user. Routes through the toast
// host so the editor stays interactive while the message is on screen
// (alert() blocks the event loop). Keeps the catch-block at every
// call-site to one line and keeps the message format consistent.
import { pushToast } from "../toast/toast-state";
import type { AppCommandError } from "../types/AppCommandError";

// AppCommandError on the wire is `{ kind, message?: ... }` where struct
// variants put their fields inside `message` (e.g. `{ kind: "validation",
// message: { detail: "..." } }`). Pull the most informative string out of
// whatever shape is there; fall back to String(err) for non-IPC errors.
function extractDetail(err: unknown): string {
  if (err === null || typeof err !== "object") return String(err);
  const e = err as { kind?: unknown; message?: unknown };
  const message = e.message;
  if (typeof message === "string") return message;
  if (message !== null && typeof message === "object") {
    const m = message as Record<string, unknown>;
    if (typeof m["detail"] === "string") return m["detail"];
    if (typeof m["message"] === "string") return m["message"];
    if (typeof m["stream"] === "string") return `requires ${m["stream"]}`;
  }
  if (typeof e.kind === "string") return e.kind;
  return String(err);
}

export function reportCommandFailure(operation: string, err: unknown): void {
  console.error(`[pixhaus] ${operation}:`, err);
  pushToast({
    kind: "error",
    title: `${operation} failed`,
    body: extractDetail(err),
  });
}

// `Unimplemented` is the IPC variant for stub commands that wait on a
// future stream. The wire shape matches the generated AppCommandError
// type — `stream` lives inside `message`, not at the top level.
export type UnimplementedError = Extract<AppCommandError, { kind: "unimplemented" }>;

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
  const stream = err.message?.stream ?? defaultStream;
  pushToast({
    kind: "info",
    title: `${label} requires ${stream} — not yet available.`,
  });
}
