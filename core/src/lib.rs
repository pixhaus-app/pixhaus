//! Pixhaus core: pixel buffers, layers, frames, blend modes, palettes, undo.
//!
//! This crate is the data model for everything that can be edited: a project, its
//! sprites, layers, frames, palettes, tilesets, and selection state. It owns no I/O
//! and no UI; downstream crates compose it into a working editor.
//!
//! Features land per the streams in `docs/planning/work/streams.md` (S01 onwards).
//! The bedrock data model is specified in `docs/planning/work/bedrock.md` (B2).

#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::missing_panics_doc
    )
)]

/// Returns the crate name. Placeholder until the data model lands (B2).
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
