// Renders the toast stack from `toastState.toasts`. Mounted once at the top of
// Shell so every surface-level catch block can call `pushToast()`
// without owning a portal.

import { For, type Component } from "solid-js";
import { dismissToast, toastState } from "./toast-state";

const ToastHost: Component = () => {
  return (
    <div class="toast-host" role="region" aria-label="Notifications" aria-live="polite">
      <For each={toastState.toasts}>
        {(t) => (
          <div class={`toast toast--${t.kind}`} role={t.kind === "error" ? "alert" : "status"}>
            <div class="toast__body">
              <div class="toast__title">{t.title}</div>
              {t.body !== undefined && <div class="toast__detail">{t.body}</div>}
            </div>
            <button
              class="toast__close"
              type="button"
              aria-label="Dismiss"
              onClick={() => dismissToast(t.id)}
            >
              ×
            </button>
          </div>
        )}
      </For>
    </div>
  );
};

export default ToastHost;
