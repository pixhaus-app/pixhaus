//! Pixhaus I/O: read and write the formats Pixhaus speaks.
//!
//! The native `.pixhaus` format is implemented in the [`pixhaus`] module.
//! PNG sprite sheet + JSON export (S10) lives in the [`png`] module.
//! Aseprite read/write, PSD import, and Tiled `.tmx` export land in
//! subsequent streams.

#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::missing_panics_doc,
        clippy::disallowed_methods,
        clippy::cast_lossless,
        clippy::cast_possible_truncation,
        clippy::uninlined_format_args,
    )
)]

pub mod animated;
pub mod aseprite;
pub mod palette;
pub mod pixhaus;
pub mod pixstyle;
pub mod png;
pub mod psd;
pub mod tiled;

pub mod error;
pub use error::{Error, Result};
