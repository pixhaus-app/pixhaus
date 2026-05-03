//! Named color palettes and their entries.
//!
//! A sprite owns one or more palettes. The active palette resolves
//! indexed-mode pixels and drives the color picker. RGBA-mode sprites
//! still benefit from palettes as named-color libraries; the editor
//! does not enforce that placed pixels match the palette.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::color::Rgba;
use super::id::PaletteId;
use super::user_data::UserData;

/// A single entry within a [`Palette`].
///
/// The optional `name` lets artists tag colors (e.g. "skin",
/// "outline") for documentation and palette-swap workflows.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PaletteEntry {
    /// The color value.
    pub color: Rgba,
    /// Optional human-readable name for the swatch.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub name: Option<String>,
}

impl PaletteEntry {
    /// Constructs an unnamed entry from a color.
    #[must_use]
    pub const fn new(color: Rgba) -> Self {
        Self { color, name: None }
    }
}

/// An ordered list of palette entries.
///
/// Index `0` is treated as the transparent index in indexed-mode
/// sprites, mirroring Aseprite's convention. The `colors` vector may
/// be empty; downstream code should fall back to an editor default
/// palette when it is.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Palette {
    /// Stable identifier.
    pub id: PaletteId,
    /// Display name in the palette panel.
    pub name: String,
    /// Ordered swatches. Position is the palette index.
    pub colors: Vec<PaletteEntry>,
    /// Free-form user metadata.
    #[serde(skip_serializing_if = "UserData::is_empty", default)]
    pub user_data: UserData,
}

impl Palette {
    /// Constructs a palette with the given colors, all unnamed.
    #[must_use]
    pub fn from_colors(id: PaletteId, name: impl Into<String>, colors: Vec<Rgba>) -> Self {
        Self {
            id,
            name: name.into(),
            colors: colors.into_iter().map(PaletteEntry::new).collect(),
            user_data: UserData::default(),
        }
    }

    /// Returns the color at `index`, or `None` if out of range.
    #[must_use]
    pub fn color_at(&self, index: usize) -> Option<Rgba> {
        self.colors.get(index).map(|e| e.color)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_colors_builds_unnamed_entries() {
        let p = Palette::from_colors(
            PaletteId::new(1),
            "default",
            vec![Rgba::transparent(), Rgba::opaque(255, 0, 0)],
        );
        assert_eq!(p.colors.len(), 2);
        assert!(p.colors[0].name.is_none());
        assert_eq!(p.color_at(1), Some(Rgba::opaque(255, 0, 0)));
        assert_eq!(p.color_at(99), None);
    }

    #[test]
    fn palette_round_trip() {
        let p = Palette {
            id: PaletteId::new(1),
            name: "ramp".into(),
            colors: vec![
                PaletteEntry::new(Rgba::opaque(10, 10, 10)),
                PaletteEntry {
                    color: Rgba::opaque(20, 20, 20),
                    name: Some("shadow".into()),
                },
            ],
            user_data: UserData::default(),
        };
        let bytes = rmp_serde::to_vec_named(&p).unwrap();
        let back: Palette = rmp_serde::from_slice(&bytes).unwrap();
        assert_eq!(p, back);
    }

    #[test]
    fn empty_palette_round_trip() {
        let p = Palette {
            id: PaletteId::new(2),
            name: "empty".into(),
            colors: Vec::new(),
            user_data: UserData::default(),
        };
        let bytes = rmp_serde::to_vec_named(&p).unwrap();
        let back: Palette = rmp_serde::from_slice(&bytes).unwrap();
        assert_eq!(p, back);
    }
}
