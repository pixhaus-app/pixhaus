//! Frame timeline commands: add, delete, duplicate, reorder, tag CRUD.

use pixhaus_core::project::{
    Frame, FrameIndex, FrameRange, FrameTag, LoopDirection, SpriteId, UserData,
};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::state::AppState;

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
#[tauri::command(async)]
pub async fn frame_add(
    sprite_id: SpriteId,
    duration_ms: u32,
    state: State<'_, AppState>,
) -> Result<FrameAddResult, String> {
    let mut doc = state.doc.lock().await;
    let result = {
        let sprite = doc
            .project
            .as_mut()
            .ok_or("no active project")?
            .sprites
            .iter_mut()
            .find(|s| s.id == sprite_id)
            .ok_or_else(|| format!("sprite {} not found", sprite_id.get()))?;
        let frame = Frame {
            duration_ms,
            user_data: UserData::default(),
        };
        let index = FrameIndex::new(
            u32::try_from(sprite.frames.len()).map_err(|_| "sprite has too many frames")?,
        );
        sprite.frames.push(frame.clone());
        FrameAddResult { frame, index }
    };
    doc.dirty = true;
    Ok(result)
}

/// Deletes a frame from the timeline by index. Also removes all cels on that frame.
#[tauri::command(async)]
pub async fn frame_delete(
    sprite_id: SpriteId,
    frame_index: FrameIndex,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut doc = state.doc.lock().await;
    {
        let sprite = doc
            .project
            .as_mut()
            .ok_or("no active project")?
            .sprites
            .iter_mut()
            .find(|s| s.id == sprite_id)
            .ok_or_else(|| format!("sprite {} not found", sprite_id.get()))?;
        let idx = frame_index.get() as usize;
        if idx >= sprite.frames.len() {
            return Err(format!(
                "frame index {} out of range (sprite has {} frames)",
                idx,
                sprite.frames.len()
            ));
        }
        sprite.frames.remove(idx);
        sprite.cels.retain(|c| c.frame_index != frame_index);
    }
    doc.dirty = true;
    Ok(())
}

/// Duplicates a frame, inserting the copy immediately after the source.
#[tauri::command(async)]
pub async fn frame_duplicate(
    sprite_id: SpriteId,
    frame_index: FrameIndex,
    state: State<'_, AppState>,
) -> Result<FrameAddResult, String> {
    let mut doc = state.doc.lock().await;
    let result = {
        let sprite = doc
            .project
            .as_mut()
            .ok_or("no active project")?
            .sprites
            .iter_mut()
            .find(|s| s.id == sprite_id)
            .ok_or_else(|| format!("sprite {} not found", sprite_id.get()))?;
        let idx = frame_index.get() as usize;
        if idx >= sprite.frames.len() {
            return Err(format!(
                "frame index {} out of range (sprite has {} frames)",
                idx,
                sprite.frames.len()
            ));
        }
        let frame = sprite.frames[idx].clone();
        let insert_at = idx + 1;
        sprite.frames.insert(insert_at, frame.clone());
        FrameAddResult {
            frame,
            index: FrameIndex::new(
                u32::try_from(insert_at).map_err(|_| "frame index exceeds u32::MAX")?,
            ),
        }
    };
    doc.dirty = true;
    Ok(result)
}

/// Moves a frame from one position to another in the timeline.
#[tauri::command(async)]
pub async fn frame_reorder(
    sprite_id: SpriteId,
    from_index: FrameIndex,
    to_index: FrameIndex,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut doc = state.doc.lock().await;
    {
        let sprite = doc
            .project
            .as_mut()
            .ok_or("no active project")?
            .sprites
            .iter_mut()
            .find(|s| s.id == sprite_id)
            .ok_or_else(|| format!("sprite {} not found", sprite_id.get()))?;
        let from = from_index.get() as usize;
        let len = sprite.frames.len();
        if from >= len {
            return Err(format!(
                "from_index {from} out of range (sprite has {len} frames)"
            ));
        }
        let to = (to_index.get() as usize).min(len.saturating_sub(1));
        let frame = sprite.frames.remove(from);
        sprite.frames.insert(to, frame);
    }
    doc.dirty = true;
    Ok(())
}

/// Updates the display duration for a single frame.
#[tauri::command(async)]
pub async fn frame_set_duration(
    sprite_id: SpriteId,
    frame_index: FrameIndex,
    duration_ms: u32,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut doc = state.doc.lock().await;
    {
        let sprite = doc
            .project
            .as_mut()
            .ok_or("no active project")?
            .sprites
            .iter_mut()
            .find(|s| s.id == sprite_id)
            .ok_or_else(|| format!("sprite {} not found", sprite_id.get()))?;
        let idx = frame_index.get() as usize;
        let frame = sprite
            .frames
            .get_mut(idx)
            .ok_or_else(|| format!("frame index {idx} out of range"))?;
        frame.duration_ms = duration_ms;
    }
    doc.dirty = true;
    Ok(())
}

/// Creates a named frame tag on a sprite.
///
/// Tags with duplicate names are rejected.
#[tauri::command(async)]
pub async fn frame_tag_create(
    args: FrameTagCreateArgs,
    state: State<'_, AppState>,
) -> Result<FrameTag, String> {
    let mut doc = state.doc.lock().await;
    let tag = {
        let sprite = doc
            .project
            .as_mut()
            .ok_or("no active project")?
            .sprites
            .iter_mut()
            .find(|s| s.id == args.sprite_id)
            .ok_or_else(|| format!("sprite {} not found", args.sprite_id.get()))?;
        if sprite.frame_tags.iter().any(|t| t.name == args.name) {
            return Err(format!("frame tag {:?} already exists", args.name));
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

/// Deletes a named frame tag from a sprite.
#[tauri::command(async)]
pub async fn frame_tag_delete(
    sprite_id: SpriteId,
    tag_name: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut doc = state.doc.lock().await;
    {
        let sprite = doc
            .project
            .as_mut()
            .ok_or("no active project")?
            .sprites
            .iter_mut()
            .find(|s| s.id == sprite_id)
            .ok_or_else(|| format!("sprite {} not found", sprite_id.get()))?;
        let before = sprite.frame_tags.len();
        sprite.frame_tags.retain(|t| t.name != tag_name);
        if sprite.frame_tags.len() == before {
            return Err(format!("frame tag {tag_name:?} not found"));
        }
    }
    doc.dirty = true;
    Ok(())
}

/// Returns all frames in a sprite's timeline.
#[tauri::command(async)]
pub async fn frame_list(
    sprite_id: SpriteId,
    state: State<'_, AppState>,
) -> Result<Vec<Frame>, String> {
    let doc = state.doc.lock().await;
    let sprite = doc
        .project
        .as_ref()
        .ok_or("no active project")?
        .sprites
        .iter()
        .find(|s| s.id == sprite_id)
        .ok_or_else(|| format!("sprite {} not found", sprite_id.get()))?;
    Ok(sprite.frames.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
