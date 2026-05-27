//! The `eframe::App` implementation and the application state it owns.
//!
//! [`DocumentStore`] is a plain field mutated through `&mut self` — single
//! owner, no `RwLock` for UI-thread access. Background AI work runs on the
//! owned tokio runtime and reports back over [`ShellMsg`] on an `mpsc` channel
//! that [`ShellApp::logic`] drains each frame.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use eframe::egui;
use egui_wgpu::RenderState;
use pixhaus_ai::plugin::VerbRuntime;
use pixhaus_core::canvas::PixelBuffer;
use pixhaus_core::project::{FrameIndex, LoopDirection, PixelBufferId, Rgba, Size};
use pixhaus_core::transforms::normalize::{NormalizeOptions, normalize_frames};
use pixhaus_render::{Viewport, ViewportRenderer};
use tokio::runtime::Runtime;
use tokio_util::sync::CancellationToken;

use crate::ai;
use crate::anim::{self, LoopMarkers, VideoFrame};
use crate::cockpit::{CockpitCandidate, CockpitReference, CreateView, PendingLineage};
use crate::document::{DocumentStore, SpriteRef};
use crate::keymap::{CommandId, Keymap};
use crate::settings::SettingsTab;

/// Default canvas size for a newly created sprite.
const DEFAULT_CANVAS: Size = Size { width: 64, height: 64 };

/// Results delivered from background tokio work to the UI thread.
#[derive(Debug)]
pub enum ShellMsg {
    /// Reference-sheet generation progress.
    SheetProgress {
        /// Completion fraction `0.0`–`1.0`, or `None` if indeterminate.
        fraction: Option<f32>,
        /// One-line status.
        message: String,
    },
    /// Reference-sheet generation finished: candidates with full provenance.
    SheetDone {
        /// Generated candidates (provenance + decoded PNG).
        variants: Vec<crate::ai::GeneratedVariant>,
        /// Reported run cost in USD, if the backend surfaced one.
        cost_usd: Option<f64>,
    },
    /// Reference-sheet generation failed.
    SheetFailed(String),
    /// Clip-generation (Generate stage) progress.
    ClipProgress {
        /// Generation epoch this message belongs to; stale ones are dropped.
        epoch: u64,
        /// One-line status.
        message: String,
    },
    /// Clip generation finished: the raw clip plus its decoded frames. The
    /// wizard scrubs, marks, and picks from these on the UI thread.
    ClipReady {
        /// Generation epoch this result belongs to; stale ones are dropped.
        epoch: u64,
        /// Raw clip bytes, exactly as the backend returned them.
        clip: Vec<u8>,
        /// MIME type of the clip.
        mime: String,
        /// Decoded RGBA frames, in order.
        frames: Vec<VideoFrame>,
    },
    /// Clip generation failed (or was canceled).
    ClipFailed {
        /// Generation epoch this failure belongs to; stale ones are dropped.
        epoch: u64,
        /// Failure reason.
        error: String,
    },
    /// AI background removal of one cel finished: the stripped PNG to apply to
    /// `buffer_id`.
    BgRemovalDone {
        /// Cel buffer the result belongs to.
        buffer_id: PixelBufferId,
        /// Stripped image as PNG bytes.
        png: Vec<u8>,
    },
    /// AI background removal of one cel failed.
    BgRemovalFailed {
        /// Cel buffer the request was for.
        buffer_id: PixelBufferId,
        /// Failure reason.
        error: String,
    },
}

/// What the selected clip card's Play button cycles.
#[derive(Clone, Copy, PartialEq, Eq)]
enum AnimPlayMode {
    /// Every decoded frame, in order.
    Clip,
    /// Only the marked `[start, end)` loop.
    Loop,
    /// Only the picked frames.
    Picks,
}

/// One generated animation clip in the gallery: the raw bytes, decoded frames,
/// the loop markers and frame picks the artist tunes, and generation
/// provenance. Editing a card's markers/picks mutates it in place; Integrate
/// lands its picks on the timeline. Nothing touches the sprite until Integrate.
pub(crate) struct ClipCandidate {
    /// Raw clip bytes exactly as the backend returned them (kept for re-decode).
    pub clip: Vec<u8>,
    /// MIME of the raw clip.
    pub mime: String,
    /// Decoded RGBA frames, in order.
    pub frames: Vec<VideoFrame>,
    /// Lazily-built thumbnail textures, one slot per decoded frame.
    pub thumbs: Vec<Option<egui::TextureHandle>>,
    /// Loop markers, seeded by `auto_loop_markers`, dragged per card.
    pub markers: LoopMarkers,
    /// Picked frame indices into [`Self::frames`].
    pub picks: Vec<usize>,
    /// Motion prompt this clip was generated from (provenance).
    pub motion: String,
    /// Playback fps requested for this clip (provenance + integrate timing).
    pub fps: u32,
    /// Seed used, if the run pinned one (provenance).
    pub seed: Option<u64>,
    /// Lazily-built first-frame thumbnail for the gallery card.
    pub card_texture: Option<egui::TextureHandle>,
}

/// Top-level workspace mode. Drives what the side dock shows and whether the
/// timeline starts expanded — the `OpenToonz` "rooms" idea, trimmed to three.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Workspace {
    /// Draw on a single frame; dock shows Colour/Layers/Sprites, timeline slim.
    Draw,
    /// Animate; same dock, timeline expanded to the cel matrix.
    Animate,
    /// AI generation; dock shows the reference-sheet / animation surface.
    Create,
}

/// Which tab the side dock shows in Draw/Animate modes.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum RightTab {
    Color,
    Layers,
    Sprites,
}

/// A keyboard-driven zoom request, applied on the next canvas paint where the
/// viewport size is known so the zoom stays centred on the viewport.
#[derive(Clone, Copy)]
pub(crate) enum ZoomAction {
    /// Zoom in one snap step.
    In,
    /// Zoom out one snap step.
    Out,
    /// Reset to 100%.
    Reset,
}

/// State of an async job surfaced in the inspector.
pub(crate) enum JobStatus {
    Idle,
    Running(String),
    Failed(String),
}

/// The eframe application.
pub struct ShellApp {
    /// Single owner of the document. Mutated through `&mut self`.
    pub(crate) doc: DocumentStore,
    /// The shell's tokio runtime; AI verb invocations run here (P4+).
    pub(crate) runtime: Runtime,
    /// Cloned and handed to background tasks so they can report results (P4+).
    pub(crate) tx: mpsc::Sender<ShellMsg>,
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
    pub(crate) playing: bool,
    /// Frame order playback walks (expanded from the active sprite's tag).
    pub(crate) play_order: Vec<FrameIndex>,
    /// Cursor into [`Self::play_order`].
    pub(crate) play_cursor: usize,
    /// When the current frame was shown; the next advance is due one frame
    /// duration later.
    pub(crate) last_advance: Instant,
    /// Interactive-editing state: tools, brush, colours, selection, onion skin,
    /// and the undo history.
    pub(crate) editor: crate::editor::EditorState,
    /// Cached full-canvas composite of the active frame. The drawing hot path
    /// recomposites and uploads only a dirty sub-rect of this buffer.
    pub(crate) display_frame: Option<PixelBuffer>,
    /// Active workspace mode (Draw / Animate / Create).
    pub(crate) workspace: Workspace,
    /// Which side-dock tab shows in Draw/Animate modes.
    pub(crate) right_tab: RightTab,
    /// Whether the bottom timeline shows the full cel matrix (vs a slim
    /// transport line).
    pub(crate) timeline_expanded: bool,
    /// The verb runtime (reference-sheet verb + FAL backend).
    pub(crate) verb_runtime: Arc<VerbRuntime>,
    /// Whether a generation backend is registered and ready.
    pub(crate) backend_ready: bool,
    /// egui context clone handed to background tasks to wake the idle UI.
    pub(crate) egui_ctx: egui::Context,
    /// Which Create-mode surface is showing (cockpit / library / animate).
    pub(crate) create_view: CreateView,
    /// Cockpit subject prompt draft.
    pub(crate) rs_prompt: String,
    /// Selected composition Structure id (free-form `Single` by default).
    pub(crate) ck_structure: String,
    /// Requested candidate count (1-4).
    pub(crate) rs_num_variants: u32,
    /// Cockpit job status.
    pub(crate) rs_status: JobStatus,
    /// Generated candidates, newest first, with provenance and lineage.
    pub(crate) rs_candidates: Vec<CockpitCandidate>,
    /// Index of the candidate currently previewed on the canvas, if any.
    /// `None` means the canvas shows the active sprite.
    pub(crate) rs_preview: Option<usize>,
    /// Whether the seed is pinned for a reproducible result.
    pub(crate) ck_seed_fixed: bool,
    /// The pinned seed value, used when [`Self::ck_seed_fixed`].
    pub(crate) ck_seed: u64,
    /// Live composed positive prompt (editable; sent as an override when edited).
    pub(crate) ck_positive: String,
    /// Live composed negative prompt.
    pub(crate) ck_negative: String,
    /// Whether the artist hand-edited the composed prompt (stops auto-recompose,
    /// sends the text verbatim).
    pub(crate) ck_prompt_edited: bool,
    /// Set when an input changed and the composed preview needs recomputing.
    pub(crate) ck_dirty: bool,
    /// Picked saved-prompt template id, or `None` for a free-form subject.
    pub(crate) ck_prompt_id: Option<String>,
    /// Current values for the picked template's variable dials.
    pub(crate) ck_vars: BTreeMap<String, String>,
    /// Per-dial lock: a locked dial is left untouched by randomize / surprise.
    pub(crate) ck_var_locked: HashMap<String, bool>,
    /// Staged drag-in references fed to the next generation.
    pub(crate) ck_references: Vec<CockpitReference>,
    /// Parent index captured by the Refine button, applied on the next Generate.
    pub(crate) ck_refine_parent: Option<usize>,
    /// Lineage for the in-flight generation, applied when results land.
    pub(crate) ck_pending: PendingLineage,
    /// `OpenAI` API key draft (entered in the AI-backends settings tab).
    pub(crate) openai_key_input: String,
    /// FAL API key draft (entered in the AI-backends settings tab).
    pub(crate) fal_key_input: String,
    /// Animation motion prompt draft.
    anim_motion: String,
    /// Requested loop frame count.
    anim_target_frames: u32,
    /// Playback FPS for the generated animation.
    anim_fps: u32,
    /// Whether the Generate stage pins a fixed RNG seed (vs random each run).
    anim_seed_fixed: bool,
    /// The fixed seed value, used when [`Self::anim_seed_fixed`] is set.
    anim_seed: u64,
    /// Generate-stage job status (progress / failure).
    anim_status: JobStatus,
    /// Generated clips, oldest first; the gallery lists them newest-first.
    anim_candidates: Vec<ClipCandidate>,
    /// Index into [`Self::anim_candidates`] of the card being edited/previewed.
    anim_selected: Option<usize>,
    /// What the selected card's Play cycles (whole clip / loop / picks).
    anim_play_mode: AnimPlayMode,
    /// Recent motion prompts, newest first, for quick recall.
    anim_recent_motions: Vec<String>,
    /// Scrub cursor into the selected card's frames.
    anim_scrub: usize,
    /// Which decoded-frame index is currently uploaded to the GPU, if any. Lets
    /// the preview re-upload only when the shown frame changes.
    anim_shown: Option<usize>,
    /// Whether the clip transport is playing.
    anim_clip_playing: bool,
    /// The frame indices the current Play cycles, and a cursor into them.
    anim_play_indices: Vec<usize>,
    /// Cursor into [`Self::anim_play_indices`].
    anim_play_cursor: usize,
    /// When the current clip frame was shown (drives the transport tick).
    anim_clip_last_advance: Instant,
    /// Cancel handle for the Generate stage's i2v await.
    anim_cancel: Option<CancellationToken>,
    /// Monotonic generation id. Bumped on every Generate and on Cancel so a
    /// canceled or superseded clip's late messages are dropped.
    anim_gen_epoch: u64,
    /// Background-removal panel: the key colour (auto-detected or eyedropped).
    pub(crate) bg_key_color: Rgba,
    /// Background-removal panel: per-channel keying tolerance.
    pub(crate) bg_tolerance: u8,
    /// Whether the background-removal live preview is driving the canvas.
    pub(crate) bg_preview: bool,
    /// Whether the op applies to every frame's active-layer cel vs just the
    /// active cel.
    pub(crate) bg_all_frames: bool,
    /// Cels with an AI removal in flight (the button shows a spinner for them).
    pub(crate) bg_remove_pending: HashSet<PixelBufferId>,
    /// Cels the last keying pass judged a likely failure (key missed or was too
    /// broad) — the AI-fallback candidates.
    pub(crate) bg_flagged: Vec<PixelBufferId>,
    /// Background-removal job status (AI fallback progress / failure).
    pub(crate) bg_status: JobStatus,
    /// Light/dark/system theme preference; persisted across launches.
    pub(crate) theme_preference: egui::ThemePreference,
    /// Keyboard bindings: active preset plus custom overrides; persisted.
    pub(crate) keymap: Keymap,
    /// The command currently capturing its next key press in the Keybinds tab.
    pub(crate) capturing: Option<CommandId>,
    /// Whether the settings window is open.
    pub(crate) settings_open: bool,
    /// Which settings tab is showing.
    pub(crate) settings_tab: SettingsTab,
    /// A pending keyboard zoom request, drained on the next canvas paint.
    pub(crate) pending_zoom: Option<ZoomAction>,
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

        // Restore the saved keybindings (preset + custom overrides), defaulting
        // to the Aseprite preset with no overrides.
        let keymap = cc.storage.and_then(|s| eframe::get_value::<Keymap>(s, "keymap")).unwrap_or_default();

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
            editor: crate::editor::EditorState::default(),
            display_frame: None,
            workspace: Workspace::Draw,
            right_tab: RightTab::Color,
            timeline_expanded: false,
            verb_runtime,
            backend_ready,
            egui_ctx: cc.egui_ctx.clone(),
            create_view: CreateView::Cockpit,
            rs_prompt: ai::DEFAULT_SHEET_PROMPT.to_owned(),
            ck_structure: ai::SINGLE_STRUCTURE_ID.to_owned(),
            rs_num_variants: 2,
            rs_status: JobStatus::Idle,
            rs_candidates: Vec::new(),
            rs_preview: None,
            ck_seed_fixed: false,
            ck_seed: 0,
            ck_positive: String::new(),
            ck_negative: String::new(),
            ck_prompt_edited: false,
            ck_dirty: true,
            ck_prompt_id: None,
            ck_vars: BTreeMap::new(),
            ck_var_locked: HashMap::new(),
            ck_references: Vec::new(),
            ck_refine_parent: None,
            ck_pending: PendingLineage::default(),
            openai_key_input: String::new(),
            fal_key_input: String::new(),
            anim_motion: "walk cycle, side view".to_owned(),
            anim_target_frames: 6,
            anim_fps: 10,
            anim_seed_fixed: false,
            anim_seed: 0,
            anim_status: JobStatus::Idle,
            anim_candidates: Vec::new(),
            anim_selected: None,
            anim_play_mode: AnimPlayMode::Loop,
            anim_recent_motions: Vec::new(),
            anim_scrub: 0,
            anim_shown: None,
            anim_clip_playing: false,
            anim_play_indices: Vec::new(),
            anim_play_cursor: 0,
            anim_clip_last_advance: Instant::now(),
            anim_cancel: None,
            anim_gen_epoch: 0,
            bg_key_color: Rgba::opaque(255, 0, 255),
            bg_tolerance: 24,
            bg_preview: false,
            bg_all_frames: false,
            bg_remove_pending: HashSet::new(),
            bg_flagged: Vec::new(),
            bg_status: JobStatus::Idle,
            theme_preference,
            keymap,
            capturing: None,
            settings_open: false,
            settings_tab: SettingsTab::default(),
            pending_zoom: None,
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

    /// Composites the active frame (with onion-skin ghosts when enabled),
    /// caches it as the display frame, and uploads it. `refit` re-fits the
    /// viewport (use on sprite selection, not on playback ticks).
    pub(crate) fn refresh_canvas(&mut self, refit: bool) {
        if let Some(frame) = self.doc.composite_with_onion(&self.editor.onion) {
            self.upload_frame(&frame, refit);
            self.display_frame = Some(frame);
            // Showing the sprite invalidates the wizard's clip-preview marker.
            self.anim_shown = None;
        }
    }

    /// Whether the canvas is showing a reference-sheet preview rather than the
    /// active sprite. Editing tools are suppressed while this is true.
    pub(crate) fn sheet_preview_active(&self) -> bool {
        self.rs_preview.is_some()
    }

    /// Whether the canvas is showing any view-only preview — a reference sheet,
    /// a wizard clip frame, or a background-removal preview. Editing and
    /// overlays are suppressed while any of these is true.
    pub(crate) fn preview_active(&self) -> bool {
        self.sheet_preview_active() || self.clip_preview_active() || self.bg_preview
    }

    /// Shows candidate `i` on the canvas at its native resolution, fit to the
    /// viewport. Non-destructive: the sprite document is untouched. A failed
    /// decode leaves the current view in place.
    pub(crate) fn show_sheet_preview(&mut self, i: usize) {
        let Some(buffer) = self.rs_candidates.get(i).and_then(|c| png_to_pixel_buffer(&c.png)) else {
            return;
        };
        self.upload_frame(&buffer, true);
        self.rs_preview = Some(i);
        self.anim_shown = None;
    }

    /// Returns the canvas to the active sprite if a preview is showing.
    pub(crate) fn exit_sheet_preview(&mut self) {
        if self.rs_preview.take().is_some() {
            self.refresh_canvas(true);
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
                ShellMsg::SheetDone { variants, cost_usd } => {
                    self.cockpit_on_done(variants, cost_usd);
                }
                ShellMsg::SheetFailed(err) => {
                    self.rs_status = JobStatus::Failed(err);
                }
                ShellMsg::ClipProgress { epoch, message } => {
                    if epoch == self.anim_gen_epoch {
                        self.anim_status = JobStatus::Running(message);
                    }
                }
                ShellMsg::ClipReady { epoch, clip, mime, frames } => {
                    if epoch == self.anim_gen_epoch {
                        self.on_clip_ready(clip, mime, frames);
                    }
                }
                ShellMsg::ClipFailed { epoch, error } => {
                    if epoch == self.anim_gen_epoch {
                        self.anim_status = JobStatus::Failed(error);
                        self.anim_cancel = None;
                    }
                }
                ShellMsg::BgRemovalDone { buffer_id, png } => {
                    self.apply_ai_bg_removal(buffer_id, &png);
                }
                ShellMsg::BgRemovalFailed { buffer_id, error } => {
                    self.bg_remove_pending.remove(&buffer_id);
                    self.bg_status = JobStatus::Failed(error);
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
            ui.add(egui::TextEdit::singleline(&mut self.new_sprite_name).hint_text("name").desired_width(120.0));
            if ui.button("New sprite").clicked() {
                let name = if self.new_sprite_name.trim().is_empty() {
                    "untitled".to_owned()
                } else {
                    self.new_sprite_name.trim().to_owned()
                };
                self.exit_sheet_preview();
                self.doc.create_sprite(name, DEFAULT_CANVAS);
                self.new_sprite_name.clear();
                self.refresh_canvas(true);
            }
        });

        ui.separator();

        let items = self.doc.sprite_list();
        let mut select: Option<SpriteRef> = None;
        for item in &items {
            let label = format!("{}  ({}x{})", item.name, item.canvas.width, item.canvas.height);
            if ui.selectable_label(item.selected, label).clicked() {
                select = Some(item.sprite_ref);
            }
        }
        if let Some(sprite_ref) = select {
            self.rs_preview = None;
            self.doc.select(sprite_ref);
            self.playing = false;
            self.refresh_canvas(true);
        }
    }

    /// Starts or stops playback. Starting computes the play order from the
    /// active sprite's tag; a single-frame sprite cannot animate.
    pub(crate) fn toggle_play(&mut self) {
        if self.playing {
            self.playing = false;
            return;
        }
        self.exit_sheet_preview();
        let order = self.doc.active_play_order();
        if order.len() < 2 {
            return;
        }
        self.play_cursor = order.iter().position(|&f| f == self.doc.active_frame).unwrap_or(0);
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
        let dur = Duration::from_millis(u64::from(self.doc.frame_duration_ms(self.doc.active_frame)));
        if self.last_advance.elapsed() >= dur {
            self.play_cursor = (self.play_cursor + 1) % self.play_order.len();
            self.doc.active_frame = self.play_order[self.play_cursor];
            self.last_advance = Instant::now();
            self.refresh_canvas(false);
        }
        ctx.request_repaint_after(dur);
    }

    /// Advances the wizard's clip transport when a clip frame's duration has
    /// elapsed, cycling the current stage's frame subset. Runs from `logic` so
    /// playback continues while the pointer is idle.
    #[allow(clippy::cast_possible_truncation)]
    fn tick_clip_playback(&mut self, ctx: &egui::Context) {
        if !self.anim_clip_playing {
            return;
        }
        if self.anim_play_indices.is_empty() {
            self.anim_clip_playing = false;
            return;
        }
        let fps = self.anim_card().map_or(self.anim_fps, |c| c.fps);
        let dur = Duration::from_millis(u64::from((1000 / fps.max(1)).max(1)));
        if self.anim_clip_last_advance.elapsed() >= dur {
            self.anim_play_cursor = (self.anim_play_cursor + 1) % self.anim_play_indices.len();
            let idx = self.anim_play_indices[self.anim_play_cursor];
            self.set_scrub(idx);
            self.anim_clip_last_advance = Instant::now();
        }
        ctx.request_repaint_after(dur);
    }

    /// Shown in the cockpit when no backend is ready: a pointer to the
    /// AI-backends settings tab where keys are actually entered.
    pub(crate) fn key_entry(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("No generation backend configured.");
            if ui.button("Configure in Settings…").clicked() {
                self.open_settings(SettingsTab::AiBackends);
            }
        });
    }

    /// Opens the settings window on `tab`.
    pub(crate) fn open_settings(&mut self, tab: SettingsTab) {
        self.settings_open = true;
        self.settings_tab = tab;
    }

    /// Stores a key and re-registers backends, updating readiness.
    pub(crate) fn save_key(&mut self, backend: &str, key: &str) {
        match ai::store_key(backend, key) {
            Ok(()) => {
                ai::try_register_openai(&self.verb_runtime);
                ai::try_register_fal(&self.verb_runtime);
                self.recompute_backend_ready();
            }
            Err(err) => self.rs_status = JobStatus::Failed(format!("keychain: {err}")),
        }
    }

    /// Clears a backend's stored key, unregisters it, and recomputes readiness.
    pub(crate) fn clear_backend(&mut self, backend: &str) {
        match ai::clear_key(&self.verb_runtime, backend) {
            Ok(()) => self.recompute_backend_ready(),
            Err(err) => self.rs_status = JobStatus::Failed(format!("keychain: {err}")),
        }
    }

    /// Recomputes [`Self::backend_ready`] from the runtime's registered
    /// backends.
    fn recompute_backend_ready(&mut self) {
        self.backend_ready =
            ai::backend_registered(&self.verb_runtime, ai::OPENAI_BACKEND_ID) || ai::backend_registered(&self.verb_runtime, ai::FAL_BACKEND_ID);
    }

    /// Animation surface: the staged wizard. Generate a clip, scrub the raw
    /// video, mark and preview the loop, pick frames, integrate. Nothing touches
    /// the sprite until Integrate, so Cancel is clean at every step.
    pub(crate) fn animation_tab(&mut self, ui: &mut egui::Ui) {
        if self.doc.active_anchor().is_none() {
            ui.label("Approve a result as an anchor first (Cockpit).");
            return;
        }
        // Keep the canvas showing the selected card's current scrub frame.
        self.sync_clip_preview();

        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            self.anim_prompt_form(ui);
            ui.add_space(10.0);
            self.anim_gallery(ui);
            if self.anim_selected.is_some() {
                ui.add_space(10.0);
                ui.separator();
                self.anim_card_editor(ui);
            }
        });
    }

    /// The selected clip card, if any.
    fn anim_card(&self) -> Option<&ClipCandidate> {
        self.anim_selected.and_then(|i| self.anim_candidates.get(i))
    }

    /// The motion prompt, presets, generation controls, and run status. Generate
    /// appends a clip card to the gallery; nothing here touches the sprite.
    fn anim_prompt_form(&mut self, ui: &mut egui::Ui) {
        ui.label("Motion");
        ui.add(
            egui::TextEdit::singleline(&mut self.anim_motion)
                .hint_text("walk cycle, side view")
                .desired_width(f32::INFINITY),
        );
        ui.horizontal_wrapped(|ui| {
            for preset in ["walk cycle, side view", "idle breathing", "run cycle, side view", "attack swing"] {
                if ui.small_button(preset).clicked() {
                    preset.clone_into(&mut self.anim_motion);
                }
            }
        });
        ui.add(egui::Slider::new(&mut self.anim_target_frames, 2..=16).text("loop frames"));
        ui.add(egui::Slider::new(&mut self.anim_fps, 4..=24).text("fps"));
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.anim_seed_fixed, "Fixed seed")
                .on_hover_text("Pin the RNG seed for a reproducible clip; off uses a random seed each run");
            ui.add_enabled(self.anim_seed_fixed, egui::DragValue::new(&mut self.anim_seed));
        });

        let generating = matches!(self.anim_status, JobStatus::Running(_));
        if ui
            .add_enabled(self.backend_ready && !generating, egui::Button::new(format!("{} Generate clip", crate::icons::FILM)))
            .clicked()
        {
            self.start_clip();
        }
        if !self.backend_ready {
            ui.colored_label(egui::Color32::LIGHT_YELLOW, "No generation backend configured.");
        }
        match &self.anim_status {
            JobStatus::Running(m) => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(m.clone());
                });
            }
            JobStatus::Failed(e) => {
                ui.colored_label(egui::Color32::LIGHT_RED, format!("failed: {e}"));
            }
            JobStatus::Idle => {}
        }
        if generating && ui.button("Cancel").clicked() {
            self.cancel_clip();
        }

        if !self.anim_recent_motions.is_empty() {
            let recent = self.anim_recent_motions.clone();
            let mut pick: Option<String> = None;
            egui::CollapsingHeader::new(format!("{} Recent motions", crate::icons::UNDO))
                .id_salt("anim_history")
                .show(ui, |ui| {
                    for m in &recent {
                        if ui
                            .add(egui::Label::new(egui::RichText::new(truncate_motion(m, 48)).small()).sense(egui::Sense::click()))
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

    /// The clip-card gallery, newest first. Clicking a card selects it for
    /// editing and drives the canvas preview from its frames.
    #[allow(clippy::cast_precision_loss)]
    fn anim_gallery(&mut self, ui: &mut egui::Ui) {
        if self.anim_candidates.is_empty() {
            ui.label(egui::RichText::new("Generated clips appear here as cards.").small().weak());
            return;
        }
        ui.label(egui::RichText::new("Clips").strong());
        let ctx = ui.ctx().clone();
        let mut select: Option<usize> = None;
        let mut remove: Option<usize> = None;
        for i in (0..self.anim_candidates.len()).rev() {
            if self.anim_candidates[i].card_texture.is_none() {
                if let Some(frame) = self.anim_candidates[i].frames.first() {
                    let tex = video_frame_to_texture(&ctx, frame);
                    self.anim_candidates[i].card_texture = Some(tex);
                }
            }
            let selected = self.anim_selected == Some(i);
            let cand = &self.anim_candidates[i];
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.horizontal(|ui| {
                    if let Some(tex) = &cand.card_texture {
                        let size = tex.size_vec2();
                        let scale = (72.0 / size.x.max(1.0)).min(1.0);
                        if ui.add(egui::Button::image((tex.id(), size * scale))).clicked() {
                            select = Some(i);
                        }
                    }
                    ui.vertical(|ui| {
                        if ui.selectable_label(selected, egui::RichText::new(truncate_motion(&cand.motion, 36)).strong()).clicked() {
                            select = Some(i);
                        }
                        ui.label(egui::RichText::new(format!("{} frames · {} fps", cand.frames.len(), cand.fps)).small().weak());
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

    /// The selected card's editor: provenance, play-mode, transport, loop
    /// markers, frame picks, and the Integrate / Re-roll actions.
    #[allow(clippy::cast_possible_truncation)]
    fn anim_card_editor(&mut self, ui: &mut egui::Ui) {
        let Some(i) = self.anim_selected else {
            return;
        };
        let n = self.anim_candidates[i].frames.len();
        if n == 0 {
            return;
        }
        let last = (n - 1) as u32;

        {
            let c = &self.anim_candidates[i];
            let seed = c.seed.map_or_else(|| "random".to_owned(), |s| s.to_string());
            ui.label(
                egui::RichText::new(format!(
                    "\"{}\" — {} frames · {} fps · seed {seed}",
                    truncate_motion(&c.motion, 36),
                    c.frames.len(),
                    c.fps
                ))
                .small()
                .weak(),
            );
            // Source clip provenance: the raw bytes kept for a future export.
            ui.label(
                egui::RichText::new(format!("source: {} · {} KB", c.mime, c.clip.len() / 1024))
                    .small()
                    .weak(),
            );
        }

        ui.horizontal(|ui| {
            ui.label("Play");
            ui.selectable_value(&mut self.anim_play_mode, AnimPlayMode::Clip, "Clip");
            ui.selectable_value(&mut self.anim_play_mode, AnimPlayMode::Loop, "Loop");
            ui.selectable_value(&mut self.anim_play_mode, AnimPlayMode::Picks, "Picks");
        });
        self.anim_transport(ui);

        ui.label(egui::RichText::new("Loop — Play cycles only the marked span").small().weak());
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

        ui.label(egui::RichText::new("Frames — the picks that land on the timeline").small().weak());
        let mut count = self.anim_target_frames;
        if ui.add(egui::Slider::new(&mut count, 2..=16).text("count")).changed() {
            self.anim_target_frames = count;
            let markers = self.anim_candidates[i].markers;
            self.anim_candidates[i].picks = anim::pick_loop_frames(&self.anim_candidates[i].frames, markers, count as usize);
        }
        let picks_len = self.anim_candidates[i].picks.len();
        if picks_len < self.anim_target_frames as usize {
            ui.colored_label(
                egui::Color32::LIGHT_YELLOW,
                "The clip can't supply that many distinct frames — widen the loop or lower the count.",
            );
        }
        let cur = self.anim_scrub;
        let included = self.anim_candidates[i].picks.contains(&cur);
        if ui.button(if included { "Remove current frame" } else { "Add current frame" }).clicked() {
            self.toggle_pick(cur);
        }
        self.pick_strip(ui);

        ui.separator();
        ui.horizontal(|ui| {
            if ui.add_enabled(picks_len > 0, egui::Button::new(format!("{} Integrate", crate::icons::CHECK))).clicked() {
                self.integrate_picked();
            }
            if ui
                .button(format!("{} Re-roll", crate::icons::DICE))
                .on_hover_text("Generate again from this motion with a new random seed")
                .clicked()
            {
                self.reroll_clip(i);
            }
        });
    }

    /// Selects clip card `i` for editing and resets the scrub/preview to it.
    fn select_clip(&mut self, i: usize) {
        self.anim_selected = Some(i);
        self.anim_scrub = 0;
        self.anim_shown = None;
        self.anim_clip_playing = false;
        self.sync_clip_preview();
    }

    /// Discards clip card `i`, fixing the selection and the canvas preview.
    fn remove_clip(&mut self, i: usize) {
        if i >= self.anim_candidates.len() {
            return;
        }
        self.anim_candidates.remove(i);
        self.anim_clip_playing = false;
        self.anim_shown = None;
        self.anim_selected = match self.anim_selected {
            Some(s) if s == i => None,
            Some(s) if s > i => Some(s - 1),
            other => other,
        };
        if self.anim_selected.is_none() {
            self.exit_clip_preview();
        } else {
            self.anim_scrub = 0;
            self.sync_clip_preview();
        }
    }

    /// Re-runs generation from card `i`'s motion with a fresh random seed; the
    /// result lands as a new card.
    fn reroll_clip(&mut self, i: usize) {
        let motion = self.anim_candidates[i].motion.clone();
        self.anim_motion = motion;
        self.anim_seed_fixed = false;
        self.start_clip();
    }

    /// Records `motion` at the head of the recent list, de-duplicated and capped.
    fn record_recent_motion(&mut self, motion: &str) {
        let m = motion.trim();
        if m.is_empty() {
            return;
        }
        self.anim_recent_motions.retain(|x| x != m);
        self.anim_recent_motions.insert(0, m.to_owned());
        self.anim_recent_motions.truncate(8);
    }

    /// Shared scrub track + transport across Review, Mark loop, and Pick frames.
    /// Play cycles whatever subset the current stage defines.
    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::cast_possible_truncation)]
    fn anim_transport(&mut self, ui: &mut egui::Ui) {
        let n = self.anim_card().map_or(0, |c| c.frames.len());
        if n == 0 {
            return;
        }
        let last = (n - 1) as u32;
        ui.horizontal(|ui| {
            let label = if self.anim_clip_playing { crate::icons::PAUSE } else { crate::icons::PLAY };
            if ui.button(label).on_hover_text("Play / pause").clicked() {
                self.toggle_clip_play();
            }
            let mut scrub = self.anim_scrub.min(n - 1) as u32;
            if ui.add(egui::Slider::new(&mut scrub, 0..=last).text("frame")).changed() {
                self.anim_clip_playing = false;
                self.set_scrub(scrub as usize);
            }
        });
    }

    /// The picked-frame thumbnail strip for the selected card; click to drop.
    #[allow(clippy::cast_precision_loss)]
    fn pick_strip(&mut self, ui: &mut egui::Ui) {
        let Some(i) = self.anim_selected else {
            return;
        };
        if self.anim_candidates[i].picks.is_empty() {
            return;
        }
        let ctx = ui.ctx().clone();
        let picks = self.anim_candidates[i].picks.clone();
        // Build any missing thumbnails for the picked frames.
        for &p in &picks {
            let needs = self.anim_candidates[i].thumbs.get(p).is_some_and(Option::is_none);
            if needs {
                if let Some(frame) = self.anim_candidates[i].frames.get(p) {
                    let tex = video_frame_to_texture(&ctx, frame);
                    if let Some(slot) = self.anim_candidates[i].thumbs.get_mut(p) {
                        *slot = Some(tex);
                    }
                }
            }
        }
        // Snapshot what to draw so the closure borrows nothing mutable from self.
        let thumbs: Vec<(usize, egui::TextureId, egui::Vec2)> = picks
            .iter()
            .filter_map(|&p| self.anim_candidates[i].thumbs.get(p).and_then(|t| t.as_ref()).map(|tex| (p, tex.id(), tex.size_vec2())))
            .collect();

        let mut remove: Option<usize> = None;
        egui::ScrollArea::horizontal().show(ui, |ui| {
            ui.horizontal(|ui| {
                for (p, id, size) in thumbs {
                    let scale = (64.0 / size.x.max(1.0)).min(1.0);
                    let image = egui::Image::new((id, size * scale)).sense(egui::Sense::click());
                    if ui.add(image).on_hover_text("Click to drop this frame").clicked() {
                        remove = Some(p);
                    }
                }
            });
        });
        if let Some(p) = remove {
            self.toggle_pick(p);
        }
    }

    /// Adds or removes `i` from the selected card's picked set, keeping it sorted.
    fn toggle_pick(&mut self, i: usize) {
        let Some(c) = self.anim_selected.and_then(|s| self.anim_candidates.get_mut(s)) else {
            return;
        };
        if let Some(pos) = c.picks.iter().position(|&x| x == i) {
            c.picks.remove(pos);
        } else {
            c.picks.push(i);
            c.picks.sort_unstable();
        }
    }

    /// Starts or stops clip playback for the current stage's frame subset.
    fn toggle_clip_play(&mut self) {
        if self.anim_clip_playing {
            self.anim_clip_playing = false;
            return;
        }
        let indices = self.current_play_indices();
        if indices.is_empty() {
            return;
        }
        self.anim_play_cursor = 0;
        let first = indices[0];
        self.anim_play_indices = indices;
        self.anim_clip_playing = true;
        self.anim_clip_last_advance = Instant::now();
        self.set_scrub(first);
    }

    /// The frame indices the selected card's Play cycles, per the play mode:
    /// the whole clip, the marked `[start, end)`, or the picks.
    fn current_play_indices(&self) -> Vec<usize> {
        let Some(c) = self.anim_card() else {
            return Vec::new();
        };
        let n = c.frames.len();
        if n == 0 {
            return Vec::new();
        }
        match self.anim_play_mode {
            AnimPlayMode::Loop => {
                let lo = c.markers.start.min(n - 1);
                let hi = c.markers.end.clamp(lo + 1, n);
                (lo..hi).collect()
            }
            AnimPlayMode::Picks => c.picks.clone(),
            AnimPlayMode::Clip => (0..n).collect(),
        }
    }

    /// Kicks off a clip generation on the tokio runtime, with a cancel
    /// handle stored so the Cancel button can abort it.
    fn start_clip(&mut self) {
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
            seed: self.anim_seed_fixed.then_some(self.anim_seed),
        };
        self.record_recent_motion(&self.anim_motion.clone());
        let cancel = CancellationToken::new();
        self.anim_cancel = Some(cancel.clone());
        self.anim_gen_epoch += 1;
        let epoch = self.anim_gen_epoch;
        self.anim_status = JobStatus::Running("starting".into());
        ai::spawn_clip(
            self.runtime.handle(),
            self.verb_runtime.clone(),
            self.egui_ctx.clone(),
            self.tx.clone(),
            job,
            cancel,
            epoch,
        );
    }

    /// Spawns an AI background removal of `buffer_id` (already encoded to PNG)
    /// on the tokio runtime. The stripped result returns over the channel as
    /// [`ShellMsg::BgRemovalDone`].
    pub(crate) fn spawn_ai_bg_removal(&mut self, buffer_id: PixelBufferId, png: Vec<u8>) {
        let cancel = CancellationToken::new();
        self.bg_remove_pending.insert(buffer_id);
        self.bg_status = JobStatus::Running("removing background (AI)".into());
        ai::spawn_bg_removal(
            self.runtime.handle(),
            self.verb_runtime.clone(),
            self.egui_ctx.clone(),
            self.tx.clone(),
            buffer_id,
            png,
            cancel,
        );
    }

    /// Cancels a running clip generation and clears the busy status.
    fn cancel_clip(&mut self) {
        if let Some(cancel) = self.anim_cancel.take() {
            cancel.cancel();
        }
        // Bump the epoch so the aborted job's late messages are dropped.
        self.anim_gen_epoch += 1;
        self.anim_status = JobStatus::Idle;
    }

    /// Receives a generated clip: lands it as a new gallery card with seeded
    /// loop markers and picks, then selects it for editing.
    fn on_clip_ready(&mut self, clip: Vec<u8>, mime: String, frames: Vec<VideoFrame>) {
        self.anim_status = JobStatus::Idle;
        self.anim_cancel = None;
        let markers = anim::auto_loop_markers(&frames);
        let picks = anim::pick_loop_frames(&frames, markers, self.anim_target_frames as usize);
        let thumbs = vec![None; frames.len()];
        self.anim_candidates.push(ClipCandidate {
            clip,
            mime,
            thumbs,
            markers,
            picks,
            motion: self.anim_motion.clone(),
            fps: self.anim_fps,
            seed: self.anim_seed_fixed.then_some(self.anim_seed),
            card_texture: None,
            frames,
        });
        let idx = self.anim_candidates.len() - 1;
        self.select_clip(idx);
    }

    /// Normalizes and integrates the selected card's picked frames onto the
    /// timeline. The frames still carry their background — removal is a separate
    /// timeline op. The clip stays in the gallery; the canvas returns to the sprite.
    fn integrate_picked(&mut self) {
        let Some(i) = self.anim_selected else {
            return;
        };
        if self.anim_candidates[i].picks.is_empty() {
            return;
        }
        let Some(sprite) = self.doc.active_sprite() else {
            return;
        };
        let (cw, ch) = (sprite.canvas.width, sprite.canvas.height);
        let buffers: Vec<PixelBuffer> = self.anim_candidates[i]
            .picks
            .iter()
            .filter_map(|&p| self.anim_candidates[i].frames.get(p))
            .filter_map(video_frame_to_pixel_buffer)
            .collect();
        if buffers.is_empty() {
            return;
        }
        let opts = NormalizeOptions {
            canvas_width: cw,
            canvas_height: ch,
            alpha_threshold: 8,
            chroma: None,
            reference_height: None,
            bottom_margin: 0,
        };
        let frames = match normalize_frames(&buffers, &opts) {
            Ok(result) => result.frames,
            Err(err) => {
                self.anim_status = JobStatus::Failed(format!("normalize: {err}"));
                return;
            }
        };
        let fps = self.anim_candidates[i].fps;
        let frame_ms = (1000 / fps.max(1)).max(1);
        let motion = self.anim_candidates[i].motion.clone();
        self.doc.integrate_frames(frames, frame_ms, &motion, LoopDirection::Forward);
        // Drop the canvas preview back to the sprite/timeline; keep the gallery.
        self.anim_selected = None;
        self.anim_clip_playing = false;
        self.exit_clip_preview();
    }

    /// Moves the scrub cursor within the selected card and re-shows that frame.
    fn set_scrub(&mut self, idx: usize) {
        let n = self.anim_card().map_or(0, |c| c.frames.len());
        self.anim_scrub = idx.min(n.saturating_sub(1));
        self.sync_clip_preview();
    }

    /// Whether a selected clip card is driving the canvas with one of its frames.
    fn clip_preview_active(&self) -> bool {
        self.workspace == Workspace::Create
            && self.create_view == CreateView::Animate
            && self.anim_card().is_some_and(|c| !c.frames.is_empty())
    }

    /// Stops clip playback and drops the clip preview from the canvas — used
    /// when the Create dock leaves the Animate surface.
    pub(crate) fn leave_animation_preview(&mut self) {
        self.anim_clip_playing = false;
        self.exit_clip_preview();
    }

    /// Uploads the current scrub frame when the wizard owns the canvas and the
    /// shown frame is stale. Re-fits only on the first show.
    fn sync_clip_preview(&mut self) {
        if !self.clip_preview_active() {
            return;
        }
        let Some(i) = self.anim_selected else {
            return;
        };
        let n = self.anim_candidates[i].frames.len();
        let idx = self.anim_scrub.min(n.saturating_sub(1));
        if self.anim_shown == Some(idx) {
            return;
        }
        if let Some(buffer) = self.anim_candidates[i].frames.get(idx).and_then(video_frame_to_pixel_buffer) {
            let refit = self.anim_shown.is_none();
            self.upload_frame(&buffer, refit);
            self.anim_shown = Some(idx);
        }
    }

    /// Reverts the canvas to the sprite if a clip frame is showing.
    fn exit_clip_preview(&mut self) {
        if self.anim_shown.take().is_some() {
            self.refresh_canvas(true);
        }
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
                if ui.button("Settings…").clicked() {
                    self.open_settings(SettingsTab::General);
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

            ui.separator();

            // Workspace segmented control: Draw / Animate / ✨ Create.
            if ui.selectable_label(self.workspace == Workspace::Draw, "Draw").clicked() {
                self.set_workspace(Workspace::Draw);
            }
            if ui.selectable_label(self.workspace == Workspace::Animate, "Animate").clicked() {
                self.set_workspace(Workspace::Animate);
            }
            if ui
                .selectable_label(self.workspace == Workspace::Create, format!("{} Create", crate::icons::SPARKLE))
                .clicked()
            {
                self.set_workspace(Workspace::Create);
            }

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

    /// The right-side dock: Colour/Layers/Sprites tabs in Draw and Animate, the
    /// AI surface in Create.
    fn dock_panel(&mut self, ui: &mut egui::Ui) {
        if self.workspace == Workspace::Create {
            self.create_dock(ui);
            return;
        }
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.right_tab, RightTab::Color, format!("{} Colour", crate::icons::PALETTE));
            ui.selectable_value(&mut self.right_tab, RightTab::Layers, format!("{} Layers", crate::icons::LAYERS));
            ui.selectable_value(&mut self.right_tab, RightTab::Sprites, "Sprites");
        });
        ui.separator();
        match self.right_tab {
            RightTab::Color => self.palette_panel(ui),
            RightTab::Layers => self.layers_panel(ui),
            RightTab::Sprites => self.library_panel(ui),
        }
    }

    /// Applies and remembers a new theme preference.
    pub(crate) fn set_theme_preference(&mut self, ctx: &egui::Context, preference: egui::ThemePreference) {
        self.theme_preference = preference;
        ctx.set_theme(preference);
    }

    /// Switches workspace mode, expanding the timeline for Animate and showing
    /// the AI surface for Create.
    fn set_workspace(&mut self, workspace: Workspace) {
        if workspace != Workspace::Create {
            self.exit_sheet_preview();
            self.anim_clip_playing = false;
            self.exit_clip_preview();
        }
        self.workspace = workspace;
        match workspace {
            Workspace::Animate => self.timeline_expanded = true,
            Workspace::Draw | Workspace::Create => {}
        }
    }

    /// Bottom status bar: a single muted line of document and session state.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // preview dims are small positive integers
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
            if let (true, Some([w, h])) = (self.sheet_preview_active(), self.frame_size) {
                ui.separator();
                ui.label(muted(format!("sheet {}x{}", w as u32, h as u32)));
            } else if let Some(sprite) = self.doc.active_sprite() {
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

impl ShellApp {
    /// Undoes the last edit and refreshes the canvas.
    pub(crate) fn do_undo(&mut self) {
        if self.editor.history.undo(&mut self.doc).is_ok() {
            self.refresh_canvas(false);
        }
    }

    /// Redoes the next edit and refreshes the canvas.
    pub(crate) fn do_redo(&mut self) {
        if self.editor.history.redo(&mut self.doc).is_ok() {
            self.refresh_canvas(false);
        }
    }

    /// Reads the pressed keys, resolves them to commands through the keymap, and
    /// dispatches each. Consumed before panels lay out so a shortcut never
    /// double-fires through a focused widget.
    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        // Skip when a text field has focus so typing a name doesn't switch tools.
        if ctx.memory(|m| m.focused().is_some()) {
            return;
        }
        // Collect inside the input closure (no `&mut self` there), dispatch after.
        let matched: Vec<CommandId> = ctx.input(|i| {
            CommandId::ALL
                .iter()
                .copied()
                .filter(|&command| {
                    self.keymap
                        .resolve(command)
                        .is_some_and(|chord| chord.modifiers_match(i.modifiers) && i.key_pressed(chord.key))
                })
                .collect()
        });
        for command in matched {
            self.dispatch(command);
        }
    }

    /// Performs a command resolved from a keyboard chord. The single place a
    /// [`CommandId`] turns into an action.
    fn dispatch(&mut self, command: CommandId) {
        use crate::editor::Tool;

        match command {
            CommandId::NewSprite => {
                self.doc.create_sprite("untitled", DEFAULT_CANVAS);
                self.refresh_canvas(true);
            }
            CommandId::Undo => self.do_undo(),
            CommandId::Redo => self.do_redo(),
            CommandId::SwapColors => self.editor.swap_colors(),
            CommandId::BrushSizeDecrease => self.editor.brush_size = self.editor.brush_size.saturating_sub(1).max(1),
            CommandId::BrushSizeIncrease => self.editor.brush_size = (self.editor.brush_size + 1).min(256),
            CommandId::Deselect => self.editor.clear_selection(),
            CommandId::DeleteSelection => self.delete_selection(),
            CommandId::ZoomIn => self.pending_zoom = Some(ZoomAction::In),
            CommandId::ZoomOut => self.pending_zoom = Some(ZoomAction::Out),
            CommandId::ZoomFit => self.needs_fit = true,
            CommandId::ZoomReset => self.pending_zoom = Some(ZoomAction::Reset),
            CommandId::PlayPause => self.toggle_play(),
            CommandId::ToolPencil => self.editor.left_tool = Tool::Pencil,
            CommandId::ToolEraser => self.editor.left_tool = Tool::Eraser,
            CommandId::ToolFill => self.editor.left_tool = Tool::Fill,
            CommandId::ToolLine => self.editor.left_tool = Tool::Line,
            CommandId::ToolRectangle => self.editor.left_tool = Tool::Rectangle,
            CommandId::ToolEllipse => self.editor.left_tool = Tool::Ellipse,
            CommandId::ToolPicker => self.editor.left_tool = Tool::Picker,
            CommandId::ToolSelectRect => self.editor.left_tool = Tool::SelectRect,
            CommandId::ToolLasso => self.editor.left_tool = Tool::Lasso,
            CommandId::ToolWand => self.editor.left_tool = Tool::Wand,
            CommandId::ToolMove => self.editor.left_tool = Tool::Move,
            CommandId::OpenSettings => self.open_settings(SettingsTab::General),
        }
    }

    /// Clears the selected pixels on the active cel (the Delete shortcut) and
    /// records an undo entry.
    fn delete_selection(&mut self) {
        let Some(mask) = self.editor.selection.clone() else {
            return;
        };
        let Some(buffer_id) = self.doc.ensure_drawable() else {
            return;
        };
        let Some(before) = self.doc.pixel_buffers.get(&buffer_id).cloned() else {
            return;
        };
        if let Some(buf) = self.doc.pixel_buffers.get_mut(&buffer_id) {
            for y in 0..buf.height() {
                for x in 0..buf.width() {
                    if mask.is_selected(x, y) {
                        buf.set_pixel(x, y, pixhaus_core::project::Rgba::transparent());
                    }
                }
            }
        }
        let b = mask.bounds();
        if !b.is_empty() {
            #[allow(clippy::cast_sign_loss)]
            self.finish_pixel_edit(
                &before,
                buffer_id,
                b.origin.x.max(0) as u32,
                b.origin.y.max(0) as u32,
                b.size.width,
                b.size.height,
                "Delete selection",
            );
        }
        self.refresh_canvas(false);
    }
}

/// Truncates a motion prompt for a label, appending an ellipsis when clipped.
fn truncate_motion(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_owned()
    } else {
        let head: String = s.chars().take(max.saturating_sub(1)).collect();
        format!("{head}…")
    }
}

/// Decodes PNG bytes into a tightly packed RGBA [`PixelBuffer`] for display on
/// the wgpu canvas. Returns `None` on a decode failure or an invalid size.
fn png_to_pixel_buffer(png: &[u8]) -> Option<PixelBuffer> {
    let rgba = image::load_from_memory(png).ok()?.to_rgba8();
    let (width, height) = (rgba.width(), rgba.height());
    PixelBuffer::from_raw(width, height, width * 4, rgba.into_raw()).ok()
}

/// Converts a decoded clip frame into a [`PixelBuffer`] for the canvas, reusing
/// the exact mechanism the reference-sheet preview uses.
fn video_frame_to_pixel_buffer(frame: &VideoFrame) -> Option<PixelBuffer> {
    PixelBuffer::from_raw(frame.width, frame.height, frame.width * 4, frame.pixels.clone()).ok()
}

/// Loads a clip frame as a NEAREST-sampled egui texture for the pick strip.
fn video_frame_to_texture(ctx: &egui::Context, frame: &VideoFrame) -> egui::TextureHandle {
    let size = [frame.width as usize, frame.height as usize];
    let image = egui::ColorImage::from_rgba_unmultiplied(size, &frame.pixels);
    ctx.load_texture("anim_thumb", image, egui::TextureOptions::NEAREST)
}

impl eframe::App for ShellApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.drain_results() {
            ctx.request_repaint();
        }
        self.tick_playback(ctx);
        self.tick_clip_playback(ctx);
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, "theme_preference", &self.theme_preference);
        eframe::set_value(storage, "keymap", &self.keymap);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.handle_shortcuts(ui.ctx());
        self.show_settings_window(ui.ctx());

        // Panel order matters: outer panels first, the central canvas last so
        // it fills the space the others leave. Two top strips (menu, then the
        // tool context bar); two bottom strips (status at the edge, timeline
        // above it).
        egui::Panel::top("menu_bar").show_inside(ui, |ui| self.menu_bar(ui));
        egui::Panel::top("context_bar").resizable(false).show_inside(ui, |ui| self.context_bar(ui));

        egui::Panel::bottom("status_bar").resizable(false).show_inside(ui, |ui| self.status_bar(ui));

        // Slim icon tool strip on the far left.
        egui::Panel::left("tools")
            .resizable(false)
            .exact_size(48.0)
            .show_inside(ui, |ui| self.tools_panel(ui));

        // One side dock on the right: Colour/Layers/Sprites tabs in Draw and
        // Animate, the AI surface in Create.
        egui::Panel::right("dock")
            .resizable(true)
            .default_size(300.0)
            .show_inside(ui, |ui| self.dock_panel(ui));

        // Collapsible timeline: a slim transport line, or the full cel matrix.
        egui::Panel::bottom("timeline")
            .resizable(self.timeline_expanded)
            .default_size(if self.timeline_expanded { 240.0 } else { 40.0 })
            .show_inside(ui, |ui| self.timeline_dock(ui));

        egui::CentralPanel::default().show_inside(ui, |ui| {
            self.canvas_ui(ui);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::{png_to_pixel_buffer, truncate_motion};

    #[test]
    fn truncate_motion_keeps_short_and_ellipsizes_long() {
        assert_eq!(truncate_motion("walk", 10), "walk");
        // Exactly at the limit is kept whole.
        assert_eq!(truncate_motion("0123456789", 10), "0123456789");
        // Over the limit clips to max chars including the ellipsis.
        let out = truncate_motion("0123456789ABC", 10);
        assert!(out.ends_with('…'));
        assert_eq!(out.chars().count(), 10);
    }

    /// A 2x2 PNG with a known opaque-red top-left pixel, encoded in memory.
    fn red_corner_png() -> Vec<u8> {
        let mut img = image::RgbaImage::new(2, 2);
        img.put_pixel(0, 0, image::Rgba([255, 0, 0, 255]));
        let mut bytes = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png)
            .expect("encode test png");
        bytes
    }

    #[test]
    fn png_to_pixel_buffer_decodes_dims_and_pixels() {
        let buffer = png_to_pixel_buffer(&red_corner_png()).expect("decode succeeds");
        assert_eq!((buffer.width(), buffer.height()), (2, 2));
        let top_left = buffer.pixel(0, 0).expect("top-left pixel");
        assert_eq!((top_left.r, top_left.g, top_left.b, top_left.a), (255, 0, 0, 255));
    }

    #[test]
    fn png_to_pixel_buffer_rejects_garbage() {
        assert!(png_to_pixel_buffer(b"not a png").is_none());
    }
}
