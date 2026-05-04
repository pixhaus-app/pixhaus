//! Pixhaus I/O: read and write the formats Pixhaus speaks.
//!
//! The native `.pixhaus` format is implemented in the [`pixhaus`] module.
//! Aseprite read/write follows in B7; PNG sequences, sprite sheets, PSD
//! import, and Tiled `.tmx` export each get their own module as the
//! corresponding streams land.

#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::missing_panics_doc
    )
)]

pub mod pixhaus;

pub mod error;
pub use error::{Error, Result};
