//! Phosphor glyph `char` constants used across the shell.
//!
//! Every constant here re-aliases an `egui_phosphor::regular::*` glyph (shipped
//! as a one-char `&str`) into the `char` form that `PanelMeta`/`ToolMeta`/
//! `WorkspaceMeta` icons require. No emoji literals anywhere: egui's default
//! fonts render emoji as tofu, and phosphor private-use codepoints render blank
//! until `theme::fonts::install_fonts` merges the phosphor family.
//!
//! `crate::icons` is the shared glyph table the shell and the module crates both
//! reach for, as `crate::icons::PENCIL` inside `ui` and `pixhaus_ui::icons::PENCIL`
//! from a module. The `allow(dead_code)` keeps the build warning-clean while not
//! every constant has a consumer yet.
#![allow(dead_code)]

use egui_phosphor::regular as ph;

/// First `char` of a phosphor glyph string, evaluated at compile time.
///
/// Phosphor ships each glyph as a single-`char` `&str`; our metadata structs
/// take `char`. A phosphor glyph is one private-use scalar, so the first decoded
/// `char` is the glyph.
const fn glyph(s: &str) -> char {
    // `str::as_bytes` is const; decode the leading UTF-8 sequence to one scalar.
    let b = s.as_bytes();
    let first = b[0];
    if first < 0x80 {
        // Single-byte ASCII. No phosphor glyph hits this branch - phosphor
        // glyphs are all private-use codepoints (U+E000+), decoded by the
        // 3-byte branch below. This branch only fires for a literal ASCII arg
        // (the `glyph("X")` test case).
        first as char
    } else if first >> 5 == 0b110 {
        let cp = ((first as u32 & 0x1f) << 6) | (b[1] as u32 & 0x3f);
        match char::from_u32(cp) {
            Some(c) => c,
            None => '\u{fffd}',
        }
    } else if first >> 4 == 0b1110 {
        let cp = ((first as u32 & 0x0f) << 12) | ((b[1] as u32 & 0x3f) << 6) | (b[2] as u32 & 0x3f);
        match char::from_u32(cp) {
            Some(c) => c,
            None => '\u{fffd}',
        }
    } else {
        let cp = ((first as u32 & 0x07) << 18) | ((b[1] as u32 & 0x3f) << 12) | ((b[2] as u32 & 0x3f) << 6) | (b[3] as u32 & 0x3f);
        match char::from_u32(cp) {
            Some(c) => c,
            None => '\u{fffd}',
        }
    }
}

// --- Tool-rail glyphs (spec tool inventory) ---
/// Pencil tool.
pub const PENCIL: char = glyph(ph::PENCIL);
/// Eraser tool.
pub const ERASER: char = glyph(ph::ERASER);
/// Flood-fill tool.
pub const FILL: char = glyph(ph::PAINT_BUCKET);
/// Line tool.
pub const LINE: char = glyph(ph::LINE_SEGMENT);
/// Rectangle tool.
pub const RECT: char = glyph(ph::RECTANGLE);
/// Ellipse tool.
pub const ELLIPSE: char = glyph(ph::CIRCLE);
/// Eyedropper (color-pick) tool.
pub const EYEDROPPER: char = glyph(ph::EYEDROPPER);
/// Marquee selection tool.
pub const SELECT: char = glyph(ph::SELECTION);
/// Freeform lasso selection tool.
pub const LASSO: char = glyph(ph::LASSO);
/// Move tool.
pub const MOVE: char = glyph(ph::ARROWS_OUT_CARDINAL);
/// Transform (scale/rotate/skew) tool.
pub const TRANSFORM: char = glyph(ph::FRAME_CORNERS);
/// Pixel-text tool.
pub const TEXT: char = glyph(ph::TEXT_T);
/// Pan (hand) tool.
pub const HAND: char = glyph(ph::HAND);
/// Zoom tool.
pub const ZOOM: char = glyph(ph::MAGNIFYING_GLASS);
/// The AI sparkle marker. Used wherever `AccentTokens::ai` applies.
pub const SPARKLE: char = glyph(ph::SPARKLE);

// --- Panel / dock glyphs ---
/// Layers panel.
pub const LAYERS: char = glyph(ph::STACK);
/// Sprites panel.
pub const SPRITES: char = glyph(ph::IMAGES);
/// Palette panel.
pub const PALETTE: char = glyph(ph::PALETTE);
/// Frames panel/tray.
pub const FRAMES: char = glyph(ph::FILM_STRIP);
/// Assets panel/tray.
pub const ASSETS: char = glyph(ph::SQUARES_FOUR);
/// Console panel/tray.
pub const CONSOLE: char = glyph(ph::TERMINAL);
/// Timeline panel.
pub const TIMELINE: char = glyph(ph::FILM_SLATE);
/// Tileset panel.
pub const TILESET: char = glyph(ph::GRID_NINE);
/// Prompt-composer panel.
pub const PROMPT: char = glyph(ph::MAGIC_WAND);
/// Generation-results panel.
pub const RESULTS: char = glyph(ph::IMAGE);
/// History panel.
pub const HISTORY: char = glyph(ph::LIST_BULLETS);
/// Export panel.
pub const EXPORT: char = glyph(ph::EXPORT);
/// Settings panel.
pub const SETTINGS: char = glyph(ph::GEAR);

// --- Workspace tab glyphs ---
/// Draw workspace tab.
pub const DRAW: char = PENCIL;
/// Animate workspace tab.
pub const ANIMATE: char = glyph(ph::FILM_STRIP);
/// Tiles workspace tab.
pub const TILES: char = glyph(ph::GRID_FOUR);
/// Generate workspace tab.
pub const GENERATE: char = SPARKLE;
/// Export workspace tab.
pub const EXPORT_WS: char = glyph(ph::EXPORT);

// --- Status / menu / control glyphs ---
/// Visibility-on toggle.
pub const EYE: char = glyph(ph::EYE);
/// Visibility-off toggle.
pub const EYE_OFF: char = glyph(ph::EYE_SLASH);
/// Locked toggle.
pub const LOCK: char = glyph(ph::LOCK);
/// Unlocked toggle.
pub const LOCK_OPEN: char = glyph(ph::LOCK_OPEN);
/// Add / new affordance.
pub const ADD: char = glyph(ph::PLUS);
/// Expanded-section caret.
pub const CARET_DOWN: char = glyph(ph::CARET_DOWN);
/// Collapsed-section caret.
pub const CARET_RIGHT: char = glyph(ph::CARET_RIGHT);
/// The status-bar status dot.
pub const STATUS_DOT: char = glyph(ph::CIRCLE);
/// Success / done marker.
pub const CHECK: char = glyph(ph::CHECK_CIRCLE);
/// Warning marker.
pub const WARN: char = glyph(ph::WARNING);
/// Close / dismiss control.
pub const CLOSE: char = glyph(ph::X);
/// Favorite / star marker.
pub const STAR: char = glyph(ph::STAR);
/// The pixel-grid status glyph (the "Pixel Grid On" status item).
pub const GRID: char = glyph(ph::GRID_FOUR);
/// Playback play control.
pub const PLAY: char = glyph(ph::PLAY);
/// Playback previous-frame control.
pub const PREV: char = glyph(ph::SKIP_BACK);
/// Playback next-frame control.
pub const NEXT: char = glyph(ph::SKIP_FORWARD);
/// Crop control.
pub const CROP: char = glyph(ph::CROP);

#[cfg(test)]
mod tests {
    use super::*;

    /// Every alias must decode to the same scalar phosphor ships in the string,
    /// i.e. `glyph` is a faithful first-char extractor, not a corruption. The
    /// helper pattern-matches the first char rather than `unwrap`-ing it: the
    /// repo bans `unwrap`/`expect` via clippy's disallowed-methods, even in tests.
    #[test]
    fn aliases_match_phosphor_strings() {
        let first = |s: &str| match s.chars().next() {
            Some(c) => c,
            None => panic!("phosphor string was empty"),
        };
        assert_eq!(PENCIL, first(ph::PENCIL));
        assert_eq!(SPARKLE, first(ph::SPARKLE));
        assert_eq!(LAYERS, first(ph::STACK));
        assert_eq!(EXPORT, first(ph::EXPORT));
        assert_eq!(CLOSE, first(ph::X));
    }

    /// No alias decoded to the replacement char - that would mean a malformed
    /// decode path, not a real glyph.
    #[test]
    fn no_alias_is_the_replacement_char() {
        for c in [
            PENCIL,
            ERASER,
            FILL,
            LINE,
            RECT,
            ELLIPSE,
            EYEDROPPER,
            SELECT,
            LASSO,
            MOVE,
            TRANSFORM,
            TEXT,
            HAND,
            ZOOM,
            SPARKLE,
            LAYERS,
            SPRITES,
            PALETTE,
            FRAMES,
            ASSETS,
            CONSOLE,
            TIMELINE,
            TILESET,
            PROMPT,
            RESULTS,
            HISTORY,
            EXPORT,
            SETTINGS,
            EYE,
            EYE_OFF,
            LOCK,
            LOCK_OPEN,
            ADD,
            CARET_DOWN,
            CARET_RIGHT,
            STATUS_DOT,
            CHECK,
            WARN,
            CLOSE,
            STAR,
            GRID,
            PLAY,
            PREV,
            NEXT,
            CROP,
        ] {
            assert_ne!(c, '\u{fffd}', "alias decoded to the replacement char");
        }
    }

    /// `glyph` decodes each UTF-8 length. Every phosphor glyph is a private-use
    /// codepoint (U+E000+), so they all take the 3-byte branch; the 1-byte and
    /// 2-byte branches only ever fire for a literal ASCII or 2-byte arg, so the
    /// test feeds those directly to keep those branches covered.
    #[test]
    fn glyph_decodes_each_utf8_length() {
        // 1-byte ASCII: only reached by a literal ASCII arg, never by phosphor.
        assert_eq!(glyph("X"), 'X');
        // 2-byte UTF-8 (U+00E9, "e-acute"): exercises the 2-byte branch, which
        // no phosphor glyph reaches.
        assert_eq!(glyph("\u{00e9}"), '\u{00e9}');
        // 3-byte UTF-8: every phosphor glyph lands here. PENCIL round-trips to
        // one private-use char.
        assert_eq!(PENCIL.len_utf8(), ph::PENCIL.len());
        assert_eq!(PENCIL.len_utf8(), 3);
    }
}
