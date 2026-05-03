//! Editor selection state.
//!
//! Selection is editor state, not document state — but it persists
//! across save/load so the user returns to the same selection on
//! reopen. Pixel-perfect mask data lives in a separate buffer (the
//! pixel-buffer subsystem owns it); this module records only the
//! bounding box and the kind.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::geometry::Rect;
use super::id::{LayerId, PixelBufferId};

/// The current selection on the canvas.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SelectionState {
    /// What kind of region is selected. `None` means nothing is
    /// selected; the canvas treats every pixel as eligible.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub region: Option<SelectionRegion>,
    /// Layer the selection is anchored to. Some operations (e.g.
    /// "fill selection") require a target layer.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub anchor_layer: Option<LayerId>,
}

impl SelectionState {
    /// Returns `true` if no region is selected.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.region.is_none()
    }
}

/// The shape of a selection.
///
/// A `Rect` selection is rectangular and exact. A `Mask` selection is
/// arbitrary-shape, with the actual mask bytes held in a pixel buffer
/// referenced by [`PixelBufferId`].
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(export)]
pub enum SelectionRegion {
    /// Axis-aligned rectangular selection.
    Rect {
        /// Selection bounds in canvas coordinates.
        bounds: Rect,
    },
    /// Arbitrary-shape selection. `bounds` is the minimum enclosing
    /// rectangle of the mask; `mask` is the buffer of per-pixel
    /// inclusion flags.
    Mask {
        /// Bounding rectangle of the mask in canvas coordinates.
        bounds: Rect,
        /// Handle to the mask pixel buffer.
        mask: PixelBufferId,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_selection_round_trip() {
        let s = SelectionState::default();
        assert!(s.is_empty());
        let bytes = rmp_serde::to_vec_named(&s).unwrap();
        let back: SelectionState = rmp_serde::from_slice(&bytes).unwrap();
        assert_eq!(s, back);
    }

    #[test]
    fn rect_selection_round_trip() {
        let s = SelectionState {
            region: Some(SelectionRegion::Rect {
                bounds: Rect::from_xywh(2, 2, 8, 8),
            }),
            anchor_layer: Some(LayerId::new(1)),
        };
        let bytes = rmp_serde::to_vec_named(&s).unwrap();
        let back: SelectionState = rmp_serde::from_slice(&bytes).unwrap();
        assert_eq!(s, back);
    }

    #[test]
    fn mask_selection_round_trip() {
        let s = SelectionState {
            region: Some(SelectionRegion::Mask {
                bounds: Rect::from_xywh(0, 0, 16, 16),
                mask: PixelBufferId::new(7),
            }),
            anchor_layer: None,
        };
        let bytes = rmp_serde::to_vec_named(&s).unwrap();
        let back: SelectionState = rmp_serde::from_slice(&bytes).unwrap();
        assert_eq!(s, back);
    }
}
