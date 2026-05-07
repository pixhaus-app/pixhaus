// Coerces a UI rect (float pixels) to the IPC contract's integer-rect
// shape. The IPC layer expects a non-zero size; sub-pixel widths round
// up to 1 to keep round-trips stable.
export type IpcRect = {
  origin: { x: number; y: number };
  size: { width: number; height: number };
};

export function toIpcRect(bounds: {
  x: number;
  y: number;
  width: number;
  height: number;
}): IpcRect {
  return {
    origin: { x: Math.round(bounds.x), y: Math.round(bounds.y) },
    size: {
      width: Math.max(1, Math.round(bounds.width)),
      height: Math.max(1, Math.round(bounds.height)),
    },
  };
}
