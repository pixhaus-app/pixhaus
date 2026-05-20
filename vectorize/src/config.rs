//! Configuration knobs for `centerline_vectorize`.
//!
//! Defaults mirror OpenToonz `CenterlineConfiguration::CenterlineConfiguration`
//! in `toonz/sources/include/toonz/vectorizerparameters.h` (BSD-3-Clause)
//! scaled to pixel-space units that match our skeleton stage.
//! See `THIRD_PARTY_NOTICES.md`.

/// Tunable knobs for the centerline pipeline.
///
/// All values are in pixel-space units except `corner_threshold_rad`,
/// which is an angle in radians.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct CenterlineConfig {
    /// Maximum stroke half-width, in pixels. Thickness annotations
    /// above this value are clamped.
    pub max_thickness: f32,
    /// Aspect-ratio multiplier for thickness. `1.0` keeps the
    /// distance-transform value as-is; smaller values produce thinner
    /// strokes.
    pub thickness_ratio: f32,
    /// Drop branch segments shorter than this many pixels during
    /// graph organization.
    pub min_segment_length: u32,
    /// Junction-flagging threshold in radians. Currently informational;
    /// reserved for the corner-detection pass in a follow-up stream.
    pub corner_threshold_rad: f32,
    /// Ramer-Douglas-Peucker simplification tolerance, in pixels.
    pub simplify_tolerance: f32,
}

impl Default for CenterlineConfig {
    fn default() -> Self {
        Self {
            max_thickness: 10.0,
            thickness_ratio: 1.0,
            min_segment_length: 2,
            corner_threshold_rad: 0.5,
            simplify_tolerance: 0.5,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_defaults_match_plan() {
        let c = CenterlineConfig::default();
        assert!((c.max_thickness - 10.0).abs() < 1e-6);
        assert!((c.thickness_ratio - 1.0).abs() < 1e-6);
        assert_eq!(c.min_segment_length, 2);
        assert!((c.corner_threshold_rad - 0.5).abs() < 1e-6);
        assert!((c.simplify_tolerance - 0.5).abs() < 1e-6);
    }

    #[test]
    fn config_is_copyable() {
        let a = CenterlineConfig::default();
        let b = a;
        assert_eq!(a, b);
    }
}
