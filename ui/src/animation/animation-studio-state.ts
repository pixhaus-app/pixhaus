// Animation studio state.
//
// Mirrors the open/close shape of `sheet/sheet-state.ts` and adds the
// generation controls, streaming request rows, candidates, and review state
// the studio needs that a single still does not. Module-level signals so the
// studio's components share state without prop-drilling, matching the
// reference-sheet editor's pattern.

import { createSignal } from "solid-js";

import type { LoopDirection } from "../lib/types/LoopDirection";
import type { NormalizeReport } from "../lib/types/NormalizeReport";
import type { EntityId } from "../lib/types/EntityId";
import type {
  AnchorImage,
  AnimationJob,
  FirstFrameImage,
  RgbaFrame,
} from "../lib/commands/animation";

// ── open / close ─────────────────────────────────────────────────────────────

export const [isAnimationStudioOpen, setAnimationStudioOpen] = createSignal(false);

export const [activeAnimationStudioEntityId, setActiveAnimationStudioEntityId] =
  createSignal<EntityId | null>(null);

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

// ── stage machine ──────────────────────────────────────────────────────────────

/**
 * The studio is a four-stage pipeline. Each stage produces a reviewable
 * artifact (reference → first frame → raw video → extracted frames) you advance
 * through explicitly. Reopening an earlier stage invalidates downstream work.
 */
export type Stage = "reference" | "first_frame" | "video" | "extract";

export const [stage, setStage] = createSignal<Stage>("reference");

// ── controls ───────────────────────────────────────────────────────────────────

/** Animation type. Sets the prompt scaffold and landing loop direction. */
export type AnimType = "idle" | "walk" | "attack" | "custom";

/** Generate direction. East is a derived flip of West, not a target. */
export type DirectionOpt = "south" | "west" | "north" | "east";

/** Image-to-video model for the video stage. */
export type VideoModel = "seedance" | "wan";

export const [animType, setAnimType] = createSignal<AnimType>("idle");
export const [direction, setDirection] = createSignal<DirectionOpt>("south");
export const [videoModel, setVideoModel] = createSignal<VideoModel>("seedance");
/** Target picked-frame count (8-12 typical). */
export const [frameCount, setFrameCount] = createSignal(10);
export const [fps, setFps] = createSignal(10);
export const [choreography, setChoreography] = createSignal("");
export const [seed, setSeed] = createSignal<number | null>(null);
/** Output sprite-frame size (the normalize canvas). */
export const [frameSize, setFrameSize] = createSignal(64);
export const [advancedOpen, setAdvancedOpen] = createSignal(false);

// ── stage 0: reference ───────────────────────────────────────────────────────

export const [referenceImage, setReferenceImage] = createSignal<AnchorImage | null>(null);

// ── stage 1: first frame ───────────────────────────────────────────────────────

export const [firstFrameCandidates, setFirstFrameCandidates] = createSignal<FirstFrameImage[]>([]);
export const [approvedFirstFrame, setApprovedFirstFrame] = createSignal<FirstFrameImage | null>(
  null,
);
/** Pose / choreography detail for the first-frame edit prompt. */
export const [firstFramePrompt, setFirstFramePrompt] = createSignal("");

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

// ── candidates ───────────────────────────────────────────────────────────────

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

export const [candidates, setCandidates] = createSignal<Candidate[]>([]);
export const [selectedCandidateId, setSelectedCandidateId] = createSignal<number | null>(null);

/** Returns the currently selected candidate, if any. */
export function selectedCandidate(): Candidate | null {
  const id = selectedCandidateId();
  return candidates().find((c) => c.id === id) ?? null;
}

// ── streaming request rows ──────────────────────────────────────────────────────

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

export const [requests, setRequests] = createSignal<RequestRow[]>([]);

// ── transport ────────────────────────────────────────────────────────────────

export const [previewPlaying, setPreviewPlaying] = createSignal(true);
export const [previewLooping, setPreviewLooping] = createSignal(true);

// ── walk-clip handoff (i2v) ─────────────────────────────────────────────────────

/** Raw i2v clip awaiting the frame picker. */
export type PendingClip = {
  clipBase64: string;
  mime: string;
  fps: number;
  targetCount: number;
  direction: DirectionOpt;
  animType: AnimType;
};

export const [pendingClip, setPendingClip] = createSignal<PendingClip | null>(null);

/**
 * The latest i2v job for the active generation, mirrored from the backend's
 * `AnimationJobUpdate` events. Module-level so the in-flight status and elapsed
 * timer survive leaving and returning to the Video stage.
 */
export const [videoJob, setVideoJob] = createSignal<AnimationJob | null>(null);

// ── reset ────────────────────────────────────────────────────────────────────

let nextId = 1;
/** Mints a process-unique id for candidates and request rows. */
export function nextStudioId(): number {
  nextId += 1;
  return nextId;
}

/** Clears all working state (stage, artifacts, candidates, requests, picker). */
export function resetStudio(): void {
  setStage("reference");
  setReferenceImage(null);
  setFirstFrameCandidates([]);
  setApprovedFirstFrame(null);
  setFirstFramePrompt("");
  setCandidates([]);
  setSelectedCandidateId(null);
  setRequests([]);
  setPendingClip(null);
  setVideoJob(null);
  setPreviewPlaying(true);
}
