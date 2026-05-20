// Reusable hover/focus tooltip.
//
// Wraps a trigger element in an inline anchor that opens a portalled
// tooltip on hover or keyboard focus. The tooltip can show a title, a
// styled shortcut chip, and a one-line description — richer than the
// native `title` attribute, and instant rather than the OS delay.
//
// The body is rendered through a Portal so the rail's `overflow: hidden`
// never clips it, and positioned with fixed coordinates derived from the
// anchor's bounding box.

import { createSignal, createUniqueId, onCleanup, Show, type JSX, type Component } from "solid-js";
import { Portal } from "solid-js/web";

type Placement = "right" | "left" | "top" | "bottom";

type Props = {
  /** Bold first line. */
  label: string;
  /** Optional shortcut shown as a chip, e.g. "W" or "Ctrl+Shift+I". */
  shortcut?: string | undefined;
  /** Optional one-line description under the label. */
  description?: string | undefined;
  /** Side of the trigger to place the tooltip. Defaults to "right". */
  placement?: Placement | undefined;
  /** Show delay in ms. Defaults to 200. */
  delay?: number | undefined;
  children: JSX.Element;
};

const GAP = 8;

const Tooltip: Component<Props> = (props) => {
  const [open, setOpen] = createSignal(false);
  const [pos, setPos] = createSignal<{ x: number; y: number }>({ x: 0, y: 0 });
  const id = createUniqueId();
  let anchorEl: HTMLSpanElement | undefined;
  let timer: number | undefined;

  const placement = () => props.placement ?? "right";

  const place = (): void => {
    if (!anchorEl) return;
    const r = anchorEl.getBoundingClientRect();
    switch (placement()) {
      case "right":
        setPos({ x: r.right + GAP, y: r.top + r.height / 2 });
        break;
      case "left":
        setPos({ x: r.left - GAP, y: r.top + r.height / 2 });
        break;
      case "top":
        setPos({ x: r.left + r.width / 2, y: r.top - GAP });
        break;
      case "bottom":
        setPos({ x: r.left + r.width / 2, y: r.bottom + GAP });
        break;
    }
  };

  const show = (): void => {
    // Deferred positioning: we want the prop values as they are when the
    // timer fires, not a tracked subscription — this runs once per hover.
    // eslint-disable-next-line solid/reactivity
    timer = window.setTimeout(() => {
      place();
      setOpen(true);
    }, props.delay ?? 200);
  };

  const hide = (): void => {
    if (timer !== undefined) {
      clearTimeout(timer);
      timer = undefined;
    }
    setOpen(false);
  };

  onCleanup(hide);

  return (
    <span
      ref={anchorEl}
      class="tt-anchor"
      onMouseEnter={show}
      onMouseLeave={hide}
      onFocusIn={() => {
        place();
        setOpen(true);
      }}
      onFocusOut={hide}
      aria-describedby={open() ? id : undefined}
    >
      {props.children}
      <Show when={open()}>
        <Portal>
          <div
            id={id}
            role="tooltip"
            class="tt"
            data-placement={placement()}
            style={{ left: `${pos().x}px`, top: `${pos().y}px` }}
          >
            <div class="tt__head">
              <span class="tt__label">{props.label}</span>
              <Show when={props.shortcut}>
                <kbd class="tt__kbd">{props.shortcut}</kbd>
              </Show>
            </div>
            <Show when={props.description}>
              <p class="tt__desc">{props.description}</p>
            </Show>
          </div>
        </Portal>
      </Show>
    </span>
  );
};

export default Tooltip;
