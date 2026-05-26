//! Cels: per-(layer, frame) drawn content.
//!
//! A `Cel` is the intersection of one layer and one frame. Most cels
//! carry pixel data via an opaque buffer handle; tilemap-layer cels
//! carry a [`TilemapData`] payload directly; linked cels share their
//! data with another frame on the same layer (Aseprite-style cel
//! linking).
//!
//! The pixel-buffer subsystem owns the bytes — this module only
//! references them by [`PixelBufferId`].

use serde::{Deserialize, Serialize};

use super::geometry::{IVec2, Size};
use super::id::{FrameIndex, LayerId, PixelBufferId};
use super::tilemap::TilemapData;
use super::user_data::UserData;

/// One cel in a sprite's `(layer, frame)` grid.
///
/// Cels are stored in a flat `Vec<Cel>` on the sprite. A cel is
/// addressed by the `(layer_id, frame_index)` pair; missing cells
/// (a layer with no content on a particular frame) simply have no
/// entry in the vector.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Cel {
    /// Layer this cel belongs to.
    pub layer_id: LayerId,
    /// Timeline position of the cel.
    pub frame_index: FrameIndex,
    /// Top-left placement of the cel content within the canvas.
    /// Cels may sit partially or fully off-canvas.
    pub position: IVec2,
    /// Per-cel opacity, multiplied with the layer's opacity at
    /// composition time.
    pub opacity: u8,
    /// Variant payload (raster pixel handle, tilemap grid, or link).
    pub data: CelData,
    /// Free-form user metadata.
    #[serde(skip_serializing_if = "UserData::is_empty", default)]
    pub user_data: UserData,
}

impl Cel {
    /// Constructs a raster cel at `(0, 0)` with full opacity.
    #[must_use]
    pub fn raster(
        layer_id: LayerId,
        frame_index: FrameIndex,
        buffer: PixelBufferId,
        size: Size,
    ) -> Self {
        Self {
            layer_id,
            frame_index,
            position: IVec2::zero(),
            opacity: 255,
            data: CelData::Raster { buffer, size },
            user_data: UserData::default(),
        }
    }
}

/// Variant payload for a [`Cel`].
///
/// `Raster` and `Tilemap` carry their own data; `Linked` borrows from
/// another frame on the same layer to keep file size down for cels
/// that don't change between frames.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CelData {
    /// Raster pixel data via an opaque buffer handle.
    Raster {
        /// Handle into the pixel-buffer registry.
        buffer: PixelBufferId,
        /// Width and height of the cel content. Stride is owned by
        /// the buffer subsystem and is not part of the data model.
        size: Size,
    },
    /// Tilemap grid embedded directly in the cel.
    Tilemap {
        /// The tile grid for this `(layer, frame)` cell.
        data: TilemapData,
    },
    /// A link to another cel on the same layer at `source_frame`.
    /// Compositors and exporters resolve the link at use; the data
    /// model never duplicates the underlying buffer.
    Linked {
        /// Frame whose cel on the same layer holds the source data.
        source_frame: FrameIndex,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::tilemap::{TileCell, TileFlags};

    #[test]
    fn raster_constructor_sets_defaults() {
        let c = Cel::raster(
            LayerId::new(1),
            FrameIndex::new(0),
            PixelBufferId::new(7),
            Size::new(32, 32),
        );
        assert_eq!(c.opacity, 255);
        assert_eq!(c.position, IVec2::zero());
        assert!(matches!(c.data, CelData::Raster { .. }));
    }

    #[test]
    fn raster_cel_round_trip() {
        let c = Cel::raster(
            LayerId::new(1),
            FrameIndex::new(2),
            PixelBufferId::new(99),
            Size::new(16, 16),
        );
        let json = serde_json::to_string(&c).unwrap();
        let back: Cel = serde_json::from_str(&json).unwrap();
        assert_eq!(c, back);
    }

    #[test]
    fn tilemap_cel_round_trip() {
        let mut data = TilemapData::empty(2, 2);
        data.cells[0] = TileCell {
            index: super::super::id::TileIndex::new(3),
            flags: TileFlags::FLIP_X,
        };
        let c = Cel {
            layer_id: LayerId::new(2),
            frame_index: FrameIndex::new(0),
            position: IVec2::new(10, 20),
            opacity: 128,
            data: CelData::Tilemap { data },
            user_data: UserData::default(),
        };
        let json = serde_json::to_string(&c).unwrap();
        let back: Cel = serde_json::from_str(&json).unwrap();
        assert_eq!(c, back);
    }

    #[test]
    fn linked_cel_round_trip() {
        let c = Cel {
            layer_id: LayerId::new(3),
            frame_index: FrameIndex::new(5),
            position: IVec2::zero(),
            opacity: 255,
            data: CelData::Linked {
                source_frame: FrameIndex::new(4),
            },
            user_data: UserData::default(),
        };
        let json = serde_json::to_string(&c).unwrap();
        let back: Cel = serde_json::from_str(&json).unwrap();
        assert_eq!(c, back);
    }
}
