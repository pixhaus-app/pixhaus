//! Lospec `.hex` palette format reader and writer.
//!
//! The format is a plain-text list of six-character lowercase hex color
//! codes, one per line, with no `#` prefix:
//!
//! ```text
//! ff0000
//! 00ff00
//! 0000ff
//! ```
//!
//! Blank lines and lines beginning with `#` are treated as comments
//! and skipped. Letters may be upper or lower case.

use std::fmt::Write as _;

use super::{PaletteFileError, PaletteFileResult};
use crate::project::color::Rgba;

/// Parses a Lospec `.hex` palette string and returns the color list.
///
/// Each non-blank, non-comment line must be exactly six hex digits
/// (`rrggbb`). Malformed lines return an error.
pub fn parse(input: &str) -> PaletteFileResult<Vec<Rgba>> {
    let mut colors = Vec::new();

    for (line_num, raw_line) in input.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        // The format allows an optional `#` prefix on color lines (e.g.
        // `#ff0000`). To avoid swallowing those as comments, we try the
        // `#`-stripped value as a 6-char hex color *first*. Only lines
        // that don't parse as a color AND start with `#` are treated as
        // comments and skipped.
        let hex = line.strip_prefix('#').unwrap_or(line);
        if hex.len() != 6 {
            if line.starts_with('#') {
                // Comment line — `#` followed by something that isn't a
                // 6-hex-digit color (e.g. `# palette name`). Skip.
                continue;
            }
            return Err(PaletteFileError::Invalid(format!(
                "hex palette line {}: expected 6-char hex, got '{hex}'",
                line_num + 1
            )));
        }
        let r = u8::from_str_radix(&hex[0..2], 16).map_err(|_| PaletteFileError::Invalid(format!("hex palette line {}: invalid hex '{hex}'", line_num + 1)))?;
        let g = u8::from_str_radix(&hex[2..4], 16).map_err(|_| PaletteFileError::Invalid(format!("hex palette line {}: invalid hex '{hex}'", line_num + 1)))?;
        let b = u8::from_str_radix(&hex[4..6], 16).map_err(|_| PaletteFileError::Invalid(format!("hex palette line {}: invalid hex '{hex}'", line_num + 1)))?;
        colors.push(Rgba::opaque(r, g, b));
    }

    Ok(colors)
}

/// Serializes a color list as a Lospec `.hex` string.
///
/// Alpha is ignored; output is lowercase hex without `#` prefix.
pub fn encode(colors: &[Rgba]) -> String {
    let mut out = String::new();
    for c in colors {
        let _ = writeln!(out, "{:02x}{:02x}{:02x}", c.r, c.g, c.b);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_standard_hex() {
        let input = "ff0000\n00ff00\n0000ff\n";
        let colors = parse(input).unwrap();
        assert_eq!(colors.len(), 3);
        assert_eq!(colors[0], Rgba::opaque(255, 0, 0));
        assert_eq!(colors[1], Rgba::opaque(0, 255, 0));
        assert_eq!(colors[2], Rgba::opaque(0, 0, 255));
    }

    #[test]
    fn parse_uppercase() {
        let colors = parse("FF0000\n").unwrap();
        assert_eq!(colors[0], Rgba::opaque(255, 0, 0));
    }

    #[test]
    fn parse_skips_blank_lines() {
        let input = "ff0000\n\n00ff00\n";
        let colors = parse(input).unwrap();
        assert_eq!(colors.len(), 2);
    }

    #[test]
    fn parse_skips_comment_lines() {
        let input = "# palette name\nff0000\n";
        let colors = parse(input).unwrap();
        assert_eq!(colors.len(), 1);
    }

    #[test]
    fn parse_accepts_hash_prefixed_colors() {
        // `#ff0000` is a valid Lospec hex color, not a comment. Regression
        // for thread 1: the prior `starts_with('#')` short-circuit silently
        // swallowed colors written with the optional `#` prefix.
        let input = "#ff0000\n#00ff00\n# palette name\n#0000ff\n";
        let colors = parse(input).unwrap();
        assert_eq!(colors.len(), 3);
        assert_eq!(colors[0], Rgba::opaque(255, 0, 0));
        assert_eq!(colors[1], Rgba::opaque(0, 255, 0));
        assert_eq!(colors[2], Rgba::opaque(0, 0, 255));
    }

    #[test]
    fn parse_rejects_short_hex() {
        assert!(parse("fff\n").is_err());
    }

    #[test]
    fn parse_rejects_invalid_hex_chars() {
        assert!(parse("gggggg\n").is_err());
    }

    #[test]
    fn encode_then_parse_round_trip() {
        let original = vec![Rgba::opaque(0, 0, 0), Rgba::opaque(128, 64, 32), Rgba::opaque(255, 255, 255)];
        let encoded = encode(&original);
        let decoded = parse(&encoded).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn encode_empty_palette() {
        let encoded = encode(&[]);
        assert!(encoded.is_empty());
    }

    #[test]
    fn encode_uses_lowercase() {
        let encoded = encode(&[Rgba::opaque(0xAB, 0xCD, 0xEF)]);
        assert!(encoded.starts_with("abcdef"));
    }
}
