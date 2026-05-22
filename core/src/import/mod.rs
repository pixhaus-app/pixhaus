//! Image-import helpers that operate on decoded pixel buffers.
//!
//! Decoding bytes into a [`crate::canvas::PixelBuffer`] is the `io` crate's
//! job; this module holds the format-agnostic analysis that runs afterwards,
//! such as detecting sprite-sheet frame boundaries from transparency.

pub mod smart_slice;

pub use smart_slice::{SmartSliceOpts, detect_frames};
