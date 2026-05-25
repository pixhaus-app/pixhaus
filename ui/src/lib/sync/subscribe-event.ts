// Component-scoped Tauri event subscription.
//
// Every component that listens for a backend event repeated the same dance:
// hold the listen() promise (not the resolved unlisten, so an unmount before
// the promise resolves still detaches), then await-and-call it in onCleanup.
// This helper collapses that to one line.
//
// Handlers stay component-local on purpose: the canvas tile-dirty handler
// drives the WebGL renderer instance and the AI panels' handlers write
// component signals during streaming, so their state is bound to the
// component lifecycle, not an app-global registry.

import { onCleanup } from "solid-js";
import { listen } from "@tauri-apps/api/event";

/**
 * Subscribes to a Tauri event for the lifetime of the calling reactive scope.
 * Must be called during component setup (it registers an onCleanup). The
 * subscription is detached on cleanup even if it resolves after unmount.
 */
export function subscribeTauriEvent<T>(event: string, handler: (payload: T) => void): void {
  const pending = listen<T>(event, (e) => handler(e.payload));
  onCleanup(() => {
    void pending
      .then((unlisten) => unlisten())
      .catch((err: unknown) => console.error(`[pixhaus] failed to unlisten "${event}":`, err));
  });
}
