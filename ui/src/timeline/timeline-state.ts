// Reactive state for the timeline panel.
//
// Frame, tag, and cel data live in Rust; this module caches the
// last-fetched lists and tracks UI-only state (selection, playback,
// tag drag, panel visibility). Mutations call Rust and then refresh
// the caches via refreshTimeline().

import { createStore } from "solid-js/store";
import type {
  Cel,
  Frame,
  FrameIndex,
  FrameRange,
  FrameTag,
  LayerId,
  LoopDirection,
  SpriteId,
} from "../lib/types";
import {
  celList,
  frameAdd,
  frameDelete,
  frameDuplicate,
  frameList,
  frameReorder,
  frameSetDuration,
  frameSetDurationMul,
  frameTagCreate,
  frameTagDelete,
  frameTagList,
  frameTagRename,
  frameTagSetPlayback,
} from "../lib/commands/frames";
import { canvasRecompositeFrame } from "../lib/commands/canvas";
import { createBackendQuery } from "../lib/sync/query";
import { runMutation } from "../lib/sync/mutation";
import {
  activeSpriteId,
  activeFrameIndex,
  setActiveFrameIndex,
  viewport,
  setOnionSkin,
  setOnionSkinPrev,
  setOnionSkinNext,
  setOnionSkinOpacity,
  scheduleViewportSync,
} from "../canvas/canvas-state";

// ── Data caches ──────────────────────────────────────────────────────────────

// Cel presence as a two-level lookup: Map<LayerId, Set<FrameIndex>>.
// Built from the flat cel list; missing entry means no cel at that (layer, frame).
export type CelPresence = ReadonlyMap<LayerId, ReadonlySet<FrameIndex>>;

interface TimelineData {
  frames: Frame[];
  tags: FrameTag[];
  celPresence: CelPresence;
}

const EMPTY_TIMELINE: TimelineData = { frames: [], tags: [], celPresence: new Map() };

// Frames, tags, and cels load together as one query keyed on the active
// sprite. They used to share a hand-rolled refresh token so a sprite change
// mid-fetch dropped all three stale responses atomically; a single
// createBackendQuery gives that for free (createResource commits only the
// latest fetch) and fetches all three in one Promise.all.
const timelineQuery = createBackendQuery<SpriteId, TimelineData>({
  key: "frames",
  source: activeSpriteId,
  fetch: async (spriteId) => {
    const [nextFrames, nextTags, nextCels]: [Frame[], FrameTag[], Cel[]] = await Promise.all([
      frameList(spriteId),
      frameTagList(spriteId),
      celList(spriteId),
    ]);
    return { frames: nextFrames, tags: nextTags, celPresence: buildCelPresence(nextCels) };
  },
  initial: EMPTY_TIMELINE,
  errorTitle: "Failed to load timeline",
});

export const frames = (): Frame[] => timelineQuery.data().frames;
export const frameTags = (): FrameTag[] => timelineQuery.data().tags;
export const celPresence = (): CelPresence => timelineQuery.data().celPresence;

/** Refetches frames, tags, and cels for the active sprite. */
export function refreshTimeline(): void {
  void timelineQuery.refetch();
}

// ── UI state ───────────────────────────────────────────────────────────────

export type TagDragState =
  | { readonly kind: "none" }
  | { readonly kind: "dragging"; startFrame: FrameIndex; endFrame: FrameIndex };

// UI-only timeline state in one store. Reads are timelineUi.selectedFrames,
// timelineUi.isPlaying, etc.; writes go through the setter functions below
// (setSelectedFrames accepts a value or updater for the Set-mutating sites).
interface TimelineUiState {
  selectedFrames: ReadonlySet<FrameIndex>;
  /** Copy source frame index; paste duplicates it after the active frame. */
  copiedFrameIndex: FrameIndex | null;
  isPlaying: boolean;
  isLooping: boolean;
  tagDragState: TagDragState;
}

const [timelineUi, setTimelineUi] = createStore<TimelineUiState>({
  selectedFrames: new Set(),
  copiedFrameIndex: null,
  isPlaying: false,
  isLooping: true,
  tagDragState: { kind: "none" },
});

export { timelineUi };

type SetArg<T> = T | ((prev: T) => T);

export function setSelectedFrames(next: SetArg<ReadonlySet<FrameIndex>>): void {
  setTimelineUi("selectedFrames", typeof next === "function" ? next : () => next);
}
export const setCopiedFrameIndex = (v: FrameIndex | null): void =>
  setTimelineUi("copiedFrameIndex", v);
export const setIsPlaying = (v: boolean): void => setTimelineUi("isPlaying", v);
export const setIsLooping = (v: boolean): void => setTimelineUi("isLooping", v);
export const setTagDragState = (v: TagDragState): void => setTimelineUi("tagDragState", v);

// ── Frame selection ──────────────────────────────────────────────────────────

export function selectFrame(index: FrameIndex, extend: boolean): void {
  setActiveFrameIndex(index);
  scheduleViewportSync();
  if (extend) {
    setSelectedFrames((prev) => new Set([...prev, index]));
  } else {
    setSelectedFrames(new Set([index]));
  }
}

export function extendSelectionTo(toIndex: FrameIndex): void {
  const from = activeFrameIndex();
  const lo = Math.min(from, toIndex);
  const hi = Math.max(from, toIndex);
  const range = new Set<FrameIndex>();
  for (let i = lo; i <= hi; i++) range.add(i);
  setSelectedFrames(range);
}

// ── Cel clipboard ────────────────────────────────────────────────────────────

export function copyActiveFrame(): void {
  setCopiedFrameIndex(activeFrameIndex());
}

/// Stores `index` as the copy source (used by the right-click menu so
/// "Copy" reflects the clicked frame, not whichever frame was last
/// foregrounded). Mirrors `copyActiveFrame` for the active-frame path.
export function copyFrame(index: FrameIndex): void {
  setCopiedFrameIndex(index);
}

export function pasteFrame(spriteId: SpriteId): void {
  const src = timelineUi.copiedFrameIndex;
  if (src === null) return;
  void runMutation({
    run: () => frameDuplicate(spriteId, src),
    invalidate: ["frames"],
    onSuccess: ({ index }) => {
      selectFrame(index, false);
      // Newly materialised frames have no tiles in the renderer's cache;
      // recomposite so the duplicated cels actually appear instead of a blank.
      recompositeFrameOrLog(spriteId, index);
    },
  });
}

// ── Playback ─────────────────────────────────────────────────────────────────

let playbackTimer: ReturnType<typeof setTimeout> | null = null;

function scheduleNextFrame(): void {
  const all = frames();
  const current = activeFrameIndex();
  const next = current + 1;

  if (next >= all.length) {
    if (timelineUi.isLooping) {
      setActiveFrameIndex(0);
    } else {
      stopPlayback();
      return;
    }
  } else {
    setActiveFrameIndex(next);
  }
  scheduleViewportSync();

  const nextIdx = activeFrameIndex();
  const nextFrame = all[nextIdx];
  const delay = nextFrame !== undefined ? effectiveDurationMs(nextFrame) : 100;
  playbackTimer = setTimeout(scheduleNextFrame, delay);
}

/** On-screen duration in ms: base `duration_ms` scaled by the float
 * `duration_mul` (defaulting to 1.0 for older data), floored at 1. */
function effectiveDurationMs(frame: Frame): number {
  const mul = frame.duration_mul ?? 1.0;
  return Math.max(1, Math.round(frame.duration_ms * mul));
}

export function startPlayback(): void {
  if (timelineUi.isPlaying) return;
  const all = frames();
  if (all.length === 0) return;
  setIsPlaying(true);
  const currentFrame = all[activeFrameIndex()];
  const delay = currentFrame !== undefined ? effectiveDurationMs(currentFrame) : 100;
  playbackTimer = setTimeout(scheduleNextFrame, delay);
}

export function stopPlayback(): void {
  if (playbackTimer !== null) {
    clearTimeout(playbackTimer);
    playbackTimer = null;
  }
  setIsPlaying(false);
}

export function togglePlayback(): void {
  if (timelineUi.isPlaying) stopPlayback();
  else startPlayback();
}

// Stop playback when the panel is hidden or the sprite changes.
export function stopPlaybackIfActive(): void {
  if (timelineUi.isPlaying) stopPlayback();
}

// ── Onion skin re-exports ────────────────────────────────────────────────────
// Canvas-state owns the viewport store (onion fields live on it). Re-exported
// so timeline components import the store + onion setters from one place;
// reads are viewport.onionSkin, viewport.onionSkinPrev, etc.
export { viewport, setOnionSkin, setOnionSkinPrev, setOnionSkinNext, setOnionSkinOpacity };

// ── Mutation helpers ─────────────────────────────────────────────────────────

export function addFrame(spriteId: SpriteId, durationMs = 100): void {
  void runMutation({
    run: () => frameAdd(spriteId, durationMs),
    invalidate: ["frames"],
    onSuccess: ({ index }) => selectFrame(index, false),
  });
}

export function deleteFrames(spriteId: SpriteId, indices: ReadonlySet<FrameIndex>): void {
  // Delete highest-index-first so surviving indices don't shift under us.
  const sorted = [...indices].sort((a, b) => b - a);
  const currentActive = activeFrameIndex();
  const wasDeleted = indices.has(currentActive);

  void runMutation({
    run: () =>
      sorted.reduce<Promise<void>>(
        (chain, idx) => chain.then(() => frameDelete(spriteId, idx)),
        Promise.resolve(),
      ),
    invalidate: ["frames"],
    onSuccess: () => {
      if (wasDeleted) {
        const firstDeleted = sorted[sorted.length - 1] ?? 0;
        setActiveFrameIndex(Math.max(0, firstDeleted - 1));
        scheduleViewportSync();
      } else {
        // Active frame survived the delete, but its index may have
        // shifted left because frames *before* it were removed. Walk
        // the deleted set and subtract one for each index < active.
        let deletedBefore = 0;
        for (const idx of indices) {
          if (idx < currentActive) deletedBefore += 1;
        }
        if (deletedBefore > 0) {
          setActiveFrameIndex(Math.max(0, currentActive - deletedBefore));
          scheduleViewportSync();
        }
      }
      setSelectedFrames(new Set<FrameIndex>());
    },
  });
}

export function duplicateFrame(spriteId: SpriteId, index: FrameIndex): void {
  void runMutation({
    run: () => frameDuplicate(spriteId, index),
    invalidate: ["frames"],
    onSuccess: ({ index: newIdx }) => {
      selectFrame(newIdx, false);
      // Newly materialised frames have no tiles in the renderer's cache;
      // recomposite so the duplicated cels actually appear instead of a blank.
      recompositeFrameOrLog(spriteId, newIdx);
    },
  });
}

// Asks the backend to recomposite `frameIndex` and emit tile-dirty events.
// Used after duplicate/paste, where Rust has new cels but the renderer's
// per-frame tile cache is empty until something pushes pixels at it.
function recompositeFrameOrLog(spriteId: SpriteId, frameIndex: FrameIndex): void {
  canvasRecompositeFrame(spriteId, frameIndex).catch((err: unknown) =>
    console.error("[pixhaus] canvas_recomposite_frame:", err),
  );
}

export function setFrameDuration(spriteId: SpriteId, index: FrameIndex, durationMs: number): void {
  void runMutation({
    run: () => frameSetDuration(spriteId, index, durationMs),
    invalidate: ["frames"],
  });
}

export function setFrameDurationMul(spriteId: SpriteId, index: FrameIndex, mul: number): void {
  void runMutation({
    run: () => frameSetDurationMul(spriteId, index, mul),
    invalidate: ["frames"],
  });
}

// Swap two frames in-place using two sequential frameReorder calls.
//
// frameReorder(A, B) moves A to B and shifts frames in (A,B] left by 1.
// frameReorder(B-1, A) then moves the original B (now at B-1) back to A
// and shifts frames in [A, B-1) right by 1, restoring inner elements.
// Together this is a pure swap: only A and B change positions.
async function swapFrames(spriteId: SpriteId, a: FrameIndex, b: FrameIndex): Promise<void> {
  if (a === b) return;
  const [lo, hi] = a < b ? [a, b] : [b, a];
  await frameReorder(spriteId, lo, hi);
  await frameReorder(spriteId, hi - 1, lo);
}

export function reverseSelectedFrames(spriteId: SpriteId, indices: ReadonlySet<FrameIndex>): void {
  const pairs = buildSwapPairs(indices);
  if (pairs.length === 0) return;

  void runMutation({
    run: () =>
      pairs.reduce<Promise<void>>(
        (p, [a, b]) => p.then(() => swapFrames(spriteId, a, b)),
        Promise.resolve(),
      ),
    invalidate: ["frames"],
  });
}

export function createTag(
  spriteId: SpriteId,
  name: string,
  range: FrameRange,
  loopDirection: LoopDirection = "forward",
  repeat = 0,
): void {
  void runMutation({
    run: () =>
      frameTagCreate({ sprite_id: spriteId, name, range, loop_direction: loopDirection, repeat }),
    invalidate: ["frames"],
  });
}

export function deleteTag(spriteId: SpriteId, tagName: string): void {
  void runMutation({
    run: () => frameTagDelete(spriteId, tagName),
    invalidate: ["frames"],
  });
}

export function renameTag(spriteId: SpriteId, oldName: string, newName: string): void {
  void runMutation({
    run: () => frameTagRename(spriteId, oldName, newName),
    invalidate: ["frames"],
  });
}

/** Sets a tag's loop direction and repeat (generated animations need both). */
export function setTagPlayback(
  spriteId: SpriteId,
  tagName: string,
  loopDirection: LoopDirection,
  repeat: number,
): void {
  void runMutation({
    run: () => frameTagSetPlayback(spriteId, tagName, loopDirection, repeat),
    invalidate: ["frames"],
  });
}

// Pure helper: generates a name from a set of existing names.
// Called by uniqueTagName(); exported for testing.
export function genUniqueTagName(existingNames: ReadonlySet<string>): string {
  let n = 1;
  while (existingNames.has(`Tag ${n}`)) n++;
  return `Tag ${n}`;
}

// Generates a name that does not collide with existing tag names.
export function uniqueTagName(): string {
  return genUniqueTagName(new Set(frameTags().map((t) => t.name)));
}

// Pure helper: build a cel-presence lookup from a flat cel list.
// Exported for testing; used in refreshTimeline().
export function buildCelPresence(cels: readonly Cel[]): Map<LayerId, Set<FrameIndex>> {
  const m = new Map<LayerId, Set<FrameIndex>>();
  for (const cel of cels) {
    let s = m.get(cel.layer_id);
    if (s === undefined) {
      s = new Set<FrameIndex>();
      m.set(cel.layer_id, s);
    }
    s.add(cel.frame_index);
  }
  return m;
}

// Pure helper: compute the sequence of (from, to) swap pairs to reverse
// a sorted list of selected frame positions.
// Exported for testing; used by reverseSelectedFrames().
export function buildSwapPairs(indices: ReadonlySet<FrameIndex>): Array<[FrameIndex, FrameIndex]> {
  const pts = [...indices].sort((a, b) => a - b);
  const pairs: Array<[FrameIndex, FrameIndex]> = [];
  let lo = 0;
  let hi = pts.length - 1;
  while (lo < hi) {
    const a = pts[lo];
    const b = pts[hi];
    if (a !== undefined && b !== undefined) pairs.push([a, b]);
    lo++;
    hi--;
  }
  return pairs;
}
