// Animation studio state.
//
// One Solid store for the whole studio: open/close, the four-stage pipeline,
// generation controls, streaming request rows, candidates, and review state.
// Reads are studio.stage, studio.candidates, etc.; writes go through the
// setter functions below, which accept either a value or an updater so the
// array-mutating call sites (candidates, requests, first-frame candidates)
// keep their callback form.

import { createStore } from "solid-js/store";

import type { LoopDirection } from "../lib/types/LoopDirection";
import type { NormalizeReport } from "../lib/types/NormalizeReport";
import type { EntityId } from "../lib/types/EntityId";
import type {
  AnchorImage,
  AnimationJob,
  FirstFrameImage,
  RgbaFrame,
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

/** Opens the studio for `entityId` (its sprite). */
export function openAnimationStudio(entityId: EntityId): void {
  setActiveAnimationStudioEntityId(entityId);
  resetStudio();
  setAnimationStudioOpen(true);
}

/** Closes the studio and clears working state. */
export function closeAnimationStudio(): void {
  setAnimationStudioOpen(false);
  setActiveAnimationStudioEntityId(null);
  resetStudio();
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

/** Clears all working state (stage, artifacts, candidates, requests, picker). */
export function resetStudio(): void {
  setStudio({
    stage: "reference",
    referenceImage: null,
    firstFrameCandidates: [],
    approvedFirstFrame: null,
    firstFramePrompt: "",
    candidates: [],
    selectedCandidateId: null,
    requests: [],
    pendingClip: null,
    videoJob: null,
    previewPlaying: true,
    previewLooping: true,
  });
}
