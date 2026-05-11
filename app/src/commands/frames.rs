//! Frame timeline commands: add, delete, duplicate, reorder, tag CRUD.

use pixhaus_core::project::{
    Cel, Frame, FrameIndex, FrameRange, FrameTag, LoopDirection, Sprite, SpriteId, UserData,
};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::error::{AppCommandError, CommandResult};
use crate::state::AppState;

/// Shifts every frame-index-bearing field in `sprite` whose index is
/// `>= from` by `delta`.
///
/// Used by `frame_delete` (delta = -1, after dropping cels at the
/// deleted index) and `frame_duplicate` (delta = +1, before inserting
/// the cloned cels). Walks `cels`, `frame_tags`, `animations`, and
/// `slices.keys` in lockstep so cels stay aligned with their frames
/// across timeline edits.
fn shift_frame_indices(sprite: &mut Sprite, from: u32, delta: i32) {
    let apply = |idx: &mut FrameIndex| {
        let n = idx.get();
        if n >= from {
            let new_n = if delta < 0 {
                n.saturating_sub(1)
            } else {
                n.saturating_add(1)
            };
            *idx = FrameIndex::new(new_n);
        }
    };

    for cel in &mut sprite.cels {
        apply(&mut cel.frame_index);
    }
    for tag in &mut sprite.frame_tags {
        let mut s = tag.range.start;
        let mut e = tag.range.end;
        apply(&mut s);
        apply(&mut e);
        tag.range = FrameRange::new(s, e);
    }
    for anim in &mut sprite.animations {
        let mut s = anim.range.start;
        let mut e = anim.range.end;
        apply(&mut s);
        apply(&mut e);
        anim.range = FrameRange::new(s, e);
    }
    for slice in &mut sprite.slices {
        for key in &mut slice.keys {
            apply(&mut key.frame);
        }
    }
}

/// Remaps a single frame index across a `frame_reorder(from, to)`
/// permutation.
///
/// - The frame at `from` lands at `to`.
/// - Frames between `from` and `to` shift by one, opposite to the move
///   direction.
/// - Frames outside the moved span are untouched.
fn remap_for_reorder(idx: FrameIndex, from: u32, to: u32) -> FrameIndex {
    let n = idx.get();
    if n == from {
        FrameIndex::new(to)
    } else if from < to && n > from && n <= to {
        FrameIndex::new(n - 1)
    } else if from > to && n >= to && n < from {
        FrameIndex::new(n + 1)
    } else {
        idx
    }
}

/// Result returned when a frame is added or duplicated.
#[derive(Debug, Serialize)]
pub struct FrameAddResult {
    /// The frame that was added.
    pub frame: Frame,
    /// Position of the new frame in the timeline.
    pub index: FrameIndex,
}

/// Arguments for creating a frame tag.
#[derive(Debug, Deserialize)]
pub struct FrameTagCreateArgs {
    /// Target sprite.
    pub sprite_id: SpriteId,
    /// Display name for the tag.
    pub name: String,
    /// Inclusive range of frames covered by the tag.
    pub range: FrameRange,
    /// Playback direction for the tag.
    pub loop_direction: LoopDirection,
    /// Number of times the range repeats. `0` means loop forever.
    pub repeat: u16,
}

/// Appends a new frame at the end of the timeline.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn frame_add(
    sprite_id: SpriteId,
    duration_ms: u32,
    state: State<'_, AppState>,
) -> CommandResult<FrameAddResult> {
    let mut doc = state.doc.write().await;
    let result = {
        let sprite = doc
            .project
            .as_mut()
            .ok_or(AppCommandError::NoActiveProject)?
            .sprite_mut(sprite_id)
            .ok_or(AppCommandError::NotFound {
                entity: "sprite".into(),
                id: u64::from(sprite_id.get()),
            })?;
        let frame = Frame {
            duration_ms,
            user_data: UserData::default(),
        };
        let index = FrameIndex::new(u32::try_from(sprite.frames.len()).map_err(|_| {
            AppCommandError::Validation {
                detail: "sprite has too many frames".into(),
            }
        })?);
        sprite.frames.push(frame.clone());
        FrameAddResult { frame, index }
    };
    doc.dirty = true;
    Ok(result)
}

/// Deletes a frame from the timeline by index. Also removes all cels on that frame.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn frame_delete(
    sprite_id: SpriteId,
    frame_index: FrameIndex,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let mut doc = state.doc.write().await;
    {
        let sprite = doc
            .project
            .as_mut()
            .ok_or(AppCommandError::NoActiveProject)?
            .sprite_mut(sprite_id)
            .ok_or(AppCommandError::NotFound {
                entity: "sprite".into(),
                id: u64::from(sprite_id.get()),
            })?;
        let idx = frame_index.get() as usize;
        if idx >= sprite.frames.len() {
            return Err(AppCommandError::OutOfRange {
                detail: format!(
                    "frame index {} out of range (sprite has {} frames)",
                    idx,
                    sprite.frames.len()
                ),
            });
        }
        sprite.frames.remove(idx);
        sprite.cels.retain(|c| c.frame_index != frame_index);
        // Shift every surviving cel + range past the deleted frame
        // back by one so cels stay aligned with their frames.
        let from = u32::try_from(idx + 1).map_err(|_| AppCommandError::Validation {
            detail: "frame index exceeds u32::MAX".into(),
        })?;
        shift_frame_indices(sprite, from, -1);
    }
    doc.dirty = true;
    Ok(())
}

/// Duplicates a frame, inserting the copy immediately after the source.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn frame_duplicate(
    sprite_id: SpriteId,
    frame_index: FrameIndex,
    state: State<'_, AppState>,
) -> CommandResult<FrameAddResult> {
    let mut doc = state.doc.write().await;
    let result = {
        let sprite = doc
            .project
            .as_mut()
            .ok_or(AppCommandError::NoActiveProject)?
            .sprite_mut(sprite_id)
            .ok_or(AppCommandError::NotFound {
                entity: "sprite".into(),
                id: u64::from(sprite_id.get()),
            })?;
        let idx = frame_index.get() as usize;
        if idx >= sprite.frames.len() {
            return Err(AppCommandError::OutOfRange {
                detail: format!(
                    "frame index {} out of range (sprite has {} frames)",
                    idx,
                    sprite.frames.len()
                ),
            });
        }
        let frame = sprite.frames[idx].clone();
        let insert_at = idx + 1;
        let insert_idx_u32 = u32::try_from(insert_at).map_err(|_| AppCommandError::Validation {
            detail: "frame index exceeds u32::MAX".into(),
        })?;

        // Clone the source frame's cels onto the new frame index BEFORE
        // shifting; shifting changes the source cels' frame_index.
        let cloned_cels: Vec<_> = sprite
            .cels
            .iter()
            .filter(|c| c.frame_index == frame_index)
            .map(|c| {
                let mut new_cel = c.clone();
                new_cel.frame_index = FrameIndex::new(insert_idx_u32);
                new_cel
            })
            .collect();

        // Shift everything past the source frame by +1 to make room.
        shift_frame_indices(sprite, insert_idx_u32, 1);

        sprite.frames.insert(insert_at, frame.clone());
        sprite.cels.extend(cloned_cels);
        FrameAddResult {
            frame,
            index: FrameIndex::new(insert_idx_u32),
        }
    };
    doc.dirty = true;
    Ok(result)
}

/// Moves a frame from one position to another in the timeline.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn frame_reorder(
    sprite_id: SpriteId,
    from_index: FrameIndex,
    to_index: FrameIndex,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let mut doc = state.doc.write().await;
    {
        let sprite = doc
            .project
            .as_mut()
            .ok_or(AppCommandError::NoActiveProject)?
            .sprite_mut(sprite_id)
            .ok_or(AppCommandError::NotFound {
                entity: "sprite".into(),
                id: u64::from(sprite_id.get()),
            })?;
        let from = from_index.get() as usize;
        let len = sprite.frames.len();
        if from >= len {
            return Err(AppCommandError::OutOfRange {
                detail: format!("from_index {from} out of range (sprite has {len} frames)"),
            });
        }
        let to = (to_index.get() as usize).min(len.saturating_sub(1));
        let frame = sprite.frames.remove(from);
        // Insert at `to` directly — Vec::insert places the element AT
        // that index, shifting later items right. After remove, the
        // post-remove indices line up so the inserted frame ends up
        // exactly at `to` in the final timeline.
        sprite.frames.insert(to, frame);

        // Remap every frame-index-bearing field across the permutation
        // so cels and ranges track their frames through the move.
        let from_u32 = from_index.get();
        let to_u32 = u32::try_from(to).map_err(|_| AppCommandError::Validation {
            detail: "frame index exceeds u32::MAX".into(),
        })?;
        for cel in &mut sprite.cels {
            cel.frame_index = remap_for_reorder(cel.frame_index, from_u32, to_u32);
        }
        for tag in &mut sprite.frame_tags {
            let s = remap_for_reorder(tag.range.start, from_u32, to_u32);
            let e = remap_for_reorder(tag.range.end, from_u32, to_u32);
            // Normalise so start <= end if the permutation flipped them.
            tag.range = if s.get() <= e.get() {
                FrameRange::new(s, e)
            } else {
                FrameRange::new(e, s)
            };
        }
        for anim in &mut sprite.animations {
            let s = remap_for_reorder(anim.range.start, from_u32, to_u32);
            let e = remap_for_reorder(anim.range.end, from_u32, to_u32);
            anim.range = if s.get() <= e.get() {
                FrameRange::new(s, e)
            } else {
                FrameRange::new(e, s)
            };
        }
        for slice in &mut sprite.slices {
            for key in &mut slice.keys {
                key.frame = remap_for_reorder(key.frame, from_u32, to_u32);
            }
        }
    }
    doc.dirty = true;
    Ok(())
}

/// Updates the display duration for a single frame.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn frame_set_duration(
    sprite_id: SpriteId,
    frame_index: FrameIndex,
    duration_ms: u32,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let mut doc = state.doc.write().await;
    {
        let sprite = doc
            .project
            .as_mut()
            .ok_or(AppCommandError::NoActiveProject)?
            .sprite_mut(sprite_id)
            .ok_or(AppCommandError::NotFound {
                entity: "sprite".into(),
                id: u64::from(sprite_id.get()),
            })?;
        let idx = frame_index.get() as usize;
        let frame = sprite
            .frames
            .get_mut(idx)
            .ok_or_else(|| AppCommandError::OutOfRange {
                detail: format!("frame index {idx} out of range"),
            })?;
        frame.duration_ms = duration_ms;
    }
    doc.dirty = true;
    Ok(())
}

/// Creates a named frame tag on a sprite.
///
/// Tags with duplicate names are rejected.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn frame_tag_create(
    args: FrameTagCreateArgs,
    state: State<'_, AppState>,
) -> CommandResult<FrameTag> {
    let mut doc = state.doc.write().await;
    let tag = {
        let sprite = doc
            .project
            .as_mut()
            .ok_or(AppCommandError::NoActiveProject)?
            .sprite_mut(args.sprite_id)
            .ok_or(AppCommandError::NotFound {
                entity: "sprite".into(),
                id: u64::from(args.sprite_id.get()),
            })?;
        if sprite.frame_tags.iter().any(|t| t.name == args.name) {
            return Err(AppCommandError::Conflict {
                detail: format!("frame tag {:?} already exists", args.name),
            });
        }
        let tag = FrameTag {
            name: args.name,
            range: args.range,
            loop_direction: args.loop_direction,
            repeat: args.repeat,
            user_data: UserData::default(),
        };
        sprite.frame_tags.push(tag.clone());
        tag
    };
    doc.dirty = true;
    Ok(tag)
}

/// Renames an existing frame tag on a sprite in place.
///
/// Pure helper extracted so the validation logic is unit-testable without
/// constructing a tauri `State`. Returns `Ok(false)` when the rename was a
/// no-op (`old == new`); `Ok(true)` when a tag was actually renamed.
fn rename_tag_in_sprite(
    sprite: &mut Sprite,
    old_name: &str,
    new_name: String,
) -> CommandResult<bool> {
    if new_name.is_empty() {
        return Err(AppCommandError::Validation {
            detail: "frame tag name must not be empty".into(),
        });
    }
    if old_name == new_name {
        return Ok(false);
    }
    if sprite.frame_tags.iter().any(|t| t.name == new_name) {
        return Err(AppCommandError::Conflict {
            detail: format!("frame tag {new_name:?} already exists"),
        });
    }
    let tag = sprite
        .frame_tags
        .iter_mut()
        .find(|t| t.name == old_name)
        .ok_or_else(|| AppCommandError::NotFoundByName {
            entity: "frame_tag".into(),
            name: old_name.to_owned(),
        })?;
    tag.name = new_name;
    Ok(true)
}

/// Renames an existing frame tag.
///
/// Rejects empty new names and collisions with another existing tag.
/// `old_name == new_name` is a no-op.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn frame_tag_rename(
    sprite_id: SpriteId,
    old_name: String,
    new_name: String,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let mut doc = state.doc.write().await;
    let changed = {
        let sprite = doc
            .project
            .as_mut()
            .ok_or(AppCommandError::NoActiveProject)?
            .sprite_mut(sprite_id)
            .ok_or(AppCommandError::NotFound {
                entity: "sprite".into(),
                id: u64::from(sprite_id.get()),
            })?;
        rename_tag_in_sprite(sprite, &old_name, new_name)?
    };
    if changed {
        doc.dirty = true;
    }
    Ok(())
}

/// Deletes a named frame tag from a sprite.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn frame_tag_delete(
    sprite_id: SpriteId,
    tag_name: String,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let mut doc = state.doc.write().await;
    {
        let sprite = doc
            .project
            .as_mut()
            .ok_or(AppCommandError::NoActiveProject)?
            .sprite_mut(sprite_id)
            .ok_or(AppCommandError::NotFound {
                entity: "sprite".into(),
                id: u64::from(sprite_id.get()),
            })?;
        let before = sprite.frame_tags.len();
        sprite.frame_tags.retain(|t| t.name != tag_name);
        if sprite.frame_tags.len() == before {
            return Err(AppCommandError::NotFoundByName {
                entity: "frame_tag".into(),
                name: tag_name.clone(),
            });
        }
    }
    doc.dirty = true;
    Ok(())
}

/// Returns all frames in a sprite's timeline.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn frame_list(
    sprite_id: SpriteId,
    state: State<'_, AppState>,
) -> CommandResult<Vec<Frame>> {
    let doc = state.doc.read().await;
    let sprite = doc
        .project
        .as_ref()
        .ok_or(AppCommandError::NoActiveProject)?
        .sprite(sprite_id)
        .ok_or(AppCommandError::NotFound {
            entity: "sprite".into(),
            id: u64::from(sprite_id.get()),
        })?;
    Ok(sprite.frames.clone())
}

/// Returns all cels in a sprite (sparse flat list keyed by layer+frame).
#[tauri::command(async, rename_all = "snake_case")]
pub async fn cel_list(sprite_id: SpriteId, state: State<'_, AppState>) -> CommandResult<Vec<Cel>> {
    let doc = state.doc.read().await;
    let sprite = doc
        .project
        .as_ref()
        .ok_or(AppCommandError::NoActiveProject)?
        .sprite(sprite_id)
        .ok_or(AppCommandError::NotFound {
            entity: "sprite".into(),
            id: u64::from(sprite_id.get()),
        })?;
    Ok(sprite.cels.clone())
}

/// Returns all frame tags for a sprite.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn frame_tag_list(
    sprite_id: SpriteId,
    state: State<'_, AppState>,
) -> CommandResult<Vec<FrameTag>> {
    let doc = state.doc.read().await;
    let sprite = doc
        .project
        .as_ref()
        .ok_or(AppCommandError::NoActiveProject)?
        .sprite(sprite_id)
        .ok_or(AppCommandError::NotFound {
            entity: "sprite".into(),
            id: u64::from(sprite_id.get()),
        })?;
    Ok(sprite.frame_tags.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pixhaus_core::project::{
        Animation, AnimationId, Cel, LayerId, PixelBufferId, Rect, Size, Slice, SliceId, SliceKey,
    };

    fn sprite_with_frames(n: u32) -> Sprite {
        let mut s = Sprite::empty(SpriteId::new(1), "t", Size::new(8, 8));
        for _ in 0..n {
            s.frames.push(Frame::default());
        }
        s
    }

    fn raster_cel(frame: u32, layer: u32) -> Cel {
        Cel::raster(
            LayerId::new(layer),
            FrameIndex::new(frame),
            PixelBufferId::new(1),
            Size::new(8, 8),
        )
    }

    fn slice_with_keys(frames: &[u32]) -> Slice {
        Slice {
            id: SliceId::new(1),
            name: "s".into(),
            keys: frames
                .iter()
                .map(|f| SliceKey {
                    frame: FrameIndex::new(*f),
                    bounds: Rect::from_xywh(0, 0, 1, 1),
                    nine_slice: None,
                    pivot: None,
                })
                .collect(),
            user_data: UserData::default(),
        }
    }

    fn cel_frames(s: &Sprite) -> Vec<u32> {
        s.cels.iter().map(|c| c.frame_index.get()).collect()
    }

    #[test]
    fn frame_add_result_index_is_zero_for_first_frame() {
        let frame = Frame::default();
        let result = FrameAddResult {
            frame: frame.clone(),
            index: FrameIndex::new(0),
        };
        assert_eq!(result.index.get(), 0);
        assert_eq!(result.frame.duration_ms, frame.duration_ms);
    }

    // shift_frame_indices ────────────────────────────────────────────────────

    #[test]
    fn shift_minus_one_drops_indices_from_threshold() {
        let mut s = sprite_with_frames(4);
        s.cels = vec![
            raster_cel(0, 1),
            raster_cel(1, 1),
            raster_cel(2, 1),
            raster_cel(3, 1),
        ];
        // Simulate frame_delete at index 1: caller drops cels at frame 1 first,
        // then shifts everything past with delta = -1.
        s.cels.retain(|c| c.frame_index.get() != 1);
        shift_frame_indices(&mut s, 2, -1);
        assert_eq!(cel_frames(&s), vec![0, 1, 2]);
    }

    #[test]
    fn shift_plus_one_makes_room_for_insertion() {
        let mut s = sprite_with_frames(3);
        s.cels = vec![raster_cel(0, 1), raster_cel(1, 1), raster_cel(2, 1)];
        shift_frame_indices(&mut s, 1, 1);
        assert_eq!(cel_frames(&s), vec![0, 2, 3]);
    }

    #[test]
    fn shift_walks_tags_animations_and_slices() {
        let mut s = sprite_with_frames(5);
        s.frame_tags.push(FrameTag {
            name: "walk".into(),
            range: FrameRange::new(FrameIndex::new(1), FrameIndex::new(3)),
            loop_direction: LoopDirection::Forward,
            repeat: 0,
            user_data: UserData::default(),
        });
        s.animations.push(Animation::forward(
            AnimationId::new(1),
            "walk",
            FrameRange::new(FrameIndex::new(2), FrameIndex::new(4)),
        ));
        s.slices.push(slice_with_keys(&[1, 3, 4]));

        shift_frame_indices(&mut s, 2, -1);

        assert_eq!(s.frame_tags[0].range.start.get(), 1);
        assert_eq!(s.frame_tags[0].range.end.get(), 2);
        assert_eq!(s.animations[0].range.start.get(), 1);
        assert_eq!(s.animations[0].range.end.get(), 3);
        let key_frames: Vec<u32> = s.slices[0].keys.iter().map(|k| k.frame.get()).collect();
        assert_eq!(key_frames, vec![1, 2, 3]);
    }

    // remap_for_reorder ──────────────────────────────────────────────────────

    #[test]
    fn remap_forward_move_shifts_middle_left() {
        // Move frame 1 to position 3: [A B C D] -> [A C D B]
        // Old frame 0 -> 0; old 1 -> 3; old 2 -> 1; old 3 -> 2.
        let r = |n: u32| remap_for_reorder(FrameIndex::new(n), 1, 3).get();
        assert_eq!(r(0), 0);
        assert_eq!(r(1), 3);
        assert_eq!(r(2), 1);
        assert_eq!(r(3), 2);
    }

    #[test]
    fn remap_backward_move_shifts_middle_right() {
        // Move frame 3 to position 1: [A B C D] -> [A D B C]
        // Old 0 -> 0; old 1 -> 2; old 2 -> 3; old 3 -> 1.
        let r = |n: u32| remap_for_reorder(FrameIndex::new(n), 3, 1).get();
        assert_eq!(r(0), 0);
        assert_eq!(r(1), 2);
        assert_eq!(r(2), 3);
        assert_eq!(r(3), 1);
    }

    #[test]
    fn remap_outside_span_is_identity() {
        let r = |n: u32| remap_for_reorder(FrameIndex::new(n), 2, 4).get();
        assert_eq!(r(0), 0);
        assert_eq!(r(1), 1);
        assert_eq!(r(5), 5);
    }

    // rename_tag_in_sprite ───────────────────────────────────────────────────

    fn sprite_with_tag(name: &str) -> Sprite {
        let mut s = sprite_with_frames(4);
        s.frame_tags.push(FrameTag {
            name: name.into(),
            range: FrameRange::new(FrameIndex::new(0), FrameIndex::new(1)),
            loop_direction: LoopDirection::Forward,
            repeat: 0,
            user_data: UserData::default(),
        });
        s
    }

    #[test]
    fn rename_tag_changes_name_and_reports_change() {
        let mut s = sprite_with_tag("walk");
        let changed = rename_tag_in_sprite(&mut s, "walk", "run".into()).expect("rename ok");
        assert!(changed, "rename to a new name reports a change");
        assert_eq!(s.frame_tags[0].name, "run");
    }

    #[test]
    fn rename_tag_to_same_name_is_a_noop() {
        let mut s = sprite_with_tag("walk");
        let changed = rename_tag_in_sprite(&mut s, "walk", "walk".into()).expect("noop ok");
        assert!(!changed, "no-op rename reports no change");
        assert_eq!(s.frame_tags[0].name, "walk");
    }

    #[test]
    fn rename_tag_rejects_empty_new_name() {
        let mut s = sprite_with_tag("walk");
        let err = rename_tag_in_sprite(&mut s, "walk", String::new()).expect_err("empty rejected");
        assert!(matches!(err, AppCommandError::Validation { .. }));
        assert_eq!(s.frame_tags[0].name, "walk");
    }

    #[test]
    fn rename_tag_rejects_collision_with_existing_tag() {
        let mut s = sprite_with_tag("walk");
        s.frame_tags.push(FrameTag {
            name: "run".into(),
            range: FrameRange::new(FrameIndex::new(2), FrameIndex::new(3)),
            loop_direction: LoopDirection::Forward,
            repeat: 0,
            user_data: UserData::default(),
        });
        let err =
            rename_tag_in_sprite(&mut s, "walk", "run".into()).expect_err("collision rejected");
        assert!(matches!(err, AppCommandError::Conflict { .. }));
        assert_eq!(s.frame_tags[0].name, "walk");
    }

    #[test]
    fn rename_tag_rejects_unknown_source_name() {
        let mut s = sprite_with_tag("walk");
        let err = rename_tag_in_sprite(&mut s, "missing", "run".into())
            .expect_err("unknown source rejected");
        assert!(matches!(err, AppCommandError::NotFoundByName { .. }));
        assert_eq!(s.frame_tags[0].name, "walk");
    }
}
