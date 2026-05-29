//! Lospec palette browser — fetch a palette by slug from lospec.com.
//!
//! Gated behind the `lospec` Cargo feature: the default build pulls in no
//! network code (no `reqwest`, no fetch, no UI). The one network feature in the
//! shell, descoped and specced last in the palette plan because it is the one
//! piece here that is not self-contained — it depends on an HTTP call that must
//! run off the egui update thread, since v2 shell is synchronous egui and a
//! blocking request would freeze the frame loop.
//!
//! The slice that ships now:
//!
//! - [`parse_lospec_json`] turns the endpoint's `{ name, colors: [hex...] }`
//!   body into `(name, Vec<Rgba>)` via [`hex::parse`]. Pure, no network, always
//!   compiled so the parse test runs in the default build.
//! - Behind the feature, [`fetch_palette`] performs the blocking GET and
//!   [`spawn_fetch`] runs it on a background thread, delivering the result back
//!   over the shell's [`ShellMsg`] channel polled in the update loop.
//! - The panel ([`ShellApp::lospec_section`]) creates a new palette named after
//!   the result and appends the colours as one `SpriteEdit`; failures surface
//!   inline.
//!
//! The parse path and slug helper are always compiled (and tested) so the
//! default build still type-checks them; `cargo test` exercises them. They are
//! only *called* from the gated fetch path, so the non-test default build marks
//! them dead — silenced rather than dropped so the test keeps covering them.

// Tested in the default build; only called from the gated fetch path.
#![cfg_attr(not(feature = "lospec"), allow(dead_code))]

use pixhaus_core::color::palette_file::hex;
use pixhaus_core::project::Rgba;
use serde::Deserialize;

/// The Lospec JSON endpoint shape: `https://lospec.com/palette-list/{slug}.json`.
///
/// The response carries the palette's display name and an array of six-char hex
/// colour strings (no `#` prefix). Other fields the endpoint returns (`slug`,
/// author, …) are ignored.
#[derive(Deserialize)]
struct LospecPalette {
    name: String,
    colors: Vec<String>,
}

/// Parses a Lospec palette JSON body into `(name, colors)`.
///
/// The colour strings are joined with newlines and run through the shared
/// [`hex::parse`] so the `.hex` format and the Lospec endpoint share one colour
/// parser. A malformed body or a bad colour string returns a human-readable
/// error string (the panel shows it inline; there is no network error to
/// distinguish here).
///
/// Pure and network-free, so the parse path is testable against a fixture
/// without a live request — and so the default build still compiles and tests
/// it even with the `lospec` feature off.
///
/// # Errors
/// Returns an error string when the body is not the expected JSON shape, or when
/// a colour string is not a six-char hex code.
pub fn parse_lospec_json(body: &str) -> Result<(String, Vec<Rgba>), String> {
    let parsed: LospecPalette = serde_json::from_str(body).map_err(|e| format!("unexpected Lospec response: {e}"))?;
    let joined = parsed.colors.join("\n");
    let colors = hex::parse(&joined).map_err(|e| format!("palette colour parse error: {e}"))?;
    Ok((parsed.name, colors))
}

/// Normalizes a user-typed slug or a full Lospec URL down to the bare slug the
/// endpoint expects — the last non-empty path component, lower-cased. A pasted
/// `https://lospec.com/palette-list/aap-16` and a bare `AAP-16` both reduce to
/// `aap-16`. Trailing slashes and a `.json` suffix are stripped.
#[must_use]
pub fn normalize_slug(input: &str) -> String {
    input
        .trim()
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or("")
        .trim_end_matches(".json")
        .to_ascii_lowercase()
}

/// Fetches a Lospec palette by slug, blocking until the request completes.
///
/// Blocking I/O — call from a background thread, never the egui update loop. The
/// `lospec` feature gates the whole `reqwest` dependency in, so this only exists
/// when the feature is on.
///
/// # Errors
/// Returns an error string on a network failure, a non-success HTTP status
/// (e.g. an unknown slug), or a body that does not parse via
/// [`parse_lospec_json`].
#[cfg(feature = "lospec")]
pub fn fetch_palette(slug: &str) -> Result<(String, Vec<Rgba>), String> {
    let url = format!("https://lospec.com/palette-list/{slug}.json");
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("could not build HTTP client: {e}"))?;
    let response = client
        .get(&url)
        .header("User-Agent", "pixhaus/0.1 (https://pixhaus.app)")
        .send()
        .map_err(|e| format!("request failed: {e}"))?;
    if !response.status().is_success() {
        let status = response.status();
        return Err(format!("palette '{slug}' not found (HTTP {status})"));
    }
    let body = response.text().map_err(|e| format!("could not read response: {e}"))?;
    parse_lospec_json(&body)
}

/// Spawns a Lospec fetch on a background OS thread, delivering the result back
/// over `tx` as a [`crate::app::ShellMsg::LospecDone`] and waking the idle UI
/// with `ctx.request_repaint()`. The shell drains the channel in its update loop
/// and lands the palette there, so no network work ever touches the egui thread.
///
/// A plain `std::thread` rather than the tokio runtime: the request is a single
/// blocking GET with no async machinery to share, and keeping it off the runtime
/// avoids coupling the one optional network feature to the AI executor.
#[cfg(feature = "lospec")]
pub fn spawn_fetch(ctx: egui::Context, tx: std::sync::mpsc::Sender<crate::app::ShellMsg>, slug: String) {
    std::thread::spawn(move || {
        let result = fetch_palette(&slug);
        let _ = tx.send(crate::app::ShellMsg::LospecDone { slug, result });
        ctx.request_repaint();
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A mock Lospec response, ported from the main checkout's `lospec.rs`
    /// `mock_response_json`. The parse path is verified against this fixture
    /// without any live request.
    fn mock_response_json() -> &'static str {
        r#"{"name":"Aap-16","slug":"aap-16","colors":["070708","1d2432","494d7e"]}"#
    }

    #[test]
    fn parses_lospec_json_into_name_and_colors() {
        let (name, colors) = parse_lospec_json(mock_response_json()).expect("parse mock body");
        assert_eq!(name, "Aap-16");
        assert_eq!(colors.len(), 3);
        assert_eq!(colors[0], Rgba::opaque(0x07, 0x07, 0x08));
        assert_eq!(colors[1], Rgba::opaque(0x1d, 0x24, 0x32));
        assert_eq!(colors[2], Rgba::opaque(0x49, 0x4d, 0x7e));
    }

    #[test]
    fn parse_accepts_hash_prefixed_colors() {
        // The shared hex parser treats `#rrggbb` as a colour, not a comment, so
        // a body whose colours carry the optional prefix still parses.
        let body = r##"{"name":"Prefixed","colors":["#ff0000","#00ff00"]}"##;
        let (name, colors) = parse_lospec_json(body).expect("parse prefixed body");
        assert_eq!(name, "Prefixed");
        assert_eq!(colors, vec![Rgba::opaque(255, 0, 0), Rgba::opaque(0, 255, 0)]);
    }

    #[test]
    fn parse_rejects_non_json() {
        assert!(parse_lospec_json("not json at all").is_err());
    }

    #[test]
    fn parse_rejects_a_bad_colour_string() {
        // A six-char field requirement: a three-char colour is rejected by the
        // hex parser, surfaced as an error rather than a silent drop.
        let body = r#"{"name":"Bad","colors":["fff"]}"#;
        assert!(parse_lospec_json(body).is_err());
    }

    #[test]
    fn parse_accepts_an_empty_colour_list() {
        let body = r#"{"name":"Empty","colors":[]}"#;
        let (name, colors) = parse_lospec_json(body).expect("parse empty body");
        assert_eq!(name, "Empty");
        assert!(colors.is_empty());
    }

    #[test]
    fn normalize_slug_reduces_a_full_url() {
        assert_eq!(normalize_slug("https://lospec.com/palette-list/aap-16"), "aap-16");
    }

    #[test]
    fn normalize_slug_lowercases_and_trims() {
        assert_eq!(normalize_slug("  AAP-16  "), "aap-16");
    }

    #[test]
    fn normalize_slug_strips_a_trailing_slash_and_json() {
        assert_eq!(normalize_slug("https://lospec.com/palette-list/nyx8.json"), "nyx8");
        assert_eq!(normalize_slug("nyx8/"), "nyx8");
    }
}
