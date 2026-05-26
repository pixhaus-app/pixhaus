//! Named color palettes and their entries.
//!
//! A sprite owns one or more palettes. The active palette resolves
//! indexed-mode pixels and drives the color picker. RGBA-mode sprites
//! still benefit from palettes as named-color libraries; the editor
//! does not enforce that placed pixels match the palette.
//!
//! Pages and animation adapted from `OpenToonz`
//! `toonz/sources/include/tpalette.h` under BSD-3-Clause. See
//! `THIRD_PARTY_NOTICES.md`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::color::Rgba;
use super::id::{FrameIndex, PaletteId, PalettePageId};
use super::user_data::UserData;

/// Returns the index of the entry whose RGB values are nearest to `target`,
/// ignoring alpha on both sides.
///
/// Uses squared Euclidean distance in sRGB space. Returns `None` if the
/// iterator yields no items. Inlined from the main checkout's
/// `color::ops::nearest_color_index`; the broader `color` ops module is not
/// part of the vertical slice.
fn nearest_color_index<I>(palette: I, target: Rgba) -> Option<usize>
where
    I: IntoIterator<Item = Rgba>,
{
    palette
        .into_iter()
        .enumerate()
        .map(|(i, c)| {
            let dr = i32::from(c.r) - i32::from(target.r);
            let dg = i32::from(c.g) - i32::from(target.g);
            let db = i32::from(c.b) - i32::from(target.b);
            (i, dr * dr + dg * dg + db * db)
        })
        .min_by_key(|&(_, dist)| dist)
        .map(|(i, _)| i)
}

/// A single entry within a [`Palette`].
///
/// The optional `name` lets artists tag colors (e.g. "skin",
/// "outline") for documentation and palette-swap workflows.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
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
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
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
    /// Named subset views over [`Self::colors`]. Empty by default;
    /// pre-S54 files load with no pages and re-save with the field
    /// omitted from the wire form.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pages: Vec<PalettePage>,
    /// Optional per-entry keyframed color changes over the project's
    /// frame range. `None` by default; pre-S54 files load with no
    /// animation and re-save with the field omitted from the wire form.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub animation: Option<PaletteAnimation>,
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
            pages: Vec::new(),
            animation: None,
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
            pages: self.pages.clone(),
            animation: self.animation.clone(),
        }
    }
}

/// Replacement palette state at a specific frame.
///
/// Aseprite stores palette changes as one full palette chunk per frame.
/// Pixhaus mirrors that by carrying a sparse list on the [`super::Sprite`]:
/// the base palette in `Sprite.palettes` covers frame 0, and each
/// override here replaces the active palette wholesale at its frame
/// boundary. Frames with no override inherit the previous frame's
/// palette state.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PaletteFrameOverride {
    /// Frame index this override takes effect on (0-based).
    pub frame: u32,
    /// Replacement entries — the full palette state at this frame.
    pub colors: Vec<PaletteEntry>,
}

/// A named subset view over a palette's entries.
///
/// Pages do not own colors; they reference entries by index into the
/// parent [`Palette::colors`] vector. The same entry can appear on more
/// than one page, and pages may be reordered freely.
///
/// Adapted from `OpenToonz` `toonz/sources/include/tpalette.h` (`class Page`)
/// under BSD-3-Clause. See `THIRD_PARTY_NOTICES.md`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PalettePage {
    /// Stable page identifier.
    pub id: PalettePageId,
    /// Display name in the palette panel.
    pub name: String,
    /// Indices into the parent palette's [`Palette::colors`] vector.
    /// Out-of-range indices are tolerated by serde but should be pruned
    /// by the editor on mutation.
    pub entry_indices: Vec<u32>,
}

impl PalettePage {
    /// Constructs a page with the given id and name and an empty entry
    /// list.
    #[must_use]
    pub fn new(id: PalettePageId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            entry_indices: Vec::new(),
        }
    }
}

/// Per-entry keyframed color changes over the project's frame range.
///
/// Outer key: palette entry index (into [`Palette::colors`]). Inner key:
/// timeline [`FrameIndex`]. Inner value: the color the entry resolves to
/// at that frame. Frames between keys resolve to the closest preceding
/// keyframe (step semantics, no easing). Frames before the first key for
/// a given entry fall through to the caller-supplied fallback.
///
/// Adapted from `OpenToonz` `toonz/sources/include/tpalette.h`
/// (`StyleAnimation`) under BSD-3-Clause. See `THIRD_PARTY_NOTICES.md`.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct PaletteAnimation {
    /// Per-entry keyframe table. See type-level docs for semantics.
    pub keyframes: BTreeMap<u32, BTreeMap<FrameIndex, Rgba>>,
}

impl PaletteAnimation {
    /// Inserts a keyframe color at `frame` for the entry at
    /// `entry_index`, replacing any existing keyframe at the same
    /// position.
    pub fn set(&mut self, entry_index: u32, frame: FrameIndex, color: Rgba) {
        self.keyframes.entry(entry_index).or_default().insert(frame, color);
    }

    /// Returns the color the entry at `entry_index` resolves to at
    /// `frame`. Falls back to `fallback` when the entry has no keyframe
    /// at or before `frame`.
    ///
    /// Step semantics: the result is the value at the closest keyframe
    /// whose frame index is `<= frame`. No interpolation between keys.
    #[must_use]
    pub fn resolve(&self, entry_index: u32, frame: FrameIndex, fallback: Rgba) -> Rgba {
        let Some(per_entry) = self.keyframes.get(&entry_index) else {
            return fallback;
        };
        per_entry.range(..=frame).next_back().map_or(fallback, |(_, color)| *color)
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
        let p = make_palette(vec![Rgba::transparent(), Rgba::opaque(255, 0, 0), Rgba::opaque(0, 255, 0)]);
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
    fn apply_cycle_negative_offset_rotates_backward() {
        let red = Rgba::opaque(255, 0, 0);
        let green = Rgba::opaque(0, 255, 0);
        let blue = Rgba::opaque(0, 0, 255);
        let p = make_palette(vec![red, green, blue]);
        let cycled = p.apply_cycle(0, 2, -1);
        assert_eq!(cycled.color_at(0), Some(green));
        assert_eq!(cycled.color_at(1), Some(blue));
        assert_eq!(cycled.color_at(2), Some(red));
    }

    #[test]
    fn apply_cycle_zero_offset_is_noop() {
        let p = make_palette(vec![Rgba::opaque(1, 0, 0), Rgba::opaque(2, 0, 0), Rgba::opaque(3, 0, 0)]);
        let cycled = p.apply_cycle(0, 2, 0);
        assert_eq!(cycled.colors, p.colors);
    }

    #[test]
    fn from_colors_builds_unnamed_entries() {
        let p = Palette::from_colors(PaletteId::new(1), "default", vec![Rgba::transparent(), Rgba::opaque(255, 0, 0)]);
        assert_eq!(p.colors.len(), 2);
        assert!(p.colors[0].name.is_none());
        assert_eq!(p.color_at(1), Some(Rgba::opaque(255, 0, 0)));
        assert_eq!(p.color_at(99), None);
    }

    #[test]
    fn palette_round_trip_via_json() {
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
            pages: Vec::new(),
            animation: None,
        };
        let json = serde_json::to_string(&p).unwrap();
        let back: Palette = serde_json::from_str(&json).unwrap();
        assert_eq!(p, back);
    }

    #[test]
    fn palette_page_new_starts_empty() {
        let page = PalettePage::new(PalettePageId::new(7), "warm");
        assert_eq!(page.id, PalettePageId::new(7));
        assert_eq!(page.name, "warm");
        assert!(page.entry_indices.is_empty());
    }

    #[test]
    fn animation_resolve_empty_returns_fallback() {
        let anim = PaletteAnimation::default();
        let fallback = Rgba::opaque(255, 0, 0);
        assert_eq!(anim.resolve(0, FrameIndex::new(5), fallback), fallback);
    }

    #[test]
    fn animation_resolve_between_keyframes_uses_step() {
        let mut anim = PaletteAnimation::default();
        let frame1 = Rgba::opaque(255, 0, 0);
        let frame5 = Rgba::opaque(0, 255, 0);
        anim.set(0, FrameIndex::new(1), frame1);
        anim.set(0, FrameIndex::new(5), frame5);
        let fallback = Rgba::opaque(0, 0, 0);
        assert_eq!(anim.resolve(0, FrameIndex::new(3), fallback), frame1);
        assert_eq!(anim.resolve(0, FrameIndex::new(5), fallback), frame5);
        assert_eq!(anim.resolve(0, FrameIndex::new(99), fallback), frame5);
    }

    #[test]
    fn animation_resolve_before_first_keyframe_returns_fallback() {
        let mut anim = PaletteAnimation::default();
        anim.set(0, FrameIndex::new(5), Rgba::opaque(0, 255, 0));
        let fallback = Rgba::opaque(0, 0, 0);
        assert_eq!(anim.resolve(0, FrameIndex::new(2), fallback), fallback);
    }
}
