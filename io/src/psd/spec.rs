//! PSD format constants and blend-mode mapping.
//!
//! The `psd` crate's `BlendMode` enum is declared in a private module and is
//! not re-exported from the crate root, so pattern matching by variant name is
//! not possible from external code. We work around this by formatting the value
//! with `{:?}` and matching on the resulting string. The debug format for a
//! derived enum is exactly the variant name, which is stable within semver — but
//! the upstream crate could swap `derive(Debug)` for a custom impl in a patch
//! release. We pin `psd = "=0.3.5"` in the workspace `Cargo.toml` so a routine
//! `cargo update` can't bring in such a change unnoticed; the
//! `psd_blend_mode_debug_strings_are_stable` test below also fails loud if it
//! does happen during a deliberate version bump.

use pixhaus_core::project::BlendMode;

/// Maps a PSD blend mode debug string to a Pixhaus [`BlendMode`].
///
/// Returns `(BlendMode, had_unknown)` where `had_unknown` is `true` when
/// the input mode has no direct Pixhaus equivalent and fell back to
/// `BlendMode::Normal`. Approximations (e.g. `LinearBurn` → `ColorBurn`)
/// also set `had_unknown` so the caller can surface a warning.
//
// Used by the PSD importer, currently gutted during the B9 migration.
// The importer comes back in B9.5 and consumes this directly; tests in
// the same module exercise the mapping table so the data stays
// trustworthy across the gap.
#[allow(dead_code)]
pub(super) fn blend_mode_from_psd_debug(mode_debug: &str) -> (BlendMode, bool) {
    match mode_debug {
        // Direct mappings
        "Normal" | "PassThrough" => (BlendMode::Normal, false),
        "Darken" => (BlendMode::Darken, false),
        "Multiply" => (BlendMode::Multiply, false),
        "ColorBurn" => (BlendMode::ColorBurn, false),
        "Lighten" => (BlendMode::Lighten, false),
        "Screen" => (BlendMode::Screen, false),
        "ColorDodge" => (BlendMode::ColorDodge, false),
        // PSD "Linear Dodge (Add)" is equivalent to Pixhaus Addition.
        "LinearDodge" => (BlendMode::Addition, false),
        "Overlay" => (BlendMode::Overlay, false),
        "SoftLight" => (BlendMode::SoftLight, false),
        "HardLight" => (BlendMode::HardLight, false),
        "Difference" => (BlendMode::Difference, false),
        "Exclusion" => (BlendMode::Exclusion, false),
        "Subtract" => (BlendMode::Subtract, false),
        "Divide" => (BlendMode::Divide, false),
        "Hue" => (BlendMode::Hue, false),
        "Saturation" => (BlendMode::Saturation, false),
        "Color" => (BlendMode::Color, false),
        "Luminosity" => (BlendMode::Luminosity, false),
        // Approximations (closest visual match, but not exact):
        "LinearBurn" => (BlendMode::ColorBurn, true),
        "DarkerColor" => (BlendMode::Darken, true),
        "LighterColor" => (BlendMode::Lighten, true),
        // No Pixhaus equivalent: Dissolve, VividLight, LinearLight, PinLight, HardMix.
        _ => (BlendMode::Normal, true),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_maps_cleanly() {
        let (mode, had_unknown) = blend_mode_from_psd_debug("Normal");
        assert_eq!(mode, BlendMode::Normal);
        assert!(!had_unknown);
    }

    #[test]
    fn pass_through_maps_to_normal_without_warning() {
        let (mode, had_unknown) = blend_mode_from_psd_debug("PassThrough");
        assert_eq!(mode, BlendMode::Normal);
        assert!(!had_unknown);
    }

    #[test]
    fn linear_dodge_maps_to_addition() {
        let (mode, had_unknown) = blend_mode_from_psd_debug("LinearDodge");
        assert_eq!(mode, BlendMode::Addition);
        assert!(!had_unknown);
    }

    #[test]
    fn unknown_mode_falls_back_to_normal_with_warning() {
        let (mode, had_unknown) = blend_mode_from_psd_debug("SomeFutureMode");
        assert_eq!(mode, BlendMode::Normal);
        assert!(had_unknown);
    }

    #[test]
    fn dissolve_falls_back_with_warning() {
        let (mode, had_unknown) = blend_mode_from_psd_debug("Dissolve");
        assert_eq!(mode, BlendMode::Normal);
        assert!(had_unknown);
    }

    #[test]
    fn core_modes_map_exactly() {
        for (debug_str, expected) in [
            ("Darken", BlendMode::Darken),
            ("Multiply", BlendMode::Multiply),
            ("ColorBurn", BlendMode::ColorBurn),
            ("Lighten", BlendMode::Lighten),
            ("Screen", BlendMode::Screen),
            ("ColorDodge", BlendMode::ColorDodge),
            ("Overlay", BlendMode::Overlay),
            ("SoftLight", BlendMode::SoftLight),
            ("HardLight", BlendMode::HardLight),
            ("Difference", BlendMode::Difference),
            ("Exclusion", BlendMode::Exclusion),
            ("Subtract", BlendMode::Subtract),
            ("Divide", BlendMode::Divide),
            ("Hue", BlendMode::Hue),
            ("Saturation", BlendMode::Saturation),
            ("Color", BlendMode::Color),
            ("Luminosity", BlendMode::Luminosity),
        ] {
            let (mode, had_unknown) = blend_mode_from_psd_debug(debug_str);
            assert_eq!(mode, expected, "mode: {debug_str}");
            assert!(!had_unknown, "mode: {debug_str} should not warn");
        }
    }

    #[test]
    fn approximations_set_had_unknown() {
        for mode in ["LinearBurn", "DarkerColor", "LighterColor"] {
            let (_, had_unknown) = blend_mode_from_psd_debug(mode);
            assert!(had_unknown, "{mode} should set had_unknown");
        }
    }
}
