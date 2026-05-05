// Surfaces a failed IPC command to the user. Today this falls back on
// `window.alert`; once a toast/snackbar lands, this is the single
// chokepoint to swap. Keeps the catch-block at every call-site to one
// line and keeps the message format consistent.
export function reportCommandFailure(operation: string, err: unknown): void {
  console.error(`[pixhaus] ${operation}:`, err);
  const detail =
    err !== null && typeof err === "object" && "message" in err
      ? String((err as { message: unknown }).message)
      : String(err);
  window.alert(`${operation} failed: ${detail}`);
}
