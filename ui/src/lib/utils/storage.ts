// Reads a JSON value from localStorage, validates it, and returns the
// fallback when the key is missing, the JSON is malformed, or validation
// fails. Errors are swallowed so callers can rely on always getting a usable
// value at startup.
export function loadStorageJSON<T>(
  key: string,
  fallback: T,
  validate: (raw: unknown) => raw is T,
): T {
  try {
    const raw = localStorage.getItem(key);
    if (raw === null) return fallback;
    const parsed: unknown = JSON.parse(raw);
    return validate(parsed) ? parsed : fallback;
  } catch {
    return fallback;
  }
}
