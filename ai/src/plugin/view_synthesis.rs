//! View-synthesis sub-trait for the Extend verb (S25).
//!
//! Backends that advertise [`super::descriptor::BackendCapabilities::VIEW_SYNTHESIS`]
//! implement [`ViewSynthesisBackend`] and override
//! [`super::backend::InferenceBackend::as_view_synthesis`] to return
//! `Some(self)`. The Extend verb calls that accessor to recover the
//! concrete calling interface without hard-coding specific backend types.
//!
//! # Adding view synthesis to a backend
//!
//! ```ignore
//! use pixhaus_ai::plugin::{BackendCapabilities, DirectionalViewRequest, PixelData,
//!                          ViewSynthesisBackend};
//! use pixhaus_ai::plugin::error::Result;
//! use tokio_util::sync::CancellationToken;
//!
//! struct MyBackend;
//!
//! impl InferenceBackend for MyBackend {
//!     fn capabilities(&self) -> BackendCapabilities {
//!         BackendCapabilities::IMAGE_GENERATION.union(BackendCapabilities::VIEW_SYNTHESIS)
//!     }
//!     fn as_view_synthesis(&self) -> Option<&dyn ViewSynthesisBackend> {
//!         Some(self)
//!     }
//!     // … other required methods
//! }
//!
//! #[async_trait::async_trait]
//! impl ViewSynthesisBackend for MyBackend {
//!     async fn generate_directional_view(
//!         &self,
//!         request: DirectionalViewRequest,
//!         _cancel: CancellationToken,
//!     ) -> Result<PixelData> {
//!         // … call inference API, return RGBA8 pixels
//!     }
//! }
//! ```

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use super::context::PixelData;
use super::error::Result;

/// One facing direction in a pixel-art sprite.
///
/// Named from the camera's perspective: `South` = sprite faces down
/// (toward the viewer), `North` = facing up (away). The ordering
/// matches the conventional RPG sprite sheet row ordering — row 0 is
/// typically south.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    /// Front-facing (down, toward viewer). The most common source
    /// direction in RPG sprite sheets.
    South,
    /// Diagonal: south + west.
    SouthWest,
    /// Side-facing left.
    West,
    /// Diagonal: north + west.
    NorthWest,
    /// Back-facing (up, away from viewer).
    North,
    /// Diagonal: north + east.
    NorthEast,
    /// Side-facing right.
    East,
    /// Diagonal: south + east.
    SouthEast,
}

impl Direction {
    /// Human-readable label shown in the layer name and progress events.
    #[must_use]
    pub fn display_name(self) -> &'static str {
        match self {
            Self::South => "South",
            Self::SouthWest => "South-West",
            Self::West => "West",
            Self::NorthWest => "North-West",
            Self::North => "North",
            Self::NorthEast => "North-East",
            Self::East => "East",
            Self::SouthEast => "South-East",
        }
    }

    /// Standard four-direction set: South, West, North, East.
    #[must_use]
    pub fn four() -> Vec<Self> {
        vec![Self::South, Self::West, Self::North, Self::East]
    }

    /// Standard eight-direction set: S, SW, W, NW, N, NE, E, SE.
    #[must_use]
    pub fn eight() -> Vec<Self> {
        vec![
            Self::South,
            Self::SouthWest,
            Self::West,
            Self::NorthWest,
            Self::North,
            Self::NorthEast,
            Self::East,
            Self::SouthEast,
        ]
    }
}

impl std::fmt::Display for Direction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.display_name())
    }
}

/// Parameters for a single novel-view synthesis call.
#[derive(Clone, Debug)]
pub struct DirectionalViewRequest {
    /// Source sprite pixels. Must be well-formed RGBA8 data.
    pub source: PixelData,
    /// Which direction the source sprite is facing.
    pub source_direction: Direction,
    /// Which direction to synthesise.
    pub target_direction: Direction,
    /// Style fidelity: 0.0 = full creative latitude, 1.0 = strict
    /// adherence to the source's style and palette.
    pub style_intensity: f32,
}

/// Backend capability for novel-view synthesis.
///
/// Implemented by backends that can generate a sprite viewed from a
/// different camera angle than the source frame. The primary users are
/// the Replicate adapter (TripoSR-class geometry estimation) and the
/// Stability adapter (style-conditioned image generation).
///
/// Backends access this interface by:
///
/// 1. Returning `BackendCapabilities::IMAGE_GENERATION.union(VIEW_SYNTHESIS)`
///    from [`super::backend::InferenceBackend::capabilities`].
/// 2. Implementing `generate_directional_view`.
/// 3. Overriding [`super::backend::InferenceBackend::as_view_synthesis`]
///    to return `Some(self)`.
///
/// The Extend verb recovers the interface via `ctx.backend.as_ref()
/// .and_then(|b| b.as_view_synthesis())`.
#[async_trait]
pub trait ViewSynthesisBackend: Send + Sync + 'static {
    /// Generate a novel view of the sprite at a different facing
    /// direction.
    ///
    /// Returns RGBA8 pixel data at the same canvas dimensions as
    /// `request.source.size()`, rendered in the sprite's style and
    /// facing `request.target_direction`.
    ///
    /// Implementations must observe `cancel` between expensive
    /// operations (network polls, diffusion steps) and return
    /// [`super::error::VerbError::Cancelled`] promptly when it fires.
    ///
    /// # Errors
    ///
    /// Returns [`super::error::VerbError::Backend`] on inference
    /// failure. Returns [`super::error::VerbError::Cancelled`] when
    /// the cancellation token fires.
    async fn generate_directional_view(
        &self,
        request: DirectionalViewRequest,
        cancel: CancellationToken,
    ) -> Result<PixelData>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direction_display_names_are_non_empty() {
        for dir in [
            Direction::South,
            Direction::SouthWest,
            Direction::West,
            Direction::NorthWest,
            Direction::North,
            Direction::NorthEast,
            Direction::East,
            Direction::SouthEast,
        ] {
            assert!(!dir.display_name().is_empty(), "{dir:?} has empty name");
            assert_eq!(format!("{dir}"), dir.display_name());
        }
    }

    #[test]
    fn four_direction_set_has_cardinal_directions() {
        let four = Direction::four();
        assert_eq!(four.len(), 4);
        assert!(four.contains(&Direction::South));
        assert!(four.contains(&Direction::West));
        assert!(four.contains(&Direction::North));
        assert!(four.contains(&Direction::East));
        assert!(!four.contains(&Direction::SouthWest));
    }

    #[test]
    fn eight_direction_set_contains_all_directions() {
        let eight = Direction::eight();
        assert_eq!(eight.len(), 8);
        for dir in [
            Direction::South,
            Direction::SouthWest,
            Direction::West,
            Direction::NorthWest,
            Direction::North,
            Direction::NorthEast,
            Direction::East,
            Direction::SouthEast,
        ] {
            assert!(eight.contains(&dir), "{dir:?} missing from 8-dir set");
        }
    }

    #[test]
    fn direction_serializes_as_snake_case() {
        let json = serde_json::to_string(&Direction::NorthEast).unwrap();
        assert_eq!(json, "\"north_east\"");
        let back: Direction = serde_json::from_str(&json).unwrap();
        assert_eq!(back, Direction::NorthEast);
    }
}
