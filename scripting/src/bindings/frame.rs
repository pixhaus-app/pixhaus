//! Lua `Frame` userdata — mirrors Aseprite's Frame class.
//!
//! `frame.duration` exposes the duration in **seconds** (Aseprite
//! convention), converting from the internal milliseconds
//! representation. `frame.frameNumber` is 1-indexed (Aseprite API
//! convention).

use std::sync::Arc;

use mlua::prelude::*;
use parking_lot::Mutex;
use pixhaus_core::project::{Frame, FrameIndex};

use crate::output::ScriptMutation;

/// Lua `Frame` userdata.
#[derive(Clone)]
pub struct LuaFrame {
    /// Frame index (0-based internally).
    pub frame_index: FrameIndex,
    /// Duration in milliseconds (internal representation).
    pub duration_ms: u32,
    pub(crate) mutations: Arc<Mutex<Vec<ScriptMutation>>>,
}

impl LuaFrame {
    /// Wraps a core `Frame` with its timeline index and a mutation sink.
    #[must_use]
    pub fn from_frame(
        frame: &Frame,
        index: FrameIndex,
        mutations: Arc<Mutex<Vec<ScriptMutation>>>,
    ) -> Self {
        Self {
            frame_index: index,
            duration_ms: frame.duration_ms,
            mutations,
        }
    }

    /// Returns the duration in seconds (Aseprite convention).
    #[must_use]
    pub fn duration_secs(&self) -> f64 {
        f64::from(self.duration_ms) / 1000.0
    }

    /// Returns the 1-indexed frame number (Aseprite convention).
    #[must_use]
    pub fn frame_number(&self) -> u32 {
        self.frame_index.get() + 1
    }
}

impl LuaUserData for LuaFrame {
    fn add_fields<F: LuaUserDataFields<Self>>(fields: &mut F) {
        // frameNumber — 1-indexed, read-only (changing it would reorder frames)
        fields.add_field_method_get("frameNumber", |_, this| Ok(this.frame_number()));

        // duration — in seconds, read/write (Aseprite convention)
        fields.add_field_method_get("duration", |_, this| Ok(this.duration_secs()));
        fields.add_field_method_set("duration", |_, this, secs: f64| {
            // Reject NaN, +/-inf, and negative values up front. Without
            // the is_finite check `(secs * 1000.0).round() as u32` would
            // saturate to 0 or u32::MAX silently.
            if !secs.is_finite() {
                return Err(LuaError::runtime(
                    "frame.duration must be a finite number (got NaN or infinity)",
                ));
            }
            if secs < 0.0 {
                return Err(LuaError::runtime("frame.duration must be non-negative"));
            }
            // After the finite + non-negative checks, the cast to u32
            // is safe: secs * 1000 is in [0, MAX_FINITE_F64], and we
            // clamp to u32::MAX explicitly to avoid saturation
            // surprises on values larger than u32::MAX milliseconds
            // (~49.7 days, well outside any sane frame duration).
            let raw = (secs * 1000.0).round();
            let clamped = raw.clamp(0.0, f64::from(u32::MAX));
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let ms = clamped as u32;
            let duration_ms = ms.max(1);
            this.mutations
                .lock()
                .push(ScriptMutation::SetFrameDuration {
                    frame: this.frame_index,
                    duration_ms,
                });
            this.duration_ms = duration_ms;
            Ok(())
        });
    }

    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_meta_method(LuaMetaMethod::ToString, |_, this, ()| {
            Ok(format!("Frame({})", this.frame_number()))
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pixhaus_core::project::{Frame, UserData};

    fn make_frame(duration_ms: u32) -> Frame {
        Frame {
            duration_ms,
            user_data: UserData::default(),
        }
    }

    #[test]
    fn duration_converts_ms_to_seconds() {
        let sink = Arc::new(Mutex::new(Vec::new()));
        let f = LuaFrame::from_frame(&make_frame(100), FrameIndex::new(0), sink);
        assert!((f.duration_secs() - 0.1).abs() < f64::EPSILON * 10.0);
    }

    #[test]
    fn frame_number_is_one_indexed() {
        let sink = Arc::new(Mutex::new(Vec::new()));
        let f = LuaFrame::from_frame(&make_frame(100), FrameIndex::new(0), sink.clone());
        assert_eq!(f.frame_number(), 1);
        let f2 = LuaFrame::from_frame(&make_frame(100), FrameIndex::new(4), sink);
        assert_eq!(f2.frame_number(), 5);
    }
}
