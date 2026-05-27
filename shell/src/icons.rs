//! Named icon glyphs, aliased from the bundled Phosphor font.
//!
//! The font itself is registered in [`crate::theme`] via
//! `egui_phosphor::add_to_fonts`, which appends it to egui's proportional
//! fallback chain — so any string containing one of these constants renders the
//! glyph inline. Call sites reference `icons::EYE` rather than a raw codepoint
//! or an emoji literal (emoji are not in the font chain and render as tofu
//! boxes). Centralising the aliases here keeps the icon vocabulary in one place
//! and lets the whole app switch Phosphor weight in a single edit.
//!
//! Phosphor is MIT-licensed; see `THIRD_PARTY_NOTICES.md`.

use egui_phosphor::regular as p;

// Tools.
pub const PENCIL: &str = p::PENCIL;
pub const ERASER: &str = p::ERASER;
pub const FILL: &str = p::PAINT_BUCKET;
pub const LINE: &str = p::LINE_SEGMENT;
pub const RECT: &str = p::RECTANGLE;
pub const ELLIPSE: &str = p::CIRCLE;
pub const PICKER: &str = p::EYEDROPPER;
pub const MARQUEE: &str = p::SELECTION;
pub const ELLIPSE_SELECT: &str = p::SELECTION_INVERSE;
pub const LASSO: &str = p::LASSO;
pub const WAND: &str = p::MAGIC_WAND;
pub const MOVE: &str = p::ARROWS_OUT_CARDINAL;

// Layers.
pub const EYE: &str = p::EYE;
pub const EYE_OFF: &str = p::EYE_SLASH;
pub const LOCK: &str = p::LOCK;
pub const UNLOCK: &str = p::LOCK_OPEN;
pub const TRASH: &str = p::TRASH;
pub const GROUP: &str = p::FOLDER_SIMPLE;
pub const ADD: &str = p::PLUS;
pub const REMOVE: &str = p::MINUS;
pub const UP: &str = p::CARET_UP;
pub const DOWN: &str = p::CARET_DOWN;

// Cels / linking.
pub const LINK: &str = p::LINK;
pub const UNLINK: &str = p::LINK_BREAK;

// Canvas operations.
pub const RESIZE: &str = p::RESIZE;
pub const FLIP_H: &str = p::FLIP_HORIZONTAL;
pub const FLIP_V: &str = p::FLIP_VERTICAL;
pub const ROTATE_CW: &str = p::ARROWS_CLOCKWISE;
pub const ROTATE_CCW: &str = p::ARROWS_COUNTER_CLOCKWISE;
pub const RENAME: &str = p::PENCIL_SIMPLE;
pub const DUPLICATE: &str = p::COPY;

// Transport.
pub const PLAY: &str = p::PLAY;
pub const PAUSE: &str = p::PAUSE;
pub const STOP: &str = p::STOP;
pub const PREV: &str = p::SKIP_BACK;
pub const NEXT: &str = p::SKIP_FORWARD;

// Colour / misc.
pub const SWAP: &str = p::SWAP;
pub const UNDO: &str = p::ARROW_COUNTER_CLOCKWISE;
pub const REDO: &str = p::ARROW_CLOCKWISE;
pub const LAYERS: &str = p::STACK;
pub const PALETTE: &str = p::PALETTE;
pub const SPARKLE: &str = p::SPARKLE;

// Create cockpit.
pub const DICE: &str = p::DICE_FIVE;
pub const ANCHOR: &str = p::ANCHOR;
pub const BRANCH: &str = p::GIT_BRANCH;
pub const IMAGE: &str = p::IMAGE;
pub const COPY: &str = p::COPY;
pub const CHECK: &str = p::CHECK;
pub const INFO: &str = p::INFO;
pub const CARD: &str = p::IDENTIFICATION_CARD;
pub const LIBRARY: &str = p::BOOKS;
pub const FILM: &str = p::FILM_STRIP;
