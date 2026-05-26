//! Interactive-editing state: the active tools, brush, colors, selection,
//! onion-skin settings, undo history, and the per-gesture session structs.
//!
//! This is the hub the canvas input router and the editing panels read and
//! mutate. It lives as a plain field on [`crate::app::ShellApp`] next to the
//! [`crate::document::DocumentStore`], so an editing action borrows the two as
//! disjoint fields and pushes undo entries with
//! `editor.history.push(cmd, &mut doc)`.

use eframe::egui;
use pixhaus_core::canvas::{BrushShape, PixelBuffer};
use pixhaus_core::project::{IVec2, LayerId, PixelBufferId, Rgba};
use pixhaus_core::selection::SelectionMask;
use pixhaus_core::undo::History;

use crate::document::DocumentStore;

/// The editing tools. Pencil through colour-picker paint; the rest select or
/// move. Adopted from Pixelorama's tool taxonomy (design vs selection vs
/// utility); see `THIRD_PARTY_NOTICES.md`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    /// Freehand brush in the foreground colour.
    Pencil,
    /// Freehand brush painting transparency.
    Eraser,
    /// Flood fill from the clicked pixel.
    Fill,
    /// Straight line, previewed while dragging.
    Line,
    /// Rectangle outline (filled with Shift).
    Rectangle,
    /// Ellipse outline (filled with Shift).
    Ellipse,
    /// Sample a canvas pixel into the foreground colour.
    Picker,
    /// Rectangular marquee selection.
    SelectRect,
    /// Elliptical marquee selection.
    SelectEllipse,
    /// Freehand polygon (lasso) selection.
    Lasso,
    /// Magic-wand flood selection by colour.
    Wand,
    /// Move the current selection's pixels.
    Move,
}

impl Tool {
    /// Whether the tool paints into a cel (so it needs a drawable cel and an
    /// undo entry on commit).
    #[must_use]
    pub fn paints(self) -> bool {
        matches!(self, Tool::Pencil | Tool::Eraser | Tool::Fill | Tool::Line | Tool::Rectangle | Tool::Ellipse)
    }
}

/// How the palette panel sorts swatches when asked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaletteSort {
    /// Sort by hue.
    Hue,
    /// Sort by perceived lightness.
    Luminance,
}

/// Onion-skin configuration. Fixed-offset (mobile) ghosts of the neighbouring
/// frames render under the current frame, tinted and faded by distance.
/// Adopted from `OpenToonz`'s mobile onion-skin (`OnionSkinMask`); see
/// `THIRD_PARTY_NOTICES.md`.
#[derive(Clone, Copy, Debug)]
pub struct OnionConfig {
    /// Whether onion skinning is on.
    pub enabled: bool,
    /// Number of previous frames to ghost.
    pub prev: u32,
    /// Number of following frames to ghost.
    pub next: u32,
    /// Opacity of the nearest ghost (0..1); further ghosts fade from here.
    pub opacity: f32,
    /// Tint applied to previous frames.
    pub prev_tint: Rgba,
    /// Tint applied to following frames.
    pub next_tint: Rgba,
}

impl Default for OnionConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            prev: 1,
            next: 1,
            opacity: 0.4,
            prev_tint: Rgba::opaque(255, 80, 80),
            next_tint: Rgba::opaque(80, 160, 255),
        }
    }
}

/// An in-progress freehand stroke (pencil/eraser). Accumulates dirty bounds so
/// the GPU upload and the undo snapshot are both bounded by the painted region,
/// not the canvas — the 8K constraint.
pub struct StrokeSession {
    /// Buffer being painted.
    pub buffer_id: PixelBufferId,
    /// Full clone of the buffer at stroke start, the source of the undo
    /// "before" region. Transient: dropped when the stroke commits.
    pub before: PixelBuffer,
    /// Every canvas point visited, for the pixel-perfect commit redraw.
    pub points: Vec<[f32; 2]>,
    /// Last canvas point, to bridge Bresenham segments across frames.
    pub last_point: Option<[f32; 2]>,
    /// Paint colour (transparent for the eraser).
    pub color: Rgba,
    /// Brush footprint.
    pub shape: BrushShape,
    /// Brush size.
    pub size: u32,
    /// Mirror across the vertical centre line.
    pub mirror_x: bool,
    /// Mirror across the horizontal centre line.
    pub mirror_y: bool,
    /// Whether this is an eraser stroke (skips the auto-add-to-palette).
    pub erase: bool,
    /// Whether the pencil pixel-perfect pass applies on commit.
    pub pixel_perfect: bool,
    /// Inclusive dirty bounds in canvas pixels, or `None` until first stamp.
    pub dirty: Option<(u32, u32, u32, u32)>,
}

impl StrokeSession {
    /// Expands the dirty bounds to include the rect `(x, y, w, h)`.
    pub fn mark_dirty(&mut self, x: u32, y: u32, w: u32, h: u32) {
        if w == 0 || h == 0 {
            return;
        }
        let (x1, y1) = (x + w - 1, y + h - 1);
        self.dirty = Some(match self.dirty {
            None => (x, y, x1, y1),
            Some((ax, ay, bx, by)) => (ax.min(x), ay.min(y), bx.max(x1), by.max(y1)),
        });
    }
}

/// A one-shot shape drag (line/rectangle/ellipse): preview while dragging,
/// commit on release. The buffer is snapshotted at press so each preview frame
/// can restore-then-redraw without accumulating.
pub struct ShapeDrag {
    /// Buffer being drawn into.
    pub buffer_id: PixelBufferId,
    /// Clean snapshot restored before each preview redraw.
    pub before: PixelBuffer,
    /// Press point in canvas pixels.
    pub start: [i32; 2],
    /// Current pointer point in canvas pixels.
    pub current: [i32; 2],
}

/// An in-progress move of the selected pixels.
pub struct MoveDrag {
    /// Buffer being moved within.
    pub buffer_id: PixelBufferId,
    /// Buffer contents before the move (for undo and per-frame restore).
    pub before: PixelBuffer,
    /// Lifted pixels (the selection's content at press), as a full-canvas
    /// buffer with everything outside the selection transparent.
    pub lifted: PixelBuffer,
    /// Press point in canvas pixels.
    pub start: [i32; 2],
    /// Accumulated integer offset.
    pub offset: [i32; 2],
}

/// All interactive-editing state, owned by [`crate::app::ShellApp`].
pub struct EditorState {
    /// Tool bound to the primary (left) mouse button.
    pub left_tool: Tool,
    /// Tool bound to the secondary (right) mouse button (Pixelorama-style dual
    /// assignment).
    pub right_tool: Tool,
    /// Brush footprint shape.
    pub brush_shape: BrushShape,
    /// Brush size in pixels.
    pub brush_size: u32,
    /// Aseprite-style pixel-perfect freehand (pencil only).
    pub pixel_perfect: bool,
    /// Mirror strokes across the canvas vertical centre line.
    pub mirror_x: bool,
    /// Mirror strokes across the canvas horizontal centre line.
    pub mirror_y: bool,
    /// Per-channel fill/wand tolerance.
    pub tolerance: u8,
    /// Foreground colour (left-button paint).
    pub fg: Rgba,
    /// Background colour (right-button paint, colour swap).
    pub bg: Rgba,
    /// Add freshly-painted colours to the active palette (Pixelorama).
    pub auto_add_palette: bool,
    /// Branching undo history over the document.
    pub history: History<DocumentStore>,
    /// In-progress freehand stroke.
    pub stroke: Option<StrokeSession>,
    /// In-progress shape drag.
    pub shape_drag: Option<ShapeDrag>,
    /// In-progress lasso polygon (canvas points).
    pub lasso: Vec<IVec2>,
    /// In-progress marquee drag (rect / ellipse selection): `(start, current)`
    /// in canvas pixels.
    pub sel_drag: Option<([i32; 2], [i32; 2])>,
    /// In-progress move drag.
    pub move_drag: Option<MoveDrag>,
    /// Current selection mask (canvas-sized), or `None` when nothing is
    /// selected (the whole canvas is editable).
    pub selection: Option<SelectionMask>,
    /// Onion-skin settings.
    pub onion: OnionConfig,
    /// Palette panel: lock the grid against accidental reordering.
    pub lock_palette_grid: bool,
    /// Palette panel: swatch edge length in points.
    pub swatch_size: f32,
    /// Palette panel: index being edited in the colour popup, if any.
    pub editing_swatch: Option<usize>,
    /// Timeline: cel-thumbnail edge length in points.
    pub cel_size: f32,
    /// Timeline: draft name for a new frame tag.
    pub new_tag_name: String,
    /// Layers panel: in-progress inline rename `(layer, draft)`.
    pub layer_rename: Option<(LayerId, String)>,
}

impl Default for EditorState {
    fn default() -> Self {
        Self {
            left_tool: Tool::Pencil,
            right_tool: Tool::Eraser,
            brush_shape: BrushShape::Pixel,
            brush_size: 1,
            pixel_perfect: true,
            mirror_x: false,
            mirror_y: false,
            tolerance: 0,
            fg: Rgba::opaque(20, 20, 28),
            bg: Rgba::transparent(),
            auto_add_palette: true,
            history: History::new(),
            stroke: None,
            shape_drag: None,
            lasso: Vec::new(),
            sel_drag: None,
            move_drag: None,
            selection: None,
            onion: OnionConfig::default(),
            lock_palette_grid: false,
            swatch_size: 18.0,
            editing_swatch: None,
            cel_size: 48.0,
            new_tag_name: String::new(),
            layer_rename: None,
        }
    }
}

impl EditorState {
    /// The tool to use for a pointer button: primary -> left tool, secondary ->
    /// right tool.
    #[must_use]
    pub fn tool_for(&self, primary: bool) -> Tool {
        if primary { self.left_tool } else { self.right_tool }
    }

    /// The colour a button paints: primary -> foreground, secondary ->
    /// background.
    #[must_use]
    pub fn color_for(&self, primary: bool) -> Rgba {
        if primary { self.fg } else { self.bg }
    }

    /// Swaps foreground and background colours (the `X` shortcut).
    pub fn swap_colors(&mut self) {
        std::mem::swap(&mut self.fg, &mut self.bg);
    }

    /// Drops the current selection (deselect-all).
    pub fn clear_selection(&mut self) {
        self.selection = None;
        self.lasso.clear();
    }
}

/// Converts a core [`Rgba`] to an egui [`egui::Color32`] (unmultiplied).
#[must_use]
pub fn to_color32(c: Rgba) -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a)
}

/// Converts an egui [`egui::Color32`] to a core [`Rgba`] (unmultiplied).
#[must_use]
pub fn from_color32(c: egui::Color32) -> Rgba {
    let [r, g, b, a] = c.to_array();
    Rgba::new(r, g, b, a)
}
