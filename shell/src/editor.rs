//! Interactive-editing state: the active tools, brush, colors, selection,
//! onion-skin settings, undo history, and the per-gesture session structs.
//!
//! This is the hub the canvas input router and the editing panels read and
//! mutate. It lives as a plain field on [`crate::app::ShellApp`] next to the
//! [`crate::document::DocumentStore`], so an editing action borrows the two as
//! disjoint fields and pushes undo entries with
//! `editor.history.push(cmd, &mut doc)`.

use std::collections::{BTreeSet, VecDeque};

use eframe::egui;
use pixhaus_core::canvas::{BrushShape, DitherPattern, PixelBuffer};
use pixhaus_core::project::{Frame, IVec2, LayerId, PaletteId, PalettePageId, PixelBufferId, Rgba, Size};
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
    /// Sort by HSV saturation.
    Saturation,
    /// Sort by HSV value (brightness).
    Value,
}

/// The harmony scheme the palette panel's Harmony tool derives from the
/// foreground colour. Each kind maps to one `core::color::harmony` function;
/// switching kinds re-derives the suggestion swatches. Mirrors the kind tabs of
/// the Tauri `HarmonyPicker.tsx`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum HarmonyKind {
    /// The single complementary colour (hue + 180°).
    #[default]
    Complement,
    /// The two split-complementary colours (hue + 150° / + 210°).
    SplitComplement,
    /// The two triadic colours (hue + 120° / + 240°).
    Triad,
    /// The three tetradic (square) colours (hue + 90° / 180° / 270°).
    Tetrad,
    /// Five analogous colours stepped ±30° around the hue.
    Analogous,
}

impl HarmonyKind {
    /// The short label shown on the kind selector.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Complement => "Comp",
            Self::SplitComplement => "Split",
            Self::Triad => "Triad",
            Self::Tetrad => "Tetrad",
            Self::Analogous => "Analog",
        }
    }
}

/// The writable palette file format the export button targets. Import sniffs
/// the format from the file's extension and (for `.pal`) its bytes, so it needs
/// no enum; export must be told which encoder to run. `.aco` is read-only in
/// every tree, so it is not an export option. Mirrors the export side of the
/// Tauri `PaletteIOMenu.tsx`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PaletteExportFormat {
    /// GIMP Palette (`.gpl`), `gpl::encode`.
    #[default]
    Gpl,
    /// Lospec `.hex`, `hex::encode`.
    Hex,
    /// JASC `.pal`, `pal::encode_jasc`.
    Pal,
}

impl PaletteExportFormat {
    /// The lowercase file extension this format writes.
    #[must_use]
    pub const fn extension(self) -> &'static str {
        match self {
            Self::Gpl => "gpl",
            Self::Hex => "hex",
            Self::Pal => "pal",
        }
    }

    /// The short label shown on the format selector.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Gpl => "GPL",
            Self::Hex => "HEX",
            Self::Pal => "PAL",
        }
    }
}

/// The active tab of the multi-tab colour picker. Each tab edits the same
/// [`Rgba`] through a different colour space; switching tabs preserves the
/// colour. Mirrors the five modes of the Tauri `ColorPicker.tsx`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PickerTab {
    /// Hue / saturation / value sliders.
    #[default]
    Hsv,
    /// Hue / saturation / lightness sliders.
    Hsl,
    /// Red / green / blue integer sliders.
    Rgb,
    /// A `#RRGGBB` text field.
    Hex,
    /// Lightness / chroma / hue sliders in OKLCH.
    Oklch,
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
    /// Dither pattern gating this stroke's writes. A per-gesture copy of
    /// [`EditorState::dither`], so changing the editor mid-stroke does not split
    /// the pattern. [`DitherPattern::None`] keeps the stroke on the solid fast
    /// path; any other value also suppresses the pixel-perfect commit pass.
    pub dither: DitherPattern,
    /// Stroke strength in `0..=255`, a per-gesture copy of
    /// [`EditorState::opacity`]. The live preview paints at full strength so the
    /// artist sees the colour; on commit, when `opacity < 255`, the stroke
    /// redraws once from [`Self::before`] blending each unique footprint pixel a
    /// single time (the per-stroke, not per-dab, Aseprite semantic). `255` keeps
    /// the stroke on the overwrite fast path.
    pub opacity: u8,
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
    /// Dither pattern for this shape, a per-gesture copy of
    /// [`EditorState::dither`]. Carried so a future dithered shape pass has the
    /// value at hand; shape rasterization stays solid for now (the freehand
    /// brush is the only dithered path in this batch).
    pub dither: DitherPattern,
    /// Shape strength in `0..=255`, a per-gesture copy of
    /// [`EditorState::opacity`]. The preview rasterizes at full strength; on
    /// commit, when `opacity < 255`, the shape's pixels are blended once over
    /// [`Self::before`] (the line/rect/ellipse outline never self-overlaps, so a
    /// single blend per pixel is the whole shape). `255` overwrites.
    pub opacity: u8,
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

/// A copied run of frames, held off the serialized document so it survives
/// edits to the source and can paste into any sprite of equal canvas size.
///
/// Unlike the Tauri clipboard, which stored a single source frame index and
/// re-duplicated it on paste, v2 stores the **resolved pixel bytes** of every
/// cel: linked cels are resolved to their source buffer at copy time, so the
/// clipboard is self-contained and a later edit to the source frame does not
/// change what paste produces. This fits v2's buffer-store model and enables
/// cross-sprite paste; paste rejects a canvas-size mismatch rather than scaling.
#[derive(Clone, Debug)]
pub struct FrameClipboard {
    /// Canvas size the frames were copied from. Paste rejects a sprite whose
    /// canvas differs, so a pasted cel's bytes always fit the target buffer.
    pub canvas: Size,
    /// The copied frames, in ascending source-frame order.
    pub frames: Vec<ClipFrame>,
}

/// One copied frame: the [`Frame`] timing/metadata value and the cels that sat
/// on it, each carrying owned bytes.
#[derive(Clone, Debug)]
pub struct ClipFrame {
    /// The frame's timing and metadata, cloned verbatim.
    pub frame: Frame,
    /// The cels on this frame, each resolved to owned pixel bytes at copy time.
    pub cels: Vec<ClipCel>,
}

/// An in-progress frame-tag drag on the tag bar: a press-and-drag over empty
/// space that, on release, becomes a tag over the normalized `[min, max]` of
/// `start` and `end`. Both endpoints are frame indices clamped to the timeline,
/// so either can be the smaller. Transient view state: never serialized, not
/// undoable (the tag it creates is the undoable step). Mirrors the Tauri
/// `TagDragState` (`ui/src/timeline/timeline-state.ts`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TagDrag {
    /// Frame index where the drag began (the press point), clamped to the
    /// timeline.
    pub start: u32,
    /// Frame index the pointer is over now, clamped to the timeline. May be
    /// below `start` when the drag runs leftward.
    pub end: u32,
}

impl TagDrag {
    /// The normalized inclusive range the drag covers: `start` and `end` sorted
    /// so the lower index is `lo`. A tag created from a drag uses `[lo, hi]`
    /// regardless of drag direction.
    #[must_use]
    pub fn normalized(self) -> (u32, u32) {
        if self.start <= self.end {
            (self.start, self.end)
        } else {
            (self.end, self.start)
        }
    }
}

/// One copied cel, resolved to owned RGBA bytes so the clipboard does not point
/// back into the buffer store. Linked cels are resolved to their source buffer
/// at copy time, so paste always builds an independent raster cel.
#[derive(Clone, Debug)]
pub struct ClipCel {
    /// Layer the cel belonged to. Paste keeps the cel on the same layer id.
    pub layer_id: LayerId,
    /// Top-left placement within the canvas.
    pub position: IVec2,
    /// Per-cel opacity.
    pub opacity: u8,
    /// Packed `width * height * 4` RGBA bytes (tight stride, `width * 4`).
    pub bytes: Vec<u8>,
    /// Pixel dimensions of [`Self::bytes`].
    pub size: Size,
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
    /// Draw the per-pixel grid overlay once the zoom is high enough to read it
    /// ([`Self::PIXEL_GRID_ZOOM_THRESHOLD`]). Default on, matching the Tauri app.
    pub show_pixel_grid: bool,
    /// Draw the major / tile grid overlay: a cyan line every
    /// [`Self::major_grid_spacing_x`] / [`Self::major_grid_spacing_y`] canvas
    /// pixels. Default off; the tile workflow binds the spacing to a tile size
    /// later.
    pub major_grid_enabled: bool,
    /// Major-grid line spacing on the X axis, in canvas pixels. Independent of
    /// the Y spacing so a non-square tile size works.
    pub major_grid_spacing_x: u32,
    /// Major-grid line spacing on the Y axis, in canvas pixels. Independent of
    /// the X spacing so a non-square tile size works.
    pub major_grid_spacing_y: u32,
    /// Dither pattern gating which footprint pixels the freehand brush writes.
    /// Default [`DitherPattern::None`] (a solid brush). Session state, not
    /// persisted, mutually exclusive with pixel-perfect (the commit pass is
    /// skipped while a pattern is active).
    pub dither: DitherPattern,
    /// Paint strength in `0..=255` for the brush, eraser, fill, and shapes.
    /// Default `255` (full strength, the overwrite fast path). Below 255 the
    /// paint blends over the backdrop per-stroke (not per-dab): a freehand drag
    /// blends each unique footprint pixel once on commit, so overlapping passes
    /// within one drag do not compound. On the eraser it reduces, rather than
    /// clears, the erased alpha (255 = full erase). Session state, not persisted.
    pub opacity: u8,
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
    /// Palette panel: the switcher's selected palette, by id. `None` until a
    /// sprite is selected or while the selection points at no live palette, in
    /// which case the panel falls back to the sprite's first palette (see
    /// [`DocumentStore::active_palette_by_id`]). Drives which palette the panel
    /// shows and every palette edit lands on. View state, never serialized; a
    /// sprite switch or a palette delete re-points it at the first palette.
    pub active_palette_id: Option<PaletteId>,
    /// Palette panel: lock the grid against accidental reordering.
    pub lock_palette_grid: bool,
    /// Palette panel: swatch edge length in points.
    pub swatch_size: f32,
    /// Palette panel: index being edited in the colour popup, if any.
    pub editing_swatch: Option<usize>,
    /// Palette panel: the multi-tab colour picker's active tab.
    pub picker_tab: PickerTab,
    /// Palette panel: the `#RRGGBB` text the HEX tab edits, re-seeded from the
    /// live colour when the tab opens and parsed back on Enter / focus loss.
    pub hex_draft: String,
    /// Palette panel: the in-progress colour of the swatch being edited, held
    /// off the document so live slider drags preview by repainting the swatch
    /// rather than committing an undo entry per tick. Committed to the sprite as
    /// one [`crate::commands::push_sprite_edit`] on drag-stop / focus loss.
    pub swatch_scratch: Option<Rgba>,
    /// Palette panel: the multi-select set of swatch indices. Ctrl-click toggles
    /// membership; a plain click sets the foreground and clears the set. Drives
    /// the remove-selected button. View state, never serialized; pruned of stale
    /// indices after a removal so it never points past the palette.
    pub selected_swatches: BTreeSet<usize>,
    /// Palette panel: the per-swatch UI lock set, by index. A locked swatch is
    /// excluded from edit, remove, and reorder, and shows a lock overlay.
    /// UI-only, never serialized — matching the Tauri panel's `lockedIndices`.
    pub locked_swatches: BTreeSet<usize>,
    /// Palette panel: the swatch index being inline-renamed, if any. Set from
    /// the per-swatch context menu; the draft commits on Enter / focus loss and
    /// discards on Escape.
    pub renaming_swatch: Option<usize>,
    /// Palette panel: the draft name the inline swatch rename edits, re-seeded
    /// from the entry's current name when the rename opens.
    pub rename_draft: String,
    /// Palette panel: the ramp tool's start swatch index. Clamped to the live
    /// palette length each frame, so it never points past the palette.
    pub ramp_from: usize,
    /// Palette panel: the ramp tool's end swatch index. Clamped like
    /// [`Self::ramp_from`].
    pub ramp_to: usize,
    /// Palette panel: the ramp tool's step count, in `3..=32`. The added swatches
    /// are the `steps - 2` intermediates (the endpoints are not duplicated).
    pub ramp_steps: usize,
    /// Palette panel: the harmony tool's scheme, derived from the foreground
    /// colour. View state, never serialized.
    pub harmony_kind: HarmonyKind,
    /// Palette panel: the selected page in the active palette's Pages section,
    /// by id, or `None` for "no page" (the grid shows every swatch). Re-validated
    /// against the live page list each frame, so a removed page never leaves a
    /// dangling selection. View state, never serialized.
    pub active_page_id: Option<PalettePageId>,
    /// Palette panel: filter the swatch grid to the selected page's members when
    /// a page is active. Off by default, so selecting a page highlights its
    /// members without hiding the rest until the artist asks. View state.
    pub filter_to_page: bool,
    /// Palette panel: the draft name an Add-page / Rename-page commits. Re-seeded
    /// from the page's current name when a rename opens; blank is rejected by the
    /// add/rename helpers. View state, never serialized.
    pub page_name_draft: String,
    /// Palette panel: the cycle range's first index for the Animation section's
    /// "Cycle range" control. Clamped to the live palette length each frame.
    pub cycle_first: usize,
    /// Palette panel: the cycle range's last index, clamped like
    /// [`Self::cycle_first`].
    pub cycle_last: usize,
    /// Palette panel: the cycle range's signed rotation offset. Positive rotates
    /// toward higher indices, negative toward lower; zero is a no-op.
    pub cycle_offset: i32,
    /// Palette panel: the format the export button writes. Import sniffs its
    /// format from the file, so only export needs this. View state, never
    /// serialized.
    pub export_format: PaletteExportFormat,
    /// Palette panel: the most recent import/export error, shown inline under
    /// the Import / Export section. `None` when the last file op succeeded or
    /// none has run. v2 has no toast system, so the field stands in for one.
    /// View state, never serialized.
    pub palette_io_error: Option<String>,
    /// Palette panel: the most-recently-committed colours, newest first, capped
    /// at [`Self::RECENT_COLORS_CAP`]. A stroke / fill / shape commit pushes its
    /// colour via [`push_recent`] (move-to-front on a repeat); the recents strip
    /// above the swatch grid clicks one back into the foreground. View state,
    /// never serialized.
    pub recent_colors: VecDeque<Rgba>,
    /// Palette panel (Lospec browser): the slug or URL the artist types into the
    /// Lospec import field, e.g. `aap-16`. Gated behind the `lospec` feature so
    /// the default build carries no Lospec UI state. View state, never serialized.
    #[cfg(feature = "lospec")]
    pub lospec_slug: String,
    /// Palette panel (Lospec browser): the latest fetch status — `None` when idle,
    /// `Some(Ok(()))` after a success, `Some(Err(msg))` for the inline error. Set
    /// off the background fetch thread's result. Gated behind the `lospec` feature.
    /// View state, never serialized.
    #[cfg(feature = "lospec")]
    pub lospec_status: Option<Result<(), String>>,
    /// Palette panel (Lospec browser): whether a fetch is in flight, disabling the
    /// import button until the result lands. Gated behind the `lospec` feature.
    #[cfg(feature = "lospec")]
    pub lospec_in_flight: bool,
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
    /// Timeline: the copied frame payload, or `None` when nothing has been
    /// copied. Holds owned pixel bytes (see [`FrameClipboard`]), so it survives
    /// edits to the source frames and pastes into any sprite of equal canvas
    /// size. View state, never serialized.
    pub frame_clipboard: Option<FrameClipboard>,
    /// Timeline: whether main-timeline playback loops. Default `true` to keep
    /// the prior always-loop behaviour. With it off, playback runs once and
    /// halts on the last frame. View state, transient, not undoable — the
    /// studio clip player carries the same toggle (`Studio::loop_playback`).
    pub loop_playback: bool,
    /// Timeline: last-set global FPS, the value the FPS field shows and writes
    /// across every frame's `duration_ms`. Default `12`. View state — the
    /// authoritative timing lives on each [`Frame`], so this only seeds and
    /// reflects the field, it is not serialized or undoable.
    pub global_fps: u32,
    /// Timeline: how many procedural inbetweens the Tween action inserts
    /// between the two selected frames on the active layer. Default `1`. View
    /// state — the generated cels are the durable artifact, this only seeds the
    /// count picker and is not serialized or undoable.
    pub tween_count: usize,
    /// Timeline: the tag the playback/rename controls target, as an index into
    /// `Sprite::frame_tags`, or `None` when no tag is selected. Set by clicking
    /// a span on the tag bar. View state, never serialized; the index is
    /// re-validated against the live tag list at use so a removed tag does not
    /// leave a dangling selection.
    pub selected_tag: Option<usize>,
    /// Timeline: an in-progress inline tag rename `(tag index, draft)`, started
    /// from the tag context menu. Mirrors [`Self::layer_rename`] but targets a
    /// tag by index. `None` when no rename is open. The draft commits on
    /// Enter/blur through the rename validation (empty and duplicate names are
    /// rejected) and discards on Escape. View state, never serialized; the
    /// committed rename is the undoable step.
    pub tag_rename: Option<(usize, String)>,
    /// Timeline: an in-progress drag on the tag bar that, on release, creates a
    /// tag over the dragged range. `None` between drags. Transient view state.
    pub tag_drag: Option<TagDrag>,
    /// Timeline: whether the scrub head is being dragged across the header
    /// strip. Set on a press-with-movement (which also stops playback) and
    /// cleared on release. Transient view state, never serialized — scrubbing
    /// only moves the playhead, it is not undoable.
    pub scrubbing: bool,
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
            show_pixel_grid: true,
            major_grid_enabled: false,
            major_grid_spacing_x: 8,
            major_grid_spacing_y: 8,
            dither: DitherPattern::None,
            opacity: 255,
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
            active_palette_id: None,
            lock_palette_grid: false,
            swatch_size: 18.0,
            editing_swatch: None,
            picker_tab: PickerTab::default(),
            hex_draft: String::new(),
            swatch_scratch: None,
            selected_swatches: BTreeSet::new(),
            locked_swatches: BTreeSet::new(),
            renaming_swatch: None,
            rename_draft: String::new(),
            ramp_from: 0,
            ramp_to: 0,
            ramp_steps: 5,
            harmony_kind: HarmonyKind::default(),
            active_page_id: None,
            filter_to_page: false,
            page_name_draft: String::new(),
            cycle_first: 0,
            cycle_last: 0,
            cycle_offset: 1,
            export_format: PaletteExportFormat::default(),
            palette_io_error: None,
            recent_colors: VecDeque::new(),
            #[cfg(feature = "lospec")]
            lospec_slug: String::new(),
            #[cfg(feature = "lospec")]
            lospec_status: None,
            #[cfg(feature = "lospec")]
            lospec_in_flight: false,
            cel_size: 48.0,
            new_tag_name: String::new(),
            layer_rename: None,
            selected_layers: BTreeSet::new(),
            pending_layer_delete: None,
            selected_frames: BTreeSet::new(),
            frame_clipboard: None,
            loop_playback: true,
            global_fps: 12,
            tween_count: 1,
            selected_tag: None,
            tag_rename: None,
            tag_drag: None,
            scrubbing: false,
        }
    }
}

impl EditorState {
    /// Zoom (screen pixels per canvas pixel) at and above which the per-pixel
    /// grid overlay draws. Below it the grid would alias into noise, so the
    /// overlay is skipped. Mirrors the Tauri `PIXEL_GRID_ZOOM_THRESHOLD`.
    pub const PIXEL_GRID_ZOOM_THRESHOLD: f32 = 4.0;

    /// How many recent colours the palette panel keeps. One short row above the
    /// swatch grid.
    pub const RECENT_COLORS_CAP: usize = 16;

    /// Records `color` as the most-recently-used colour, newest first, capped at
    /// [`Self::RECENT_COLORS_CAP`]. A repeat moves to the front rather than
    /// duplicating. Called from the stroke / fill / shape commit so the recents
    /// strip tracks painted colours regardless of the auto-add-to-palette toggle.
    pub fn push_recent(&mut self, color: Rgba) {
        push_recent(&mut self.recent_colors, color, Self::RECENT_COLORS_CAP);
    }

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

/// Pushes `color` to the front of the recent-colours deque, capped at `cap`.
///
/// A colour already present moves to the front (dedup-to-front) rather than
/// duplicating; a fresh colour pushes to the front and the oldest entry is
/// evicted from the back once the deque exceeds `cap`. A `cap` of 0 leaves the
/// deque empty. Pure so the panel and tests share one rule.
pub fn push_recent(recent: &mut VecDeque<Rgba>, color: Rgba, cap: usize) {
    if cap == 0 {
        recent.clear();
        return;
    }
    if let Some(pos) = recent.iter().position(|&c| c == color) {
        recent.remove(pos);
    }
    recent.push_front(color);
    while recent.len() > cap {
        recent.pop_back();
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
            dither: DitherPattern::None,
            opacity: 255,
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

    mod recent_colors {
        use std::collections::VecDeque;

        use pixhaus_core::project::Rgba;

        use crate::editor::push_recent;

        fn colors(deque: &VecDeque<Rgba>) -> Vec<Rgba> {
            deque.iter().copied().collect()
        }

        #[test]
        fn push_recent_puts_the_newest_color_first() {
            let mut d = VecDeque::new();
            push_recent(&mut d, Rgba::opaque(1, 0, 0), 16);
            push_recent(&mut d, Rgba::opaque(2, 0, 0), 16);
            assert_eq!(colors(&d), vec![Rgba::opaque(2, 0, 0), Rgba::opaque(1, 0, 0)]);
        }

        #[test]
        fn push_recent_moves_a_repeat_to_the_front_without_duplicating() {
            let mut d = VecDeque::new();
            for n in 1..=3 {
                push_recent(&mut d, Rgba::opaque(n, 0, 0), 16);
            }
            // Re-using the oldest colour moves it to the front; no duplicate.
            push_recent(&mut d, Rgba::opaque(1, 0, 0), 16);
            assert_eq!(colors(&d), vec![Rgba::opaque(1, 0, 0), Rgba::opaque(3, 0, 0), Rgba::opaque(2, 0, 0)]);
        }

        #[test]
        fn push_recent_evicts_the_oldest_past_the_cap() {
            let mut d = VecDeque::new();
            // Cap of 2: pushing a third distinct colour drops the oldest.
            push_recent(&mut d, Rgba::opaque(1, 0, 0), 2);
            push_recent(&mut d, Rgba::opaque(2, 0, 0), 2);
            push_recent(&mut d, Rgba::opaque(3, 0, 0), 2);
            assert_eq!(colors(&d), vec![Rgba::opaque(3, 0, 0), Rgba::opaque(2, 0, 0)]);
        }

        #[test]
        fn push_recent_with_zero_cap_keeps_the_deque_empty() {
            let mut d = VecDeque::new();
            push_recent(&mut d, Rgba::opaque(1, 0, 0), 0);
            assert!(d.is_empty());
        }
    }

    mod frame_selection {
        use std::collections::BTreeSet;

        use crate::editor::{EditorState, frame_range_set};

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

        #[test]
        fn tag_drag_normalizes_regardless_of_direction() {
            use crate::editor::TagDrag;
            // A rightward drag keeps its order.
            assert_eq!(TagDrag { start: 2, end: 5 }.normalized(), (2, 5));
            // A leftward drag swaps so the lower index is first.
            assert_eq!(TagDrag { start: 5, end: 2 }.normalized(), (2, 5));
            // A single-frame drag collapses to a one-frame range.
            assert_eq!(TagDrag { start: 3, end: 3 }.normalized(), (3, 3));
        }
    }

    mod selection {
        use std::collections::BTreeSet;

        use pixhaus_core::project::{Layer, LayerId, LayerKind, Size, Sprite, SpriteId};

        use crate::editor::{EditorState, reseed_selection};

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
