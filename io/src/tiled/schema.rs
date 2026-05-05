//! Structural TMX validator aligned with the Tiled 1.10 map schema.
//!
//! The Tiled project publishes an XSD at
//! <https://mapeditor.org/dtd/1.0/map.dtd> (DTD) and documents the XML
//! format at <https://doc.mapeditor.org/en/stable/reference/tmx-map-format/>.
//! This module implements the subset of constraints that the Pixhaus exporter
//! is obligated to satisfy — element ordering, required attributes, and value
//! ranges — without taking on a full XSD processor dependency.
//!
//! # What is checked
//!
//! - XML declaration (`<?xml ... ?>`) is the first line.
//! - `<map>` root element has: `version`, `tiledversion`, `orientation`,
//!   `renderorder`, `width`, `height`, `tilewidth`, `tileheight`,
//!   `infinite`, `nextlayerid`, `nextobjectid`.
//! - `<map>` closes with `</map>`.
//! - `infinite` attribute value is `"0"` or `"1"`.
//! - Every `<tileset>` has `firstgid` (positive integer) and `source`.
//! - `firstgid` values are monotonically increasing.
//! - First `firstgid` is `1`.
//! - Every `<layer>` has `id`, `name`, `width`, `height`.
//! - Every `<layer>` contains a `<data>` element with `encoding="csv"`.
//!
//! # What is not checked
//!
//! - TSX file content (validated separately by the tileset XML builder).
//! - CSV data correctness (tile index ranges validated at export time).
//! - Object layers, group layers, image layers (not produced by this exporter).

use crate::error::{Error, Result};

/// Validates the structural constraints of a Tiled 1.10 TMX string.
///
/// See the module documentation for the full list of checks.
///
/// # Errors
///
/// Returns [`Error::TiledSchemaViolation`] with a message naming the first
/// constraint that fails.
pub fn validate_tmx(tmx: &str) -> Result<()> {
    check_xml_declaration(tmx)?;
    check_map_element(tmx)?;
    check_map_closing(tmx)?;
    check_infinite_attribute(tmx)?;
    let firstgids = check_tileset_elements(tmx)?;
    check_firstgid_sequence(&firstgids)?;
    check_layer_elements(tmx)?;
    Ok(())
}

// ── XML declaration ───────────────────────────────────────────────────────────

fn check_xml_declaration(tmx: &str) -> Result<()> {
    let first_line = tmx.lines().next().unwrap_or("");
    if !first_line.starts_with("<?xml") {
        return Err(Error::TiledSchemaViolation(
            "TMX must begin with an XML declaration (<?xml ...)".into(),
        ));
    }
    Ok(())
}

// ── <map> root element ────────────────────────────────────────────────────────

const MAP_REQUIRED_ATTRS: &[&str] = &[
    "version",
    "tiledversion",
    "orientation",
    "renderorder",
    "width",
    "height",
    "tilewidth",
    "tileheight",
    "infinite",
    "nextlayerid",
    "nextobjectid",
];

/// Extracts the full text of the `<map ...>` opening tag from `tmx`.
///
/// The Pixhaus exporter writes the map tag across multiple lines, so we scan
/// from `<map ` to the first `>` rather than treating it as a single line.
fn extract_map_open_tag(tmx: &str) -> Option<&str> {
    let start = tmx.find("<map ")?;
    let end = tmx[start..].find('>')? + start + 1;
    Some(&tmx[start..end])
}

fn check_map_element(tmx: &str) -> Result<()> {
    let map_tag = extract_map_open_tag(tmx)
        .ok_or_else(|| Error::TiledSchemaViolation("no <map> root element found".into()))?;

    for attr in MAP_REQUIRED_ATTRS {
        if !map_tag.contains(&format!("{attr}=\"")) {
            return Err(Error::TiledSchemaViolation(format!(
                "<map> is missing required attribute \"{attr}\""
            )));
        }
    }
    Ok(())
}

fn check_map_closing(tmx: &str) -> Result<()> {
    if !tmx.contains("</map>") {
        return Err(Error::TiledSchemaViolation(
            "TMX has no closing </map> tag".into(),
        ));
    }
    Ok(())
}

fn check_infinite_attribute(tmx: &str) -> Result<()> {
    let map_tag = extract_map_open_tag(tmx).unwrap_or(""); // already checked
    if map_tag.contains("infinite=\"0\"") || map_tag.contains("infinite=\"1\"") {
        return Ok(());
    }
    Err(Error::TiledSchemaViolation(
        "<map> attribute \"infinite\" must be \"0\" or \"1\"".into(),
    ))
}

// ── <tileset> elements ────────────────────────────────────────────────────────

/// Checks each `<tileset>` line for required attributes, returning the
/// collected `firstgid` values in document order.
fn check_tileset_elements(tmx: &str) -> Result<Vec<u32>> {
    let mut firstgids = Vec::new();
    for (line_no, line) in tmx.lines().enumerate() {
        let trimmed = line.trim_start();
        if !trimmed.starts_with("<tileset ") {
            continue;
        }
        // Require firstgid.
        let fg = extract_attr_u32(trimmed, "firstgid").ok_or_else(|| {
            Error::TiledSchemaViolation(format!(
                "<tileset> on line {line_no} is missing a valid \"firstgid\" attribute"
            ))
        })?;
        // Require source.
        if !trimmed.contains("source=\"") {
            return Err(Error::TiledSchemaViolation(format!(
                "<tileset> on line {line_no} is missing required attribute \"source\""
            )));
        }
        firstgids.push(fg);
    }
    Ok(firstgids)
}

fn check_firstgid_sequence(firstgids: &[u32]) -> Result<()> {
    if firstgids.is_empty() {
        return Ok(());
    }
    if firstgids[0] != 1 {
        return Err(Error::TiledSchemaViolation(format!(
            "first <tileset> must have firstgid=\"1\", got {}",
            firstgids[0]
        )));
    }
    for window in firstgids.windows(2) {
        if window[1] <= window[0] {
            return Err(Error::TiledSchemaViolation(format!(
                "tileset firstgid values must be strictly increasing: {} followed by {}",
                window[0], window[1]
            )));
        }
    }
    Ok(())
}

// ── <layer> elements ──────────────────────────────────────────────────────────

const LAYER_REQUIRED_ATTRS: &[&str] = &["id", "name", "width", "height"];

fn check_layer_elements(tmx: &str) -> Result<()> {
    // Collect the positions of every <layer> opening and the matching </layer>
    // close so we can check that <data encoding="csv"> exists inside each one.
    let layer_positions: Vec<usize> = tmx.match_indices("<layer ").map(|(pos, _)| pos).collect();

    for (n, &start) in layer_positions.iter().enumerate() {
        let layer_snippet = &tmx[start..];

        // Extract just the opening tag for attribute checks (ends at '>').
        let tag_end = layer_snippet.find('>').unwrap_or(layer_snippet.len());
        let open_tag = &layer_snippet[..tag_end];

        for attr in LAYER_REQUIRED_ATTRS {
            if !open_tag.contains(&format!("{attr}=\"")) {
                return Err(Error::TiledSchemaViolation(format!(
                    "<layer> #{} is missing required attribute \"{attr}\"",
                    n + 1
                )));
            }
        }

        // Verify <data encoding="csv"> appears before the closing </layer>.
        let close_pos = layer_snippet
            .find("</layer>")
            .unwrap_or(layer_snippet.len());
        let layer_body = &layer_snippet[..close_pos];
        if !layer_body.contains("encoding=\"csv\"") {
            return Err(Error::TiledSchemaViolation(format!(
                "<layer> #{} has no <data encoding=\"csv\"> block",
                n + 1
            )));
        }
    }
    Ok(())
}

// ── Attribute helpers ─────────────────────────────────────────────────────────

/// Extracts the value of attribute `name` from a tag string and parses it as
/// `u32`. Returns `None` if the attribute is absent or not a valid integer.
fn extract_attr_u32(tag: &str, name: &str) -> Option<u32> {
    let needle = format!("{name}=\"");
    let start = tag.find(&needle)? + needle.len();
    let end = tag[start..].find('"')? + start;
    tag[start..end].parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_tmx() -> String {
        [
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>",
            "<map version=\"1.10\" tiledversion=\"1.10.0\"",
            "     orientation=\"orthogonal\" renderorder=\"right-down\"",
            "     width=\"4\" height=\"4\"",
            "     tilewidth=\"16\" tileheight=\"16\"",
            "     infinite=\"0\" nextlayerid=\"2\" nextobjectid=\"1\">",
            "",
            "  <tileset firstgid=\"1\" source=\"dungeon.tsx\"/>",
            "",
            "  <layer id=\"1\" name=\"ground\" width=\"4\" height=\"4\">",
            "    <data encoding=\"csv\">",
            "0,0,0,0,",
            "0,0,0,0,",
            "0,0,0,0,",
            "0,0,0,0",
            "    </data>",
            "  </layer>",
            "",
            "</map>",
        ]
        .join("\n")
    }

    #[test]
    fn minimal_tmx_passes_validation() {
        validate_tmx(&minimal_tmx()).unwrap();
    }

    #[test]
    fn missing_xml_declaration_fails() {
        let tmx = minimal_tmx().replace("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n", "");
        let err = validate_tmx(&tmx).unwrap_err();
        assert!(err.to_string().contains("XML declaration"));
    }

    #[test]
    fn missing_map_element_fails() {
        let tmx = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<notmap/>\n</notmap>\n";
        let err = validate_tmx(tmx).unwrap_err();
        assert!(err.to_string().contains("<map>"));
    }

    #[test]
    fn missing_map_attribute_fails() {
        // Remove the "width" attribute from the map line.
        let tmx = minimal_tmx().replace("width=\"4\" height=\"4\"", "height=\"4\"");
        let err = validate_tmx(&tmx).unwrap_err();
        assert!(err.to_string().contains("\"width\""));
    }

    #[test]
    fn missing_map_close_tag_fails() {
        let tmx = minimal_tmx().replace("</map>", "");
        let err = validate_tmx(&tmx).unwrap_err();
        assert!(err.to_string().contains("</map>"));
    }

    #[test]
    fn invalid_infinite_value_fails() {
        // Replace infinite="0" with infinite="2" (not a valid value).
        let tmx = minimal_tmx().replace("infinite=\"0\"", "infinite=\"2\"");
        let err = validate_tmx(&tmx).unwrap_err();
        assert!(err.to_string().contains("infinite"));
    }

    #[test]
    fn tileset_missing_firstgid_fails() {
        let tmx = minimal_tmx().replace(
            "firstgid=\"1\" source=\"dungeon.tsx\"",
            "source=\"dungeon.tsx\"",
        );
        let err = validate_tmx(&tmx).unwrap_err();
        assert!(err.to_string().contains("firstgid"));
    }

    #[test]
    fn tileset_missing_source_fails() {
        let tmx = minimal_tmx().replace("firstgid=\"1\" source=\"dungeon.tsx\"", "firstgid=\"1\"");
        let err = validate_tmx(&tmx).unwrap_err();
        assert!(err.to_string().contains("source"));
    }

    #[test]
    fn first_firstgid_not_one_fails() {
        let tmx = minimal_tmx().replace(
            "firstgid=\"1\" source=\"dungeon.tsx\"",
            "firstgid=\"2\" source=\"dungeon.tsx\"",
        );
        let err = validate_tmx(&tmx).unwrap_err();
        assert!(err.to_string().contains("firstgid"));
    }

    #[test]
    fn non_monotonic_firstgids_fail() {
        let tmx = minimal_tmx().replace(
            "  <tileset firstgid=\"1\" source=\"dungeon.tsx\"/>",
            "  <tileset firstgid=\"1\" source=\"dungeon.tsx\"/>\n  <tileset firstgid=\"1\" source=\"forest.tsx\"/>",
        );
        let err = validate_tmx(&tmx).unwrap_err();
        assert!(err.to_string().contains("increasing"));
    }

    #[test]
    fn two_tilesets_with_correct_firstgids_pass() {
        // First tileset tile_count=6 → secondfirstgid=6.
        let tmx = minimal_tmx().replace(
            "  <tileset firstgid=\"1\" source=\"dungeon.tsx\"/>",
            "  <tileset firstgid=\"1\" source=\"dungeon.tsx\"/>\n  <tileset firstgid=\"6\" source=\"forest.tsx\"/>",
        );
        validate_tmx(&tmx).unwrap();
    }

    #[test]
    fn layer_missing_id_fails() {
        let tmx = minimal_tmx().replace("id=\"1\" name=\"ground\"", "name=\"ground\"");
        let err = validate_tmx(&tmx).unwrap_err();
        assert!(err.to_string().contains("\"id\""));
    }

    #[test]
    fn layer_missing_data_encoding_fails() {
        let tmx = minimal_tmx().replace("encoding=\"csv\"", "encoding=\"base64\"");
        let err = validate_tmx(&tmx).unwrap_err();
        assert!(err.to_string().contains("csv"));
    }

    #[test]
    fn empty_layer_list_passes_validation() {
        // A TMX with no layers is valid (Tiled allows empty maps).
        let tmx = [
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>",
            "<map version=\"1.10\" tiledversion=\"1.10.0\"",
            "     orientation=\"orthogonal\" renderorder=\"right-down\"",
            "     width=\"0\" height=\"0\"",
            "     tilewidth=\"16\" tileheight=\"16\"",
            "     infinite=\"0\" nextlayerid=\"1\" nextobjectid=\"1\">",
            "",
            "  <tileset firstgid=\"1\" source=\"dungeon.tsx\"/>",
            "",
            "</map>",
        ]
        .join("\n");
        validate_tmx(&tmx).unwrap();
    }

    #[test]
    fn no_tilesets_and_no_layers_passes_validation() {
        let tmx = [
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>",
            "<map version=\"1.10\" tiledversion=\"1.10.0\"",
            "     orientation=\"orthogonal\" renderorder=\"right-down\"",
            "     width=\"0\" height=\"0\"",
            "     tilewidth=\"0\" tileheight=\"0\"",
            "     infinite=\"0\" nextlayerid=\"1\" nextobjectid=\"1\">",
            "",
            "</map>",
        ]
        .join("\n");
        validate_tmx(&tmx).unwrap();
    }
}
