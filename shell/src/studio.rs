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

use eframe::egui;
use tokio_util::sync::CancellationToken;

use pixhaus_ai::backends::fal::{FAL_I2V, FAL_SEEDANCE};
use pixhaus_core::canvas::PixelBuffer;
use pixhaus_core::project::{Rgba, SpriteId};
use pixhaus_core::transforms::normalize::{ChromaKey, chroma_key, detect_key_color};

use crate::ai::{self, FirstFrameJob};
use crate::anim::{self, VideoFrame};
use crate::app::{AnimPlayMode, JobStatus, ShellApp, Workspace};
use crate::document::SpriteRef;
use crate::gizmo::{BoxGizmo, GizmoHandle};

/// The studio's ordered stages. Navigation is free — the rail lets you step back
/// to an earlier stage without losing later work — so this is a position in a
/// workspace, not a forced march.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum StudioStage {
    /// Pick the approved reference sheet plus direction and animation kind.
    Anchor,
    /// Generate and refine the seed pose (text-to-image + inpaint).
    FirstFrame,
    /// Choreography prompt, model pick, fps, frame count, seed.
    Motion,
    /// Scrub the raw clip and drag the loop markers.
    Clip,
    /// Toggle the evenly-spaced picks and read the seam score.
    Pick,
    /// Normalize and land the picks on the timeline.
    Land,
}

impl StudioStage {
    /// The stages in pipeline order, for the rail.
    const ALL: [StudioStage; 6] = [
        StudioStage::Anchor,
        StudioStage::FirstFrame,
        StudioStage::Motion,
        StudioStage::Clip,
        StudioStage::Pick,
        StudioStage::Land,
    ];

    /// Rail label for the stage.
    fn label(self) -> &'static str {
        match self {
            StudioStage::Anchor => "Anchor",
            StudioStage::FirstFrame => "First frame",
            StudioStage::Motion => "Motion",
            StudioStage::Clip => "Clip & loop",
            StudioStage::Pick => "Frame pick",
            StudioStage::Land => "Land",
        }
    }
}

/// The animation kind, which scaffolds the motion prompt.
#[derive(Clone, Copy, PartialEq, Eq)]
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
}

/// The facing direction the animation conditions on.
#[derive(Clone, Copy, PartialEq, Eq)]
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
}

/// The image-to-video model the Motion stage drives.
#[derive(Clone, Copy, PartialEq, Eq)]
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

/// Breadcrumb for a hand-edit round trip: the sprite to re-select on return and
/// the temporary edit sprite to drop. The edited image lands in the first-frame
/// thread.
pub(crate) struct StudioReturn {
    /// The sprite that was active before hand-editing, restored on return.
    pub origin: Option<SpriteRef>,
    /// The scratch sprite created for the edit, deleted on return.
    pub edit: SpriteRef,
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
    /// Whether the second clip is shown beside the first for comparison.
    pub compare: bool,
    /// The other clip index in a side-by-side comparison.
    pub compare_other: Option<usize>,
    /// Set once Land integrated a loop, so Land can confirm the result.
    pub landed: bool,
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
    /// The sprite this session belongs to. When the active sprite changes the
    /// session resets so one sprite's candidates never leak into another's.
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
            compare: false,
            compare_other: None,
            landed: false,
            drag_handle: None,
            loop_playback: true,
            picking_key: false,
            keyed_preview: false,
            remove_on_land: false,
            owner: None,
        }
    }
}

impl StudioState {
    /// The default seed-pose prompt built from the kind and facing.
    fn default_pose_prompt(&self) -> String {
        format!("character seed pose, {}", self.facing.view_fragment())
    }

    /// The motion prompt scaffolded from the kind and facing.
    fn motion_scaffold(&self) -> String {
        match self.kind.motion_fragment() {
            Some(kind) => format!("{kind}, {}", self.facing.view_fragment()),
            None => self.facing.view_fragment().to_owned(),
        }
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
        self.studio.stage = StudioStage::Anchor;
        self.studio.landed = false;
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
            // The composition-library overlay: the browser fills the center, the
            // record editor the right dock, over the rest of the studio.
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

    /// Resets the studio session when the active sprite changes, so one sprite's
    /// candidates, masks, and clips never appear under another. Cancels any
    /// in-flight first-frame job so its late results are dropped.
    fn sync_studio_owner(&mut self) {
        let active = self.doc.active_sprite().map(|s| s.id);
        if self.studio.owner == active {
            return;
        }
        if let Some(token) = self.studio.frame_gen.cancel.take() {
            token.cancel();
        }
        self.leave_animation_preview();
        self.anim_candidates.clear();
        self.anim_selected = None;
        self.studio = StudioState::default();
        self.studio.owner = active;
        if self.studio.frame_gen.prompt.trim().is_empty() {
            self.studio.frame_gen.prompt = self.studio.default_pose_prompt();
        }
    }

    /// The stage that must finish before the current one is usable, if it is not
    /// done yet. `None` once the current stage's prerequisite is satisfied (and
    /// always for the Anchor stage, which has none).
    fn studio_unmet_prereq(&self) -> Option<StudioStage> {
        let idx = StudioStage::ALL.iter().position(|s| *s == self.studio.stage)?;
        let prev = StudioStage::ALL.get(idx.checked_sub(1)?)?;
        if self.studio_stage_complete(*prev) { None } else { Some(*prev) }
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
            StudioStage::Clip => self.anim_card().is_some(),
            StudioStage::Pick => self.anim_card().is_some_and(|c| !c.picks.is_empty()),
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
            StudioStage::Clip => self.studio_clip_surface(ui),
            StudioStage::Pick => self.studio_pick_surface(ui),
            StudioStage::Land => self.studio_land_surface(ui),
        }
    }

    /// The right inspector for the current stage.
    fn studio_inspector(&mut self, ui: &mut egui::Ui) {
        if self.studio_unmet_prereq().is_some() {
            ui.label(egui::RichText::new("Finish the earlier stage to unlock this one.").weak());
            return;
        }
        if !self.backend_ready && !matches!(self.studio.stage, StudioStage::Clip | StudioStage::Pick) {
            self.key_entry(ui);
            ui.separator();
        }
        match self.studio.stage {
            StudioStage::Anchor => self.studio_anchor_inspector(ui),
            StudioStage::FirstFrame => self.studio_first_frame_inspector(ui),
            StudioStage::Motion => self.studio_motion_inspector(ui),
            StudioStage::Clip => self.studio_clip_inspector(ui),
            StudioStage::Pick => self.studio_pick_inspector(ui),
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

    /// The Anchor stage's inspector: the framing dials that scaffold the
    /// seed-pose and motion prompts, the cockpit composition controls, and the
    /// results gallery (cards) below.
    fn studio_anchor_inspector(&mut self, ui: &mut egui::Ui) {
        ui.label(egui::RichText::new("Animation kind").strong());
        ui.horizontal_wrapped(|ui| {
            for kind in AnimKind::ALL {
                ui.selectable_value(&mut self.studio.kind, kind, kind.label());
            }
        });
        ui.add_space(8.0);
        ui.label(egui::RichText::new("Direction").strong());
        ui.horizontal_wrapped(|ui| {
            for facing in Facing::ALL {
                ui.selectable_value(&mut self.studio.facing, facing, facing.label());
            }
        });
        ui.add_space(8.0);
        if ui
            .button("Apply framing to prompts")
            .on_hover_text("Scaffold the seed-pose and motion prompts from this framing")
            .clicked()
        {
            self.studio.frame_gen.prompt = self.studio.default_pose_prompt();
            self.anim_motion = self.studio.motion_scaffold();
        }
        ui.add_space(8.0);
        ui.separator();
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
        self.studio_return = Some(StudioReturn { origin, edit });
        self.studio.frame_gen.view.painting = false;
        self.set_workspace(Workspace::Draw);
        self.refresh_canvas(true);
    }

    /// Returns from a hand-edit: captures the edited pixels as a new candidate in
    /// the first-frame thread, restores the original sprite, drops the scratch
    /// sprite, and re-enters the studio at the first-frame stage.
    pub(crate) fn finish_hand_edit(&mut self) {
        let Some(ret) = self.studio_return.take() else {
            return;
        };
        let png = self.doc.composite_active_frame().and_then(|buf| pixel_buffer_to_png(&buf));
        if let Some(origin) = ret.origin {
            self.doc.select(origin);
        }
        self.doc.delete_sprite(ret.edit);
        if let Some(png) = png {
            let parent = self.gen_ref().selected;
            self.on_gen_ready(vec![png], parent, true);
        }
        self.workspace = Workspace::Create;
        self.studio.stage = StudioStage::FirstFrame;
        self.refresh_canvas(true);
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
        ui.horizontal(|ui| {
            ui.label("Model");
            ui.selectable_value(&mut self.studio.i2v_model, I2vModel::Seedance, I2vModel::Seedance.label());
            ui.selectable_value(&mut self.studio.i2v_model, I2vModel::Wan, I2vModel::Wan.label());
        });
        ui.add(egui::Slider::new(&mut self.anim_target_frames, 2..=16).text("loop frames"));
        ui.add(egui::Slider::new(&mut self.anim_fps, 4..=24).text("fps"));
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.anim_seed_fixed, "Fixed seed")
                .on_hover_text("Pin the RNG seed for a reproducible clip; off uses a random seed each run");
            ui.add_enabled(self.anim_seed_fixed, egui::DragValue::new(&mut self.anim_seed));
        });

        let busy = matches!(self.anim_status, JobStatus::Running(_));
        if ui
            .add_enabled(self.backend_ready && !busy, egui::Button::new(format!("{} Generate clip", crate::icons::FILM)))
            .clicked()
        {
            self.studio.landed = false;
            // A from-scratch clip has no lineage parent.
            self.clear_clip_lineage();
            self.start_clip();
            self.studio.stage = StudioStage::Clip;
        }
        if !self.backend_ready {
            ui.colored_label(ui.visuals().warn_fg_color, "No generation backend configured.");
        }
        studio_status_line(ui, &self.anim_status);
        if busy && ui.button("Cancel").clicked() {
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
                let key = ChromaKey {
                    color: self.bg_key_color,
                    tolerance: self.bg_tolerance,
                };
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
        if ui.button(format!("{} Next: land", crate::icons::RIGHT)).clicked() {
            self.studio.stage = StudioStage::Land;
        }
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

    /// Lands the picked frames and flags the session as landed.
    fn studio_land(&mut self) {
        self.integrate_picked();
        self.studio.landed = true;
    }
}

/// A centered, wrapped muted hint filling the surface — the empty-state message.
fn centered_hint(ui: &mut egui::Ui, text: &str) {
    ui.centered_and_justified(|ui| {
        ui.label(egui::RichText::new(text).weak());
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
    remove_on_land.then_some(ChromaKey { color, tolerance })
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
    fn key_color_prompt_handles_an_empty_prompt() {
        let p = key_color_prompt("   ", Rgba::opaque(0, 128, 64));
        assert_eq!(p, "isolated on a solid #008040 background");
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
}
