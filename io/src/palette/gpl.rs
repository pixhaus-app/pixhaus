//! GIMP Palette (`.gpl`) reader and writer.
//!
//! Format reference:
//! <https://wiki.gnome.org/Apps/Gimp/FileFormats>
//!
//! ```text
//! GIMP Palette
//! Name: <name>
//! Columns: <n>
//! #
//! <R> <G> <B>[ <optional name>]
//! ...
//! ```
//!
//! Lines beginning with `#` are comments and are skipped (except the
//! single `#` header separator, which marks the end of the metadata block).

use std::fmt::Write as _;

use pixhaus_core::project::color::Rgba;

use crate::error::{Error, Result};

/// Parses a GIMP Palette string and returns the palette name and colors.
///
/// Lines with fewer than three whitespace-separated integers are skipped.
/// The transparent index (color index 0) has no special treatment in GPL;
/// callers decide how to handle transparency.
pub fn parse(input: &str) -> Result<(String, Vec<Rgba>)> {
    let mut lines = input.lines();

    // First line must be "GIMP Palette"
    let first = lines.next().unwrap_or("").trim();
    if first != "GIMP Palette" {
        return Err(Error::InvalidPalette(
            "missing 'GIMP Palette' header".into(),
        ));
    }

    let mut name = String::new();
    let mut in_colors = false;
    let mut colors = Vec::new();

    for line in lines {
        let line = line.trim();

        if in_colors {
            // Skip blank lines and comment lines
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some(color) = parse_color_line(line) {
                colors.push(color);
            }
        } else {
            // Metadata block. Only the bare `#` (optionally with trailing
            // whitespace) is the metadata/colors separator. Other lines
            // beginning with `#` (e.g. `# Created by GIMP`) are comments
            // that stay inside the metadata block — the prior
            // `starts_with('#')` short-circuit prematurely flipped to
            // colors mode and could miss `Name:` lines that followed
            // a comment.
            if let Some(val) = line.strip_prefix("Name:") {
                val.trim().clone_into(&mut name);
            } else if line == "#" {
                in_colors = true;
            }
            // Other `#` lines are metadata comments; ignored.
        }
    }

    Ok((name, colors))
}

/// Serializes a palette name and color list to a GPL string.
pub fn encode(name: &str, colors: &[Rgba]) -> String {
    let mut out = String::new();
    out.push_str("GIMP Palette\n");
    let _ = writeln!(out, "Name: {name}");
    out.push_str("Columns: 0\n");
    out.push_str("#\n");
    for c in colors {
        let _ = writeln!(out, "{} {} {}", c.r, c.g, c.b);
    }
    out
}

/// Parses a single color line: `<R> <G> <B>[ optional name...]`.
fn parse_color_line(line: &str) -> Option<Rgba> {
    let mut parts = line.split_whitespace();
    let r: u8 = parts.next()?.parse().ok()?;
    let g: u8 = parts.next()?.parse().ok()?;
    let b: u8 = parts.next()?.parse().ok()?;
    Some(Rgba::opaque(r, g, b))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "GIMP Palette\n\
Name: Test\n\
Columns: 0\n\
#\n\
255 0 0\n\
0 255 0\n\
0 0 255\n";

    #[test]
    fn parse_standard_palette() {
        let (name, colors) = parse(SAMPLE).unwrap();
        assert_eq!(name, "Test");
        assert_eq!(colors.len(), 3);
        assert_eq!(colors[0], Rgba::opaque(255, 0, 0));
        assert_eq!(colors[1], Rgba::opaque(0, 255, 0));
        assert_eq!(colors[2], Rgba::opaque(0, 0, 255));
    }

    #[test]
    fn parse_skips_comments_in_color_block() {
        let input = "GIMP Palette\nName: X\n#\n# comment\n255 0 0\n";
        let (_, colors) = parse(input).unwrap();
        assert_eq!(colors.len(), 1);
    }

    #[test]
    fn parse_color_line_with_name() {
        let input = "GIMP Palette\nName: X\n#\n255 128 0 Orange\n";
        let (_, colors) = parse(input).unwrap();
        assert_eq!(colors[0], Rgba::opaque(255, 128, 0));
    }

    #[test]
    fn parse_rejects_missing_header() {
        assert!(parse("not a palette\n").is_err());
    }

    #[test]
    fn parse_keeps_metadata_comments_inside_metadata_block() {
        // Regression for thread 2: prior code flipped to colors mode on
        // any `#`-prefixed line, so `Name:` after a comment was missed.
        let input = "GIMP Palette\n\
# Created by Pixhaus\n\
Name: Heroes\n\
Columns: 4\n\
#\n\
255 0 0\n";
        let (name, colors) = parse(input).unwrap();
        assert_eq!(
            name, "Heroes",
            "Name: must survive a leading metadata comment"
        );
        assert_eq!(colors.len(), 1);
    }

    #[test]
    fn encode_then_parse_round_trip() {
        let original = vec![Rgba::opaque(10, 20, 30), Rgba::opaque(40, 50, 60)];
        let encoded = encode("RoundTrip", &original);
        let (name, colors) = parse(&encoded).unwrap();
        assert_eq!(name, "RoundTrip");
        assert_eq!(colors, original);
    }

    #[test]
    fn encode_empty_palette() {
        let encoded = encode("Empty", &[]);
        let (name, colors) = parse(&encoded).unwrap();
        assert_eq!(name, "Empty");
        assert!(colors.is_empty());
    }
}
