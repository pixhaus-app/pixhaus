// Returns the trailing path component (filename) for a Windows or POSIX path.
// Falls back to the input itself when there is no separator.
export function extractFilename(path: string): string {
  const parts = path.split(/[\\/]/);
  return parts[parts.length - 1] ?? path;
}
