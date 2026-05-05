//! Pixhaus core: pixel buffers, layers, frames, blend modes, palettes, undo.
//!
//! This crate is the data model for everything that can be edited: a project, its
//! sprites, layers, frames, palettes, tilesets, and selection state. It owns no I/O
//! and no UI; downstream crates compose it into a working editor.
//!
//! Features land per the streams in `docs/planning/work/streams.md` (S01 onwards).
//! The bedrock data model is specified in `docs/planning/work/bedrock.md` (B2) and
//! lives in [`project`].

#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::missing_panics_doc,
        // The workspace-wide clippy.toml disallows std `unwrap`/`expect`
        // via the `disallowed_methods` lint; tests get the same exemption
        // as `unwrap_used`/`expect_used` so they may panic on failure
        // rather than thread `Result` through an assert.
        clippy::disallowed_methods,
        clippy::cast_lossless,
        clippy::cast_possible_truncation,
    )
)]
// The `ts-rs` derive macro emits an internal `_ts_rs_export()` test that
// calls `.expect()` on `std::fs` results. Since the generated code is
// always behind `#[cfg(test)]`, allowing the lint at crate root only
// affects test compilation.
#![cfg_attr(test, allow(clippy::missing_const_for_fn))]

pub mod canvas;
pub mod color;
pub mod fixtures;
pub mod project;
pub mod selection;
pub mod tilemap;
pub mod undo;

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
