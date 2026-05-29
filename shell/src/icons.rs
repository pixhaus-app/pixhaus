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
pub const COLOR_RANGE: &str = p::EYEDROPPER_SAMPLE;
pub const MOVE: &str = p::ARROWS_OUT_CARDINAL;
pub const TRANSFORM: &str = p::FRAME_CORNERS;

// Selection-combine modes (the add/subtract/intersect segmented control).
pub const SELECT_REPLACE: &str = p::SELECTION;
pub const SELECT_ADD: &str = p::SELECTION_PLUS;
pub const SELECT_SUBTRACT: &str = p::SUBTRACT;
pub const SELECT_INTERSECT: &str = p::INTERSECT;

// Select menu commands.
pub const SELECT_ALL: &str = p::SELECTION_ALL;
pub const SELECT_NONE: &str = p::SELECTION_SLASH;
pub const SELECT_INVERT: &str = p::SELECTION_INVERSE;
pub const SELECT_GROW: &str = p::ARROWS_OUT;
pub const SELECT_SHRINK: &str = p::ARROWS_IN;
pub const SELECT_FEATHER: &str = p::FEATHER;

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
pub const RIGHT: &str = p::CARET_RIGHT;
pub const LEFT: &str = p::CARET_LEFT;

// Cels / linking.
pub const LINK: &str = p::LINK;
pub const UNLINK: &str = p::LINK_BREAK;

// Frame clipboard / reorder (timeline context menu).
pub const CUT: &str = p::SCISSORS;
pub const PASTE: &str = p::CLIPBOARD;
pub const REVERSE: &str = p::ARROWS_LEFT_RIGHT;

// Clipping masks: a layer clipped to the one below it.
pub const CLIP: &str = p::ARROW_BEND_DOWN_LEFT;

// Canvas operations.
pub const RESIZE: &str = p::RESIZE;
pub const FLIP_H: &str = p::FLIP_HORIZONTAL;
pub const FLIP_V: &str = p::FLIP_VERTICAL;
pub const ROTATE_CW: &str = p::ARROWS_CLOCKWISE;
pub const ROTATE_CCW: &str = p::ARROWS_COUNTER_CLOCKWISE;
pub const RENAME: &str = p::PENCIL_SIMPLE;
pub const DUPLICATE: &str = p::COPY;

// Transform menu. The Phosphor set has no dedicated skew or perspective glyph,
// so these reuse the nearest geometric icons.
pub const ROTATE_FREE: &str = p::ARROW_CLOCKWISE;
pub const SKEW: &str = p::FLIP_HORIZONTAL;
pub const CROP: &str = p::CROP;
pub const PERSPECTIVE: &str = p::PERSPECTIVE;
pub const ANTIALIAS: &str = p::SCRIBBLE;

// Transport.
pub const PLAY: &str = p::PLAY;
pub const PAUSE: &str = p::PAUSE;
pub const STOP: &str = p::STOP;
pub const PREV: &str = p::SKIP_BACK;
pub const NEXT: &str = p::SKIP_FORWARD;
pub const REPEAT: &str = p::REPEAT;

// Colour / misc.
// Used only by the `lospec` feature's import button; dead in the default build.
#[cfg_attr(not(feature = "lospec"), allow(dead_code))]
pub const DOWNLOAD: &str = p::DOWNLOAD_SIMPLE;
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
/// Reject / dismiss (suggested-tag reject chip, close affordances).
pub const X: &str = p::X;
pub const INFO: &str = p::INFO;
pub const CARD: &str = p::IDENTIFICATION_CARD;
pub const LIBRARY: &str = p::BOOKS;
pub const FILM: &str = p::FILM_STRIP;
/// Library tag chip / tag-manager section.
pub const TAG: &str = p::TAG;
/// Promote a variant to a final, high-quality render.
pub const PROMOTE: &str = p::SEAL_CHECK;
/// Import an existing image as a sheet variant.
pub const UPLOAD: &str = p::UPLOAD_SIMPLE;
/// Project AI-defaults section header.
pub const AI_DEFAULTS: &str = p::SLIDERS;
