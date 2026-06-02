//! The generation result types and their provenance metadata.

use serde::{Deserialize, Serialize};

use pixhaus_core::LoopMode;

/// A single still generated image: decoded RGBA8 plus full provenance (bible 14.7).
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

/// One decoded animation frame: RGBA8 plus dimensions, ready for the
/// `ApplyGeneratedAnimation` command.
#[derive(Clone, Debug)]
pub struct GeneratedFrame {
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
    /// Bytes per row (`>= width * 4`).
    pub stride: u32,
    /// Decoded straight-alpha RGBA8 bytes.
    pub rgba: Vec<u8>,
}

/// What an animation generation job returns: the sliced, keyed frames in playback
/// order plus timing and full provenance. The generation module turns this into a
/// `core::commands::ApplyGeneratedAnimation`, so `core` never depends on this type.
#[derive(Clone, Debug)]
pub struct GeneratedAnimation {
    /// Frames in playback order.
    pub frames: Vec<GeneratedFrame>,
    /// Suggested clip name (e.g. `"idle"`) — content, not an i18n key.
    pub clip_name: String,
    /// Playback rate in frames per second.
    pub fps: u16,
    /// How the resulting clip should loop.
    pub loop_mode: LoopMode,
    /// How this animation was produced, for reproducibility and history.
    pub provenance: GenerationProvenance,
}

/// A completed generation result: either a single still anchor or an animation.
///
/// One enum lets the [`ResultStore`](crate::ResultStore) hold both kinds in one
/// ordered tray and a job deposit whichever its provider produced.
#[derive(Clone, Debug)]
pub enum GeneratedResult {
    /// A single still image (an anchor or any text-to-sprite result).
    Sprite(GeneratedAsset),
    /// A multi-frame animation.
    Animation(GeneratedAnimation),
}

impl GeneratedResult {
    /// The provenance shared by both result kinds.
    pub fn provenance(&self) -> &GenerationProvenance {
        match self {
            Self::Sprite(asset) => &asset.provenance,
            Self::Animation(animation) => &animation.provenance,
        }
    }

    /// A lightweight summary of this result's kind, for the UI's read-only mirror.
    pub fn kind(&self) -> ResultKind {
        match self {
            Self::Sprite(_) => ResultKind::Sprite,
            Self::Animation(animation) => ResultKind::Animation {
                frames: u32::try_from(animation.frames.len()).unwrap_or(u32::MAX),
            },
        }
    }
}

/// A summary of a result's kind: a still sprite, or an animation with its frame
/// count. Cheap to copy; the shell mirrors it so panels gate buttons and draw a
/// frame-count badge without reaching into the [`ResultStore`](crate::ResultStore).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ResultKind {
    /// A single still image.
    Sprite,
    /// An animation with `frames` frames.
    Animation {
        /// The number of frames in the animation.
        frames: u32,
    },
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
