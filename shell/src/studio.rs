//! The Create-mode **animation studio**: a full-screen, focused workspace for
//! the whole generate-and-refine animation loop.
//!
//! The studio is a [`crate::cockpit::CreateView::Studio`] takeover. When active,
//! `app.rs` routes the central panel here and hides the tools rail, side dock,
//! and timeline, so the studio owns the whole canvas area. Inside, it lays out
//! three regions — a stages rail (left), a stage surface (center), and an
//! inspector (right) — under a header with a back-to-editor exit.
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

use crate::ai::{self, FirstFrameJob};
use crate::app::{AnimPlayMode, JobStatus, ShellApp, Workspace};

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

/// A paint-over mask aligned to the selected first-frame candidate. White cells
/// mark the region to repaint; exported to a PNG for the inpaint call.
pub(crate) struct MaskOverlay {
    /// Mask width in pixels (matches the candidate image).
    pub width: u32,
    /// Mask height in pixels.
    pub height: u32,
    /// One flag per pixel, row-major; `true` means "repaint here".
    pub cells: Vec<bool>,
    /// Set when [`Self::cells`] changed so the overlay texture is rebuilt.
    pub dirty: bool,
    /// Lazily-built overlay texture (red where set), drawn over the candidate.
    pub texture: Option<egui::TextureHandle>,
}

impl MaskOverlay {
    /// An empty mask sized to `width` x `height`.
    fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            cells: vec![false; (width as usize) * (height as usize)],
            dirty: true,
            texture: None,
        }
    }

    /// Whether no pixel is marked.
    fn is_empty(&self) -> bool {
        !self.cells.iter().any(|&c| c)
    }

    /// Clears every marked pixel.
    fn clear(&mut self) {
        self.cells.iter_mut().for_each(|c| *c = false);
        self.dirty = true;
    }

    /// Marks a filled disc of `radius` mask-pixels centred on `(cx, cy)` in
    /// mask-pixel coordinates.
    fn stamp(&mut self, cx: f32, cy: f32, radius: f32) {
        let r = radius.max(0.5);
        let (w, h) = (self.width as i32, self.height as i32);
        let r2 = r * r;
        let min_x = ((cx - r).floor() as i32).max(0);
        let max_x = ((cx + r).ceil() as i32).min(w - 1);
        let min_y = ((cy - r).floor() as i32).max(0);
        let max_y = ((cy + r).ceil() as i32).min(h - 1);
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let dx = x as f32 + 0.5 - cx;
                let dy = y as f32 + 0.5 - cy;
                if dx * dx + dy * dy <= r2 {
                    self.cells[(y as usize) * (self.width as usize) + (x as usize)] = true;
                }
            }
        }
        self.dirty = true;
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
    /// Seed-pose prompt for the first-frame text-to-image step.
    pub ff_prompt: String,
    /// Requested seed-pose candidate count (1-4).
    pub ff_variants: u32,
    /// The first-frame gallery, newest last.
    pub ff_candidates: Vec<FirstFrameCandidate>,
    /// Selected gallery index, the one shown large with the mask overlay.
    pub ff_selected: Option<usize>,
    /// The approved seed pose (PNG), the single input that drives the clip.
    pub approved_first_frame: Option<Vec<u8>>,
    /// First-frame job status.
    pub ff_status: JobStatus,
    /// Cancel handle for an in-flight first-frame generation.
    pub ff_cancel: Option<CancellationToken>,
    /// Monotonic first-frame generation id; bumped on each generate and cancel
    /// so a superseded run's late messages are dropped.
    pub ff_epoch: u64,
    /// Whether the mask paint tool is armed (drags paint instead of nothing).
    pub painting: bool,
    /// Inpaint mask aligned to the selected candidate.
    pub mask: Option<MaskOverlay>,
    /// Mask brush radius in mask pixels.
    pub brush: f32,
    /// Fix prompt for the inpaint refinement.
    pub inpaint_prompt: String,
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
            ff_prompt: String::new(),
            ff_variants: 2,
            ff_candidates: Vec::new(),
            ff_selected: None,
            approved_first_frame: None,
            ff_status: JobStatus::Idle,
            ff_cancel: None,
            ff_epoch: 0,
            painting: false,
            mask: None,
            brush: 4.0,
            inpaint_prompt: String::new(),
            i2v_model: I2vModel::Seedance,
            compare: false,
            compare_other: None,
            landed: false,
            drag_handle: None,
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
        self.workspace == Workspace::Create && self.create_view == crate::cockpit::CreateView::Studio
    }

    /// Enters the studio full-screen, seeding the first-frame prompt from the
    /// anchor framing when it is still blank.
    pub(crate) fn enter_studio(&mut self) {
        self.workspace = Workspace::Create;
        self.create_view = crate::cockpit::CreateView::Studio;
        self.studio.stage = StudioStage::Anchor;
        self.studio.landed = false;
        if self.studio.ff_prompt.trim().is_empty() {
            self.studio.ff_prompt = self.studio.default_pose_prompt();
        }
        if self.anim_motion.trim().is_empty() {
            self.anim_motion = self.studio.motion_scaffold();
        }
    }

    /// Leaves the studio for the normal drawing editor, dropping any in-progress
    /// preview. Studio session state is kept so re-entering resumes where it left.
    pub(crate) fn exit_studio(&mut self) {
        self.studio.painting = false;
        self.leave_animation_preview();
        self.create_view = crate::cockpit::CreateView::Cockpit;
        self.set_workspace(Workspace::Draw);
    }

    /// Lays out the studio's three regions under its header.
    pub(crate) fn studio_view(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("studio_header").resizable(false).show_inside(ui, |ui| self.studio_header(ui));
        egui::Panel::left("studio_rail")
            .resizable(false)
            .exact_size(190.0)
            .show_inside(ui, |ui| self.studio_rail(ui));
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

    /// The header: back-to-editor exit, the title, and the framing summary.
    fn studio_header(&mut self, ui: &mut egui::Ui) {
        let mut back = false;
        ui.horizontal(|ui| {
            if ui.button(format!("{} Back to editor", crate::icons::LEFT)).clicked() {
                back = true;
            }
            ui.separator();
            ui.heading(format!("{} Animation studio", crate::icons::FILM));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(egui::RichText::new(self.studio.framing_label()).weak());
            });
        });
        if back {
            self.exit_studio();
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

    /// Shows the approved anchor large, or guidance to approve one first.
    fn studio_anchor_surface(&mut self, ui: &mut egui::Ui) {
        let Some(anchor) = self.doc.active_anchor().map(<[u8]>::to_vec) else {
            centered_hint(
                ui,
                "Approve a result as an anchor in the Cockpit first — it conditions everything the studio generates.",
            );
            return;
        };
        let ctx = ui.ctx().clone();
        let tex = load_png_texture(&ctx, "studio_anchor", &anchor);
        if let Some(tex) = tex {
            fit_image(ui, &tex);
        } else {
            centered_hint(ui, "The anchor image could not be decoded.");
        }
    }

    /// Direction + animation-kind dials, which scaffold the motion prompt.
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
        ui.add_space(10.0);
        if ui
            .button("Apply to prompts")
            .on_hover_text("Scaffold the seed-pose and motion prompts from this framing")
            .clicked()
        {
            self.studio.ff_prompt = self.studio.default_pose_prompt();
            self.anim_motion = self.studio.motion_scaffold();
        }
        ui.add_space(10.0);
        if self.doc.active_anchor().is_some() {
            if ui.button(format!("{} Next: first frame", crate::icons::RIGHT)).clicked() {
                self.studio.stage = StudioStage::FirstFrame;
            }
        } else {
            ui.colored_label(ui.visuals().warn_fg_color, "No anchor approved yet.");
        }
    }

    // ── First-frame stage ─────────────────────────────────────────────────────

    /// The selected candidate large with the mask overlay, then the gallery strip.
    fn studio_first_frame_surface(&mut self, ui: &mut egui::Ui) {
        if self.studio.ff_candidates.is_empty() {
            centered_hint(ui, "Generate a seed pose from the anchor in the inspector. Candidates appear here.");
            return;
        }
        // Reserve the gallery strip at the bottom, then the rest for the big view.
        let strip_h = 96.0;
        let total = ui.available_size();
        let big_h = (total.y - strip_h - 12.0).max(80.0);
        ui.allocate_ui(egui::vec2(total.x, big_h), |ui| {
            if let Some(i) = self.studio.ff_selected {
                self.studio_mask_canvas(ui, i);
            } else {
                centered_hint(ui, "Select a candidate below to preview and refine it.");
            }
        });
        ui.separator();
        self.studio_ff_gallery(ui);
    }

    /// The horizontal first-frame candidate gallery; click to select.
    fn studio_ff_gallery(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        for cand in &mut self.studio.ff_candidates {
            if cand.texture.is_none() {
                cand.texture = load_png_texture(&ctx, "studio_ff", &cand.png);
            }
        }
        let mut select: Option<usize> = None;
        egui::ScrollArea::horizontal().id_salt("studio_ff_strip").show(ui, |ui| {
            ui.horizontal(|ui| {
                for (i, cand) in self.studio.ff_candidates.iter().enumerate() {
                    let selected = self.studio.ff_selected == Some(i);
                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        ui.vertical(|ui| {
                            if let Some(tex) = &cand.texture {
                                let size = tex.size_vec2();
                                let scale = (72.0 / size.x.max(1.0)).min(72.0 / size.y.max(1.0));
                                let img = egui::Image::new((tex.id(), size * scale)).sense(egui::Sense::click());
                                if ui.add(img).clicked() {
                                    select = Some(i);
                                }
                            }
                            if ui.selectable_label(selected, ff_lineage_label(cand)).clicked() {
                                select = Some(i);
                            }
                        });
                    });
                }
            });
        });
        if let Some(i) = select {
            self.select_first_frame(i);
        }
    }

    /// Draws the selected candidate with the mask overlay, and stamps the mask
    /// while the paint tool is armed and the pointer drags over it.
    fn studio_mask_canvas(&mut self, ui: &mut egui::Ui, i: usize) {
        let ctx = ui.ctx().clone();
        if self.studio.ff_candidates[i].texture.is_none() {
            self.studio.ff_candidates[i].texture = load_png_texture(&ctx, "studio_ff", &self.studio.ff_candidates[i].png);
        }
        let Some(tex) = self.studio.ff_candidates[i].texture.clone() else {
            centered_hint(ui, "This candidate could not be decoded.");
            return;
        };
        let img_size = tex.size_vec2();
        let (iw, ih) = (tex.size()[0] as u32, tex.size()[1] as u32);
        self.ensure_mask(iw, ih);

        // Fit the image into the available area, preserving aspect.
        let avail = ui.available_size();
        let scale = (avail.x / img_size.x.max(1.0)).min(avail.y / img_size.y.max(1.0)).max(0.01);
        let draw = img_size * scale;
        let (rect, resp) = ui.allocate_exact_size(draw, egui::Sense::click_and_drag());

        // Paint into the mask while armed and dragging over the image.
        if self.studio.painting {
            if let Some(pos) = resp.interact_pointer_pos() {
                if rect.contains(pos) {
                    let mx = (pos.x - rect.left()) / rect.width() * iw as f32;
                    let my = (pos.y - rect.top()) / rect.height() * ih as f32;
                    let brush = self.studio.brush;
                    if let Some(mask) = self.studio.mask.as_mut() {
                        mask.stamp(mx, my, brush);
                    }
                }
            }
        }

        // Rebuild the mask overlay texture if it changed.
        self.rebuild_mask_texture(&ctx);

        let painter = ui.painter_at(rect);
        let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
        painter.image(tex.id(), rect, uv, egui::Color32::WHITE);
        if let Some(mask) = &self.studio.mask {
            if let Some(mtex) = &mask.texture {
                painter.image(mtex.id(), rect, uv, egui::Color32::WHITE);
            }
        }
        painter.rect_stroke(
            rect,
            2.0,
            egui::Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color),
            egui::StrokeKind::Inside,
        );
    }

    /// Ensures a mask sized to `(w, h)` exists; replaces it when the dimensions
    /// changed (a different candidate was selected).
    fn ensure_mask(&mut self, w: u32, h: u32) {
        let needs = self.studio.mask.as_ref().is_none_or(|m| m.width != w || m.height != h);
        if needs {
            self.studio.mask = Some(MaskOverlay::new(w, h));
        }
    }

    /// Rebuilds the mask overlay texture from the cells when it is dirty.
    fn rebuild_mask_texture(&mut self, ctx: &egui::Context) {
        let Some(mask) = self.studio.mask.as_mut() else {
            return;
        };
        if !mask.dirty {
            return;
        }
        let size = [mask.width as usize, mask.height as usize];
        // Translucent red where set, transparent elsewhere; drawn over the frame.
        let mut bytes = Vec::with_capacity(mask.cells.len() * 4);
        for &set in &mask.cells {
            if set {
                bytes.extend_from_slice(&[220, 40, 40, 120]);
            } else {
                bytes.extend_from_slice(&[0, 0, 0, 0]);
            }
        }
        let image = egui::ColorImage::from_rgba_unmultiplied(size, &bytes);
        mask.texture = Some(ctx.load_texture("studio_mask", image, egui::TextureOptions::NEAREST));
        mask.dirty = false;
    }

    /// Selects first-frame candidate `i` and resets the mask to its dimensions.
    fn select_first_frame(&mut self, i: usize) {
        self.studio.ff_selected = Some(i);
        self.studio.mask = None;
        self.studio.painting = false;
    }

    /// The first-frame inspector: the seed-pose prompt and Generate, then the
    /// inpaint mask controls and the Approve action.
    fn studio_first_frame_inspector(&mut self, ui: &mut egui::Ui) {
        if self.doc.active_anchor().is_none() {
            ui.colored_label(ui.visuals().warn_fg_color, "Approve an anchor first (Anchor stage).");
            return;
        }
        ui.label(egui::RichText::new("Seed pose").strong());
        ui.add(
            egui::TextEdit::multiline(&mut self.studio.ff_prompt)
                .hint_text("a small knight standing, neutral pose")
                .desired_rows(2)
                .desired_width(f32::INFINITY),
        );
        ui.add(egui::Slider::new(&mut self.studio.ff_variants, 1..=4).text("variants"));

        let busy = matches!(self.studio.ff_status, JobStatus::Running(_));
        if ui
            .add_enabled(
                self.backend_ready && !busy,
                egui::Button::new(format!("{} Generate seed pose", crate::icons::SPARKLE)),
            )
            .clicked()
        {
            self.start_first_frame();
        }
        studio_status_line(ui, &self.studio.ff_status);
        if busy && ui.button("Cancel").clicked() {
            self.cancel_first_frame();
        }

        ui.add_space(8.0);
        ui.separator();
        ui.label(egui::RichText::new(format!("{} Refine by inpaint", crate::icons::PENCIL)).strong());
        if self.studio.ff_selected.is_none() {
            ui.label(egui::RichText::new("Select a candidate to paint a mask over what's wrong.").small().weak());
        } else {
            let mut painting = self.studio.painting;
            if ui.checkbox(&mut painting, "Paint mask").changed() {
                self.studio.painting = painting;
            }
            ui.add(egui::Slider::new(&mut self.studio.brush, 1.0..=24.0).text("brush"));
            if ui.button(format!("{} Clear mask", crate::icons::REMOVE)).clicked() {
                if let Some(mask) = self.studio.mask.as_mut() {
                    mask.clear();
                }
            }
            ui.add(
                egui::TextEdit::multiline(&mut self.studio.inpaint_prompt)
                    .hint_text("fix the face — make it a single glowing screen")
                    .desired_rows(2)
                    .desired_width(f32::INFINITY),
            );
            let has_mask = self.studio.mask.as_ref().is_some_and(|m| !m.is_empty());
            if ui
                .add_enabled(
                    self.backend_ready && !busy && has_mask,
                    egui::Button::new(format!("{} Regenerate masked region", crate::icons::SPARKLE)),
                )
                .on_hover_text("Repaint only the masked pixels, conditioned on the anchor")
                .clicked()
            {
                self.start_inpaint();
            }
            if !has_mask {
                ui.label(egui::RichText::new("Paint a mask first.").small().weak());
            }
        }

        ui.add_space(8.0);
        ui.separator();
        let can_approve = self.studio.ff_selected.is_some();
        if ui
            .add_enabled(can_approve, egui::Button::new(format!("{} Approve pose", crate::icons::CHECK)))
            .on_hover_text("Lock this pose as the seed that drives the clip")
            .clicked()
        {
            self.approve_first_frame();
        }
        if self.studio.approved_first_frame.is_some() {
            ui.colored_label(
                crate::theme::Palette::for_theme(ui.ctx().theme()).success,
                format!("{} pose approved", crate::icons::CHECK),
            );
        }
    }

    /// Kicks off a from-scratch first-frame text-to-image generation on the
    /// tokio runtime. Inpaint refinements go through [`Self::start_inpaint`].
    fn start_first_frame(&mut self) {
        let Some(anchor) = self.doc.active_anchor().map(<[u8]>::to_vec) else {
            return;
        };
        let Some(sprite) = self.doc.active_sprite() else {
            return;
        };
        let job = FirstFrameJob::Generate {
            anchor_png: anchor,
            canvas: (sprite.canvas.width, sprite.canvas.height),
            prompt: self.studio.ff_prompt.clone(),
            num_variants: self.studio.ff_variants,
            seed: None,
        };
        self.spawn_first_frame(job, None, false);
    }

    /// Kicks off an inpaint refinement of the selected candidate.
    fn start_inpaint(&mut self) {
        let Some(i) = self.studio.ff_selected else {
            return;
        };
        let Some(anchor) = self.doc.active_anchor().map(<[u8]>::to_vec) else {
            return;
        };
        let Some(mask) = self.studio.mask.as_ref() else {
            return;
        };
        let Some(mask_png) = mask_overlay_png(mask) else {
            self.studio.ff_status = JobStatus::Failed("could not encode the mask".to_owned());
            return;
        };
        let base = self.studio.ff_candidates[i].png.clone();
        let prompt = if self.studio.inpaint_prompt.trim().is_empty() {
            self.studio.ff_prompt.clone()
        } else {
            self.studio.inpaint_prompt.clone()
        };
        let job = FirstFrameJob::Inpaint {
            base,
            mask: mask_png,
            anchor_png: anchor,
            prompt,
            num_variants: 1,
        };
        self.spawn_first_frame(job, Some(i), true);
    }

    /// Spawns `job` on the runtime with a fresh epoch and cancel handle.
    fn spawn_first_frame(&mut self, job: FirstFrameJob, parent: Option<usize>, append: bool) {
        let cancel = CancellationToken::new();
        self.studio.ff_cancel = Some(cancel.clone());
        self.studio.ff_epoch += 1;
        let epoch = self.studio.ff_epoch;
        self.studio.ff_status = JobStatus::Running("starting".to_owned());
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

    /// Cancels an in-flight first-frame generation.
    fn cancel_first_frame(&mut self) {
        if let Some(cancel) = self.studio.ff_cancel.take() {
            cancel.cancel();
        }
        self.studio.ff_epoch += 1;
        self.studio.ff_status = JobStatus::Idle;
    }

    /// Lands generated first-frame candidates into the gallery. Decodes each PNG
    /// (dropping any that fail), stamps lineage, and selects the first new one.
    pub(crate) fn on_first_frame_ready(&mut self, images: Vec<Vec<u8>>, parent: Option<usize>, append: bool) {
        self.studio.ff_status = JobStatus::Idle;
        self.studio.ff_cancel = None;
        if !append {
            self.studio.ff_candidates.clear();
            self.studio.ff_selected = None;
        }
        let origin = if parent.is_some() { FfOrigin::Inpaint } else { FfOrigin::Fresh };
        let first_new = self.studio.ff_candidates.len();
        for png in images {
            // Skip a candidate whose bytes don't decode rather than show a broken card.
            if image::load_from_memory(&png).is_err() {
                continue;
            }
            self.studio.ff_candidates.push(FirstFrameCandidate {
                png,
                texture: None,
                parent,
                origin,
            });
        }
        if self.studio.ff_candidates.len() > first_new {
            self.select_first_frame(first_new);
        }
    }

    /// Approves the selected candidate as the seed pose and advances to Motion.
    fn approve_first_frame(&mut self) {
        let Some(i) = self.studio.ff_selected else {
            return;
        };
        self.studio.approved_first_frame = Some(self.studio.ff_candidates[i].png.clone());
        self.studio.painting = false;
        self.studio.stage = StudioStage::Motion;
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

    /// The raw clip plays large, with the draggable loop scrubber beneath it.
    /// In compare mode two clips play side by side.
    fn studio_clip_surface(&mut self, ui: &mut egui::Ui) {
        if self.anim_candidates.is_empty() {
            centered_hint(ui, "Generate a clip in the Motion stage. It plays here so you can mark the loop.");
            return;
        }
        let Some(i) = self.anim_selected else {
            centered_hint(ui, "Select a clip in the inspector gallery.");
            return;
        };
        let scrubber_h = 56.0;
        let total = ui.available_size();
        let view_h = (total.y - scrubber_h - 12.0).max(80.0);

        if self.studio.compare {
            if let Some(other) = self.studio.compare_other.filter(|&o| o != i && o < self.anim_candidates.len()) {
                let scrub = self.anim_scrub;
                ui.allocate_ui(egui::vec2(total.x, view_h), |ui| {
                    ui.columns(2, |cols| {
                        self.studio_clip_frame(&mut cols[0], i, scrub);
                        self.studio_clip_frame(&mut cols[1], other, scrub);
                    });
                });
                ui.separator();
                self.studio_scrubber(ui, i);
                return;
            }
        }

        ui.allocate_ui(egui::vec2(total.x, view_h), |ui| {
            let scrub = self.anim_scrub;
            self.studio_clip_frame(ui, i, scrub);
        });
        ui.separator();
        self.studio_scrubber(ui, i);
    }

    /// Draws clip `i`'s frame `idx` fit to the available area, building the
    /// frame texture lazily into the candidate's thumbnail slot.
    fn studio_clip_frame(&mut self, ui: &mut egui::Ui, i: usize, idx: usize) {
        let ctx = ui.ctx().clone();
        let n = self.anim_candidates[i].frames.len();
        if n == 0 {
            return;
        }
        let idx = idx.min(n - 1);
        if self.anim_candidates[i].thumbs.get(idx).is_some_and(Option::is_none) {
            if let Some(frame) = self.anim_candidates[i].frames.get(idx) {
                let tex = crate::app::video_frame_to_texture(&ctx, frame);
                if let Some(slot) = self.anim_candidates[i].thumbs.get_mut(idx) {
                    *slot = Some(tex);
                }
            }
        }
        if let Some(Some(tex)) = self.anim_candidates[i].thumbs.get(idx) {
            fit_image(ui, tex);
        }
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
            ui.selectable_value(&mut self.anim_play_mode, AnimPlayMode::Clip, "Clip");
            ui.selectable_value(&mut self.anim_play_mode, AnimPlayMode::Loop, "Loop");
            ui.selectable_value(&mut self.anim_play_mode, AnimPlayMode::Picks, "Picks");
        });
        self.anim_transport(ui);

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
        ui.label(
            egui::RichText::new("Background removal is a re-runnable timeline operation — strip the loop's backgrounds after landing, from the editor.")
                .small()
                .weak(),
        );
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

/// Draws `tex` scaled to fit the available area, preserving aspect, centered.
fn fit_image(ui: &mut egui::Ui, tex: &egui::TextureHandle) {
    let size = tex.size_vec2();
    let avail = ui.available_size();
    let scale = (avail.x / size.x.max(1.0)).min(avail.y / size.y.max(1.0)).max(0.01);
    ui.centered_and_justified(|ui| {
        ui.add(egui::Image::new((tex.id(), size * scale)));
    });
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
}
