// FPS-gated `requestAnimationFrame` loop primitive for Solid.
//
// Adapted from FalSprite (https://github.com/lovisdotio/falsprite)
// `public/app.js:469-527`. MIT-licensed, Copyright (c) 2026 lovisdotio.
// See `LICENSES/falsprite-MIT.txt` for the full upstream license text
// and `LICENSES/NOTICE.txt` for the project-wide attribution registry.
//
// The playback gate (`now - last >= 1000 / fps`) decouples the loop's
// tick rate from the monitor refresh, so a 60 Hz display animating an
// 8 fps sprite ticks 8 times a second rather than every frame. The
// pause/resume re-anchor (set `last = -1` on pause, seed on the first
// playing tick) keeps the bottled-up interval from firing at once when
// playback resumes.

import { createSignal, onCleanup } from "solid-js";

/**
 * Drives a frame index forward at the requested FPS while `playing` is
 * truthy. Returns the current frame index as a Solid signal.
 *
 * - `fps` and `playing` are *accessors* (functions returning the current
 *   value) so the loop reads the freshest reactive state on every tick
 *   without recreating the loop when they change.
 * - `frameCount` likewise. When `frameCount() <= 1`, the loop never
 *   advances past 0.
 * - `onTick` runs on each frame advance with the *new* index.
 *
 * The loop is one `requestAnimationFrame` and one `cancelAnimationFrame`
 * — Solid's `onCleanup` tears it down when the host component unmounts.
 */
export type AnimationScheduler = {
  request: (cb: (timestamp: number) => void) => number;
  cancel: (handle: number) => void;
  now: () => number;
};

export function useAnimationLoop(opts: {
  fps: () => number;
  playing: () => boolean;
  frameCount: () => number;
  onTick?: (frameIndex: number) => void;
  initial?: number;
  /**
   * Injected RAF / cancellation pair for tests. Defaults to `window`
   * when omitted. The shape matches the standard browser globals so
   * tests can swap in a fake scheduler.
   */
  raf?: AnimationScheduler;
}): () => number {
  const [index, setIndex] = createSignal(Math.max(0, opts.initial ?? 0));
  let last = -1;
  let handle = 0;

  const defaultScheduler: AnimationScheduler = {
    request: (cb: (timestamp: number) => void): number => window.requestAnimationFrame(cb),
    cancel: (h: number): void => window.cancelAnimationFrame(h),
    now: (): number => performance.now(),
  };
  const sched: AnimationScheduler = opts.raf ?? defaultScheduler;

  const tick = (now: number): void => {
    if (opts.playing()) {
      const interval = 1000 / Math.max(1, opts.fps());
      // `last < 0` is the post-pause / first-frame anchor: seed it to
      // `now` so the very next tick measures from here rather than
      // firing the bottled-up interval at once.
      if (last < 0) {
        last = now;
      } else if (now - last >= interval) {
        last = now;
        const total = Math.max(1, opts.frameCount());
        const next = (index() + 1) % total;
        setIndex(next);
        opts.onTick?.(next);
      }
    } else {
      // Reset the timing reference so the next play-press doesn't snap
      // forward by the accumulated paused interval.
      last = -1;
    }
    handle = sched.request(tick);
  };

  handle = sched.request(tick);
  onCleanup(() => sched.cancel(handle));
  return index;
}
