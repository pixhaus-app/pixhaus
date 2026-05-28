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
use pixhaus_ai::plugin::{PixelData, VerbRuntime};
use pixhaus_core::canvas::PixelBuffer;
use pixhaus_core::project::{CelData, ColorMode, FrameIndex, GroupId, LoopDirection, PixelBufferId, Rgba, Size, Sprite, SpriteId};
use pixhaus_core::transforms::CanvasAnchor;
use pixhaus_core::transforms::normalize::{NormalizeOptions, normalize_frames};
use pixhaus_render::{Viewport, ViewportRenderer};
use tokio::runtime::Runtime;
use tokio_util::sync::CancellationToken;

use crate::ai;
use crate::anim::{self, LoopMarkers, VideoFrame};
use crate::cockpit::{CockpitCandidate, CockpitReference, PendingLineage};
use crate::commands::{CanvasBufferSwap, CanvasEdit, CanvasOp, integrate_frames_undoable};
use crate::document::{DocumentStore, LibraryRow, SpriteRef};
use crate::keymap::{CommandId, Keymap};
use crate::settings::SettingsTab;

/// Default canvas size for a newly created sprite.
const DEFAULT_CANVAS: Size = Size { width: 64, height: 64 };

/// Largest canvas dimension on a side. Matches wgpu's default
/// `max_texture_dimension_2d` of 8192 — a single texture larger than this needs
/// tiling, which is out of scope. The size inputs clamp to this ceiling.
const MAX_CANVAS_DIM: u32 = 8192;

/// Pixel-count threshold above which a canvas op runs on a blocking thread
/// rather than inline on the UI thread. Below it the work is instant; above it
/// (~1 megapixel) a full-buffer pass is worth keeping off the frame loop.
const TRANSFORM_OFFLOAD_PIXELS: u64 = 1024 * 1024;

/// Common canvas sizes offered in the new-sprite and resize dialogs: square
/// pixel-art presets, then a few game/sheet sizes. A `Custom…` entry covers
/// anything else up to [`MAX_CANVAS_DIM`].
const SIZE_PRESETS: &[(&str, u32, u32)] = &[
    ("16 x 16", 16, 16),
    ("32 x 32", 32, 32),
    ("64 x 64", 64, 64),
    ("128 x 128", 128, 128),
    ("256 x 256", 256, 256),
    ("512 x 512", 512, 512),
    ("1024 x 1024", 1024, 1024),
    ("320 x 180", 320, 180),
    ("640 x 360", 640, 360),
    ("1920 x 1080", 1920, 1080),
];

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
    /// A streamed partial preview frame for the in-flight reference-sheet
    /// generation: a progressively sharper image to paint on the canvas before
    /// the final candidates land.
    SheetPartial {
        /// Decoded RGBA preview pixels.
        pixels: PixelData,
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
    /// First-frame (studio seed-pose) generation progress.
    FirstFrameProgress {
        /// Generation epoch this message belongs to; stale ones are dropped.
        epoch: u64,
        /// One-line status.
        message: String,
    },
    /// First-frame generation finished: one or more candidate images, as PNG
    /// bytes, to land in the first-frame thread.
    FirstFrameDone {
        /// Generation epoch this result belongs to; stale ones are dropped.
        epoch: u64,
        /// Candidate images as PNG bytes.
        images: Vec<Vec<u8>>,
        /// Thread index this batch descends from (an inpaint's source), or
        /// `None` for a from-scratch text-to-image batch.
        parent: Option<usize>,
        /// Whether the batch appends to the thread (refinement) or replaces it
        /// (a fresh generation).
        append: bool,
    },
    /// First-frame generation failed (or was canceled).
    FirstFrameFailed {
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
    /// A heavy canvas transform finished off-thread. Boxed because the payload
    /// (a whole sprite plus buffers) dwarfs every other message.
    TransformDone(Box<TransformResult>),
    /// A keychain key-op finished off-thread: refreshed per-backend configured
    /// state, overall readiness, and any keychain error to surface.
    BackendsRefreshed {
        /// Whether an `OpenAI` key is stored.
        openai_configured: bool,
        /// Whether a FAL key is stored.
        fal_configured: bool,
        /// Whether at least one backend is registered and ready.
        ready: bool,
        /// Keychain error from the op, if any.
        error: Option<String>,
    },
}

/// Result of an off-thread canvas transform, carried by
/// [`ShellMsg::TransformDone`]. Holds the inputs the transform ran against so
/// the UI thread can detect a document change that happened meanwhile.
#[derive(Debug)]
pub struct TransformResult {
    /// Sprite the transform ran against.
    pub sprite_id: SpriteId,
    /// Sprite value captured before the transform.
    pub before_sprite: Sprite,
    /// Buffers (id + bytes) the transform ran against.
    pub before_buffers: Vec<(PixelBufferId, PixelBuffer)>,
    /// Transformed buffers, in the same order, or `None` if the op failed.
    pub after_buffers: Option<Vec<(PixelBufferId, PixelBuffer)>>,
    /// Resulting canvas size.
    pub new_size: Size,
    /// History label.
    pub label: String,
}

/// What the selected clip card's Play button cycles.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum AnimPlayMode {
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
    /// The gallery index this clip was re-rolled or branched from, if any —
    /// the clip's lineage, mirroring `CockpitCandidate::parent`, so the studio
    /// can show where a clip came from.
    pub parent: Option<usize>,
    /// Lazily-built first-frame thumbnail for the gallery card.
    pub card_texture: Option<egui::TextureHandle>,
    /// Lazily-built chroma-keyed preview textures, one slot per decoded frame,
    /// for the studio's "show the clip keyed" toggle. Cleared when the key the
    /// cache was built for ([`Self::keyed_sig`]) no longer matches.
    pub keyed_thumbs: Vec<Option<egui::TextureHandle>>,
    /// The `(key colour, tolerance)` the keyed thumbnails were built for, so a
    /// key change invalidates them.
    pub keyed_sig: Option<(Rgba, u8)>,
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
    /// Whether the bottom timeline shows the full cel matrix (vs a slim
    /// transport line).
    pub(crate) timeline_expanded: bool,
    /// The verb runtime (reference-sheet verb + FAL backend).
    pub(crate) verb_runtime: Arc<VerbRuntime>,
    /// Whether a generation backend is registered and ready.
    pub(crate) backend_ready: bool,
    /// Cached "`OpenAI` key is stored" flag, refreshed off-thread after a save
    /// or clear so the settings panel never reads the keychain on the UI thread.
    pub(crate) openai_key_configured: bool,
    /// Cached "FAL key is stored" flag; see [`Self::openai_key_configured`].
    pub(crate) fal_key_configured: bool,
    /// Set while a heavy canvas transform runs off-thread, to block re-entry and
    /// to disable the transform controls until the result lands.
    pub(crate) transform_in_flight: bool,
    /// Adapter currently driving the canvas, read from the live render state.
    pub(crate) active_adapter: Option<wgpu::AdapterInfo>,
    /// Every adapter wgpu enumerated at startup, for the Settings GPU picker.
    pub(crate) available_adapters: Vec<wgpu::AdapterInfo>,
    /// Saved GPU preference (the pinned adapter), or `None` for automatic. Takes
    /// effect on the next launch.
    pub(crate) gpu_pref: Option<crate::gpu::GpuPreference>,
    /// egui context clone handed to background tasks to wake the idle UI.
    pub(crate) egui_ctx: egui::Context,
    /// Whether the composition-library overlay is open over the studio.
    pub(crate) studio_library_open: bool,
    /// Which composition-library tab shows in the Library view.
    pub(crate) library_tab: crate::library::LibraryTab,
    /// The composition-library record currently open in the editor, if any.
    /// Acts as the browser's selection: the row whose id matches is highlighted.
    pub(crate) library_draft: Option<crate::library::LibraryDraft>,
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
    /// Whether the canvas is showing a streamed partial-generation frame. Set
    /// while a reference-sheet run streams previews, before any candidate
    /// exists to index via [`Self::rs_preview`].
    pub(crate) rs_partial_preview: bool,
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
    pub(crate) anim_motion: String,
    /// Requested loop frame count.
    pub(crate) anim_target_frames: u32,
    /// Playback FPS for the generated animation.
    pub(crate) anim_fps: u32,
    /// Whether the Generate stage pins a fixed RNG seed (vs random each run).
    pub(crate) anim_seed_fixed: bool,
    /// The fixed seed value, used when [`Self::anim_seed_fixed`] is set.
    pub(crate) anim_seed: u64,
    /// Generate-stage job status (progress / failure).
    pub(crate) anim_status: JobStatus,
    /// Generated clips, oldest first; the gallery lists them newest-first.
    pub(crate) anim_candidates: Vec<ClipCandidate>,
    /// Index into [`Self::anim_candidates`] of the card being edited/previewed.
    pub(crate) anim_selected: Option<usize>,
    /// What the selected card's Play cycles (whole clip / loop / picks).
    pub(crate) anim_play_mode: AnimPlayMode,
    /// Recent motion prompts, newest first, for quick recall.
    pub(crate) anim_recent_motions: Vec<String>,
    /// Scrub cursor into the selected card's frames.
    pub(crate) anim_scrub: usize,
    /// Which decoded-frame index is currently uploaded to the GPU, if any. Lets
    /// the preview re-upload only when the shown frame changes.
    anim_shown: Option<usize>,
    /// Whether the clip transport is playing.
    pub(crate) anim_clip_playing: bool,
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
    /// Lineage parent captured by a re-roll/branch, stamped on the next clip
    /// card when it lands. `None` for a from-scratch generation.
    anim_pending_parent: Option<usize>,
    /// Animation-studio session state: the current stage, the anchor framing,
    /// the first-frame gallery and approved seed pose, the inpaint mask, and the
    /// motion-model pick. The clip candidates themselves live in `anim_*`, which
    /// the studio's clip/pick/land stages drive unchanged. All of it lives off
    /// the document until Land, so Cancel and restart are clean at every stage.
    pub(crate) studio: crate::studio::StudioState,
    /// Set while hand-editing a generated image in the drawing editor: the
    /// breadcrumb back to the studio thread the edit returns to.
    pub(crate) studio_return: Option<crate::studio::StudioReturn>,
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
    /// Whether the new-sprite dialog is open.
    new_sprite_open: bool,
    /// Draft canvas width for the new-sprite dialog; also the last-used size,
    /// persisted across launches.
    new_sprite_w: u32,
    /// Draft canvas height for the new-sprite dialog.
    new_sprite_h: u32,
    /// Whether the new-sprite size is being entered as a custom W×H rather than
    /// picked from a preset.
    new_sprite_custom: bool,
    /// Draft color mode for the new-sprite dialog.
    new_sprite_color: ColorMode,
    /// The sprite row being renamed inline: address, draft text, and a one-shot
    /// "request focus on the next frame" flag (focus is requested once, not
    /// every frame, so the editor's `lost_focus` can fire on Enter/click-away).
    renaming: Option<(SpriteRef, String, bool)>,
    /// The folder row being renamed inline: id, draft text, one-shot focus flag.
    renaming_group: Option<(GroupId, String, bool)>,
    /// Folders whose subtree is collapsed in the library panel (UI-only state).
    collapsed_groups: HashSet<GroupId>,
    /// The sprite awaiting delete confirmation.
    confirm_delete: Option<SpriteRef>,
    /// Whether the resize-canvas dialog is open.
    resize_open: bool,
    /// Draft target width for the resize dialog.
    resize_w: u32,
    /// Draft target height for the resize dialog.
    resize_h: u32,
    /// Anchor for an anchor-based (non-scaling) resize.
    resize_anchor: CanvasAnchor,
    /// Whether the resize dialog scales the pixels (resample) rather than
    /// padding/cropping the canvas.
    resize_resample: bool,
}

impl ShellApp {
    /// Builds the app from the eframe creation context, taking ownership of the
    /// shell's tokio runtime. Installs the [`ViewportRenderer`] and creates one
    /// starter sprite so the canvas shows immediately.
    pub fn new(cc: &eframe::CreationContext<'_>, runtime: Runtime) -> Self {
        let (tx, rx) = mpsc::channel();
        let render_state = cc.wgpu_render_state.clone();
        // Snapshot the active adapter and the full enumerated list for the GPU
        // picker, before `render_state` is moved into the struct below.
        let active_adapter = render_state.as_ref().map(|rs| rs.adapter.get_info());
        let available_adapters: Vec<wgpu::AdapterInfo> = render_state
            .as_ref()
            .map(|rs| rs.available_adapters.iter().map(wgpu::Adapter::get_info).collect())
            .unwrap_or_default();
        let gpu_pref = crate::gpu::load();
        let verb_runtime = ai::build_runtime();

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

        // Restore the last-used new-sprite size, defaulting to 64×64.
        let (last_w, last_h) = cc
            .storage
            .and_then(|s| eframe::get_value::<(u32, u32)>(s, "new_sprite_size"))
            .unwrap_or((DEFAULT_CANVAS.width, DEFAULT_CANVAS.height));

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
            timeline_expanded: false,
            verb_runtime,
            backend_ready: false,
            openai_key_configured: false,
            fal_key_configured: false,
            transform_in_flight: false,
            active_adapter,
            available_adapters,
            gpu_pref,
            egui_ctx: cc.egui_ctx.clone(),
            studio_library_open: false,
            library_tab: crate::library::LibraryTab::Templates,
            library_draft: None,
            rs_prompt: ai::DEFAULT_SHEET_PROMPT.to_owned(),
            ck_structure: ai::SINGLE_STRUCTURE_ID.to_owned(),
            rs_num_variants: 2,
            rs_status: JobStatus::Idle,
            rs_candidates: Vec::new(),
            rs_preview: None,
            rs_partial_preview: false,
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
            anim_pending_parent: None,
            studio: crate::studio::StudioState::default(),
            studio_return: None,
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
            new_sprite_open: false,
            new_sprite_w: last_w.clamp(1, MAX_CANVAS_DIM),
            new_sprite_h: last_h.clamp(1, MAX_CANVAS_DIM),
            new_sprite_custom: !is_preset_size(last_w, last_h),
            new_sprite_color: ColorMode::Rgba,
            renaming: None,
            renaming_group: None,
            collapsed_groups: HashSet::new(),
            confirm_delete: None,
            resize_open: false,
            resize_w: DEFAULT_CANVAS.width,
            resize_h: DEFAULT_CANVAS.height,
            resize_anchor: CanvasAnchor::Center,
            resize_resample: false,
        };
        app.install_renderer();
        app.doc.create_sprite("untitled", DEFAULT_CANVAS);
        app.refresh_canvas(true);
        // Register backends from the keychain off-thread so the blocking reads
        // never delay the first paint; readiness arrives over the channel.
        ai::spawn_backend_key_op(
            app.runtime.handle(),
            app.verb_runtime.clone(),
            app.egui_ctx.clone(),
            app.tx.clone(),
            ai::KeyOp::RegisterFromKeychain,
        );
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
        self.rs_preview.is_some() || self.rs_partial_preview
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

    /// Paints a streamed partial-generation frame on the canvas as a live
    /// preview. Non-destructive: the sprite document is untouched. Re-fits the
    /// viewport only on the first frame of a run so later frames don't jump.
    fn show_partial_preview(&mut self, pixels: &PixelData) {
        let Some(buffer) = pixel_data_to_pixel_buffer(pixels) else {
            return;
        };
        let first = !self.rs_partial_preview;
        self.upload_frame(&buffer, first);
        self.rs_partial_preview = true;
        self.anim_shown = None;
    }

    /// Drops a streamed partial preview and returns the canvas to the active
    /// sprite. Used when a streamed run fails so a half-drawn frame doesn't linger.
    fn clear_partial_preview(&mut self) {
        if std::mem::take(&mut self.rs_partial_preview) {
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
                ShellMsg::SheetPartial { pixels } => {
                    self.show_partial_preview(&pixels);
                }
                ShellMsg::SheetDone { variants, cost_usd } => {
                    self.cockpit_on_done(variants, cost_usd);
                }
                ShellMsg::SheetFailed(err) => {
                    self.clear_partial_preview();
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
                ShellMsg::FirstFrameProgress { epoch, message } => {
                    if epoch == self.studio.frame_gen.epoch {
                        self.studio.frame_gen.status = JobStatus::Running(message);
                    }
                }
                ShellMsg::FirstFrameDone { epoch, images, parent, append } => {
                    if epoch == self.studio.frame_gen.epoch {
                        self.on_gen_ready(images, parent, append);
                    }
                }
                ShellMsg::FirstFrameFailed { epoch, error } => {
                    if epoch == self.studio.frame_gen.epoch {
                        self.studio.frame_gen.status = JobStatus::Failed(error);
                        self.studio.frame_gen.cancel = None;
                    }
                }
                ShellMsg::BgRemovalDone { buffer_id, png } => {
                    self.apply_ai_bg_removal(buffer_id, &png);
                }
                ShellMsg::BgRemovalFailed { buffer_id, error } => {
                    self.bg_remove_pending.remove(&buffer_id);
                    self.bg_status = JobStatus::Failed(error);
                }
                ShellMsg::TransformDone(result) => {
                    self.transform_in_flight = false;
                    let TransformResult {
                        sprite_id,
                        before_sprite,
                        before_buffers,
                        after_buffers,
                        new_size,
                        label,
                    } = *result;
                    self.finish_canvas_transform(sprite_id, before_sprite, before_buffers, after_buffers, new_size, label);
                }
                ShellMsg::BackendsRefreshed {
                    openai_configured,
                    fal_configured,
                    ready,
                    error,
                } => {
                    self.openai_key_configured = openai_configured;
                    self.fal_key_configured = fal_configured;
                    self.backend_ready = ready;
                    if let Some(err) = error {
                        self.rs_status = JobStatus::Failed(format!("keychain: {err}"));
                    }
                }
            }
        }
        applied
    }

    /// Left panel: the sprite library as a collapsible folder tree. Toolbar to
    /// create sprites/folders; rows carry context-menu CRUD; drag a sprite onto
    /// a folder to file it, or a folder onto another to nest it, or onto the
    /// empty area to send it to the top level.
    #[allow(clippy::too_many_lines)] // one cohesive panel: render loop + action dispatch
    pub(crate) fn library_panel(&mut self, ui: &mut egui::Ui) {
        let mut new_sprite = false;
        let mut new_folder = false;
        ui.horizontal(|ui| {
            new_sprite = ui.button(format!("{} New sprite", crate::icons::ADD)).clicked();
            new_folder = ui.button(format!("{} New folder", crate::icons::GROUP)).clicked();
        });
        ui.separator();

        // `library_rows` returns owned rows, so `self` is free to mutate inside
        // the render closures. Collect actions, apply them after the loop.
        let rows = self.doc.library_rows(&self.collapsed_groups);
        let mut select: Option<SpriteRef> = None;
        let mut toggle_collapse: Option<GroupId> = None;
        let mut cancel_rename = false;
        let mut sprite_rename_commit: Option<SpriteRef> = None;
        let mut group_rename_commit: Option<GroupId> = None;
        let mut start_sprite_rename: Option<(SpriteRef, String)> = None;
        let mut start_group_rename: Option<(GroupId, String)> = None;
        let mut to_duplicate: Option<SpriteRef> = None;
        let mut to_delete: Option<SpriteRef> = None;
        let mut sprite_move: Option<(SpriteRef, bool)> = None;
        let mut group_move: Option<(GroupId, bool)> = None;
        let mut group_delete: Option<GroupId> = None;
        let mut new_subfolder: Option<GroupId> = None;
        let mut reparent_sprite: Option<(SpriteRef, Option<GroupId>)> = None;
        let mut reparent_group: Option<(GroupId, Option<GroupId>)> = None;

        for row in &rows {
            match row {
                LibraryRow::Group {
                    id,
                    name,
                    depth,
                    collapsed,
                    has_children,
                } => {
                    let (id, depth, collapsed, has_children) = (*id, *depth, *collapsed, *has_children);
                    let ((), payload) = crate::dnd::drop_target::<LibraryDrag, _>(ui, crate::dnd::DropHint::Into, |ui| {
                        ui.horizontal(|ui| {
                            ui.add_space(f32::from(depth) * 14.0);
                            let chevron = if collapsed { crate::icons::RIGHT } else { crate::icons::DOWN };
                            if ui.add_enabled(has_children, egui::Button::new(chevron).frame(false)).clicked() {
                                toggle_collapse = Some(id);
                            }
                            if let Some((gid, draft, needs_focus)) = self.renaming_group.as_mut() {
                                if *gid == id {
                                    let resp = ui.add(egui::TextEdit::singleline(draft).desired_width(f32::INFINITY));
                                    if *needs_focus {
                                        resp.request_focus();
                                        *needs_focus = false;
                                    }
                                    if resp.lost_focus() {
                                        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                                            cancel_rename = true;
                                        } else {
                                            group_rename_commit = Some(id);
                                        }
                                    }
                                    return;
                                }
                            }
                            let label = format!("{} {}", crate::icons::GROUP, name);
                            // One widget senses both click (toggle) and drag (nest); a clean
                            // click stays a click because `drag_started` needs pointer motion.
                            let resp = ui.add(egui::Button::selectable(false, label).sense(egui::Sense::click_and_drag()));
                            if resp.clicked() {
                                toggle_collapse = Some(id);
                            }
                            resp.dnd_set_drag_payload(LibraryDrag::Group(id));
                            resp.context_menu(|ui| {
                                if ui.button(format!("{} Rename", crate::icons::RENAME)).clicked() {
                                    start_group_rename = Some((id, name.clone()));
                                    ui.close();
                                }
                                if ui.button(format!("{} New subfolder", crate::icons::ADD)).clicked() {
                                    new_subfolder = Some(id);
                                    ui.close();
                                }
                                ui.separator();
                                if ui.button(format!("{} Move up", crate::icons::UP)).clicked() {
                                    group_move = Some((id, true));
                                    ui.close();
                                }
                                if ui.button(format!("{} Move down", crate::icons::DOWN)).clicked() {
                                    group_move = Some((id, false));
                                    ui.close();
                                }
                                ui.separator();
                                if ui.button(format!("{} Delete folder", crate::icons::TRASH)).clicked() {
                                    group_delete = Some(id);
                                    ui.close();
                                }
                            });
                        });
                    });
                    if let Some(payload) = payload {
                        match *payload {
                            LibraryDrag::Sprite(s) => reparent_sprite = Some((s, Some(id))),
                            LibraryDrag::Group(g) => reparent_group = Some((g, Some(id))),
                        }
                    }
                }
                LibraryRow::Sprite {
                    sprite_ref,
                    name,
                    canvas,
                    selected,
                    depth,
                } => {
                    let (sprite_ref, depth) = (*sprite_ref, *depth);
                    ui.horizontal(|ui| {
                        // Indent past the chevron column so sprites line up under folders.
                        ui.add_space(f32::from(depth) * 14.0 + 14.0);
                        if let Some((rref, draft, needs_focus)) = self.renaming.as_mut() {
                            if *rref == sprite_ref {
                                let resp = ui.add(egui::TextEdit::singleline(draft).desired_width(f32::INFINITY));
                                if *needs_focus {
                                    resp.request_focus();
                                    *needs_focus = false;
                                }
                                if resp.lost_focus() {
                                    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                                        cancel_rename = true;
                                    } else {
                                        sprite_rename_commit = Some(sprite_ref);
                                    }
                                }
                                return;
                            }
                        }
                        let label = format!("{}  ({}x{})", name, canvas.width, canvas.height);
                        // One widget senses both click (select) and drag (file into a folder).
                        let resp = ui.add(egui::Button::selectable(*selected, label).sense(egui::Sense::click_and_drag()));
                        if resp.clicked() {
                            select = Some(sprite_ref);
                        }
                        resp.dnd_set_drag_payload(LibraryDrag::Sprite(sprite_ref));
                        resp.context_menu(|ui| {
                            if ui.button(format!("{} Rename", crate::icons::RENAME)).clicked() {
                                start_sprite_rename = Some((sprite_ref, name.clone()));
                                ui.close();
                            }
                            if ui.button(format!("{} Duplicate", crate::icons::DUPLICATE)).clicked() {
                                to_duplicate = Some(sprite_ref);
                                ui.close();
                            }
                            ui.separator();
                            if ui.button(format!("{} Move up", crate::icons::UP)).clicked() {
                                sprite_move = Some((sprite_ref, true));
                                ui.close();
                            }
                            if ui.button(format!("{} Move down", crate::icons::DOWN)).clicked() {
                                sprite_move = Some((sprite_ref, false));
                                ui.close();
                            }
                            ui.separator();
                            if ui.button(format!("{} Delete", crate::icons::TRASH)).clicked() {
                                to_delete = Some(sprite_ref);
                                ui.close();
                            }
                        });
                    });
                }
            }
        }

        // A slim strip, shown only while dragging, files the item at top level.
        if let Some(payload) = crate::dnd::top_level_strip::<LibraryDrag>(ui, "Move to top level") {
            match *payload {
                LibraryDrag::Sprite(s) => reparent_sprite = Some((s, None)),
                LibraryDrag::Group(g) => reparent_group = Some((g, None)),
            }
        }

        // ---- apply collected actions ----
        if new_sprite {
            self.open_new_sprite_dialog();
        }
        if new_folder {
            if let Some(id) = self.doc.create_group("New folder", None) {
                self.renaming = None;
                self.renaming_group = Some((id, "New folder".to_owned(), true));
            }
        }
        if let Some(parent) = new_subfolder {
            if let Some(id) = self.doc.create_group("New folder", Some(parent)) {
                self.collapsed_groups.remove(&parent);
                self.renaming = None;
                self.renaming_group = Some((id, "New folder".to_owned(), true));
            }
        }
        if cancel_rename {
            self.renaming = None;
            self.renaming_group = None;
        }
        if let Some(r) = sprite_rename_commit {
            if let Some((_, name, _)) = self.renaming.take() {
                self.doc.rename_sprite(r, &name);
            }
        }
        if let Some(g) = group_rename_commit {
            if let Some((_, name, _)) = self.renaming_group.take() {
                self.doc.rename_group(g, &name);
            }
        }
        if let Some((r, name)) = start_sprite_rename {
            self.renaming = Some((r, name, true));
            self.renaming_group = None;
        }
        if let Some((g, name)) = start_group_rename {
            self.renaming_group = Some((g, name, true));
            self.renaming = None;
        }
        if let Some(g) = toggle_collapse {
            if !self.collapsed_groups.remove(&g) {
                self.collapsed_groups.insert(g);
            }
        }
        if let Some(r) = to_duplicate {
            self.exit_sheet_preview();
            if self.doc.duplicate_sprite(r).is_some() {
                self.playing = false;
                self.refresh_canvas(true);
            }
        }
        if let Some((r, up)) = sprite_move {
            self.doc.move_sprite(r, up);
        }
        if let Some((g, up)) = group_move {
            self.doc.move_group(g, up);
        }
        if let Some(g) = group_delete {
            self.doc.delete_group(g);
        }
        if let Some((s, group)) = reparent_sprite {
            self.doc.move_sprite_to_group(s, group);
        }
        if let Some((g, parent)) = reparent_group {
            self.doc.set_group_parent(g, parent);
        }
        if let Some(r) = to_delete {
            self.confirm_delete = Some(r);
        }
        if let Some(sprite_ref) = select {
            self.rs_preview = None;
            self.doc.select(sprite_ref);
            self.playing = false;
            self.refresh_canvas(true);
        }
    }

    /// Opens the new-sprite dialog, seeding the size from the active entity's
    /// default canvas size when it declares one, else from the last-used size.
    fn open_new_sprite_dialog(&mut self) {
        if let Some(size) = self.active_entity_default_canvas() {
            self.new_sprite_w = size.width.clamp(1, MAX_CANVAS_DIM);
            self.new_sprite_h = size.height.clamp(1, MAX_CANVAS_DIM);
            self.new_sprite_custom = !is_preset_size(size.width, size.height);
        }
        self.new_sprite_open = true;
    }

    /// The active library entity's default canvas size, if it declares one.
    fn active_entity_default_canvas(&self) -> Option<Size> {
        let entity_id = self.doc.active_entity_id()?;
        self.doc
            .project
            .library
            .entities
            .iter()
            .find(|e| e.id == entity_id)
            .and_then(|e| e.defaults.canvas_size)
    }

    /// Creates the sprite described by the new-sprite dialog and closes it.
    fn commit_new_sprite(&mut self) {
        let name = if self.new_sprite_name.trim().is_empty() {
            "untitled".to_owned()
        } else {
            self.new_sprite_name.trim().to_owned()
        };
        let w = self.new_sprite_w.clamp(1, MAX_CANVAS_DIM);
        let h = self.new_sprite_h.clamp(1, MAX_CANVAS_DIM);
        self.exit_sheet_preview();
        self.doc.create_sprite(name, Size::new(w, h));
        if self.new_sprite_color != ColorMode::Rgba {
            if let Some(sprite) = self.doc.active_sprite_mut() {
                sprite.color_mode = self.new_sprite_color;
            }
        }
        self.new_sprite_name.clear();
        self.new_sprite_open = false;
        self.new_sprite_w = w;
        self.new_sprite_h = h;
        self.playing = false;
        self.refresh_canvas(true);
    }

    /// Renders the new-sprite dialog: name, size preset/custom, and color mode.
    fn show_new_sprite_dialog(&mut self, ctx: &egui::Context) {
        if !self.new_sprite_open {
            return;
        }
        let mut open = true;
        let mut create = false;
        egui::Window::new("New sprite")
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Name");
                    ui.add(egui::TextEdit::singleline(&mut self.new_sprite_name).hint_text("untitled"));
                });
                ui.horizontal(|ui| {
                    ui.label("Size");
                    let current = if self.new_sprite_custom {
                        "Custom…".to_owned()
                    } else {
                        format!("{} x {}", self.new_sprite_w, self.new_sprite_h)
                    };
                    egui::ComboBox::from_id_salt("new_sprite_preset").selected_text(current).show_ui(ui, |ui| {
                        for (label, w, h) in SIZE_PRESETS {
                            let selected = !self.new_sprite_custom && self.new_sprite_w == *w && self.new_sprite_h == *h;
                            if ui.selectable_label(selected, *label).clicked() {
                                self.new_sprite_w = *w;
                                self.new_sprite_h = *h;
                                self.new_sprite_custom = false;
                            }
                        }
                        if ui.selectable_label(self.new_sprite_custom, "Custom…").clicked() {
                            self.new_sprite_custom = true;
                        }
                    });
                });
                if self.new_sprite_custom {
                    ui.horizontal(|ui| {
                        ui.label("W");
                        ui.add(egui::DragValue::new(&mut self.new_sprite_w).range(1..=MAX_CANVAS_DIM).suffix(" px"));
                        ui.label("H");
                        ui.add(egui::DragValue::new(&mut self.new_sprite_h).range(1..=MAX_CANVAS_DIM).suffix(" px"));
                    });
                }
                ui.horizontal(|ui| {
                    ui.label("Color");
                    egui::ComboBox::from_id_salt("new_sprite_color")
                        .selected_text(color_mode_label(self.new_sprite_color))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.new_sprite_color, ColorMode::Rgba, "RGBA");
                            ui.selectable_value(&mut self.new_sprite_color, ColorMode::Grayscale, "Grayscale");
                            ui.selectable_value(&mut self.new_sprite_color, ColorMode::Indexed, "Indexed");
                        });
                });
                if let Some(text) = large_canvas_warning(self.new_sprite_w, self.new_sprite_h) {
                    ui.colored_label(ui.visuals().warn_fg_color, text);
                }
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("Create").clicked() {
                        create = true;
                    }
                    if ui.button("Cancel").clicked() {
                        self.new_sprite_open = false;
                    }
                });
            });
        if !open {
            self.new_sprite_open = false;
        }
        if create {
            self.commit_new_sprite();
        }
    }

    /// Renders the delete-confirmation dialog. Deletion is not undoable, so it
    /// asks first.
    fn show_delete_confirm(&mut self, ctx: &egui::Context) {
        let Some(sprite_ref) = self.confirm_delete else {
            return;
        };
        let mut open = true;
        let mut confirm = false;
        egui::Window::new("Delete sprite")
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .show(ctx, |ui| {
                ui.label("Delete this sprite? This cannot be undone.");
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button(format!("{} Delete", crate::icons::TRASH)).clicked() {
                        confirm = true;
                    }
                    if ui.button("Cancel").clicked() {
                        self.confirm_delete = None;
                    }
                });
            });
        if !open {
            self.confirm_delete = None;
        }
        if confirm {
            self.exit_sheet_preview();
            self.doc.delete_sprite(sprite_ref);
            self.confirm_delete = None;
            self.playing = false;
            self.refresh_canvas(true);
        }
    }

    /// Opens the resize-canvas dialog, seeded from the active sprite's size.
    fn open_resize_dialog(&mut self) {
        if let Some(sprite) = self.doc.active_sprite() {
            self.resize_w = sprite.canvas.width;
            self.resize_h = sprite.canvas.height;
        }
        self.resize_open = true;
    }

    /// Renders the resize-canvas dialog: target size, scale-vs-anchor mode, and
    /// the 3×3 anchor grid.
    fn show_resize_dialog(&mut self, ctx: &egui::Context) {
        if !self.resize_open {
            return;
        }
        let mut open = true;
        let mut apply = false;
        egui::Window::new("Resize canvas")
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("W");
                    ui.add(egui::DragValue::new(&mut self.resize_w).range(1..=MAX_CANVAS_DIM).suffix(" px"));
                    ui.label("H");
                    ui.add(egui::DragValue::new(&mut self.resize_h).range(1..=MAX_CANVAS_DIM).suffix(" px"));
                });
                ui.checkbox(&mut self.resize_resample, "Scale image (nearest-neighbor)");
                ui.add_enabled_ui(!self.resize_resample, |ui| {
                    ui.label("Anchor");
                    anchor_grid(ui, &mut self.resize_anchor);
                });
                if let Some(text) = large_canvas_warning(self.resize_w, self.resize_h) {
                    ui.colored_label(ui.visuals().warn_fg_color, text);
                }
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("Apply").clicked() {
                        apply = true;
                    }
                    if ui.button("Cancel").clicked() {
                        self.resize_open = false;
                    }
                });
            });
        if !open {
            self.resize_open = false;
        }
        if apply {
            let op = if self.resize_resample {
                CanvasOp::Resample {
                    width: self.resize_w,
                    height: self.resize_h,
                }
            } else {
                CanvasOp::Resize {
                    width: self.resize_w,
                    height: self.resize_h,
                    anchor: self.resize_anchor,
                }
            };
            self.resize_open = false;
            self.run_canvas_transform(op);
        }
    }

    /// Applies a whole-canvas op to the active sprite, recording it as one undo
    /// entry. Rewrites every raster cel buffer and, where dimensions change, the
    /// sprite canvas. Large canvases transform on a blocking thread so the frame
    /// loop is not stalled (the 8K constraint).
    pub(crate) fn run_canvas_transform(&mut self, op: CanvasOp) {
        if self.transform_in_flight {
            return;
        }
        self.exit_sheet_preview();
        let Some(sprite_id) = self.doc.project.active_sprite_id() else {
            return;
        };
        let Some(before_sprite) = self.doc.project.sprite(sprite_id).cloned() else {
            return;
        };
        let new_size = op.result_size(before_sprite.canvas);
        if new_size.is_empty() || new_size.width > MAX_CANVAS_DIM || new_size.height > MAX_CANVAS_DIM {
            return;
        }

        // Distinct raster buffers referenced by the sprite, in cel order.
        let mut ids: Vec<PixelBufferId> = Vec::new();
        let mut seen: HashSet<PixelBufferId> = HashSet::new();
        for cel in &before_sprite.cels {
            if let CelData::Raster { buffer, .. } = cel.data {
                if seen.insert(buffer) {
                    ids.push(buffer);
                }
            }
        }
        let before_buffers: Vec<(PixelBufferId, PixelBuffer)> = ids.iter().filter_map(|id| self.doc.pixel_buffers.get(id).map(|b| (*id, b.clone()))).collect();

        let inputs = before_buffers.clone();
        let compute =
            move || -> Option<Vec<(PixelBufferId, PixelBuffer)>> { inputs.iter().map(|(id, buf)| op.apply(buf).ok().map(|out| (*id, out))).collect() };
        let label = op.label().to_owned();

        if before_sprite.canvas.pixel_count() > TRANSFORM_OFFLOAD_PIXELS {
            // A multi-megapixel pass would stall the frame, so run it on a
            // blocking thread and record the edit when the result arrives
            // (drained as ShellMsg::TransformDone). The in-flight flag blocks
            // re-entry; finish_canvas_transform drops the result if the
            // document changed meanwhile.
            self.transform_in_flight = true;
            let tx = self.tx.clone();
            let ctx = self.egui_ctx.clone();
            self.runtime.handle().spawn_blocking(move || {
                let after_buffers = compute();
                let _ = tx.send(ShellMsg::TransformDone(Box::new(TransformResult {
                    sprite_id,
                    before_sprite,
                    before_buffers,
                    after_buffers,
                    new_size,
                    label,
                })));
                ctx.request_repaint();
            });
        } else {
            let after_buffers = compute();
            self.finish_canvas_transform(sprite_id, before_sprite, before_buffers, after_buffers, new_size, label);
        }
    }

    /// Records a completed canvas transform as a [`CanvasEdit`]. Shared by the
    /// synchronous (small canvas) and off-thread (heavy) paths. Drops the result
    /// when the op failed, was a no-op, or the document changed under an
    /// off-thread run (an interim draw or structural edit), so a stale transform
    /// never clobbers newer work.
    fn finish_canvas_transform(
        &mut self,
        sprite_id: SpriteId,
        before_sprite: Sprite,
        before_buffers: Vec<(PixelBufferId, PixelBuffer)>,
        after_buffers: Option<Vec<(PixelBufferId, PixelBuffer)>>,
        new_size: Size,
        label: String,
    ) {
        let Some(after_buffers) = after_buffers else {
            return;
        };

        // Stale-result guard: the sprite value and every input buffer must still
        // match what the transform ran against.
        let unchanged_since = self.doc.project.sprite(sprite_id) == Some(&before_sprite)
            && before_buffers.iter().all(|(id, before)| self.doc.pixel_buffers.get(id) == Some(before));
        if !unchanged_since {
            return;
        }

        // Build the after sprite: new canvas size and per-raster-cel size.
        let mut after_sprite = before_sprite.clone();
        after_sprite.canvas = new_size;
        for cel in &mut after_sprite.cels {
            if let CelData::Raster { size, .. } = &mut cel.data {
                *size = new_size;
            }
        }

        let swaps: Vec<CanvasBufferSwap> = before_buffers
            .into_iter()
            .zip(after_buffers)
            .map(|((id, before), (_, after))| CanvasBufferSwap { id, before, after })
            .collect();

        // Skip recording a true no-op (e.g. flipping an empty canvas).
        let unchanged = after_sprite == before_sprite && swaps.iter().all(|s| s.before == s.after);
        if unchanged {
            return;
        }

        let edit = CanvasEdit {
            sprite_id,
            before_sprite,
            after_sprite,
            buffers: swaps,
            label,
        };
        let _ = self.editor.history.push(Box::new(edit), &mut self.doc);
        // Selection coordinates may fall outside the new canvas.
        self.editor.clear_selection();
        self.refresh_canvas(true);
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
            let next = self.anim_play_cursor + 1;
            if next >= self.anim_play_indices.len() {
                // At the end: wrap when the studio's loop toggle is on, else stop.
                if self.studio.loop_playback {
                    self.anim_play_cursor = 0;
                } else {
                    self.anim_clip_playing = false;
                    return;
                }
            } else {
                self.anim_play_cursor = next;
            }
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
        ai::spawn_backend_key_op(
            self.runtime.handle(),
            self.verb_runtime.clone(),
            self.egui_ctx.clone(),
            self.tx.clone(),
            ai::KeyOp::Save {
                backend: backend.to_owned(),
                key: key.to_owned(),
            },
        );
    }

    /// Clears a backend's stored key and unregisters it, off the UI thread. The
    /// refreshed readiness and configured flags land over the channel.
    pub(crate) fn clear_backend(&mut self, backend: &str) {
        ai::spawn_backend_key_op(
            self.runtime.handle(),
            self.verb_runtime.clone(),
            self.egui_ctx.clone(),
            self.tx.clone(),
            ai::KeyOp::Clear { backend: backend.to_owned() },
        );
    }

    /// Cached "is a key stored for `backend`" flag, refreshed off-thread. Reads
    /// no keychain, so the settings panel can call it every frame.
    #[must_use]
    pub(crate) fn key_configured(&self, backend: &str) -> bool {
        match backend {
            ai::OPENAI_BACKEND_ID => self.openai_key_configured,
            ai::FAL_BACKEND_ID => self.fal_key_configured,
            _ => false,
        }
    }

    /// Pins `info` as the adapter to use on the next launch and persists it.
    pub(crate) fn select_gpu(&mut self, info: &wgpu::AdapterInfo) {
        let pref = crate::gpu::from_info(info);
        match crate::gpu::save(Some(&pref)) {
            Ok(()) => self.gpu_pref = Some(pref),
            Err(err) => self.rs_status = JobStatus::Failed(format!("GPU preference: {err}")),
        }
    }

    /// Clears the GPU preference so the next launch uses the default adapter.
    pub(crate) fn clear_gpu_pref(&mut self) {
        match crate::gpu::save(None) {
            Ok(()) => self.gpu_pref = None,
            Err(err) => self.rs_status = JobStatus::Failed(format!("GPU preference: {err}")),
        }
    }

    /// The selected clip card, if any.
    pub(crate) fn anim_card(&self) -> Option<&ClipCandidate> {
        self.anim_selected.and_then(|i| self.anim_candidates.get(i))
    }

    /// Selects clip card `i` for editing and resets the scrub/preview to it.
    pub(crate) fn select_clip(&mut self, i: usize) {
        self.anim_selected = Some(i);
        self.anim_scrub = 0;
        self.anim_shown = None;
        self.anim_clip_playing = false;
        self.sync_clip_preview();
    }

    /// Discards clip card `i`, fixing the selection and the canvas preview.
    pub(crate) fn remove_clip(&mut self, i: usize) {
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
    /// result lands as a new card that records `i` as its lineage parent.
    pub(crate) fn reroll_clip(&mut self, i: usize) {
        let motion = self.anim_candidates[i].motion.clone();
        self.anim_motion = motion;
        self.anim_seed_fixed = false;
        self.anim_pending_parent = Some(i);
        self.start_clip();
    }

    /// Clears the pending clip-lineage parent so the next clip lands as a
    /// from-scratch generation (no re-roll/branch ancestry).
    pub(crate) fn clear_clip_lineage(&mut self) {
        self.anim_pending_parent = None;
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

    /// The picked-frame thumbnail strip for the selected card; click to drop.
    #[allow(clippy::cast_precision_loss)]
    pub(crate) fn pick_strip(&mut self, ui: &mut egui::Ui) {
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
            .filter_map(|&p| {
                self.anim_candidates[i]
                    .thumbs
                    .get(p)
                    .and_then(|t| t.as_ref())
                    .map(|tex| (p, tex.id(), tex.size_vec2()))
            })
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
    pub(crate) fn toggle_pick(&mut self, i: usize) {
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
    pub(crate) fn toggle_clip_play(&mut self) {
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
    pub(crate) fn current_play_indices(&self) -> Vec<usize> {
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
    pub(crate) fn start_clip(&mut self) {
        let Some(anchor) = self.doc.active_anchor().map(<[u8]>::to_vec) else {
            return;
        };
        let Some(sprite) = self.doc.active_sprite() else {
            return;
        };
        let job = ai::AnimJob {
            canvas: (sprite.canvas.width, sprite.canvas.height),
            anchor_png: anchor,
            // The studio's approved seed pose drives the i2v call; without one
            // the clip path generates its own first frame from the anchor.
            first_frame_png: self.studio.approved_first_frame.clone(),
            motion_prompt: self.anim_motion.clone(),
            i2v_model: Some(self.studio.i2v_model.model_id()),
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
    pub(crate) fn cancel_clip(&mut self) {
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
        let keyed_thumbs = vec![None; frames.len()];
        self.anim_candidates.push(ClipCandidate {
            clip,
            mime,
            thumbs,
            markers,
            picks,
            motion: self.anim_motion.clone(),
            fps: self.anim_fps,
            seed: self.anim_seed_fixed.then_some(self.anim_seed),
            parent: self.anim_pending_parent.take(),
            card_texture: None,
            keyed_thumbs,
            keyed_sig: None,
            frames,
        });
        let idx = self.anim_candidates.len() - 1;
        self.select_clip(idx);
    }

    /// Normalizes and integrates the selected card's picked frames onto the
    /// timeline. The frames still carry their background — removal is a separate
    /// timeline op. The clip stays in the gallery; the canvas returns to the sprite.
    pub(crate) fn integrate_picked(&mut self) {
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
            // When the studio's "remove background on Land" is set, key the
            // backdrop out during normalize so the loop lands already stripped.
            chroma: crate::studio::land_chroma(self.studio.remove_on_land, self.bg_key_color, self.bg_tolerance),
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
        integrate_frames_undoable(&mut self.editor, &mut self.doc, frames, frame_ms, &motion, LoopDirection::Forward);
        // Drop the canvas preview back to the sprite/timeline; keep the gallery.
        self.anim_selected = None;
        self.anim_clip_playing = false;
        self.exit_clip_preview();
    }

    /// Moves the scrub cursor within the selected card and re-shows that frame.
    pub(crate) fn set_scrub(&mut self, idx: usize) {
        let n = self.anim_card().map_or(0, |c| c.frames.len());
        self.anim_scrub = idx.min(n.saturating_sub(1));
        self.sync_clip_preview();
    }

    /// Whether the wgpu canvas is currently showing a clip frame (the
    /// canvas-preview marker is set). The animation studio is a full-screen
    /// takeover that renders clip frames as egui textures in its own surface, so
    /// it never sets this — but the marker stays the honest source of truth, so
    /// the preview machinery (`sync_clip_preview` / `exit_clip_preview`) remains
    /// coherent if a non-studio caller ever drives the canvas again.
    fn clip_preview_active(&self) -> bool {
        self.anim_shown.is_some()
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
                if ui.button("New sprite…").clicked() {
                    self.open_new_sprite_dialog();
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

            ui.menu_button("Sprite", |ui| {
                let has_sprite = self.doc.active_sprite().is_some();
                ui.add_enabled_ui(has_sprite, |ui| {
                    if ui.button(format!("{} Resize canvas…", crate::icons::RESIZE)).clicked() {
                        self.open_resize_dialog();
                        ui.close();
                    }
                    ui.separator();
                    if ui.button(format!("{} Flip horizontal", crate::icons::FLIP_H)).clicked() {
                        self.run_canvas_transform(CanvasOp::FlipHorizontal);
                        ui.close();
                    }
                    if ui.button(format!("{} Flip vertical", crate::icons::FLIP_V)).clicked() {
                        self.run_canvas_transform(CanvasOp::FlipVertical);
                        ui.close();
                    }
                    ui.separator();
                    if ui.button(format!("{} Rotate 90° CW", crate::icons::ROTATE_CW)).clicked() {
                        self.run_canvas_transform(CanvasOp::Rotate90Cw);
                        ui.close();
                    }
                    if ui.button(format!("{} Rotate 90° CCW", crate::icons::ROTATE_CCW)).clicked() {
                        self.run_canvas_transform(CanvasOp::Rotate90Ccw);
                        ui.close();
                    }
                    if ui.button("Rotate 180°").clicked() {
                        self.run_canvas_transform(CanvasOp::Rotate180);
                        ui.close();
                    }
                });
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
                // Create lands in the full-screen AI studio; the cockpit is
                // reached from the studio's Anchor stage.
                self.enter_studio();
            }

            // A hand-edit round trip: a prominent way back to the studio that
            // lands the edited pixels as a new candidate.
            if self.studio_return.is_some() {
                ui.separator();
                if ui
                    .button(format!("{} Finish hand-edit", crate::icons::CHECK))
                    .on_hover_text("Return to the AI studio and add this edit to the thread")
                    .clicked()
                {
                    self.finish_hand_edit();
                }
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

    /// The right-side dock in Draw and Animate: Palette and Layers stacked as
    /// collapsing sections so a colour and the layer stack are usable at once,
    /// with Sprites collapsed below. Create has no dock — the full-screen studio
    /// owns the whole canvas area.
    fn dock_panel(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            egui::CollapsingHeader::new(format!("{} Palette", crate::icons::PALETTE))
                .default_open(true)
                .show(ui, |ui| self.palette_panel(ui));
            egui::CollapsingHeader::new(format!("{} Layers", crate::icons::LAYERS))
                .default_open(true)
                .show(ui, |ui| self.layers_panel(ui));
            egui::CollapsingHeader::new(format!("{} Sprites", crate::icons::IMAGE))
                .default_open(false)
                .show(ui, |ui| self.library_panel(ui));
        });
    }

    /// Applies and remembers a new theme preference.
    pub(crate) fn set_theme_preference(&mut self, ctx: &egui::Context, preference: egui::ThemePreference) {
        self.theme_preference = preference;
        ctx.set_theme(preference);
    }

    /// Switches workspace mode, expanding the timeline for Animate and showing
    /// the AI surface for Create.
    pub(crate) fn set_workspace(&mut self, workspace: Workspace) {
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
                self.open_new_sprite_dialog();
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

/// A library drag payload: a sprite (filed into a folder) or a folder (nested
/// under another). One type so a drop zone accepts either via `dnd_drop_zone`.
#[derive(Clone, Copy)]
enum LibraryDrag {
    Sprite(SpriteRef),
    Group(GroupId),
}

/// Whether `(w, h)` exactly matches one of the [`SIZE_PRESETS`] entries.
fn is_preset_size(w: u32, h: u32) -> bool {
    SIZE_PRESETS.iter().any(|(_, pw, ph)| *pw == w && *ph == h)
}

/// The dropdown label for a color mode.
fn color_mode_label(mode: ColorMode) -> &'static str {
    match mode {
        ColorMode::Rgba => "RGBA",
        ColorMode::Grayscale => "Grayscale",
        ColorMode::Indexed => "Indexed",
    }
}

/// A memory-cost warning for a large canvas, or `None` below the threshold.
///
/// Surfaces the per-buffer RGBA cost so the artist can confirm before
/// allocating: an 8192×8192 buffer is 256 MB, and a sprite allocates one per
/// cel. The threshold is ~4 megapixels (a 2048-square buffer is 16 MB).
fn large_canvas_warning(w: u32, h: u32) -> Option<String> {
    let pixels = u64::from(w) * u64::from(h);
    if pixels < 2048 * 2048 {
        return None;
    }
    let mb = pixels * 4 / (1024 * 1024);
    Some(format!("Large canvas: ~{mb} MB per layer buffer"))
}

/// A 3×3 selectable grid for picking a [`CanvasAnchor`]. The cell position is
/// the meaning (top-left cell pins to the top-left); the selected cell is
/// highlighted. Kept glyph-free so it renders identically in any font.
fn anchor_grid(ui: &mut egui::Ui, anchor: &mut CanvasAnchor) {
    use CanvasAnchor::{Bottom, BottomLeft, BottomRight, Center, Left, Right, Top, TopLeft, TopRight};
    let rows = [[TopLeft, Top, TopRight], [Left, Center, Right], [BottomLeft, Bottom, BottomRight]];
    egui::Grid::new("resize_anchor_grid").spacing([2.0, 2.0]).show(ui, |ui| {
        for row in rows {
            for cell in row {
                let selected = *anchor == cell;
                let glyph = if selected { "*" } else { " " };
                if ui.add_sized([20.0, 20.0], egui::Button::selectable(selected, glyph)).clicked() {
                    *anchor = cell;
                }
            }
            ui.end_row();
        }
    });
}

/// Truncates a motion prompt for a label, appending an ellipsis when clipped.
pub(crate) fn truncate_motion(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_owned()
    } else {
        let head: String = s.chars().take(max.saturating_sub(1)).collect();
        format!("{head}…")
    }
}

/// Decodes PNG bytes into a tightly packed RGBA [`PixelBuffer`] for display on
/// the wgpu canvas. Returns `None` on a decode failure or an invalid size.
pub(crate) fn png_to_pixel_buffer(png: &[u8]) -> Option<PixelBuffer> {
    let rgba = image::load_from_memory(png).ok()?.to_rgba8();
    let (width, height) = (rgba.width(), rgba.height());
    PixelBuffer::from_raw(width, height, width * 4, rgba.into_raw()).ok()
}

/// Wraps a streamed [`PixelData`] preview frame into a display [`PixelBuffer`]
/// for the wgpu canvas. Returns `None` if the bytes don't form a valid buffer,
/// so a malformed partial frame is skipped rather than crashing the UI.
fn pixel_data_to_pixel_buffer(pixels: &PixelData) -> Option<PixelBuffer> {
    PixelBuffer::from_raw(pixels.width, pixels.height, pixels.stride, pixels.bytes.clone()).ok()
}

/// Converts a decoded clip frame into a [`PixelBuffer`] for the canvas, reusing
/// the exact mechanism the reference-sheet preview uses.
pub(crate) fn video_frame_to_pixel_buffer(frame: &VideoFrame) -> Option<PixelBuffer> {
    PixelBuffer::from_raw(frame.width, frame.height, frame.width * 4, frame.pixels.clone()).ok()
}

/// Loads a clip frame as a NEAREST-sampled egui texture for the pick strip.
pub(crate) fn video_frame_to_texture(ctx: &egui::Context, frame: &VideoFrame) -> egui::TextureHandle {
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
        eframe::set_value(storage, "new_sprite_size", &(self.new_sprite_w, self.new_sprite_h));
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.handle_shortcuts(ui.ctx());
        self.show_settings_window(ui.ctx());
        self.show_new_sprite_dialog(ui.ctx());
        self.show_delete_confirm(ui.ctx());
        self.show_resize_dialog(ui.ctx());

        // Panel order matters: outer panels first, the central canvas last so
        // it fills the space the others leave. The menu bar is always shown;
        // the rest of the editor chrome yields when the animation studio takes
        // over full-screen.
        egui::Panel::top("menu_bar").show_inside(ui, |ui| self.menu_bar(ui));

        let studio = self.studio_active();
        if !studio {
            // Two top strips (menu, then the tool context bar); two bottom strips
            // (status at the edge, timeline above it).
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
        }

        egui::CentralPanel::default().show_inside(ui, |ui| {
            // Create-mode takes over the canvas area with the full-screen studio
            // (which hosts the composition library as an overlay). Everything
            // else paints the sprite canvas.
            if studio {
                self.studio_view(ui);
            } else {
                self.canvas_ui(ui);
            }
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
