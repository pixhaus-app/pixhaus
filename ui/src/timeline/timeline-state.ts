// Reactive state for the timeline panel.
//
// Frame, tag, and cel data live in Rust; this module caches the
// last-fetched lists and tracks UI-only state (selection, playback,
// tag drag, panel visibility). Mutations call Rust and then refresh
// the caches via refreshTimeline().

import { createSignal } from "solid-js";
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
  frameTagCreate,
  frameTagDelete,
  frameTagList,
  frameTagRename,
} from "../lib/commands/frames";
import { canvasRecompositeFrame } from "../lib/commands/canvas";
import { reportCommandFailure } from "../lib/utils/errors";
import {
  activeSpriteId,
  activeFrameIndex,
  setActiveFrameIndex,
  onionSkin,
  setOnionSkin,
  onionSkinPrev,
  setOnionSkinPrev,
  onionSkinNext,
  setOnionSkinNext,
  onionSkinOpacity,
  setOnionSkinOpacity,
  scheduleViewportSync,
} from "../canvas/canvas-state";

// ── Panel visibility ─────────────────────────────────────────────────────────

export const [isTimelinePanelVisible, setTimelinePanelVisible] = createSignal(true);

// ── Data caches ──────────────────────────────────────────────────────────────

export const [frames, setFrames] = createSignal<Frame[]>([]);
export const [frameTags, setFrameTags] = createSignal<FrameTag[]>([]);

// Cel presence as a two-level lookup: Map<LayerId, Set<FrameIndex>>.
// Built from the flat cel list; missing entry means no cel at that (layer, frame).
export type CelPresence = ReadonlyMap<LayerId, ReadonlySet<FrameIndex>>;
export const [celPresence, setCelPresence] = createSignal<CelPresence>(new Map());

// All three caches share one refresh token so a sprite change mid-fetch
// drops all three stale responses atomically, not just the first that resolves.
let refreshToken = 0;

export function refreshTimeline(): void {
  refreshToken += 1;
  const myToken = refreshToken;
  const spriteId = activeSpriteId();
  if (spriteId === null) {
    setFrames([]);
    setFrameTags([]);
    setCelPresence(new Map());
    return;
  }
  void Promise.all([frameList(spriteId), frameTagList(spriteId), celList(spriteId)])
    .then(([nextFrames, nextTags, nextCels]: [Frame[], FrameTag[], Cel[]]) => {
      if (myToken !== refreshToken) return;
      setFrames(nextFrames);
      setFrameTags(nextTags);
      setCelPresence(buildCelPresence(nextCels));
    })
    .catch((err: unknown) => {
      console.error("[pixhaus] timeline refresh:", err);
    });
}

// ── Frame selection ──────────────────────────────────────────────────────────

export const [selectedFrames, setSelectedFrames] = createSignal<ReadonlySet<FrameIndex>>(new Set());

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

// Copy stores the source frame index; paste duplicates it after the active frame.
export const [copiedFrameIndex, setCopiedFrameIndex] = createSignal<FrameIndex | null>(null);

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
  const src = copiedFrameIndex();
  if (src === null) return;
  frameDuplicate(spriteId, src)
    .then(({ index }) => {
      refreshTimeline();
      selectFrame(index, false);
      // Newly materialised frames have no tiles in the renderer's cache;
      // recomposite so the duplicated cels actually appear instead of a blank.
      recompositeFrameOrLog(spriteId, index);
    })
    .catch((err: unknown) => reportCommandFailure("frame paste", err));
}

// ── Playback ─────────────────────────────────────────────────────────────────

export const [isPlaying, setIsPlaying] = createSignal(false);
export const [isLooping, setIsLooping] = createSignal(true);

let playbackTimer: ReturnType<typeof setTimeout> | null = null;

function scheduleNextFrame(): void {
  const all = frames();
  const current = activeFrameIndex();
  const next = current + 1;

  if (next >= all.length) {
    if (isLooping()) {
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
  const delay = nextFrame !== undefined ? Math.max(1, nextFrame.duration_ms) : 100;
  playbackTimer = setTimeout(scheduleNextFrame, delay);
}

export function startPlayback(): void {
  if (isPlaying()) return;
  const all = frames();
  if (all.length === 0) return;
  setIsPlaying(true);
  const currentFrame = all[activeFrameIndex()];
  const delay = currentFrame !== undefined ? Math.max(1, currentFrame.duration_ms) : 100;
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
  if (isPlaying()) stopPlayback();
  else startPlayback();
}

// Stop playback when the panel is hidden or the sprite changes.
export function stopPlaybackIfActive(): void {
  if (isPlaying()) stopPlayback();
}

// ── Onion skin re-exports ────────────────────────────────────────────────────
// Canvas-state owns these signals. Re-exported so timeline components
// import from one place rather than two.
export {
  onionSkin,
  setOnionSkin,
  onionSkinPrev,
  setOnionSkinPrev,
  onionSkinNext,
  setOnionSkinNext,
  onionSkinOpacity,
  setOnionSkinOpacity,
};

// ── Tag drag state ───────────────────────────────────────────────────────────

export type TagDragState =
  | { readonly kind: "none" }
  | { readonly kind: "dragging"; startFrame: FrameIndex; endFrame: FrameIndex };

export const [tagDragState, setTagDragState] = createSignal<TagDragState>({ kind: "none" });

// ── Mutation helpers ─────────────────────────────────────────────────────────

export function addFrame(spriteId: SpriteId, durationMs = 100): void {
  frameAdd(spriteId, durationMs)
    .then(({ index }) => {
      refreshTimeline();
      selectFrame(index, false);
    })
    .catch((err: unknown) => reportCommandFailure("frame_add", err));
}

export function deleteFrames(spriteId: SpriteId, indices: ReadonlySet<FrameIndex>): void {
  // Delete highest-index-first so surviving indices don't shift under us.
  const sorted = [...indices].sort((a, b) => b - a);
  const currentActive = activeFrameIndex();
  const wasDeleted = indices.has(currentActive);

  sorted
    .reduce<Promise<void>>(
      (chain, idx) => chain.then(() => frameDelete(spriteId, idx)),
      Promise.resolve(),
    )
    .then(() => {
      refreshTimeline();
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
    })
    .catch((err: unknown) => reportCommandFailure("frame_delete", err));
}

export function duplicateFrame(spriteId: SpriteId, index: FrameIndex): void {
  frameDuplicate(spriteId, index)
    .then(({ index: newIdx }) => {
      refreshTimeline();
      selectFrame(newIdx, false);
      // Newly materialised frames have no tiles in the renderer's cache;
      // recomposite so the duplicated cels actually appear instead of a blank.
      recompositeFrameOrLog(spriteId, newIdx);
    })
    .catch((err: unknown) => reportCommandFailure("frame_duplicate", err));
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
  frameSetDuration(spriteId, index, durationMs)
    .then(() => refreshTimeline())
    .catch((err: unknown) => reportCommandFailure("frame_set_duration", err));
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

  pairs
    .reduce<Promise<void>>(
      (p, [a, b]) => p.then(() => swapFrames(spriteId, a, b)),
      Promise.resolve(),
    )
    .then(() => refreshTimeline())
    .catch((err: unknown) => reportCommandFailure("frame_reverse", err));
}

export function createTag(
  spriteId: SpriteId,
  name: string,
  range: FrameRange,
  loopDirection: LoopDirection = "forward",
  repeat = 0,
): void {
  frameTagCreate({ sprite_id: spriteId, name, range, loop_direction: loopDirection, repeat })
    .then(() => refreshTimeline())
    .catch((err: unknown) => reportCommandFailure("frame_tag_create", err));
}

export function deleteTag(spriteId: SpriteId, tagName: string): void {
  frameTagDelete(spriteId, tagName)
    .then(() => refreshTimeline())
    .catch((err: unknown) => reportCommandFailure("frame_tag_delete", err));
}

export function renameTag(spriteId: SpriteId, oldName: string, newName: string): void {
  frameTagRename(spriteId, oldName, newName)
    .then(() => refreshTimeline())
    .catch((err: unknown) => reportCommandFailure("frame_tag_rename", err));
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
