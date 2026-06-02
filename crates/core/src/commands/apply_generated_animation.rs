//! [`ApplyGeneratedAnimation`]: insert generated frames as a new animated sprite.
//!
//! The animation counterpart to [`ApplyGeneratedAsset`](super::ApplyGeneratedAsset).
//! It takes plain decoded RGBA8 frames plus timing and a clip name - never a
//! `services` result type - so `core` stays unaware of the generation layer. The
//! generation module builds it from a `GeneratedAnimation`. One command inserts the
//! whole clip, so the user sees one undo step for one conceptual action (bible 12.3).

use crate::animation::LoopMode;
use crate::command::{Command, CommandError};
use crate::document::{DEFAULT_FRAME_DURATION_MS, Document, Sprite};
use crate::ids::{PixelBufferId, SpriteId};
use crate::pixel::PixelBuffer;

/// One generated frame's pixels: a row `stride` and RGBA8 bytes. Width and height
/// come from the command (every frame shares the sprite's size). Plain data so
/// `core` never depends on a generation result type.
#[derive(Clone, Debug)]
pub struct GeneratedFrameData {
    /// Bytes per row (`>= width * 4`).
    pub stride: u32,
    /// Straight-alpha RGBA8 bytes, row-major.
    pub rgba: Vec<u8>,
}

/// Records what [`ApplyGeneratedAnimation::apply`] inserted, so undo removes exactly
/// that and reclaims the pixels for a later redo.
struct Inserted {
    sprite: SpriteId,
    buffers: Vec<PixelBufferId>,
    prev_active: Option<SpriteId>,
}

/// Inserts generated frames as a new multi-frame sprite with one clip, made active.
pub struct ApplyGeneratedAnimation {
    name: String,
    clip_name: String,
    width: u32,
    height: u32,
    fps: u16,
    loop_mode: LoopMode,
    /// Source frames: `Some` while pending (before apply, or after undo); `None`
    /// while applied, when the bytes live in the document's buffer store.
    frames: Option<Vec<GeneratedFrameData>>,
    inserted: Option<Inserted>,
}

impl ApplyGeneratedAnimation {
    /// A command that will insert `frames` as a new animated sprite named `name`,
    /// with one clip named `clip_name` spanning all the frames at `fps`.
    pub fn new(name: String, clip_name: String, width: u32, height: u32, fps: u16, loop_mode: LoopMode, frames: Vec<GeneratedFrameData>) -> Self {
        Self {
            name,
            clip_name,
            width,
            height,
            fps,
            loop_mode,
            frames: Some(frames),
            inserted: None,
        }
    }

    /// The per-frame hold time derived from the clip fps, clamped to at least 1ms.
    fn frame_duration_ms(&self) -> u32 {
        if self.fps == 0 {
            DEFAULT_FRAME_DURATION_MS
        } else {
            (1000 / u32::from(self.fps)).max(1)
        }
    }
}

impl Command for ApplyGeneratedAnimation {
    fn apply(&mut self, doc: &mut Document) -> Result<(), CommandError> {
        let frames = self.frames.take().ok_or(CommandError::InvalidState)?;
        if frames.is_empty() {
            // An animation needs at least one frame; restore for a clean caller error.
            self.frames = Some(frames);
            return Err(CommandError::InvalidState);
        }
        // Build (and validate) every frame's pixels before touching the document, so a
        // malformed frame leaves the document untouched rather than half-mutated.
        let mut built = Vec::with_capacity(frames.len());
        for frame in frames {
            built.push(PixelBuffer::from_rgba8(self.width, self.height, frame.stride, frame.rgba)?);
        }
        let frame_count = u32::try_from(built.len()).map_err(|_| CommandError::InvalidState)?;
        let frame_ms = self.frame_duration_ms();

        let sprite_id = doc.mint_sprite_id();
        let mut sprite = Sprite::new_single_frame(sprite_id, self.name.clone(), self.width, self.height);
        let mut buffers = Vec::with_capacity(built.len());
        for (index, pixels) in built.into_iter().enumerate() {
            let buffer = doc.buffers.insert(pixels);
            buffers.push(buffer);
            // Frame 0 is the one `new_single_frame` created; the rest are appended.
            if index > 0 {
                sprite.push_frame(frame_ms);
            }
            if let Some(frame) = sprite.frames.last_mut() {
                frame.duration_ms = frame_ms;
                frame.add_layer(self.name.clone(), buffer);
            }
        }
        sprite.push_clip(self.clip_name.clone(), 0, frame_count - 1, self.fps, self.loop_mode);

        let prev_active = doc.active_sprite;
        doc.sprites.push(sprite);
        doc.active_sprite = Some(sprite_id);
        doc.bump_revision();
        self.inserted = Some(Inserted {
            sprite: sprite_id,
            buffers,
            prev_active,
        });
        Ok(())
    }

    fn undo(&mut self, doc: &mut Document) -> Result<(), CommandError> {
        let inserted = self.inserted.take().ok_or(CommandError::InvalidState)?;
        let pos = doc
            .sprites
            .iter()
            .position(|s| s.id == inserted.sprite)
            .ok_or(CommandError::SpriteNotFound(inserted.sprite))?;
        doc.sprites.remove(pos);
        // Reclaim the pixels in insertion order so a redo rebuilds identical frames.
        let mut frames = Vec::with_capacity(inserted.buffers.len());
        for buffer_id in inserted.buffers {
            let buffer = doc.buffers.remove(buffer_id).ok_or(CommandError::InvalidState)?;
            frames.push(GeneratedFrameData {
                stride: buffer.stride(),
                rgba: buffer.into_bytes(),
            });
        }
        doc.active_sprite = inserted.prev_active;
        doc.bump_revision();
        self.frames = Some(frames);
        Ok(())
    }

    fn label_key(&self) -> &'static str {
        "command.apply_generated_animation"
    }

    fn estimated_size_bytes(&self) -> usize {
        // The frames count toward history memory only while the command holds them
        // (pending / undone); while applied they live in the document, not here.
        let frame_bytes: usize = self.frames.as_ref().map_or(0, |fs| fs.iter().map(|f| f.rgba.len()).sum());
        frame_bytes + std::mem::size_of::<Self>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::composite::composite_frame;

    // A frame filled with one opaque colour, tightly packed at the given size.
    fn solid_frame(width: u32, height: u32, rgba: [u8; 4]) -> GeneratedFrameData {
        let mut bytes = Vec::with_capacity((width * height * 4) as usize);
        for _ in 0..(width * height) {
            bytes.extend_from_slice(&rgba);
        }
        GeneratedFrameData {
            stride: width * 4,
            rgba: bytes,
        }
    }

    fn three_frames() -> Vec<GeneratedFrameData> {
        vec![
            solid_frame(2, 2, [255, 0, 0, 255]),
            solid_frame(2, 2, [0, 255, 0, 255]),
            solid_frame(2, 2, [0, 0, 255, 255]),
        ]
    }

    #[test]
    fn apply_inserts_a_multi_frame_sprite_with_one_clip() {
        let mut doc = Document::new();
        let mut cmd = ApplyGeneratedAnimation::new("bit".to_owned(), "idle".to_owned(), 2, 2, 12, LoopMode::Loop, three_frames());
        cmd.apply(&mut doc).unwrap();

        let id = doc.active_sprite().unwrap();
        let sprite = doc.sprite(id).unwrap();
        assert_eq!(sprite.frames().len(), 3, "one frame per generated image");
        assert_eq!(sprite.clips().len(), 1, "one idle clip");
        let clip = &sprite.clips()[0];
        assert_eq!(clip.name, "idle");
        assert_eq!(clip.frame_count(), 3);
        assert_eq!(clip.fps, 12);
        assert_eq!(clip.loop_mode, LoopMode::Loop);
    }

    #[test]
    fn each_frame_composites_to_its_generated_bytes() {
        let mut doc = Document::new();
        let mut cmd = ApplyGeneratedAnimation::new("bit".to_owned(), "idle".to_owned(), 2, 2, 8, LoopMode::Loop, three_frames());
        cmd.apply(&mut doc).unwrap();

        let id = doc.active_sprite().unwrap();
        let frame_ids: Vec<_> = doc.sprite(id).unwrap().frames().iter().map(|f| f.id).collect();
        let red = composite_frame(&doc, id, frame_ids[0]).unwrap();
        let green = composite_frame(&doc, id, frame_ids[1]).unwrap();
        assert_eq!(red.pixel(0, 0).unwrap(), crate::Rgba::new(255, 0, 0, 255));
        assert_eq!(green.pixel(0, 0).unwrap(), crate::Rgba::new(0, 255, 0, 255));
    }

    #[test]
    fn undo_removes_all_buffers_then_redo_restores() {
        let mut doc = Document::new();
        let mut cmd = ApplyGeneratedAnimation::new("bit".to_owned(), "idle".to_owned(), 2, 2, 8, LoopMode::Loop, three_frames());

        cmd.apply(&mut doc).unwrap();
        assert_eq!(doc.sprites().len(), 1);
        assert_eq!(doc.buffers().len(), 3, "one buffer per frame");

        cmd.undo(&mut doc).unwrap();
        assert_eq!(doc.sprites().len(), 0);
        assert_eq!(doc.buffers().len(), 0, "undo reclaims every frame buffer");

        cmd.apply(&mut doc).unwrap();
        let id = doc.active_sprite().unwrap();
        assert_eq!(doc.sprite(id).unwrap().frames().len(), 3, "redo rebuilds every frame");
    }

    #[test]
    fn empty_frames_is_invalid_state() {
        let mut doc = Document::new();
        let mut cmd = ApplyGeneratedAnimation::new("bit".to_owned(), "idle".to_owned(), 2, 2, 8, LoopMode::Loop, Vec::new());
        assert!(matches!(cmd.apply(&mut doc), Err(CommandError::InvalidState)));
    }

    #[test]
    fn size_held_only_while_pending() {
        let mut cmd = ApplyGeneratedAnimation::new("bit".to_owned(), "idle".to_owned(), 2, 2, 8, LoopMode::Loop, three_frames());
        let pending = cmd.estimated_size_bytes();
        assert!(pending > 3 * 16, "pending holds three 2x2 RGBA frames");
        let mut doc = Document::new();
        cmd.apply(&mut doc).unwrap();
        assert!(cmd.estimated_size_bytes() < pending, "applied frames live in the document, not the command");
    }
}
