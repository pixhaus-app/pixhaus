//! Pixel buffer storage and the layer compositor.
//!
//! This is the rendering substrate the rest of the editor builds on.
//! Three concerns live here:
//!
//! - [`PixelBuffer`] — the universal RGBA pixel buffer with explicit
//!   row stride, the type every blend mode and tool consumes.
//! - [`IndexedBuffer`] — palette-indexed storage for indexed-mode
//!   sprites; resolves to [`PixelBuffer`] on demand.
//! - The blend mode functions in [`mod@blend`] and the multi-layer
//!   compositor in [`mod@composite`].
//!
//! The blend math matches Aseprite's `src/doc/blend_funcs.cpp`
//! byte-for-byte so files round-trip without altering visual output.
//! The compositor is parallel: row work fans out across rayon's
//! global thread pool.

pub mod blend;
pub mod buffer;
pub mod composite;
pub mod error;

pub use blend::{blend, blend_normal, mul_un8, premix};
pub use buffer::{IndexedBuffer, PixelBuffer, RGBA_BYTES_PER_PIXEL};
pub use composite::{LayerInput, composite_layers, composite_onto};
pub use error::{Error, Result};
