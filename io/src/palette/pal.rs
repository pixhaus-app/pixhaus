//! Microsoft RIFF `.pal` and JASC `.pal` palette parsers and writers.
//!
//! Both formats use the `.pal` extension but are structurally different:
//!
//! - **RIFF PAL** — binary format. Header: `RIFF` + file size + `PAL ` +
//!   `data` + data size + version (`0x0300`) + count, then 4 bytes per entry
//!   (R, G, B, flags).
//!
//! - **JASC PAL** — text format (`JASC-PAL` header, version `0100`,
//!   count, then `R G B` per line).

use std::fmt::Write as _;

use pixhaus_core::project::color::Rgba;

use crate::error::{Error, Result};

// ── RIFF PAL ─────────────────────────────────────────────────────────────────

/// Parses a Microsoft RIFF `.pal` binary buffer and returns the color list.
pub fn parse_riff(data: &[u8]) -> Result<Vec<Rgba>> {
    // Minimum size: 8 (RIFF header) + 4 (PAL ) + 4 (data) + 4 (chunk size)
    // + 2 (version) + 2 (count) = 24 bytes
    if data.len() < 24 {
        return Err(Error::InvalidPalette("RIFF PAL file is too short".into()));
    }

    // Check "RIFF" signature
    if &data[0..4] != b"RIFF" {
        return Err(Error::InvalidPalette(
            "not a RIFF file (missing 'RIFF' signature)".into(),
        ));
    }

    // Check "PAL " format type at offset 8
    if &data[8..12] != b"PAL " {
        return Err(Error::InvalidPalette(
            "not a RIFF PAL file (missing 'PAL ' type)".into(),
        ));
    }

    // "data" chunk at offset 12
    if &data[12..16] != b"data" {
        return Err(Error::InvalidPalette(
            "RIFF PAL: expected 'data' chunk".into(),
        ));
    }

    // Version at offset 20–21 (little-endian u16); must be 0x0300
    let version = u16::from_le_bytes([data[20], data[21]]);
    if version != 0x0300 {
        return Err(Error::InvalidPalette(format!(
            "RIFF PAL: unsupported version {version:#06x}"
        )));
    }

    // Count at offset 22–23 (little-endian u16)
    let count = u16::from_le_bytes([data[22], data[23]]) as usize;

    // Each entry is 4 bytes: R G B flags; entries start at offset 24
    let entries_start = 24_usize;
    let required_len = entries_start + count * 4;
    if data.len() < required_len {
        return Err(Error::InvalidPalette(format!(
            "RIFF PAL: buffer too short for {count} entries"
        )));
    }

    let colors = (0..count)
        .map(|i| {
            let base = entries_start + i * 4;
            Rgba::opaque(data[base], data[base + 1], data[base + 2])
        })
        .collect();

    Ok(colors)
}

/// Encodes a color list as a Microsoft RIFF `.pal` binary buffer.
pub fn encode_riff(colors: &[Rgba]) -> Vec<u8> {
    let count = colors.len();
    // data chunk: 2 (version) + 2 (count) + count * 4
    let data_size = 4 + count * 4;
    // total file: 4 (RIFF) + 4 (file_size) + 4 (PAL ) + 4 (data) + 4 (data_size) + data_size
    let file_size = 4 + 4 + 4 + data_size; // "PAL " + "data" + data_size field + data
    let mut out = Vec::with_capacity(8 + file_size);

    // RIFF header
    // Palette data is always small (≤256 entries × 4 bytes); these casts are safe.
    #[allow(clippy::cast_possible_truncation)]
    out.extend_from_slice(b"RIFF");
    #[allow(clippy::cast_possible_truncation)]
    out.extend_from_slice(&(file_size as u32).to_le_bytes());
    out.extend_from_slice(b"PAL ");

    // data chunk
    out.extend_from_slice(b"data");
    #[allow(clippy::cast_possible_truncation)]
    out.extend_from_slice(&(data_size as u32).to_le_bytes());
    out.extend_from_slice(&0x0300_u16.to_le_bytes()); // version
    #[allow(clippy::cast_possible_truncation)]
    out.extend_from_slice(&(count as u16).to_le_bytes()); // count

    for c in colors {
        out.push(c.r);
        out.push(c.g);
        out.push(c.b);
        out.push(0x00); // flags
    }

    out
}

// ── JASC PAL ─────────────────────────────────────────────────────────────────

/// Parses a JASC `.pal` text file and returns the color list.
///
/// JASC format:
/// ```text
/// JASC-PAL
/// 0100
/// <count>
/// <R> <G> <B>
/// ...
/// ```
pub fn parse_jasc(input: &str) -> Result<Vec<Rgba>> {
    let mut lines = input.lines().map(str::trim).filter(|l| !l.is_empty());

    // Header line
    match lines.next() {
        Some("JASC-PAL") => {}
        _ => {
            return Err(Error::InvalidPalette("missing 'JASC-PAL' header".into()));
        }
    }

    // Version line
    lines.next(); // "0100" — we accept any value and move on

    // Count line
    let count_str = lines
        .next()
        .ok_or_else(|| Error::InvalidPalette("JASC PAL: missing color count".into()))?;
    let count: usize = count_str
        .parse()
        .map_err(|_| Error::InvalidPalette(format!("JASC PAL: invalid count '{count_str}'")))?;

    let mut colors = Vec::with_capacity(count);
    for line in lines.take(count) {
        let mut parts = line.split_whitespace();
        let parse_channel = |p: Option<&str>| -> Result<u8> {
            p.ok_or_else(|| Error::InvalidPalette("JASC PAL: short color entry".into()))?
                .parse::<u8>()
                .map_err(|_| Error::InvalidPalette("JASC PAL: invalid channel value".into()))
        };
        let r = parse_channel(parts.next())?;
        let g = parse_channel(parts.next())?;
        let b = parse_channel(parts.next())?;
        colors.push(Rgba::opaque(r, g, b));
    }

    Ok(colors)
}

/// Encodes a color list as a JASC `.pal` text string.
pub fn encode_jasc(colors: &[Rgba]) -> String {
    let mut out = format!("JASC-PAL\r\n0100\r\n{}\r\n", colors.len());
    for c in colors {
        let _ = write!(out, "{} {} {}\r\n", c.r, c.g, c.b);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_colors() -> Vec<Rgba> {
        vec![
            Rgba::opaque(255, 0, 0),
            Rgba::opaque(0, 255, 0),
            Rgba::opaque(0, 0, 255),
        ]
    }

    // ── RIFF ─────────────────────────────────────────────────────────────────

    #[test]
    fn riff_round_trip() {
        let original = sample_colors();
        let encoded = encode_riff(&original);
        let decoded = parse_riff(&encoded).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn riff_rejects_wrong_magic() {
        let mut data = encode_riff(&sample_colors());
        data[0] = b'X';
        assert!(parse_riff(&data).is_err());
    }

    #[test]
    fn riff_rejects_wrong_type() {
        let mut data = encode_riff(&sample_colors());
        data[8] = b'X';
        assert!(parse_riff(&data).is_err());
    }

    #[test]
    fn riff_rejects_too_short() {
        assert!(parse_riff(&[0u8; 10]).is_err());
    }

    #[test]
    fn riff_encodes_correct_count() {
        let encoded = encode_riff(&sample_colors());
        // Count is at offset 22–23
        let count = u16::from_le_bytes([encoded[22], encoded[23]]);
        assert_eq!(count, 3);
    }

    // ── JASC ─────────────────────────────────────────────────────────────────

    #[test]
    fn jasc_round_trip() {
        let original = sample_colors();
        let encoded = encode_jasc(&original);
        let decoded = parse_jasc(&encoded).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn jasc_rejects_wrong_header() {
        assert!(parse_jasc("NOT-JASC\n0100\n3\n255 0 0\n").is_err());
    }

    #[test]
    fn jasc_rejects_invalid_channel() {
        let input = "JASC-PAL\r\n0100\r\n1\r\n256 0 0\r\n";
        assert!(parse_jasc(input).is_err());
    }

    #[test]
    fn jasc_empty_palette() {
        let encoded = encode_jasc(&[]);
        let decoded = parse_jasc(&encoded).unwrap();
        assert!(decoded.is_empty());
    }

    #[test]
    fn jasc_parse_real_example() {
        let input = "JASC-PAL\r\n0100\r\n4\r\n0 0 0\r\n255 0 0\r\n0 255 0\r\n0 0 255\r\n";
        let colors = parse_jasc(input).unwrap();
        assert_eq!(colors.len(), 4);
        assert_eq!(colors[0], Rgba::opaque(0, 0, 0));
    }
}
