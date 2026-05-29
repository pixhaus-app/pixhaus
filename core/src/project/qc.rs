//! Per-frame QC and provenance for a landed animation.
//!
//! The studio measures a [`NormalizeReport`](crate::transforms::NormalizeReport)
//! and per-frame [`FrameMetrics`](crate::transforms::FrameMetrics) when it
//! reviews a normalized loop, then drops both when it lands the frames. This
//! module is the serializable record that survives the Land: the exact key,
//! tolerance, alpha threshold, and canvas the frames came from, the aggregate
//! drift/scale/seam/component report, and a per-frame QC row.
//!
//! Why it exists: a pipeline that iterates cheaply must record how each artifact
//! was processed so a follow-up pass can re-run post-processing with adjusted
//! flags before an expensive re-generation. An [`AnimationQc`] turns the studio's
//! observe-once measurement into a durable record an artist — or an automated
//! loop — reads to decide reprocess-with-tighter-flags vs regenerate.
//!
//! Every field is serde-defaulted where it is optional so an older record (one
//! written before a field existed) still loads. This rides on the in-memory
//! [`Animation`](super::animation::Animation) today and serializes for free when
//! the `.pixhaus` save lands.

use serde::{Deserialize, Serialize};

use crate::project::Rgba;
use crate::transforms::{FrameMetrics, NormalizeReport, SeamMatch};

/// The reproducible inputs a normalize pass ran with, captured so the post-pass
/// can be re-run with adjusted flags before a regenerate.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QcSettings {
    /// The chroma key colour the frames were stripped against.
    pub key_color: Rgba,
    /// The key tolerance slider used.
    pub key_tolerance: u8,
    /// The alpha cutoff that counted a pixel as opaque.
    pub alpha_threshold: u8,
    /// The output canvas `(width, height)` frames were re-padded onto.
    pub canvas: (u32, u32),
    /// The bottom margin, in pixels, the foot baseline was placed above.
    pub bottom_margin: u32,
    /// The reference visible height every subject was scaled toward.
    pub reference_height: u32,
    /// Whether the chroma key was baked into the landed frames (vs left for the
    /// timeline op).
    pub remove_on_land: bool,
}

/// Per-frame QC for one landed frame, mirroring the
/// [`FrameMetrics`](crate::transforms::FrameMetrics) fields the record keeps.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrameQc {
    /// The frame's opaque bounding box `(x, y, w, h)`.
    pub bbox: (u32, u32, u32, u32),
    /// Horizontal centre of the opaque content.
    pub center_x: u32,
    /// Bottom row of opaque pixels — the foot baseline.
    pub foot_baseline_y: u32,
    /// `true` when the frame had no opaque pixels.
    pub empty: bool,
    /// Number of 4-connected opaque components.
    pub num_components: u32,
    /// Opaque-pixel area of the largest component.
    pub largest_component_area: u32,
    /// Bounding box `(x, y, w, h)` of the largest component, omitted on the wire
    /// when the frame is empty.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub largest_component_bbox: Option<(u32, u32, u32, u32)>,
    /// `true` when any opaque pixel sits on the frame's 1px border.
    pub edge_touch: bool,
}

impl From<&FrameMetrics> for FrameQc {
    fn from(m: &FrameMetrics) -> Self {
        Self {
            bbox: (m.bbox_x, m.bbox_y, m.visible_width, m.visible_height),
            center_x: m.center_x,
            foot_baseline_y: m.foot_baseline_y,
            empty: m.empty,
            num_components: m.num_components,
            largest_component_area: m.largest_component_area,
            largest_component_bbox: m.largest_component_bbox,
            edge_touch: m.edge_touch,
        }
    }
}

/// The aggregate report fields the QC record keeps, the serializable summary of
/// [`NormalizeReport`](crate::transforms::NormalizeReport).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NormalizeReportSummary {
    /// Worst foot-baseline deviation across the landed frames, in pixels.
    pub baseline_drift_px: u32,
    /// Pre-scale height agreement as a percentage.
    pub scale_match_pct: u32,
    /// Loop-seam verdict between the first and last frame.
    pub seam: SeamMatch,
    /// The reference visible height every subject was scaled toward.
    pub reference_height: u32,
    /// The most components any landed frame split into.
    pub max_components: u32,
    /// `true` when any landed frame has an opaque pixel on its 1px border.
    pub any_edge_touch: bool,
    /// Human-readable warnings recorded by the pass, omitted on the wire when
    /// empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

impl From<&NormalizeReport> for NormalizeReportSummary {
    fn from(r: &NormalizeReport) -> Self {
        Self {
            baseline_drift_px: r.baseline_drift_px,
            scale_match_pct: r.scale_match_pct,
            seam: r.seam,
            reference_height: r.reference_height,
            max_components: r.max_components,
            any_edge_touch: r.any_edge_touch,
            warnings: r.warnings.clone(),
        }
    }
}

/// The full QC record persisted on a landed animation: the settings it ran
/// with, the aggregate report, and a per-frame row.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnimationQc {
    /// The reproducible inputs the pass ran with.
    pub settings: QcSettings,
    /// The aggregate drift/scale/seam/component report.
    pub report: NormalizeReportSummary,
    /// Per-frame QC, one row per landed frame, in play order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub frames: Vec<FrameQc>,
}

impl AnimationQc {
    /// Builds a record from the pass output (`report`, per-frame `metrics`) and
    /// the `settings` the caller built the [`NormalizeOptions`] from. The studio
    /// holds all three at Land before it drops them.
    ///
    /// [`NormalizeOptions`]: crate::transforms::NormalizeOptions
    #[must_use]
    pub fn from_pass(settings: QcSettings, report: &NormalizeReport, metrics: &[FrameMetrics]) -> Self {
        Self {
            settings,
            report: NormalizeReportSummary::from(report),
            frames: metrics.iter().map(FrameQc::from).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transforms::{NormalizeOptions, measure_qc, normalize_frames};

    fn settings() -> QcSettings {
        QcSettings {
            key_color: Rgba::opaque(255, 0, 255),
            key_tolerance: 24,
            alpha_threshold: 8,
            canvas: (32, 32),
            bottom_margin: 1,
            reference_height: 20,
            remove_on_land: true,
        }
    }

    fn solid(w: u32, h: u32, color: Rgba) -> crate::canvas::buffer::PixelBuffer {
        crate::canvas::buffer::PixelBuffer::filled(w, h, color).unwrap()
    }

    /// A populated QC record built from a real normalize pass, for the snapshot
    /// and round-trip tests.
    fn populated_qc() -> AnimationQc {
        let mut f = solid(32, 32, Rgba::opaque(255, 0, 255));
        for y in 16..28 {
            for x in 12..20 {
                f.set_pixel(x, y, Rgba::opaque(10, 20, 30));
            }
        }
        let opts = NormalizeOptions {
            bottom_margin: 1,
            ..NormalizeOptions::square(32)
        };
        let res = normalize_frames(&[f.clone(), f], &opts).expect("normalize");
        AnimationQc::from_pass(settings(), &res.report, &res.metrics)
    }

    #[test]
    fn frame_qc_from_metrics_maps_every_field() {
        let mut buf = crate::canvas::buffer::PixelBuffer::new(10, 10).unwrap();
        for y in 4..8 {
            for x in 3..6 {
                buf.set_pixel(x, y, Rgba::opaque(10, 20, 30));
            }
        }
        buf.set_pixel(8, 1, Rgba::opaque(10, 20, 30)); // a detached speck
        let m = measure_qc(&buf, 0);
        let qc = FrameQc::from(&m);
        assert_eq!(qc.bbox, (m.bbox_x, m.bbox_y, m.visible_width, m.visible_height));
        assert_eq!(qc.center_x, m.center_x);
        assert_eq!(qc.foot_baseline_y, m.foot_baseline_y);
        assert_eq!(qc.empty, m.empty);
        assert_eq!(qc.num_components, 2, "body plus speck");
        assert_eq!(qc.largest_component_area, m.largest_component_area);
        assert_eq!(qc.largest_component_bbox, m.largest_component_bbox);
        assert_eq!(qc.edge_touch, m.edge_touch);
    }

    #[test]
    fn report_summary_from_report_maps_fields() {
        let f = solid(16, 16, Rgba::opaque(255, 0, 255));
        let res = normalize_frames(&[f.clone(), f], &NormalizeOptions::square(16)).expect("normalize");
        let summary = NormalizeReportSummary::from(&res.report);
        assert_eq!(summary.baseline_drift_px, res.report.baseline_drift_px);
        assert_eq!(summary.scale_match_pct, res.report.scale_match_pct);
        assert_eq!(summary.seam, res.report.seam);
        assert_eq!(summary.reference_height, res.report.reference_height);
        assert_eq!(summary.max_components, res.report.max_components);
        assert_eq!(summary.any_edge_touch, res.report.any_edge_touch);
        assert_eq!(summary.warnings, res.report.warnings);
    }

    #[test]
    fn animation_qc_round_trip() {
        let qc = populated_qc();
        let json = serde_json::to_string(&qc).expect("encode");
        let back: AnimationQc = serde_json::from_str(&json).expect("decode");
        assert_eq!(back, qc, "the record survives the JSON round-trip");
    }

    #[test]
    fn animation_qc_round_trip_msgpack() {
        // The on-disk format is MessagePack; the record must round-trip there too.
        let qc = populated_qc();
        let bytes = rmp_serde::to_vec_named(&qc).expect("encode msgpack");
        let back: AnimationQc = rmp_serde::from_slice(&bytes).expect("decode msgpack");
        assert_eq!(back, qc, "the record survives the MessagePack round-trip");
    }

    #[test]
    fn deserializes_record_missing_optional_fields() {
        // A record written before the optional fields existed: the per-frame
        // `frames` list and `largest_component_bbox` are absent, the report
        // `warnings` are absent. All must default.
        let json = r#"{
            "settings": {
                "key_color": {"r":255,"g":0,"b":255,"a":255},
                "key_tolerance": 24,
                "alpha_threshold": 8,
                "canvas": [32, 32],
                "bottom_margin": 0,
                "reference_height": 20,
                "remove_on_land": true
            },
            "report": {
                "baseline_drift_px": 0,
                "scale_match_pct": 100,
                "seam": "ok",
                "reference_height": 20,
                "max_components": 1,
                "any_edge_touch": false
            }
        }"#;
        let qc: AnimationQc = serde_json::from_str(json).expect("decode legacy record");
        assert!(qc.frames.is_empty(), "the frames list defaults to empty");
        assert!(qc.report.warnings.is_empty(), "warnings default to empty");
    }

    #[test]
    fn empty_frame_qc_skips_bbox_on_the_wire() {
        let qc = FrameQc {
            bbox: (0, 0, 0, 0),
            center_x: 8,
            foot_baseline_y: 15,
            empty: true,
            num_components: 0,
            largest_component_area: 0,
            largest_component_bbox: None,
            edge_touch: false,
        };
        let json = serde_json::to_string(&qc).expect("encode");
        assert!(!json.contains("largest_component_bbox"), "a None bbox is skipped: {json}");
        let back: FrameQc = serde_json::from_str(&json).expect("decode");
        assert_eq!(back, qc);
    }

    #[test]
    fn snapshot_populated_record() {
        let qc = populated_qc();
        let json = serde_json::to_string_pretty(&qc).expect("encode");
        insta::assert_snapshot!(json);
    }
}
