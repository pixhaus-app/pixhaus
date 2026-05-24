// Mutation helper.
//
// Collapses the pattern repeated across every domain —
//   someCmd(...).then(() => refreshX()).catch((e) => console.error(e))
// — into one call that, on success, invalidates the affected queries and,
// on failure, rolls back any optimistic change and shows the user a toast
// (the old code only logged to the console, so failures were invisible).
//
// Backend commands return Result<T, AppCommandError>; a rejection carries
// the serialized AppCommandError, which humanizeCommandError turns into a
// readable line.

import { invalidate } from "./invalidation";
import { pushToast } from "../toast/toast-state";
import type { AppCommandError } from "../types/AppCommandError";

export interface RunMutationOptions<T> {
  /** The IPC call to run. */
  run: () => Promise<T>;
  /** Query keys to refetch on success. */
  invalidate?: string[];
  /**
   * Optimistic update applied before the call and rolled back if it rejects.
   * `apply` runs synchronously before `run`; `rollback` runs on rejection.
   */
  optimistic?: { apply: () => void; rollback: () => void };
  /**
   * Toast title on failure. A string, a builder from the error, or false to
   * suppress the toast (the caller handles errors itself). Defaults to a
   * humanized title derived from the error kind.
   */
  errorToast?: string | ((err: unknown) => string) | false;
  /** Extra work to run after a successful call, before returning. */
  onSuccess?: (value: T) => void;
}

/**
 * Runs a backend mutation with uniform success/failure handling. Resolves to
 * the command's value on success, or `undefined` on failure (already toasted
 * unless suppressed) so callers can branch without a try/catch.
 */
export async function runMutation<T>(opts: RunMutationOptions<T>): Promise<T | undefined> {
  opts.optimistic?.apply();
  try {
    const value = await opts.run();
    if (opts.invalidate !== undefined && opts.invalidate.length > 0) {
      invalidate(...opts.invalidate);
    }
    opts.onSuccess?.(value);
    return value;
  } catch (err) {
    opts.optimistic?.rollback();
    if (opts.errorToast !== false) {
      const title =
        typeof opts.errorToast === "function"
          ? opts.errorToast(err)
          : (opts.errorToast ?? defaultErrorTitle(err));
      pushToast({ kind: "error", title, body: humanizeCommandError(err) });
    }
    return undefined;
  }
}

function defaultErrorTitle(err: unknown): string {
  if (isAppCommandError(err)) {
    switch (err.kind) {
      case "no_active_project":
        return "No project open";
      case "not_found":
      case "not_found_by_name":
        return "Not found";
      case "layer_locked":
        return "Layer is locked";
      case "validation":
      case "out_of_range":
        return "Invalid input";
      case "conflict":
        return "Conflict";
      case "unimplemented":
        return "Not available yet";
      case "nothing_to_undo":
        return "Nothing to undo";
      case "nothing_to_redo":
        return "Nothing to redo";
      default:
        return "Operation failed";
    }
  }
  return "Operation failed";
}

/** A readable one-line detail from an AppCommandError (or any thrown value). */
export function humanizeCommandError(err: unknown): string {
  if (isAppCommandError(err)) {
    switch (err.kind) {
      case "no_active_project":
        return "Open or create a project first.";
      case "not_found":
        return `${capitalize(err.message.entity)} #${err.message.id.toString()} no longer exists.`;
      case "not_found_by_name":
        return `${capitalize(err.message.entity)} "${err.message.name}" not found.`;
      case "out_of_range":
        return err.message.detail;
      case "conflict":
        return err.message.detail;
      case "validation":
        return err.message.detail;
      case "unimplemented":
        return `Coming with ${err.message.stream}.`;
      case "history_corrupted":
        return err.message.detail;
      case "verb_error":
        return err.message.message;
      case "layer_locked":
        return `Layer #${err.message.layer_id.toString()} (or its group) is locked.`;
      case "nothing_to_undo":
        return "There is nothing to undo.";
      case "nothing_to_redo":
        return "There is nothing to redo.";
      default:
        return "Unexpected error.";
    }
  }
  if (typeof err === "string") return err;
  if (err instanceof Error) return err.message;
  return "Unexpected error.";
}

function isAppCommandError(err: unknown): err is AppCommandError {
  return err !== null && typeof err === "object" && "kind" in err;
}

function capitalize(s: string): string {
  return s.length === 0 ? s : s[0]!.toUpperCase() + s.slice(1);
}
