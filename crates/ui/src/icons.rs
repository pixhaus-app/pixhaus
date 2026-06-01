//! Phosphor glyph `char` constants used across the shell.
//!
//! Every constant here re-aliases an `egui_phosphor::regular::*` glyph (shipped
//! as a one-char `&str`) into the `char` form that `PanelMeta`/`ToolMeta`/
//! `WorkspaceMeta` icons require. No emoji literals anywhere: egui's default
//! fonts render emoji as tofu, and phosphor private-use codepoints render blank
//! until `theme::fonts::install_fonts` merges the phosphor family.
//!
//! `crate::icons` is crate-private; later layers reference these as
//! `crate::icons::PENCIL`. The `allow(dead_code)` keeps the build warning-clean
//! while the consuming layers are still landing.
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
        // single-byte ASCII (the `X` close glyph among others)
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
pub const PENCIL: char = glyph(ph::PENCIL);
pub const ERASER: char = glyph(ph::ERASER);
pub const FILL: char = glyph(ph::PAINT_BUCKET);
pub const LINE: char = glyph(ph::LINE_SEGMENT);
pub const RECT: char = glyph(ph::RECTANGLE);
pub const ELLIPSE: char = glyph(ph::CIRCLE);
pub const EYEDROPPER: char = glyph(ph::EYEDROPPER);
pub const SELECT: char = glyph(ph::SELECTION);
pub const LASSO: char = glyph(ph::LASSO);
pub const MOVE: char = glyph(ph::ARROWS_OUT_CARDINAL);
pub const TRANSFORM: char = glyph(ph::FRAME_CORNERS);
pub const TEXT: char = glyph(ph::TEXT_T);
pub const HAND: char = glyph(ph::HAND);
pub const ZOOM: char = glyph(ph::MAGNIFYING_GLASS);
/// The AI sparkle marker. Used wherever `AccentTokens::ai` applies.
pub const SPARKLE: char = glyph(ph::SPARKLE);

// --- Panel / dock glyphs ---
pub const LAYERS: char = glyph(ph::STACK);
pub const SPRITES: char = glyph(ph::IMAGES);
pub const PALETTE: char = glyph(ph::PALETTE);
pub const FRAMES: char = glyph(ph::FILM_STRIP);
pub const ASSETS: char = glyph(ph::SQUARES_FOUR);
pub const CONSOLE: char = glyph(ph::TERMINAL);
pub const TIMELINE: char = glyph(ph::FILM_SLATE);
pub const TILESET: char = glyph(ph::GRID_NINE);
pub const PROMPT: char = glyph(ph::MAGIC_WAND);
pub const RESULTS: char = glyph(ph::IMAGE);
pub const HISTORY: char = glyph(ph::LIST_BULLETS);
pub const EXPORT: char = glyph(ph::EXPORT);
pub const SETTINGS: char = glyph(ph::GEAR);

// --- Workspace tab glyphs ---
pub const DRAW: char = PENCIL;
pub const ANIMATE: char = glyph(ph::FILM_STRIP);
pub const TILES: char = glyph(ph::GRID_FOUR);
pub const GENERATE: char = SPARKLE;
pub const EXPORT_WS: char = glyph(ph::EXPORT);

// --- Status / menu / control glyphs ---
pub const EYE: char = glyph(ph::EYE);
pub const EYE_OFF: char = glyph(ph::EYE_SLASH);
pub const LOCK: char = glyph(ph::LOCK);
pub const LOCK_OPEN: char = glyph(ph::LOCK_OPEN);
pub const ADD: char = glyph(ph::PLUS);
pub const CARET_DOWN: char = glyph(ph::CARET_DOWN);
pub const CARET_RIGHT: char = glyph(ph::CARET_RIGHT);
pub const STATUS_DOT: char = glyph(ph::CIRCLE);
pub const CHECK: char = glyph(ph::CHECK_CIRCLE);
pub const WARN: char = glyph(ph::WARNING);
pub const CLOSE: char = glyph(ph::X);
pub const STAR: char = glyph(ph::STAR);
pub const PLAY: char = glyph(ph::PLAY);
pub const PREV: char = glyph(ph::SKIP_BACK);
pub const NEXT: char = glyph(ph::SKIP_FORWARD);
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
            PLAY,
            PREV,
            NEXT,
            CROP,
        ] {
            assert_ne!(c, '\u{fffd}', "alias decoded to the replacement char");
        }
    }

    /// The ASCII branch of `glyph` is exercised by `X` (single-byte close glyph)
    /// and the multi-byte branches by the private-use phosphor glyphs.
    #[test]
    fn glyph_decodes_ascii_and_multibyte() {
        assert_eq!(glyph("X"), 'X');
        // PENCIL is a 3-byte private-use codepoint; round-trips to one char.
        assert_eq!(PENCIL.len_utf8(), ph::PENCIL.len());
    }
}
