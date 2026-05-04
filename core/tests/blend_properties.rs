//! Property tests for the blend mode math.
//!
//! These run against the public surface of `pixhaus_core::canvas` so
//! they double as a smoke test that the API stays usable from outside
//! the crate. The properties pin invariants the editor relies on:
//!
//! - Identity: blending a transparent source leaves the backdrop alone.
//! - Idempotence at zero opacity.
//! - Algebraic guarantees per blend mode (commutativity for symmetric
//!   modes, monotonicity for darken/lighten, etc.).
//!
//! Property failures get shrunk to a minimal counterexample, and the
//! `proptest-regressions/` folder commits each minimised seed so it
//! re-runs first on the next test pass.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::missing_panics_doc,
    clippy::panic,
    clippy::disallowed_methods
)]

use pixhaus_core::canvas::blend::{
    blend, blend_normal, channel_darken, channel_difference, channel_exclusion, channel_lighten,
    channel_multiply, channel_screen,
};
use pixhaus_core::project::{BlendMode, Rgba};
use proptest::prelude::*;

fn rgba_strategy() -> impl Strategy<Value = Rgba> {
    (0u8..=255, 0u8..=255, 0u8..=255, 0u8..=255).prop_map(|(r, g, b, a)| Rgba::new(r, g, b, a))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn transparent_source_is_identity(
        // dst.a > 0 is required: Aseprite (and our compositor) preserve
        // the source's RGB through a transparent-backdrop case so a
        // fade-in animation keeps its colour metadata. The "identity"
        // invariant we care about for the editor is only that an
        // already-painted pixel survives a transparent stroke.
        rgb in (0u8..=255, 0u8..=255, 0u8..=255),
        a in 1u8..=255,
        opacity in 0u8..=255,
    ) {
        let dst = Rgba::new(rgb.0, rgb.1, rgb.2, a);
        let out = blend(BlendMode::Normal, Rgba::transparent(), dst, opacity);
        prop_assert_eq!(out, dst);
    }

    #[test]
    fn zero_opacity_keeps_dst(
        mode in prop_oneof![
            Just(BlendMode::Normal),
            Just(BlendMode::Multiply),
            Just(BlendMode::Screen),
            Just(BlendMode::Overlay),
            Just(BlendMode::Darken),
            Just(BlendMode::Lighten),
            Just(BlendMode::Difference),
            Just(BlendMode::Hue),
            Just(BlendMode::Color),
        ],
        src in rgba_strategy(),
        // Same caveat as `transparent_source_is_identity`: when the
        // backdrop is fully transparent, Aseprite returns the
        // modulated source RGB rather than the dst, so the property
        // only holds for visible backdrops.
        rgb in (0u8..=255, 0u8..=255, 0u8..=255),
        a in 1u8..=255,
    ) {
        let dst = Rgba::new(rgb.0, rgb.1, rgb.2, a);
        let out = blend(mode, src, dst, 0);
        prop_assert_eq!(out, dst);
    }

    #[test]
    fn opaque_src_full_opacity_replaces_dst_for_normal(
        src_rgb in (0u8..=255, 0u8..=255, 0u8..=255),
        dst in rgba_strategy(),
    ) {
        let src = Rgba::new(src_rgb.0, src_rgb.1, src_rgb.2, 255);
        let out = blend_normal(src, dst, 255);
        prop_assert_eq!(out, src);
    }

    #[test]
    fn multiply_is_commutative(b in 0u8..=255, s in 0u8..=255) {
        prop_assert_eq!(channel_multiply(b, s), channel_multiply(s, b));
    }

    #[test]
    fn screen_is_commutative(b in 0u8..=255, s in 0u8..=255) {
        prop_assert_eq!(channel_screen(b, s), channel_screen(s, b));
    }

    #[test]
    fn darken_lighten_pair(b in 0u8..=255, s in 0u8..=255) {
        prop_assert!(channel_darken(b, s) <= b.min(s));
        prop_assert!(channel_lighten(b, s) >= b.max(s));
        // Equality, in fact:
        prop_assert_eq!(channel_darken(b, s), b.min(s));
        prop_assert_eq!(channel_lighten(b, s), b.max(s));
    }

    #[test]
    fn difference_is_symmetric(b in 0u8..=255, s in 0u8..=255) {
        prop_assert_eq!(channel_difference(b, s), channel_difference(s, b));
    }

    #[test]
    fn exclusion_is_symmetric(b in 0u8..=255, s in 0u8..=255) {
        prop_assert_eq!(channel_exclusion(b, s), channel_exclusion(s, b));
    }

    #[test]
    fn multiply_against_white_is_identity(b in 0u8..=255) {
        prop_assert_eq!(channel_multiply(b, 255), b);
        prop_assert_eq!(channel_multiply(255, b), b);
    }

    #[test]
    fn multiply_against_black_is_zero(b in 0u8..=255) {
        prop_assert_eq!(channel_multiply(b, 0), 0);
        prop_assert_eq!(channel_multiply(0, b), 0);
    }

    #[test]
    fn screen_against_black_is_identity(b in 0u8..=255) {
        prop_assert_eq!(channel_screen(b, 0), b);
        prop_assert_eq!(channel_screen(0, b), b);
    }

    #[test]
    fn screen_against_white_is_white(b in 0u8..=255) {
        prop_assert_eq!(channel_screen(b, 255), 255);
        prop_assert_eq!(channel_screen(255, b), 255);
    }

    #[test]
    fn alpha_monotonic_in_normal_blend(
        a1 in 0u8..=255,
        a2 in 0u8..=255,
        rgb in (0u8..=255, 0u8..=255, 0u8..=255),
    ) {
        let dst = Rgba::opaque(0, 0, 0);
        let s1 = Rgba::new(rgb.0, rgb.1, rgb.2, a1);
        let s2 = Rgba::new(rgb.0, rgb.1, rgb.2, a2);
        let r1 = blend_normal(s1, dst, 255);
        let r2 = blend_normal(s2, dst, 255);
        if a1 <= a2 {
            // Higher source alpha drags backdrop toward source rgb.
            // Use luma comparison to avoid per-channel direction flips.
            let l1 = u32::from(r1.r) + u32::from(r1.g) + u32::from(r1.b);
            let l2 = u32::from(r2.r) + u32::from(r2.g) + u32::from(r2.b);
            let target = u32::from(rgb.0) + u32::from(rgb.1) + u32::from(rgb.2);
            let dist1 = l1.abs_diff(target);
            let dist2 = l2.abs_diff(target);
            prop_assert!(dist2 <= dist1, "r1={r1:?} r2={r2:?} target={rgb:?}");
        }
    }
}
