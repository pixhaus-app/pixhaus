// Frame timeline commands: add, delete, duplicate, reorder, tag CRUD.

import { invoke } from "@tauri-apps/api/core";
import type {
  Cel,
  Frame,
  FrameIndex,
  FrameRange,
  FrameTag,
  LoopDirection,
  SpriteId,
} from "../types";

// ── response types ────────────────────────────────────────────────────────────

export type FrameAddResult = {
  frame: Frame;
  index: FrameIndex;
};

// ── argument types ────────────────────────────────────────────────────────────

export type FrameTagCreateArgs = {
  sprite_id: SpriteId;
  name: string;
  range: FrameRange;
  loop_direction: LoopDirection;
  repeat: number;
};

// ── commands ──────────────────────────────────────────────────────────────────

/** Appends a new frame at the end of the timeline. */
export function frameAdd(sprite_id: SpriteId, duration_ms: number): Promise<FrameAddResult> {
  return invoke<FrameAddResult>("frame_add", { sprite_id, duration_ms });
}

/** Deletes a frame from the timeline. Also removes all cels on that frame. */
export function frameDelete(sprite_id: SpriteId, frame_index: FrameIndex): Promise<void> {
  return invoke<void>("frame_delete", { sprite_id, frame_index });
}

/** Duplicates a frame, inserting the copy immediately after the source. */
export function frameDuplicate(
  sprite_id: SpriteId,
  frame_index: FrameIndex,
): Promise<FrameAddResult> {
  return invoke<FrameAddResult>("frame_duplicate", { sprite_id, frame_index });
}

/** Moves a frame from one position to another in the timeline. */
export function frameReorder(
  sprite_id: SpriteId,
  from_index: FrameIndex,
  to_index: FrameIndex,
): Promise<void> {
  return invoke<void>("frame_reorder", { sprite_id, from_index, to_index });
}

/** Updates the display duration for a single frame. */
export function frameSetDuration(
  sprite_id: SpriteId,
  frame_index: FrameIndex,
  duration_ms: number,
): Promise<void> {
  return invoke<void>("frame_set_duration", { sprite_id, frame_index, duration_ms });
}

/** Creates a named frame tag on a sprite. Tags with duplicate names are rejected. */
export function frameTagCreate(args: FrameTagCreateArgs): Promise<FrameTag> {
  return invoke<FrameTag>("frame_tag_create", { args });
}

/** Deletes a named frame tag from a sprite. */
export function frameTagDelete(sprite_id: SpriteId, tag_name: string): Promise<void> {
  return invoke<void>("frame_tag_delete", { sprite_id, tag_name });
}

/** Returns all frames in a sprite's timeline. */
export function frameList(sprite_id: SpriteId): Promise<Frame[]> {
  return invoke<Frame[]>("frame_list", { sprite_id });
}

/** Returns all cels for a sprite (sparse flat list keyed by layer+frame). */
export function celList(sprite_id: SpriteId): Promise<Cel[]> {
  return invoke<Cel[]>("cel_list", { sprite_id });
}

/** Returns all frame tags for a sprite. */
export function frameTagList(sprite_id: SpriteId): Promise<FrameTag[]> {
  return invoke<FrameTag[]>("frame_tag_list", { sprite_id });
}
