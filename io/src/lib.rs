//! Pixhaus I/O: read and write the formats Pixhaus speaks.
//!
//! The native `.pixhaus` format lands in B3. Aseprite read/write follows the
//! compatibility spec in B7. PNG sequences, sprite sheets, PSD import, and
//! Tiled `.tmx` export each get their own module under this crate.

#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::missing_panics_doc
    )
)]

/// Returns the crate name. Placeholder until file format support lands.
#[must_use]
pub fn crate_name() -> &'static str {
    "pixhaus-io"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_name_is_stable() {
        assert_eq!(crate_name(), "pixhaus-io");
    }
}
