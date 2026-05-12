// Reactive object URL for raw image bytes.
//
// Builds a Blob + `URL.createObjectURL` once per source value, revokes the
// previous URL when the source changes, and revokes the final URL on owner
// cleanup. Use this in place of a `data:base64,...` URL when the source
// would otherwise re-encode multi-megabyte buffers on every render.

import { createEffect, createSignal, onCleanup, type Accessor } from "solid-js";

/**
 * Returns a reactive accessor for an object URL pointing at `source`.
 *
 * The accessor resolves to an empty string when `source()` is null or
 * carries no bytes — render an `<img>` placeholder against that case.
 */
export function useImageObjectUrl(
  source: Accessor<{ bytes: number[]; mime: string } | null>,
): Accessor<string> {
  const [url, setUrl] = createSignal("");
  createEffect(() => {
    const src = source();
    if (src === null || src.bytes.length === 0) {
      setUrl("");
      return;
    }
    const blob = new Blob([new Uint8Array(src.bytes)], { type: src.mime });
    const u = URL.createObjectURL(blob);
    setUrl(u);
    onCleanup(() => URL.revokeObjectURL(u));
  });
  return url;
}
