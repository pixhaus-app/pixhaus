//! Lospec palette browser and importer.
//!
//! Uses the undocumented but stable Lospec JSON endpoint:
//! `https://lospec.com/palette-list/{slug}.json`
//!
//! The response contains the palette name and an array of lowercase
//! six-char hex color strings.
//!
//! # Blocking client
//!
//! This module exposes a **blocking** API. Call it from a Tokio
//! `spawn_blocking` task when invoked from an async context.

use pixhaus_core::project::color::Rgba;
use serde::Deserialize;

use crate::error::{Error, Result};
use crate::palette::hex;

// ── Response types ────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct LospecPalette {
    name: String,
    colors: Vec<String>,
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Fetches a Lospec palette by its URL slug and returns `(name, colors)`.
///
/// The `slug` is the last path component of the palette's Lospec URL —
/// for example, `"aap-16"` for
/// `https://lospec.com/palette-list/aap-16`.
///
/// Network errors and unexpected API responses are surfaced as [`Error`].
/// This function performs a **blocking** HTTP request; run it in
/// `tokio::task::spawn_blocking` from async contexts.
pub fn fetch_palette(
    client: &reqwest::blocking::Client,
    slug: &str,
) -> Result<(String, Vec<Rgba>)> {
    let url = format!("https://lospec.com/palette-list/{slug}.json");

    let response = client
        .get(&url)
        .header("User-Agent", "pixhaus/0.1 (https://pixhaus.app)")
        .send()?;

    if !response.status().is_success() {
        let status = response.status();
        return Err(Error::LospecApi(format!(
            "palette '{slug}' not found (HTTP {status})"
        )));
    }

    let data: LospecPalette = response
        .json()
        .map_err(|e| Error::LospecApi(format!("failed to parse response: {e}")))?;

    // Parse the hex colors using the existing hex module
    let hex_input: String = data.colors.join("\n");
    let colors = hex::parse(&hex_input)
        .map_err(|e| Error::LospecApi(format!("palette color parse error: {e}")))?;

    Ok((data.name, colors))
}

/// Creates a default [`reqwest::blocking::Client`] suitable for Lospec
/// API calls, with a 10-second timeout.
pub fn default_client() -> Result<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(Error::Http)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Parse a mock response to verify the parsing logic without making real HTTP calls.
    fn mock_response_json() -> &'static str {
        r#"{"name":"Aap-16","slug":"aap-16","colors":["070708","1d2432","494d7e"]}"#
    }

    #[test]
    fn parses_lospec_json_format() {
        let parsed: LospecPalette = serde_json::from_str(mock_response_json()).unwrap();
        assert_eq!(parsed.name, "Aap-16");
        assert_eq!(parsed.colors.len(), 3);
        assert_eq!(parsed.colors[0], "070708");
    }

    #[test]
    fn colors_convert_to_rgba() {
        let hex_input = "070708\n1d2432\n494d7e\n";
        let colors = hex::parse(hex_input).unwrap();
        assert_eq!(colors.len(), 3);
        assert_eq!(colors[0], Rgba::opaque(0x07, 0x07, 0x08));
        assert_eq!(colors[1], Rgba::opaque(0x1d, 0x24, 0x32));
    }

    #[test]
    fn default_client_creates_successfully() {
        assert!(default_client().is_ok());
    }
}
