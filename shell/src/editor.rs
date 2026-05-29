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
use crate::gizmo::{BoxGizmo, GizmoHandle};

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
    /// Select every pixel matching the clicked colour, canvas-wide (ignores
    /// contiguity, unlike the wand).
    ColorRange,
    /// Move the current selection's pixels.
    Move,
    /// Free-transform the current selection's pixels: scale, rotate, and warp
    /// the lifted region via the on-canvas box gizmo.
    Transform,
}

/// How a freshly-made mask combines with the existing selection. Resolved from
/// the keyboard modifiers held at the gesture's start (Shift -> Add, Alt ->
/// Subtract, Shift+Alt -> Intersect, none -> Replace). Transient: it never
/// persists across gestures, so the default is always Replace.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SelectionMode {
    /// Discard the old selection and keep only the new mask.
    #[default]
    Replace,
    /// Union the new mask with the old selection.
    Add,
    /// Remove the new mask from the old selection.
    Subtract,
    /// Keep only the overlap of the new mask and the old selection.
    Intersect,
}

impl SelectionMode {
    /// Resolves the mode from held modifiers: Shift -> Add, Alt -> Subtract,
    /// Shift+Alt -> Intersect, neither -> Replace. Shift+Alt wins over either
    /// alone so the intersect gesture is reachable.
    #[must_use]
    pub fn from_modifiers(modifiers: egui::Modifiers) -> Self {
        match (modifiers.shift, modifiers.alt) {
            (true, true) => Self::Intersect,
            (true, false) => Self::Add,
            (false, true) => Self::Subtract,
            (false, false) => Self::Replace,
        }
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
    /// Inclusive dirty bounds in canvas pixels over the whole stroke, or `None`
    /// until first stamp. Read once at commit to bound the undo snapshot.
    pub dirty: Option<(u32, u32, u32, u32)>,
    /// Inclusive dirty bounds accumulated *since the last upload*, or `None`
    /// when nothing is pending. Separate from [`Self::dirty`] so the per-move
    /// GPU upload and recomposite are bounded by the latest dab's footprint, not
    /// the whole stroke — drawing stays O(brush), not O(stroke), at any canvas
    /// size (the 8K constraint).
    pub pending: Option<(u32, u32, u32, u32)>,
}

impl StrokeSession {
    /// Expands both the cumulative dirty bounds and the pending (since-last-
    /// upload) bounds to include the rect `(x, y, w, h)`.
    pub fn mark_dirty(&mut self, x: u32, y: u32, w: u32, h: u32) {
        if w == 0 || h == 0 {
            return;
        }
        let (x1, y1) = (x + w - 1, y + h - 1);
        self.dirty = Some(union_bounds(self.dirty, (x, y, x1, y1)));
        self.pending = Some(union_bounds(self.pending, (x, y, x1, y1)));
    }

    /// Returns the pending dirty rect as `(x, y, w, h)` and clears it, so the
    /// next move starts fresh. `None` when nothing is pending.
    pub fn take_pending(&mut self) -> Option<(u32, u32, u32, u32)> {
        let (x, y, x1, y1) = self.pending.take()?;
        Some((x, y, x1 - x + 1, y1 - y + 1))
    }
}

/// Unions an optional inclusive bounds box with `(x, y, x1, y1)`.
fn union_bounds(current: Option<(u32, u32, u32, u32)>, (x, y, x1, y1): (u32, u32, u32, u32)) -> (u32, u32, u32, u32) {
    match current {
        None => (x, y, x1, y1),
        Some((ax, ay, bx, by)) => (ax.min(x), ay.min(y), bx.max(x1), by.max(y1)),
    }
}

/// A one-shot shape drag (line/rectangle/ellipse): preview while dragging,
/// commit on release. The buffer is snapshotted at press so each preview frame
/// can restore-then-redraw without accumulating.
pub struct ShapeDrag {
    /// Buffer being drawn into.
    pub buffer_id: PixelBufferId,
    /// The shape tool driving this drag (Line / Rectangle / Ellipse), so the
    /// preview overlay can draw the matching geometry.
    pub tool: Tool,
    /// Clean snapshot restored before each preview redraw.
    pub before: PixelBuffer,
    /// Press point in canvas pixels.
    pub start: [i32; 2],
    /// Current pointer point in canvas pixels.
    pub current: [i32; 2],
    /// Inclusive bounds of the previous preview, restored from `before` before
    /// the next redraw so each move touches only the old and new footprints,
    /// not the whole canvas.
    pub last_dirty: Option<(u32, u32, u32, u32)>,
}

/// An in-progress move of the selected pixels.
pub struct MoveDrag {
    /// Buffer being moved within.
    pub buffer_id: PixelBufferId,
    /// Buffer contents before the move (for the undo "before" snapshot).
    pub before: PixelBuffer,
    /// The background with the selection lifted out (transparent in the
    /// selection). Per-move restore reads from this so the move never recopies
    /// the whole buffer.
    pub base: PixelBuffer,
    /// Lifted pixels (the selection's content at press), as a full-canvas
    /// buffer with everything outside the selection transparent.
    pub lifted: PixelBuffer,
    /// Inclusive bounds of the selection at press (pre-offset), used to bound
    /// per-move stamping and the dirty rect. `None` for an empty selection.
    pub sel_bounds: Option<(u32, u32, u32, u32)>,
    /// Inclusive bounds of the previous frame's stamped footprint.
    pub last_dirty: Option<(u32, u32, u32, u32)>,
    /// Press point in canvas pixels.
    pub start: [i32; 2],
    /// Accumulated integer offset.
    pub offset: [i32; 2],
}

/// An in-progress free transform of the selected pixels via the box gizmo.
///
/// Mirrors [`MoveDrag`]'s lift-and-stamp shape — the selection is lifted into a
/// canvas-sized `lifted` buffer and cleared from the live buffer, with `base`
/// the cleared background and `before` the pre-transform snapshot — but the
/// stamp is an affine resample of `lifted` through the gizmo instead of an
/// integer translate. Per-frame work is bounded by the gizmo's axis-aligned
/// bounding box, not the canvas (the 8K constraint).
pub struct FreeTransformDrag {
    /// Buffer being transformed within.
    pub buffer_id: PixelBufferId,
    /// Buffer contents before the transform (the undo "before" snapshot).
    pub before: PixelBuffer,
    /// The background with the selection lifted out (transparent in the
    /// selection). Per-frame restore reads from this.
    pub base: PixelBuffer,
    /// Lifted pixels (the selection's content at press) as a full-canvas buffer,
    /// transparent outside the selection. The resample source.
    pub lifted: PixelBuffer,
    /// Inclusive bounds of the selection at press, the gizmo's seed rect and the
    /// source rectangle the resample maps from. `None` for an empty selection.
    pub sel_bounds: Option<(u32, u32, u32, u32)>,
    /// Inclusive bounds of the previous frame's stamped footprint, restored
    /// before the next resample.
    pub last_dirty: Option<(u32, u32, u32, u32)>,
    /// The transform box, in canvas-pixel space.
    pub gizmo: BoxGizmo,
    /// The handle the current drag is moving, picked on press. `None` between
    /// drags (a hover, or a press that missed the gizmo).
    pub gizmo_drag: Option<GizmoHandle>,
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
    /// Magic wand: use 8-connectivity (diagonals) instead of the default 4.
    pub wand_eight: bool,
    /// Magic wand: run the auto-close gap pre-pass so the flood does not leak
    /// through small breaks in an outline.
    pub wand_gap_close: bool,
    /// Magic wand: maximum gap, in pixels, the auto-close pass bridges. Only
    /// consulted when [`Self::wand_gap_close`] is on.
    pub wand_gap_distance: u32,
    /// Per-channel tolerance for the colour-range (select-by-colour) tool. Kept
    /// separate from [`Self::tolerance`] so the colour-range and fill/wand
    /// sliders move independently, matching the legacy TS app.
    pub color_range_tolerance: u8,
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
    /// In-progress free-transform drag.
    pub free_transform: Option<FreeTransformDrag>,
    /// Current selection mask (canvas-sized), or `None` when nothing is
    /// selected (the whole canvas is editable).
    pub selection: Option<SelectionMask>,
    /// How the next committed mask combines with [`Self::selection`]. Set from
    /// the modifiers at gesture start; transient, defaults to Replace.
    pub selection_mode: SelectionMode,
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
    /// Layers panel: in-progress inline rename `(layer, draft, needs_focus)`.
    /// `needs_focus` is set when the rename starts so the text field grabs
    /// focus on its first frame.
    pub layer_rename: Option<(LayerId, String, bool)>,
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
            wand_eight: false,
            wand_gap_close: false,
            wand_gap_distance: 10,
            color_range_tolerance: 0,
            fg: Rgba::opaque(20, 20, 28),
            bg: Rgba::transparent(),
            auto_add_palette: true,
            history: History::new(),
            stroke: None,
            shape_drag: None,
            lasso: Vec::new(),
            sel_drag: None,
            move_drag: None,
            free_transform: None,
            selection: None,
            selection_mode: SelectionMode::default(),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn session() -> StrokeSession {
        StrokeSession {
            buffer_id: PixelBufferId::new(1),
            before: PixelBuffer::empty(),
            points: Vec::new(),
            last_point: None,
            color: Rgba::opaque(255, 0, 0),
            shape: BrushShape::Circle,
            size: 40,
            mirror_x: false,
            mirror_y: false,
            erase: false,
            pixel_perfect: false,
            dirty: None,
            pending: None,
        }
    }

    #[test]
    fn pending_is_bounded_per_move_while_dirty_accumulates() {
        // The bug this guards: per-move upload must track only the footprint
        // since the last upload, not the whole-stroke bounding box.
        let mut s = session();

        // Move 1: a dab near the origin.
        s.mark_dirty(0, 0, 48, 48);
        assert_eq!(s.take_pending(), Some((0, 0, 48, 48)), "first move uploads its own footprint");
        assert!(s.take_pending().is_none(), "pending resets after it is taken");

        // Move 2: a dab far away. Pending must be only this dab — NOT the union
        // spanning back to the origin (that union is what caused O(stroke) lag).
        s.mark_dirty(900, 900, 48, 48);
        assert_eq!(s.take_pending(), Some((900, 900, 48, 48)), "second move uploads only its own footprint");

        // The cumulative dirty (used for the undo snapshot at commit) spans the
        // whole stroke, from the first dab to the last.
        assert_eq!(s.dirty, Some((0, 0, 947, 947)), "cumulative dirty still covers the whole stroke");
    }

    #[test]
    fn pending_unions_stamps_made_before_an_upload() {
        let mut s = session();
        // Two stamps with no take between them union into one pending rect.
        s.mark_dirty(0, 0, 10, 10);
        s.mark_dirty(20, 20, 10, 10);
        assert_eq!(s.take_pending(), Some((0, 0, 30, 30)));
    }
}
