import { createEffect, createUniqueId, onCleanup, Show, type JSX } from "solid-js";
import { Portal } from "solid-js/web";

type Size = "sm" | "md" | "lg" | "full";
type InitialFocus = "first-input" | "primary" | "none";

interface DialogProps {
  open: boolean;
  title: string;
  onClose: () => void;
  size?: Size;
  closeOnBackdrop?: boolean;
  closeOnEscape?: boolean;
  initialFocus?: InitialFocus;
  testId?: string;
  children: JSX.Element;
}

interface SectionProps {
  class?: string;
  children: JSX.Element;
}

const FOCUSABLE =
  'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

function focusableElements(root: HTMLElement): HTMLElement[] {
  return Array.from(root.querySelectorAll<HTMLElement>(FOCUSABLE)).filter(
    (el) => el.offsetParent !== null || el === document.activeElement,
  );
}

export function Dialog(props: DialogProps) {
  const titleId = createUniqueId();
  let panelRef: HTMLDivElement | undefined;
  let previousFocus: HTMLElement | null = null;

  createEffect(() => {
    if (!props.open) return;

    previousFocus = document.activeElement as HTMLElement | null;
    const previousOverflow = document.body.style.overflow;
    document.body.style.overflow = "hidden";

    queueMicrotask(() => {
      if (!panelRef) return;
      const mode = props.initialFocus ?? "first-input";
      if (mode === "none") return;
      const candidates = focusableElements(panelRef);
      if (candidates.length === 0) return;
      if (mode === "first-input") {
        const input = candidates.find((el) => el.matches("input, textarea, select"));
        (input ?? candidates[0])?.focus();
      } else {
        const primary = candidates.find((el) => el.classList.contains("btn--primary"));
        (primary ?? candidates[0])?.focus();
      }
    });

    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape" && (props.closeOnEscape ?? true)) {
        e.preventDefault();
        props.onClose();
        return;
      }
      if (e.key === "Tab" && panelRef) {
        const focs = focusableElements(panelRef);
        const first = focs[0];
        const last = focs[focs.length - 1];
        if (!first || !last) return;
        const active = document.activeElement as HTMLElement | null;
        if (e.shiftKey && (active === first || !panelRef.contains(active))) {
          e.preventDefault();
          last.focus();
        } else if (!e.shiftKey && (active === last || !panelRef.contains(active))) {
          e.preventDefault();
          first.focus();
        }
      }
    };
    document.addEventListener("keydown", onKey);

    onCleanup(() => {
      document.removeEventListener("keydown", onKey);
      document.body.style.overflow = previousOverflow;
      if (previousFocus && document.contains(previousFocus)) {
        previousFocus.focus();
      }
    });
  });

  // Pair mousedown + click so a text-selection drag that starts inside the
  // dialog body and releases on the backdrop doesn't dismiss the dialog
  // (and the inverse — drag started on backdrop, released on panel — also
  // doesn't dismiss). The previous one-shot mousedown handler dismissed
  // on either mismatched path.
  let mouseDownOnBackdrop = false;

  const onBackdropMouseDown = (e: MouseEvent) => {
    mouseDownOnBackdrop = e.target === e.currentTarget;
  };

  const onBackdropClick = (e: MouseEvent) => {
    const dismiss =
      mouseDownOnBackdrop && e.target === e.currentTarget && (props.closeOnBackdrop ?? true);
    mouseDownOnBackdrop = false;
    if (dismiss) props.onClose();
  };

  const sizeClass = () => `dialog--${props.size ?? "md"}`;

  return (
    <Show when={props.open}>
      <Portal>
        <div class="dialog__backdrop" onMouseDown={onBackdropMouseDown} onClick={onBackdropClick}>
          <div
            ref={panelRef}
            class={`dialog ${sizeClass()}`}
            role="dialog"
            aria-modal="true"
            aria-labelledby={titleId}
            data-testid={props.testId}
          >
            <header class="dialog__header">
              <h2 class="dialog__title" id={titleId}>
                {props.title}
              </h2>
              <Show when={(props.closeOnEscape ?? true) || (props.closeOnBackdrop ?? true)}>
                <button
                  type="button"
                  class="dialog__close"
                  aria-label="Close"
                  onClick={() => props.onClose()}
                >
                  ×
                </button>
              </Show>
            </header>
            {props.children}
          </div>
        </div>
      </Portal>
    </Show>
  );
}

Dialog.Body = function DialogBody(props: SectionProps) {
  return <div class={`dialog__body${props.class ? ` ${props.class}` : ""}`}>{props.children}</div>;
};

Dialog.Footer = function DialogFooter(props: SectionProps) {
  return (
    <div class={`dialog__footer${props.class ? ` ${props.class}` : ""}`}>{props.children}</div>
  );
};
