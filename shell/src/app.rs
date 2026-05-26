//! The `eframe::App` implementation and the application state it owns.
//!
//! [`DocumentStore`] is a plain field mutated through `&mut self` — single
//! owner, no `RwLock` for UI-thread access. Background AI work runs on the
//! owned tokio runtime and reports back over [`ShellMsg`] on an `mpsc` channel
//! that [`ShellApp::logic`] drains each frame.

use std::sync::mpsc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use eframe::egui;
use egui_wgpu::RenderState;
use pixhaus_ai::plugin::VerbRuntime;
use pixhaus_core::canvas::PixelBuffer;
use pixhaus_core::project::{FrameIndex, LoopDirection, Size};
use pixhaus_render::{Viewport, ViewportRenderer};
use tokio::runtime::Runtime;

use crate::ai;
use crate::document::{DocumentStore, SpriteRef};

/// Default canvas size for a newly created sprite.
const DEFAULT_CANVAS: Size = Size {
    width: 64,
    height: 64,
};

/// Results delivered from background tokio work to the UI thread.
// Variants share the `Sheet` prefix today; animation variants (P5) will join
// them under this one channel pump.
#[allow(clippy::enum_variant_names)]
#[derive(Debug)]
pub enum ShellMsg {
    /// Reference-sheet generation progress.
    SheetProgress {
        /// Completion fraction `0.0`–`1.0`, or `None` if indeterminate.
        fraction: Option<f32>,
        /// One-line status.
        message: String,
    },
    /// Reference-sheet generation finished: one PNG per variant.
    SheetDone {
        /// Candidate sheet images as PNG bytes.
        variants: Vec<Vec<u8>>,
    },
    /// Reference-sheet generation failed.
    SheetFailed(String),
    /// Animation pipeline progress.
    AnimProgress {
        /// One-line status.
        message: String,
    },
    /// Animation pipeline finished: normalized loop frames + per-frame duration.
    AnimDone {
        /// Loop frames, canvas-sized.
        frames: Vec<PixelBuffer>,
        /// Per-frame duration in milliseconds.
        frame_duration_ms: u32,
    },
    /// Animation pipeline failed.
    AnimFailed(String),
}

/// Which inspector tab is showing.
#[derive(Clone, Copy, PartialEq, Eq)]
enum InspectorTab {
    ReferenceSheet,
    Animation,
}

/// State of an async job surfaced in the inspector.
enum JobStatus {
    Idle,
    Running(String),
    Failed(String),
}

/// A generated reference-sheet candidate plus its lazily-loaded preview texture.
struct SheetCandidate {
    png: Vec<u8>,
    texture: Option<egui::TextureHandle>,
}

/// The eframe application.
pub struct ShellApp {
    /// Single owner of the document. Mutated through `&mut self`.
    pub(crate) doc: DocumentStore,
    /// The shell's tokio runtime; AI verb invocations run here (P4+).
    #[allow(dead_code)]
    runtime: Runtime,
    /// Cloned and handed to background tasks so they can report results (P4+).
    #[allow(dead_code)]
    tx: mpsc::Sender<ShellMsg>,
    /// Drained each frame by [`ShellApp::logic`].
    rx: mpsc::Receiver<ShellMsg>,
    /// wgpu device/queue/format handle; `None` only if the wgpu backend failed
    /// to initialize.
    pub(crate) render_state: Option<RenderState>,
    /// Pan/zoom state for the canvas viewport.
    pub(crate) viewport: Viewport,
    /// Canvas size of whatever frame is currently uploaded, in canvas pixels.
    pub(crate) frame_size: Option<[f32; 2]>,
    /// Set when the next canvas paint should fit the frame to the viewport.
    pub(crate) needs_fit: bool,
    /// Draft name for the new-sprite control.
    new_sprite_name: String,
    /// Whether playback is running.
    playing: bool,
    /// Frame order playback walks (expanded from the active sprite's tag).
    play_order: Vec<FrameIndex>,
    /// Cursor into [`Self::play_order`].
    play_cursor: usize,
    /// When the current frame was shown; the next advance is due one frame
    /// duration later.
    last_advance: Instant,
    /// The verb runtime (reference-sheet verb + FAL backend).
    verb_runtime: Arc<VerbRuntime>,
    /// Whether a generation backend is registered and ready.
    backend_ready: bool,
    /// egui context clone handed to background tasks to wake the idle UI.
    egui_ctx: egui::Context,
    /// Which inspector tab is showing.
    inspector_tab: InspectorTab,
    /// Reference-sheet prompt draft.
    rs_prompt: String,
    /// Selected template index into [`ai::TEMPLATES`].
    rs_template: usize,
    /// Requested candidate count (1-4).
    rs_num_variants: u32,
    /// Reference-sheet job status.
    rs_status: JobStatus,
    /// Generated candidates awaiting approval.
    rs_candidates: Vec<SheetCandidate>,
    /// `OpenAI` API key draft (entered when no backend is configured).
    openai_key_input: String,
    /// FAL API key draft (entered when no backend is configured).
    fal_key_input: String,
    /// Animation motion prompt draft.
    anim_motion: String,
    /// Requested loop frame count.
    anim_target_frames: u32,
    /// Playback FPS for the generated animation.
    anim_fps: u32,
    /// Animation job status.
    anim_status: JobStatus,
    /// Light/dark/system theme preference; persisted across launches.
    theme_preference: egui::ThemePreference,
}

impl ShellApp {
    /// Builds the app from the eframe creation context, taking ownership of the
    /// shell's tokio runtime. Installs the [`ViewportRenderer`] and creates one
    /// starter sprite so the canvas shows immediately.
    pub fn new(cc: &eframe::CreationContext<'_>, runtime: Runtime) -> Self {
        let (tx, rx) = mpsc::channel();
        let render_state = cc.wgpu_render_state.clone();
        let (verb_runtime, backend_ready) = ai::build_runtime();

        // Restore the saved theme preference (defaults to following the OS) and
        // install the brand theme and fonts before the first frame draws.
        let theme_preference = cc
            .storage
            .and_then(|s| eframe::get_value::<egui::ThemePreference>(s, "theme_preference"))
            .unwrap_or_default();
        crate::theme::install(&cc.egui_ctx, theme_preference);

        let mut app = Self {
            doc: DocumentStore::new(),
            runtime,
            tx,
            rx,
            render_state,
            viewport: Viewport::new(),
            frame_size: None,
            needs_fit: false,
            new_sprite_name: String::new(),
            playing: false,
            play_order: Vec::new(),
            play_cursor: 0,
            last_advance: Instant::now(),
            verb_runtime,
            backend_ready,
            egui_ctx: cc.egui_ctx.clone(),
            inspector_tab: InspectorTab::ReferenceSheet,
            rs_prompt: ai::DEFAULT_SHEET_PROMPT.to_owned(),
            rs_template: 0,
            rs_num_variants: 2,
            rs_status: JobStatus::Idle,
            rs_candidates: Vec::new(),
            openai_key_input: String::new(),
            fal_key_input: String::new(),
            anim_motion: "walk cycle, side view".to_owned(),
            anim_target_frames: 6,
            anim_fps: 10,
            anim_status: JobStatus::Idle,
            theme_preference,
        };
        app.install_renderer();
        app.doc.create_sprite("untitled", DEFAULT_CANVAS);
        app.refresh_canvas(true);
        app
    }

    /// Inserts the [`ViewportRenderer`] into egui-wgpu's callback resources so
    /// the paint callback can reach it.
    fn install_renderer(&self) {
        let Some(rs) = self.render_state.as_ref() else {
            tracing::error!("no wgpu render state; canvas will be blank");
            return;
        };
        let renderer = ViewportRenderer::new(&rs.device, rs.target_format);
        rs.renderer.write().callback_resources.insert(renderer);
    }

    /// Composites the active frame and uploads it to the renderer. `refit`
    /// re-fits the viewport (use on sprite selection, not on playback ticks).
    pub(crate) fn refresh_canvas(&mut self, refit: bool) {
        if let Some(frame) = self.doc.composite_active_frame() {
            self.upload_frame(&frame, refit);
        }
    }

    /// Drains background results into the document. Returns true if any message
    /// was applied, so the caller can request a repaint.
    fn drain_results(&mut self) -> bool {
        let mut applied = false;
        while let Ok(msg) = self.rx.try_recv() {
            applied = true;
            match msg {
                ShellMsg::SheetProgress { fraction, message } => {
                    let pct = fraction.map_or_else(String::new, |f| format!("{:.0}% ", f * 100.0));
                    self.rs_status = JobStatus::Running(format!("{pct}{message}"));
                }
                ShellMsg::SheetDone { variants } => {
                    self.rs_status = JobStatus::Idle;
                    self.rs_candidates = variants
                        .into_iter()
                        .map(|png| SheetCandidate { png, texture: None })
                        .collect();
                }
                ShellMsg::SheetFailed(err) => {
                    self.rs_status = JobStatus::Failed(err);
                }
                ShellMsg::AnimProgress { message } => {
                    self.anim_status = JobStatus::Running(message);
                }
                ShellMsg::AnimDone {
                    frames,
                    frame_duration_ms,
                } => {
                    self.anim_status = JobStatus::Idle;
                    self.doc.integrate_frames(
                        frames,
                        frame_duration_ms,
                        &self.anim_motion.clone(),
                        LoopDirection::Forward,
                    );
                    self.refresh_canvas(true);
                }
                ShellMsg::AnimFailed(err) => {
                    self.anim_status = JobStatus::Failed(err);
                }
            }
        }
        applied
    }

    /// Left panel: the sprite library — create and select sprites.
    fn library_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Library");
        ui.add_space(4.0);

        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.new_sprite_name)
                    .hint_text("name")
                    .desired_width(120.0),
            );
            if ui.button("New sprite").clicked() {
                let name = if self.new_sprite_name.trim().is_empty() {
                    "untitled".to_owned()
                } else {
                    self.new_sprite_name.trim().to_owned()
                };
                self.doc.create_sprite(name, DEFAULT_CANVAS);
                self.new_sprite_name.clear();
                self.refresh_canvas(true);
            }
        });

        ui.separator();

        let items = self.doc.sprite_list();
        let mut select: Option<SpriteRef> = None;
        for item in &items {
            let label = format!(
                "{}  ({}x{})",
                item.name, item.canvas.width, item.canvas.height
            );
            if ui.selectable_label(item.selected, label).clicked() {
                select = Some(item.sprite_ref);
            }
        }
        if let Some(sprite_ref) = select {
            self.doc.select(sprite_ref);
            self.playing = false;
            self.refresh_canvas(true);
        }
    }

    /// Starts or stops playback. Starting computes the play order from the
    /// active sprite's tag; a single-frame sprite cannot animate.
    fn toggle_play(&mut self) {
        if self.playing {
            self.playing = false;
            return;
        }
        let order = self.doc.active_play_order();
        if order.len() < 2 {
            return;
        }
        self.play_cursor = order
            .iter()
            .position(|&f| f == self.doc.active_frame)
            .unwrap_or(0);
        self.play_order = order;
        self.playing = true;
        self.last_advance = Instant::now();
    }

    /// Advances the playhead when the current frame's duration has elapsed, and
    /// schedules the next wake-up. Driven from [`eframe::App::logic`] so it runs
    /// even while the pointer is idle.
    fn tick_playback(&mut self, ctx: &egui::Context) {
        if !self.playing {
            return;
        }
        if self.play_order.len() < 2 {
            self.playing = false;
            return;
        }
        let dur = Duration::from_millis(u64::from(
            self.doc.frame_duration_ms(self.doc.active_frame),
        ));
        if self.last_advance.elapsed() >= dur {
            self.play_cursor = (self.play_cursor + 1) % self.play_order.len();
            self.doc.active_frame = self.play_order[self.play_cursor];
            self.last_advance = Instant::now();
            self.refresh_canvas(false);
        }
        ctx.request_repaint_after(dur);
    }

    /// Bottom panel: frames, playhead, transport.
    #[allow(clippy::cast_possible_truncation)] // frame counts fit u32
    fn timeline_panel(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading("Timeline");
            ui.separator();
            let play_label = if self.playing { "⏸ Pause" } else { "▶ Play" };
            if ui.button(play_label).clicked() {
                self.toggle_play();
            }
            ui.separator();
            ui.label(format!("{} frames", self.doc.frame_count()));
            ui.label(format!("playhead {}", self.doc.active_frame.get()));
            ui.separator();
            if ui.button("Add demo animation").clicked() {
                self.doc.add_demo_animation();
                self.playing = false;
                self.refresh_canvas(true);
            }
        });
        ui.separator();

        let count = self.doc.frame_count();
        let active = self.doc.active_frame;
        let mut select: Option<FrameIndex> = None;
        egui::ScrollArea::horizontal().show(ui, |ui| {
            ui.horizontal(|ui| {
                for i in 0..count {
                    let idx = FrameIndex::new(i as u32);
                    if ui
                        .selectable_label(idx == active, format!("{i}"))
                        .clicked()
                    {
                        select = Some(idx);
                    }
                }
            });
        });
        if let Some(idx) = select {
            self.playing = false;
            self.doc.active_frame = idx;
            self.refresh_canvas(false);
        }
    }

    /// Right panel: the dockable inspector with Reference-sheet and Animation
    /// tabs.
    fn inspector_panel(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.selectable_value(
                &mut self.inspector_tab,
                InspectorTab::ReferenceSheet,
                "Reference sheet",
            );
            ui.selectable_value(&mut self.inspector_tab, InspectorTab::Animation, "Animation");
        });
        ui.separator();
        match self.inspector_tab {
            InspectorTab::ReferenceSheet => self.reference_sheet_tab(ui),
            InspectorTab::Animation => self.animation_tab(ui),
        }
    }

    /// Reference-sheet tab: prompt, template, generate against the real verb,
    /// review candidates, approve one as the sprite's canonical anchor.
    fn reference_sheet_tab(&mut self, ui: &mut egui::Ui) {
        if !self.backend_ready {
            self.key_entry(ui);
            ui.separator();
        }

        ui.label("Prompt");
        ui.add(
            egui::TextEdit::multiline(&mut self.rs_prompt)
                .hint_text("a small knight with a round shield")
                .desired_rows(2),
        );

        egui::ComboBox::from_label("Template")
            .selected_text(ai::TEMPLATES[self.rs_template].0)
            .show_ui(ui, |ui| {
                for (i, (name, _)) in ai::TEMPLATES.iter().enumerate() {
                    ui.selectable_value(&mut self.rs_template, i, *name);
                }
            });

        ui.add(egui::Slider::new(&mut self.rs_num_variants, 1..=4).text("variants"));

        let running = matches!(self.rs_status, JobStatus::Running(_));
        let can_generate = self.backend_ready && !running && self.doc.active_entity_id().is_some();
        if ui
            .add_enabled(can_generate, egui::Button::new("Generate"))
            .clicked()
        {
            self.start_reference_sheet();
        }

        match &self.rs_status {
            JobStatus::Running(msg) => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(msg);
                });
            }
            JobStatus::Failed(err) => {
                ui.colored_label(egui::Color32::LIGHT_RED, format!("failed: {err}"));
            }
            JobStatus::Idle => {}
        }

        if self.doc.active_anchor().is_some() {
            ui.colored_label(egui::Color32::LIGHT_GREEN, "✓ anchor approved");
        }

        ui.separator();
        self.candidate_strip(ui);
    }

    /// API-key entry shown when no backend is configured. `OpenAI` (gpt-image-2)
    /// covers reference sheets; FAL also covers the animation pipeline.
    fn key_entry(&mut self, ui: &mut egui::Ui) {
        ui.label("No generation backend configured. Paste a key:");
        ui.horizontal(|ui| {
            ui.label("OpenAI");
            ui.add(
                egui::TextEdit::singleline(&mut self.openai_key_input)
                    .password(true)
                    .desired_width(160.0),
            );
            if ui.button("Save").clicked() && !self.openai_key_input.trim().is_empty() {
                let key = self.openai_key_input.trim().to_owned();
                self.save_key(ai::OPENAI_BACKEND_ID, &key);
                self.openai_key_input.clear();
            }
        });
        ui.horizontal(|ui| {
            ui.label("FAL    ");
            ui.add(
                egui::TextEdit::singleline(&mut self.fal_key_input)
                    .password(true)
                    .desired_width(160.0),
            );
            if ui.button("Save").clicked() && !self.fal_key_input.trim().is_empty() {
                let key = self.fal_key_input.trim().to_owned();
                self.save_key(ai::FAL_BACKEND_ID, &key);
                self.fal_key_input.clear();
            }
        });
        ui.label("OpenAI generates reference sheets; FAL also does animation (image-to-video).");
    }

    /// Stores a key and re-registers backends, updating readiness.
    fn save_key(&mut self, backend: &str, key: &str) {
        match ai::store_key(backend, key) {
            Ok(()) => {
                let openai = ai::try_register_openai(&self.verb_runtime);
                let fal = ai::try_register_fal(&self.verb_runtime);
                self.backend_ready = openai || fal;
            }
            Err(err) => self.rs_status = JobStatus::Failed(format!("keychain: {err}")),
        }
    }

    /// Kicks off a reference-sheet generation on the tokio runtime.
    fn start_reference_sheet(&mut self) {
        let Some(entity_id) = self.doc.active_entity_id() else {
            return;
        };
        let job = ai::SheetJob {
            meta: self.doc.project.metadata.clone(),
            entity_id,
            structure_id: ai::TEMPLATES[self.rs_template].1.to_owned(),
            prompt: self.rs_prompt.clone(),
            num_variants: self.rs_num_variants,
        };
        self.rs_status = JobStatus::Running("starting".into());
        self.rs_candidates.clear();
        ai::spawn_reference_sheet(
            self.runtime.handle(),
            self.verb_runtime.clone(),
            self.egui_ctx.clone(),
            self.tx.clone(),
            job,
        );
    }

    /// Renders the candidate sheets with an Approve action.
    fn candidate_strip(&mut self, ui: &mut egui::Ui) {
        if self.rs_candidates.is_empty() {
            return;
        }
        let ctx = ui.ctx().clone();
        for cand in &mut self.rs_candidates {
            if cand.texture.is_none() {
                cand.texture = load_png_texture(&ctx, &cand.png);
            }
        }

        let mut approve: Option<usize> = None;
        egui::ScrollArea::vertical().show(ui, |ui| {
            for (i, cand) in self.rs_candidates.iter().enumerate() {
                ui.group(|ui| {
                    if let Some(tex) = &cand.texture {
                        let size = tex.size_vec2();
                        let scale = (260.0 / size.x).min(1.0);
                        ui.image((tex.id(), size * scale));
                    } else {
                        ui.label("(decode failed)");
                    }
                    if ui.button("Approve as anchor").clicked() {
                        approve = Some(i);
                    }
                });
            }
        });

        if let Some(i) = approve {
            let png = self.rs_candidates[i].png.clone();
            self.doc.set_active_anchor(png);
            self.inspector_tab = InspectorTab::Animation;
        }
    }

    /// Animation tab: generate one animation from the approved anchor through
    /// the real FAL image-to-video pipeline, then integrate the loop frames.
    fn animation_tab(&mut self, ui: &mut egui::Ui) {
        if self.doc.active_anchor().is_none() {
            ui.label("Approve a reference sheet first (Reference sheet tab).");
            return;
        }

        ui.label("Motion");
        ui.add(
            egui::TextEdit::singleline(&mut self.anim_motion)
                .hint_text("walk cycle, side view")
                .desired_width(260.0),
        );
        ui.add(egui::Slider::new(&mut self.anim_target_frames, 2..=16).text("loop frames"));
        ui.add(egui::Slider::new(&mut self.anim_fps, 4..=24).text("fps"));

        let running = matches!(self.anim_status, JobStatus::Running(_));
        let can_generate = self.backend_ready && !running;
        if ui
            .add_enabled(can_generate, egui::Button::new("Generate animation"))
            .clicked()
        {
            self.start_animation();
        }

        match &self.anim_status {
            JobStatus::Running(msg) => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(msg);
                });
                ui.label("(image-to-video runs for many seconds)");
            }
            JobStatus::Failed(err) => {
                ui.colored_label(egui::Color32::LIGHT_RED, format!("failed: {err}"));
            }
            JobStatus::Idle => {}
        }

        if self.doc.frame_count() > 1 {
            ui.separator();
            ui.label(format!(
                "{} frames in the timeline — press Play.",
                self.doc.frame_count()
            ));
        }
    }

    /// Kicks off the animation pipeline on the tokio runtime.
    fn start_animation(&mut self) {
        let Some(anchor) = self.doc.active_anchor().map(<[u8]>::to_vec) else {
            return;
        };
        let Some(sprite) = self.doc.active_sprite() else {
            return;
        };
        let job = ai::AnimJob {
            canvas: (sprite.canvas.width, sprite.canvas.height),
            anchor_png: anchor,
            motion_prompt: self.anim_motion.clone(),
            target_frames: self.anim_target_frames,
            fps: self.anim_fps,
        };
        self.anim_status = JobStatus::Running("starting".into());
        ai::spawn_animation(
            self.runtime.handle(),
            self.verb_runtime.clone(),
            self.egui_ctx.clone(),
            self.tx.clone(),
            job,
        );
    }

    /// Top panel: menu bar with the wordmark, File/View menus, and a theme
    /// toggle pinned to the right.
    fn menu_bar(&mut self, ui: &mut egui::Ui) {
        egui::MenuBar::new().ui(ui, |ui| {
            let accent = ui.visuals().hyperlink_color;
            ui.label(egui::RichText::new("Pixhaus").heading().color(accent));
            ui.separator();

            ui.menu_button("File", |ui| {
                if ui.button("New sprite").clicked() {
                    self.doc.create_sprite("untitled", DEFAULT_CANVAS);
                    self.refresh_canvas(true);
                    ui.close();
                }
                ui.separator();
                if ui.button("Quit").clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });

            ui.menu_button("View", |ui| {
                ui.label("Theme");
                let mut pref = self.theme_preference;
                ui.radio_value(&mut pref, egui::ThemePreference::System, "System");
                ui.radio_value(&mut pref, egui::ThemePreference::Dark, "Dark");
                ui.radio_value(&mut pref, egui::ThemePreference::Light, "Light");
                if pref != self.theme_preference {
                    self.set_theme_preference(ui.ctx(), pref);
                }
            });

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .button(crate::theme::preference_label(self.theme_preference))
                    .on_hover_text("Cycle System / Dark / Light")
                    .clicked()
                {
                    let next = crate::theme::next_preference(self.theme_preference);
                    self.set_theme_preference(ui.ctx(), next);
                }
            });
        });
    }

    /// Applies and remembers a new theme preference.
    fn set_theme_preference(&mut self, ctx: &egui::Context, preference: egui::ThemePreference) {
        self.theme_preference = preference;
        ctx.set_theme(preference);
    }

    /// Bottom status bar: a single muted line of document and session state.
    fn status_bar(&mut self, ui: &mut egui::Ui) {
        let palette = crate::theme::Palette::for_theme(ui.ctx().theme());
        ui.horizontal(|ui| {
            let muted = |text: String| egui::RichText::new(text).small().weak();
            ui.label(muted(crate::theme::preference_label(self.theme_preference).to_owned()));
            ui.separator();
            let (backend, color) = if self.backend_ready {
                ("backend ready", palette.success)
            } else {
                ("no backend", palette.warning)
            };
            ui.label(egui::RichText::new(backend).small().color(color));
            if let Some(sprite) = self.doc.active_sprite() {
                ui.separator();
                ui.label(muted(format!("{}x{}", sprite.canvas.width, sprite.canvas.height)));
            }
            ui.separator();
            ui.label(muted(format!("{} frames", self.doc.frame_count())));
            ui.separator();
            ui.label(muted(format!("{:.0}% zoom", self.viewport.zoom * 100.0)));
        });
    }
}

/// Decodes PNG bytes into an egui texture with NEAREST sampling (pixel art).
fn load_png_texture(ctx: &egui::Context, png: &[u8]) -> Option<egui::TextureHandle> {
    let rgba = image::load_from_memory(png).ok()?.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    let color = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
    Some(ctx.load_texture("rs_candidate", color, egui::TextureOptions::NEAREST))
}

impl eframe::App for ShellApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.drain_results() {
            ctx.request_repaint();
        }
        self.tick_playback(ctx);
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, "theme_preference", &self.theme_preference);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // Panel order matters: outer panels first, the central canvas last so
        // it fills the space the others leave. The status bar is added before
        // the timeline so it sits at the very bottom edge.
        egui::Panel::top("menu_bar").show_inside(ui, |ui| self.menu_bar(ui));

        egui::Panel::bottom("status_bar")
            .resizable(false)
            .show_inside(ui, |ui| self.status_bar(ui));

        egui::Panel::left("library")
            .resizable(true)
            .default_size(240.0)
            .show_inside(ui, |ui| self.library_panel(ui));

        egui::Panel::right("inspector")
            .resizable(true)
            .default_size(340.0)
            .show_inside(ui, |ui| self.inspector_panel(ui));

        egui::Panel::bottom("timeline")
            .resizable(true)
            .default_size(160.0)
            .show_inside(ui, |ui| self.timeline_panel(ui));

        egui::CentralPanel::default().show_inside(ui, |ui| {
            self.canvas_ui(ui);
        });
    }
}
