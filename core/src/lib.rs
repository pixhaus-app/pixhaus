//! Pixhaus core — vertical-slice subset.
//!
//! Minimal port from the main checkout's `core` crate: the project model
//! (sprite, layer, cel, frame, animation, frame tag, library), the
//! `PixelBuffer` and compositor, and frame normalization. No drawing,
//! undo, or selection — the slice displays and plays sprites, it does not
//! edit pixels.

#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::missing_panics_doc,
        // The workspace clippy.toml disallows std `unwrap`/`expect` via the
        // `disallowed_methods` lint; tests get the same exemption as
        // `unwrap_used`/`expect_used` so they may panic on failure rather
        // than thread `Result` through an assert.
        clippy::disallowed_methods,
        clippy::cast_lossless,
        clippy::cast_possible_truncation,
    )
)]

pub mod canvas;
pub mod color;
pub mod project;
pub mod transforms;

/// Returns the crate name. Stable, public marker that downstream crates
/// can use as a sanity check after a workspace-wide upgrade.
#[must_use]
pub fn crate_name() -> &'static str {
    "pixhaus-core"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_name_is_stable() {
        assert_eq!(crate_name(), "pixhaus-core");
    }
}
