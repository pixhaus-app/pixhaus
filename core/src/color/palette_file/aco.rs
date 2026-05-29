//! Photoshop Color Swatch (`.aco`) reader.
//!
//! Format reference: Adobe Photoshop File Formats Specification, section
//! "Color Swatch".
//!
//! # Structure
//!
//! An ACO file may have one or two sections:
//!
//! - **Section 1** (version 1): 2-byte version (`0x0001`), 2-byte count,
//!   then count × 10 bytes per color (2-byte color space + 4 × 2-byte
//!   components).
//!
//! - **Section 2** (version 2): identical layout but each entry is followed
//!   by a null-terminated UTF-16BE name.
//!
//! Pixhaus reads RGB colors (color space `0`) from either section.
//! Other color spaces are skipped with a warning; unknown mandatory spaces
//! return an error.

use super::{PaletteFileError, PaletteFileResult};
use crate::project::color::Rgba;

// Color space codes
const CS_RGB: u16 = 0;
const CS_HSB: u16 = 1;
const CS_CMYK: u16 = 2;
const CS_LAB: u16 = 7;
const CS_GRAYSCALE: u16 = 8;

/// Reads an Adobe `.aco` swatch file and returns the RGB colors.
///
/// Non-RGB color spaces are converted where straightforward (grayscale) or
/// skipped (Lab, CMYK, HSB) — Pixhaus works in sRGB.
pub fn parse(data: &[u8]) -> PaletteFileResult<Vec<Rgba>> {
    if data.len() < 4 {
        return Err(PaletteFileError::Invalid("ACO file too short".into()));
    }

    let version = read_u16_be(data, 0)?;
    let count = read_u16_be(data, 2)? as usize;

    match version {
        1 => read_section_v1(data, 4, count),
        2 => read_section_v2(data, 4, count),
        _ => Err(PaletteFileError::Invalid(format!("ACO: unsupported version {version}"))),
    }
}

// ── Internal helpers ─────────────────────────────────────────────────────────

fn read_u16_be(data: &[u8], offset: usize) -> PaletteFileResult<u16> {
    data.get(offset..offset + 2)
        .map(|b| u16::from_be_bytes([b[0], b[1]]))
        .ok_or_else(|| PaletteFileError::Invalid("ACO: unexpected end of file".into()))
}

fn read_section_v1(data: &[u8], start: usize, count: usize) -> PaletteFileResult<Vec<Rgba>> {
    let mut colors = Vec::with_capacity(count);
    let mut offset = start;

    for _ in 0..count {
        // The prior `break` here returned `Ok(colors)` with a partial
        // palette when the file was shorter than `count` × 10 bytes —
        // silently dropping data. File I/O on untrusted input must
        // surface the truncation.
        if offset + 10 > data.len() {
            return Err(PaletteFileError::Invalid(
                "ACO v1: file ends mid-entry; declared count is larger than payload".into(),
            ));
        }
        let cs = read_u16_be(data, offset)?;
        let c1 = read_u16_be(data, offset + 2)?;
        let c2 = read_u16_be(data, offset + 4)?;
        let c3 = read_u16_be(data, offset + 6)?;
        // c4 (offset + 8) is unused for all supported spaces
        offset += 10;

        if let Some(rgba) = decode_color(cs, c1, c2, c3)? {
            colors.push(rgba);
        }
    }

    Ok(colors)
}

fn read_section_v2(data: &[u8], start: usize, count: usize) -> PaletteFileResult<Vec<Rgba>> {
    let mut colors = Vec::with_capacity(count);
    let mut offset = start;

    for _ in 0..count {
        if offset + 10 > data.len() {
            return Err(PaletteFileError::Invalid(
                "ACO v2: file ends mid-entry; declared count is larger than payload".into(),
            ));
        }
        let cs = read_u16_be(data, offset)?;
        let c1 = read_u16_be(data, offset + 2)?;
        let c2 = read_u16_be(data, offset + 4)?;
        let c3 = read_u16_be(data, offset + 6)?;
        offset += 10;

        // Skip the name: 4-byte length (u32 BE = number of UTF-16 chars)
        // + len * 2 bytes. The arithmetic must be checked end-to-end
        // because `name_len` is read directly from the file: an
        // adversarial `name_len = u32::MAX` would otherwise overflow the
        // multiplication on 32-bit targets, or wrap to a small value
        // and skip past most of the buffer on 64-bit, parsing garbage.
        if offset + 4 > data.len() {
            return Err(PaletteFileError::Invalid("ACO v2: file ends before name length field".into()));
        }
        let name_len = u32::from_be_bytes([data[offset], data[offset + 1], data[offset + 2], data[offset + 3]]) as usize;
        let name_bytes = name_len
            .checked_mul(2)
            .ok_or_else(|| PaletteFileError::Invalid("ACO v2: name length overflow".into()))?;
        let new_offset = offset
            .checked_add(4)
            .and_then(|o| o.checked_add(name_bytes))
            .ok_or_else(|| PaletteFileError::Invalid("ACO v2: offset overflow".into()))?;
        if new_offset > data.len() {
            return Err(PaletteFileError::Invalid("ACO v2: file ends mid-name".into()));
        }
        offset = new_offset;

        if let Some(rgba) = decode_color(cs, c1, c2, c3)? {
            colors.push(rgba);
        }
    }

    Ok(colors)
}

/// Converts ACO color components to `Rgba`, or returns `None` to skip.
#[allow(clippy::cast_possible_truncation)]
fn decode_color(cs: u16, c1: u16, c2: u16, c3: u16) -> PaletteFileResult<Option<Rgba>> {
    match cs {
        CS_RGB => {
            // Components are in range [0, 65535]; divide by 257 to get [0, 255].
            // 65535 / 257 = 255, 0 / 257 = 0.
            Ok(Some(Rgba::opaque((c1 / 257) as u8, (c2 / 257) as u8, (c3 / 257) as u8)))
        }
        CS_GRAYSCALE => {
            // c1 is gray in [0, 10000]; scale to [0, 255].
            let gray = (u32::from(c1) * 255 / 10000).min(255) as u8;
            Ok(Some(Rgba::opaque(gray, gray, gray)))
        }
        // Skip HSB, CMYK, Lab — conversion is lossy and out of scope
        CS_HSB | CS_CMYK | CS_LAB => Ok(None),
        code => Err(PaletteFileError::UnsupportedColorSpace { code }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal version-1 ACO buffer.
    fn build_v1(colors: &[(u16, u16, u16, u16)]) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&1_u16.to_be_bytes()); // version
        buf.extend_from_slice(&(colors.len() as u16).to_be_bytes()); // count
        for &(cs, c1, c2, c3) in colors {
            buf.extend_from_slice(&cs.to_be_bytes());
            buf.extend_from_slice(&c1.to_be_bytes());
            buf.extend_from_slice(&c2.to_be_bytes());
            buf.extend_from_slice(&c3.to_be_bytes());
            buf.extend_from_slice(&0_u16.to_be_bytes()); // c4
        }
        buf
    }

    #[test]
    fn parse_v1_rgb_colors() {
        // Red: R=65535, G=0, B=0
        let data = build_v1(&[(CS_RGB, 65535, 0, 0)]);
        let colors = parse(&data).unwrap();
        assert_eq!(colors.len(), 1);
        assert_eq!(colors[0], Rgba::opaque(255, 0, 0));
    }

    #[test]
    fn parse_v1_grayscale() {
        // Mid gray: c1 = 5000 → ~127
        let data = build_v1(&[(CS_GRAYSCALE, 5000, 0, 0)]);
        let colors = parse(&data).unwrap();
        assert_eq!(colors.len(), 1);
        assert_eq!(colors[0].r, colors[0].g);
        assert_eq!(colors[0].g, colors[0].b);
    }

    #[test]
    fn parse_skips_hsb_colors() {
        let data = build_v1(&[(CS_HSB, 0, 0, 0), (CS_RGB, 0, 0, 0)]);
        let colors = parse(&data).unwrap();
        // Only the RGB entry should remain
        assert_eq!(colors.len(), 1);
    }

    #[test]
    fn parse_rejects_unknown_color_space() {
        let data = build_v1(&[(999, 0, 0, 0)]);
        assert!(parse(&data).is_err());
    }

    #[test]
    fn parse_rejects_too_short() {
        assert!(parse(&[0_u8; 2]).is_err());
    }

    #[test]
    fn parse_rejects_unknown_version() {
        let mut data = build_v1(&[]);
        data[0] = 0;
        data[1] = 3; // version 3
        assert!(parse(&data).is_err());
    }

    #[test]
    fn v1_truncated_payload_returns_error_not_partial_palette() {
        // Header declares 3 colors but only 2 entries' worth of bytes
        // follow. Prior code returned `Ok(colors)` with 2 entries — silent
        // truncation. New behaviour: error.
        let mut data = build_v1(&[(CS_RGB, 0, 0, 0), (CS_RGB, 0, 0, 0)]);
        // Bump the count to 3 without adding a third entry
        data[2..4].copy_from_slice(&3_u16.to_be_bytes());
        assert!(parse(&data).is_err());
    }

    #[test]
    fn v2_huge_name_len_does_not_overflow() {
        // Adversarial v2 entry where name_len = u32::MAX would, with
        // unchecked arithmetic, overflow to a small wrapped offset and
        // proceed to parse garbage. The checked_mul + checked_add guard
        // converts that into a clean Invalid error.
        let mut data = Vec::new();
        data.extend_from_slice(&2_u16.to_be_bytes()); // version 2
        data.extend_from_slice(&1_u16.to_be_bytes()); // count = 1
        // Color entry: 10 bytes (RGB)
        data.extend_from_slice(&CS_RGB.to_be_bytes());
        data.extend_from_slice(&0_u16.to_be_bytes());
        data.extend_from_slice(&0_u16.to_be_bytes());
        data.extend_from_slice(&0_u16.to_be_bytes());
        data.extend_from_slice(&0_u16.to_be_bytes());
        // Adversarial name_len = u32::MAX
        data.extend_from_slice(&u32::MAX.to_be_bytes());
        // (no actual name bytes — relies on the guard)
        assert!(parse(&data).is_err());
    }
}
