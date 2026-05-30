//! The Create-mode **AI studio**: the full-screen workspace for the whole
//! generate-and-refine loop — the anchor (cockpit composition + variant
//! gallery), the seed pose, and the animation.
//!
//! The studio is the entire Create workspace: `app.rs` routes the central panel
//! here whenever [`ShellApp::studio_active`] (i.e. `Workspace::Create`) and hides
//! the tools rail, side dock, and timeline, so the studio owns the whole canvas
//! area. Inside, the left panel holds the sprite gallery and the stages rail, the
//! center is the stage surface, and the right is the stage inspector — under a
//! header with a back-to-editor exit and a composition-library overlay toggle.
//!
//! It is mostly assembly over parts that already exist. The clip mechanics —
//! decode, loop detection, frame picking, normalize, background removal — live
//! in [`crate::anim`], [`crate::ai`], and the clip-stage helpers on
//! [`ShellApp`] (`start_clip`, `select_clip`, `integrate_picked`, …), which the
//! studio drives unchanged. The one part v2 lacked, and the studio adds, is the
//! **first-frame stage**: a text-to-image seed pose with a variant gallery, plus
//! an inpaint mask overlay to repaint a wrong region and approve the result.
//! That approved frame is the single input that drives the whole clip.
//!
//! Nothing the studio does touches the document until Land — first-frame
//! candidates, the approved pose, the mask, and the clip candidates all live on
//! [`ShellApp`] (here and in `anim_*`), so Cancel and restart are clean at every
//! stage.

use std::collections::HashMap;

use eframe::egui;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use pixhaus_ai::backends::fal::{FAL_I2V, FAL_SEEDANCE};
use pixhaus_core::canvas::PixelBuffer;
use pixhaus_core::project::{AnchorDirection, AnimationKind, Rgba, SpriteId};
use pixhaus_core::transforms::normalize::{ChromaKey, ComponentMode, NormalizeResult, SeamMatch, chroma_key, detect_key_color};
use pixhaus_core::transforms::sheet::SliceGrid;

use crate::ai::{self, FirstFrameJob};
use crate::anim::{self, VideoFrame};
use crate::app::{AnimPlayMode, JobStatus, ShellApp, Workspace};
use crate::document::SpriteRef;
use crate::gizmo::{BoxGizmo, GizmoHandle};

/// The studio's ordered stages. Navigation is free — the rail lets you step back
/// to an earlier stage without losing later work — so this is a position in a
/// workspace, not a forced march.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum StudioStage {
    /// Generate and approve the canonical reference sheet — the character's
    /// look. Direction and animation kind are framing for later stages, not
    /// chosen here.
    Anchor,
    /// Generate and refine the seed pose (text-to-image + inpaint).
    FirstFrame,
    /// Choreography prompt, model pick, fps, frame count, seed.
    Motion,
    /// Cut the raw grid sheet: a slice gizmo over the generated sheet that
    /// re-slices live. Only reached on the static-sheet path; the i2v path skips
    /// straight to the clip.
    Sheet,
    /// Scrub the raw clip and drag the loop markers.
    Clip,
    /// Toggle the evenly-spaced picks and read the seam score.
    Pick,
    /// Review the normalization report (drift / scale / seam) before landing.
    Normalize,
    /// Land the reviewed, normalized picks on the timeline.
    Land,
}

impl StudioStage {
    /// The stages in pipeline order, for the rail.
    const ALL: [StudioStage; 8] = [
        StudioStage::Anchor,
        StudioStage::FirstFrame,
        StudioStage::Motion,
        StudioStage::Sheet,
        StudioStage::Clip,
        StudioStage::Pick,
        StudioStage::Normalize,
        StudioStage::Land,
    ];

    /// Rail label for the stage.
    fn label(self) -> &'static str {
        match self {
            StudioStage::Anchor => "Anchor",
            StudioStage::FirstFrame => "First frame",
            StudioStage::Motion => "Motion",
            StudioStage::Sheet => "Slice",
            StudioStage::Clip => "Clip & loop",
            StudioStage::Pick => "Frame pick",
            StudioStage::Normalize => "Normalize",
            StudioStage::Land => "Land",
        }
    }
}

/// The animation kind, which scaffolds the motion prompt.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AnimKind {
    Idle,
    Walk,
    Attack,
    Custom,
}

impl AnimKind {
    const ALL: [AnimKind; 4] = [AnimKind::Idle, AnimKind::Walk, AnimKind::Attack, AnimKind::Custom];

    fn label(self) -> &'static str {
        match self {
            AnimKind::Idle => "Idle",
            AnimKind::Walk => "Walk",
            AnimKind::Attack => "Attack",
            AnimKind::Custom => "Custom",
        }
    }

    /// The motion-prompt fragment this kind contributes, or `None` for custom.
    fn motion_fragment(self) -> Option<&'static str> {
        match self {
            AnimKind::Idle => Some("idle breathing"),
            AnimKind::Walk => Some("walk cycle"),
            AnimKind::Attack => Some("attack swing"),
            AnimKind::Custom => None,
        }
    }

    /// A stable `snake_case` tag for the durable job record's `animation_kind`.
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            AnimKind::Idle => "idle",
            AnimKind::Walk => "walk",
            AnimKind::Attack => "attack",
            AnimKind::Custom => "custom",
        }
    }

    /// Maps the studio kind to the core [`AnimationKind`] used in the directional
    /// cascade. [`AnimKind::Custom`] has no cascade equivalent and returns `None`,
    /// so a custom animation lands its frames but records no cascade edge.
    pub(crate) fn to_animation_kind(self) -> Option<AnimationKind> {
        match self {
            AnimKind::Idle => Some(AnimationKind::Idle),
            AnimKind::Walk => Some(AnimationKind::Walk),
            AnimKind::Attack => Some(AnimationKind::Attack),
            AnimKind::Custom => None,
        }
    }
}

/// The facing direction the animation conditions on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Facing {
    South,
    North,
    East,
    West,
}

impl Facing {
    const ALL: [Facing; 4] = [Facing::South, Facing::North, Facing::East, Facing::West];

    fn label(self) -> &'static str {
        match self {
            Facing::South => "South",
            Facing::North => "North",
            Facing::East => "East",
            Facing::West => "West",
        }
    }

    /// The prompt fragment describing the view from this facing.
    fn view_fragment(self) -> &'static str {
        match self {
            Facing::South => "front view, facing the camera",
            Facing::North => "back view, facing away",
            Facing::East => "side view, facing right",
            Facing::West => "side view, facing left",
        }
    }

    /// A stable `snake_case` tag for the durable job record's `direction`.
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Facing::South => "south",
            Facing::North => "north",
            Facing::East => "east",
            Facing::West => "west",
        }
    }

    /// Maps the studio facing to the core [`AnchorDirection`] the directional
    /// cascade indexes by. Every facing has a direction (east is the flip of
    /// west, but the cascade still records the east cell).
    pub(crate) fn to_anchor_direction(self) -> AnchorDirection {
        match self {
            Facing::South => AnchorDirection::South,
            Facing::North => AnchorDirection::North,
            Facing::East => AnchorDirection::East,
            Facing::West => AnchorDirection::West,
        }
    }
}

/// The image-to-video model the Motion stage drives.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum I2vModel {
    /// `ByteDance` Seedance 2.0 (the backend default).
    Seedance,
    /// Wan 2.1 image-to-video.
    Wan,
}

impl I2vModel {
    fn label(self) -> &'static str {
        match self {
            I2vModel::Seedance => "Seedance 2.0",
            I2vModel::Wan => "Wan 2.1",
        }
    }

    /// The FAL endpoint id for this model, threaded into [`ai::AnimJob::i2v_model`].
    #[must_use]
    pub(crate) fn model_id(self) -> String {
        match self {
            I2vModel::Seedance => FAL_SEEDANCE.to_owned(),
            I2vModel::Wan => FAL_I2V.to_owned(),
        }
    }
}

/// How the Motion stage produces animation frames.
///
/// `Animated` is the image-to-video path: a clip from an i2v model, interpolating
/// true motion. `Static` is the i2v-optional path: one solid-magenta `rows × cols`
/// grid sheet, sliced into frames — no video model and no per-clip video API
/// cost, at the price of relying on the model's frame-to-frame consistency.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum GenMode {
    /// Image-to-video: generate and decode a clip (the i2v path).
    Animated,
    /// Static grid sheet: generate one sheet and slice the cells.
    Static,
}

/// Which Motion-stage producer a [`GenMode`] dispatches to. The Generate button
/// matches on this so the routing has a single, testable source of truth.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum GenProducer {
    /// Drive `start_clip` (the i2v path).
    Clip,
    /// Drive `start_static_sheet` (the grid-sheet path).
    StaticSheet,
}

impl GenMode {
    /// The Motion-inspector label for this mode.
    fn label(self) -> &'static str {
        match self {
            GenMode::Animated => "Animated (i2v)",
            GenMode::Static => "Static sheet",
        }
    }

    /// The producer the Generate button routes this mode to.
    pub(crate) fn producer(self) -> GenProducer {
        match self {
            GenMode::Animated => GenProducer::Clip,
            GenMode::Static => GenProducer::StaticSheet,
        }
    }
}

/// How a first-frame candidate came to be, for the lineage caption.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum FfOrigin {
    /// A from-scratch text-to-image generation.
    Fresh,
    /// An inpaint refinement of an earlier candidate.
    Inpaint,
}

/// One generated seed-pose candidate in the first-frame gallery.
pub(crate) struct FirstFrameCandidate {
    /// Decoded PNG bytes — for the texture and the i2v seed once approved.
    pub png: Vec<u8>,
    /// Lazily-built preview texture.
    pub texture: Option<egui::TextureHandle>,
    /// Gallery index this candidate was refined from, for the lineage caption.
    pub parent: Option<usize>,
    /// How this candidate came to be.
    pub origin: FfOrigin,
}

/// A paint-over mask aligned to the selected result. White cells mark the region
/// to repaint; exported to a PNG for the inpaint call. Edits accumulate a dirty
/// rect so the overlay texture is updated by sub-region ([`rebuild_mask_overlay`])
/// rather than re-uploaded whole each frame — the canvas's dirty-region pattern.
pub(crate) struct MaskOverlay {
    /// Mask width in pixels (matches the candidate image).
    pub width: u32,
    /// Mask height in pixels.
    pub height: u32,
    /// One flag per pixel, row-major; `true` means "repaint here".
    pub cells: Vec<bool>,
    /// The region of `cells` changed since the last texture update, as
    /// `[x, y, w, h]` in mask pixels. `None` means nothing to upload.
    pub dirty: Option<[u32; 4]>,
    /// The box gizmo's last-rasterized bounding box, so the next rasterize can
    /// clear the cells it vacated without scanning the whole mask.
    pub box_bbox: Option<[u32; 4]>,
    /// Lazily-built overlay texture (red where set), drawn over the candidate.
    pub texture: Option<egui::TextureHandle>,
}

/// The smallest `[x, y, w, h]` rect covering both `a` and `b`.
fn rect_union(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
    let min_x = a[0].min(b[0]);
    let min_y = a[1].min(b[1]);
    let max_x = (a[0] + a[2]).max(b[0] + b[2]);
    let max_y = (a[1] + a[3]).max(b[1] + b[3]);
    [min_x, min_y, max_x - min_x, max_y - min_y]
}

impl MaskOverlay {
    /// An empty mask sized to `width` x `height`, dirty over its full extent so
    /// the first rebuild allocates the texture at the right size.
    fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            cells: vec![false; (width as usize) * (height as usize)],
            dirty: Some([0, 0, width, height]),
            box_bbox: None,
            texture: None,
        }
    }

    /// Whether no pixel is marked.
    fn is_empty(&self) -> bool {
        !self.cells.iter().any(|&c| c)
    }

    /// Unions `[x, y, w, h]` into the pending dirty rect. A zero-area rect is a
    /// no-op.
    fn mark(&mut self, x: u32, y: u32, w: u32, h: u32) {
        if w == 0 || h == 0 {
            return;
        }
        let rect = [x, y, w, h];
        self.dirty = Some(self.dirty.map_or(rect, |prev| rect_union(prev, rect)));
    }

    /// Clears every marked pixel and dirties the whole mask.
    fn clear(&mut self) {
        self.cells.iter_mut().for_each(|c| *c = false);
        self.box_bbox = None;
        self.mark(0, 0, self.width, self.height);
    }

    /// Marks a filled disc of `radius` mask-pixels centred on `(cx, cy)` in
    /// mask-pixel coordinates, dirtying just that disc's bounding box.
    fn stamp(&mut self, cx: f32, cy: f32, radius: f32) {
        let r = radius.max(0.5);
        let (w, h) = (self.width as i32, self.height as i32);
        let r2 = r * r;
        let min_x = ((cx - r).floor() as i32).clamp(0, w - 1);
        let max_x = ((cx + r).ceil() as i32).clamp(0, w - 1);
        let min_y = ((cy - r).floor() as i32).clamp(0, h - 1);
        let max_y = ((cy + r).ceil() as i32).clamp(0, h - 1);
        if max_x < min_x || max_y < min_y {
            return;
        }
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let dx = x as f32 + 0.5 - cx;
                let dy = y as f32 + 0.5 - cy;
                if dx * dx + dy * dy <= r2 {
                    self.cells[(y as usize) * (self.width as usize) + (x as usize)] = true;
                }
            }
        }
        self.mark(min_x as u32, min_y as u32, (max_x - min_x + 1) as u32, (max_y - min_y + 1) as u32);
    }
}

/// How the inpaint mask is shaped: a freeform brush, or a transformable box.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum MaskTool {
    /// Paint cells with a round brush.
    Brush,
    /// A rectangle you move, scale, and rotate; its interior is the mask.
    Box,
}

/// Fills `mask` with the box gizmo's rotated interior. Only the union of the
/// previous and new bounding box is rewritten, so per-drag cost is the rect's
/// area rather than the whole mask. The gizmo itself is the shared
/// [`crate::gizmo::BoxGizmo`]; this is the studio's mask consumer of it (the
/// free-transform tool consumes the same gizmo by resampling pixels instead).
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::cast_precision_loss)]
fn rasterize_gizmo(giz: &BoxGizmo, mask: &mut MaskOverlay) {
    let new_bbox = giz.aabb(mask.width, mask.height);
    let region = mask.box_bbox.map_or(new_bbox, |prev| rect_union(prev, new_bbox));
    let [rx, ry, rw, rh] = region;
    for y in ry..ry + rh {
        for x in rx..rx + rw {
            let inside = giz.contains(x as f32 + 0.5, y as f32 + 0.5);
            mask.cells[(y as usize) * (mask.width as usize) + x as usize] = inside;
        }
    }
    mask.box_bbox = Some(new_bbox);
    mask.mark(rx, ry, rw, rh);
}

/// Where a Normalize-stage "edit pixels" round trip returns to: the clip and the
/// frame index within it whose buffer the edit replaces, so the edit flows back
/// into the pick set without re-picking.
#[derive(Clone, Copy)]
pub(crate) struct PickReturn {
    /// The selected clip candidate index.
    pub clip: usize,
    /// The frame index within the clip (a pick value, not a strip slot).
    pub frame: usize,
}

/// Breadcrumb for a hand-edit round trip: the sprite to re-select on return and
/// the temporary edit sprite to drop. Without [`Self::pick`] the edited image
/// lands in the first-frame thread (the seed-pose hand edit); with it, the edit
/// returns into that clip frame's buffer (the Normalize-stage frame edit).
pub(crate) struct StudioReturn {
    /// The sprite that was active before hand-editing, restored on return.
    pub origin: Option<SpriteRef>,
    /// The scratch sprite created for the edit, deleted on return.
    pub edit: SpriteRef,
    /// When set, the edit returns into this clip frame's buffer instead of the
    /// first-frame thread.
    pub pick: Option<PickReturn>,
}

/// The center-viewport refine state shared by the generation stages: the pan and
/// zoom of the selected result, plus the inpaint mask painted over it. Lives on
/// each stage so the Anchor and First-frame centers behave identically.
pub(crate) struct RefineView {
    /// Zoom factor over the fit-to-area scale (1.0 = fit-to-view).
    pub zoom: f32,
    /// Pan offset in screen pixels from the centered position.
    pub pan: egui::Vec2,
    /// Whether the brush paint tool is armed.
    pub painting: bool,
    /// Inpaint mask aligned to the selected result.
    pub mask: Option<MaskOverlay>,
    /// Mask brush radius in mask pixels.
    pub brush: f32,
    /// Which mask tool shapes the mask.
    pub mask_tool: MaskTool,
    /// The box gizmo, when the box tool is in use (sized to the result).
    pub gizmo: Option<BoxGizmo>,
    /// The gizmo handle currently being dragged.
    pub gizmo_drag: Option<GizmoHandle>,
    /// Prompt for the inpaint refinement.
    pub inpaint_prompt: String,
}

impl Default for RefineView {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            pan: egui::Vec2::ZERO,
            painting: false,
            mask: None,
            brush: 4.0,
            mask_tool: MaskTool::Brush,
            gizmo: None,
            gizmo_drag: None,
            inpaint_prompt: String::new(),
        }
    }
}

impl RefineView {
    /// Drops the mask, gizmo, and pan/zoom — called when a different result is
    /// selected so the center starts fresh.
    pub(crate) fn reset(&mut self) {
        self.zoom = 1.0;
        self.pan = egui::Vec2::ZERO;
        self.painting = false;
        self.mask = None;
        self.gizmo = None;
        self.gizmo_drag = None;
    }
}

/// The first-frame stage's conversational generate-and-refine thread: a prompt,
/// the candidate images it produced (newest last, with lineage), the selected
/// candidate, and the shared center-viewport refine state.
pub(crate) struct GenThread {
    /// The positive prompt for the next text-to-image generation.
    pub prompt: String,
    /// Requested candidate count (1-4).
    pub variants: u32,
    /// The candidate gallery, newest last.
    pub candidates: Vec<FirstFrameCandidate>,
    /// Selected gallery index, shown large in the center viewport.
    pub selected: Option<usize>,
    /// Job status for this thread.
    pub status: JobStatus,
    /// Cancel handle for an in-flight generation.
    pub cancel: Option<CancellationToken>,
    /// Monotonic generation id; bumped per generate and cancel so a superseded
    /// run's late messages are dropped.
    pub epoch: u64,
    /// The center-viewport refine state for the selected candidate.
    pub view: RefineView,
    /// The generation reveal effect for this stage's center viewport.
    pub reveal: crate::reveal::RevealState,
}

impl Default for GenThread {
    fn default() -> Self {
        Self {
            prompt: String::new(),
            variants: 2,
            candidates: Vec::new(),
            selected: None,
            status: JobStatus::Idle,
            cancel: None,
            epoch: 0,
            view: RefineView::default(),
            reveal: crate::reveal::RevealState::default(),
        }
    }
}

/// Max per-sprite sessions kept in the map (and the sidecar). Past this, the
/// heaviest sessions are dropped first — mirrors the `MAX_JOBS` cap on the
/// durable job queue and bounds the sidecar's on-disk size.
pub(crate) const MAX_STUDIO_SESSIONS: usize = 24;

/// The serializable subset of a studio session, kept per sprite so switching
/// sprites away and back — and restarting the app — resumes where the artist
/// left off.
///
/// This holds only durable facts: the stage, the framing (kind / facing), the
/// i2v model, the approved seed pose (the one heavy field — a PNG, kept because
/// it is the single input that drives a clip and is expensive to regenerate),
/// the remove-on-land flag, and the motion parameters that live on [`ShellApp`]
/// (motion prompt, target frames, fps, seed). Transient view state — texture
/// handles, the reveal effect, drag state, and the inpaint mask paint state —
/// is rebuilt lazily and never serialized.
///
/// Interim home: this persists to an app-data sidecar
/// (`studio-sessions.json` under `eframe::storage_dir`), not the project
/// archive. When the io crate lands and `.pixhaus` gains save/open, this moves
/// onto the per-entity `AiMetadata` or a new `Project` field and rides the
/// project archive; the serialization shape here is designed so that is a
/// type-move, not a redesign.
///
/// Every field carries `#[serde(default)]` so an older sidecar (one written
/// before a field existed, or a missing sidecar) still deserializes —
/// forward-compat matters more than strictness for app-data sidecars.
#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct StudioSession {
    /// The stage in view when the session was flushed.
    #[serde(default = "default_stage")]
    pub stage: StudioStage,
    /// Animation kind, scaffolding the motion prompt.
    #[serde(default = "default_kind")]
    pub kind: AnimKind,
    /// Facing direction the generation conditions on.
    #[serde(default = "default_facing")]
    pub facing: Facing,
    /// The image-to-video model the Motion stage drives.
    #[serde(default = "default_i2v_model")]
    pub i2v_model: I2vModel,
    /// How the Motion stage produces frames: an i2v clip or a sliced grid sheet.
    #[serde(default = "default_gen_mode")]
    pub gen_mode: GenMode,
    /// Static-sheet grid rows.
    #[serde(default = "default_grid_rows")]
    pub grid_rows: u32,
    /// Static-sheet grid columns.
    #[serde(default = "default_grid_cols")]
    pub grid_cols: u32,
    /// The approved seed pose (PNG), the single input that drives the clip.
    /// The one heavy field — kept on purpose.
    #[serde(default)]
    pub approved_first_frame: Option<Vec<u8>>,
    /// Whether Land bakes the chroma key into the landed loop.
    #[serde(default)]
    pub remove_on_land: bool,
    /// The motion (choreography) prompt.
    #[serde(default)]
    pub motion: String,
    /// Requested loop frame count.
    #[serde(default = "default_target_frames")]
    pub target_frames: u32,
    /// Playback FPS for the generated animation.
    #[serde(default = "default_fps")]
    pub fps: u32,
    /// Pinned RNG seed, or `None` for a random seed each run.
    #[serde(default)]
    pub seed: Option<u64>,
}

fn default_stage() -> StudioStage {
    StudioStage::Anchor
}

fn default_kind() -> AnimKind {
    AnimKind::Walk
}

fn default_facing() -> Facing {
    Facing::East
}

fn default_i2v_model() -> I2vModel {
    I2vModel::Seedance
}

fn default_gen_mode() -> GenMode {
    GenMode::Animated
}

fn default_grid_rows() -> u32 {
    2
}

fn default_grid_cols() -> u32 {
    3
}

fn default_target_frames() -> u32 {
    6
}

fn default_fps() -> u32 {
    10
}

impl Default for StudioSession {
    fn default() -> Self {
        Self {
            stage: default_stage(),
            kind: default_kind(),
            facing: default_facing(),
            i2v_model: default_i2v_model(),
            gen_mode: default_gen_mode(),
            grid_rows: default_grid_rows(),
            grid_cols: default_grid_cols(),
            approved_first_frame: None,
            remove_on_land: false,
            motion: String::new(),
            target_frames: default_target_frames(),
            fps: default_fps(),
            seed: None,
        }
    }
}

/// On-disk shape of the per-sprite session sidecar. Entries are a list of
/// `(SpriteId, StudioSession)` pairs rather than a JSON object so the format
/// never depends on serde's integer-map-key coercion. `#[serde(default)]` makes
/// an empty or missing file deserialize to an empty session set.
#[derive(Default, Serialize, Deserialize)]
struct StudioSessionsSidecar {
    #[serde(default)]
    sessions: Vec<(SpriteId, StudioSession)>,
}

/// The studio-sessions sidecar path under eframe's app-data storage, a sibling
/// of `gpu.json` and the `animation-jobs` dir. `None` when no storage dir is
/// resolvable.
fn sessions_path() -> Option<std::path::PathBuf> {
    eframe::storage_dir("pixhaus").map(|dir| dir.join("studio-sessions.json"))
}

/// Serializes the per-sprite sessions to the sidecar's JSON bytes. The pure,
/// filesystem-free core of [`save_studio_sessions`], so the round-trip is
/// testable without a storage dir.
fn encode_sessions(sessions: &HashMap<SpriteId, StudioSession>) -> Result<Vec<u8>, serde_json::Error> {
    let sidecar = StudioSessionsSidecar {
        sessions: sessions.iter().map(|(id, s)| (*id, s.clone())).collect(),
    };
    serde_json::to_vec(&sidecar)
}

/// Parses the sidecar's JSON bytes into a per-sprite session map. The pure,
/// filesystem-free core of [`load_studio_sessions`].
fn decode_sessions(bytes: &[u8]) -> Result<HashMap<SpriteId, StudioSession>, serde_json::Error> {
    let sidecar: StudioSessionsSidecar = serde_json::from_slice(bytes)?;
    Ok(sidecar.sessions.into_iter().collect())
}

/// Loads the persisted per-sprite sessions. A missing or unreadable sidecar
/// yields an empty map — equivalent to a fresh install. Never errors up to the
/// UI: a corrupt sidecar is logged and treated as empty.
pub(crate) fn load_studio_sessions() -> HashMap<SpriteId, StudioSession> {
    let Some(path) = sessions_path() else {
        return HashMap::new();
    };
    let Ok(bytes) = std::fs::read(&path) else {
        return HashMap::new();
    };
    match decode_sessions(&bytes) {
        Ok(map) => map,
        Err(err) => {
            tracing::warn!(error = %err, "failed to parse studio-sessions sidecar; starting empty");
            HashMap::new()
        }
    }
}

/// Persists the per-sprite sessions to the sidecar. A write failure degrades to
/// not-persisted: it is logged and swallowed, never propagated, so a full disk
/// cannot crash the studio. Matches the `gpu.rs` / `anim_jobs.rs` sidecar policy.
pub(crate) fn save_studio_sessions(sessions: &HashMap<SpriteId, StudioSession>) {
    let Some(path) = sessions_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        if let Err(err) = std::fs::create_dir_all(parent) {
            tracing::warn!(error = %err, "failed to create storage dir for studio sessions");
            return;
        }
    }
    match encode_sessions(sessions) {
        Ok(bytes) => {
            if let Err(err) = std::fs::write(&path, bytes) {
                tracing::warn!(error = %err, "failed to persist studio sessions");
            }
        }
        Err(err) => tracing::warn!(error = %err, "failed to serialize studio sessions"),
    }
}

/// The editable normalize knobs that have no other home on the studio. The
/// canvas comes from the active sprite, the chroma key from the background-key
/// fields, and the component mode from the Land keying toggles, so those are not
/// duplicated here; this carries only the measurement knobs the Normalize
/// inspector exposes. Folded into [`NormalizeCacheKey`] so a change re-runs the
/// pass in place.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct NormalizeKnobs {
    /// Alpha at or below this counts as background when measuring the bbox.
    pub alpha_threshold: u8,
    /// Reference visible height every subject is scaled to. `None` infers it
    /// from the tallest picked subject.
    pub reference_height: Option<u32>,
    /// Pixels the foot baseline sits above the canvas bottom.
    pub bottom_margin: u32,
}

impl Default for NormalizeKnobs {
    fn default() -> Self {
        // Matches the values the static-sheet path used before the knobs existed,
        // so an untouched inspector reproduces the prior cut exactly.
        Self {
            alpha_threshold: 8,
            reference_height: None,
            bottom_margin: 0,
        }
    }
}

/// The inputs that determine the cached [`StudioState::normalize`] result. When
/// any of these change, the cached normalization is stale and must be recomputed
/// before Land trusts it. Cheap to compare (no buffer hashing): the selected
/// clip, its pick set, the target canvas, and the key settings fully determine
/// the normalize output, and the pick set already mutates whenever the clip's
/// frames change.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct NormalizeCacheKey {
    /// The selected clip candidate index.
    pub clip: usize,
    /// The picked frame indices, in order.
    pub picks: Vec<usize>,
    /// The target canvas size (sprite width, height).
    pub canvas: (u32, u32),
    /// Whether the chroma key is baked in on Land.
    pub remove_on_land: bool,
    /// The chroma key colour, as `(r, g, b)`.
    pub key_color: (u8, u8, u8),
    /// The chroma key tolerance.
    pub key_tolerance: u8,
    /// Whether Land keeps every part above the min area (vs the largest only).
    pub all_parts: bool,
    /// The min opaque-pixel area a part needs to survive the `All` mode.
    pub min_area: u32,
    /// The alpha background threshold the measurement uses.
    pub alpha_threshold: u8,
    /// The reference height subjects scale toward (`None` infers it).
    pub reference_height: Option<u32>,
    /// The foot-baseline bottom margin.
    pub bottom_margin: u32,
}

/// The studio's whole session state. Lives on [`ShellApp`]; the clip candidates
/// themselves live in `anim_*`, which the clip/pick/land stages drive.
pub(crate) struct StudioState {
    /// The stage currently in view.
    pub stage: StudioStage,
    /// Animation kind, scaffolding the motion prompt.
    pub kind: AnimKind,
    /// Facing direction the generation conditions on.
    pub facing: Facing,
    /// The first-frame stage's conversational generation thread.
    pub frame_gen: GenThread,
    /// The Anchor stage's center-viewport refine state for the selected cockpit
    /// result (the cockpit gallery itself lives in `rs_candidates` on `ShellApp`).
    pub anchor_view: RefineView,
    /// The cockpit result selected for the Anchor stage's center viewport, an
    /// index into `rs_candidates`.
    pub anchor_selected: Option<usize>,
    /// The Anchor stage's generation reveal effect for its center viewport.
    pub anchor_reveal: crate::reveal::RevealState,
    /// The approved seed pose (PNG), the single input that drives the clip.
    pub approved_first_frame: Option<Vec<u8>>,
    /// The image-to-video model the Motion stage drives.
    pub i2v_model: I2vModel,
    /// How the Motion stage produces frames: an i2v clip or a sliced grid sheet.
    pub gen_mode: GenMode,
    /// Static-sheet grid rows (consulted only in [`GenMode::Static`]).
    pub grid_rows: u32,
    /// Static-sheet grid columns (consulted only in [`GenMode::Static`]).
    pub grid_cols: u32,
    /// The raw generated grid sheet, retained for the Sheet stage to show and
    /// re-cut. `None` until a static sheet lands.
    pub sheet: Option<PixelBuffer>,
    /// The slice geometry for the retained sheet, seeded uniform from the
    /// generation grid and adjusted by the slice gizmo.
    pub slice: SliceGrid,
    /// The per-cell size the retained sheet was generated for (`cell_w`,
    /// `cell_h`).
    pub sheet_cell: (u32, u32),
    /// Cached texture for the retained sheet, built lazily on the Sheet stage and
    /// dropped when a new sheet lands.
    pub sheet_texture: Option<egui::TextureHandle>,
    /// Whether the Sheet stage shows the sheet at 1:1 (else fit-to-view).
    pub sheet_one_to_one: bool,
    /// Which slice edge a Sheet-stage drag is currently moving, set on press.
    pub sheet_drag: Option<SliceEdge>,
    /// The slice spec the cached preview frames were last cut with, so a changed
    /// spec triggers a live re-slice. `None` until the first re-slice runs.
    pub sheet_sliced: Option<SliceGrid>,
    /// Whether the second clip is shown beside the first for comparison.
    pub compare: bool,
    /// The other clip index in a side-by-side comparison.
    pub compare_other: Option<usize>,
    /// Set once Land integrated a loop, so Land can confirm the result.
    pub landed: bool,
    /// The reviewed normalization output, cached transiently so the Normalize
    /// stage and Land share one pass: Land commits exactly these frames rather
    /// than re-normalizing. `None` until the Normalize stage runs; invalidated
    /// when [`NormalizeCacheKey`] changes (a different clip, pick set, canvas,
    /// or key). Never serialized — it is derived from the picks.
    pub normalize: Option<NormalizeResult>,
    /// The inputs the cached [`StudioState::normalize`] was computed from. A
    /// mismatch against the current inputs invalidates the cache.
    pub normalize_key: Option<NormalizeCacheKey>,
    /// Lazily-built textures for the normalized strip, one per cached frame.
    /// Cleared whenever the cache is recomputed.
    pub normalize_textures: Vec<Option<egui::TextureHandle>>,
    /// The editable normalize knobs the Normalize inspector drives. Folded into
    /// [`NormalizeCacheKey`] so a change re-runs the pass in place.
    pub norm_knobs: NormalizeKnobs,
    /// Which scrubber handle a drag is currently moving.
    pub drag_handle: Option<ScrubHandle>,
    /// Whether the raw-clip player loops at the end (vs stopping). Defaults on.
    pub loop_playback: bool,
    /// Whether the key eyedropper is armed: a click on the clip frame samples
    /// its colour into `bg_key_color`.
    pub picking_key: bool,
    /// Whether the player shows each frame chroma-keyed (backdrop removed).
    pub keyed_preview: bool,
    /// Whether Land bakes the chroma key into the landed loop. Turns on when a
    /// key is chosen; off leaves removal to the timeline op.
    pub remove_on_land: bool,
    /// Land's component policy when keying: `false` keeps only the largest
    /// component (the hero body); `true` keeps every part above
    /// [`StudioState::land_min_area`]. Only consulted when `remove_on_land`.
    pub land_all_parts: bool,
    /// Minimum opaque-pixel area a detached part needs to survive Land's `All`
    /// mode. Small by default so only true noise is dropped.
    pub land_min_area: u32,
    /// The sprite this transient view belongs to. When the active sprite
    /// changes, the current view is flushed into `studio_sessions` for this
    /// owner and a fresh view is hydrated from the map for the new owner, so one
    /// sprite's work never leaks into another's and switching back restores it.
    pub owner: Option<SpriteId>,
}

/// The scrubber handle a drag is moving.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ScrubHandle {
    /// The loop in-point.
    Start,
    /// The loop out-point (exclusive).
    End,
    /// The playhead.
    Playhead,
}

impl Default for StudioState {
    fn default() -> Self {
        Self {
            stage: StudioStage::Anchor,
            kind: AnimKind::Walk,
            facing: Facing::East,
            frame_gen: GenThread::default(),
            anchor_view: RefineView::default(),
            anchor_selected: None,
            anchor_reveal: crate::reveal::RevealState::default(),
            approved_first_frame: None,
            i2v_model: I2vModel::Seedance,
            gen_mode: default_gen_mode(),
            grid_rows: default_grid_rows(),
            grid_cols: default_grid_cols(),
            sheet: None,
            slice: SliceGrid::uniform(default_grid_rows(), default_grid_cols()),
            sheet_cell: (0, 0),
            sheet_texture: None,
            sheet_one_to_one: false,
            sheet_drag: None,
            sheet_sliced: None,
            compare: false,
            compare_other: None,
            landed: false,
            normalize: None,
            normalize_key: None,
            normalize_textures: Vec::new(),
            norm_knobs: NormalizeKnobs::default(),
            drag_handle: None,
            loop_playback: true,
            picking_key: false,
            keyed_preview: false,
            remove_on_land: false,
            land_all_parts: false,
            land_min_area: DEFAULT_LAND_MIN_AREA,
            owner: None,
        }
    }
}

impl StudioState {
    /// The seed-pose prompt for a given facing. Shared so the First-frame
    /// Direction dial can recognize its own untouched scaffold and keep it in
    /// step without duplicating the format string.
    fn pose_prompt_for(facing: Facing) -> String {
        format!("character seed pose, {}", facing.view_fragment())
    }

    /// The motion prompt scaffolded from a given kind and facing. Shared with
    /// the Motion stage's kind dial for the same reason as [`Self::pose_prompt_for`].
    fn motion_prompt_for(kind: AnimKind, facing: Facing) -> String {
        match kind.motion_fragment() {
            Some(kind) => format!("{kind}, {}", facing.view_fragment()),
            None => facing.view_fragment().to_owned(),
        }
    }

    /// The default seed-pose prompt built from the current framing.
    fn default_pose_prompt(&self) -> String {
        Self::pose_prompt_for(self.facing)
    }

    /// The motion prompt scaffolded from the current framing.
    fn motion_scaffold(&self) -> String {
        Self::motion_prompt_for(self.kind, self.facing)
    }

    /// A one-line framing summary for the header.
    fn framing_label(&self) -> String {
        format!("{} · {}", self.kind.label(), self.facing.label())
    }
}

impl ShellApp {
    /// Whether the animation studio is the active surface (Create mode +
    /// `Studio` view). Drives the full-screen takeover in `app.rs`.
    #[must_use]
    pub(crate) fn studio_active(&self) -> bool {
        self.workspace == Workspace::Create
    }

    /// Enters the studio full-screen, seeding the first-frame prompt from the
    /// anchor framing when it is still blank.
    pub(crate) fn enter_studio(&mut self) {
        self.workspace = Workspace::Create;
        self.studio_library_open = false;
        self.anim_set_open = false;
        self.studio.stage = StudioStage::Anchor;
        self.studio.landed = false;
        // A fresh Create session inherits the project's AI defaults (quality
        // tier, candidate count) the artist set in the Project AI section.
        self.reset_cockpit_form_defaults();
        if self.studio.frame_gen.prompt.trim().is_empty() {
            self.studio.frame_gen.prompt = self.studio.default_pose_prompt();
        }
        if self.anim_motion.trim().is_empty() {
            self.anim_motion = self.studio.motion_scaffold();
        }
    }

    /// Leaves the studio for the normal drawing editor, dropping any in-progress
    /// preview. Studio session state is kept so re-entering resumes where it left.
    pub(crate) fn exit_studio(&mut self) {
        self.studio.frame_gen.view.painting = false;
        self.leave_animation_preview();
        self.set_workspace(Workspace::Draw);
    }

    /// Lays out the studio's regions under its header: a left panel with the
    /// sprite gallery and the stages rail, the center stage surface, and the
    /// right inspector.
    pub(crate) fn studio_view(&mut self, ui: &mut egui::Ui) {
        self.sync_studio_owner();
        egui::Panel::top("studio_header").resizable(false).show_inside(ui, |ui| self.studio_header(ui));
        if self.studio_library_open {
            // The library overlay hosts two browsers behind a sub-tab: the
            // composition library (prompts/structures/styles, with the record
            // editor in the right dock) and the asset library (references,
            // character cards, style swatches), which fills the center alone.
            egui::Panel::top("studio_library_tabs").resizable(false).show_inside(ui, |ui| {
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.asset_browser_open, false, format!("{} Composition", crate::icons::LIBRARY));
                    ui.selectable_value(&mut self.asset_browser_open, true, format!("{} Assets", crate::icons::CARD));
                });
                ui.add_space(2.0);
            });
            if self.asset_browser_open {
                egui::CentralPanel::default().show_inside(ui, |ui| self.asset_library_view(ui));
            } else {
                egui::Panel::right("studio_inspector")
                    .resizable(true)
                    .default_size(340.0)
                    .show_inside(ui, |ui| {
                        egui::ScrollArea::vertical()
                            .auto_shrink([false, false])
                            .id_salt("library_editor_scroll")
                            .show(ui, |ui| self.library_editor(ui));
                    });
                egui::CentralPanel::default().show_inside(ui, |ui| self.library_view(ui));
            }
            return;
        }
        if self.anim_set_open {
            // The coverage-grid overlay fills the center alone: a read-only
            // directional-cascade plan over the active entity's anchor.
            egui::CentralPanel::default().show_inside(ui, |ui| {
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| self.anim_set_view(ui));
            });
            return;
        }
        egui::Panel::left("studio_left")
            .resizable(true)
            .default_size(240.0)
            .show_inside(ui, |ui| self.studio_left(ui));
        egui::Panel::right("studio_inspector")
            .resizable(true)
            .default_size(340.0)
            .show_inside(ui, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| self.studio_inspector(ui));
            });
        egui::CentralPanel::default().show_inside(ui, |ui| self.studio_surface(ui));
    }

    /// The header: back-to-editor exit, the title, the Library overlay toggle,
    /// and the framing summary.
    fn studio_header(&mut self, ui: &mut egui::Ui) {
        let mut back = false;
        ui.horizontal(|ui| {
            if ui.button(format!("{} Back to editor", crate::icons::LEFT)).clicked() {
                back = true;
            }
            ui.separator();
            ui.heading(format!("{} AI studio", crate::icons::SPARKLE));
            ui.separator();
            if self.studio_library_open {
                if ui.button(format!("{} Done", crate::icons::CHECK)).clicked() {
                    self.studio_library_open = false;
                }
            } else if ui
                .button(format!("{} Library", crate::icons::LIBRARY))
                .on_hover_text("Saved prompt templates, structures, and styles")
                .clicked()
            {
                self.studio_library_open = true;
            }
            if self.anim_set_open {
                if ui.button(format!("{} Done", crate::icons::CHECK)).clicked() {
                    self.anim_set_open = false;
                }
            } else if ui
                .button(format!("{} Coverage", crate::icons::FILM))
                .on_hover_text("Directional-cascade coverage grid for this entity")
                .clicked()
            {
                self.studio_library_open = false;
                self.anim_set_open = true;
            }
            // The framing summary stays in the header at every stage: it is the
            // at-a-glance readout of the direction/kind framing — including
            // values seeded from the coverage grid — now that the dials live in
            // the First-frame and Motion stages rather than the Anchor inspector.
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(egui::RichText::new(self.studio.framing_label()).weak());
            });
        });
        if back {
            self.exit_studio();
        }
    }

    /// The left panel: the sprite gallery on top, the stages rail below. The
    /// gallery is the full sprite browser, so every sprite is selectable and
    /// manageable without leaving the studio.
    fn studio_left(&mut self, ui: &mut egui::Ui) {
        ui.add_space(6.0);
        ui.label(egui::RichText::new(format!("{} Sprites", crate::icons::IMAGE)).strong());
        ui.add_space(4.0);
        let gallery_h = (ui.available_height() * 0.5).max(120.0);
        egui::ScrollArea::vertical()
            .id_salt("studio_gallery")
            .max_height(gallery_h)
            .auto_shrink([false, false])
            .show(ui, |ui| self.library_panel(ui));
        ui.separator();
        self.studio_rail(ui);
    }

    /// Snapshots the current transient session into a [`StudioSession`] — the
    /// serializable subset (stage, framing, model, approved pose, motion params),
    /// dropping the transient view (textures, reveal, drag, mask paint).
    fn studio_session_snapshot(&self) -> StudioSession {
        StudioSession {
            stage: self.studio.stage,
            kind: self.studio.kind,
            facing: self.studio.facing,
            i2v_model: self.studio.i2v_model,
            gen_mode: self.studio.gen_mode,
            grid_rows: self.studio.grid_rows,
            grid_cols: self.studio.grid_cols,
            approved_first_frame: self.studio.approved_first_frame.clone(),
            remove_on_land: self.studio.remove_on_land,
            motion: self.anim_motion.clone(),
            target_frames: self.anim_target_frames,
            fps: self.anim_fps,
            seed: self.anim_seed_fixed.then_some(self.anim_seed),
        }
    }

    /// Replaces the transient session view with a fresh one hydrated from
    /// `session`, owned by `owner`. The clip candidates and any in-flight job are
    /// dropped by the caller; the transient view (textures, reveal, mask paint)
    /// rebuilds lazily from the durable facts.
    fn hydrate_studio_session(&mut self, owner: Option<SpriteId>, session: &StudioSession) {
        let mut state = StudioState {
            stage: session.stage,
            kind: session.kind,
            facing: session.facing,
            approved_first_frame: session.approved_first_frame.clone(),
            i2v_model: session.i2v_model,
            gen_mode: session.gen_mode,
            grid_rows: session.grid_rows,
            grid_cols: session.grid_cols,
            remove_on_land: session.remove_on_land,
            owner,
            ..StudioState::default()
        };
        // Seed the first-frame prompt from the framing when the session carried
        // no prompt of its own.
        if state.frame_gen.prompt.trim().is_empty() {
            state.frame_gen.prompt = state.default_pose_prompt();
        }
        self.studio = state;
        self.anim_motion.clone_from(&session.motion);
        self.anim_target_frames = session.target_frames;
        self.anim_fps = session.fps;
        match session.seed {
            Some(seed) => {
                self.anim_seed_fixed = true;
                self.anim_seed = seed;
            }
            None => self.anim_seed_fixed = false,
        }
    }

    /// Flushes the active sprite's transient view into `studio_sessions` so the
    /// in-progress session is captured at the latest moment — called from
    /// `App::save` before the sidecar is written. Ownerless (no active sprite)
    /// is a no-op.
    pub(crate) fn flush_active_studio_session(&mut self) {
        if let Some(owner) = self.studio.owner {
            let snapshot = self.studio_session_snapshot();
            self.studio_sessions.insert(owner, snapshot);
            self.prune_studio_sessions();
        }
    }

    /// Switches the studio session when the active sprite changes: flush the
    /// current transient view into `studio_sessions` for the old owner, then
    /// hydrate a fresh view from the map for the new owner (default if absent).
    /// One sprite's candidates, masks, and clips never appear under another, and
    /// switching away and back restores the work. Cancels any in-flight
    /// first-frame job so its late results are dropped.
    fn sync_studio_owner(&mut self) {
        let active = self.doc.active_sprite().map(|s| s.id);
        if self.studio.owner == active {
            return;
        }
        if let Some(token) = self.studio.frame_gen.cancel.take() {
            token.cancel();
        }
        // Flush the outgoing owner's durable facts so a later switch back resumes
        // it. An ownerless transient session (no sprite) is not persisted.
        if let Some(prev) = self.studio.owner {
            let snapshot = self.studio_session_snapshot();
            self.studio_sessions.insert(prev, snapshot);
            self.prune_studio_sessions();
        }
        self.leave_animation_preview();
        self.anim_candidates.clear();
        self.anim_selected = None;
        let session = active.and_then(|id| self.studio_sessions.get(&id).cloned()).unwrap_or_default();
        self.hydrate_studio_session(active, &session);
    }

    /// Caps `studio_sessions` to [`MAX_STUDIO_SESSIONS`], dropping the heaviest
    /// (those carrying an approved-pose PNG) first, then any remainder. The cap
    /// bounds the sidecar so a long-lived install does not grow it without limit;
    /// the approved pose is the only large field, so evicting pose-carrying
    /// sessions first reclaims the most bytes per drop.
    fn prune_studio_sessions(&mut self) {
        while self.studio_sessions.len() > MAX_STUDIO_SESSIONS {
            let victim = self
                .studio_sessions
                .iter()
                .filter(|(id, _)| Some(**id) != self.studio.owner)
                .max_by_key(|(_, s)| s.approved_first_frame.as_ref().map_or(0, Vec::len))
                .map(|(id, _)| *id);
            match victim {
                Some(id) => {
                    self.studio_sessions.remove(&id);
                }
                // Nothing droppable (only the active owner is left): stop.
                None => break,
            }
        }
    }

    /// The stage that must finish before the current one is usable, if it is not
    /// done yet. `None` once the current stage's prerequisite is satisfied (and
    /// always for the Anchor stage, which has none).
    fn studio_unmet_prereq(&self) -> Option<StudioStage> {
        let idx = StudioStage::ALL.iter().position(|s| *s == self.studio.stage)?;
        let mut prev = *StudioStage::ALL.get(idx.checked_sub(1)?)?;
        // The Sheet stage only applies to the static-sheet path. On the i2v path
        // no sheet is retained, so it is never "complete" and would wrongly gate
        // the Clip stage behind it; skip it as a prerequisite then, falling back
        // to the stage before it (Motion).
        if prev == StudioStage::Sheet && self.studio.sheet.is_none() {
            let prev_idx = StudioStage::ALL.iter().position(|s| *s == prev)?;
            prev = *StudioStage::ALL.get(prev_idx.checked_sub(1)?)?;
        }
        if self.studio_stage_complete(prev) { None } else { Some(prev) }
    }

    /// A centered guide shown in place of a gated stage's surface, with a button
    /// that jumps back to the stage that must finish first.
    fn studio_gate_cta(&mut self, ui: &mut egui::Ui, prereq: StudioStage) {
        let mut go = false;
        ui.vertical_centered(|ui| {
            ui.add_space(ui.available_height() * 0.4);
            ui.label(egui::RichText::new(format!("Finish the {} stage first.", prereq.label())).weak());
            ui.add_space(8.0);
            if ui.button(format!("{} Go to {}", crate::icons::LEFT, prereq.label())).clicked() {
                go = true;
            }
        });
        if go {
            self.studio.stage = prereq;
        }
    }

    /// The stages rail: the pipeline as a vertical list, current stage
    /// highlighted, completed stages checked. Navigation is free.
    fn studio_rail(&mut self, ui: &mut egui::Ui) {
        ui.add_space(6.0);
        ui.label(egui::RichText::new("Stages").strong());
        ui.add_space(4.0);
        let accent = crate::theme::Palette::for_theme(ui.ctx().theme()).success;
        let mut goto: Option<StudioStage> = None;
        for stage in StudioStage::ALL {
            let done = self.studio_stage_complete(stage);
            let current = self.studio.stage == stage;
            let mark = if done { format!("{} ", crate::icons::CHECK) } else { "   ".to_owned() };
            let mut text = egui::RichText::new(format!("{mark}{}", stage.label()));
            if done {
                text = text.color(accent);
            }
            if ui.selectable_label(current, text).clicked() {
                goto = Some(stage);
            }
        }
        if let Some(stage) = goto {
            self.studio.stage = stage;
            self.studio.landed = false;
        }
    }

    /// Whether `stage` has produced its output (drives the rail checkmarks).
    fn studio_stage_complete(&self, stage: StudioStage) -> bool {
        match stage {
            StudioStage::Anchor => self.doc.active_anchor().is_some(),
            StudioStage::FirstFrame => self.studio.approved_first_frame.is_some(),
            StudioStage::Motion => !self.anim_candidates.is_empty(),
            // The Sheet stage is complete once a sheet has been retained and a
            // clip exists for it; the i2v path leaves it empty (no sheet) and the
            // rail just shows it unchecked, which is fine — it is a no-op there.
            StudioStage::Sheet => self.studio.sheet.is_some() && self.anim_card().is_some(),
            StudioStage::Clip => self.anim_card().is_some(),
            StudioStage::Pick => self.anim_card().is_some_and(|c| !c.picks.is_empty()),
            StudioStage::Normalize => self.studio.normalize.is_some(),
            StudioStage::Land => self.studio.landed,
        }
    }

    /// The center surface for the current stage.
    fn studio_surface(&mut self, ui: &mut egui::Ui) {
        if let Some(prereq) = self.studio_unmet_prereq() {
            self.studio_gate_cta(ui, prereq);
            return;
        }
        match self.studio.stage {
            StudioStage::Anchor => self.studio_anchor_surface(ui),
            StudioStage::FirstFrame => self.studio_first_frame_surface(ui),
            StudioStage::Motion => self.studio_motion_surface(ui),
            StudioStage::Sheet => self.studio_sheet_surface(ui),
            StudioStage::Clip => self.studio_clip_surface(ui),
            StudioStage::Pick => self.studio_pick_surface(ui),
            StudioStage::Normalize => self.studio_normalize_surface(ui),
            StudioStage::Land => self.studio_land_surface(ui),
        }
    }

    /// The right inspector for the current stage.
    fn studio_inspector(&mut self, ui: &mut egui::Ui) {
        if self.studio_unmet_prereq().is_some() {
            ui.label(egui::RichText::new("Finish the earlier stage to unlock this one.").weak());
            return;
        }
        if !self.backend_ready
            && !matches!(
                self.studio.stage,
                StudioStage::Sheet | StudioStage::Clip | StudioStage::Pick | StudioStage::Normalize
            )
        {
            self.key_entry(ui);
            ui.separator();
        }
        match self.studio.stage {
            StudioStage::Anchor => self.studio_anchor_inspector(ui),
            StudioStage::FirstFrame => self.studio_first_frame_inspector(ui),
            StudioStage::Motion => self.studio_motion_inspector(ui),
            StudioStage::Sheet => self.studio_sheet_inspector(ui),
            StudioStage::Clip => self.studio_clip_inspector(ui),
            StudioStage::Pick => self.studio_pick_inspector(ui),
            StudioStage::Normalize => self.studio_normalize_inspector(ui),
            StudioStage::Land => self.studio_land_inspector(ui),
        }
    }

    // ── Anchor stage ────────────────────────────────────────────────────────

    /// The Anchor stage's center surface is the cockpit's variant gallery.
    /// The Anchor stage's center surface: the selected cockpit result large in
    /// the shared pan/zoom viewport with its inpaint mask, or a hint.
    fn studio_anchor_surface(&mut self, ui: &mut egui::Ui) {
        // While a generation is in flight, the reveal effect takes the center;
        // it hands back to the static viewport the frame its snap completes.
        if self.reveal_effect_enabled && crate::reveal::render_reveal(&mut self.studio.anchor_reveal, ui, self.render_state.is_some()) {
            return;
        }
        let Some(i) = self.studio.anchor_selected else {
            let hint = if self.rs_candidates.is_empty() {
                "Describe the character in the inspector and Generate. Results appear there; pick one to refine here."
            } else {
                "Select a result from the inspector to preview and refine it."
            };
            centered_hint(ui, hint);
            return;
        };
        if i >= self.rs_candidates.len() {
            self.studio.anchor_selected = None;
            return;
        }
        let enabled = self.backend_ready;
        let cand = &mut self.rs_candidates[i];
        let view = &mut self.studio.anchor_view;
        let do_inpaint = refine_surface(ui, &cand.png, &mut cand.texture, view, enabled);
        if do_inpaint {
            self.start_anchor_inpaint();
        }
    }

    /// The Anchor stage's inspector: the cockpit composition controls and the
    /// results gallery (cards) below. The animation-kind and direction dials do
    /// not condition the anchor itself — they scaffold the seed-pose and motion
    /// prompts — so they live at their point of use (the First-frame and Motion
    /// stages), not here where they only crowd the canonical-reference step.
    fn studio_anchor_inspector(&mut self, ui: &mut egui::Ui) {
        self.anchor_inspector(ui);
        ui.add_space(8.0);
        ui.separator();
        self.anchor_gallery(ui);
    }

    // ── First-frame stage ─────────────────────────────────────────────────────

    /// The center surface: the selected candidate large in the shared pan/zoom
    /// viewport with its inpaint mask, or a hint when nothing is selected.
    fn studio_first_frame_surface(&mut self, ui: &mut egui::Ui) {
        if self.reveal_effect_enabled && crate::reveal::render_reveal(&mut self.studio.frame_gen.reveal, ui, self.render_state.is_some()) {
            return;
        }
        let Some(i) = self.studio.frame_gen.selected else {
            let hint = if self.studio.frame_gen.candidates.is_empty() {
                "Describe the pose in the inspector and generate. Results appear in the thread."
            } else {
                "Select a result from the thread to preview and refine it."
            };
            centered_hint(ui, hint);
            return;
        };
        let enabled = self.backend_ready;
        let thread = &mut self.studio.frame_gen;
        let cand = &mut thread.candidates[i];
        let do_inpaint = refine_surface(ui, &cand.png, &mut cand.texture, &mut thread.view, enabled);
        if do_inpaint {
            self.start_inpaint();
        }
    }

    /// The first-frame inspector delegates to the generation composer.
    fn studio_first_frame_inspector(&mut self, ui: &mut egui::Ui) {
        self.studio_gen_inspector(ui);
    }

    // ── First-frame conversational generation ─────────────────────────────────

    /// The first-frame generation thread.
    fn gen_ref(&self) -> &GenThread {
        &self.studio.frame_gen
    }

    /// The first-frame generation thread, mutably.
    fn gen_mut(&mut self) -> &mut GenThread {
        &mut self.studio.frame_gen
    }

    /// Selects candidate `i` and resets the center view (mask, gizmo, pan/zoom).
    fn select_candidate(&mut self, i: usize) {
        let thread = self.gen_mut();
        thread.selected = Some(i);
        thread.view.reset();
    }

    /// The first-frame generation composer: a prompt and Generate, the candidate
    /// thread (newest last), and the Approve / Hand-edit actions. The inpaint mask
    /// lives in the center viewport ([`refine_surface`]), not here.
    fn studio_gen_inspector(&mut self, ui: &mut egui::Ui) {
        let backend_ready = self.backend_ready;
        let ctx = ui.ctx().clone();
        let success = crate::theme::Palette::for_theme(ui.ctx().theme()).success;

        // The seed pose is the first directional frame, so the Direction dial
        // lives here, where it actually scaffolds this stage's pose prompt.
        ui.label(egui::RichText::new("Direction").strong());
        let prev_facing = self.studio.facing;
        let mut facing_changed = false;
        ui.horizontal_wrapped(|ui| {
            for facing in Facing::ALL {
                if ui.selectable_value(&mut self.studio.facing, facing, facing.label()).changed() {
                    facing_changed = true;
                }
            }
        });
        // Keep the prompt in step with the dial while it is still the untouched
        // scaffold for the old direction; a hand-edited prompt is never clobbered.
        if facing_changed && self.studio.frame_gen.prompt == StudioState::pose_prompt_for(prev_facing) {
            self.studio.frame_gen.prompt = StudioState::pose_prompt_for(self.studio.facing);
        }
        if ui
            .small_button("Apply to prompt")
            .on_hover_text("Replace the prompt with a seed pose for this direction")
            .clicked()
        {
            let prompt = self.studio.default_pose_prompt();
            self.studio.frame_gen.prompt = prompt;
        }
        ui.add_space(8.0);
        ui.separator();

        let do_generate;
        let mut do_cancel = false;
        let mut select: Option<usize> = None;
        {
            let thread = self.gen_mut();
            let busy = matches!(thread.status, JobStatus::Running(_));
            ui.label(egui::RichText::new("Prompt").strong());
            ui.add(
                egui::TextEdit::multiline(&mut thread.prompt)
                    .hint_text("a small knight standing, neutral pose")
                    .desired_rows(2)
                    .desired_width(f32::INFINITY),
            );
            ui.add(egui::Slider::new(&mut thread.variants, 1..=4).text("variants"));
            do_generate = ui
                .add_enabled(
                    backend_ready && !busy,
                    egui::Button::new(format!("{} Generate seed pose", crate::icons::SPARKLE)),
                )
                .clicked();
            studio_status_line(ui, &thread.status);
            if busy {
                do_cancel = ui.button("Cancel").clicked();
            }

            ui.add_space(8.0);
            ui.separator();
            ui.label(egui::RichText::new("Thread").strong());
            for cand in &mut thread.candidates {
                if cand.texture.is_none() {
                    cand.texture = load_png_texture(&ctx, "studio_gen", &cand.png);
                }
            }
            if thread.candidates.is_empty() {
                ui.label(egui::RichText::new("No results yet.").small().weak());
            }
            for (i, cand) in thread.candidates.iter().enumerate() {
                let selected = thread.selected == Some(i);
                ui.push_id(("frame_thread", i), |ui| {
                    ui.horizontal(|ui| {
                        if let Some(tex) = &cand.texture {
                            let size = tex.size_vec2();
                            let scale = (56.0 / size.x.max(1.0)).min(56.0 / size.y.max(1.0));
                            if ui.add(egui::Image::new((tex.id(), size * scale)).sense(egui::Sense::click())).clicked() {
                                select = Some(i);
                            }
                        }
                        if ui.selectable_label(selected, ff_lineage_label(cand)).clicked() {
                            select = Some(i);
                        }
                    });
                });
            }
        }

        ui.add_space(8.0);
        ui.separator();
        let can_approve = self.gen_ref().selected.is_some();
        let do_approve = ui
            .add_enabled(can_approve, egui::Button::new(format!("{} Approve pose", crate::icons::CHECK)))
            .clicked();
        let do_hand_edit = ui
            .add_enabled(can_approve, egui::Button::new(format!("{} Hand-edit", crate::icons::PENCIL)))
            .on_hover_text("Open the selected result in the drawing editor; return adds the edit to the thread")
            .clicked();
        if self.studio.approved_first_frame.is_some() {
            ui.colored_label(success, format!("{} approved", crate::icons::CHECK));
        }

        if do_hand_edit {
            self.hand_edit();
        }
        if let Some(i) = select {
            self.select_candidate(i);
        }
        if do_generate {
            self.start_gen();
        }
        if do_cancel {
            self.cancel_gen();
        }
        if do_approve {
            self.approve_gen();
        }
    }

    /// Kicks off a from-scratch seed-pose generation conditioned on the anchor.
    fn start_gen(&mut self) {
        let Some(canvas) = self.doc.active_sprite().map(|s| (s.canvas.width, s.canvas.height)) else {
            return;
        };
        let references = self.gen_references();
        if references.is_empty() {
            return;
        }
        let job = FirstFrameJob::Generate {
            reference_images: references,
            canvas,
            prompt: key_color_prompt(&self.gen_ref().prompt, self.bg_key_color),
            num_variants: self.gen_ref().variants,
            seed: None,
        };
        self.spawn_gen(job, None, false);
        // The seed pose renders at the sprite's size; play the reveal there.
        if self.reveal_effect_enabled {
            let now = self.egui_ctx.input(|i| i.time);
            self.studio.frame_gen.reveal.begin(canvas, now);
        }
    }

    /// Kicks off an inpaint refinement of the selected candidate.
    fn start_inpaint(&mut self) {
        let Some(i) = self.gen_ref().selected else {
            return;
        };
        let Some(mask) = self.gen_ref().view.mask.as_ref() else {
            return;
        };
        let mask_png = mask_overlay_png(mask);
        let Some(mask_png) = mask_png else {
            self.gen_mut().status = JobStatus::Failed("could not encode the mask".to_owned());
            return;
        };
        let base = self.gen_ref().candidates[i].png.clone();
        let prompt = {
            let thread = self.gen_ref();
            if thread.view.inpaint_prompt.trim().is_empty() {
                thread.prompt.clone()
            } else {
                thread.view.inpaint_prompt.clone()
            }
        };
        let job = FirstFrameJob::Inpaint {
            base,
            mask: mask_png,
            // The base pose already carries the character; passing the anchor
            // sheet as an edit reference makes gpt-image composite toward the
            // sheet instead of repainting the masked region.
            reference_images: Vec::new(),
            prompt,
            num_variants: 1,
        };
        self.spawn_gen(job, Some(i), true);
    }

    /// The reference images the seed-pose generation conditions on: the approved
    /// anchor.
    fn gen_references(&self) -> Vec<Vec<u8>> {
        self.doc.active_anchor().map(<[u8]>::to_vec).into_iter().collect()
    }

    /// Spawns `job` on the runtime with a fresh epoch and cancel.
    fn spawn_gen(&mut self, job: FirstFrameJob, parent: Option<usize>, append: bool) {
        let cancel = CancellationToken::new();
        let epoch = {
            let thread = self.gen_mut();
            thread.cancel = Some(cancel.clone());
            thread.epoch += 1;
            thread.status = JobStatus::Running("starting".to_owned());
            thread.epoch
        };
        ai::spawn_first_frame(
            self.runtime.handle(),
            self.verb_runtime.clone(),
            self.egui_ctx.clone(),
            self.tx.clone(),
            job,
            cancel,
            epoch,
            parent,
            append,
        );
    }

    /// Cancels an in-flight seed-pose generation.
    fn cancel_gen(&mut self) {
        let thread = self.gen_mut();
        if let Some(cancel) = thread.cancel.take() {
            cancel.cancel();
        }
        thread.epoch += 1;
        thread.status = JobStatus::Idle;
        // Drop the reveal so it doesn't strand on the scatter cloud after a cancel.
        thread.reveal.fail();
    }

    /// Lands generated candidates into the thread. Decodes each PNG (dropping any
    /// that fail), stamps lineage, and selects the first new one.
    pub(crate) fn on_gen_ready(&mut self, images: Vec<Vec<u8>>, parent: Option<usize>, append: bool) {
        let first_new = {
            let thread = self.gen_mut();
            thread.status = JobStatus::Idle;
            thread.cancel = None;
            if !append {
                thread.candidates.clear();
                thread.selected = None;
            }
            let origin = if parent.is_some() { FfOrigin::Inpaint } else { FfOrigin::Fresh };
            let first_new = thread.candidates.len();
            for png in images {
                // Skip a candidate whose bytes don't decode rather than show a broken card.
                if image::load_from_memory(&png).is_err() {
                    continue;
                }
                thread.candidates.push(FirstFrameCandidate {
                    png,
                    texture: None,
                    parent,
                    origin,
                });
            }
            (thread.candidates.len() > first_new).then_some(first_new)
        };
        if let Some(i) = first_new {
            self.select_candidate(i);
        }
    }

    /// Approves the selected candidate as the seed pose and advances to Motion.
    fn approve_gen(&mut self) {
        let Some(i) = self.gen_ref().selected else {
            return;
        };
        let png = self.gen_ref().candidates[i].png.clone();
        self.gen_mut().view.painting = false;
        self.studio.approved_first_frame = Some(png);
        self.studio.stage = StudioStage::Motion;
    }

    /// Opens the selected candidate in the drawing editor as a scratch sprite
    /// and leaves the studio for Draw. [`Self::finish_hand_edit`] brings the
    /// edited pixels back as a new thread candidate.
    fn hand_edit(&mut self) {
        let Some(i) = self.gen_ref().selected else {
            return;
        };
        let png = self.gen_ref().candidates[i].png.clone();
        let Some(buffer) = crate::app::png_to_pixel_buffer(&png) else {
            return;
        };
        let origin = self.doc.active_sprite_ref();
        let edit = self.doc.create_sprite_from_buffer("Hand-edit", buffer);
        self.studio_return = Some(StudioReturn { origin, edit, pick: None });
        self.studio.frame_gen.view.painting = false;
        self.set_workspace(Workspace::Draw);
        self.refresh_canvas(true);
    }

    /// Returns from a hand-edit: captures the edited pixels, restores the original
    /// sprite, drops the scratch sprite, and re-enters the studio. A first-frame
    /// hand edit lands the result as a new thread candidate at the first-frame
    /// stage; a Normalize-stage frame edit ([`StudioReturn::pick`]) writes the
    /// edited buffer back into that clip frame and returns to the Normalize stage,
    /// re-running the cached normalize so the strip and report reflect the edit.
    pub(crate) fn finish_hand_edit(&mut self) {
        let Some(ret) = self.studio_return.take() else {
            return;
        };
        let edited = self.doc.composite_active_frame();
        if let Some(origin) = ret.origin {
            self.doc.select(origin);
        }
        self.doc.delete_sprite(ret.edit);
        self.workspace = Workspace::Create;
        if let Some(pick) = ret.pick {
            self.finish_pick_edit(pick, edited);
        } else {
            if let Some(png) = edited.and_then(|buf| pixel_buffer_to_png(&buf)) {
                let parent = self.gen_ref().selected;
                self.on_gen_ready(vec![png], parent, true);
            }
            self.studio.stage = StudioStage::FirstFrame;
        }
        self.refresh_canvas(true);
    }

    /// Writes the `edited` buffer back into the clip frame the Normalize-stage
    /// "edit pixels" round trip came from, restores the selection, and re-runs
    /// the cached normalize so the strip and report show the edit. A `None` edit
    /// (the scratch sprite produced no frame) is a no-op beyond returning to the
    /// stage. The frame is rebuilt as a fresh [`VideoFrame`] from the edited
    /// buffer, keeping its timestamp, and its cached thumbnail is dropped so the
    /// pick strip rebuilds it.
    fn finish_pick_edit(&mut self, pick: PickReturn, edited: Option<PixelBuffer>) {
        self.studio.stage = StudioStage::Normalize;
        self.anim_selected = Some(pick.clip);
        let Some(buffer) = edited else {
            return;
        };
        let Some(pixels) = pixel_buffer_to_png(&buffer).and_then(|png| crate::app::png_to_pixel_buffer(&png)) else {
            return;
        };
        let Some(cand) = self.anim_candidates.get_mut(pick.clip) else {
            return;
        };
        let Some(slot) = cand.frames.get_mut(pick.frame) else {
            return;
        };
        let timestamp_ms = slot.timestamp_ms;
        *slot = VideoFrame {
            pixels: pixels.as_bytes().to_vec(),
            width: pixels.width(),
            height: pixels.height(),
            timestamp_ms,
        };
        // Drop the cached thumbnail so the pick strip rebuilds it from the edit.
        if let Some(thumb) = cand.thumbs.get_mut(pick.frame) {
            *thumb = None;
        }
        // The frames changed, so the cached normalize is stale; recompute it.
        self.refresh_normalize_cache();
        self.sync_clip_preview();
    }

    /// Inpaint-refines the selected anchor result over its painted mask. The
    /// repainted image lands as a new linked result (see [`Self::land_anchor_refine`]).
    fn start_anchor_inpaint(&mut self) {
        let Some(i) = self.studio.anchor_selected else {
            return;
        };
        let Some(mask) = self.studio.anchor_view.mask.as_ref() else {
            return;
        };
        let Some(mask_png) = mask_overlay_png(mask) else {
            self.rs_status = JobStatus::Failed("could not encode the mask".to_owned());
            return;
        };
        let Some(base) = self.rs_candidates.get(i).map(|c| c.png.clone()) else {
            return;
        };
        let prompt = {
            let view = &self.studio.anchor_view;
            if view.inpaint_prompt.trim().is_empty() {
                self.ck_positive.clone()
            } else {
                view.inpaint_prompt.clone()
            }
        };
        // The painted mask is structured refinement metadata: a single freeform
        // mask lands as RefinementKind::Masked on the variant (a multi-region
        // box surface, when it exists, would land Regional instead). Capture it
        // now, keyed by epoch, so land_anchor_refine can stamp it on the result.
        self.rs_pending_refinement = Some(pixhaus_core::project::RefinementKind::Masked {
            mask_png: pixhaus_core::project::ReferenceImage {
                bytes: mask_png.clone(),
                mime: "image/png".to_owned(),
            },
        });
        let job = FirstFrameJob::Inpaint {
            base,
            mask: mask_png,
            reference_images: Vec::new(),
            prompt,
            num_variants: 1,
        };
        self.rs_refine_epoch += 1;
        let epoch = self.rs_refine_epoch;
        self.rs_status = JobStatus::Running("refining".to_owned());
        ai::spawn_anchor_refine(
            self.runtime.handle(),
            self.verb_runtime.clone(),
            self.egui_ctx.clone(),
            self.tx.clone(),
            job,
            epoch,
            i,
        );
    }

    // ── Motion stage ───────────────────────────────────────────────────────────

    /// Shows the approved seed pose — the frame the clip animates.
    fn studio_motion_surface(&mut self, ui: &mut egui::Ui) {
        let Some(png) = self.studio.approved_first_frame.clone() else {
            centered_hint(ui, "Approve a seed pose in the first-frame stage; the clip animates it.");
            return;
        };
        let ctx = ui.ctx().clone();
        if let Some(tex) = load_png_texture(&ctx, "studio_motion", &png) {
            fit_image(ui, &tex);
        }
    }

    /// Motion prompt, model pick, fps, frame count, seed, and Generate.
    fn studio_motion_inspector(&mut self, ui: &mut egui::Ui) {
        if self.doc.active_anchor().is_none() {
            ui.colored_label(ui.visuals().warn_fg_color, "Approve an anchor first.");
            return;
        }
        ui.label(egui::RichText::new("Motion").strong());
        ui.add(
            egui::TextEdit::multiline(&mut self.anim_motion)
                .hint_text("walk cycle, side view")
                .desired_rows(2)
                .desired_width(f32::INFINITY),
        );
        ui.horizontal_wrapped(|ui| {
            for preset in ["walk cycle, side view", "idle breathing", "run cycle, side view", "attack swing"] {
                if ui.small_button(preset).clicked() {
                    preset.clone_into(&mut self.anim_motion);
                }
            }
        });

        ui.add_space(6.0);
        // The animation kind is what this clip *is*, so its dial lives here at
        // the Motion stage where it scaffolds the motion prompt.
        ui.label(egui::RichText::new("Animation kind").strong());
        let prev_kind = self.studio.kind;
        let mut kind_changed = false;
        ui.horizontal_wrapped(|ui| {
            for kind in AnimKind::ALL {
                if ui.selectable_value(&mut self.studio.kind, kind, kind.label()).changed() {
                    kind_changed = true;
                }
            }
        });
        // Keep the motion prompt in step with the dial while it is still the
        // untouched scaffold; a preset or hand-edited motion is never clobbered.
        if kind_changed && self.anim_motion == StudioState::motion_prompt_for(prev_kind, self.studio.facing) {
            self.anim_motion = StudioState::motion_prompt_for(self.studio.kind, self.studio.facing);
        }
        if ui
            .small_button("Apply to motion prompt")
            .on_hover_text("Rebuild the motion prompt from this animation kind and direction")
            .clicked()
        {
            let motion = self.studio.motion_scaffold();
            self.anim_motion = motion;
        }

        ui.add_space(6.0);
        // Generation mode: an i2v clip or a static grid sheet. The choice drives
        // which inputs show below and which producer the Generate button calls.
        ui.horizontal(|ui| {
            ui.label("Mode");
            ui.selectable_value(&mut self.studio.gen_mode, GenMode::Animated, GenMode::Animated.label())
                .on_hover_text("Generate a clip with an image-to-video model");
            ui.selectable_value(&mut self.studio.gen_mode, GenMode::Static, GenMode::Static.label())
                .on_hover_text("Generate one magenta grid sheet and slice the cells — no video model needed");
        });

        match self.studio.gen_mode {
            GenMode::Animated => {
                ui.horizontal(|ui| {
                    ui.label("Model");
                    ui.selectable_value(&mut self.studio.i2v_model, I2vModel::Seedance, I2vModel::Seedance.label());
                    ui.selectable_value(&mut self.studio.i2v_model, I2vModel::Wan, I2vModel::Wan.label());
                });
            }
            GenMode::Static => {
                ui.horizontal(|ui| {
                    ui.label("Grid");
                    ui.add(egui::DragValue::new(&mut self.studio.grid_rows).range(1..=8).prefix("rows "));
                    ui.add(egui::DragValue::new(&mut self.studio.grid_cols).range(1..=8).prefix("cols "));
                });
                ui.colored_label(
                    ui.visuals().weak_text_color(),
                    "Sliced from one sheet; consistency depends on the model. Watch the normalize report.",
                );
            }
        }
        ui.add(egui::Slider::new(&mut self.anim_target_frames, 2..=16).text("loop frames"));
        ui.add(egui::Slider::new(&mut self.anim_fps, 4..=24).text("fps"));
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.anim_seed_fixed, "Fixed seed")
                .on_hover_text("Pin the RNG seed for a reproducible run; off uses a random seed each run");
            ui.add_enabled(self.anim_seed_fixed, egui::DragValue::new(&mut self.anim_seed));
        });

        let busy = matches!(self.anim_status, JobStatus::Running(_));
        let generate_label = match self.studio.gen_mode {
            GenMode::Animated => format!("{} Generate clip", crate::icons::FILM),
            GenMode::Static => format!("{} Generate sheet", crate::icons::FILM),
        };
        if ui.add_enabled(self.backend_ready && !busy, egui::Button::new(generate_label)).clicked() {
            self.studio.landed = false;
            // A from-scratch run has no lineage parent.
            self.clear_clip_lineage();
            match self.studio.gen_mode.producer() {
                GenProducer::Clip => self.start_clip(),
                GenProducer::StaticSheet => self.start_static_sheet(),
            }
            self.studio.stage = StudioStage::Clip;
        }
        if !self.backend_ready {
            ui.colored_label(ui.visuals().warn_fg_color, "No generation backend configured.");
        }

        ui.add_space(6.0);
        ui.separator();
        // A user-supplied video bypasses i2v: it decodes into the same clip
        // pipeline (Clip -> Pick -> Normalize -> Land), so no backend is needed.
        if ui
            .add_enabled(!busy, egui::Button::new(format!("{} Use a video file", crate::icons::UPLOAD)))
            .on_hover_text("Import an mp4, mov, gif, or apng and loop it like a generated clip — no generation backend needed")
            .clicked()
        {
            self.studio.landed = false;
            self.clear_clip_lineage();
            self.import_video_clip();
            self.studio.stage = StudioStage::Clip;
        }

        studio_status_line(ui, &self.anim_status);
        // Cancel applies only to an i2v run (which holds a cancel token via its
        // job id). A video import is a quick local decode with nothing to cancel.
        if busy && self.anim_job_id.is_some() && ui.button("Cancel").clicked() {
            self.cancel_clip();
        }

        if !self.anim_recent_motions.is_empty() {
            let recent = self.anim_recent_motions.clone();
            let mut pick: Option<String> = None;
            egui::CollapsingHeader::new(format!("{} Recent motions", crate::icons::UNDO))
                .id_salt("studio_motion_history")
                .show(ui, |ui| {
                    for m in &recent {
                        if ui
                            .add(egui::Label::new(egui::RichText::new(crate::app::truncate_motion(m, 48)).small()).sense(egui::Sense::click()))
                            .on_hover_text(m)
                            .clicked()
                        {
                            pick = Some(m.clone());
                        }
                    }
                });
            if let Some(m) = pick {
                self.anim_motion = m;
            }
        }
    }

    // ── Clip & loop stage ──────────────────────────────────────────────────────

    /// The raw clip plays large in a player — the frame, the seekbar (which also
    /// carries the loop handles), and a transport row. In compare mode two clips
    /// play side by side over one transport.
    fn studio_clip_surface(&mut self, ui: &mut egui::Ui) {
        if self.anim_candidates.is_empty() {
            centered_hint(ui, "Generate a clip in the Motion stage. It plays here so you can watch it and mark the loop.");
            return;
        }
        let Some(i) = self.anim_selected else {
            centered_hint(ui, "Select a clip in the inspector gallery.");
            return;
        };
        self.studio_clip_player(ui, i);
    }

    /// The raw-clip player: a fit-to-area frame (click to play/pause, or eyedrop
    /// the key when armed), the seekbar with loop handles, and a transport row
    /// with play/pause, the time readout, and the loop toggle.
    fn studio_clip_player(&mut self, ui: &mut egui::Ui, i: usize) {
        let n = self.anim_candidates[i].frames.len();
        if n == 0 {
            return;
        }
        let transport_h = 28.0;
        let seekbar_h = 40.0;
        let spacing = 8.0;
        let total = ui.available_size();
        let frame_h = (total.y - transport_h - seekbar_h - spacing).max(80.0);
        let scrub = self.anim_scrub.min(n - 1);

        // Frame area: the raw clip (or two clips side by side in compare mode).
        let compare = self
            .studio
            .compare
            .then_some(self.studio.compare_other)
            .flatten()
            .filter(|&o| o != i && o < self.anim_candidates.len());
        if let Some(other) = compare {
            ui.allocate_ui(egui::vec2(total.x, frame_h), |ui| {
                ui.columns(2, |cols| {
                    self.studio_clip_frame(&mut cols[0], i, scrub);
                    self.studio_clip_frame(&mut cols[1], other, scrub);
                });
            });
        } else {
            ui.allocate_ui(egui::vec2(total.x, frame_h), |ui| {
                self.studio_clip_frame_interactive(ui, i, scrub);
            });
        }

        // The seekbar doubles as the loop-marker track.
        self.studio_scrubber(ui, i);
        self.studio_clip_transport(ui, i);
    }

    /// The transport row: play/pause, the `mm:ss / mm:ss` readout, the loop
    /// toggle, and an eyedrop hint when the key picker is armed.
    fn studio_clip_transport(&mut self, ui: &mut egui::Ui, i: usize) {
        let n = self.anim_candidates[i].frames.len();
        let fps = self.anim_candidates[i].fps;
        ui.horizontal(|ui| {
            let label = if self.anim_clip_playing { crate::icons::PAUSE } else { crate::icons::PLAY };
            if ui.button(label).on_hover_text("Play / pause").clicked() {
                self.toggle_clip_play();
            }
            ui.label(format_clip_time(self.anim_scrub.min(n.saturating_sub(1)), fps, n));
            ui.checkbox(&mut self.studio.loop_playback, "Loop");
            if self.studio.picking_key {
                ui.label(egui::RichText::new("eyedrop armed — click the clip").small().weak());
            }
        });
    }

    /// Draws clip `i`'s frame `idx` fit to the available area (raw or keyed,
    /// per the preview toggle), building the texture lazily.
    fn studio_clip_frame(&mut self, ui: &mut egui::Ui, i: usize, idx: usize) {
        let ctx = ui.ctx().clone();
        if let Some((id, size)) = self.clip_frame_texture(&ctx, i, idx) {
            fit_image_sized(ui, id, size);
        }
    }

    /// Draws the frame and senses clicks: when the eyedropper is armed a click
    /// samples the key colour from the raw frame; otherwise it toggles playback.
    fn studio_clip_frame_interactive(&mut self, ui: &mut egui::Ui, i: usize, idx: usize) {
        let ctx = ui.ctx().clone();
        let Some((id, size)) = self.clip_frame_texture(&ctx, i, idx) else {
            return;
        };
        let avail = ui.available_size();
        let scale = (avail.x / size.x.max(1.0)).min(avail.y / size.y.max(1.0)).max(0.01);
        let draw = size * scale;
        let (outer, _) = ui.allocate_exact_size(avail, egui::Sense::hover());
        let image_rect = egui::Rect::from_center_size(outer.center(), draw);
        let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
        ui.painter().image(id, image_rect, uv, egui::Color32::WHITE);
        let resp = ui.interact(image_rect, ui.id().with(("studio_clip_frame", i)), egui::Sense::click());
        if resp.clicked() {
            if self.studio.picking_key {
                if let Some(pos) = resp.interact_pointer_pos() {
                    let u = (pos.x - image_rect.left()) / image_rect.width();
                    let v = (pos.y - image_rect.top()) / image_rect.height();
                    if let Some(rgba) = self.anim_candidates[i].frames.get(idx).and_then(|f| frame_texel_rgba(f, u, v)) {
                        self.set_studio_key(rgba);
                        self.studio.picking_key = false;
                    }
                }
            } else {
                self.toggle_clip_play();
            }
        }
    }

    /// Returns the texture id and size for clip `i`'s frame `idx`, building it
    /// lazily — keyed through the current chroma key when the keyed-preview
    /// toggle is on (rebuilding the cache when the key changed), else raw.
    fn clip_frame_texture(&mut self, ctx: &egui::Context, i: usize, idx: usize) -> Option<(egui::TextureId, egui::Vec2)> {
        let n = self.anim_candidates[i].frames.len();
        if n == 0 {
            return None;
        }
        let idx = idx.min(n - 1);
        if self.studio.keyed_preview {
            let sig = (self.bg_key_color, self.bg_tolerance);
            if self.anim_candidates[i].keyed_sig != Some(sig) {
                self.anim_candidates[i].keyed_sig = Some(sig);
                self.anim_candidates[i].keyed_thumbs.iter_mut().for_each(|slot| *slot = None);
            }
            if self.anim_candidates[i].keyed_thumbs.get(idx).is_some_and(Option::is_none) {
                let key = ChromaKey::new(self.bg_key_color, self.bg_tolerance);
                let tex = self.anim_candidates[i]
                    .frames
                    .get(idx)
                    .and_then(crate::app::video_frame_to_pixel_buffer)
                    .map(|buf| chroma_key(&buf, key))
                    .and_then(|keyed| pixel_buffer_to_texture(ctx, "studio_keyed", &keyed));
                if let (Some(tex), Some(slot)) = (tex, self.anim_candidates[i].keyed_thumbs.get_mut(idx)) {
                    *slot = Some(tex);
                }
            }
            return self.anim_candidates[i]
                .keyed_thumbs
                .get(idx)
                .and_then(|t| t.as_ref())
                .map(|t| (t.id(), t.size_vec2()));
        }
        if self.anim_candidates[i].thumbs.get(idx).is_some_and(Option::is_none) {
            if let Some(frame) = self.anim_candidates[i].frames.get(idx) {
                let tex = crate::app::video_frame_to_texture(ctx, frame);
                if let Some(slot) = self.anim_candidates[i].thumbs.get_mut(idx) {
                    *slot = Some(tex);
                }
            }
        }
        self.anim_candidates[i]
            .thumbs
            .get(idx)
            .and_then(|t| t.as_ref())
            .map(|t| (t.id(), t.size_vec2()))
    }

    /// Sets the chroma key colour and arms "remove background on Land", so the
    /// key chosen against the raw clip carries into the landed loop.
    fn set_studio_key(&mut self, color: Rgba) {
        self.bg_key_color = color;
        self.studio.remove_on_land = true;
    }

    /// A draggable loop scrubber: a track with the marked `[start, end)` window
    /// highlighted, draggable in/out handles, and a playhead. Maps pointer x to a
    /// frame index; a plain click moves the playhead.
    fn studio_scrubber(&mut self, ui: &mut egui::Ui, i: usize) {
        let n = self.anim_candidates[i].frames.len();
        if n < 2 {
            return;
        }
        let width = ui.available_width();
        let (rect, resp) = ui.allocate_exact_size(egui::vec2(width, 40.0), egui::Sense::click_and_drag());
        let n_f = n as f32;
        let frame_to_x = |idx: usize| rect.left() + (idx as f32 + 0.5) / n_f * rect.width();
        let x_to_frame = |x: f32| (((x - rect.left()) / rect.width() * n_f).floor() as i64).clamp(0, n as i64 - 1) as usize;

        let markers = self.anim_candidates[i].markers;
        let start = markers.start.min(n - 1);
        let end = markers.end.clamp(start + 1, n);
        let scrub = self.anim_scrub.min(n - 1);

        // Resolve a drag: pick the nearest handle on press, then move it.
        if resp.drag_started() {
            if let Some(pos) = resp.interact_pointer_pos() {
                let f = x_to_frame(pos.x) as i64;
                let ds = (f - start as i64).abs();
                let de = (f - end as i64).abs();
                let dp = (f - scrub as i64).abs();
                self.studio.drag_handle = Some(if ds <= de && ds <= dp {
                    ScrubHandle::Start
                } else if de <= dp {
                    ScrubHandle::End
                } else {
                    ScrubHandle::Playhead
                });
            }
        }
        if resp.dragged() {
            if let (Some(handle), Some(pos)) = (self.studio.drag_handle, resp.interact_pointer_pos()) {
                let f = x_to_frame(pos.x);
                match handle {
                    ScrubHandle::Start => {
                        let s = f.min(end.saturating_sub(1));
                        self.anim_candidates[i].markers.start = s;
                        self.anim_clip_playing = false;
                        self.set_scrub(s);
                    }
                    ScrubHandle::End => {
                        let e = (f + 1).max(start + 1).min(n);
                        self.anim_candidates[i].markers.end = e;
                        self.anim_clip_playing = false;
                        self.set_scrub(e - 1);
                    }
                    ScrubHandle::Playhead => {
                        self.anim_clip_playing = false;
                        self.set_scrub(f);
                    }
                }
            }
        }
        if resp.drag_stopped() {
            self.studio.drag_handle = None;
        }
        if resp.clicked() {
            if let Some(pos) = resp.interact_pointer_pos() {
                self.anim_clip_playing = false;
                self.set_scrub(x_to_frame(pos.x));
            }
        }

        // Re-read markers after a possible drag, then paint the track.
        let markers = self.anim_candidates[i].markers;
        let start = markers.start.min(n - 1);
        let end = markers.end.clamp(start + 1, n);
        let scrub = self.anim_scrub.min(n - 1);
        let palette = crate::theme::Palette::for_theme(ui.ctx().theme());
        let painter = ui.painter_at(rect);
        let track = egui::Rect::from_min_max(egui::pos2(rect.left(), rect.center().y - 4.0), egui::pos2(rect.right(), rect.center().y + 4.0));
        painter.rect_filled(track, 3.0, ui.visuals().extreme_bg_color);
        // Playback progress: a muted fill from the start of the track to the playhead.
        let progress = egui::Rect::from_min_max(track.left_top(), egui::pos2(frame_to_x(scrub), track.bottom()));
        painter.rect_filled(progress, 3.0, ui.visuals().weak_text_color());
        let win = egui::Rect::from_min_max(
            egui::pos2(frame_to_x(start), track.top()),
            egui::pos2(frame_to_x(end.saturating_sub(1)), track.bottom()),
        );
        let accent = palette.accent;
        let win_fill = egui::Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 130);
        painter.rect_filled(win, 3.0, win_fill);
        // In/out handles.
        for (x, color) in [(frame_to_x(start), palette.accent), (frame_to_x(end.saturating_sub(1)), palette.accent)] {
            painter.line_segment([egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())], egui::Stroke::new(2.0, color));
        }
        // Playhead.
        let px = frame_to_x(scrub);
        painter.line_segment(
            [egui::pos2(px, rect.top()), egui::pos2(px, rect.bottom())],
            egui::Stroke::new(2.0, palette.success),
        );
    }

    /// The clip inspector: the gallery, play-mode + transport, the loop markers,
    /// and compare/branch controls.
    fn studio_clip_inspector(&mut self, ui: &mut egui::Ui) {
        self.studio_clip_gallery(ui);
        let Some(i) = self.anim_selected else {
            return;
        };
        let n = self.anim_candidates[i].frames.len();
        if n == 0 {
            return;
        }
        ui.separator();
        {
            let c = &self.anim_candidates[i];
            let seed = c.seed.map_or_else(|| "random".to_owned(), |s| s.to_string());
            ui.label(
                egui::RichText::new(format!(
                    "\"{}\" — {} frames · {} fps · seed {seed}",
                    crate::app::truncate_motion(&c.motion, 32),
                    n,
                    c.fps
                ))
                .small()
                .weak(),
            );
            // Source clip provenance: the raw bytes kept for a future export.
            ui.label(egui::RichText::new(format!("source: {} · {} KB", c.mime, c.clip.len() / 1024)).small().weak());
        }
        ui.horizontal(|ui| {
            ui.label("Play");
            ui.selectable_value(&mut self.anim_play_mode, AnimPlayMode::Clip, "Clip")
                .on_hover_text("Play the whole raw clip");
            ui.selectable_value(&mut self.anim_play_mode, AnimPlayMode::Loop, "Loop")
                .on_hover_text("Play only the marked loop");
            ui.selectable_value(&mut self.anim_play_mode, AnimPlayMode::Picks, "Picks")
                .on_hover_text("Play only the picked frames");
        });

        self.studio_key_controls(ui, i);

        ui.add_space(6.0);
        ui.label(
            egui::RichText::new("Loop — drag the handles on the scrubber, or set exactly here")
                .small()
                .weak(),
        );
        let last = (n - 1) as u32;
        let mut start = self.anim_candidates[i].markers.start as u32;
        if ui.add(egui::Slider::new(&mut start, 0..=last).text("start")).changed() {
            self.anim_clip_playing = false;
            let end = self.anim_candidates[i].markers.end;
            let s = (start as usize).min(end.saturating_sub(1));
            self.anim_candidates[i].markers.start = s;
            self.set_scrub(s);
        }
        let mut end = self.anim_candidates[i].markers.end as u32;
        if ui.add(egui::Slider::new(&mut end, 1..=(n as u32)).text("end (excluded)")).changed() {
            self.anim_clip_playing = false;
            let start = self.anim_candidates[i].markers.start;
            let e = (end as usize).max(start + 1).min(n);
            self.anim_candidates[i].markers.end = e;
            self.set_scrub(e - 1);
        }

        ui.add_space(8.0);
        ui.separator();
        ui.horizontal(|ui| {
            if ui.button(format!("{} Next: pick frames", crate::icons::RIGHT)).clicked() {
                self.studio.stage = StudioStage::Pick;
            }
            if ui
                .button(format!("{} Re-roll", crate::icons::DICE))
                .on_hover_text("Generate again from this motion with a new random seed")
                .clicked()
            {
                self.reroll_clip(i);
            }
        });
        if self.anim_candidates.len() > 1 {
            ui.checkbox(&mut self.studio.compare, "Compare two clips side by side")
                .on_hover_text("Play another clip beside this one to judge the motion");
            if self.studio.compare {
                self.studio_compare_picker(ui, i);
            }
        }
    }

    /// The background-key controls: the swatch, tolerance, Detect, eyedrop, a
    /// keyed-preview toggle, and whether Land bakes the key in. Picked against
    /// the raw clip so you judge the strip before committing; reuses
    /// `bg_key_color` / `bg_tolerance`, the same key the timeline op uses.
    fn studio_key_controls(&mut self, ui: &mut egui::Ui, i: usize) {
        ui.add_space(6.0);
        egui::CollapsingHeader::new(format!("{} Background key", crate::icons::PALETTE))
            .id_salt("studio_key")
            .default_open(true)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Key");
                    let mut col = crate::editor::to_color32(self.bg_key_color);
                    if ui.color_edit_button_srgba(&mut col).changed() {
                        self.set_studio_key(crate::editor::from_color32(col));
                    }
                    if ui
                        .button("Detect")
                        .on_hover_text("Sample the clip frame's border for the backdrop colour")
                        .clicked()
                    {
                        self.detect_studio_key(i);
                    }
                    let eyedrop = egui::Button::selectable(self.studio.picking_key, format!("{} Eyedrop", crate::icons::PICKER));
                    if ui.add(eyedrop).on_hover_text("Click the clip to sample its backdrop colour").clicked() {
                        self.studio.picking_key = !self.studio.picking_key;
                    }
                });
                ui.add(egui::Slider::new(&mut self.bg_tolerance, 0..=128).text("tolerance"));
                ui.checkbox(&mut self.studio.keyed_preview, "Preview keyed")
                    .on_hover_text("Show the clip with the backdrop removed");
                ui.checkbox(&mut self.studio.remove_on_land, "Remove background on Land")
                    .on_hover_text("Bake this key into the loop when it lands; off leaves removal to the timeline op");
                // The component policy only bites when Land keys the backdrop —
                // without keying there are no detached specks to isolate.
                if self.studio.remove_on_land {
                    ui.checkbox(&mut self.studio.land_all_parts, "Keep all parts").on_hover_text(
                        "On: keep every detached part above the min area (multi-part FX). Off: keep only the largest body, dropping stray specks",
                    );
                }
            });
    }

    /// Auto-detects the key colour from the selected clip frame's border.
    fn detect_studio_key(&mut self, i: usize) {
        let idx = self.anim_scrub.min(self.anim_candidates[i].frames.len().saturating_sub(1));
        if let Some(color) = self.anim_candidates[i]
            .frames
            .get(idx)
            .and_then(crate::app::video_frame_to_pixel_buffer)
            .and_then(|buf| detect_key_color(&buf))
        {
            self.set_studio_key(color);
        }
    }

    /// The clip-card gallery, newest first; click to select, trash to discard.
    fn studio_clip_gallery(&mut self, ui: &mut egui::Ui) {
        if self.anim_candidates.is_empty() {
            ui.label(egui::RichText::new("Generated clips appear here.").small().weak());
            return;
        }
        let ctx = ui.ctx().clone();
        let mut select: Option<usize> = None;
        let mut remove: Option<usize> = None;
        for i in (0..self.anim_candidates.len()).rev() {
            if self.anim_candidates[i].card_texture.is_none() {
                if let Some(frame) = self.anim_candidates[i].frames.first() {
                    let tex = crate::app::video_frame_to_texture(&ctx, frame);
                    self.anim_candidates[i].card_texture = Some(tex);
                }
            }
            let selected = self.anim_selected == Some(i);
            let cand = &self.anim_candidates[i];
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.horizontal(|ui| {
                    if let Some(tex) = &cand.card_texture {
                        let size = tex.size_vec2();
                        let scale = (56.0 / size.x.max(1.0)).min(1.0);
                        if ui.add(egui::Button::image((tex.id(), size * scale))).clicked() {
                            select = Some(i);
                        }
                    }
                    ui.vertical(|ui| {
                        if ui
                            .selectable_label(selected, egui::RichText::new(crate::app::truncate_motion(&cand.motion, 28)).strong())
                            .clicked()
                        {
                            select = Some(i);
                        }
                        let lineage = match cand.parent {
                            Some(p) => format!("{} frames · {} fps · branch of #{}", cand.frames.len(), cand.fps, p + 1),
                            None => format!("{} frames · {} fps", cand.frames.len(), cand.fps),
                        };
                        ui.label(egui::RichText::new(lineage).small().weak());
                        if ui.small_button(crate::icons::TRASH).on_hover_text("Discard this clip").clicked() {
                            remove = Some(i);
                        }
                    });
                });
            });
        }
        if let Some(i) = select {
            self.select_clip(i);
        }
        if let Some(i) = remove {
            self.remove_clip(i);
        }
    }

    /// The picker for the second clip in a side-by-side comparison.
    fn studio_compare_picker(&mut self, ui: &mut egui::Ui, current: usize) {
        let current_label = self
            .studio
            .compare_other
            .and_then(|o| self.anim_candidates.get(o))
            .map_or_else(|| "pick a clip".to_owned(), |c| crate::app::truncate_motion(&c.motion, 20));
        egui::ComboBox::from_id_salt("studio_compare").selected_text(current_label).show_ui(ui, |ui| {
            for j in 0..self.anim_candidates.len() {
                if j == current {
                    continue;
                }
                let label = crate::app::truncate_motion(&self.anim_candidates[j].motion, 24);
                ui.selectable_value(&mut self.studio.compare_other, Some(j), label);
            }
        });
    }

    // ── Frame-pick stage ───────────────────────────────────────────────────────

    /// The picked-frame strip plus the current scrub frame.
    fn studio_pick_surface(&mut self, ui: &mut egui::Ui) {
        let Some(i) = self.anim_selected else {
            centered_hint(ui, "Generate and select a clip first.");
            return;
        };
        let n = self.anim_candidates[i].frames.len();
        if n == 0 {
            return;
        }
        let strip_h = 96.0;
        let total = ui.available_size();
        let view_h = (total.y - strip_h - 12.0).max(80.0);
        ui.allocate_ui(egui::vec2(total.x, view_h), |ui| {
            let scrub = self.anim_scrub;
            self.studio_clip_frame(ui, i, scrub);
        });
        ui.separator();
        self.pick_strip(ui);
    }

    /// The pick inspector: count, add/remove current, the seam score, and play.
    fn studio_pick_inspector(&mut self, ui: &mut egui::Ui) {
        let Some(i) = self.anim_selected else {
            ui.label(egui::RichText::new("No clip selected.").weak());
            return;
        };
        let n = self.anim_candidates[i].frames.len();
        if n == 0 {
            return;
        }
        ui.label(egui::RichText::new("Frames — the picks that land on the timeline").small().weak());
        let mut count = self.anim_target_frames;
        if ui.add(egui::Slider::new(&mut count, 2..=16).text("count")).changed() {
            self.anim_target_frames = count;
            let markers = self.anim_candidates[i].markers;
            self.anim_candidates[i].picks = crate::anim::pick_loop_frames(&self.anim_candidates[i].frames, markers, count as usize);
        }
        let picks_len = self.anim_candidates[i].picks.len();
        if picks_len < self.anim_target_frames as usize {
            ui.colored_label(
                ui.visuals().warn_fg_color,
                "The clip can't supply that many distinct frames — widen the loop or lower the count.",
            );
        }

        let cur = self.anim_scrub;
        let included = self.anim_candidates[i].picks.contains(&cur);
        if ui.button(if included { "Remove current frame" } else { "Add current frame" }).clicked() {
            self.toggle_pick(cur);
        }

        // Loop-seam quality: how cleanly the last pick meets the first.
        if let (Some(&first), Some(&last)) = (self.anim_candidates[i].picks.first(), self.anim_candidates[i].picks.last()) {
            let score = crate::anim::seam_similarity(&self.anim_candidates[i].frames, last, first);
            let palette = crate::theme::Palette::for_theme(ui.ctx().theme());
            let color = if score > 0.9 {
                palette.success
            } else if score > 0.75 {
                palette.warning
            } else {
                palette.error
            };
            ui.colored_label(color, format!("seam quality {:.0}%", score * 100.0));
        }

        ui.horizontal(|ui| {
            let label = if self.anim_clip_playing { crate::icons::PAUSE } else { crate::icons::PLAY };
            if ui.button(format!("{label} Preview picks")).clicked() {
                self.anim_play_mode = AnimPlayMode::Picks;
                self.toggle_clip_play();
            }
        });

        ui.add_space(8.0);
        ui.separator();
        if ui.button(format!("{} Next: normalize", crate::icons::RIGHT)).clicked() {
            self.studio.stage = StudioStage::Normalize;
        }
    }

    // ── Normalize stage ──────────────────────────────────────────────────────

    /// The normalized frame strip, lazily computed and cached. Shows the locked
    /// frames the way they will land — same canvas, same baseline — so the
    /// review reflects exactly what Land commits.
    fn studio_normalize_surface(&mut self, ui: &mut egui::Ui) {
        if self.anim_selected.is_none() {
            centered_hint(ui, "Pick frames from a clip first.");
            return;
        }
        self.refresh_normalize_cache();
        let Some(result) = self.studio.normalize.as_ref() else {
            centered_hint(ui, "These picks could not be normalized — widen the loop or re-pick.");
            return;
        };
        if result.frames.is_empty() {
            centered_hint(ui, "No normalized frames to review.");
            return;
        }
        let ctx = ui.ctx().clone();
        // Build the strip textures once per cache.
        if self.studio.normalize_textures.len() != result.frames.len() {
            self.studio.normalize_textures = vec![None; result.frames.len()];
        }
        // The strip is in pick order: strip slot `idx` is the normalized form of
        // the selected card's `picks[idx]`. Snapshot the picks so a per-frame drop
        // can name the source frame after the `result` borrow ends.
        let picks: Vec<usize> = self.anim_card().map(|c| c.picks.clone()).unwrap_or_default();
        let target_baseline = result.frames.first().map_or(0, PixelBuffer::height).saturating_sub(1);
        let palette = crate::theme::Palette::for_theme(ctx.theme());
        ui.label(egui::RichText::new("Normalized loop — exactly what lands").small().weak());
        ui.label(egui::RichText::new("Drop a junk frame, or send one to Draw for manual surgery.").small().weak());
        ui.add_space(4.0);
        // Deferred per-frame action, applied after the `result` borrow releases:
        // dropping a pick or handing a frame to the Draw editor both mutate `self`.
        let mut drop_pick: Option<usize> = None;
        let mut edit_pick: Option<usize> = None;
        egui::ScrollArea::horizontal().show(ui, |ui| {
            ui.horizontal(|ui| {
                for idx in 0..result.frames.len() {
                    if self.studio.normalize_textures[idx].is_none() {
                        self.studio.normalize_textures[idx] = pixel_buffer_to_texture(&ctx, &format!("normalize_{idx}"), &result.frames[idx]);
                    }
                    let Some(tex) = self.studio.normalize_textures[idx].as_ref() else {
                        continue;
                    };
                    let metric = result.metrics.get(idx);
                    let drift = metric.filter(|m| !m.empty).map_or(0, |m| m.foot_baseline_y.abs_diff(target_baseline));
                    let edge_touch = result.report.edge_touch_frames.contains(&idx);
                    ui.push_id(idx, |ui| {
                        ui.vertical(|ui| {
                            let size = tex.size_vec2();
                            let scale = (96.0 / size.y.max(1.0)).min(1.0);
                            let mut image = egui::Image::new((tex.id(), size * scale));
                            // Tint an edge-touch frame so the why reads at a glance,
                            // not just as a count in the report below.
                            if edge_touch {
                                image = image.tint(egui::Color32::from_rgb(255, 170, 170));
                            }
                            ui.add(image);
                            let drift_color = if drift == 0 { palette.success } else { palette.warning };
                            ui.colored_label(drift_color, egui::RichText::new(format!("Δ {drift}px")).small());
                            // A clipped frame — its landed bbox reaches a canvas edge —
                            // gets an advisory badge under the drift label. STALE is the
                            // codebase's name for the phosphor warning glyph.
                            if edge_touch {
                                ui.colored_label(palette.error, egui::RichText::new(format!("{} clipped", crate::icons::STALE)).small());
                            }
                            ui.horizontal(|ui| {
                                if ui
                                    .small_button(crate::icons::TRASH)
                                    .on_hover_text("Drop this frame from the pick set")
                                    .clicked()
                                {
                                    drop_pick = picks.get(idx).copied();
                                }
                                if ui
                                    .small_button(crate::icons::PENCIL)
                                    .on_hover_text("Edit this frame's pixels in Draw")
                                    .clicked()
                                {
                                    edit_pick = picks.get(idx).copied();
                                }
                            });
                        });
                    });
                }
            });
        });
        if let Some(p) = drop_pick {
            self.toggle_pick(p);
        } else if let Some(p) = edit_pick {
            self.edit_pick_in_draw(p);
        }
    }

    /// The editable normalize knobs: chroma key colour + tolerance, the keep-
    /// background toggle and its component mode, the alpha background threshold,
    /// the bottom margin, and the reference height. Each change invalidates the
    /// normalize cache and re-runs the pass in place, so the strip and the report
    /// reflect the new cut. The pass is CPU-only over a handful of small pick
    /// buffers, so the re-run is inline — no lock, no await.
    fn normalize_knobs(&mut self, ui: &mut egui::Ui) {
        let mut changed = false;

        // Canvas is read-only here — it is the active sprite's size, not a knob.
        if let Some((cw, ch)) = self.doc.active_sprite().map(|s| (s.canvas.width, s.canvas.height)) {
            ui.label(egui::RichText::new(format!("Canvas {cw}×{ch}px")).small().weak());
        }

        // Chroma key colour + tolerance. These also drive the background-removal
        // panel, so the values stay in step across the studio.
        ui.horizontal(|ui| {
            ui.label("Key colour");
            let mut rgb = [self.bg_key_color.r, self.bg_key_color.g, self.bg_key_color.b];
            if ui.color_edit_button_srgb(&mut rgb).changed() {
                self.bg_key_color = Rgba::opaque(rgb[0], rgb[1], rgb[2]);
                changed = true;
            }
        });
        ui.horizontal(|ui| {
            ui.label("Key tolerance");
            changed |= ui.add(egui::Slider::new(&mut self.bg_tolerance, 0..=128)).changed();
        });

        // Keep-background toggle: when set, the backdrop is keyed out during
        // normalize so the loop lands already stripped. Its component mode picks
        // how detached parts survive.
        changed |= ui.checkbox(&mut self.studio.remove_on_land, "Remove background").changed();
        if self.studio.remove_on_land {
            ui.indent("norm_key", |ui| {
                changed |= ui.checkbox(&mut self.studio.land_all_parts, "Keep all parts (vs largest only)").changed();
                if self.studio.land_all_parts {
                    ui.horizontal(|ui| {
                        ui.label("Min area");
                        changed |= ui.add(egui::DragValue::new(&mut self.studio.land_min_area).range(1..=128)).changed();
                    });
                }
            });
        }

        // The measurement knobs that have no other home.
        let knobs = &mut self.studio.norm_knobs;
        ui.horizontal(|ui| {
            ui.label("Alpha threshold");
            changed |= ui.add(egui::Slider::new(&mut knobs.alpha_threshold, 0..=64)).changed();
        });
        ui.horizontal(|ui| {
            ui.label("Bottom margin");
            changed |= ui.add(egui::DragValue::new(&mut knobs.bottom_margin).range(0..=256)).changed();
        });

        // Reference height: an explicit target the subjects scale toward, or the
        // tallest picked subject when auto.
        let mut auto_ref = knobs.reference_height.is_none();
        ui.horizontal(|ui| {
            if ui.checkbox(&mut auto_ref, "Auto reference height").changed() {
                knobs.reference_height = if auto_ref { None } else { Some(1) };
                changed = true;
            }
        });
        if let Some(h) = knobs.reference_height.as_mut() {
            ui.horizontal(|ui| {
                ui.label("Reference height");
                changed |= ui.add(egui::DragValue::new(h).range(1..=4096)).changed();
            });
        }

        if changed {
            self.refresh_normalize_cache();
        }
    }

    /// The normalize inspector: the editable [`NormalizeKnobs`] up top, then the
    /// [`NormalizeReport`] rendered as drift, scale-match, the seam verdict (same
    /// colour thresholds as the pick stage), and the warnings list, with a Land
    /// affordance.
    fn studio_normalize_inspector(&mut self, ui: &mut egui::Ui) {
        if self.anim_selected.is_none() {
            ui.label(egui::RichText::new("No clip selected.").weak());
            return;
        }
        self.refresh_normalize_cache();
        let palette = crate::theme::Palette::for_theme(ui.ctx().theme());
        ui.label(egui::RichText::new("Normalization review").strong());
        ui.label(
            egui::RichText::new("Tune the pass, then read the result below — the strip and report re-run as you change a knob.")
                .small()
                .weak(),
        );
        ui.add_space(6.0);

        // The editable knobs run before the report borrow: each change invalidates
        // the cache and re-runs the pass in place, so the strip and the report
        // below reflect the new cut.
        self.normalize_knobs(ui);

        ui.add_space(6.0);
        ui.separator();
        ui.label(egui::RichText::new("Result").small().strong());
        ui.add_space(4.0);

        // Borrow the (re-run) result after the knobs above refreshed the cache.
        let Some(result) = self.studio.normalize.as_ref() else {
            ui.colored_label(palette.error, "These picks could not be normalized.");
            return;
        };
        let report = &result.report;

        // Baseline drift: 0px is clean, anything else a warning.
        report_row(
            ui,
            &palette,
            drift_status(report.baseline_drift_px),
            "Baseline drift",
            &format!("{}px", report.baseline_drift_px),
        );
        // Scale match: how well subject heights agreed before correction.
        report_row(
            ui,
            &palette,
            scale_status(report.scale_match_pct),
            "Scale match",
            &format!("{}%", report.scale_match_pct),
        );
        // Edge clear: any frame whose landed bbox touches a canvas edge ships
        // clipped. Ok when none touch, Error otherwise.
        let touched = report.edge_touch_frames.len();
        report_row(
            ui,
            &palette,
            edge_status(touched),
            "Edge clear",
            &if touched == 0 { "clear".to_string() } else { format!("{touched} clipped") },
        );
        // Scale parity: how well the landed subject heights agree AFTER scale
        // correction — the post-scale companion to Scale match.
        report_row(
            ui,
            &palette,
            scale_status(report.scale_parity_pct),
            "Scale parity",
            &format!("{}%", report.scale_parity_pct),
        );
        // Components: the most parts any landed frame split into. One clean body
        // is Ok; more is a warning — a stray keyed-out speck or a fragmented body,
        // the defect edge-touch alone misses. Static sheets, with no interframe
        // motion to lean on, are the most prone to it, so it reads here.
        report_row(
            ui,
            &palette,
            components_status(report.max_components),
            "Components",
            &if report.max_components <= 1 {
                "single body".to_string()
            } else {
                format!("{} parts", report.max_components)
            },
        );
        // Seam verdict: reuse the pick-stage colour thresholds via the SeamMatch.
        report_row(ui, &palette, seam_status(report.seam), "Loop seam", seam_label(report.seam));
        ui.label(egui::RichText::new(format!("Reference height {}px", report.reference_height)).small().weak());

        if !report.warnings.is_empty() {
            ui.add_space(6.0);
            ui.separator();
            ui.label(egui::RichText::new("Warnings").small().strong());
            for warning in &report.warnings {
                ui.colored_label(palette.warning, egui::RichText::new(format!("{} {warning}", crate::icons::INFO)).small());
            }
        }

        ui.add_space(8.0);
        ui.separator();
        if ui.button(format!("{} Next: land", crate::icons::RIGHT)).clicked() {
            self.studio.stage = StudioStage::Land;
        }
    }

    /// Recomputes the cached [`NormalizeResult`] when its inputs changed, so the
    /// Normalize stage and Land share one pass. A no-op when the cache is fresh.
    /// CPU-only over the pick buffers (a handful of small frames bounded by the
    /// pick region), so it runs inline; if the frame count and canvas ever make
    /// it hitch, move the `normalize_frames` call into `spawn_blocking` — the
    /// inputs are owned and there is no lock.
    fn refresh_normalize_cache(&mut self) {
        let Some(key) = self.normalize_cache_key() else {
            self.studio.normalize = None;
            self.studio.normalize_key = None;
            self.studio.normalize_textures.clear();
            return;
        };
        if self.studio.normalize_key.as_ref() == Some(&key) && self.studio.normalize.is_some() {
            return;
        }
        let result = self.compute_normalize();
        self.studio.normalize_textures.clear();
        self.studio.normalize = result;
        self.studio.normalize_key = self.studio.normalize.as_ref().map(|_| key);
    }

    // ── Land stage ───────────────────────────────────────────────────────────

    /// A preview of the picks about to land, or a confirmation once landed.
    fn studio_land_surface(&mut self, ui: &mut egui::Ui) {
        if self.studio.landed {
            centered_hint(
                ui,
                "Landed — the loop is on the timeline. Back to editor to keep working, or generate another clip.",
            );
            return;
        }
        let Some(i) = self.anim_selected else {
            centered_hint(ui, "Pick frames from a clip first.");
            return;
        };
        let n = self.anim_candidates[i].frames.len();
        if n == 0 {
            return;
        }
        let total = ui.available_size();
        ui.allocate_ui(egui::vec2(total.x, (total.y - 8.0).max(80.0)), |ui| {
            let scrub = self.anim_scrub;
            self.studio_clip_frame(ui, i, scrub);
        });
    }

    /// The land inspector: Integrate, and the background-removal note.
    fn studio_land_inspector(&mut self, ui: &mut egui::Ui) {
        let picks_len = self.anim_card().map_or(0, |c| c.picks.len());
        ui.label(egui::RichText::new("Land the loop").strong());
        ui.label(
            egui::RichText::new(format!(
                "{picks_len} picked frames normalize and land as a new layer and a tagged range, in one undoable edit."
            ))
            .small()
            .weak(),
        );
        if ui
            .add_enabled(
                picks_len > 0 && !self.studio.landed,
                egui::Button::new(format!("{} Integrate onto timeline", crate::icons::CHECK)),
            )
            .clicked()
        {
            self.studio_land();
        }
        if self.studio.landed {
            ui.colored_label(
                crate::theme::Palette::for_theme(ui.ctx().theme()).success,
                format!("{} landed", crate::icons::CHECK),
            );
            ui.add_space(6.0);
            if ui
                .button(format!("{} Export sprite…", crate::icons::DOWNLOAD))
                .on_hover_text("Write the landed loop as a PNG sequence, looping GIF, or packed atlas")
                .clicked()
            {
                self.open_export_dialog();
            }
        }
        if self.studio.kind == AnimKind::Custom {
            ui.label(egui::RichText::new("Custom animations are not tracked in the cascade grid.").small().weak());
        }
        ui.add_space(8.0);
        ui.separator();
        if self.studio.remove_on_land {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Lands keyed with").small().weak());
                let mut col = crate::editor::to_color32(self.bg_key_color);
                if ui.color_edit_button_srgba(&mut col).changed() {
                    self.set_studio_key(crate::editor::from_color32(col));
                }
                if ui.small_button("don't").on_hover_text("Leave removal to the timeline op instead").clicked() {
                    self.studio.remove_on_land = false;
                }
            });
        } else {
            ui.label(
                egui::RichText::new("Lands with the backdrop intact. Pick a key in the Clip stage to land it stripped, or remove it later from the timeline.")
                    .small()
                    .weak(),
            );
        }
    }

    // ── Sheet (slice) stage ────────────────────────────────────────────────────

    /// The Sheet stage surface: the raw generated sheet at fit-to-view (or 1:1),
    /// with the slice grid drawn over it. Dragging a grid edge adjusts the matching
    /// uniform slice parameter and re-slices live. Only the static-sheet path
    /// retains a sheet; the i2v path shows a hint.
    #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn studio_sheet_surface(&mut self, ui: &mut egui::Ui) {
        let Some(sheet) = self.studio.sheet.as_ref() else {
            centered_hint(
                ui,
                "No sheet to slice. This stage is for the static grid-sheet path — generate a sheet in the Motion stage.",
            );
            return;
        };
        let (iw, ih) = (sheet.width(), sheet.height());
        if iw == 0 || ih == 0 {
            centered_hint(ui, "The generated sheet is empty.");
            return;
        }
        let ctx = ui.ctx().clone();
        if self.studio.sheet_texture.is_none() {
            self.studio.sheet_texture = pixel_buffer_to_texture(&ctx, "studio_sheet", sheet);
        }
        let Some(tex) = self.studio.sheet_texture.as_ref() else {
            centered_hint(ui, "The generated sheet could not be displayed.");
            return;
        };
        let tex_id = tex.id();

        let canvas = ui.available_rect_before_wrap();
        // Fit-to-view, or 1:1 (one screen pixel per sheet pixel).
        let fit = (canvas.width() / iw as f32).min(canvas.height() / ih as f32).max(0.01);
        let scale = if self.studio.sheet_one_to_one { 1.0 } else { fit };
        let draw = egui::vec2(iw as f32 * scale, ih as f32 * scale);
        let rect = egui::Rect::from_center_size(canvas.center(), draw);

        let resp = ui.allocate_rect(canvas, egui::Sense::click_and_drag());
        let painter = ui.painter_at(canvas);
        painter.image(
            tex_id,
            rect,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );

        self.sheet_grid_drag(&resp, rect, iw, ih);
        sheet_draw_grid(&painter, &self.studio.slice, rect, iw, ih, ui.ctx().theme());

        // Re-slice live when the spec changed since the last cut.
        if self.studio.sheet_sliced.as_ref() != Some(&self.studio.slice) {
            self.reslice_static_sheet();
        }
    }

    /// Handles an edge drag on the slice grid: picks the grabbed edge on press,
    /// then maps the frame's drag delta (screen pixels) into sheet pixels and
    /// applies it to the uniform slice parameter that edge controls.
    #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
    fn sheet_grid_drag(&mut self, resp: &egui::Response, rect: egui::Rect, iw: u32, ih: u32) {
        if resp.drag_started() {
            self.studio.sheet_drag = resp
                .interact_pointer_pos()
                .and_then(|p| sheet_pick_edge(&self.studio.slice, p, rect, iw, ih, 8.0));
        }
        if resp.drag_stopped() {
            self.studio.sheet_drag = None;
        }
        let Some(edge) = self.studio.sheet_drag else {
            return;
        };
        let d = resp.drag_delta();
        let (dx, dy) = (d.x / rect.width().max(1.0) * iw as f32, d.y / rect.height().max(1.0) * ih as f32);
        let delta = match edge {
            SliceEdge::Top | SliceEdge::Bottom | SliceEdge::InteriorY(_) => dy,
            _ => dx,
        };
        let delta = delta.round() as i32;
        if delta != 0 {
            let spec = self.studio.slice.clone();
            self.studio.slice = slice_drag_edge(spec, edge, delta, iw, ih);
        }
    }

    /// Re-cuts the retained sheet through the current slice spec and replaces the
    /// selected candidate's frames in place, re-deriving its loop markers and
    /// picks so the Clip / Pick / Normalize / Land tail sees the new cut. A no-op
    /// when no sheet is retained or no candidate is selected.
    fn reslice_static_sheet(&mut self) {
        let Some(sheet) = self.studio.sheet.as_ref() else {
            return;
        };
        let spec = self.studio.slice.clone();
        let fps = self.anim_fps;
        let Ok(frames) = ai::slice_sheet_to_frames(sheet, &spec, fps) else {
            // A collapsed spec yields no frames; leave the last good cut in place
            // and mark this spec as attempted so we do not retry every frame.
            self.studio.sheet_sliced = Some(spec);
            return;
        };
        let Some(idx) = self.anim_selected else {
            self.studio.sheet_sliced = Some(spec);
            return;
        };
        let Some(candidate) = self.anim_candidates.get_mut(idx) else {
            // Mark the spec resolved like the sibling guards above so a stale
            // `anim_selected` index doesn't re-slice the sheet every frame.
            self.studio.sheet_sliced = Some(spec);
            return;
        };
        candidate.markers = crate::anim::auto_loop_markers(&frames);
        candidate.picks = crate::anim::pick_loop_frames(&frames, candidate.markers, self.anim_target_frames as usize);
        candidate.frames = frames;
        self.studio.sheet_sliced = Some(spec);
        // The new frame set invalidates any cached scrub position and normalize.
        self.anim_scrub = 0;
        self.studio.normalize = None;
        self.studio.normalize_key = None;
        self.studio.normalize_textures.clear();
    }

    /// The Sheet stage inspector: the slice-grid spinners (rows, cols, offsets,
    /// gutters, inset), a fit / 1:1 toggle, a reset-to-uniform action, and a
    /// confirm affordance that carries the cut into the Clip stage.
    fn studio_sheet_inspector(&mut self, ui: &mut egui::Ui) {
        if self.studio.sheet.is_none() {
            ui.label(egui::RichText::new("No sheet retained. Generate a static sheet in the Motion stage.").weak());
            return;
        }
        let (sw, sh) = self.studio.sheet.as_ref().map_or((0, 0), |s| (s.width(), s.height()));
        ui.label(egui::RichText::new("Slice the sheet").strong());
        ui.label(
            egui::RichText::new("Drag a grid edge or use the spinners. The cells re-slice live; a uniform grid cuts exactly as before.")
                .small()
                .weak(),
        );
        ui.add_space(6.0);

        ui.checkbox(&mut self.studio.sheet_one_to_one, "Show at 1:1")
            .on_hover_text("Off fits the sheet to the view; on shows it at one screen pixel per sheet pixel");
        ui.add_space(6.0);

        let before = self.studio.slice.clone();
        let mut spec = self.studio.slice.clone();
        egui::Grid::new("slice_grid_spinners").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
            ui.label("Rows");
            ui.add(egui::DragValue::new(&mut spec.rows).range(1..=16));
            ui.end_row();
            ui.label("Cols");
            ui.add(egui::DragValue::new(&mut spec.cols).range(1..=16));
            ui.end_row();
            ui.label("Offset x");
            ui.add(egui::DragValue::new(&mut spec.offset_x).range(0..=sw.saturating_sub(1)));
            ui.end_row();
            ui.label("Offset y");
            ui.add(egui::DragValue::new(&mut spec.offset_y).range(0..=sh.saturating_sub(1)));
            ui.end_row();
            ui.label("Gutter x");
            ui.add(egui::DragValue::new(&mut spec.gutter_x).range(0..=sw / 2));
            ui.end_row();
            ui.label("Gutter y");
            ui.add(egui::DragValue::new(&mut spec.gutter_y).range(0..=sh / 2));
            ui.end_row();
            ui.label("Inset");
            ui.add(egui::DragValue::new(&mut spec.inset).range(0..=sw.min(sh) / 2));
            ui.end_row();
        });
        if spec != before {
            self.studio.slice = spec;
        }

        let (cw, ch) = slice_cell_size(&self.studio.slice, sw, sh);
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(format!(
                "Cell {cw} x {ch}px, {} cells",
                self.studio.slice.rows.max(1) * self.studio.slice.cols.max(1)
            ))
            .small()
            .weak(),
        );

        // Phase B status: surface how many dividers are off the uniform grid so a
        // non-uniform cut is legible without re-reading the overlay.
        let custom = self.studio.slice.overrides.x.len() + self.studio.slice.overrides.y.len();
        if custom > 0 {
            ui.label(
                egui::RichText::new(format!("{custom} custom divider(s) — drag a single divider for a variable cell"))
                    .small()
                    .weak(),
            );
        } else {
            ui.label(egui::RichText::new("Drag a single interior divider for a variable cell size").small().weak());
        }

        ui.horizontal(|ui| {
            if ui
                .button("Reset to uniform")
                .on_hover_text("Drop offset, gutter, inset, and any custom dividers back to a plain even grid")
                .clicked()
            {
                self.studio.slice = SliceGrid::uniform(self.studio.slice.rows, self.studio.slice.cols);
            }
        });

        // A collapsed spec produced no cut; warn rather than silently keeping the
        // last good frames.
        if self
            .studio
            .sheet
            .as_ref()
            .is_some_and(|s| ai::slice_sheet_to_frames(s, &self.studio.slice, self.anim_fps).is_err())
        {
            ui.colored_label(
                ui.visuals().warn_fg_color,
                "This spec collapses a cell to nothing — widen a cell or lower a margin.",
            );
        }

        ui.add_space(8.0);
        ui.separator();
        if ui.button(format!("{} Confirm slice — next: clip", crate::icons::RIGHT)).clicked() {
            // Make sure the latest spec is cut before moving on, then route to the
            // same Clip tail the static path used before this stage existed.
            self.reslice_static_sheet();
            self.studio.stage = StudioStage::Clip;
        }
    }

    /// Lands the picked frames and flags the session as landed.
    fn studio_land(&mut self) {
        self.integrate_picked();
        self.studio.landed = true;
    }
}

/// Which edge of the slice grid a pointer drag grabbed. Vertical edges move on
/// the x axis (changing `offset_x` or the trailing inset), horizontal edges on
/// the y axis. Interior edges are the dividers between cells; the outer edges are
/// the grid's bounding rectangle.
///
/// An interior divider carries its index (`1..cols` for `InteriorX`, `1..rows`
/// for `InteriorY`): the boundary after that column/row. Dragging it sets a
/// per-divider override (Phase B), so one divider moves and its two neighbouring
/// cells resize while the rest of the grid holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SliceEdge {
    /// The grid's left bound — drags adjust `offset_x`.
    Left,
    /// The grid's top bound — drags adjust `offset_y`.
    Top,
    /// The grid's right bound — drags adjust the implied cell width (uniform).
    Right,
    /// The grid's bottom bound — drags adjust the implied cell height (uniform).
    Bottom,
    /// Interior vertical divider `index` (between columns `index - 1` and
    /// `index`) — drags set an x override for that divider.
    InteriorX(u32),
    /// Interior horizontal divider `index` (between rows `index - 1` and
    /// `index`) — drags set a y override for that divider.
    InteriorY(u32),
}

/// The derived cell size for a [`SliceGrid`] over a `sheet_w` x `sheet_h` sheet,
/// matching `slice_grid_spec`'s formula (saturating, floor-divided). Returns
/// `(cell_w, cell_h)`; either may be zero for a degenerate spec.
#[must_use]
pub(crate) fn slice_cell_size(spec: &SliceGrid, sheet_w: u32, sheet_h: u32) -> (u32, u32) {
    let cols = spec.cols.max(1);
    let rows = spec.rows.max(1);
    let cols_gutter = spec.gutter_x.saturating_mul(cols - 1);
    let rows_gutter = spec.gutter_y.saturating_mul(rows - 1);
    let usable_w = sheet_w.saturating_sub(spec.offset_x).saturating_sub(cols_gutter);
    let usable_h = sheet_h.saturating_sub(spec.offset_y).saturating_sub(rows_gutter);
    (usable_w / cols, usable_h / rows)
}

/// The x positions (in sheet pixels) of every vertical cell edge for `spec`:
/// for each column, its left and right edge, so the returned vector has
/// `2 * cols` entries (a left/right pair per column, gutters showing as gaps
/// between pairs). Used to draw the grid and hit-test edge drags. Delegates to
/// [`SliceGrid::x_edges`] so the overlay reflects any per-divider override.
#[must_use]
pub(crate) fn slice_x_edges(spec: &SliceGrid, sheet_w: u32) -> Vec<u32> {
    spec.x_edges(sheet_w).into_iter().flat_map(|(l, r)| [l, r]).collect()
}

/// The y positions (in sheet pixels) of every horizontal cell edge for `spec`:
/// for each row, its top and bottom edge, so the returned vector has `2 * rows`
/// entries. The vertical companion to [`slice_x_edges`].
#[must_use]
pub(crate) fn slice_y_edges(spec: &SliceGrid, sheet_h: u32) -> Vec<u32> {
    spec.y_edges(sheet_h).into_iter().flat_map(|(t, b)| [t, b]).collect()
}

/// Applies a drag of `delta` sheet-pixels on `edge` to `spec`, adjusting the one
/// parameter that edge controls, clamped so the grid stays valid against a
/// `sheet_w` x `sheet_h` sheet. Pure so the drag math is testable without egui.
///
/// - Left/top outer edges move `offset_x`/`offset_y` — the leading margin before
///   the grid.
/// - Right/bottom outer edges have no stored field of their own: the cell size is
///   derived from the offsets and gutters, with the trailing margin absorbing the
///   slack. Pulling them inward (a negative delta) maps to a positive `inset`,
///   trimming every cell uniformly so the visible outer edge moves in.
/// - Interior dividers set a per-divider override (Phase B): the dragged divider
///   alone moves, resizing its two neighbouring cells; the rest of the grid
///   holds. A divider dragged exactly back onto its uniform position drops its
///   override, so a fully reset grid is byte-identical to the uniform spec.
#[must_use]
pub(crate) fn slice_drag_edge(mut spec: SliceGrid, edge: SliceEdge, delta: i32, sheet_w: u32, sheet_h: u32) -> SliceGrid {
    let apply = |value: u32, delta: i32, max: u32| -> u32 {
        let next = i64::from(value) + i64::from(delta);
        next.clamp(0, i64::from(max)) as u32
    };
    match edge {
        SliceEdge::Left => {
            // Cap the offset so at least one pixel of cell width survives.
            let max = sheet_w.saturating_sub(spec.cols.max(1));
            spec.offset_x = apply(spec.offset_x, delta, max);
        }
        SliceEdge::Top => {
            let max = sheet_h.saturating_sub(spec.rows.max(1));
            spec.offset_y = apply(spec.offset_y, delta, max);
        }
        SliceEdge::Right => {
            // The right edge has no stored field; growing it shrinks the implied
            // trailing margin, which the derived cell size already absorbs. A
            // negative drag (pulling in) maps to a positive inset, trimming every
            // cell uniformly so the visible right edge moves in.
            let max = slice_cell_size(&spec, sheet_w, sheet_h).0 / 2;
            spec.inset = apply(spec.inset, -delta, max);
        }
        SliceEdge::Bottom => {
            let max = slice_cell_size(&spec, sheet_w, sheet_h).1 / 2;
            spec.inset = apply(spec.inset, -delta, max);
        }
        SliceEdge::InteriorX(index) => drag_divider(&mut spec, Axis::X, index, delta, sheet_w),
        SliceEdge::InteriorY(index) => drag_divider(&mut spec, Axis::Y, index, delta, sheet_h),
    }
    spec
}

/// Which axis a per-divider drag moves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Axis {
    X,
    Y,
}

/// Moves interior divider `index` on `axis` by `delta` sheet-pixels via a
/// per-divider override, clamped between its neighbouring cell edges. A divider
/// returned exactly to its uniform position drops the override, so an untouched
/// grid stays the uniform Phase A spec.
fn drag_divider(spec: &mut SliceGrid, axis: Axis, index: u32, delta: i32, span: u32) {
    let (count, gutter) = match axis {
        Axis::X => (spec.cols.max(1), spec.gutter_x),
        Axis::Y => (spec.rows.max(1), spec.gutter_y),
    };
    if index == 0 || index >= count {
        return;
    }
    // The current edge pairs (with overrides already applied), so a drag stacks
    // on the divider's present position rather than its uniform seed.
    let pairs = match axis {
        Axis::X => spec.x_edges(span),
        Axis::Y => spec.y_edges(span),
    };
    let prev = (index - 1) as usize;
    let next = index as usize;
    let Some(&(_, cur)) = pairs.get(prev) else {
        return;
    };
    // Bound the divider strictly between its neighbours so neither cell collapses.
    let lower = pairs.get(prev).map_or(0, |&(l, _)| l + 1);
    let upper = pairs.get(next).map_or(span, |&(_, r)| r.saturating_sub(gutter + 1));
    if lower > upper {
        return;
    }
    let pos = (i64::from(cur) + i64::from(delta)).clamp(i64::from(lower), i64::from(upper)) as u32;

    // The uniform position of this divider; matching it drops the override.
    let uniform = uniform_divider(spec, axis, index, span);
    set_divider_override(spec, axis, index, pos, uniform);
}

/// The uniform (no-override) position of interior divider `index` on `axis`:
/// the right edge of cell `index - 1`.
fn uniform_divider(spec: &SliceGrid, axis: Axis, index: u32, span: u32) -> u32 {
    let bare = SliceGrid {
        overrides: pixhaus_core::transforms::SliceOverrides::default(),
        ..spec.clone()
    };
    let pairs = match axis {
        Axis::X => bare.x_edges(span),
        Axis::Y => bare.y_edges(span),
    };
    pairs.get((index - 1) as usize).map_or(0, |&(_, r)| r)
}

/// Sets (or clears) the override for divider `index` on `axis`. A `pos` equal to
/// the uniform divider clears the entry; otherwise it is recorded, replacing any
/// prior entry for the same divider.
fn set_divider_override(spec: &mut SliceGrid, axis: Axis, index: u32, pos: u32, uniform: u32) {
    let list = match axis {
        Axis::X => &mut spec.overrides.x,
        Axis::Y => &mut spec.overrides.y,
    };
    list.retain(|&(d, _)| d != index);
    if pos != uniform {
        list.push((index, pos));
        list.sort_unstable_by_key(|&(d, _)| d);
    }
}

/// Picks the slice edge a press at screen position `p` grabs, given the screen
/// `rect` the `iw` x `ih` sheet is drawn into and a `tol` in screen pixels.
/// Vertical edges (outer left/right, interior columns) are tested against `p.x`,
/// horizontal edges against `p.y`; the nearest within `tol` wins. `None` if the
/// press missed every edge. Outer edges win ties so they stay grabbable.
fn sheet_pick_edge(spec: &SliceGrid, p: egui::Pos2, rect: egui::Rect, iw: u32, ih: u32, tol: f32) -> Option<SliceEdge> {
    let to_x = |sx: u32| view_to_screen(sx as f32, 0.0, rect, iw, ih).x;
    let to_y = |sy: u32| view_to_screen(0.0, sy as f32, rect, iw, ih).y;
    // Resolved edges (overrides applied) so a moved divider is grabbable where
    // it is drawn, not where the uniform grid would place it.
    let xs = spec.x_edges(iw);
    let ys = spec.y_edges(ih);

    // Outer edges first so they take ties with adjacent interior dividers.
    let mut best: Option<(SliceEdge, f32)> = None;
    let mut consider = |edge: SliceEdge, screen: f32, axis: f32| {
        let d = (screen - axis).abs();
        if d <= tol && best.is_none_or(|(_, bd)| d < bd) {
            best = Some((edge, d));
        }
    };
    // Left/right outer bounds, from the first cell's left and the last cell's right.
    if let Some(&(left, _)) = xs.first() {
        consider(SliceEdge::Left, to_x(left), p.x);
    }
    if let Some(&(_, right)) = xs.last() {
        consider(SliceEdge::Right, to_x(right), p.x);
    }
    // Top/bottom outer bounds.
    if let Some(&(top, _)) = ys.first() {
        consider(SliceEdge::Top, to_y(top), p.y);
    }
    if let Some(&(_, bottom)) = ys.last() {
        consider(SliceEdge::Bottom, to_y(bottom), p.y);
    }
    // Interior vertical dividers (between columns): divider `c` is the right edge
    // of column `c - 1`, for `c` in `1..cols`.
    for (idx, pair) in xs.iter().enumerate().skip(1) {
        let index = idx as u32;
        consider(SliceEdge::InteriorX(index), to_x(pair.0), p.x);
    }
    // Interior horizontal dividers (between rows).
    for (idx, pair) in ys.iter().enumerate().skip(1) {
        let index = idx as u32;
        consider(SliceEdge::InteriorY(index), to_y(pair.0), p.y);
    }
    best.map(|(edge, _)| edge)
}

/// Draws the slice grid over the sheet: a cell rectangle per cell, plus center
/// crosshairs on the first cell so drift against the subject reads at a glance
/// (borrowed from agent-sprite-forge's layout guide).
#[allow(clippy::cast_precision_loss)]
fn sheet_draw_grid(painter: &egui::Painter, spec: &SliceGrid, rect: egui::Rect, iw: u32, ih: u32, theme: egui::Theme) {
    let palette = crate::theme::Palette::for_theme(theme);
    let line = egui::Stroke::new(1.5, palette.success);
    let faint = egui::Stroke::new(1.0, palette.success.gamma_multiply(0.4));
    let xs = slice_x_edges(spec, iw);
    let ys = slice_y_edges(spec, ih);
    let to = |sx: f32, sy: f32| view_to_screen(sx, sy, rect, iw, ih);
    // Cell rectangles: pair (left, right) over (top, bottom).
    for (xl, xr) in xs.chunks_exact(2).map(|p| (p[0], p[1])) {
        for (yt, yb) in ys.chunks_exact(2).map(|p| (p[0], p[1])) {
            let cell = egui::Rect::from_min_max(to(xl as f32, yt as f32), to(xr as f32, yb as f32));
            painter.rect_stroke(cell, 0.0, line, egui::StrokeKind::Inside);
        }
    }
    // Crosshairs through the first cell's center, to gauge subject centering.
    // Size from the first resolved cell so an overridden divider 1 still centers.
    if let (&[xl, xr, ..], &[yt, yb, ..]) = (xs.as_slice(), ys.as_slice()) {
        let cx = f32::midpoint(xl as f32, xr as f32);
        let cy = f32::midpoint(yt as f32, yb as f32);
        painter.line_segment([to(xl as f32, cy), to(xr as f32, cy)], faint);
        painter.line_segment([to(cx, yt as f32), to(cx, yb as f32)], faint);
    }
}

/// A centered, wrapped muted hint filling the surface — the empty-state message.
fn centered_hint(ui: &mut egui::Ui, text: &str) {
    ui.centered_and_justified(|ui| {
        ui.label(egui::RichText::new(text).weak());
    });
}

/// The review category for one [`NormalizeReport`] field, decoupled from egui so
/// it is unit-testable: a value maps to Ok / Warning / Error, which the inspector
/// renders with the studio's success / warning / error palette colours.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ReportStatus {
    /// Within the clean band — rendered with the success colour and a check.
    Ok,
    /// Usable but worth a glance — rendered with the warning colour.
    Warning,
    /// A defect that will read in-game — rendered with the error colour.
    Error,
}

impl ReportStatus {
    /// The status glyph, from the Phosphor icon set — never an emoji.
    fn icon(self) -> &'static str {
        match self {
            ReportStatus::Ok => crate::icons::CHECK,
            ReportStatus::Warning => crate::icons::INFO,
            ReportStatus::Error => crate::icons::X,
        }
    }

    /// The palette colour for this status.
    fn color(self, palette: &crate::theme::Palette) -> egui::Color32 {
        match self {
            ReportStatus::Ok => palette.success,
            ReportStatus::Warning => palette.warning,
            ReportStatus::Error => palette.error,
        }
    }
}

/// Classifies the foot-baseline drift: zero is clean, anything else is a warning
/// (the loop still lands, but the feet wobble).
#[must_use]
pub(crate) fn drift_status(drift_px: u32) -> ReportStatus {
    if drift_px == 0 { ReportStatus::Ok } else { ReportStatus::Warning }
}

/// Classifies the scale-match percent: near-perfect is clean, a moderate gap is
/// a warning, a wide spread is an error. Mirrors the `< 60%` warning the core
/// normalize pass already records.
#[must_use]
pub(crate) fn scale_status(pct: u32) -> ReportStatus {
    if pct >= 90 {
        ReportStatus::Ok
    } else if pct >= 60 {
        ReportStatus::Warning
    } else {
        ReportStatus::Error
    }
}

/// Classifies the edge-touch count: zero clipped frames is clean, any clipped
/// frame is an error (a subject at a canvas edge ships visibly cropped). The QC
/// is advisory-strong — it surfaces the defect, it does not block Land.
#[must_use]
pub(crate) fn edge_status(touched_frames: usize) -> ReportStatus {
    if touched_frames == 0 { ReportStatus::Ok } else { ReportStatus::Error }
}

/// Classifies the component count: a single clean body (`<= 1`) is Ok, anything
/// more is a warning — a frame that split into multiple parts is usually a stray
/// keyed-out speck or a body that fragmented, the defect edge-touch alone misses.
/// Advisory, like the rest of the QC; it surfaces the defect, it does not block.
#[must_use]
pub(crate) fn components_status(max_components: u32) -> ReportStatus {
    if max_components <= 1 { ReportStatus::Ok } else { ReportStatus::Warning }
}

/// Classifies the loop seam, reusing the pick-stage thresholds: a clean seam is
/// Ok, a close seam is a warning, a drifting seam is an error.
#[must_use]
pub(crate) fn seam_status(seam: SeamMatch) -> ReportStatus {
    match seam {
        SeamMatch::Ok => ReportStatus::Ok,
        SeamMatch::Close => ReportStatus::Warning,
        SeamMatch::Drift => ReportStatus::Error,
    }
}

/// A human-readable label for the seam verdict.
#[must_use]
pub(crate) fn seam_label(seam: SeamMatch) -> &'static str {
    match seam {
        SeamMatch::Ok => "clean",
        SeamMatch::Close => "close",
        SeamMatch::Drift => "drifts",
    }
}

/// One status row in the normalize inspector: a coloured glyph, the field name,
/// and its value.
fn report_row(ui: &mut egui::Ui, palette: &crate::theme::Palette, status: ReportStatus, name: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.colored_label(status.color(palette), status.icon());
        ui.label(egui::RichText::new(name).small().weak());
        ui.label(egui::RichText::new(value).small().strong());
    });
}

/// The shared center viewport for a generation stage: a toolbar (Fit, mask tool,
/// brush, inpaint prompt, Regenerate) above the pan/zoom canvas of `png` with its
/// mask overlay. Returns whether "Regenerate masked region" was clicked. The same
/// surface drives both the Anchor and First-frame stages.
fn refine_surface(ui: &mut egui::Ui, png: &[u8], texture: &mut Option<egui::TextureHandle>, view: &mut RefineView, enabled: bool) -> bool {
    let mut do_inpaint = false;
    ui.horizontal_wrapped(|ui| {
        if ui.button("Fit").on_hover_text("Reset zoom and pan").clicked() {
            view.zoom = 1.0;
            view.pan = egui::Vec2::ZERO;
        }
        ui.separator();
        ui.label("Mask:");
        ui.selectable_value(&mut view.mask_tool, MaskTool::Brush, "Brush");
        ui.selectable_value(&mut view.mask_tool, MaskTool::Box, "Box");
        match view.mask_tool {
            MaskTool::Brush => {
                ui.checkbox(&mut view.painting, "Paint");
                ui.add(egui::Slider::new(&mut view.brush, 1.0..=24.0).text("brush"));
            }
            MaskTool::Box => {
                ui.label(egui::RichText::new("drag to move · corners scale · stem rotates").small().weak());
            }
        }
        if ui.button(format!("{} Clear", crate::icons::REMOVE)).clicked() {
            if let Some(mask) = view.mask.as_mut() {
                mask.clear();
            }
            view.gizmo = None;
        }
    });
    ui.horizontal(|ui| {
        let has_mask = view.mask.as_ref().is_some_and(|m| !m.is_empty());
        // Right-to-left so the button keeps its natural width and the prompt
        // field fills whatever is left, instead of reserving a fixed slice that
        // clipped the button on narrower panels.
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            do_inpaint = ui
                .add_enabled(
                    enabled && has_mask,
                    egui::Button::new(format!("{} Regenerate masked region", crate::icons::SPARKLE)),
                )
                .on_hover_text("Repaint only the masked pixels")
                .clicked();
            ui.add(
                egui::TextEdit::singleline(&mut view.inpaint_prompt)
                    .hint_text("fix the masked region")
                    .desired_width(ui.available_width()),
            );
        });
    });
    ui.separator();
    refine_canvas(ui, png, texture, view);
    do_inpaint
}

/// Draws `png` (cached in `texture`) in the remaining area at `view`'s pan/zoom,
/// overlays the inpaint mask and box gizmo, and routes wheel-zoom, drag-pan, brush
/// paint, and gizmo manipulation into `view`.
fn refine_canvas(ui: &mut egui::Ui, png: &[u8], texture: &mut Option<egui::TextureHandle>, view: &mut RefineView) {
    let ctx = ui.ctx().clone();
    if texture.is_none() {
        *texture = load_png_texture(&ctx, "refine", png);
    }
    let Some(tex) = texture.clone() else {
        centered_hint(ui, "This result could not be decoded.");
        return;
    };
    let img = tex.size_vec2();
    let (iw, ih) = (tex.size()[0] as u32, tex.size()[1] as u32);
    ensure_view_mask(view, iw, ih);

    let (canvas, resp) = ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());

    // Wheel zoom about the cursor.
    let scroll = ui.input(|i| i.smooth_scroll_delta.y);
    if resp.hovered() && scroll.abs() > 0.01 {
        if let Some(cursor) = resp.hover_pos() {
            let before = image_view_rect(canvas, img, view.zoom, view.pan);
            let pivot = view_to_image(cursor, before, iw, ih);
            view.zoom = (view.zoom * (1.0 + scroll / 400.0)).clamp(0.2, 16.0);
            let after = image_view_rect(canvas, img, view.zoom, view.pan);
            view.pan += cursor - view_to_screen(pivot.0, pivot.1, after, iw, ih);
        }
    }
    let rect = image_view_rect(canvas, img, view.zoom, view.pan);
    let to_screen = |ix: f32, iy: f32| view_to_screen(ix, iy, rect, iw, ih);
    let to_image = |p: egui::Pos2| view_to_image(p, rect, iw, ih);

    // Middle-drag always pans; otherwise route the primary drag to the active
    // tool, falling back to panning on empty space.
    if resp.dragged_by(egui::PointerButton::Middle) {
        view.pan += resp.drag_delta();
    } else if resp.dragged_by(egui::PointerButton::Primary) {
        match view.mask_tool {
            MaskTool::Brush if view.painting => {
                if let Some(pos) = resp.interact_pointer_pos() {
                    if rect.contains(pos) {
                        let (mx, my) = to_image(pos);
                        let brush = view.brush;
                        if let Some(mask) = view.mask.as_mut() {
                            mask.stamp(mx, my, brush);
                        }
                    }
                }
            }
            MaskTool::Box => {
                if !gizmo_drag_update(&resp, view, iw, ih, rect) {
                    view.pan += resp.drag_delta();
                }
            }
            MaskTool::Brush => view.pan += resp.drag_delta(),
        }
    }
    if resp.drag_stopped() {
        view.gizmo_drag = None;
    }

    rebuild_mask_overlay(&ctx, view.mask.as_mut());

    let stroke_color = ui.visuals().widgets.noninteractive.bg_stroke.color;
    let accent = ui.visuals().selection.stroke.color;
    let painter = ui.painter_at(canvas);
    let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
    painter.image(tex.id(), rect, uv, egui::Color32::WHITE);
    if let Some(mask) = &view.mask {
        if let Some(mtex) = &mask.texture {
            painter.image(mtex.id(), rect, uv, egui::Color32::WHITE);
        }
    }
    match view.mask_tool {
        MaskTool::Box => {
            if let Some(giz) = view.gizmo {
                let corners = giz.corners();
                let pts: Vec<egui::Pos2> = corners.iter().map(|&(x, y)| to_screen(x, y)).collect();
                for k in 0..4 {
                    painter.line_segment([pts[k], pts[(k + 1) % 4]], egui::Stroke::new(1.5, accent));
                }
                for p in &pts {
                    painter.circle_filled(*p, 4.0, accent);
                }
                let top_mid = to_screen(f32::midpoint(corners[0].0, corners[1].0), f32::midpoint(corners[0].1, corners[1].1));
                let (rx, ry) = giz.rotate_handle();
                let rh = to_screen(rx, ry);
                painter.line_segment([top_mid, rh], egui::Stroke::new(1.5, accent));
                painter.circle_filled(rh, 4.0, accent);
            }
        }
        MaskTool::Brush => {
            // A sizing cursor at the pointer so the brush radius is visible.
            if let Some(pos) = resp.hover_pos() {
                if rect.contains(pos) {
                    let screen_radius = (view.brush / iw as f32 * rect.width()).max(1.0);
                    painter.circle_stroke(pos, screen_radius, egui::Stroke::new(1.5, accent));
                }
            }
        }
    }
    painter.rect_stroke(rect, 2.0, egui::Stroke::new(1.0, stroke_color), egui::StrokeKind::Inside);
}

/// The screen rect the image occupies given the fit-to-`canvas` scale, the user
/// `zoom`, and the `pan` offset.
fn image_view_rect(canvas: egui::Rect, img: egui::Vec2, zoom: f32, pan: egui::Vec2) -> egui::Rect {
    let fit = (canvas.width() / img.x.max(1.0)).min(canvas.height() / img.y.max(1.0)).max(0.001);
    egui::Rect::from_center_size(canvas.center() + pan, img * (fit * zoom))
}

/// Maps an image-pixel coordinate to a screen position within `rect`.
fn view_to_screen(ix: f32, iy: f32, rect: egui::Rect, iw: u32, ih: u32) -> egui::Pos2 {
    egui::pos2(rect.left() + ix / iw as f32 * rect.width(), rect.top() + iy / ih as f32 * rect.height())
}

/// Maps a screen position to an image-pixel coordinate within `rect`.
fn view_to_image(p: egui::Pos2, rect: egui::Rect, iw: u32, ih: u32) -> (f32, f32) {
    (
        (p.x - rect.left()) / rect.width().max(1.0) * iw as f32,
        (p.y - rect.top()) / rect.height().max(1.0) * ih as f32,
    )
}

/// Ensures `view`'s mask is sized to `(w, h)`, replacing it when the dimensions
/// changed (a different result was selected).
fn ensure_view_mask(view: &mut RefineView, w: u32, h: u32) {
    if view.mask.as_ref().is_none_or(|m| m.width != w || m.height != h) {
        view.mask = Some(MaskOverlay::new(w, h));
    }
}

/// Applies a primary-button drag to the box gizmo (picking a handle on press,
/// then move / scale / rotate) and rasterizes it into the mask. Returns whether
/// the drag was consumed; `false` lets the caller pan instead.
#[allow(clippy::cast_precision_loss)]
fn gizmo_drag_update(resp: &egui::Response, view: &mut RefineView, iw: u32, ih: u32, rect: egui::Rect) -> bool {
    if view.gizmo.is_none() {
        view.gizmo = Some(BoxGizmo::centered(iw, ih));
    }
    let Some(mut giz) = view.gizmo else {
        return false;
    };
    if resp.drag_started() {
        view.gizmo_drag = resp.interact_pointer_pos().and_then(|p| {
            let (ix, iy) = view_to_image(p, rect, iw, ih);
            giz.pick_handle(|cx, cy| view_to_screen(cx, cy, rect, iw, ih), p, (ix, iy), 8.0)
        });
    }
    let Some(handle) = view.gizmo_drag else {
        return false;
    };
    if let Some(p) = resp.interact_pointer_pos() {
        let (ix, iy) = view_to_image(p, rect, iw, ih);
        // The body translates by the drag delta mapped into image space; corners
        // and the stem read the pointer's absolute image position.
        let d = resp.drag_delta();
        let dx = d.x / rect.width().max(1.0) * iw as f32;
        let dy = d.y / rect.height().max(1.0) * ih as f32;
        giz.drag_handle(handle, ix, iy, dx, dy, false);
        giz.cx = giz.cx.clamp(0.0, iw as f32);
        giz.cy = giz.cy.clamp(0.0, ih as f32);
        view.gizmo = Some(giz);
        if let Some(mask) = view.mask.as_mut() {
            rasterize_gizmo(&giz, mask);
        }
    }
    true
}

/// Rebuilds a mask overlay texture (translucent red where set) when it is dirty.
/// Updates the mask's overlay texture for its pending dirty rect. The handle is
/// allocated once (the first dirty rect is the full extent) and thereafter only
/// the changed sub-rect is re-uploaded with `set_partial` — never a full
/// per-frame `load_texture`, which is what made painting lag.
fn rebuild_mask_overlay(ctx: &egui::Context, mask: Option<&mut MaskOverlay>) {
    let Some(mask) = mask else {
        return;
    };
    let Some([rx, ry, rw, rh]) = mask.dirty.take() else {
        return;
    };
    if rw == 0 || rh == 0 {
        return;
    }
    // Pack just the dirty rect, red where set and transparent elsewhere.
    let mut bytes = Vec::with_capacity((rw * rh) as usize * 4);
    for y in ry..ry + rh {
        for x in rx..rx + rw {
            let set = mask.cells[(y as usize) * (mask.width as usize) + (x as usize)];
            bytes.extend_from_slice(if set { &[220, 40, 40, 120] } else { &[0, 0, 0, 0] });
        }
    }
    let sub = egui::ColorImage::from_rgba_unmultiplied([rw as usize, rh as usize], &bytes);
    if let Some(handle) = &mut mask.texture {
        handle.set_partial([rx as usize, ry as usize], sub, egui::TextureOptions::NEAREST);
    } else {
        // First build dirties the full extent, so `sub` is the whole image and
        // this allocates the texture at full size for later partial updates.
        mask.texture = Some(ctx.load_texture("studio_mask", sub, egui::TextureOptions::NEAREST));
    }
}

/// Draws `tex` scaled to fit the available area, preserving aspect, centered.
fn fit_image(ui: &mut egui::Ui, tex: &egui::TextureHandle) {
    fit_image_sized(ui, tex.id(), tex.size_vec2());
}

/// Draws a texture by id+size scaled to fit the available area, centered.
fn fit_image_sized(ui: &mut egui::Ui, id: egui::TextureId, size: egui::Vec2) {
    let avail = ui.available_size();
    let scale = (avail.x / size.x.max(1.0)).min(avail.y / size.y.max(1.0)).max(0.01);
    ui.centered_and_justified(|ui| {
        ui.add(egui::Image::new((id, size * scale)));
    });
}

/// Encodes a [`PixelBuffer`] to PNG, dropping any row padding. `None` if the
/// rows don't form a tight RGBA image.
fn pixel_buffer_to_png(buf: &PixelBuffer) -> Option<Vec<u8>> {
    let (w, h) = (buf.width(), buf.height());
    let row_bytes = (w * 4) as usize;
    let mut pixels = Vec::with_capacity(row_bytes * h as usize);
    for y in 0..h {
        pixels.extend_from_slice(buf.row(y)?.get(..row_bytes)?);
    }
    anim::encode_png(&VideoFrame {
        pixels,
        width: w,
        height: h,
        timestamp_ms: 0,
    })
}

/// Builds a NEAREST egui texture from a [`PixelBuffer`], dropping any row
/// padding. `None` if the buffer's rows don't form a tight RGBA image.
fn pixel_buffer_to_texture(ctx: &egui::Context, name: &str, buf: &PixelBuffer) -> Option<egui::TextureHandle> {
    let (w, h) = (buf.width(), buf.height());
    let row_bytes = (w * 4) as usize;
    let mut tight = Vec::with_capacity(row_bytes * h as usize);
    for y in 0..h {
        let row = buf.row(y)?;
        tight.extend_from_slice(row.get(..row_bytes)?);
    }
    if tight.len() != row_bytes * h as usize {
        return None;
    }
    let image = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &tight);
    Some(ctx.load_texture(name, image, egui::TextureOptions::NEAREST))
}

/// Reads the RGBA at normalized clip coordinates `(u, v)` in `0.0..=1.0`,
/// mapping them to a texel of `frame`. `None` for an empty or malformed frame.
#[must_use]
fn frame_texel_rgba(frame: &VideoFrame, u: f32, v: f32) -> Option<Rgba> {
    if frame.width == 0 || frame.height == 0 {
        return None;
    }
    let x = ((u.clamp(0.0, 1.0) * frame.width as f32) as u32).min(frame.width - 1);
    let y = ((v.clamp(0.0, 1.0) * frame.height as f32) as u32).min(frame.height - 1);
    let idx = ((y * frame.width + x) * 4) as usize;
    let p = frame.pixels.get(idx..idx + 4)?;
    Some(Rgba::new(p[0], p[1], p[2], p[3]))
}

/// Formats a clip position as `m:ss / m:ss` from the current frame, the fps, and
/// the total frame count. The total reads the last frame's time, so the readout
/// reaches its end exactly at the final frame.
#[must_use]
fn format_clip_time(frame: usize, fps: u32, total: usize) -> String {
    let fps = fps.max(1);
    let cur = frame as u32 / fps;
    let dur = total.saturating_sub(1) as u32 / fps;
    format!("{} / {}", fmt_mmss(cur), fmt_mmss(dur))
}

/// Formats whole seconds as `m:ss`.
fn fmt_mmss(secs: u32) -> String {
    format!("{}:{:02}", secs / 60, secs % 60)
}

/// The chroma key to bake into Land: `Some` when "remove background on Land" is
/// set, else `None` (removal left to the timeline op). A pure seam for testing
/// the Land wiring.
#[must_use]
pub(crate) fn land_chroma(remove_on_land: bool, color: Rgba, tolerance: u8) -> Option<ChromaKey> {
    remove_on_land.then_some(ChromaKey::new(color, tolerance))
}

/// The default `All`-mode min area: parts under this many opaque pixels are
/// dropped as keying noise.
pub(crate) const DEFAULT_LAND_MIN_AREA: u32 = 6;

/// The component policy Land measures the keyed picks under. `WholeAlpha` when
/// the backdrop is left for the timeline op (no keying, so no detached specks to
/// isolate). When keying on Land, `Largest` keeps only the hero body — a stray
/// keyed speck no longer inflates the bbox — unless `all_parts` opts into
/// keeping every component at or above `min_area`. A pure seam for testing the
/// Land wiring.
#[must_use]
pub(crate) fn land_component_mode(remove_on_land: bool, all_parts: bool, min_area: u32) -> ComponentMode {
    if !remove_on_land {
        ComponentMode::WholeAlpha
    } else if all_parts {
        ComponentMode::All { min_area }
    } else {
        ComponentMode::Largest
    }
}

/// Builds the first-frame generation prompt: the user's text plus a request to
/// place the sprite on a flat key-colour background. The colour is the studio's
/// chroma key, so the deferred background-removal step has a known colour to
/// strip.
fn key_color_prompt(prompt: &str, key: Rgba) -> String {
    let bg = format!("isolated on a solid #{:02x}{:02x}{:02x} background", key.r, key.g, key.b);
    let base = prompt.trim();
    if base.is_empty() { bg } else { format!("{base}, {bg}") }
}

/// Renders a spinner+message for a running job, or an error line for a failure.
fn studio_status_line(ui: &mut egui::Ui, status: &JobStatus) {
    match status {
        JobStatus::Running(m) => {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(m.clone());
            });
        }
        JobStatus::Failed(e) => {
            ui.colored_label(ui.visuals().error_fg_color, format!("failed: {e}"));
        }
        JobStatus::Idle => {}
    }
}

/// A short lineage caption for a first-frame candidate card.
fn ff_lineage_label(cand: &FirstFrameCandidate) -> egui::RichText {
    let text = match (cand.origin, cand.parent) {
        (FfOrigin::Inpaint, Some(p)) => format!("inpaint of #{}", p + 1),
        (FfOrigin::Inpaint, None) => "inpaint".to_owned(),
        (FfOrigin::Fresh, _) => "seed".to_owned(),
    };
    egui::RichText::new(text).small().weak()
}

/// A NEAREST-sampled egui texture from PNG bytes, or `None` on a decode failure.
fn load_png_texture(ctx: &egui::Context, name: &str, png: &[u8]) -> Option<egui::TextureHandle> {
    let rgba = image::load_from_memory(png).ok()?.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    let color = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
    Some(ctx.load_texture(name, color, egui::TextureOptions::NEAREST))
}

/// Encodes a [`MaskOverlay`] to a PNG for the inpaint call: white where the mask
/// is set (repaint), black elsewhere (keep), fully opaque. Returns `None` if the
/// mask is malformed.
#[must_use]
pub(crate) fn mask_overlay_png(mask: &MaskOverlay) -> Option<Vec<u8>> {
    use std::io::Cursor;
    if mask.cells.len() != (mask.width as usize) * (mask.height as usize) || mask.width == 0 || mask.height == 0 {
        return None;
    }
    let mut img = image::RgbaImage::new(mask.width, mask.height);
    for (pixel, &set) in img.pixels_mut().zip(mask.cells.iter()) {
        let v = if set { 255 } else { 0 };
        *pixel = image::Rgba([v, v, v, 255]);
    }
    let mut out = Vec::new();
    image::DynamicImage::ImageRgba8(img)
        .write_to(&mut Cursor::new(&mut out), image::ImageFormat::Png)
        .ok()?;
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_color_prompt_appends_the_hex_background() {
        let p = key_color_prompt("a knight", Rgba::opaque(255, 0, 255));
        assert_eq!(p, "a knight, isolated on a solid #ff00ff background");
    }

    #[test]
    fn anim_kind_maps_to_animation_kind_except_custom() {
        assert_eq!(AnimKind::Idle.to_animation_kind(), Some(AnimationKind::Idle));
        assert_eq!(AnimKind::Walk.to_animation_kind(), Some(AnimationKind::Walk));
        assert_eq!(AnimKind::Attack.to_animation_kind(), Some(AnimationKind::Attack));
        // Custom has no cascade kind, so Land skips its edge.
        assert_eq!(AnimKind::Custom.to_animation_kind(), None);
    }

    #[test]
    fn facing_maps_to_anchor_direction() {
        assert_eq!(Facing::South.to_anchor_direction(), AnchorDirection::South);
        assert_eq!(Facing::North.to_anchor_direction(), AnchorDirection::North);
        assert_eq!(Facing::East.to_anchor_direction(), AnchorDirection::East);
        assert_eq!(Facing::West.to_anchor_direction(), AnchorDirection::West);
    }

    #[test]
    fn key_color_prompt_handles_an_empty_prompt() {
        let p = key_color_prompt("   ", Rgba::opaque(0, 128, 64));
        assert_eq!(p, "isolated on a solid #008040 background");
    }

    /// `start_gen` always conditions on the anchor (it early-returns when the
    /// reference is empty) yet its seed-pose prompt carries no lock of its own.
    /// The Generate arm must add the Pose lock exactly once, so the most
    /// user-visible conditioned generation does not drift identity or scale. This
    /// guards the seam the isolated prompt tests miss, mirroring the cascade test.
    #[test]
    fn start_gen_seed_pose_prompt_locks_the_pose_once() {
        let lock_count = |haystack: &str, needle: &str| haystack.matches(needle).count();

        for facing in [Facing::South, Facing::North, Facing::West, Facing::East] {
            // The exact prompt `start_gen` builds for an untouched scaffold.
            let seed = key_color_prompt(&StudioState::pose_prompt_for(facing), Rgba::opaque(255, 0, 255));
            // `start_gen` always has a non-empty anchor, so the Generate arm conditions.
            let final_prompt = crate::ai::lock_when_conditioned(crate::ai::generate_frame_prompt(&seed), true);

            assert_eq!(
                lock_count(&final_prompt, "keep the same character from the reference image"),
                1,
                "the seed-pose prompt locks identity exactly once for {facing:?}: {final_prompt}"
            );
            assert_eq!(
                lock_count(&final_prompt, "change only the pose"),
                1,
                "the seed-pose prompt frees the pose for {facing:?}: {final_prompt}"
            );
            assert!(
                !final_prompt.contains("change only the facing direction"),
                "the seed-pose prompt must not free facing for {facing:?}: {final_prompt}"
            );
        }
    }

    #[test]
    fn gizmo_rasterizes_and_contains() {
        let giz = BoxGizmo {
            cx: 4.0,
            cy: 4.0,
            hw: 2.0,
            hh: 2.0,
            angle: 0.0,
        };
        assert!(giz.contains(4.0, 4.0));
        assert!(!giz.contains(0.5, 0.5));
        let mut mask = MaskOverlay::new(8, 8);
        rasterize_gizmo(&giz, &mut mask);
        assert!(!mask.is_empty());
        assert!(mask.cells[4 * 8 + 4]);
        assert!(!mask.cells[0]);
        assert!(mask.dirty.is_some());
        assert!(mask.box_bbox.is_some());
    }

    #[test]
    fn stamp_marks_only_its_disc_bbox() {
        let mut mask = MaskOverlay::new(64, 64);
        mask.dirty = None; // simulate a state already uploaded
        mask.stamp(32.0, 32.0, 3.0);
        let [x, y, w, h] = mask.dirty.expect("stamp dirties");
        // The dirty rect is the small disc bbox, not the whole 64x64 mask.
        assert!(x >= 28 && y >= 28);
        assert!(w <= 9 && h <= 9);
        // The stamped centre is inside the rect.
        assert!(x <= 32 && 32 < x + w && y <= 32 && 32 < y + h);
    }

    #[test]
    fn box_rasterize_clears_the_trail_when_it_moves() {
        let mut mask = MaskOverlay::new(64, 64);
        let a = BoxGizmo {
            cx: 16.0,
            cy: 16.0,
            hw: 6.0,
            hh: 6.0,
            angle: 0.0,
        };
        rasterize_gizmo(&a, &mut mask);
        assert!(mask.cells[16 * 64 + 16]);
        // Move the box away; the old cells must clear (no trail).
        mask.dirty = None;
        let b = BoxGizmo {
            cx: 48.0,
            cy: 48.0,
            hw: 6.0,
            hh: 6.0,
            angle: 0.0,
        };
        rasterize_gizmo(&b, &mut mask);
        assert!(!mask.cells[16 * 64 + 16], "vacated cells should clear");
        assert!(mask.cells[48 * 64 + 48]);
        // The re-uploaded region spans both the old and new boxes.
        let [x, y, w, h] = mask.dirty.expect("rasterize dirties");
        assert!(x <= 16 && y <= 16 && x + w >= 48 && y + h >= 48);
    }

    #[test]
    fn clear_empties_and_dirties_the_whole_mask() {
        let mut mask = MaskOverlay::new(8, 8);
        mask.stamp(4.0, 4.0, 2.0);
        mask.dirty = None;
        mask.clear();
        assert!(mask.is_empty());
        assert_eq!(mask.dirty, Some([0, 0, 8, 8]));
        assert!(mask.box_bbox.is_none());
    }

    #[test]
    fn gen_thread_defaults_are_sane() {
        let thread = GenThread::default();
        assert_eq!(thread.variants, 2);
        assert!((thread.view.brush - 4.0).abs() < f32::EPSILON);
        assert!(thread.candidates.is_empty());
        assert!(thread.selected.is_none());
    }

    #[test]
    fn image_view_rect_maps_round_trip_under_zoom_and_pan() {
        let canvas = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(200.0, 200.0));
        let img = egui::vec2(64.0, 64.0);
        let rect = image_view_rect(canvas, img, 2.0, egui::vec2(10.0, -5.0));
        // A screen point maps to image coords and back to the same screen point.
        let screen = egui::pos2(120.0, 90.0);
        let (ix, iy) = view_to_image(screen, rect, 64, 64);
        let back = view_to_screen(ix, iy, rect, 64, 64);
        assert!((back.x - screen.x).abs() < 0.01);
        assert!((back.y - screen.y).abs() < 0.01);
        // Zoom widens the drawn rect beyond the fit size.
        let fit = image_view_rect(canvas, img, 1.0, egui::Vec2::ZERO);
        assert!(rect.width() > fit.width());
    }

    #[test]
    fn mask_stamp_marks_a_disc_and_reports_non_empty() {
        let mut mask = MaskOverlay::new(8, 8);
        assert!(mask.is_empty());
        mask.stamp(4.0, 4.0, 2.0);
        assert!(!mask.is_empty());
        // The centre cell is set; a far corner is not.
        assert!(mask.cells[4 * 8 + 4]);
        assert!(!mask.cells[0]);
    }

    #[test]
    fn mask_clear_resets_every_cell() {
        let mut mask = MaskOverlay::new(4, 4);
        mask.stamp(2.0, 2.0, 4.0);
        mask.clear();
        assert!(mask.is_empty());
    }

    #[test]
    fn mask_overlay_png_round_trips_dimensions_and_is_white_where_set() {
        let mut mask = MaskOverlay::new(4, 4);
        mask.stamp(1.0, 1.0, 1.0);
        let png = mask_overlay_png(&mask).expect("encode mask");
        let decoded = image::load_from_memory(&png).expect("decode mask").to_rgba8();
        assert_eq!((decoded.width(), decoded.height()), (4, 4));
        // A set cell is white; an unset corner is black; both fully opaque.
        let set = decoded.get_pixel(1, 1);
        assert_eq!(set.0, [255, 255, 255, 255]);
        let unset = decoded.get_pixel(3, 3);
        assert_eq!(unset.0, [0, 0, 0, 255]);
    }

    #[test]
    fn i2v_model_ids_map_to_fal_endpoints() {
        assert_eq!(I2vModel::Seedance.model_id().as_str(), FAL_SEEDANCE);
        assert_eq!(I2vModel::Wan.model_id().as_str(), FAL_I2V);
    }

    #[test]
    fn motion_scaffold_combines_kind_and_facing() {
        let walk = StudioState {
            kind: AnimKind::Walk,
            facing: Facing::East,
            ..Default::default()
        };
        assert!(walk.motion_scaffold().starts_with("walk cycle"));
        let custom = StudioState {
            kind: AnimKind::Custom,
            facing: Facing::East,
            ..Default::default()
        };
        // Custom contributes no kind fragment, just the view.
        assert!(!custom.motion_scaffold().contains("cycle"));
    }

    #[test]
    fn scaffold_helpers_agree_with_current_framing_methods() {
        // The First-frame and Motion dials keep their prompt in step only while
        // it equals the scaffold for the *current* framing. That detection is
        // sound only if the instance methods produce exactly what the per-value
        // helpers do — so pin the two formulations together across every kind
        // and facing. Break one formula without the other and this fails.
        for facing in Facing::ALL {
            for kind in AnimKind::ALL {
                let state = StudioState {
                    kind,
                    facing,
                    ..Default::default()
                };
                assert_eq!(state.default_pose_prompt(), StudioState::pose_prompt_for(facing), "pose prompt for {facing:?}");
                assert_eq!(
                    state.motion_scaffold(),
                    StudioState::motion_prompt_for(kind, facing),
                    "motion prompt for {kind:?} / {facing:?}"
                );
            }
        }
    }

    /// A 2x2 frame with a distinct colour in each quadrant.
    fn quad_frame() -> VideoFrame {
        VideoFrame {
            pixels: vec![
                10, 20, 30, 255, // (0,0)
                40, 50, 60, 255, // (1,0)
                70, 80, 90, 255, // (0,1)
                100, 110, 120, 255, // (1,1)
            ],
            width: 2,
            height: 2,
            timestamp_ms: 0,
        }
    }

    #[test]
    fn frame_texel_rgba_maps_corners_to_quadrants() {
        let f = quad_frame();
        assert_eq!(frame_texel_rgba(&f, 0.0, 0.0), Some(Rgba::new(10, 20, 30, 255)));
        assert_eq!(frame_texel_rgba(&f, 0.99, 0.0), Some(Rgba::new(40, 50, 60, 255)));
        assert_eq!(frame_texel_rgba(&f, 0.0, 0.99), Some(Rgba::new(70, 80, 90, 255)));
        assert_eq!(frame_texel_rgba(&f, 0.99, 0.99), Some(Rgba::new(100, 110, 120, 255)));
        // Out-of-range coords clamp into the frame rather than reading past it.
        assert_eq!(frame_texel_rgba(&f, 5.0, 5.0), Some(Rgba::new(100, 110, 120, 255)));
    }

    #[test]
    fn frame_texel_rgba_rejects_empty_frame() {
        let empty = VideoFrame {
            pixels: Vec::new(),
            width: 0,
            height: 0,
            timestamp_ms: 0,
        };
        assert_eq!(frame_texel_rgba(&empty, 0.5, 0.5), None);
    }

    #[test]
    fn format_clip_time_handles_sub_minute_and_over_minute() {
        // 25th frame at 10 fps is 2s in; a 60-frame clip's last frame (59) is 5s.
        assert_eq!(format_clip_time(25, 10, 60), "0:02 / 0:05");
        // Over a minute: 700 frames at 10 fps is 70s.
        assert_eq!(format_clip_time(700, 10, 800), "1:10 / 1:19");
        // A zero fps is treated as 1 fps, not a divide-by-zero.
        assert_eq!(format_clip_time(3, 0, 4), "0:03 / 0:03");
    }

    #[test]
    fn land_chroma_is_some_only_when_removing() {
        let color = Rgba::new(255, 0, 255, 255);
        let keyed = land_chroma(true, color, 24).expect("keyed");
        assert_eq!(keyed.color, color);
        assert_eq!(keyed.tolerance, 24);
        assert!(land_chroma(false, color, 24).is_none());
    }

    #[test]
    fn land_component_mode_isolates_only_when_keying() {
        // No keying on Land: WholeAlpha, regardless of the part toggles — there
        // are no detached keying specks to isolate.
        assert_eq!(land_component_mode(false, false, 6), ComponentMode::WholeAlpha);
        assert_eq!(land_component_mode(false, true, 6), ComponentMode::WholeAlpha);
        // Keying on Land, largest-only: drop stray specks, keep the hero body.
        assert_eq!(land_component_mode(true, false, 6), ComponentMode::Largest);
        // Keying on Land, all parts: keep every component at or above min_area.
        assert_eq!(land_component_mode(true, true, 8), ComponentMode::All { min_area: 8 });
    }

    /// A session with every field set away from its default, to prove a
    /// round-trip carries the real values and not the defaults.
    fn sample_session() -> StudioSession {
        StudioSession {
            stage: StudioStage::Pick,
            kind: AnimKind::Attack,
            facing: Facing::North,
            i2v_model: I2vModel::Wan,
            gen_mode: GenMode::Static,
            grid_rows: 3,
            grid_cols: 4,
            approved_first_frame: Some(vec![1, 2, 3, 4, 5]),
            remove_on_land: true,
            motion: "attack swing, back view".to_owned(),
            target_frames: 12,
            fps: 24,
            seed: Some(99),
        }
    }

    #[test]
    fn studio_session_round_trips_through_serde() {
        let session = sample_session();
        let bytes = serde_json::to_vec(&session).expect("serialize");
        let back: StudioSession = serde_json::from_slice(&bytes).expect("deserialize");
        assert_eq!(back.stage, session.stage);
        assert_eq!(back.kind, session.kind);
        assert_eq!(back.facing, session.facing);
        assert_eq!(back.i2v_model, session.i2v_model);
        assert_eq!(back.gen_mode, session.gen_mode);
        assert_eq!(back.grid_rows, session.grid_rows);
        assert_eq!(back.grid_cols, session.grid_cols);
        assert_eq!(back.approved_first_frame, session.approved_first_frame);
        assert_eq!(back.remove_on_land, session.remove_on_land);
        assert_eq!(back.motion, session.motion);
        assert_eq!(back.target_frames, session.target_frames);
        assert_eq!(back.fps, session.fps);
        assert_eq!(back.seed, session.seed);
    }

    #[test]
    fn studio_session_enums_serialize_snake_case() {
        // The framing enums must serialize snake_case so a sidecar is stable and
        // forward-compatible across renames.
        assert_eq!(serde_json::to_string(&StudioStage::FirstFrame).unwrap(), "\"first_frame\"");
        assert_eq!(serde_json::to_string(&AnimKind::Walk).unwrap(), "\"walk\"");
        assert_eq!(serde_json::to_string(&Facing::South).unwrap(), "\"south\"");
        assert_eq!(serde_json::to_string(&I2vModel::Seedance).unwrap(), "\"seedance\"");
        assert_eq!(serde_json::to_string(&GenMode::Animated).unwrap(), "\"animated\"");
        assert_eq!(serde_json::to_string(&GenMode::Static).unwrap(), "\"static\"");
    }

    #[test]
    fn gen_mode_static_routes_to_static_sheet() {
        // The Generate dispatch routes each mode to a single producer; static
        // mode picks the sheet producer, animated stays on the clip producer.
        assert_eq!(GenMode::Static.producer(), GenProducer::StaticSheet);
        assert_eq!(GenMode::Animated.producer(), GenProducer::Clip);
    }

    #[test]
    fn missing_sidecar_fields_fall_back_to_defaults() {
        // An older sidecar written before a field existed still deserializes,
        // filling the gaps with the documented defaults.
        let json = r#"{ "motion": "idle breathing" }"#;
        let session: StudioSession = serde_json::from_str(json).expect("deserialize sparse");
        assert_eq!(session.stage, StudioStage::Anchor);
        assert_eq!(session.kind, AnimKind::Walk);
        assert_eq!(session.facing, Facing::East);
        assert_eq!(session.i2v_model, I2vModel::Seedance);
        assert_eq!(session.gen_mode, GenMode::Animated);
        assert_eq!(session.grid_rows, 2);
        assert_eq!(session.grid_cols, 3);
        assert_eq!(session.target_frames, 6);
        assert_eq!(session.fps, 10);
        assert_eq!(session.motion, "idle breathing");
        assert!(session.approved_first_frame.is_none());
        assert!(session.seed.is_none());
        assert!(!session.remove_on_land);
    }

    #[test]
    fn two_sprite_sessions_survive_a_round_trip_through_the_map() {
        // Mirror the flush-then-hydrate switch: sprite A's session is flushed to
        // the map, sprite B's is hydrated, then A is read back unchanged.
        let mut map: HashMap<SpriteId, StudioSession> = HashMap::new();
        let a = SpriteId::new(1);
        let b = SpriteId::new(2);

        let session_a = StudioSession {
            stage: StudioStage::Motion,
            facing: Facing::West,
            motion: "walk cycle, side view".to_owned(),
            approved_first_frame: Some(vec![7, 7, 7]),
            ..StudioSession::default()
        };
        let session_b = StudioSession {
            stage: StudioStage::FirstFrame,
            facing: Facing::North,
            motion: "idle, back view".to_owned(),
            ..StudioSession::default()
        };

        // Working on A, then switching to B (B flushed in turn) must not disturb A.
        map.insert(a, session_a.clone());
        map.insert(b, session_b.clone());

        let back_a = map.get(&a).expect("sprite A session retained");
        assert_eq!(back_a.stage, StudioStage::Motion);
        assert_eq!(back_a.facing, Facing::West);
        assert_eq!(back_a.motion, "walk cycle, side view");
        assert_eq!(back_a.approved_first_frame.as_deref(), Some(&[7, 7, 7][..]));

        // And the whole map survives the sidecar serde round-trip.
        let bytes = encode_sessions(&map).expect("encode");
        let restored = decode_sessions(&bytes).expect("decode");
        assert_eq!(restored.len(), 2);
        assert_eq!(restored.get(&a).unwrap().motion, session_a.motion);
        assert_eq!(restored.get(&b).unwrap().facing, session_b.facing);
        assert_eq!(restored.get(&a).unwrap().approved_first_frame, session_a.approved_first_frame);
    }

    #[test]
    fn empty_and_missing_sidecar_decode_to_empty() {
        // An empty JSON object (no sessions key) decodes to an empty map.
        let map = decode_sessions(b"{}").expect("decode empty object");
        assert!(map.is_empty());
        // A round-trip of an empty map is also empty.
        let bytes = encode_sessions(&HashMap::new()).expect("encode empty");
        assert!(decode_sessions(&bytes).expect("decode").is_empty());
    }

    // ── Normalize-review classification ──────────────────────────────────────

    use pixhaus_core::transforms::normalize::{NormalizeOptions, normalize_frames};

    #[test]
    fn drift_status_is_ok_only_at_zero() {
        assert_eq!(drift_status(0), ReportStatus::Ok);
        assert_eq!(drift_status(1), ReportStatus::Warning);
        assert_eq!(drift_status(40), ReportStatus::Warning);
    }

    #[test]
    fn scale_status_bands_by_match_percent() {
        assert_eq!(scale_status(100), ReportStatus::Ok);
        assert_eq!(scale_status(90), ReportStatus::Ok);
        assert_eq!(scale_status(89), ReportStatus::Warning);
        assert_eq!(scale_status(60), ReportStatus::Warning);
        assert_eq!(scale_status(59), ReportStatus::Error);
        assert_eq!(scale_status(0), ReportStatus::Error);
    }

    #[test]
    fn seam_status_maps_each_verdict() {
        assert_eq!(seam_status(SeamMatch::Ok), ReportStatus::Ok);
        assert_eq!(seam_status(SeamMatch::Close), ReportStatus::Warning);
        assert_eq!(seam_status(SeamMatch::Drift), ReportStatus::Error);
    }

    #[test]
    fn edge_status_is_ok_only_at_zero() {
        assert_eq!(edge_status(0), ReportStatus::Ok);
        assert_eq!(edge_status(1), ReportStatus::Error);
        assert_eq!(edge_status(5), ReportStatus::Error);
    }

    #[test]
    fn edge_status_color_is_error() {
        // A clipped frame reads in-game, so the inspector paints it the error
        // colour — not the softer warning the drift field uses.
        let palette = crate::theme::Palette::for_theme(egui::Theme::Dark);
        assert_eq!(edge_status(1).color(&palette), palette.error);
    }

    #[test]
    fn components_status_is_ok_for_a_single_body() {
        // Zero (an all-empty loop) and one (a clean body) both read Ok; a
        // fragmented loop reads Warning — advisory, not a blocking error.
        assert_eq!(components_status(0), ReportStatus::Ok);
        assert_eq!(components_status(1), ReportStatus::Ok);
        assert_eq!(components_status(2), ReportStatus::Warning);
        assert_eq!(components_status(7), ReportStatus::Warning);
    }

    #[test]
    fn report_status_colors_match_the_palette() {
        // The inspector reuses the studio palette: Ok→success, Warning→warning,
        // Error→error. Pinning the mapping guards the colour coding the plan asks
        // the seam verdict to share with the pick stage.
        let palette = crate::theme::Palette::for_theme(egui::Theme::Dark);
        assert_eq!(ReportStatus::Ok.color(&palette), palette.success);
        assert_eq!(ReportStatus::Warning.color(&palette), palette.warning);
        assert_eq!(ReportStatus::Error.color(&palette), palette.error);
    }

    /// A 16×16 magenta frame with a `color` subject rectangle at `(x, y)`
    /// spanning `w × h`. The magenta keys out; the rectangle is the measured
    /// subject. The colour drives the loop-seam diff after normalization.
    fn synthetic_frame(x: u32, y: u32, w: u32, h: u32, color: Rgba) -> PixelBuffer {
        let mut f = PixelBuffer::filled(16, 16, Rgba::opaque(255, 0, 255)).expect("frame");
        for sy in y..(y + h).min(16) {
            for sx in x..(x + w).min(16) {
                f.set_pixel(sx, sy, color);
            }
        }
        f
    }

    #[test]
    fn three_frame_sequence_reports_clean_baseline_as_ok() {
        // Three subjects of equal height land on a locked baseline, so the
        // drift field reads Ok and the scale field reads a perfect match. A 1px
        // bottom margin keeps the feet off the very last row, so the by-design
        // full-bleed-bottom edge touch does not fire on this otherwise clean
        // loop — the Edge-clear and Scale-parity rows both read Ok.
        let black = Rgba::opaque(0, 0, 0);
        let frames = [
            synthetic_frame(6, 8, 4, 6, black),
            synthetic_frame(5, 8, 6, 6, black),
            synthetic_frame(6, 8, 4, 6, black),
        ];
        let opts = NormalizeOptions {
            bottom_margin: 1,
            safe_margin: 1,
            ..NormalizeOptions::square(16)
        };
        let report = normalize_frames(&frames, &opts).expect("normalize").report;
        assert_eq!(drift_status(report.baseline_drift_px), ReportStatus::Ok);
        assert_eq!(scale_status(report.scale_match_pct), ReportStatus::Ok);
        assert_eq!(
            edge_status(report.edge_touch_frames.len()),
            ReportStatus::Ok,
            "small centred subjects land clear of every edge"
        );
        assert_eq!(
            scale_status(report.scale_parity_pct),
            ReportStatus::Ok,
            "the corrected heights agree, so parity reads Ok"
        );
    }

    #[test]
    fn three_frame_sequence_with_drifting_seam_renders_error_category() {
        // The last frame's subject is a different colour from the first's, and
        // each subject fills the canvas so its colour dominates the per-pixel
        // mean. After normalization both occupy the same locked cell, so their
        // per-channel diff is large and the loop seam reads Drift — the error
        // category the inspector renders with the error colour.
        let first = synthetic_frame(0, 0, 16, 16, Rgba::opaque(0, 0, 0));
        let mid = synthetic_frame(0, 0, 16, 16, Rgba::opaque(128, 128, 128));
        let last = synthetic_frame(0, 0, 16, 16, Rgba::opaque(255, 255, 255));
        let report = normalize_frames(&[first, mid, last], &NormalizeOptions::square(16)).expect("normalize").report;
        assert_eq!(report.seam, SeamMatch::Drift, "first and last differ enough to drift");
        assert_eq!(seam_status(report.seam), ReportStatus::Error);
        let palette = crate::theme::Palette::for_theme(egui::Theme::Dark);
        assert_eq!(seam_status(report.seam).color(&palette), palette.error);
        // The pass records the drift in the warnings list the inspector shows.
        assert!(
            report.warnings.iter().any(|w| w.contains("loop seam")),
            "seam warning surfaced: {:?}",
            report.warnings
        );
    }

    #[test]
    fn edge_touching_frame_reads_error_in_the_inspector() {
        // A tall narrow frame sets a high shared reference height; a short wide
        // frame scaled up to it balloons past the canvas and repad clips the
        // overhang, so its landed bbox spans edge to edge. The inspector's
        // Edge-clear row must read Error and the frame index must appear in
        // edge_touch_frames so the strip badge shows.
        let black = Rgba::opaque(0, 0, 0);
        let tall = synthetic_frame(7, 1, 2, 14, black);
        let wide = synthetic_frame(1, 7, 14, 2, black);
        let report = normalize_frames(&[tall, wide], &NormalizeOptions::square(16)).expect("normalize").report;
        assert!(
            report.edge_touch_frames.contains(&1),
            "the over-scaled wide frame is flagged: {:?}",
            report.edge_touch_frames
        );
        assert_eq!(
            edge_status(report.edge_touch_frames.len()),
            ReportStatus::Error,
            "the inspector reads Error for a clipped frame"
        );
        let palette = crate::theme::Palette::for_theme(egui::Theme::Dark);
        assert_eq!(edge_status(report.edge_touch_frames.len()).color(&palette), palette.error);
    }
}
