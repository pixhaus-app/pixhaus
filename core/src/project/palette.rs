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
use crate::color::ops::nearest_color_index;

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

    /// Returns the index of the palette entry whose RGB value is nearest to
    /// `target`, ignoring alpha on both sides.
    ///
    /// Returns `None` if the palette is empty. Index `0` is the transparent
    /// index in indexed-mode sprites. Iterates `self.colors` directly — no
    /// per-call `Vec` allocation, since this is a hot path for color picking
    /// and quantization.
    #[must_use]
    pub fn nearest_index(&self, target: Rgba) -> Option<usize> {
        nearest_color_index(self.colors.iter().map(|e| e.color), target)
    }

    /// Returns a copy of this palette with the entries in `[first..=last]`
    /// shifted by `offset` positions (wrapping within the range).
    ///
    /// `offset > 0` rotates entries toward higher indices (forward);
    /// `offset < 0` rotates toward lower indices (backward); `offset == 0`
    /// is a no-op. Entries outside `[first..=last]` are unchanged.
    /// `first` and `last` are palette indices (i.e. `u8` values cast to
    /// `usize`). If the range is invalid (`first > last`) or out of
    /// bounds, the palette is returned unchanged.
    #[must_use]
    pub fn apply_cycle(&self, first: u8, last: u8, offset: i32) -> Self {
        let mut new_entries = self.colors.clone();
        let first = first as usize;
        let last = last as usize;
        if first <= last && last < new_entries.len() && offset != 0 {
            let range_len = last - first + 1;
            // palette len is always < isize::MAX
            #[allow(clippy::cast_possible_wrap)]
            let offset = (offset as isize).rem_euclid(range_len as isize) as usize;
            new_entries[first..=last].rotate_right(offset);
        }
        Self {
            id: self.id,
            name: self.name.clone(),
            colors: new_entries,
            user_data: self.user_data.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_palette(colors: Vec<Rgba>) -> Palette {
        Palette::from_colors(PaletteId::new(1), "test", colors)
    }

    #[test]
    fn nearest_index_exact_match() {
        let p = make_palette(vec![
            Rgba::transparent(),
            Rgba::opaque(255, 0, 0),
            Rgba::opaque(0, 255, 0),
        ]);
        assert_eq!(p.nearest_index(Rgba::opaque(255, 0, 0)), Some(1));
        assert_eq!(p.nearest_index(Rgba::opaque(0, 255, 0)), Some(2));
    }

    #[test]
    fn nearest_index_empty_palette_returns_none() {
        let p = make_palette(vec![]);
        assert_eq!(p.nearest_index(Rgba::opaque(255, 0, 0)), None);
    }

    #[test]
    fn apply_cycle_shifts_range_forward() {
        let red = Rgba::opaque(255, 0, 0);
        let green = Rgba::opaque(0, 255, 0);
        let blue = Rgba::opaque(0, 0, 255);
        let p = make_palette(vec![red, green, blue]);
        let cycled = p.apply_cycle(0, 2, 1);
        assert_eq!(cycled.color_at(0), Some(blue));
        assert_eq!(cycled.color_at(1), Some(red));
        assert_eq!(cycled.color_at(2), Some(green));
    }

    #[test]
    fn apply_cycle_preserves_names() {
        let mut p = make_palette(vec![Rgba::opaque(255, 0, 0), Rgba::opaque(0, 255, 0)]);
        p.colors[0].name = Some("red".into());
        p.colors[1].name = Some("green".into());
        let cycled = p.apply_cycle(0, 1, 1);
        // Colors shifted but names should follow their entries
        assert_eq!(cycled.colors[0].name, Some("green".into()));
        assert_eq!(cycled.colors[1].name, Some("red".into()));
    }

    #[test]
    fn apply_cycle_negative_offset_rotates_backward() {
        // Doc clarification (thread 9): negative offsets must work as
        // backward rotation, not no-op or panic.
        let red = Rgba::opaque(255, 0, 0);
        let green = Rgba::opaque(0, 255, 0);
        let blue = Rgba::opaque(0, 0, 255);
        let p = make_palette(vec![red, green, blue]);
        let cycled = p.apply_cycle(0, 2, -1);
        // Backward by one: [red, green, blue] -> [green, blue, red]
        assert_eq!(cycled.color_at(0), Some(green));
        assert_eq!(cycled.color_at(1), Some(blue));
        assert_eq!(cycled.color_at(2), Some(red));
    }

    #[test]
    fn apply_cycle_zero_offset_is_noop() {
        let p = make_palette(vec![
            Rgba::opaque(1, 0, 0),
            Rgba::opaque(2, 0, 0),
            Rgba::opaque(3, 0, 0),
        ]);
        let cycled = p.apply_cycle(0, 2, 0);
        assert_eq!(cycled.colors, p.colors);
    }

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
