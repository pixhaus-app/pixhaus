// Helpers for positioning floating menus / popovers near the cursor.
//
// Right-click context menus are anchored at clientX/clientY. Without a
// post-mount measure they overflow the viewport whenever the user
// right-clicks near the bottom or right edge.

export type Point = { readonly x: number; readonly y: number };

const VIEWPORT_MARGIN = 4;

/**
 * Returns a clamped position so a menu of size `width × height` stays inside
 * `window.innerWidth × window.innerHeight` with a small margin. Pure helper
 * for unit testing; the runtime path uses `clampMenuToViewport()`.
 */
export function clampPositionToBounds(
  x: number,
  y: number,
  width: number,
  height: number,
  viewportWidth: number,
  viewportHeight: number,
  margin = VIEWPORT_MARGIN,
): Point {
  const maxX = Math.max(margin, viewportWidth - width - margin);
  const maxY = Math.max(margin, viewportHeight - height - margin);
  return {
    x: Math.max(margin, Math.min(x, maxX)),
    y: Math.max(margin, Math.min(y, maxY)),
  };
}

/**
 * Callback ref helper. Wires up `el` so that after the next paint its
 * `left` / `top` are clamped to the viewport. The element must be
 * `position: fixed` (or absolutely positioned in a fixed container) for
 * the px values to make sense.
 */
export function clampMenuToViewport(x: number, y: number) {
  return (el: HTMLElement): void => {
    // Measure on the next tick so the element has been laid out.
    requestAnimationFrame(() => {
      const rect = el.getBoundingClientRect();
      const clamped = clampPositionToBounds(
        x,
        y,
        rect.width,
        rect.height,
        window.innerWidth,
        window.innerHeight,
      );
      el.style.left = `${clamped.x}px`;
      el.style.top = `${clamped.y}px`;
    });
  };
}
