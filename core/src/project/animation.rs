//! Engine-handoff animation entries.
//!
//! [`super::frame::FrameTag`] organises the timeline for the editor.
//! `Animation` is the analogous entry on the engine side: a named
//! frame range with a playback hint, plus a `speed_multiplier` an
//! exporter can write into the target engine's animation clip.

use serde::{Deserialize, Serialize};

use super::frame::{FrameRange, LoopDirection};
use super::id::AnimationId;
use super::qc::AnimationQc;
use super::user_data::UserData;

/// A named animation entry referencing a frame range.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Animation {
    /// Stable identifier.
    pub id: AnimationId,
    /// Display name. Exporters use this verbatim as the engine-side
    /// clip name; pick something legal for the target runtime.
    pub name: String,
    /// Inclusive range of frames this animation plays.
    pub range: FrameRange,
    /// Direction in which the range plays.
    pub loop_direction: LoopDirection,
    /// Multiplier applied to per-frame durations during playback. `1.0`
    /// is the editor speed; `2.0` plays twice as fast.
    pub speed_multiplier: f32,
    /// Per-frame QC and provenance recorded when an AI-generated loop landed:
    /// the key/tolerance/canvas the frames came from and per-frame component /
    /// edge-touch metrics. `None` for an animation that was not landed through
    /// the studio's normalize path. Serde-defaulted and omitted when absent so
    /// older files load and animations without QC stay byte-identical.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub qc: Option<AnimationQc>,
    /// Free-form user metadata.
    #[serde(skip_serializing_if = "UserData::is_empty", default)]
    pub user_data: UserData,
}

impl Animation {
    /// Constructs an animation that plays `range` forward at editor speed.
    #[must_use]
    pub fn forward(id: AnimationId, name: impl Into<String>, range: FrameRange) -> Self {
        Self {
            id,
            name: name.into(),
            range,
            loop_direction: LoopDirection::Forward,
            speed_multiplier: 1.0,
            qc: None,
            user_data: UserData::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::id::FrameIndex;

    #[test]
    fn forward_constructor_defaults() {
        let r = FrameRange::new(FrameIndex::new(0), FrameIndex::new(3));
        let a = Animation::forward(AnimationId::new(1), "idle", r);
        assert_eq!(a.loop_direction, LoopDirection::Forward);
        assert!((a.speed_multiplier - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn animation_round_trip() {
        let a = Animation::forward(AnimationId::new(1), "walk", FrameRange::new(FrameIndex::new(0), FrameIndex::new(3)));
        let json = serde_json::to_string(&a).unwrap();
        let back: Animation = serde_json::from_str(&json).unwrap();
        assert_eq!(a, back);
    }

    fn sample_qc() -> crate::project::qc::AnimationQc {
        use crate::project::Rgba;
        use crate::project::qc::{AnimationQc, FrameQc, NormalizeReportSummary, QcSettings};
        use crate::transforms::SeamMatch;
        AnimationQc {
            settings: QcSettings {
                key_color: Rgba::opaque(255, 0, 255),
                key_tolerance: 24,
                alpha_threshold: 8,
                canvas: (32, 32),
                bottom_margin: 1,
                reference_height: 20,
                remove_on_land: true,
            },
            report: NormalizeReportSummary {
                baseline_drift_px: 0,
                scale_match_pct: 100,
                seam: SeamMatch::Ok,
                reference_height: 20,
                max_components: 1,
                any_edge_touch: false,
                warnings: Vec::new(),
            },
            frames: vec![FrameQc {
                bbox: (12, 8, 8, 12),
                center_x: 16,
                foot_baseline_y: 19,
                empty: false,
                num_components: 1,
                largest_component_area: 96,
                largest_component_bbox: Some((12, 8, 8, 12)),
                edge_touch: false,
            }],
        }
    }

    #[test]
    fn animation_round_trip_with_qc() {
        let mut a = Animation::forward(AnimationId::new(1), "walk", FrameRange::new(FrameIndex::new(0), FrameIndex::new(3)));
        a.qc = Some(sample_qc());
        let json = serde_json::to_string(&a).unwrap();
        assert!(json.contains("\"qc\""), "the qc payload is written: {json}");
        let back: Animation = serde_json::from_str(&json).unwrap();
        assert_eq!(a, back);
    }

    #[test]
    fn animation_without_qc_omits_the_field() {
        let a = Animation::forward(AnimationId::new(1), "idle", FrameRange::new(FrameIndex::new(0), FrameIndex::new(1)));
        assert_eq!(a.qc, None);
        let json = serde_json::to_string(&a).unwrap();
        assert!(!json.contains("qc"), "an animation with no qc omits the field: {json}");
        let back: Animation = serde_json::from_str(&json).unwrap();
        assert_eq!(a, back, "the None case is byte-identical");
    }

    #[test]
    fn deserializes_old_animation_without_qc() {
        // A record written before the qc field existed still loads.
        let json = r#"{"id":1,"name":"walk","range":{"start":0,"end":3},"loop_direction":"forward","speed_multiplier":1.0}"#;
        let a: Animation = serde_json::from_str(json).expect("decode legacy animation");
        assert_eq!(a.qc, None, "qc defaults to None");
        assert_eq!(a.name, "walk");
    }
}
