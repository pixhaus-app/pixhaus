//! Photoshop `.psd` import for Pixhaus.
//!
//! This module reads PSD files and converts them into a [`ConvertedArchive`]
//! suitable for re-encoding through the native `.pixhaus` writer or for
//! in-editor use. Write support is intentionally out of scope: PSD is used
//! as a migration path from Photoshop sprite workflows, not as a round-trip
//! format.
//!
//! ## What is imported
//!
//! - Layer hierarchy (groups and raster layers) with correct parent-child
//!   relationships.
//! - Per-layer blend mode (mapped to the nearest Pixhaus equivalent).
//! - Per-layer opacity and visibility.
//! - Layer pixel data as 8-bit RGBA, positioned at the layer's canvas offset.
//!
//! ## What is skipped or approximated
//!
//! - **Raster (user-supplied) layer masks**: detected and reported as
//!   [`ConversionWarning::RasterMaskIgnored`]; the `psd` crate does not
//!   expose mask channel bytes through its public API, so the mask cannot be
//!   applied to the layer's alpha channel. Unmasked pixel data is preserved.
//! - **Clipping masks**: pixel data is included; the mask is not applied.
//!   Reported as [`ConversionWarning::ClippingMaskIgnored`].
//! - **Adjustment layers** and **smart objects**: imported as raster layers
//!   with the pixel data the `psd` crate can extract.
//! - **16-bit and 32-bit** per-channel modes: the `psd` crate converts to
//!   8-bit automatically; precision is lost. A [`ConversionWarning`] is
//!   emitted.
//! - **CMYK, Lab, Indexed, Multichannel**: rejected with [`Error::PsdParse`].
//! - **Layer effects** (drop shadow, glow, etc.): not preserved.
//! - **Text layers**: imported as rasterized pixel data (no editable text).
//! - **Vector masks**: no channel data in the `psd` crate; silently absent.
//!
//! ## Layer ordering note
//!
//! The `psd` crate exposes groups and pixel layers as separate collections.
//! Pixhaus emits all groups first (in visual order) then all pixel layers
//! (in visual order). The parent-child tree is always correct; only the
//! flat-list stacking order may differ from the PSD source when groups and
//! pixel layers are interleaved at the same hierarchy level.
//!
//! [`Error::PsdParse`]: crate::error::Error::PsdParse

pub mod archive;
pub(crate) mod spec;

mod read;

pub use archive::{ConversionWarning, ConvertedArchive};
pub use read::{decode, decode_from_file};
