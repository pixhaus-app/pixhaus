// Animation studio IPC commands.
//
// The Rust handler signatures live in app/src/commands/animation.rs. Keep
// these types in sync — drift surfaces only at the IPC boundary as a serde
// error. Hand-rolled until tauri-specta generates bindings, matching the
// existing TODO in verbs.ts.

import type { LoopDirection } from "../types/LoopDirection";
import type { NormalizeReport } from "../types/NormalizeReport";
import type { SpriteId } from "../types/SpriteId";
import { invoke } from "../ipc";

// ── shared ─────────────────────────────────────────────────────────────────────

/** A tightly-packed RGBA8 frame crossing the IPC boundary. */
export type RgbaFrame = {
  width: number;
  height: number;
  /** Tightly-packed RGBA8 bytes: `pixels.length === width * height * 4`. */
  pixels: number[];
};

// ── normalize ──────────────────────────────────────────────────────────────────

export type NormalizeArgs = {
  frames: RgbaFrame[];
  canvas_size?: number | null;
  /** "magenta" | "green" | "none". */
  chroma?: string | null;
  reference_height?: number | null;
};

export type NormalizeResultDto = {
  frames: RgbaFrame[];
  report: NormalizeReport;
};

/** Runs the normalization pass over candidate frames. */
export function animationNormalize(args: NormalizeArgs): Promise<NormalizeResultDto> {
  return invoke<NormalizeResultDto>("animation_normalize", { args });
}

// ── frame pick (i2v) ─────────────────────────────────────────────────────────────

export type PickFramesArgs = {
  clip_base64: string;
  mime: string;
  fps?: number | null;
  target_count?: number | null;
  /** Explicit `[start, end]` markers; omit to auto-detect. */
  markers?: [number, number] | null;
};

export type PickFramesResult = {
  frames: RgbaFrame[];
  markers: [number, number];
  total_frames: number;
  seam_similarity: number;
};

/** Decodes an i2v clip and picks an even loop. */
export function animationPickFrames(args: PickFramesArgs): Promise<PickFramesResult> {
  return invoke<PickFramesResult>("animation_pick_frames", { args });
}

// ── integrate ──────────────────────────────────────────────────────────────────

export type IntegrateArgs = {
  sprite_id: SpriteId;
  name: string;
  frames: RgbaFrame[];
  loop_direction: LoopDirection;
  repeat?: number;
  frame_duration_ms: number;
  /** Mirror each frame horizontally — east-as-flip-of-west. */
  flip_horizontal?: boolean;
  /** Entity whose CharacterAnchor records this as a derived sheet. */
  entity_id?: number | null;
  /** Animation kind for the cascade record ("idle" | "walk" | "attack"). */
  animation_kind?: string | null;
  /** Direction for the cascade record ("south" | "west" | "north" | "east"). */
  direction?: string | null;
};

export type IntegrateResult = {
  layer_id: number;
  start_frame: number;
  end_frame: number;
};

/** Lands frames as a layer + cels + frames + tag + animation, one undo step. */
export function animationIntegrate(args: IntegrateArgs): Promise<IntegrateResult> {
  return invoke<IntegrateResult>("animation_integrate", { args });
}

// ── animation set (directional cascade) ─────────────────────────────────────────

export type CellStatus = "missing" | "present" | "stale";

export type AnimationSetCell = {
  animation_type: string;
  direction: string;
  status: CellStatus;
};

export type AnimationSet = {
  anchor_approved: boolean;
  cells: AnimationSetCell[];
  has_stale: boolean;
};

/** Queries the per-direction / per-type coverage and staleness for an entity. */
export function animationSet(entityId: number): Promise<AnimationSet> {
  return invoke<AnimationSet>("animation_set", { entity_id: entityId });
}

/** Returns the stale (kind, direction) cells the studio should regenerate. */
export function animationRerollDependents(entityId: number): Promise<AnimationSetCell[]> {
  return invoke<AnimationSetCell[]>("animation_reroll_dependents", { entity_id: entityId });
}

/** Derives the effect-stripped neutral anchor for an entity (Variant verb). */
export function animationDeriveNeutral(entityId: number): Promise<void> {
  return invoke<void>("animation_derive_neutral", { entity_id: entityId });
}

// ── reference anchor (stage 0) ───────────────────────────────────────────────

export type AnchorImage = {
  png_base64: string;
  width: number;
  height: number;
};

/** Returns the entity's resolved directional reference anchor as a PNG. */
export function animationGetAnchor(entityId: number, direction: string): Promise<AnchorImage> {
  return invoke<AnchorImage>("animation_get_anchor", { entity_id: entityId, direction });
}

// ── first frame (stage 1, image edit) ────────────────────────────────────────

export type GenerateFirstFrameArgs = {
  entity_id: number;
  direction: string;
  /** "idle" | "walk" | "attack" | "custom". */
  animation_kind?: string | null;
  /** User pose / choreography detail appended to the scaffold. */
  prompt?: string | null;
  /** Base image to edit (base64 PNG); defaults to the anchor. Set for inpaint. */
  base_image_base64?: string | null;
  /** Inpaint mask (base64 PNG): white = edit, rest kept. */
  mask_base64?: string | null;
  num_images?: number | null;
};

export type FirstFrameImage = {
  png_base64: string;
  width: number;
  height: number;
};

export type GenerateFirstFrameResult = {
  images: FirstFrameImage[];
};

/** Edits the reference sheet into a single isolated pose on magenta (gpt-image-2). */
export function animationGenerateFirstFrame(
  args: GenerateFirstFrameArgs,
): Promise<GenerateFirstFrameResult> {
  return invoke<GenerateFirstFrameResult>("animation_generate_first_frame", { args });
}

// ── clip (image-to-video) ────────────────────────────────────────────────────

export type GenerateClipArgs = {
  /** Entity the clip belongs to (tracks/lists its jobs). */
  entity_id: number;
  /** The approved first frame the motion derives from (base64 PNG). */
  first_frame_base64: string;
  direction: string;
  /** "idle" | "walk" | "attack" | "custom" (null → generic loop). */
  animation_kind?: string | null;
  /** Free-text motion description (used for "custom", appended otherwise). */
  choreography?: string | null;
  /** Video model: "seedance" (default) or "wan". */
  model?: string | null;
  num_frames?: number | null;
  fps?: number | null;
  seed?: number | null;
};

export type GenerateClipResult = {
  clip_base64: string;
  mime: string;
  fps: number;
  target_count: number;
};

/** Lifecycle of an i2v job (mirrors the Rust `AnimationJobStatus`). */
export type AnimationJobStatus = "running" | "done" | "error" | "cancelled" | "interrupted";

/** One i2v job record (payload of the `AnimationJobUpdate` event). */
export type AnimationJob = {
  id: number;
  entity_id: number;
  direction: string;
  animation_kind?: string | null;
  model: string;
  status: AnimationJobStatus;
  created_at_ms: number;
  target_count: number;
  fps: number;
  mime: string;
  error?: string | null;
};

/** Starts an i2v generation as a durable background job; returns the job id. */
export function animationGenerateClip(args: GenerateClipArgs): Promise<number> {
  return invoke<number>("animation_generate_clip", { args });
}

/** Lists the entity's i2v jobs, newest first. */
export function animationJobsList(entityId: number): Promise<AnimationJob[]> {
  return invoke<AnimationJob[]>("animation_jobs_list", { entity_id: entityId });
}

/** Returns one job's current state (authoritative source for status polling). */
export function animationJobGet(jobId: number): Promise<AnimationJob | null> {
  return invoke<AnimationJob | null>("animation_job_get", { job_id: jobId });
}

/** Fetches a finished job's clip (base64) for the frame picker. */
export function animationJobClip(jobId: number): Promise<GenerateClipResult> {
  return invoke<GenerateClipResult>("animation_job_clip", { job_id: jobId });
}

/** Cancels an in-flight i2v job. */
export function animationCancelClipJob(jobId: number): Promise<void> {
  return invoke<void>("animation_cancel_clip_job", { job_id: jobId });
}
