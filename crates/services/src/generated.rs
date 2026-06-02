//! The generation result type and its provenance metadata.

use serde::{Deserialize, Serialize};

/// What a generation job returns: decoded RGBA8 plus full provenance (bible 14.7).
///
/// The generation module turns this into a `core::commands::ApplyGeneratedAsset`
/// (plain RGBA + size), so `core` never depends on this type.
#[derive(Clone, Debug)]
pub struct GeneratedAsset {
    /// Image width in pixels.
    pub width: u32,
    /// Image height in pixels.
    pub height: u32,
    /// Bytes per row (`>= width * 4`).
    pub stride: u32,
    /// Decoded straight-alpha RGBA8 bytes, ready for `PixelBuffer::from_rgba8`.
    pub rgba: Vec<u8>,
    /// How this asset was produced, for reproducibility and history.
    pub provenance: GenerationProvenance,
}

/// Reproducibility metadata for a generated asset (bible 14.7). Stored as data,
/// never as i18n keys.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GenerationProvenance {
    /// The prompt the user submitted (content, not a key).
    pub prompt: String,
    /// The seed the provider used.
    pub seed: u64,
    /// The id of the provider that produced the asset.
    pub provider_id: String,
    /// The provider-reported model name.
    pub model: String,
    /// Unix milliseconds when the asset was produced (0 if unavailable).
    pub created_unix_ms: u64,
}
