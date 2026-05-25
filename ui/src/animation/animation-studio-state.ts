// Animation studio state.
//
// One Solid store for the whole studio: open/close, the four-stage pipeline,
// generation controls, streaming request rows, candidates, and review state.
// Reads are studio.stage, studio.candidates, etc.; writes go through the
// setter functions below, which accept either a value or an updater so the
// array-mutating call sites (candidates, requests, first-frame candidates)
// keep their callback form.

import { createStore, unwrap } from "solid-js/store";

import type { LoopDirection } from "../lib/types/LoopDirection";
import type { NormalizeReport } from "../lib/types/NormalizeReport";
import type { EntityId } from "../lib/types/EntityId";
import {
  animationStudioGetState,
  animationStudioSetState,
  type AnchorImage,
  type AnimationJob,
  type FirstFrameImage,
  type RgbaFrame,
} from "../lib/commands/animation";

// ── Types ──────────────────────────────────────────────────────────────────

/**
 * The studio is a four-stage pipeline. Each stage produces a reviewable
 * artifact (reference → first frame → raw video → extracted frames) you advance
 * through explicitly. Reopening an earlier stage invalidates downstream work.
 */
export type Stage = "reference" | "first_frame" | "video" | "extract";

/** Animation type. Sets the prompt scaffold and landing loop direction. */
export type AnimType = "idle" | "walk" | "attack" | "custom";

/** Generate direction. East is a derived flip of West, not a target. */
export type DirectionOpt = "south" | "west" | "north" | "east";

/** Image-to-video model for the video stage. */
export type VideoModel = "seedance" | "wan";

/** One reviewable generated animation candidate. */
export type Candidate = {
  id: number;
  /** Normalized loop frames. */
  frames: RgbaFrame[];
  /** Direction the candidate was generated for. */
  direction: DirectionOpt;
  /** Animation type. */
  animType: AnimType;
  /** Normalization report (drift / scale / seam). */
  report: NormalizeReport | null;
  /** Whether this candidate must be flipped on integrate (east). */
  flip: boolean;
};

/** Status of one in-flight generation request. */
export type RequestStatus = "running" | "done" | "error" | "cancelled";

/** One row in the request strip, mirroring SheetRequestProgress. */
export type RequestRow = {
  id: number;
  label: string;
  status: RequestStatus;
  /** Elapsed seconds, ticked while running. */
  elapsedS: number;
  /** Verb invocation id for cancellation. */
  invocationId: string | null;
  /** True for the slower / pricier i2v path. */
  i2v: boolean;
  /** Error message when status is "error". */
  error: string | null;
};

/** Raw i2v clip awaiting the frame picker. */
export type PendingClip = {
  clipBase64: string;
  mime: string;
  fps: number;
  targetCount: number;
  direction: DirectionOpt;
  animType: AnimType;
};

interface StudioState {
  isAnimationStudioOpen: boolean;
  activeAnimationStudioEntityId: EntityId | null;
  stage: Stage;
  // controls
  animType: AnimType;
  direction: DirectionOpt;
  videoModel: VideoModel;
  /** Target picked-frame count (8-12 typical). */
  frameCount: number;
  fps: number;
  choreography: string;
  seed: number | null;
  /** Output sprite-frame size (the normalize canvas). */
  frameSize: number;
  advancedOpen: boolean;
  // stage 0: reference
  referenceImage: AnchorImage | null;
  // stage 1: first frame
  firstFrameCandidates: FirstFrameImage[];
  /** Candidate currently highlighted in the first-frame strip. In the store
   * (not the component) so the selection survives leaving and returning to the
   * stage. */
  selectedFirstFrame: FirstFrameImage | null;
  approvedFirstFrame: FirstFrameImage | null;
  /** Pose / choreography detail for the first-frame edit prompt. */
  firstFramePrompt: string;
  // candidates
  candidates: Candidate[];
  selectedCandidateId: number | null;
  // streaming request rows
  requests: RequestRow[];
  // transport
  previewPlaying: boolean;
  previewLooping: boolean;
  // walk-clip handoff (i2v)
  pendingClip: PendingClip | null;
  /** Latest i2v job for the active generation, mirrored from the backend's
   * AnimationJobUpdate events. In the store so the in-flight status and elapsed
   * timer survive leaving and returning to the Video stage. */
  videoJob: AnimationJob | null;
}

const INITIAL: StudioState = {
  isAnimationStudioOpen: false,
  activeAnimationStudioEntityId: null,
  stage: "reference",
  animType: "idle",
  direction: "south",
  videoModel: "seedance",
  frameCount: 10,
  fps: 10,
  choreography: "",
  seed: null,
  frameSize: 64,
  advancedOpen: false,
  referenceImage: null,
  firstFrameCandidates: [],
  selectedFirstFrame: null,
  approvedFirstFrame: null,
  firstFramePrompt: "",
  candidates: [],
  selectedCandidateId: null,
  requests: [],
  previewPlaying: true,
  previewLooping: true,
  pendingClip: null,
  videoJob: null,
};

export const [studio, setStudio] = createStore<StudioState>({ ...INITIAL });

// ── Setters ──────────────────────────────────────────────────────────────────

type SetArg<T> = T | ((prev: T) => T);
function setter<K extends keyof StudioState>(key: K) {
  return (next: SetArg<StudioState[K]>): void => {
    // The store's leaf setter accepts both a value and a (prev) => next fn.
    setStudio(key, next as never);
  };
}

export const setAnimationStudioOpen = setter("isAnimationStudioOpen");
export const setActiveAnimationStudioEntityId = setter("activeAnimationStudioEntityId");
export const setStage = setter("stage");
export const setAnimType = setter("animType");
export const setDirection = setter("direction");
export const setVideoModel = setter("videoModel");
export const setFrameCount = setter("frameCount");
export const setFps = setter("fps");
export const setChoreography = setter("choreography");
export const setSeed = setter("seed");
export const setFrameSize = setter("frameSize");
export const setAdvancedOpen = setter("advancedOpen");
export const setReferenceImage = setter("referenceImage");
export const setFirstFrameCandidates = setter("firstFrameCandidates");
export const setSelectedFirstFrame = setter("selectedFirstFrame");
export const setApprovedFirstFrame = setter("approvedFirstFrame");
export const setFirstFramePrompt = setter("firstFramePrompt");
export const setCandidates = setter("candidates");
export const setSelectedCandidateId = setter("selectedCandidateId");
export const setRequests = setter("requests");
export const setPreviewPlaying = setter("previewPlaying");
export const setPreviewLooping = setter("previewLooping");
export const setPendingClip = setter("pendingClip");
export const setVideoJob = setter("videoJob");

// ── open / close ─────────────────────────────────────────────────────────────

/**
 * Opens the studio for `entityId` (its sprite). The working state persists
 * across close/reopen of the *same* sprite so the user resumes where they left
 * off (stage, reference, first-frame candidates, approved frame, video job,
 * etc.). Switching to a *different* sprite starts fresh.
 */
export function openAnimationStudio(entityId: EntityId): void {
  if (entityId !== studio.activeAnimationStudioEntityId) {
    // Persist the outgoing entity's working state before we reset. flushStudioState
    // reads the active entity id and serializes the snapshot synchronously (before
    // its await), so it captures the old entity's state even though we switch next.
    void flushStudioState();
    setActiveAnimationStudioEntityId(entityId);
    resetStudio();
    // Different sprite: start from its persisted studio state (if the project
    // has one saved), loaded async so the UI opens immediately.
    void hydrateStudioFor(entityId);
  }
  setAnimationStudioOpen(true);
}

/**
 * Closes the studio. Working state is intentionally kept (not reset) so
 * reopening the same sprite resumes the pipeline; it's reset only when a
 * different sprite is opened (see openAnimationStudio). Flushes the state into
 * the project first so a later save persists it.
 */
export function closeAnimationStudio(): void {
  void flushStudioState();
  setAnimationStudioOpen(false);
}

// ── Persistence to the project file ──────────────────────────────────────────
//
// The studio store is per-entity working state. It's mirrored into the project
// (AiMetadata.animation_studio_state, a JSON blob) so it survives save/reload
// and travels with the .pixhaus file. We flush on close and before save rather
// than syncing continuously, because the blob includes the multi-MB raw clip.

/** Fields excluded from the persisted snapshot: window identity/visibility and
 * the in-flight request rows (meaningless after a reload). */
type PersistedStudio = Omit<
  StudioState,
  "isAnimationStudioOpen" | "activeAnimationStudioEntityId" | "requests"
>;

function serializeStudio(): string {
  // Shallow-copy the unwrapped store, then drop the fields we don't persist.
  const snap: Partial<StudioState> = { ...unwrap(studio) };
  delete snap.isAnimationStudioOpen;
  delete snap.activeAnimationStudioEntityId;
  delete snap.requests;
  return JSON.stringify(snap as PersistedStudio);
}

/**
 * Writes the current studio snapshot into the project for the active entity so
 * a subsequent save persists it. Awaitable so callers (the save path) can
 * ensure the project holds the latest before serializing; swallows its own
 * errors so a flush failure never blocks the save.
 */
export async function flushStudioState(): Promise<void> {
  const id = studio.activeAnimationStudioEntityId;
  if (id === null) return;
  try {
    await animationStudioSetState(id, serializeStudio());
  } catch (err: unknown) {
    console.error("[pixhaus] animation_studio_set_state:", err);
  }
}

/** Loads a sprite's persisted studio state and applies it, unless the user has
 * already switched to a different sprite while the fetch was in flight. */
async function hydrateStudioFor(entityId: EntityId): Promise<void> {
  try {
    const json = await animationStudioGetState(entityId);
    if (json === null) return;
    if (studio.activeAnimationStudioEntityId !== entityId) return;
    const parsed = JSON.parse(json) as PersistedStudio;
    // Reset in-flight rows; everything else restores from the snapshot.
    setStudio({ ...parsed, requests: [] });
  } catch (err: unknown) {
    console.error("[pixhaus] animation_studio_get_state:", err);
  }
}

// ── helpers ──────────────────────────────────────────────────────────────────

/**
 * Sets the approved first frame and invalidates downstream artifacts — the
 * existing video and any candidates derive from the previous frame.
 */
export function approveFirstFrame(frame: FirstFrameImage | null): void {
  setApprovedFirstFrame(frame);
  setPendingClip(null);
  setVideoJob(null);
  setCandidates([]);
  setSelectedCandidateId(null);
}

/** Maps the chosen animation type to its landing loop direction. */
export function loopDirectionFor(t: AnimType): LoopDirection {
  switch (t) {
    case "idle":
      return "ping_pong";
    case "walk":
    case "attack":
    case "custom":
      return "forward";
  }
}

/** Returns the currently selected candidate, if any. */
export function selectedCandidate(): Candidate | null {
  const id = studio.selectedCandidateId;
  return studio.candidates.find((c) => c.id === id) ?? null;
}

let nextId = 1;
/** Mints a process-unique id for candidates and request rows. */
export function nextStudioId(): number {
  nextId += 1;
  return nextId;
}

/**
 * Clears all working state back to defaults — artifacts, candidates, picker,
 * and the generation controls (animType, direction, frame count, etc.) — so a
 * different sprite starts fresh. Only window identity (the active entity id and
 * open flag) is preserved; hydration then overlays any persisted state.
 */
export function resetStudio(): void {
  setStudio({
    ...INITIAL,
    activeAnimationStudioEntityId: studio.activeAnimationStudioEntityId,
    isAnimationStudioOpen: studio.isAnimationStudioOpen,
  });
}
