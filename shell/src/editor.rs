//! Interactive-editing state: the active tools, brush, colors, selection,
//! onion-skin settings, undo history, and the per-gesture session structs.
//!
//! This is the hub the canvas input router and the editing panels read and
//! mutate. It lives as a plain field on [`crate::app::ShellApp`] next to the
//! [`crate::document::DocumentStore`], so an editing action borrows the two as
//! disjoint fields and pushes undo entries with
//! `editor.history.push(cmd, &mut doc)`.

use std::collections::BTreeSet;

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
    /// Layers panel: the multi-select set, driving batch ops (merge selected,
    /// delete multi). View state, not document state — never serialized and not
    /// undoable. The paint target stays [`DocumentStore::active_layer`] (the
    /// anchor: the last-clicked row, whose blend/opacity strip shows); this set
    /// is only the batch-op target.
    pub selected_layers: BTreeSet<LayerId>,
    /// Layers panel: a delete awaiting confirmation. Set when a delete would
    /// remove more than one layer or a group with children — destructive enough
    /// to ask first. Carries the exact ids to drop; the confirm modal runs the
    /// delete, cancel clears it. A single leaf delete skips this and deletes
    /// straight away. View state, not undoable in itself (the delete it triggers
    /// is the undoable step).
    pub pending_layer_delete: Option<Vec<LayerId>>,
    /// Timeline: the multi-frame selection set, by frame index. Drives batch
    /// frame ops (multi-delete, copy/paste of a range, reverse). View state, not
    /// document state — never serialized and not undoable. An empty set means
    /// "just the active frame": [`Self::effective_frames`] resolves that
    /// fallback. The active frame still drives the canvas; this set is only the
    /// batch-op target.
    pub selected_frames: BTreeSet<u32>,
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
            selected_layers: BTreeSet::new(),
            pending_layer_delete: None,
            selected_frames: BTreeSet::new(),
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

    /// Drops the multi-frame selection. Called on a sprite switch so a stale
    /// set never points at frames the new sprite does not have. Distinct from
    /// [`Self::clear_selection`], which clears the pixel selection.
    pub fn clear_frame_selection(&mut self) {
        self.selected_frames.clear();
    }

    /// The frames a batch op should target: [`Self::selected_frames`] when the
    /// set is non-empty, else `{active}`. An empty set means "just the active
    /// frame", so a delete/copy/reverse with nothing explicitly selected still
    /// acts on the playhead's frame. Mirrors the Tauri `effectiveSelection`.
    // Consumed by multi-delete, copy/paste, and reverse (plan tasks 3, 4, 2);
    // landed with the selection model so every later task reads one accessor.
    #[allow(dead_code)]
    #[must_use]
    pub fn effective_frames(&self, active: u32) -> BTreeSet<u32> {
        if self.selected_frames.is_empty() {
            let mut set = BTreeSet::new();
            set.insert(active);
            set
        } else {
            self.selected_frames.clone()
        }
    }

    /// Resolves a layer-panel click into the new multi-select set and anchor.
    /// `display_order` is the panel's top-first display order
    /// (`Sprite::layer_display_order`), `anchor` is the current anchor
    /// (`DocumentStore::active_layer`, the last-clicked row). Returns the new
    /// anchor for the caller to store back into `active_layer`.
    ///
    /// - Plain click (`additive` and `range` both false): replace the set with
    ///   `{id}` and make `id` the anchor.
    /// - Ctrl/Cmd-click (`additive`): toggle `id` in the set and move the anchor
    ///   to `id`. Removing the anchor leaves the set as-is (the caller's reseed
    ///   handles an emptied set).
    /// - Shift-click (`range`): select the contiguous display-order span between
    ///   the current anchor and `id`. The anchor does not move (Shift extends
    ///   from the same anchor, matching the Tauri panel). With no anchor, falls
    ///   back to a plain click.
    ///
    /// Range wins over toggle when both modifiers are held, matching the panel.
    pub fn resolve_layer_selection(&mut self, display_order: &[LayerId], anchor: Option<LayerId>, id: LayerId, additive: bool, range: bool) -> LayerId {
        if range {
            if let Some(span) = anchor.and_then(|a| display_range(display_order, a, id)) {
                self.selected_layers = span.into_iter().collect();
                return anchor.unwrap_or(id);
            }
            // No anchor (or it left the tree): treat as a plain click.
        }
        if additive && !range {
            if !self.selected_layers.insert(id) {
                self.selected_layers.remove(&id);
            }
            return id;
        }
        // Plain click (or range fall-through): collapse to the clicked row.
        self.selected_layers.clear();
        self.selected_layers.insert(id);
        id
    }

    /// The current multi-select as a `Vec`, in ascending `LayerId` order.
    /// Batch ops read this; they re-order by composite position themselves.
    // Consumed by merge-selected and delete-multi (plan tasks 4 and 5); kept on
    // the selection model now so every later task reads one accessor.
    #[allow(dead_code)]
    #[must_use]
    pub fn selected_layer_ids(&self) -> Vec<LayerId> {
        self.selected_layers.iter().copied().collect()
    }
}

/// The inclusive set of frame indices between `a` and `b`, in either order.
/// Drives Shift-click range selection on the cel-matrix header: the anchor
/// (the active frame) and the clicked frame, plus everything between them.
#[must_use]
pub fn frame_range_set(a: u32, b: u32) -> BTreeSet<u32> {
    let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
    (lo..=hi).collect()
}

/// The contiguous span of `display_order` between `a` and `b` inclusive, in
/// display order. `None` when either id is absent from the order.
fn display_range(display_order: &[LayerId], a: LayerId, b: LayerId) -> Option<Vec<LayerId>> {
    let pa = display_order.iter().position(|&l| l == a)?;
    let pb = display_order.iter().position(|&l| l == b)?;
    let (lo, hi) = if pa <= pb { (pa, pb) } else { (pb, pa) };
    Some(display_order[lo..=hi].to_vec())
}

/// Reseeds `selected` and the anchor against the live `display_order` (top-first).
/// Drops selected ids no longer in the order; when the set has emptied or the
/// anchor is gone, seeds both from the top remaining layer. Returns the new
/// anchor (the first display-order layer, or `None` for an empty sprite). Pure
/// so the shell wrapper and tests share one rule.
pub fn reseed_selection(display_order: &[LayerId], selected: &mut BTreeSet<LayerId>, anchor: Option<LayerId>) -> Option<LayerId> {
    selected.retain(|id| display_order.contains(id));
    let anchor_live = anchor.is_some_and(|id| display_order.contains(&id));
    if anchor_live && !selected.is_empty() {
        return anchor;
    }
    let top = display_order.first().copied();
    selected.clear();
    if let Some(id) = top {
        selected.insert(id);
    }
    top
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

    mod frame_selection {
        use std::collections::BTreeSet;

        use crate::editor::{frame_range_set, EditorState};

        fn set(ids: &[u32]) -> BTreeSet<u32> {
            ids.iter().copied().collect()
        }

        #[test]
        fn effective_frames_falls_back_to_the_active_frame_when_empty() {
            let ed = EditorState::default();
            assert!(ed.selected_frames.is_empty());
            assert_eq!(ed.effective_frames(3), set(&[3]), "an empty set resolves to just the active frame");
        }

        #[test]
        fn effective_frames_returns_the_set_verbatim_when_non_empty() {
            let ed = EditorState {
                selected_frames: set(&[0, 2, 4]),
                ..EditorState::default()
            };
            // The active frame is ignored once the set has any member.
            assert_eq!(ed.effective_frames(1), set(&[0, 2, 4]));
        }

        #[test]
        fn frame_range_set_is_inclusive_in_ascending_order() {
            assert_eq!(frame_range_set(2, 5), set(&[2, 3, 4, 5]));
        }

        #[test]
        fn frame_range_set_is_order_independent() {
            // Shift-clicking below the anchor yields the same inclusive span.
            assert_eq!(frame_range_set(5, 2), set(&[2, 3, 4, 5]));
        }

        #[test]
        fn frame_range_set_of_one_frame_is_a_singleton() {
            assert_eq!(frame_range_set(4, 4), set(&[4]));
        }

        #[test]
        fn clear_frame_selection_empties_the_set() {
            let mut ed = EditorState {
                selected_frames: set(&[1, 2, 3]),
                ..EditorState::default()
            };
            ed.clear_frame_selection();
            assert!(ed.selected_frames.is_empty());
        }
    }

    mod selection {
        use std::collections::BTreeSet;

        use pixhaus_core::project::{Layer, LayerId, LayerKind, Size, Sprite, SpriteId};

        use crate::editor::{reseed_selection, EditorState};

        fn lid(n: u32) -> LayerId {
            LayerId::new(n)
        }

        /// Bottom raster (1), a group (2) holding one child raster (3), and a
        /// top raster (4). Vec order is bottom-first; the display order this
        /// yields is top-first: [4, 2, 3, 1].
        fn nested_order() -> Vec<LayerId> {
            let mut s = Sprite::empty(SpriteId::new(1), "s", Size::new(8, 8));
            let mut group = Layer::raster(lid(2), "group");
            group.kind = LayerKind::Group { collapsed: false };
            let mut child = Layer::raster(lid(3), "child");
            child.parent = Some(lid(2));
            s.layers = vec![Layer::raster(lid(1), "bottom"), group, child, Layer::raster(lid(4), "top")];
            s.layer_display_order().into_iter().map(|(id, _)| id).collect()
        }

        fn set(ids: &[u32]) -> BTreeSet<LayerId> {
            ids.iter().map(|n| lid(*n)).collect()
        }

        fn editor_with(ids: &[u32]) -> EditorState {
            EditorState {
                selected_layers: set(ids),
                ..EditorState::default()
            }
        }

        #[test]
        fn display_order_is_top_first_through_the_group() {
            assert_eq!(nested_order(), vec![lid(4), lid(2), lid(3), lid(1)]);
        }

        #[test]
        fn shift_range_spans_the_display_order_across_the_group() {
            let order = nested_order();
            let mut ed = EditorState::default();
            // Anchor on the top layer (4), shift-click the bottom layer (1):
            // the range covers the whole top-first span, including the group
            // header and its child, not Vec order.
            let new_anchor = ed.resolve_layer_selection(&order, Some(lid(4)), lid(1), false, true);
            assert_eq!(new_anchor, lid(4), "shift extends from the anchor without moving it");
            assert_eq!(ed.selected_layers, set(&[1, 2, 3, 4]));

            // A tighter range: anchor 2, shift-click 3 -> contiguous [2, 3].
            let mut ed2 = EditorState::default();
            ed2.resolve_layer_selection(&order, Some(lid(2)), lid(3), false, true);
            assert_eq!(ed2.selected_layers, set(&[2, 3]));
        }

        #[test]
        fn shift_with_no_anchor_falls_back_to_a_plain_click() {
            let order = nested_order();
            let mut ed = EditorState::default();
            let new_anchor = ed.resolve_layer_selection(&order, None, lid(3), false, true);
            assert_eq!(new_anchor, lid(3));
            assert_eq!(ed.selected_layers, set(&[3]));
        }

        #[test]
        fn ctrl_toggle_adds_then_removes() {
            let order = nested_order();
            let mut ed = EditorState::default();
            // Seed with a plain click on 4.
            ed.resolve_layer_selection(&order, None, lid(4), false, false);
            assert_eq!(ed.selected_layers, set(&[4]));

            // Ctrl-click 1: adds it and moves the anchor to 1.
            let anchor = ed.resolve_layer_selection(&order, Some(lid(4)), lid(1), true, false);
            assert_eq!(anchor, lid(1));
            assert_eq!(ed.selected_layers, set(&[1, 4]));

            // Ctrl-click 1 again: removes it. The anchor still tracks 1.
            let anchor = ed.resolve_layer_selection(&order, Some(lid(1)), lid(1), true, false);
            assert_eq!(anchor, lid(1));
            assert_eq!(ed.selected_layers, set(&[4]));
        }

        #[test]
        fn plain_click_collapses_to_one() {
            let order = nested_order();
            let mut ed = editor_with(&[1, 2, 3, 4]);
            let anchor = ed.resolve_layer_selection(&order, Some(lid(4)), lid(3), false, false);
            assert_eq!(anchor, lid(3));
            assert_eq!(ed.selected_layers, set(&[3]), "plain click discards the rest of the selection");
        }

        #[test]
        fn range_wins_over_toggle_when_both_modifiers_held() {
            let order = nested_order();
            let mut ed = EditorState::default();
            // Both Ctrl and Shift down: the range path runs, not the toggle.
            ed.resolve_layer_selection(&order, Some(lid(2)), lid(3), true, true);
            assert_eq!(ed.selected_layers, set(&[2, 3]));
        }

        #[test]
        fn deleting_the_anchor_reseeds_both_anchor_and_set() {
            // Start with the full selection, anchor on the top layer (4). Drop 4
            // from the order (simulating a delete) and reseed.
            let mut selected = set(&[1, 2, 3, 4]);
            selected.remove(&lid(4));
            // Order after 4 is gone: top-first [2, 3, 1].
            let order_after = vec![lid(2), lid(3), lid(1)];
            let anchor = reseed_selection(&order_after, &mut selected, Some(lid(4)));
            // The dead anchor reseeds to the top remaining layer; the set follows.
            assert_eq!(anchor, Some(lid(2)));
            assert_eq!(selected, set(&[2]));
        }

        #[test]
        fn reseed_is_a_no_op_while_anchor_and_set_stay_live() {
            let order = nested_order();
            let mut selected = set(&[2, 3]);
            let anchor = reseed_selection(&order, &mut selected, Some(lid(2)));
            assert_eq!(anchor, Some(lid(2)));
            assert_eq!(selected, set(&[2, 3]), "a live anchor with a non-empty set is left untouched");
        }

        #[test]
        fn reseed_seeds_from_the_top_when_the_set_empties() {
            let order = nested_order();
            let mut selected: BTreeSet<LayerId> = BTreeSet::new();
            let anchor = reseed_selection(&order, &mut selected, None);
            assert_eq!(anchor, Some(lid(4)));
            assert_eq!(selected, set(&[4]));
        }

        #[test]
        fn selected_layer_ids_returns_ascending_ids() {
            let ed = editor_with(&[4, 1, 3]);
            assert_eq!(ed.selected_layer_ids(), vec![lid(1), lid(3), lid(4)]);
        }
    }
}
