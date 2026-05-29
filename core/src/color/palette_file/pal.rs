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

use super::{PaletteFileError, PaletteFileResult};
use crate::project::color::Rgba;

// ── RIFF PAL ─────────────────────────────────────────────────────────────────

/// Parses a Microsoft RIFF `.pal` binary buffer and returns the color list.
pub fn parse_riff(data: &[u8]) -> PaletteFileResult<Vec<Rgba>> {
    // Minimum size: 8 (RIFF header) + 4 (PAL ) + 4 (data) + 4 (chunk size)
    // + 2 (version) + 2 (count) = 24 bytes
    if data.len() < 24 {
        return Err(PaletteFileError::Invalid("RIFF PAL file is too short".into()));
    }

    // Check "RIFF" signature
    if &data[0..4] != b"RIFF" {
        return Err(PaletteFileError::Invalid("not a RIFF file (missing 'RIFF' signature)".into()));
    }

    // Check "PAL " format type at offset 8
    if &data[8..12] != b"PAL " {
        return Err(PaletteFileError::Invalid("not a RIFF PAL file (missing 'PAL ' type)".into()));
    }

    // "data" chunk at offset 12
    if &data[12..16] != b"data" {
        return Err(PaletteFileError::Invalid("RIFF PAL: expected 'data' chunk".into()));
    }

    // Version at offset 20–21 (little-endian u16); must be 0x0300
    let version = u16::from_le_bytes([data[20], data[21]]);
    if version != 0x0300 {
        return Err(PaletteFileError::Invalid(format!("RIFF PAL: unsupported version {version:#06x}")));
    }

    // Count at offset 22–23 (little-endian u16)
    let count = u16::from_le_bytes([data[22], data[23]]) as usize;

    // Each entry is 4 bytes: R G B flags; entries start at offset 24
    let entries_start = 24_usize;
    let required_len = entries_start + count * 4;
    if data.len() < required_len {
        return Err(PaletteFileError::Invalid(format!("RIFF PAL: buffer too short for {count} entries")));
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
///
/// # Errors
///
/// Returns [`PaletteFileError::TooLarge`] when `colors.len() > u16::MAX`.
/// The RIFF PAL count field is a `u16`, so larger palettes can't be
/// represented faithfully — the prior implementation silently truncated
/// the count and emitted a malformed file.
pub fn encode_riff(colors: &[Rgba]) -> PaletteFileResult<Vec<u8>> {
    let count = colors.len();
    let count_u16 = u16::try_from(count).map_err(|_| PaletteFileError::TooLarge {
        count,
        max: u16::MAX as usize,
        format: "RIFF PAL",
    })?;
    // data chunk: 2 (version) + 2 (count) + count * 4
    let data_size = 4 + count * 4;
    // total file: 4 (RIFF) + 4 (file_size) + 4 (PAL ) + 4 (data) + 4 (data_size) + data_size
    let file_size = 4 + 4 + 4 + data_size; // "PAL " + "data" + data_size field + data
    let mut out = Vec::with_capacity(8 + file_size);

    // RIFF header
    // file_size and data_size both fit in u32 because count is bounded
    // by u16::MAX (above), so file_size ≤ 16 + 65535*4 ≈ 256 KiB.
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
    out.extend_from_slice(&count_u16.to_le_bytes()); // count

    for c in colors {
        out.push(c.r);
        out.push(c.g);
        out.push(c.b);
        out.push(0x00); // flags
    }

    Ok(out)
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
pub fn parse_jasc(input: &str) -> PaletteFileResult<Vec<Rgba>> {
    let mut lines = input.lines().map(str::trim).filter(|l| !l.is_empty());

    // Header line
    match lines.next() {
        Some("JASC-PAL") => {}
        _ => {
            return Err(PaletteFileError::Invalid("missing 'JASC-PAL' header".into()));
        }
    }

    // Version line — must be present and equal "0100" (the only known
    // JASC PAL version). Without this check, a file missing the version
    // line would treat its count as the version and shift every subsequent
    // line by one.
    let version = lines.next().ok_or_else(|| PaletteFileError::Invalid("JASC PAL: missing version line".into()))?;
    if version != "0100" {
        return Err(PaletteFileError::Invalid(format!("JASC PAL: unsupported version '{version}'")));
    }

    // Count line
    let count_str = lines.next().ok_or_else(|| PaletteFileError::Invalid("JASC PAL: missing color count".into()))?;
    let count: usize = count_str
        .parse()
        .map_err(|_| PaletteFileError::Invalid(format!("JASC PAL: invalid count '{count_str}'")))?;

    let mut colors = Vec::with_capacity(count);
    for line in (&mut lines).take(count) {
        let mut parts = line.split_whitespace();
        let parse_channel = |p: Option<&str>| -> PaletteFileResult<u8> {
            p.ok_or_else(|| PaletteFileError::Invalid("JASC PAL: short color entry".into()))?
                .parse::<u8>()
                .map_err(|_| PaletteFileError::Invalid("JASC PAL: invalid channel value".into()))
        };
        let r = parse_channel(parts.next())?;
        let g = parse_channel(parts.next())?;
        let b = parse_channel(parts.next())?;
        colors.push(Rgba::opaque(r, g, b));
    }

    // The prior implementation returned `Ok(colors)` even when the file
    // ran out of lines before `count` was reached, silently producing a
    // truncated palette. Catch that case here.
    if colors.len() != count {
        return Err(PaletteFileError::Invalid(format!(
            "JASC PAL: declared {count} colors but only {got} are present",
            got = colors.len()
        )));
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
        vec![Rgba::opaque(255, 0, 0), Rgba::opaque(0, 255, 0), Rgba::opaque(0, 0, 255)]
    }

    // ── RIFF ─────────────────────────────────────────────────────────────────

    #[test]
    fn riff_round_trip() {
        let original = sample_colors();
        let encoded = encode_riff(&original).unwrap();
        let decoded = parse_riff(&encoded).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn riff_rejects_wrong_magic() {
        let mut data = encode_riff(&sample_colors()).unwrap();
        data[0] = b'X';
        assert!(parse_riff(&data).is_err());
    }

    #[test]
    fn riff_rejects_wrong_type() {
        let mut data = encode_riff(&sample_colors()).unwrap();
        data[8] = b'X';
        assert!(parse_riff(&data).is_err());
    }

    #[test]
    fn riff_rejects_too_short() {
        assert!(parse_riff(&[0u8; 10]).is_err());
    }

    #[test]
    fn riff_encodes_correct_count() {
        let encoded = encode_riff(&sample_colors()).unwrap();
        // Count is at offset 22–23
        let count = u16::from_le_bytes([encoded[22], encoded[23]]);
        assert_eq!(count, 3);
    }

    #[test]
    fn riff_encode_rejects_oversize_palette() {
        // 70_000 entries exceeds u16::MAX (65_535) — the count field
        // can't represent it, so encoding must fail rather than silently
        // truncate.
        let huge: Vec<Rgba> = (0..70_000).map(|_| Rgba::opaque(0, 0, 0)).collect();
        assert!(matches!(encode_riff(&huge), Err(PaletteFileError::TooLarge { .. })));
    }

    #[test]
    fn riff_encode_accepts_max_palette() {
        // u16::MAX entries is the largest representable count.
        let max: Vec<Rgba> = (0..u16::MAX as usize).map(|_| Rgba::opaque(0, 0, 0)).collect();
        let encoded = encode_riff(&max).unwrap();
        let count = u16::from_le_bytes([encoded[22], encoded[23]]);
        assert_eq!(count, u16::MAX);
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

    #[test]
    fn jasc_rejects_missing_version_line() {
        // Without a version line, `parse_jasc` would treat the count as
        // the version and shift every subsequent line — hard to debug.
        // The parser must require the version line and reject when it's
        // absent or unsupported.
        let no_version = "JASC-PAL\r\n3\r\n255 0 0\r\n0 255 0\r\n0 0 255\r\n";
        assert!(parse_jasc(no_version).is_err());
    }

    #[test]
    fn jasc_rejects_unsupported_version() {
        let bad = "JASC-PAL\r\n0200\r\n1\r\n255 0 0\r\n";
        assert!(parse_jasc(bad).is_err());
    }

    #[test]
    fn jasc_rejects_short_color_block() {
        // Declared count is 3 but only 2 color lines are present. The
        // prior `lines.take(count)` followed by `Ok(colors)` silently
        // produced a truncated palette.
        let short = "JASC-PAL\r\n0100\r\n3\r\n255 0 0\r\n0 255 0\r\n";
        assert!(parse_jasc(short).is_err());
    }
}
