import { describe, expect, it } from "vitest";
import { createRoot, createSignal } from "solid-js";

import { useAnimationLoop } from "./use-animation-loop";

/**
 * Fake scheduler — captures the pending callback so tests can drive
 * time deterministically. Each `tick(ms)` calls the captured callback
 * with the new timestamp.
 */
function makeScheduler() {
  let next: ((t: number) => void) | null = null;
  let now = 0;
  return {
    request: (cb: (t: number) => void) => {
      next = cb;
      return 1;
    },
    cancel: () => {
      next = null;
    },
    now: () => now,
    tick(ms: number) {
      now += ms;
      const cb = next;
      next = null;
      cb?.(now);
    },
    drain(times: number, ms: number) {
      for (let i = 0; i < times; i++) this.tick(ms);
    },
  };
}

describe("useAnimationLoop", () => {
  it("advances the frame index at the requested FPS", () => {
    createRoot((dispose) => {
      const sched = makeScheduler();
      const [playing] = createSignal(true);
      const [fps] = createSignal(10); // 100 ms per frame
      const [frames] = createSignal(4);
      const idx = useAnimationLoop({
        playing,
        fps,
        frameCount: frames,
        raf: sched,
      });

      // First tick seeds the timing anchor — no advance.
      sched.tick(50);
      expect(idx()).toBe(0);

      // Less than the interval since the anchor — no advance.
      sched.tick(40);
      expect(idx()).toBe(0);

      // Cross the 100 ms interval (from the anchor at 50 ms → now at
      // 160 ms means 110 ms elapsed) — advance once.
      sched.tick(70);
      expect(idx()).toBe(1);

      // Step forward in 120 ms increments and wrap.
      sched.tick(120);
      expect(idx()).toBe(2);
      sched.tick(120);
      expect(idx()).toBe(3);
      sched.tick(120);
      expect(idx()).toBe(0);

      dispose();
    });
  });

  it("does not advance while paused", () => {
    createRoot((dispose) => {
      const sched = makeScheduler();
      const [playing, setPlaying] = createSignal(false);
      const [fps] = createSignal(10);
      const [frames] = createSignal(4);
      const idx = useAnimationLoop({
        playing,
        fps,
        frameCount: frames,
        raf: sched,
      });

      sched.drain(10, 200);
      expect(idx()).toBe(0);

      setPlaying(true);
      // First post-resume tick anchors `last`. Second crosses the interval.
      sched.tick(200);
      expect(idx()).toBe(0);
      sched.tick(200);
      expect(idx()).toBe(1);

      dispose();
    });
  });

  it("ignores frame counts <= 1 (no wrap means stay at 0)", () => {
    createRoot((dispose) => {
      const sched = makeScheduler();
      const [playing] = createSignal(true);
      const [fps] = createSignal(60);
      const [frames] = createSignal(1);
      const idx = useAnimationLoop({
        playing,
        fps,
        frameCount: frames,
        raf: sched,
      });

      sched.drain(5, 50);
      expect(idx()).toBe(0);
      dispose();
    });
  });

  it("calls onTick with the new index", () => {
    createRoot((dispose) => {
      const sched = makeScheduler();
      const [playing] = createSignal(true);
      const [fps] = createSignal(10);
      const [frames] = createSignal(3);
      const seen: number[] = [];
      useAnimationLoop({
        playing,
        fps,
        frameCount: frames,
        onTick: (n) => seen.push(n),
        raf: sched,
      });

      sched.tick(150); // anchor — no advance
      sched.tick(150); // → 1
      sched.tick(150); // → 2
      sched.tick(150); // → 0 (wrap)
      expect(seen).toEqual([1, 2, 0]);
      dispose();
    });
  });

  it("resets timing when paused so resume doesn't jump", () => {
    createRoot((dispose) => {
      const sched = makeScheduler();
      const [playing, setPlaying] = createSignal(true);
      const [fps] = createSignal(10);
      const [frames] = createSignal(4);
      const idx = useAnimationLoop({
        playing,
        fps,
        frameCount: frames,
        raf: sched,
      });

      sched.tick(150); // anchor
      sched.tick(150); // advance to 1
      expect(idx()).toBe(1);

      // Pause for 5 seconds.
      setPlaying(false);
      sched.drain(50, 100);
      expect(idx()).toBe(1);

      // Resume — the next tick should NOT immediately advance (the
      // reset re-anchors `last` so the bottled-up interval does not
      // fire).
      setPlaying(true);
      sched.tick(50); // re-anchor
      expect(idx()).toBe(1);
      sched.tick(120); // advance
      expect(idx()).toBe(2);
      dispose();
    });
  });
});
