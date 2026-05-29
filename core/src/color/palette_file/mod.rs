//! Palette file format I/O for the native editor.
//!
//! A standalone port of the parsers and encoders from the main checkout's
//! `io` crate, depending only on [`crate::project::color::Rgba`] and `std`.
//! No image decode, no network — `.aco`/`.pal`/`.gpl`/`.hex` are text or
//! binary palette lists. The Lospec HTTP browser is not ported here.
//!
//! | Module | Format | Read | Write |
//! |--------|--------|------|-------|
//! | [`gpl`] | GIMP Palette (`.gpl`) | yes | yes |
//! | [`pal`] | Microsoft RIFF `.pal` | yes | yes |
//! | [`pal`] | JASC `.pal` | yes | yes |
//! | [`aco`] | Photoshop Color Swatch (`.aco`) | yes | no |
//! | [`hex`] | Lospec `.hex` | yes | yes |

pub mod aco;
pub mod gpl;
pub mod hex;
pub mod pal;

/// Errors returned by the palette-file parsers and encoders.
///
/// A leaf error — never `Box<dyn Error>`, never `anyhow` (core is a
/// library crate). Maps the main checkout io crate's palette variants:
/// `InvalidPalette` to [`Self::Invalid`], `UnsupportedColorSpace` to
/// [`Self::UnsupportedColorSpace`], `PaletteTooLarge` to [`Self::TooLarge`].
/// The `Http`/`LospecApi` variants are dropped because Lospec is not ported.
#[derive(Debug, thiserror::Error)]
pub enum PaletteFileError {
    /// A palette file is malformed or contains an unsupported variant.
    #[error("invalid palette file: {0}")]
    Invalid(String),

    /// A RIFF PAL or ACO file contains a color space the editor cannot
    /// convert.
    #[error("unsupported color space code {code}")]
    UnsupportedColorSpace {
        /// The raw color space code read from the file.
        code: u16,
    },

    /// A palette has more entries than the target format's count field can
    /// represent. RIFF PAL caps at `u16::MAX`; an oversize palette would
    /// silently truncate the count and emit a malformed file, so we reject
    /// up front.
    #[error("palette too large: {count} entries exceeds {max} for {format}")]
    TooLarge {
        /// Number of colors the caller tried to encode.
        count: usize,
        /// Maximum supported by the target format.
        max: usize,
        /// Format name, for the error message (e.g. `"RIFF PAL"`).
        format: &'static str,
    },
}

/// Result alias for the palette-file module.
pub type PaletteFileResult<T> = Result<T, PaletteFileError>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::color::Rgba;

    #[test]
    fn gpl_encode_parse_cross_module_round_trip() {
        // Cross-module round trip: encode through gpl, parse it back, and
        // assert the colors survive. Names attached to entries are dropped
        // by the gpl writer (it emits bare `R G B` lines), matching the
        // encoder's behaviour.
        let colors = vec![
            Rgba::opaque(12, 34, 56),
            Rgba::opaque(200, 100, 50),
            Rgba::opaque(0, 0, 0),
            Rgba::opaque(255, 255, 255),
        ];
        let encoded = gpl::encode("CrossModule", &colors);
        let (name, decoded) = gpl::parse(&encoded).expect("gpl parse round trip");
        assert_eq!(name, "CrossModule");
        assert_eq!(decoded, colors);
    }
}
