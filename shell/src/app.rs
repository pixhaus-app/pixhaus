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
use pixhaus_core::selection::SelectionMask;
use pixhaus_core::transforms::normalize::{NormalizeOptions, NormalizeResult, normalize_frames};
use pixhaus_core::transforms::{CanvasAnchor, MlaaConfig, RotationAlgorithm};
use pixhaus_render::{Viewport, ViewportRenderer};
use tokio::runtime::Runtime;
use tokio_util::sync::CancellationToken;

use crate::ai;
use crate::anim::{self, LoopMarkers, VideoFrame};
use crate::cockpit::{CockpitCandidate, CockpitReference, PendingLineage};
use crate::commands::{CanvasBufferSwap, CanvasEdit, CanvasOp, CropEdit, integrate_frames_undoable, push_sprite_edit};
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

/// The grid and overlay preferences persisted across launches through eframe
/// `Storage` under the `"grid_prefs"` key, alongside `theme_preference` and
/// `keymap`. These are view settings, not part of the serialized project, so
/// they never touch the `.pixhaus` schema. Dither and stroke opacity are
/// per-session tool state and are deliberately not persisted, matching the
/// Tauri app, which reset them each launch.
///
/// The defaults mirror [`crate::editor::EditorState`] so a fresh install and a
/// stored-but-empty value agree.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct GridPrefs {
    /// Show the per-pixel grid once the zoom is high enough to read it.
    pub show_pixel_grid: bool,
    /// Show the major / tile grid overlay.
    pub major_grid_enabled: bool,
    /// Major-grid spacing on the X axis, in canvas pixels.
    pub spacing_x: u32,
    /// Major-grid spacing on the Y axis, in canvas pixels.
    pub spacing_y: u32,
}

impl Default for GridPrefs {
    fn default() -> Self {
        let editor = crate::editor::EditorState::default();
        Self::from_editor(&editor)
    }
}

impl GridPrefs {
    /// Snapshots the persistable grid fields off the live editor state.
    #[must_use]
    pub fn from_editor(editor: &crate::editor::EditorState) -> Self {
        Self {
            show_pixel_grid: editor.show_pixel_grid,
            major_grid_enabled: editor.major_grid_enabled,
            spacing_x: editor.major_grid_spacing_x,
            spacing_y: editor.major_grid_spacing_y,
        }
    }

    /// Writes the stored grid fields back onto the editor state on load.
    pub fn apply_to(self, editor: &mut crate::editor::EditorState) {
        editor.show_pixel_grid = self.show_pixel_grid;
        editor.major_grid_enabled = self.major_grid_enabled;
        // Spacing is a divisor in the major-grid step math; a zero would stride
        // by nothing, so clamp a corrupt stored value up to 1.
        editor.major_grid_spacing_x = self.spacing_x.max(1);
        editor.major_grid_spacing_y = self.spacing_y.max(1);
    }
}

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
        /// Durable job id, so the queue can mark this run `Done` and persist the
        /// clip blob.
        job_id: u64,
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
        /// Durable job id, so the queue can mark this run `Error`.
        job_id: u64,
        /// Failure reason.
        error: String,
    },
    /// A user-supplied video file finished decoding off the UI thread. It feeds
    /// the same `on_clip_ready` path a generated clip uses; the i2v fields
    /// (approved first frame, motion prompt) play no part. The result is `Err`
    /// with a user-facing message when the pick was cancelled-then-failed or the
    /// codec is unsupported.
    VideoImported {
        /// Entity captured when the picker opened, so the import records against
        /// the right entity even if the active sprite changed since.
        entity_id: pixhaus_core::project::EntityId,
        /// Sprite captured when the picker opened.
        sprite_id: SpriteId,
        /// Frames-per-second used for the decode and the candidate.
        fps: u32,
        /// Loop-frame target captured when the picker opened.
        target: u32,
        /// The decoded clip (raw bytes, mime, frames), or a user-facing error.
        result: Result<(Vec<u8>, String, Vec<VideoFrame>), String>,
    },
    /// A WAV file finished decoding off the UI thread for "sync to audio". The
    /// payload is the per-frame `duration_ms` list the energy-onset detector
    /// produced (one entry per detected beat), or a user-facing error when the
    /// pick was cancelled or the input is not PCM WAV. The audio is read for
    /// timing only and never embedded.
    AudioSynced {
        /// Sprite captured when the picker opened, so the durations land on the
        /// right sprite even if the active sprite changed since.
        sprite_id: SpriteId,
        /// Inclusive frame range the durations apply to, captured at pick time
        /// (the selected tag's range, or the whole timeline when no tag is
        /// selected).
        range: pixhaus_core::project::FrameRange,
        /// Frames-per-second the onsets were snapped to.
        fps: u32,
        /// Per-frame durations in milliseconds (one per detected beat), or a
        /// user-facing error.
        result: Result<Vec<u32>, String>,
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
    /// An anchor inpaint-refine finished: the repainted image, to land as a new
    /// linked result under `parent`.
    AnchorRefineDone {
        /// Refine epoch this result belongs to; stale ones are dropped.
        epoch: u64,
        /// Gallery index the refine descends from.
        parent: usize,
        /// The repainted image as PNG bytes.
        image: Vec<u8>,
    },
    /// An anchor inpaint-refine failed.
    AnchorRefineFailed {
        /// Refine epoch this failure belongs to; stale ones are dropped.
        epoch: u64,
        /// Failure reason.
        error: String,
    },
    /// A Tier-1 neutral-anchor derivation finished: the clean neutral image to
    /// stamp `NeutralReset` and store on the entity's `CharacterAnchor`.
    NeutralDerived {
        /// Derivation epoch this result belongs to; stale ones are dropped.
        epoch: u64,
        /// The entity whose `CharacterAnchor.neutral` slot the result fills.
        entity_id: pixhaus_core::project::EntityId,
        /// The canonical variant this neutral was derived from — the
        /// `parent_variant_id` edge the staleness cascade walks back.
        canonical_id: pixhaus_core::project::SheetVariantId,
        /// The derived neutral image as PNG bytes.
        image: Vec<u8>,
    },
    /// A neutral-anchor derivation failed (or was canceled).
    NeutralFailed {
        /// Derivation epoch this failure belongs to; stale ones are dropped.
        epoch: u64,
        /// Failure reason.
        error: String,
    },
    /// A directional-anchor derivation finished: the directional image to stamp
    /// `DirectionalAnchor` and store on the entity's `CharacterAnchor`. East is
    /// never generated (it is the flip of west), so `direction` is only ever
    /// south / west / north.
    DirectionalDerived {
        /// Derivation epoch this result belongs to; stale ones are dropped.
        epoch: u64,
        /// The entity whose directional slot the result fills.
        entity_id: pixhaus_core::project::EntityId,
        /// The direction this anchor was generated for.
        direction: pixhaus_core::project::AnchorDirection,
        /// The neutral variant this directional anchor was derived from — the
        /// `parent_variant_id` edge the staleness cascade walks back.
        neutral_id: pixhaus_core::project::SheetVariantId,
        /// The derived directional image as PNG bytes.
        image: Vec<u8>,
    },
    /// A directional-anchor derivation failed (or was canceled).
    DirectionalFailed {
        /// Derivation epoch this failure belongs to; stale ones are dropped.
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
    /// A Lospec palette fetch finished on its background thread: the slug it ran
    /// for and either `(name, colors)` or an error string to surface inline.
    /// Gated behind the `lospec` feature so the default build carries no Lospec
    /// message.
    #[cfg(feature = "lospec")]
    LospecDone {
        /// The slug the fetch ran for (for the status line).
        slug: String,
        /// The fetched `(name, colors)` or an error string.
        result: Result<(String, Vec<Rgba>), String>,
    },
    /// A composition-pack export finished off-thread: the chosen path (for the
    /// status line) on success, or an error to surface. A cancelled save dialog
    /// is reported as `Ok(None)` so the UI stays silent.
    PackExportDone {
        /// The path written on success, `None` if the dialog was cancelled.
        path: Result<Option<std::path::PathBuf>, String>,
    },
    /// A composition-pack import finished decoding off-thread: the decoded pack
    /// to merge on the UI thread, or an error to surface. A cancelled open
    /// dialog is reported as `Ok(None)` so the UI stays silent.
    PackImportDecoded {
        /// The decoded pack, `None` if the dialog was cancelled.
        pack: Result<Option<pixhaus_core::project::StylePack>, String>,
        /// The conflict policy the artist chose before the import ran.
        policy: pixhaus_core::project::ConflictPolicy,
    },
    /// A GUI sprite export finished off-thread: the written path on success
    /// (for the status line), `None` if the save dialog was cancelled, or an
    /// error to surface. Drained in [`ShellApp::logic`].
    SpriteExported {
        /// The path written on success, `None` if the dialog was cancelled, or
        /// a user-facing error string.
        result: Result<Option<std::path::PathBuf>, String>,
    },
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
    /// Whether the directional-cascade coverage grid is open over the studio.
    pub(crate) anim_set_open: bool,
    /// Which composition-library tab shows in the Library view.
    pub(crate) library_tab: crate::library::LibraryTab,
    /// The composition-library record currently open in the editor, if any.
    /// Acts as the browser's selection: the row whose id matches is highlighted.
    pub(crate) library_draft: Option<crate::library::LibraryDraft>,
    /// Whether the library overlay shows the asset browser (saved references,
    /// character cards, style swatches) instead of the composition library.
    pub(crate) asset_browser_open: bool,
    /// Decoded asset thumbnails, keyed by [`pixhaus_core::project::AssetId`]'s raw
    /// value. Each PNG decodes once on first display and is reused; deletes evict
    /// their entry. Keeps the browser from re-decoding on every frame.
    pub(crate) asset_tex_cache: HashMap<u32, egui::TextureHandle>,
    /// The asset (card or swatch) being renamed inline: its raw id and draft text.
    pub(crate) asset_rename: Option<(u32, String)>,
    /// Draft name for a new library tag, typed into the tag-manager add field.
    pub(crate) tag_new_name: String,
    /// The tag being renamed inline: its raw [`pixhaus_core::project::TagId`] value
    /// and draft text.
    pub(crate) tag_rename: Option<(u32, String)>,
    /// Cockpit subject prompt draft.
    pub(crate) rs_prompt: String,
    /// Selected composition Structure id (free-form `Single` by default).
    pub(crate) ck_structure: String,
    /// Anchor output aspect ratio. Applies to free-form output only; Paneled
    /// structures fix their own canvas.
    pub(crate) ck_aspect: crate::cockpit::AnchorAspect,
    /// Anchor output resolution on the long edge, in pixels (free-form only).
    pub(crate) ck_resolution: u32,
    /// Custom anchor output `(width, height)`, used when [`Self::ck_aspect`] is
    /// [`crate::cockpit::AnchorAspect::Custom`].
    pub(crate) ck_custom: (u32, u32),
    /// Requested candidate count (1-4). Seeded from the project's
    /// [`pixhaus_core::project::ProjectAi::default_candidate_count`] by
    /// [`ShellApp::reset_cockpit_form_defaults`].
    pub(crate) rs_num_variants: u32,
    /// Requested quality tier for a fresh generation. Seeded from the project's
    /// [`pixhaus_core::project::ProjectAi::default_quality`] by
    /// [`ShellApp::reset_cockpit_form_defaults`]. A promotion overrides this with
    /// [`pixhaus_core::project::Quality::High`].
    pub(crate) ck_quality: pixhaus_core::project::Quality,
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
    /// Monotonic id for anchor inpaint-refine jobs; a superseded run's late
    /// result is dropped by epoch.
    pub(crate) rs_refine_epoch: u64,
    /// Structured refinement metadata for the in-flight anchor refine — the
    /// painted mask or regional definition captured at the start of the run and
    /// stamped onto the landed variant. `None` for a prompt-only re-run.
    pub(crate) rs_pending_refinement: Option<pixhaus_core::project::RefinementKind>,
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
    /// Picked look Style id folded into the composed preview, or `None` to
    /// inherit only the project style notes.
    pub(crate) ck_style_id: Option<String>,
    /// Anchor influence in `0.0..=1.0`, fed to
    /// [`pixhaus_ai::plugin::AnchorPayload::from_sprite_entity`] when the
    /// approved anchor conditions a downstream generation. Transient UI state
    /// (no undo); defaults to
    /// [`pixhaus_ai::plugin::DEFAULT_ANCHOR_STRENGTH`].
    pub(crate) ck_anchor_strength: f32,
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
    /// Durable i2v job queue: owns each clip run's lifecycle, its cancel token,
    /// and on-disk persistence so an in-flight or finished clip survives an app
    /// restart. Mutated through `&mut self` — single owner, no lock.
    pub(crate) anim_jobs: crate::anim_jobs::AnimJobQueue,
    /// Id of the clip job currently in flight, if any. `cancel_clip` reads this
    /// to cancel the right job; cleared on any terminal transition. The Motion
    /// inspector reads it to show Cancel only for a cancellable i2v run.
    pub(crate) anim_job_id: Option<u64>,
    /// Monotonic generation id. Bumped on every Generate and on Cancel so a
    /// canceled or superseded clip's late messages are dropped. Distinct from a
    /// job id: the epoch drops stale *messages*, the id names a *durable record*.
    anim_gen_epoch: u64,
    /// Lineage parent captured by a re-roll/branch, stamped on the next clip
    /// card when it lands. `None` for a from-scratch generation.
    anim_pending_parent: Option<usize>,
    /// Monotonic id for neutral-anchor derivation runs; a superseded run's late
    /// result is dropped by epoch. Distinct from the clip epoch — a derivation
    /// is its own short-lived text-to-image pass.
    pub(crate) neutral_epoch: u64,
    /// Neutral-derivation job status (progress / failure), surfaced on the
    /// coverage grid's anchor header.
    pub(crate) neutral_status: JobStatus,
    /// Monotonic id for directional-anchor derivation runs (south/west/north); a
    /// superseded run's late result is dropped by epoch. East is never generated
    /// — it is the flip of west — so it has no derivation epoch.
    pub(crate) directional_epoch: u64,
    /// Directional-derivation job status (progress / failure), surfaced on the
    /// coverage grid's anchor header.
    pub(crate) directional_status: JobStatus,
    /// Animation-studio session state: the current stage, the anchor framing,
    /// the first-frame gallery and approved seed pose, the inpaint mask, and the
    /// motion-model pick. The clip candidates themselves live in `anim_*`, which
    /// the studio's clip/pick/land stages drive unchanged. All of it lives off
    /// the document until Land, so Cancel and restart are clean at every stage.
    pub(crate) studio: crate::studio::StudioState,
    /// Per-sprite durable studio sessions. The transient `studio` view is the
    /// active sprite's session; on a sprite switch it is flushed here and the new
    /// sprite's session is hydrated back (default if absent). Persisted to a
    /// `studio-sessions.json` sidecar so the last session per sprite survives an
    /// app restart. Interim home — moves into the project archive when the io
    /// crate lands.
    pub(crate) studio_sessions: HashMap<SpriteId, crate::studio::StudioSession>,
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
    /// Whether the studio plays the generation reveal effect (default on);
    /// persisted across launches.
    pub(crate) reveal_effect_enabled: bool,
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
    /// The selection-morphology dialog (Grow / Shrink / Feather), or `None` when
    /// closed. Carries which op runs and its current pixel amount.
    morph_dialog: Option<MorphDialog>,
    /// The open whole-canvas transform dialog (Rotate / Skew / Perspective /
    /// Antialias), or `None` when none is open. Carries the draft parameters.
    transform_dialog: Option<TransformDialog>,
    /// A transient one-line status message and when it was set, shown in the
    /// status bar and faded out after a few seconds. The non-modal way a
    /// rejected action (e.g. an invalid merge) surfaces its reason without a
    /// dialog. `None` when nothing is being shown.
    status_message: Option<(String, Instant)>,
    /// The conflict policy chosen in the open `.pixstyle` import modal, or
    /// `None` when the modal is closed. Opening the modal seeds it with a
    /// default; confirming spawns the import worker with the chosen policy.
    pub(crate) pack_import_policy: Option<pixhaus_core::project::ConflictPolicy>,
    /// The open sprite-export dialog (kind picker + atlas column count), or
    /// `None` when closed. Opened from the studio Land stage and the File menu;
    /// confirming spawns the off-thread encode worker.
    pub(crate) export_dialog: Option<ExportDialog>,
}

/// Draft state for the sprite-export dialog: which shape to write and, for the
/// atlas, the grid column count. Bounded so a corrupt count never zeroes the
/// pack.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ExportDialog {
    /// Which of the three shapes is selected.
    pub kind: ExportDialogKind,
    /// Atlas grid column count, used only when [`ExportDialogKind::Atlas`].
    pub columns: u32,
}

/// The export-shape choice in [`ExportDialog`]. Mirrors
/// [`crate::export::ExportKind`] without the atlas column payload, which the
/// dialog carries separately so switching kinds keeps the count.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ExportDialogKind {
    /// One transparent PNG per frame.
    PngSequence,
    /// An infinitely-looping, hard-masked GIF.
    LoopingGif,
    /// A packed atlas PNG plus a sidecar JSON manifest.
    Atlas,
}

impl Default for ExportDialog {
    fn default() -> Self {
        Self {
            kind: ExportDialogKind::LoopingGif,
            columns: 4,
        }
    }
}

/// The pending selection-morphology operation and its amount, driven by the
/// Grow / Shrink / Feather dialog. The amount is bounded to `1..=64` so the
/// `O(area * by^2)` morphology stays in budget at 8K.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct MorphDialog {
    /// Which morphology op the dialog applies on Apply.
    op: MorphOp,
    /// Pixel amount: grow/shrink radius, or feather radius.
    amount: u32,
}

/// The three selection-morphology commands the dialog drives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MorphOp {
    /// Dilate the mask outward by the amount (`expand`).
    Grow,
    /// Erode the mask inward by the amount (`contract`).
    Shrink,
    /// Soften the mask edges over the radius (`feather`).
    Feather,
}

impl MorphOp {
    /// The dialog title for this op.
    const fn title(self) -> &'static str {
        match self {
            Self::Grow => "Grow selection",
            Self::Shrink => "Shrink selection",
            Self::Feather => "Feather selection",
        }
    }

    /// The amount-field label for this op.
    const fn amount_label(self) -> &'static str {
        match self {
            Self::Grow | Self::Shrink => "Amount",
            Self::Feather => "Radius",
        }
    }
}

/// The inclusive amount range the morphology dialog clamps to. Bounds the
/// `O(area * by^2)` cost of grow/shrink so the pass stays responsive at 8K.
const MORPH_AMOUNT_RANGE: std::ops::RangeInclusive<u32> = 1..=64;

/// An open whole-canvas transform dialog and its draft parameters. One variant
/// per dialog-driven [`CanvasOp`]; closing it sets the field back to `None`.
/// Apply forwards the parameters to [`ShellApp::run_canvas_transform`], which
/// owns the offload threshold and undo wiring.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum TransformDialog {
    /// Arbitrary-angle rotation: degrees and the resampling algorithm.
    Rotate {
        /// Clockwise angle in degrees.
        degrees: f32,
        /// Resampling algorithm.
        algorithm: RotationAlgorithm,
    },
    /// Skew/shear: a horizontal and a vertical factor.
    Skew {
        /// Horizontal shear factor.
        fx: f32,
        /// Vertical shear factor.
        fy: f32,
    },
    /// Perspective warp via four destination corners, seeded to the identity
    /// rect (the source size) when opened.
    Perspective {
        /// Destination corners `[TL, TR, BR, BL]` in pixels.
        corners: [(f32, f32); 4],
    },
    /// Morphological antialias: threshold and softness.
    Mlaa {
        /// Edge-classifier threshold and softening factor.
        config: MlaaConfig,
    },
}

/// The inclusive shear-factor range the skew dialog clamps to. Bounds the
/// per-row/column shift so an extreme factor cannot push every pixel off-canvas
/// by accident.
const SKEW_FACTOR_RANGE: std::ops::RangeInclusive<f32> = -4.0..=4.0;

/// How long a transient status-bar message stays visible before it clears.
const STATUS_MESSAGE_TTL: Duration = Duration::from_secs(5);

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

        // Restore the reveal-effect preference (on by default).
        let reveal_effect_enabled = cc.storage.and_then(|s| eframe::get_value::<bool>(s, "reveal_effect_enabled")).unwrap_or(true);

        // Restore the saved keybindings (preset + custom overrides), defaulting
        // to the Aseprite preset with no overrides.
        let keymap = cc.storage.and_then(|s| eframe::get_value::<Keymap>(s, "keymap")).unwrap_or_default();

        // Restore the grid / overlay preferences, defaulting to the editor's own
        // defaults. Applied to the editor after the struct is built.
        let grid_prefs = cc.storage.and_then(|s| eframe::get_value::<GridPrefs>(s, "grid_prefs")).unwrap_or_default();

        // Restore the last-used new-sprite size, defaulting to 64×64.
        let (last_w, last_h) = cc
            .storage
            .and_then(|s| eframe::get_value::<(u32, u32)>(s, "new_sprite_size"))
            .unwrap_or((DEFAULT_CANVAS.width, DEFAULT_CANVAS.height));

        // Restore the per-sprite studio sessions from their app-data sidecar.
        // A missing or corrupt sidecar yields an empty map (a fresh install).
        let studio_sessions = crate::studio::load_studio_sessions();

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
            anim_set_open: false,
            library_tab: crate::library::LibraryTab::Templates,
            library_draft: None,
            asset_browser_open: false,
            asset_tex_cache: HashMap::new(),
            asset_rename: None,
            tag_new_name: String::new(),
            tag_rename: None,
            rs_prompt: ai::DEFAULT_SHEET_PROMPT.to_owned(),
            ck_structure: ai::SINGLE_STRUCTURE_ID.to_owned(),
            ck_aspect: crate::cockpit::AnchorAspect::Square,
            ck_resolution: 1536,
            ck_custom: (1536, 1536),
            rs_num_variants: 2,
            ck_quality: pixhaus_core::project::Quality::Medium,
            rs_status: JobStatus::Idle,
            rs_candidates: Vec::new(),
            rs_preview: None,
            rs_partial_preview: false,
            rs_refine_epoch: 0,
            rs_pending_refinement: None,
            ck_seed_fixed: false,
            ck_seed: 0,
            ck_positive: String::new(),
            ck_negative: String::new(),
            ck_prompt_edited: false,
            ck_dirty: true,
            ck_prompt_id: None,
            ck_style_id: None,
            ck_anchor_strength: pixhaus_ai::plugin::DEFAULT_ANCHOR_STRENGTH,
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
            anim_jobs: crate::anim_jobs::AnimJobQueue::new(),
            anim_job_id: None,
            anim_gen_epoch: 0,
            anim_pending_parent: None,
            neutral_epoch: 0,
            neutral_status: JobStatus::Idle,
            directional_epoch: 0,
            directional_status: JobStatus::Idle,
            studio: crate::studio::StudioState::default(),
            studio_sessions,
            studio_return: None,
            bg_key_color: Rgba::opaque(255, 0, 255),
            bg_tolerance: 24,
            bg_preview: false,
            bg_all_frames: false,
            bg_remove_pending: HashSet::new(),
            bg_flagged: Vec::new(),
            bg_status: JobStatus::Idle,
            theme_preference,
            reveal_effect_enabled,
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
            morph_dialog: None,
            transform_dialog: None,
            status_message: None,
            pack_import_policy: None,
            export_dialog: None,
        };
        grid_prefs.apply_to(&mut app.editor);
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

    /// Inserts the [`ViewportRenderer`] and the studio reveal effect's
    /// [`crate::reveal::RevealRenderer`] into egui-wgpu's callback resources so
    /// their paint callbacks can reach them.
    fn install_renderer(&self) {
        let Some(rs) = self.render_state.as_ref() else {
            tracing::error!("no wgpu render state; canvas will be blank");
            return;
        };
        let renderer = ViewportRenderer::new(&rs.device, rs.target_format);
        let reveal = crate::reveal::RevealRenderer::new(&rs.device, rs.target_format);
        let mut guard = rs.renderer.write();
        guard.callback_resources.insert(renderer);
        guard.callback_resources.insert(reveal);
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

    /// Sets a transient status-bar message (shown muted, fades out). The
    /// non-modal way an op reports a rejection or a brief result. Replaces any
    /// message already showing.
    pub(crate) fn set_status(&mut self, message: impl Into<String>) {
        self.status_message = Some((message.into(), Instant::now()));
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
                    if self.studio.anchor_reveal.is_active() {
                        if let Some((rgba, w, h)) = rgba_from_pixel_data(&pixels) {
                            let now = self.egui_ctx.input(|i| i.time);
                            self.studio.anchor_reveal.on_partial(rgba, w, h, now);
                        }
                    }
                    self.show_partial_preview(&pixels);
                }
                ShellMsg::SheetDone { variants, cost_usd } => {
                    if self.studio.anchor_reveal.is_active() {
                        let now = self.egui_ctx.input(|i| i.time);
                        match variants.first().and_then(|v| crate::reveal::decode_png_rgba(&v.png)) {
                            Some((rgba, w, h)) => self.studio.anchor_reveal.on_final(rgba, w, h, now),
                            None => self.studio.anchor_reveal.finish(now),
                        }
                    }
                    self.cockpit_on_done(variants, cost_usd);
                }
                ShellMsg::SheetFailed(err) => {
                    self.studio.anchor_reveal.fail();
                    self.clear_partial_preview();
                    self.rs_status = JobStatus::Failed(err);
                }
                ShellMsg::ClipProgress { epoch, message } => {
                    if epoch == self.anim_gen_epoch {
                        self.anim_status = JobStatus::Running(message);
                    }
                }
                ShellMsg::ClipReady {
                    epoch,
                    job_id,
                    clip,
                    mime,
                    frames,
                } => {
                    // Persist the durable record (and clip blob) even for a stale
                    // result, so the job leaves Running rather than reloading as
                    // Interrupted next launch.
                    self.anim_jobs.finish_done(job_id, &mime, &clip);
                    if epoch == self.anim_gen_epoch {
                        self.on_clip_ready(clip, mime, frames);
                    }
                }
                ShellMsg::ClipFailed { epoch, job_id, error } => {
                    self.anim_jobs.finish_error(job_id, &error);
                    if epoch == self.anim_gen_epoch {
                        self.anim_status = JobStatus::Failed(error);
                        self.anim_job_id = None;
                    }
                }
                ShellMsg::VideoImported {
                    entity_id,
                    sprite_id,
                    fps,
                    target,
                    result,
                } => self.on_video_imported(entity_id, sprite_id, fps, target, result),
                ShellMsg::AudioSynced { sprite_id, range, fps, result } => self.on_audio_synced(sprite_id, range, fps, result),
                ShellMsg::FirstFrameProgress { epoch, message } => {
                    if epoch == self.studio.frame_gen.epoch {
                        self.studio.frame_gen.status = JobStatus::Running(message);
                    }
                }
                ShellMsg::FirstFrameDone { epoch, images, parent, append } => {
                    if epoch == self.studio.frame_gen.epoch {
                        if self.studio.frame_gen.reveal.is_active() {
                            let now = self.egui_ctx.input(|i| i.time);
                            match images.first().and_then(|png| crate::reveal::decode_png_rgba(png)) {
                                Some((rgba, w, h)) => self.studio.frame_gen.reveal.on_final(rgba, w, h, now),
                                None => self.studio.frame_gen.reveal.finish(now),
                            }
                        }
                        self.on_gen_ready(images, parent, append);
                    }
                }
                ShellMsg::FirstFrameFailed { epoch, error } => {
                    if epoch == self.studio.frame_gen.epoch {
                        self.studio.frame_gen.reveal.fail();
                        self.studio.frame_gen.status = JobStatus::Failed(error);
                        self.studio.frame_gen.cancel = None;
                    }
                }
                ShellMsg::AnchorRefineDone { epoch, parent, image } => {
                    if epoch == self.rs_refine_epoch {
                        self.land_anchor_refine(parent, &image);
                    }
                }
                ShellMsg::AnchorRefineFailed { epoch, error } => {
                    if epoch == self.rs_refine_epoch {
                        self.rs_status = JobStatus::Failed(error);
                    }
                }
                ShellMsg::NeutralDerived {
                    epoch,
                    entity_id,
                    canonical_id,
                    image,
                } => {
                    if epoch == self.neutral_epoch {
                        self.land_neutral(entity_id, canonical_id, image);
                    }
                }
                ShellMsg::NeutralFailed { epoch, error } => {
                    if epoch == self.neutral_epoch {
                        self.neutral_status = JobStatus::Failed(error);
                    }
                }
                ShellMsg::DirectionalDerived {
                    epoch,
                    entity_id,
                    direction,
                    neutral_id,
                    image,
                } => {
                    if epoch == self.directional_epoch {
                        self.land_directional(entity_id, direction, neutral_id, image);
                    }
                }
                ShellMsg::DirectionalFailed { epoch, error } => {
                    if epoch == self.directional_epoch {
                        self.directional_status = JobStatus::Failed(error);
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
                #[cfg(feature = "lospec")]
                ShellMsg::LospecDone { slug, result } => {
                    self.on_lospec_done(&slug, result);
                }
                ShellMsg::PackExportDone { path } => match path {
                    Ok(Some(p)) => self.set_status(format!("Exported pack to {}", p.display())),
                    Ok(None) => {}
                    Err(e) => self.set_status(format!("Pack export failed: {e}")),
                },
                ShellMsg::PackImportDecoded { pack, policy } => match pack {
                    Ok(Some(pack)) => self.apply_imported_pack(pack, policy),
                    Ok(None) => {}
                    Err(e) => self.set_status(format!("Pack import failed: {e}")),
                },
                ShellMsg::SpriteExported { result } => self.on_sprite_exported(result),
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
                        self.entity_tag_chips(ui, sprite_ref.entity_id);
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
            // The frame selection is per-sprite UI state; a stale set would
            // point at frames the new sprite may not have.
            self.editor.clear_frame_selection();
            self.playing = false;
            self.refresh_canvas(true);
        }
    }

    /// Renders the confirmed tags of entity `entity_id` as small chips on the
    /// library tree row, resolving each [`pixhaus_core::project::TagId`] against
    /// the library's tag definitions. Tags with an accent color tint the chip;
    /// the rest fall back to a weak label. A no-op when the entity has no tags.
    fn entity_tag_chips(&self, ui: &mut egui::Ui, entity_id: pixhaus_core::project::EntityId) {
        let Some(entity) = self.doc.project.library.entities.iter().find(|e| e.id == entity_id) else {
            return;
        };
        if entity.tags.is_empty() {
            return;
        }
        for tag_id in &entity.tags {
            let Some(def) = self.doc.project.library.tags.iter().find(|t| t.id == *tag_id) else {
                continue;
            };
            let text = egui::RichText::new(format!("{} {}", crate::icons::TAG, def.name)).small();
            match def.color {
                Some(c) => {
                    ui.label(text.color(egui::Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a)));
                }
                None => {
                    ui.label(text.weak());
                }
            }
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

    /// Renders the delete-layers confirmation modal. Shown only for a delete
    /// destructive enough to ask first — more than one layer, or a group with
    /// children. Confirm runs the staged delete; Cancel clears it. A single leaf
    /// delete never reaches here (it deletes straight away), so this dialog is
    /// only ever opened for the multi/group case.
    fn show_delete_layers_confirm(&mut self, ctx: &egui::Context) {
        let Some(count) = self.editor.pending_layer_delete.as_ref().map(Vec::len) else {
            return;
        };
        let mut open = true;
        let mut confirm = false;
        let message = if count == 1 {
            "Delete this group and reparent its children?".to_owned()
        } else {
            format!("Delete {count} layers?")
        };
        egui::Window::new("Delete layers")
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .show(ctx, |ui| {
                ui.label(message);
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button(format!("{} Delete", crate::icons::TRASH)).clicked() {
                        confirm = true;
                    }
                    if ui.button("Cancel").clicked() {
                        self.cancel_pending_layer_delete();
                    }
                });
            });
        if confirm {
            self.confirm_pending_layer_delete();
        } else if !open {
            // Closing via the window's X is a cancel.
            self.cancel_pending_layer_delete();
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

    /// Opens a whole-canvas transform dialog with default parameters. The
    /// perspective dialog seeds its corners to the source-size identity rect so
    /// dragging them outward warps from the unchanged image.
    fn open_transform_dialog(&mut self, dialog: TransformDialog) {
        if self.doc.active_sprite().is_none() {
            return;
        }
        let dialog = if let TransformDialog::Perspective { .. } = dialog {
            // Re-seed perspective corners to the active sprite's identity rect,
            // ignoring the placeholder the menu passed in.
            #[allow(clippy::cast_precision_loss)]
            let (w, h) = self.doc.active_sprite().map_or((0.0, 0.0), |s| (s.canvas.width as f32, s.canvas.height as f32));
            TransformDialog::Perspective {
                corners: [(0.0, 0.0), (w, 0.0), (w, h), (0.0, h)],
            }
        } else {
            dialog
        };
        self.transform_dialog = Some(dialog);
    }

    /// Renders the open whole-canvas transform dialog (Rotate / Skew /
    /// Perspective / Antialias). Each is modeled on [`ShellApp::show_resize_dialog`]:
    /// draft fields plus Apply / Cancel. Apply closes the dialog and forwards the
    /// op to [`ShellApp::run_canvas_transform`], which owns the offload threshold
    /// and undo wiring.
    #[allow(clippy::too_many_lines)]
    fn show_transform_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut dialog) = self.transform_dialog else {
            return;
        };
        let title = match dialog {
            TransformDialog::Rotate { .. } => "Rotate",
            TransformDialog::Skew { .. } => "Skew",
            TransformDialog::Perspective { .. } => "Perspective",
            TransformDialog::Mlaa { .. } => "Antialias (MLAA)",
        };
        let mut open = true;
        let mut apply = false;
        egui::Window::new(title).collapsible(false).resizable(false).open(&mut open).show(ctx, |ui| {
            match &mut dialog {
                TransformDialog::Rotate { degrees, algorithm } => {
                    ui.horizontal(|ui| {
                        ui.label("Angle");
                        ui.add(egui::DragValue::new(degrees).range(-360.0..=360.0).speed(1.0).suffix("°"));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Algorithm");
                        egui::ComboBox::from_id_salt("rotate_algorithm")
                            .selected_text(rotation_algorithm_label(*algorithm))
                            .show_ui(ui, |ui| {
                                ui.selectable_value(algorithm, RotationAlgorithm::RotSprite, rotation_algorithm_label(RotationAlgorithm::RotSprite));
                                ui.selectable_value(algorithm, RotationAlgorithm::Bilinear, rotation_algorithm_label(RotationAlgorithm::Bilinear));
                                ui.selectable_value(
                                    algorithm,
                                    RotationAlgorithm::NearestNeighbor,
                                    rotation_algorithm_label(RotationAlgorithm::NearestNeighbor),
                                );
                            });
                    });
                }
                TransformDialog::Skew { fx, fy } => {
                    ui.horizontal(|ui| {
                        ui.label("X factor");
                        ui.add(egui::DragValue::new(fx).range(SKEW_FACTOR_RANGE).speed(0.05));
                        ui.label("Y factor");
                        ui.add(egui::DragValue::new(fy).range(SKEW_FACTOR_RANGE).speed(0.05));
                    });
                }
                TransformDialog::Perspective { corners } => {
                    ui.label("Destination corners (px)");
                    let labels = ["Top-left", "Top-right", "Bottom-right", "Bottom-left"];
                    for (corner, name) in corners.iter_mut().zip(labels) {
                        ui.horizontal(|ui| {
                            ui.label(name);
                            ui.add(egui::DragValue::new(&mut corner.0).speed(1.0).prefix("x "));
                            ui.add(egui::DragValue::new(&mut corner.1).speed(1.0).prefix("y "));
                        });
                    }
                }
                TransformDialog::Mlaa { config } => {
                    ui.horizontal(|ui| {
                        ui.label("Threshold");
                        ui.add(egui::Slider::new(&mut config.threshold, 0..=255));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Softness");
                        ui.add(egui::Slider::new(&mut config.softness, 0..=255));
                    });
                }
            }
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Apply").clicked() {
                    apply = true;
                }
                if ui.button("Cancel").clicked() {
                    self.transform_dialog = None;
                }
            });
        });
        if !open {
            self.transform_dialog = None;
        }
        // Persist live edits to the draft even before Apply is pressed.
        if self.transform_dialog.is_some() {
            self.transform_dialog = Some(dialog);
        }
        if apply {
            self.transform_dialog = None;
            let op = match dialog {
                TransformDialog::Rotate { degrees, algorithm } => CanvasOp::RotateArbitrary { degrees, algorithm },
                TransformDialog::Skew { fx, fy } => CanvasOp::Skew { fx, fy },
                TransformDialog::Perspective { corners } => CanvasOp::Perspective { corners },
                TransformDialog::Mlaa { config } => CanvasOp::Mlaa { config },
            };
            self.run_canvas_transform(op);
        }
    }

    /// Opens the selection-morphology dialog for `op`, seeding its amount: grow
    /// and shrink default to 1 pixel, feather to 0 (off). A no-op with no active
    /// selection — the menu items are disabled in that case.
    fn open_morph_dialog(&mut self, op: MorphOp) {
        if self.editor.selection.is_none() {
            return;
        }
        let amount = match op {
            MorphOp::Grow | MorphOp::Shrink => 1,
            MorphOp::Feather => 0,
        };
        self.morph_dialog = Some(MorphDialog { op, amount });
    }

    /// Draws the Grow / Shrink / Feather dialog: an amount field and Apply /
    /// Cancel. Apply runs the op on the current mask and closes the dialog.
    fn show_morph_dialog(&mut self, ctx: &egui::Context) {
        let Some(dialog) = self.morph_dialog else {
            return;
        };
        let mut open = true;
        let mut apply = false;
        let mut amount = dialog.amount;
        // Feather goes off at 0; grow/shrink need at least 1 to do anything.
        let range = if dialog.op == MorphOp::Feather {
            0..=*MORPH_AMOUNT_RANGE.end()
        } else {
            MORPH_AMOUNT_RANGE
        };
        egui::Window::new(dialog.op.title())
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(dialog.op.amount_label());
                    ui.add(egui::DragValue::new(&mut amount).range(range).suffix(" px"));
                });
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("Apply").clicked() {
                        apply = true;
                    }
                    if ui.button("Cancel").clicked() {
                        self.morph_dialog = None;
                    }
                });
            });
        if !open {
            self.morph_dialog = None;
        }
        // Persist a live edit to the amount even if Apply isn't pressed yet.
        if let Some(d) = self.morph_dialog.as_mut() {
            d.amount = amount;
        }
        if apply {
            self.morph_dialog = None;
            self.apply_morphology(dialog.op, amount);
        }
    }

    /// Runs the chosen morphology op on the active selection in place. Grow and
    /// shrink replace the mask with the dilated/eroded result; feather replaces
    /// it with the softened (coverage-graded) mask the clip blend then honours.
    /// A no-op with no selection or on `Err(DimensionOverflow)` — the mask is
    /// left untouched rather than cleared.
    pub(crate) fn apply_morphology(&mut self, op: MorphOp, amount: u32) {
        let Some(mask) = self.editor.selection.as_ref() else {
            return;
        };
        match morph_mask(mask, op, amount) {
            MorphResult::Unchanged => {}
            MorphResult::Cleared => self.editor.selection = None,
            MorphResult::Replaced(out) => self.editor.selection = Some(out),
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

    /// Crops the canvas to the active selection's bounding box: shrinks the
    /// sprite to that rect and copies each raster cel's sub-rectangle into a
    /// smaller buffer, as one undo entry. A no-op without a selection (the menu
    /// item is disabled then) or when the bounds are empty. The selection mask is
    /// canvas-scale, so its [`SelectionMask::bounds`] is already in canvas space.
    pub(crate) fn crop_to_selection(&mut self) {
        let Some(mask) = self.editor.selection.as_ref() else {
            return;
        };
        let bounds = mask.bounds();
        if bounds.is_empty() {
            return;
        }
        let Some(sprite_id) = self.doc.project.active_sprite_id() else {
            return;
        };
        let Some(before_sprite) = self.doc.project.sprite(sprite_id).cloned() else {
            return;
        };

        // Clamp the rect to the canvas: bounds is sourced from a canvas-scale
        // mask, but a selection edited under a since-resized sprite could overhang.
        let canvas = before_sprite.canvas;
        #[allow(clippy::cast_sign_loss)]
        let bx = bounds.origin.x.max(0) as u32;
        #[allow(clippy::cast_sign_loss)]
        let by = bounds.origin.y.max(0) as u32;
        if bx >= canvas.width || by >= canvas.height {
            return;
        }
        let bw = bounds.size.width.min(canvas.width - bx);
        let bh = bounds.size.height.min(canvas.height - by);
        // Floor at a non-zero crop; a zero-area rect has nothing to keep. The
        // MAX_CANVAS_DIM ceiling needs no check here — crop only shrinks.
        if bw == 0 || bh == 0 {
            return;
        }
        // Cropping to the whole canvas changes nothing.
        if bx == 0 && by == 0 && bw == canvas.width && bh == canvas.height {
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

        let mut swaps: Vec<CanvasBufferSwap> = Vec::with_capacity(ids.len());
        for id in ids {
            let Some(before) = self.doc.pixel_buffers.get(&id).cloned() else {
                continue;
            };
            let Ok(after) = pixhaus_core::transforms::crop(&before, bx, by, bw, bh) else {
                // A degenerate crop (empty buffer or zero-area rect) is already
                // ruled out above; bail rather than commit a partial edit.
                return;
            };
            swaps.push(CanvasBufferSwap { id, before, after });
        }

        let new_size = Size::new(bw, bh);
        let mut after_sprite = before_sprite.clone();
        after_sprite.canvas = new_size;
        for cel in &mut after_sprite.cels {
            if let CelData::Raster { size, .. } = &mut cel.data {
                *size = new_size;
            }
        }

        let edit = CropEdit {
            sprite_id,
            before_sprite,
            after_sprite,
            buffers: swaps,
            label: "Crop to selection".to_owned(),
        };
        let _ = self.editor.history.push(Box::new(edit), &mut self.doc);
        // The crop consumed the selection; its coordinates no longer map.
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
            if let Some(next) = next_cursor(self.play_cursor, self.play_order.len(), self.editor.loop_playback) {
                self.play_cursor = next;
                self.doc.active_frame = self.play_order[self.play_cursor];
                self.last_advance = Instant::now();
                self.refresh_canvas(false);
            } else {
                // Loop off and at the last frame: halt, leaving the playhead on
                // the final frame. Stop requesting repaints so the loop goes
                // idle.
                self.playing = false;
                return;
            }
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

    /// Kicks off a clip generation on the tokio runtime. The run is registered
    /// as a durable job so it survives an app restart (recovered as
    /// `Interrupted`), and its cancel token rides on the queue so the Cancel
    /// button can abort the right job.
    pub(crate) fn start_clip(&mut self) {
        let Some(anchor) = self.doc.active_anchor().map(<[u8]>::to_vec) else {
            return;
        };
        let Some(sprite) = self.doc.active_sprite() else {
            return;
        };
        let Some(entity_id) = self.doc.active_entity_id() else {
            return;
        };
        let sprite_id = sprite.id;
        let seed = self.anim_seed_fixed.then_some(self.anim_seed);
        let model = self.studio.i2v_model.model_id();
        // Resolve the art-style kind from the picked style at generation time so
        // the finisher gate matches what was generated, not a later picker value.
        let art_style_kind = ai::resolve_art_style_kind(&self.doc.project.library.ai, self.ck_style_id.as_deref());
        let job = ai::AnimJob {
            canvas: (sprite.canvas.width, sprite.canvas.height),
            anchor_png: anchor,
            // The studio's approved seed pose drives the i2v call; without one
            // the clip path generates its own first frame from the anchor.
            first_frame_png: self.studio.approved_first_frame.clone(),
            motion_prompt: self.anim_motion.clone(),
            i2v_model: Some(model.clone()),
            target_frames: self.anim_target_frames,
            fps: self.anim_fps,
            seed,
            art_style_kind,
        };
        self.record_recent_motion(&self.anim_motion.clone());
        // Register the durable job. `start` returns the id (for the terminal
        // arms) and the cancel token (handed to the spawned task).
        let (job_id, cancel) = self.anim_jobs.start(
            entity_id,
            sprite_id,
            self.studio.facing.as_str().to_owned(),
            Some(self.studio.kind.as_str().to_owned()),
            model,
            self.anim_motion.clone(),
            self.anim_target_frames,
            self.anim_fps,
            seed,
        );
        self.anim_job_id = Some(job_id);
        // The epoch stays as the message-staleness tag, distinct from the job id.
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
            job_id,
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

    /// Cancels the running clip generation and clears the busy status. Fires the
    /// job's cancel token and flips its durable record to `Cancelled`.
    pub(crate) fn cancel_clip(&mut self) {
        if let Some(id) = self.anim_job_id.take() {
            self.anim_jobs.cancel(id);
            self.anim_jobs.finish_cancelled(id);
        }
        // Bump the epoch so the aborted job's late messages are dropped.
        self.anim_gen_epoch += 1;
        self.anim_status = JobStatus::Idle;
    }

    /// Receives a generated clip: lands it as a new gallery card with seeded
    /// loop markers and picks, then selects it for editing.
    fn on_clip_ready(&mut self, clip: Vec<u8>, mime: String, frames: Vec<VideoFrame>) {
        self.anim_status = JobStatus::Idle;
        self.anim_job_id = None;
        let motion = self.anim_motion.clone();
        let fps = self.anim_fps;
        let seed = self.anim_seed_fixed.then_some(self.anim_seed);
        let parent = self.anim_pending_parent.take();
        self.push_clip_candidate(clip, mime, frames, motion, fps, seed, parent);
    }

    /// Pushes a decoded clip as a new gallery card with auto-detected loop
    /// markers and evenly-spaced picks, then selects it. Shared by the i2v result
    /// path ([`on_clip_ready`]) and the user-video import path
    /// ([`on_video_imported`]), which differ only in provenance (motion prompt,
    /// fps, seed, lineage).
    fn push_clip_candidate(
        &mut self,
        clip: Vec<u8>,
        mime: String,
        frames: Vec<VideoFrame>,
        motion: String,
        fps: u32,
        seed: Option<u64>,
        parent: Option<usize>,
    ) {
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
            motion,
            fps,
            seed,
            parent,
            card_texture: None,
            keyed_thumbs,
            keyed_sig: None,
            frames,
        });
        let idx = self.anim_candidates.len() - 1;
        self.select_clip(idx);
    }

    /// Picks a video file (off the UI thread), reads and decodes it through
    /// [`anim::decode_clip`], and routes the frames through the same clip path an
    /// i2v generation uses. The approved-first-frame and motion-prompt fields play
    /// no part; the rest of the pipeline (Clip -> Pick -> Normalize -> Land) is
    /// identical. The dialog, file read, and decode block, so they run on a
    /// worker; the result returns over [`ShellMsg::VideoImported`].
    pub(crate) fn import_video_clip(&mut self) {
        let Some(entity_id) = self.doc.active_entity_id() else {
            self.set_status("Approve an anchor before importing a video.");
            return;
        };
        let Some(sprite) = self.doc.active_sprite() else {
            return;
        };
        let sprite_id = sprite.id;
        let fps = self.anim_fps;
        let target = self.anim_target_frames;
        let tx = self.tx.clone();
        let ctx = self.egui_ctx.clone();
        self.anim_status = JobStatus::Running("importing video".into());
        self.runtime.handle().spawn_blocking(move || {
            let result = pick_and_decode_video(fps);
            let _ = tx.send(ShellMsg::VideoImported {
                entity_id,
                sprite_id,
                fps,
                target,
                result,
            });
            ctx.request_repaint();
        });
    }

    /// Lands a decoded user video as a clip candidate, or surfaces a clear error.
    /// On success, when the job queue has a storage dir, the import is recorded as
    /// a `Done` job tagged `model = "imported"` so it shows in the jobs history
    /// alongside generated clips.
    fn on_video_imported(
        &mut self,
        entity_id: pixhaus_core::project::EntityId,
        sprite_id: SpriteId,
        fps: u32,
        target: u32,
        result: Result<(Vec<u8>, String, Vec<VideoFrame>), String>,
    ) {
        self.anim_status = JobStatus::Idle;
        match result {
            Ok((clip, mime, frames)) => {
                self.anim_jobs.record_import(
                    entity_id,
                    sprite_id,
                    self.studio.facing.as_str().to_owned(),
                    Some(self.studio.kind.as_str().to_owned()),
                    &mime,
                    target,
                    fps,
                    &clip,
                );
                // Imported clips have no i2v lineage, prompt, or seed.
                self.anim_pending_parent = None;
                self.push_clip_candidate(clip, mime, frames, "imported video".into(), fps, None, None);
                self.set_status("Imported video clip.");
            }
            Err(err) => self.set_status(err),
        }
    }

    /// Picks a WAV file (off the UI thread), detects beat onsets with the core
    /// energy-envelope detector, and converts them to per-frame durations at the
    /// timeline FPS. The result returns over [`ShellMsg::AudioSynced`], which
    /// retimes the active tag's range (or the whole timeline) in one undo entry.
    ///
    /// The dialog and file read block, so they run on a worker. The audio is
    /// read for timing only — nothing is embedded in the project. Beat mode
    /// only; the old verb's lip-sync mouth layer is out of scope.
    #[allow(clippy::cast_possible_truncation)]
    pub(crate) fn sync_to_audio(&mut self) {
        let Some(sprite) = self.doc.active_sprite() else {
            self.set_status("Open a sprite before syncing to audio.");
            return;
        };
        let frame_count = sprite.frames.len() as u32;
        if frame_count == 0 {
            self.set_status("This sprite has no frames to retime.");
            return;
        }
        let sprite_id = sprite.id;

        // Retime the selected tag's range; with no selection, the whole
        // timeline. The range is captured now so a later sprite switch cannot
        // misdirect the durations.
        let range = self.editor.selected_tag.and_then(|i| sprite.frame_tags.get(i)).map_or_else(
            || pixhaus_core::project::FrameRange::new(FrameIndex::new(0), FrameIndex::new(frame_count - 1)),
            |t| t.range,
        );

        let fps = self.editor.global_fps.max(1);
        let tx = self.tx.clone();
        let ctx = self.egui_ctx.clone();
        self.set_status("Syncing to audio…");
        self.runtime.handle().spawn_blocking(move || {
            let result = pick_and_detect_audio(fps);
            let _ = tx.send(ShellMsg::AudioSynced { sprite_id, range, fps, result });
            ctx.request_repaint();
        });
    }

    /// Applies detected beat durations to the timeline as one undo entry: every
    /// frame in `range` takes the duration of the beat it maps to, and a `"Beat"`
    /// frame tag is added (or its range adjusted) to mark the synced span.
    ///
    /// The beat count rarely matches the frame count exactly. The durations are
    /// mapped across the range proportionally — beat `i` covers a contiguous run
    /// of frames — so a 6-beat detection over 12 frames holds each beat for two
    /// frames. With more beats than frames the tail beats are dropped.
    #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss, clippy::cast_sign_loss)]
    fn on_audio_synced(&mut self, sprite_id: SpriteId, range: pixhaus_core::project::FrameRange, _fps: u32, result: Result<Vec<u32>, String>) {
        let durations = match result {
            Ok(d) => d,
            Err(err) => {
                self.set_status(err);
                return;
            }
        };
        if durations.is_empty() {
            self.set_status("No beats detected; try a clearer track or a different range.");
            return;
        }
        // Guard against a sprite switch between pick and result.
        if self.doc.project.active_sprite_id() != Some(sprite_id) {
            self.set_status("Active sprite changed; audio sync was discarded.");
            return;
        }

        let beat_count = durations.len();
        push_sprite_edit(&mut self.editor, &mut self.doc, "Sync timing to audio", |sprite| {
            let frame_total = sprite.frames.len() as u32;
            // Clamp the captured range to the current frame count.
            let lo = range.start.get().min(frame_total.saturating_sub(1));
            let hi = range.end.get().min(frame_total.saturating_sub(1));
            if frame_total == 0 || hi < lo {
                return;
            }
            let span = hi - lo + 1;

            // Spread the beats evenly across the range, then write each frame's
            // duration from the beat it maps to.
            let mapped = map_beats_to_frames(&durations, span);
            for (offset, &dur) in mapped.iter().enumerate() {
                if let Some(frame) = sprite.frames.get_mut(lo as usize + offset) {
                    frame.duration_ms = dur.max(1);
                }
            }

            // Add or adjust the "Beat" tag over the retimed range.
            let beat_range = pixhaus_core::project::FrameRange::new(FrameIndex::new(lo), FrameIndex::new(hi));
            if let Some(tag) = sprite.frame_tags.iter_mut().find(|t| t.name == "Beat") {
                tag.range = beat_range;
            } else {
                sprite.frame_tags.push(pixhaus_core::project::FrameTag {
                    name: "Beat".into(),
                    range: beat_range,
                    loop_direction: LoopDirection::Forward,
                    repeat: 0,
                    user_data: pixhaus_core::project::UserData::default(),
                });
            }
        });
        self.set_status(format!("Synced {beat_count} beats to the timeline."));
    }

    /// The inputs that determine the normalize result for the current picks, or
    /// `None` when there is nothing to normalize (no selection, empty picks, or
    /// no active sprite). Cheap to compute and compare; the Normalize stage uses
    /// it to invalidate its cached pass when the picks or key change.
    pub(crate) fn normalize_cache_key(&self) -> Option<crate::studio::NormalizeCacheKey> {
        let i = self.anim_selected?;
        let cand = self.anim_candidates.get(i)?;
        if cand.picks.is_empty() {
            return None;
        }
        let sprite = self.doc.active_sprite()?;
        Some(crate::studio::NormalizeCacheKey {
            clip: i,
            picks: cand.picks.clone(),
            canvas: (sprite.canvas.width, sprite.canvas.height),
            remove_on_land: self.studio.remove_on_land,
            key_color: (self.bg_key_color.r, self.bg_key_color.g, self.bg_key_color.b),
            key_tolerance: self.bg_tolerance,
            all_parts: self.studio.land_all_parts,
            min_area: self.studio.land_min_area,
        })
    }

    /// Runs the normalization pass over the selected card's picks and returns the
    /// full [`NormalizeResult`] (frames, metrics, and the report). `None` when
    /// there is nothing to normalize or the pass fails — the failure surfaces on
    /// `anim_status` so the studio can show it. The pass is CPU-only over a
    /// handful of small pick buffers (bounded by the pick region, not the 8K
    /// canvas), so it runs inline.
    pub(crate) fn compute_normalize(&mut self) -> Option<NormalizeResult> {
        let i = self.anim_selected?;
        if self.anim_candidates[i].picks.is_empty() {
            return None;
        }
        let sprite = self.doc.active_sprite()?;
        let (cw, ch) = (sprite.canvas.width, sprite.canvas.height);
        let buffers: Vec<PixelBuffer> = self.anim_candidates[i]
            .picks
            .iter()
            .filter_map(|&p| self.anim_candidates[i].frames.get(p))
            .filter_map(video_frame_to_pixel_buffer)
            .collect();
        if buffers.is_empty() {
            return None;
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
            // Isolate the hero from detached keying specks on Land, so a stray
            // pixel cannot inflate the bbox and shrink the body. WholeAlpha when
            // the backdrop is left for the timeline op (no keying, no specks).
            component_mode: crate::studio::land_component_mode(self.studio.remove_on_land, self.studio.land_all_parts, self.studio.land_min_area),
            // Edge-touch QC flags a landed bbox literally at a canvas edge. No
            // safe band on Land — the studio surfaces it advisory, not as a gate.
            safe_margin: 0,
        };
        match normalize_frames(&buffers, &opts) {
            Ok(result) => Some(result),
            Err(err) => {
                self.anim_status = JobStatus::Failed(format!("normalize: {err}"));
                None
            }
        }
    }

    /// Integrates the reviewed, normalized picks onto the timeline. Land commits
    /// exactly the frames the Normalize stage reviewed — it reuses the cached
    /// [`NormalizeResult`] when fresh, and only recomputes as a fallback (a
    /// caller that skipped the stage), never a second pass over an already
    /// reviewed result. The frames may still carry their background unless the
    /// studio keyed it out on Land; the clip stays in the gallery and the canvas
    /// returns to the sprite.
    pub(crate) fn integrate_picked(&mut self) {
        let Some(i) = self.anim_selected else {
            return;
        };
        // Prefer the reviewed result; recompute only when the cache is stale or
        // absent so Land never double-normalizes what Normalize already showed.
        let result = match (self.studio.normalize.take(), self.normalize_cache_key()) {
            (Some(cached), key) if self.studio.normalize_key == key => cached,
            _ => match self.compute_normalize() {
                Some(result) => result,
                None => return,
            },
        };
        self.studio.normalize = None;
        self.studio.normalize_key = None;
        self.studio.normalize_textures.clear();
        if result.frames.is_empty() {
            return;
        }
        // East-as-flip-of-west: when the studio is facing east and the entity has
        // enabled `east_from_west`, east frames are the west loop mirrored. Flip
        // the reviewed frames horizontally before integrate; the loop still plays
        // forward (a mirror is not a reversal). Correct only for left-right-
        // symmetric subjects — the cascade grid badges east as a flip, not a
        // generation. The flip is bounded by the pick region, not the 8K canvas.
        let frames = if self.east_flip_on_land() {
            flip_frames_horizontal(result.frames)
        } else {
            result.frames
        };
        // Style gate (Brief 8): pixel-class styles get the palette-snap +
        // downscale finisher before the frames hit the timeline; clean-HD/map
        // land the normalized frames verbatim. Resolve the kind from the picked
        // style, not a stale value. The frames are already canvas-sized, so the
        // finisher snaps colour without changing dimensions (a no-op downscale).
        let art_style_kind = ai::resolve_art_style_kind(&self.doc.project.library.ai, self.ck_style_id.as_deref());
        let frames = if art_style_kind.is_pixel() {
            self.finish_landing_frames(frames)
        } else {
            frames
        };
        let fps = self.anim_candidates[i].fps;
        let frame_ms = (1000 / fps.max(1)).max(1);
        let motion = self.anim_candidates[i].motion.clone();
        integrate_frames_undoable(&mut self.editor, &mut self.doc, frames, frame_ms, &motion, LoopDirection::Forward);
        // Record the directional-cascade edge as a second, separately-undoable
        // entry: the frames landed as "Integrate animation", and the edge as
        // "Record cascade edge". `motion` is the frame tag integrate created, so
        // the edge names the same range. Skipped for custom animations.
        self.record_cascade_edge(&motion);
        // Drop the canvas preview back to the sprite/timeline; keep the gallery.
        self.anim_selected = None;
        self.anim_clip_playing = false;
        self.exit_clip_preview();
    }

    /// Runs the pixel finisher over the landing `frames` and seeds the active
    /// sprite's palette from the result. Returns the snapped, grid-aligned
    /// buffers. The frames are already canvas-sized so the downscale is a no-op
    /// and only the palette snap changes pixels. The palette is seeded only when
    /// the sprite is still on its default palette, so a curated palette is not
    /// clobbered. Pure aside from the palette seed; bounded by the frame count
    /// and canvas, not the 8K limit (small post-grid buffers).
    ///
    /// When the active entity carries an anchor palette (the studio derives from
    /// the approved anchor), the snap uses `Fixed(anchor_palette)` so the loop
    /// keeps the established anchor colours instead of re-extracting a fresh
    /// per-frame palette that can drift off the anchor (Brief 8's over-quantising
    /// guard). Falls back to `Extract` (`for_canvas`) when no anchor palette is
    /// present.
    fn finish_landing_frames(&mut self, frames: Vec<PixelBuffer>) -> Vec<PixelBuffer> {
        let Some((w, h)) = self.doc.active_sprite().map(|s| (s.canvas.width, s.canvas.height)) else {
            return frames;
        };
        let anchor_palette = self.active_anchor_palette();
        let opts = landing_finish_options(w, h, &anchor_palette);
        let Ok(finished) = pixhaus_core::transforms::finisher::finish_frames(&frames, &opts) else {
            return frames;
        };
        // Seed the sprite palette from the first frame's snapped palette so the
        // landed loop and the palette panel agree. Only when the sprite still
        // carries the editor default palette, never over a curated one.
        if let Some(palette) = finished.iter().map(|f| &f.palette).find(|p| !p.colors.is_empty()) {
            let colors: Vec<Rgba> = palette.colors.iter().map(|e| e.color).collect();
            if let Some(sprite) = self.doc.active_sprite_mut() {
                if let Some(first) = sprite.palettes.first_mut() {
                    if first.name == "default" {
                        first.colors = colors.into_iter().map(pixhaus_core::project::PaletteEntry::new).collect();
                    }
                }
            }
        }
        finished.into_iter().map(|f| f.buffer).collect()
    }

    /// Whether Land should mirror the picked frames for east: `true` only when the
    /// studio is facing east and the active entity has enabled `east_from_west`.
    /// East is the flip of west — no clip is generated for it — so Land mirrors the
    /// reviewed west loop. Reads the active entity's `CharacterAnchor`; `false`
    /// when no entity is active or its sheet carries no anchor.
    fn east_flip_on_land(&self) -> bool {
        if self.studio.facing != crate::studio::Facing::East {
            return false;
        }
        let Some(entity_id) = self.doc.active_entity_id() else {
            return false;
        };
        let Some(entity) = self.doc.project.library.entities.iter().find(|e| e.id == entity_id) else {
            return false;
        };
        let pixhaus_core::project::EntityContent::Sprites {
            reference_sheet: Some(sheet), ..
        } = &entity.content
        else {
            return false;
        };
        sheet.anchor.directional.east_from_west
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

    /// Opens the sprite-export dialog, seeded with the looping-GIF default. A
    /// no-op (with a status note) when no sprite has any frames to export.
    pub(crate) fn open_export_dialog(&mut self) {
        if self.doc.frame_count() == 0 {
            self.set_status("Nothing to export: the sprite has no frames");
            return;
        }
        self.export_dialog = Some(ExportDialog::default());
    }

    /// The export dialog: pick the shape (PNG sequence / looping GIF / atlas)
    /// and, for an atlas, the column count. Export spawns the off-thread worker
    /// and closes the dialog; Cancel just closes it.
    pub(crate) fn show_export_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut dialog) = self.export_dialog else {
            return;
        };
        let mut open = true;
        let mut confirm = false;
        egui::Window::new(format!("{} Export sprite", crate::icons::DOWNLOAD))
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .show(ctx, |ui| {
                ui.label(egui::RichText::new("Shape").strong());
                ui.radio_value(&mut dialog.kind, ExportDialogKind::PngSequence, "PNG frame sequence")
                    .on_hover_text("One transparent PNG per frame");
                ui.radio_value(&mut dialog.kind, ExportDialogKind::LoopingGif, "Looping GIF")
                    .on_hover_text("Infinitely-looping GIF with a hard alpha mask (no halo)");
                ui.radio_value(&mut dialog.kind, ExportDialogKind::Atlas, "Packed atlas + manifest")
                    .on_hover_text("One PNG grid plus a JSON manifest of frame rects");
                if dialog.kind == ExportDialogKind::Atlas {
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.label("Columns");
                        ui.add(egui::DragValue::new(&mut dialog.columns).range(1..=64));
                    });
                }
                ui.add_space(8.0);
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button(format!("{} Export…", crate::icons::DOWNLOAD)).clicked() {
                        confirm = true;
                    }
                    if ui.button("Cancel").clicked() {
                        self.export_dialog = None;
                    }
                });
            });
        // Keep live edits until the dialog closes.
        if self.export_dialog.is_some() {
            self.export_dialog = Some(dialog);
        }
        if !open {
            self.export_dialog = None;
        }
        if confirm {
            self.export_dialog = None;
            self.spawn_export(dialog);
        }
    }

    /// Composites the active sprite's frames and spawns the off-thread export
    /// worker for `dialog`'s shape. The frames are read into owned buffers on
    /// the UI thread before the worker spawns, so no document borrow crosses the
    /// `rfd` dialog. The result returns over [`ShellMsg::SpriteExported`].
    fn spawn_export(&mut self, dialog: ExportDialog) {
        let mut frames = Vec::with_capacity(self.doc.frame_count());
        for i in 0..self.doc.frame_count() {
            let idx = FrameIndex::new(u32::try_from(i).unwrap_or(0));
            if let Some(frame) = self.doc.composite_frame(idx) {
                frames.push(frame);
            }
        }
        if frames.is_empty() {
            self.set_status("Nothing to export: the sprite has no frames");
            return;
        }
        let frame_ms = self.doc.frame_duration_ms(FrameIndex::new(0));
        let base_name = self.doc.active_sprite().map_or_else(|| "sprite".to_owned(), |s| sanitize_base_name(&s.name));
        let kind = match dialog.kind {
            ExportDialogKind::PngSequence => crate::export::ExportKind::PngSequence,
            ExportDialogKind::LoopingGif => crate::export::ExportKind::LoopingGif,
            ExportDialogKind::Atlas => crate::export::ExportKind::Atlas {
                columns: dialog.columns.max(1),
            },
        };
        let req = crate::export::ExportRequest::new(kind, frames, frame_ms, base_name);
        let tx = self.tx.clone();
        let ctx = self.egui_ctx.clone();
        // spawn_blocking: the rfd dialog and the encode both block, kept off the
        // egui thread. The owned frames move into the worker.
        self.runtime.handle().spawn_blocking(move || {
            let result = crate::export::run_export(req);
            let _ = tx.send(ShellMsg::SpriteExported { result });
            ctx.request_repaint();
        });
        self.set_status("Exporting sprite…");
    }

    /// Surfaces a finished export on the status line. A cancelled dialog
    /// (`Ok(None)`) stays silent.
    fn on_sprite_exported(&mut self, result: Result<Option<std::path::PathBuf>, String>) {
        match result {
            Ok(Some(path)) => self.set_status(format!("Exported sprite to {}", path.display())),
            Ok(None) => {}
            Err(e) => self.set_status(format!("Export failed: {e}")),
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
                let has_frames = self.doc.frame_count() > 0;
                if ui
                    .add_enabled(has_frames, egui::Button::new(format!("{} Export sprite…", crate::icons::DOWNLOAD)))
                    .on_disabled_hover_text("Create a sprite with at least one frame first")
                    .clicked()
                {
                    self.open_export_dialog();
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

            ui.menu_button("Layer", |ui| {
                let can_merge_down = self.active_layer_can_merge_down();
                if ui
                    .add_enabled(can_merge_down, egui::Button::new(format!("{} Merge down", crate::icons::LAYERS)))
                    .on_disabled_hover_text("Select a raster layer with a raster layer directly below it")
                    .clicked()
                {
                    self.merge_down();
                    ui.close();
                }
                let can_merge_selected = self.can_merge_selected();
                if ui
                    .add_enabled(can_merge_selected, egui::Button::new(format!("{} Merge selected", crate::icons::LAYERS)))
                    .on_disabled_hover_text("Select at least two raster layers")
                    .clicked()
                {
                    self.merge_selected();
                    ui.close();
                }
                let can_flatten = self.can_flatten_visible();
                if ui
                    .add_enabled(can_flatten, egui::Button::new(format!("{} Flatten visible", crate::icons::LAYERS)))
                    .on_disabled_hover_text("Needs at least two visible raster layers")
                    .clicked()
                {
                    self.flatten_visible();
                    ui.close();
                }
            });

            ui.menu_button("Transform", |ui| {
                let has_sprite = self.doc.active_sprite().is_some();
                let has_selection = self.editor.selection.is_some();
                ui.add_enabled_ui(has_sprite, |ui| {
                    if ui.button(format!("{} Rotate arbitrary…", crate::icons::ROTATE_FREE)).clicked() {
                        self.open_transform_dialog(TransformDialog::Rotate {
                            degrees: 0.0,
                            algorithm: RotationAlgorithm::default(),
                        });
                        ui.close();
                    }
                    if ui.button(format!("{} Skew…", crate::icons::SKEW)).clicked() {
                        self.open_transform_dialog(TransformDialog::Skew { fx: 0.0, fy: 0.0 });
                        ui.close();
                    }
                    if ui.button(format!("{} Perspective…", crate::icons::PERSPECTIVE)).clicked() {
                        // open_transform_dialog re-seeds the corners to the
                        // active sprite's identity rect.
                        self.open_transform_dialog(TransformDialog::Perspective { corners: [(0.0, 0.0); 4] });
                        ui.close();
                    }
                    if ui.button(format!("{} Antialias (MLAA)…", crate::icons::ANTIALIAS)).clicked() {
                        self.open_transform_dialog(TransformDialog::Mlaa { config: MlaaConfig::default() });
                        ui.close();
                    }
                    ui.separator();
                    // Crop to selection shrinks the canvas to the selection
                    // bounds; it needs an active selection to act on.
                    let crop = egui::Button::new(format!("{} Crop to selection", crate::icons::CROP));
                    if ui.add_enabled(has_selection, crop).on_disabled_hover_text("Select a region first").clicked() {
                        self.crop_to_selection();
                        ui.close();
                    }
                });
            });

            ui.menu_button("Select", |ui| {
                let has_sprite = self.doc.active_sprite().is_some();
                let has_selection = self.editor.selection.is_some();
                ui.add_enabled_ui(has_sprite, |ui| {
                    if ui.button(format!("{} Select all", crate::icons::SELECT_ALL)).clicked() {
                        self.select_all();
                        ui.close();
                    }
                    if ui
                        .add_enabled(has_selection, egui::Button::new(format!("{} Deselect", crate::icons::SELECT_NONE)))
                        .clicked()
                    {
                        self.editor.clear_selection();
                        ui.close();
                    }
                    if ui.button(format!("{} Invert", crate::icons::SELECT_INVERT)).clicked() {
                        self.invert_selection();
                        ui.close();
                    }
                    ui.separator();
                    // Grow / Shrink / Feather edit the current mask. Each opens a
                    // small amount dialog; they need an active selection.
                    ui.add_enabled_ui(has_selection, |ui| {
                        if ui.button(format!("{} Grow…", crate::icons::SELECT_GROW)).clicked() {
                            self.open_morph_dialog(MorphOp::Grow);
                            ui.close();
                        }
                        if ui.button(format!("{} Shrink…", crate::icons::SELECT_SHRINK)).clicked() {
                            self.open_morph_dialog(MorphOp::Shrink);
                            ui.close();
                        }
                        if ui.button(format!("{} Feather…", crate::icons::SELECT_FEATHER)).clicked() {
                            self.open_morph_dialog(MorphOp::Feather);
                            ui.close();
                        }
                    });
                    ui.separator();
                    if ui.button(format!("{} Color range", crate::icons::COLOR_RANGE)).clicked() {
                        self.editor.left_tool = crate::editor::Tool::ColorRange;
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

                ui.separator();
                ui.checkbox(&mut self.editor.show_pixel_grid, "Pixel grid")
                    .on_hover_text("Show a per-pixel grid once the zoom is high enough to read it");
                ui.checkbox(&mut self.editor.major_grid_enabled, "Tile / major grid")
                    .on_hover_text("Show a cyan grid every N canvas pixels for tile-sized alignment");
                if self.editor.major_grid_enabled {
                    ui.horizontal(|ui| {
                        ui.label("spacing");
                        ui.add(egui::DragValue::new(&mut self.editor.major_grid_spacing_x).range(1..=4096).prefix("X "));
                        ui.add(egui::DragValue::new(&mut self.editor.major_grid_spacing_y).range(1..=4096).prefix("Y "));
                    });
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

            // A transient message (e.g. a rejected merge) shows at the right
            // edge, then clears once it ages past its time-to-live.
            if let Some((text, set_at)) = &self.status_message {
                if set_at.elapsed() < STATUS_MESSAGE_TTL {
                    let text = text.clone();
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(egui::RichText::new(text).small().color(palette.warning));
                    });
                } else {
                    self.status_message = None;
                }
            }
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
            CommandId::SelectAll => self.select_all(),
            CommandId::Deselect => {
                // Commit any in-flight free transform before dropping the
                // selection, so Escape ends the transform rather than abandoning
                // the lifted pixels mid-resample.
                self.commit_free_transform();
                self.editor.clear_selection();
            }
            CommandId::InvertSelection => self.invert_selection(),
            CommandId::DeleteSelection => self.delete_selection(),
            CommandId::ZoomIn => self.pending_zoom = Some(ZoomAction::In),
            CommandId::ZoomOut => self.pending_zoom = Some(ZoomAction::Out),
            CommandId::ZoomFit => self.needs_fit = true,
            CommandId::ZoomReset => self.pending_zoom = Some(ZoomAction::Reset),
            CommandId::ToggleGrid => self.editor.show_pixel_grid = !self.editor.show_pixel_grid,
            CommandId::ToggleMajorGrid => self.editor.major_grid_enabled = !self.editor.major_grid_enabled,
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
            CommandId::ToolColorRange => self.editor.left_tool = Tool::ColorRange,
            CommandId::ToolMove => self.editor.left_tool = Tool::Move,
            CommandId::ToolTransform => self.editor.left_tool = Tool::Transform,
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

/// The next playback cursor after `cursor` over a `len`-frame play order, or
/// `None` to stop. While inside the order it advances by one. At the last frame
/// it wraps to `0` when `looping`, else returns `None` so the caller halts on
/// the final frame. `len == 0` always stops.
fn next_cursor(cursor: usize, len: usize, looping: bool) -> Option<usize> {
    if len == 0 {
        return None;
    }
    let next = cursor + 1;
    if next < len {
        Some(next)
    } else if looping {
        Some(0)
    } else {
        None
    }
}

/// Whether `(w, h)` exactly matches one of the [`SIZE_PRESETS`] entries.
fn is_preset_size(w: u32, h: u32) -> bool {
    SIZE_PRESETS.iter().any(|(_, pw, ph)| *pw == w && *ph == h)
}

/// The outcome of a morphology pass over a selection mask.
#[derive(Clone, Debug, PartialEq, Eq)]
enum MorphResult {
    /// The core op overflowed (`Err(DimensionOverflow)`); leave the existing
    /// selection untouched.
    Unchanged,
    /// The op left nothing selected (a contract that ate the whole mask); clear
    /// the selection so the ants vanish and drawing is unclipped again, the same
    /// convention `combine_masks` uses for a subtract-to-empty.
    Cleared,
    /// The new, non-empty mask.
    Replaced(SelectionMask),
}

/// Runs `op` on `mask` at the given pixel `amount` and reports the new
/// selection state. Feather grades coverage rather than toggling it; the
/// resulting partial bytes are what the canvas clip blend honours to soften
/// painted edges.
fn morph_mask(mask: &SelectionMask, op: MorphOp, amount: u32) -> MorphResult {
    let out = match op {
        MorphOp::Grow => pixhaus_core::selection::expand(mask, amount),
        MorphOp::Shrink => pixhaus_core::selection::contract(mask, amount),
        MorphOp::Feather => pixhaus_core::selection::feather(mask, amount),
    };
    match out {
        Err(_) => MorphResult::Unchanged,
        Ok(out) if out.selected_count() > 0 => MorphResult::Replaced(out),
        Ok(_) => MorphResult::Cleared,
    }
}

/// The dropdown label for a color mode.
fn color_mode_label(mode: ColorMode) -> &'static str {
    match mode {
        ColorMode::Rgba => "RGBA",
        ColorMode::Grayscale => "Grayscale",
        ColorMode::Indexed => "Indexed",
    }
}

/// Turns a sprite name into a safe file stem for export: keeps ASCII
/// alphanumerics, dash, and underscore; collapses every other character to an
/// underscore. Falls back to `"sprite"` when nothing survives, so the worker
/// always has a usable default file name.
fn sanitize_base_name(name: &str) -> String {
    let cleaned: String = name
        .trim()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect();
    let trimmed = cleaned.trim_matches('_');
    if trimmed.is_empty() { "sprite".to_owned() } else { trimmed.to_owned() }
}

/// The dropdown label for a rotation algorithm.
fn rotation_algorithm_label(algorithm: RotationAlgorithm) -> &'static str {
    match algorithm {
        RotationAlgorithm::RotSprite => "RotSprite (pixel art)",
        RotationAlgorithm::Bilinear => "Bilinear (smooth)",
        RotationAlgorithm::NearestNeighbor => "Nearest (hard edges)",
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

/// Mirrors every frame left-to-right, in order, for east-as-flip-of-west at Land.
/// Each frame is the west loop frame mirrored across X, so the east loop plays the
/// same motion facing right. An empty buffer can't flip — `flip_horizontal`
/// returns an error there — so it is passed through unchanged rather than dropped,
/// keeping the frame count and order stable.
fn flip_frames_horizontal(frames: Vec<PixelBuffer>) -> Vec<PixelBuffer> {
    frames
        .into_iter()
        .map(|frame| match pixhaus_core::transforms::flip_horizontal(&frame) {
            Ok(flipped) => flipped,
            Err(_) => frame,
        })
        .collect()
}

/// Builds [`FinishOptions`](pixhaus_core::transforms::finisher::FinishOptions)
/// for a studio Land at grid `(w, h)`: `with_palette` snapping to the anchor's
/// swatches when `anchor_palette` is non-empty, else `for_canvas` (Extract) as
/// the fallback. Pure, so the over-quantising guard is testable without a full
/// [`ShellApp`].
fn landing_finish_options(w: u32, h: u32, anchor_palette: &[pixhaus_core::project::PaletteEntry]) -> pixhaus_core::transforms::finisher::FinishOptions {
    use pixhaus_core::project::palette::Palette;
    use pixhaus_core::transforms::finisher::FinishOptions;
    if anchor_palette.is_empty() {
        FinishOptions::for_canvas(w, h)
    } else {
        let colors = anchor_palette.iter().map(|e| e.color).collect();
        FinishOptions::with_palette((w, h), Palette::from_colors(pixhaus_core::project::PaletteId::new(0), "anchor", colors))
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

/// Repacks a streamed [`PixelData`] partial into tightly packed RGBA8 plus
/// dimensions for the reveal effect. Returns `None` for non-RGBA data or a
/// buffer too short for its declared stride and height.
fn rgba_from_pixel_data(pixels: &PixelData) -> Option<(Vec<u8>, u32, u32)> {
    if pixels.bytes_per_pixel != 4 {
        return None;
    }
    let (w, h, stride) = (pixels.width as usize, pixels.height as usize, pixels.stride as usize);
    if w == 0 || h == 0 || stride < w * 4 || pixels.bytes.len() < stride * h {
        return None;
    }
    let mut rgba = Vec::with_capacity(w * h * 4);
    for row in 0..h {
        let start = row * stride;
        rgba.extend_from_slice(&pixels.bytes[start..start + w * 4]);
    }
    Some((rgba, pixels.width, pixels.height))
}

/// Converts a decoded clip frame into a [`PixelBuffer`] for the canvas, reusing
/// the exact mechanism the reference-sheet preview uses.
pub(crate) fn video_frame_to_pixel_buffer(frame: &VideoFrame) -> Option<PixelBuffer> {
    PixelBuffer::from_raw(frame.width, frame.height, frame.width * 4, frame.pixels.clone()).ok()
}

/// Maps a file extension to the MIME `anim::decode_clip` understands. `None`
/// for an extension we never decode, so the caller can reject it before reading
/// the file. Case-insensitive.
fn mime_from_extension(ext: &str) -> Option<&'static str> {
    match ext.to_ascii_lowercase().as_str() {
        "gif" => Some("image/gif"),
        "png" | "apng" => Some("image/apng"),
        "mp4" | "m4v" => Some("video/mp4"),
        "mov" => Some("video/quicktime"),
        _ => None,
    }
}

/// Worker-thread half of the video import: open the picker, sniff the MIME from
/// the extension, read the bytes, and decode. Returns the raw clip, its MIME,
/// and the decoded frames, or a user-facing error. `Ok` with an empty error is
/// never produced; a cancelled dialog returns a benign "cancelled" message the
/// caller shows as a status line. Blocks — call only from `spawn_blocking`.
fn pick_and_decode_video(fps: u32) -> Result<(Vec<u8>, String, Vec<VideoFrame>), String> {
    let Some(path) = rfd::FileDialog::new()
        .set_title("Import a video clip")
        .add_filter("Video / animation", &["mp4", "m4v", "mov", "gif", "png", "apng"])
        .pick_file()
    else {
        return Err("Video import cancelled.".to_owned());
    };
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or_default();
    let Some(mime) = mime_from_extension(ext) else {
        return Err(format!("Unsupported file type \".{ext}\". Use mp4, mov, gif, or apng."));
    };
    let bytes = std::fs::read(&path).map_err(|e| format!("Could not read the file: {e}"))?;
    let frames = anim::decode_clip(&bytes, mime, fps.max(1)).map_err(|e| match e {
        anim::DecodeError::Unsupported(detail) => format!("Unsupported clip: {detail}. Use mp4/H.264, gif, or apng."),
        anim::DecodeError::Decode(detail) => format!("Could not decode the clip: {detail}"),
    })?;
    if frames.is_empty() {
        return Err("The clip decoded to zero frames.".to_owned());
    }
    Ok((bytes, mime.to_owned(), frames))
}

/// Maps `durations.len()` beats onto `span` frames, returning one duration per
/// frame.
///
/// Beat `b` covers the contiguous run of frames `[b * span / beats, (b + 1) *
/// span / beats)`, so the beats spread evenly across the range: more frames
/// than beats holds each beat across several frames; more beats than frames
/// drops the tail beats. Returns an empty vector when either side is empty.
#[must_use]
#[allow(clippy::cast_possible_truncation)]
pub(crate) fn map_beats_to_frames(durations: &[u32], span: u32) -> Vec<u32> {
    let beats = durations.len();
    if beats == 0 || span == 0 {
        return Vec::new();
    }
    (0..span)
        .map(|offset| {
            let beat = ((u64::from(offset) * beats as u64) / u64::from(span)) as usize;
            durations.get(beat).copied().unwrap_or_else(|| durations.last().copied().unwrap_or(100))
        })
        .collect()
}

/// Worker-thread half of "sync to audio": open the picker, read the WAV bytes,
/// detect beat onsets, and convert them to per-frame durations at `fps`. Returns
/// the durations or a user-facing error; a cancelled dialog returns a benign
/// "cancelled" message. The audio is read for timing only and never stored.
/// Blocks — call only from `spawn_blocking`.
fn pick_and_detect_audio(fps: u32) -> Result<Vec<u32>, String> {
    use pixhaus_core::transforms::audio_timing::{AudioError, detect_onsets_wav, onset_frame_durations};

    let Some(path) = rfd::FileDialog::new()
        .set_title("Sync timing to a WAV file")
        .add_filter("PCM WAV audio", &["wav"])
        .pick_file()
    else {
        return Err("Audio sync cancelled.".to_owned());
    };
    let bytes = std::fs::read(&path).map_err(|e| format!("Could not read the file: {e}"))?;
    // A sensitivity of 0.5 matches the old verb's default — a balanced middle
    // between catching every transient and only the loudest beats.
    let onsets = detect_onsets_wav(&bytes, 0.5).map_err(|e| match e {
        AudioError::Unsupported(detail) => detail,
        AudioError::Malformed(detail) => format!("That WAV file is malformed ({detail})."),
    })?;
    let durations = onset_frame_durations(&onsets, fps as f32);
    if durations.is_empty() {
        return Err("No beats detected; try a clearer track.".to_owned());
    }
    Ok(durations)
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
        eframe::set_value(storage, "reveal_effect_enabled", &self.reveal_effect_enabled);
        eframe::set_value(storage, "keymap", &self.keymap);
        eframe::set_value(storage, "new_sprite_size", &(self.new_sprite_w, self.new_sprite_h));
        eframe::set_value(storage, "grid_prefs", &GridPrefs::from_editor(&self.editor));
        // Capture the active sprite's in-progress session, then write the
        // per-sprite sessions to their own JSON sidecar under the storage dir
        // (the approved-pose PNG is too heavy for eframe's RON value store).
        self.flush_active_studio_session();
        crate::studio::save_studio_sessions(&self.studio_sessions);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.handle_shortcuts(ui.ctx());
        self.show_settings_window(ui.ctx());
        self.show_new_sprite_dialog(ui.ctx());
        self.show_delete_confirm(ui.ctx());
        self.show_delete_layers_confirm(ui.ctx());
        self.show_resize_dialog(ui.ctx());
        self.show_morph_dialog(ui.ctx());
        self.show_transform_dialog(ui.ctx());
        self.show_pack_import_dialog(ui.ctx());
        self.show_export_dialog(ui.ctx());

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
    use super::{
        GridPrefs, MorphOp, MorphResult, SelectionMask, map_beats_to_frames, mime_from_extension, morph_mask, next_cursor, png_to_pixel_buffer, truncate_motion,
    };

    // --- playback cursor: loop vs stop --------------------------------------

    #[test]
    fn next_cursor_advances_inside_the_play_order() {
        // Mid-order, the cursor steps forward by one regardless of loop mode.
        assert_eq!(next_cursor(0, 4, true), Some(1));
        assert_eq!(next_cursor(2, 4, false), Some(3));
    }

    #[test]
    fn next_cursor_wraps_at_the_end_when_looping() {
        // Last frame with loop on cycles back to the start.
        assert_eq!(next_cursor(3, 4, true), Some(0));
    }

    #[test]
    fn next_cursor_stops_at_the_end_when_not_looping() {
        // Last frame with loop off returns None so playback halts on the final
        // frame instead of wrapping.
        assert_eq!(next_cursor(3, 4, false), None);
    }

    #[test]
    fn next_cursor_stops_on_an_empty_order() {
        // A zero-length order never advances, in either mode.
        assert_eq!(next_cursor(0, 0, true), None);
        assert_eq!(next_cursor(0, 0, false), None);
    }

    // --- video import: extension -> mime ------------------------------------

    #[test]
    fn mime_from_extension_maps_supported_video_types() {
        // The decode-supported containers map to the MIME `decode_clip` matches.
        assert_eq!(mime_from_extension("mp4"), Some("video/mp4"));
        assert_eq!(mime_from_extension("m4v"), Some("video/mp4"));
        assert_eq!(mime_from_extension("mov"), Some("video/quicktime"));
        assert_eq!(mime_from_extension("gif"), Some("image/gif"));
        assert_eq!(mime_from_extension("png"), Some("image/apng"));
        assert_eq!(mime_from_extension("apng"), Some("image/apng"));
    }

    #[test]
    fn mime_from_extension_is_case_insensitive() {
        assert_eq!(mime_from_extension("MP4"), Some("video/mp4"));
        assert_eq!(mime_from_extension("Gif"), Some("image/gif"));
    }

    #[test]
    fn mime_from_extension_rejects_unsupported_types() {
        // webm/avi/mkv have no native decode path, so the picker rejects them
        // before reading the file rather than failing deep in the decoder.
        assert_eq!(mime_from_extension("webm"), None);
        assert_eq!(mime_from_extension("avi"), None);
        assert_eq!(mime_from_extension("mkv"), None);
        assert_eq!(mime_from_extension(""), None);
    }

    // --- selection morphology: grow / shrink / feather ----------------------

    #[test]
    fn shrink_full_5x5_to_interior_3x3() {
        // A full 5x5 mask contracted by 1 erodes its border to the 3x3 centre,
        // mirroring the core `contract_erodes_edges` fixture.
        let mask = SelectionMask::full(5, 5).expect("full mask");
        let MorphResult::Replaced(out) = morph_mask(&mask, MorphOp::Shrink, 1) else {
            panic!("shrink of a full mask is non-empty");
        };
        assert_eq!(out.selected_count(), 9, "5x5 shrinks to a 3x3 interior");
        for y in 0..5 {
            for x in 0..5 {
                let interior = (1..=3).contains(&x) && (1..=3).contains(&y);
                assert_eq!(out.is_selected(x, y), interior, "({x}, {y}) selected == interior");
            }
        }
    }

    #[test]
    fn grow_single_pixel_to_a_plus() {
        // A single selected pixel grown by 1 dilates to a plus (centre + four
        // cardinal neighbours), mirroring the core `expand_grows_single_pixel`.
        let mut mask = SelectionMask::new(8, 8).expect("mask");
        mask.set(4, 4, 255);
        let MorphResult::Replaced(out) = morph_mask(&mask, MorphOp::Grow, 1) else {
            panic!("grow of a single pixel is non-empty");
        };
        let plus = [(4, 4), (3, 4), (5, 4), (4, 3), (4, 5)];
        assert_eq!(out.selected_count() as usize, plus.len(), "grow yields a five-pixel plus");
        for (x, y) in plus {
            assert!(out.is_selected(x, y), "plus arm ({x}, {y}) is selected");
        }
        // Diagonals stay out — the structuring element is a disc, not a square.
        assert!(!out.is_selected(3, 3), "diagonal (3, 3) is not selected");
    }

    #[test]
    fn feather_grades_a_boundary_to_intermediate_coverage() {
        // Feathering a left-half row leaves the boundary pixel at partial
        // coverage (neither 0 nor 255) — the soft edge the clip blend honours.
        let mut mask = SelectionMask::new(10, 1).expect("mask");
        for x in 0..5u32 {
            mask.set(x, 0, 255);
        }
        let MorphResult::Replaced(out) = morph_mask(&mask, MorphOp::Feather, 2) else {
            panic!("feather of a half mask is non-empty");
        };
        let edge = out.get(4, 0).expect("edge coverage");
        assert!(edge > 0 && edge < 255, "feathered boundary has intermediate coverage: {edge}");
    }

    #[test]
    fn shrink_that_empties_the_mask_clears_the_selection() {
        // A single-row selection contracted by 1 erodes to nothing (every pixel
        // borders the out-of-canvas edge). morph_mask reports Cleared so the
        // caller drops the selection entirely.
        let mut mask = SelectionMask::new(6, 1).expect("mask");
        for x in 0..6u32 {
            mask.set(x, 0, 255);
        }
        assert_eq!(
            morph_mask(&mask, MorphOp::Shrink, 1),
            MorphResult::Cleared,
            "an emptied contract clears the selection"
        );
    }

    #[test]
    fn feather_radius_zero_is_a_no_op() {
        // Feather at radius 0 (the off default) returns the mask unchanged.
        let mut mask = SelectionMask::new(4, 4).expect("mask");
        mask.set(1, 1, 255);
        let MorphResult::Replaced(out) = morph_mask(&mask, MorphOp::Feather, 0) else {
            panic!("feather of a non-empty mask is non-empty");
        };
        assert_eq!(out, mask, "feather radius 0 leaves the mask untouched");
    }

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

    // --- grid preference persistence ----------------------------------------

    #[test]
    fn grid_prefs_default_mirrors_the_editor_default() {
        // A fresh install and a never-saved profile must agree: GridPrefs's
        // Default reads from EditorState's own defaults rather than hardcoding.
        let editor = crate::editor::EditorState::default();
        assert_eq!(GridPrefs::default(), GridPrefs::from_editor(&editor));
    }

    #[test]
    fn grid_prefs_round_trips_through_editor() {
        // apply_to then from_editor is the load/save pair; it must be lossless
        // for in-range values. Seed a default editor with a non-default prefs
        // value, then read it straight back.
        let prefs = GridPrefs {
            show_pixel_grid: false,
            major_grid_enabled: true,
            spacing_x: 16,
            spacing_y: 24,
        };
        let mut editor = crate::editor::EditorState::default();
        prefs.apply_to(&mut editor);

        assert!(!editor.show_pixel_grid);
        assert!(editor.major_grid_enabled);
        assert_eq!(editor.major_grid_spacing_x, 16);
        assert_eq!(editor.major_grid_spacing_y, 24);
        // The snapshot off the seeded editor equals the prefs we wrote in.
        assert_eq!(GridPrefs::from_editor(&editor), prefs);
    }

    #[test]
    fn grid_prefs_apply_clamps_zero_spacing_up() {
        // A corrupt stored zero would stride by nothing in the major-grid math,
        // so apply_to lifts it to 1.
        let prefs = GridPrefs {
            show_pixel_grid: true,
            major_grid_enabled: true,
            spacing_x: 0,
            spacing_y: 0,
        };
        let mut editor = crate::editor::EditorState::default();
        prefs.apply_to(&mut editor);
        assert_eq!(editor.major_grid_spacing_x, 1);
        assert_eq!(editor.major_grid_spacing_y, 1);
    }

    #[test]
    fn grid_prefs_serde_round_trips() {
        // The struct persists through eframe Storage, which is JSON; assert the
        // serde shape survives a round trip with the documented field names.
        let prefs = GridPrefs {
            show_pixel_grid: false,
            major_grid_enabled: true,
            spacing_x: 32,
            spacing_y: 8,
        };
        let json = serde_json::to_string(&prefs).expect("serialize grid prefs");
        let back: GridPrefs = serde_json::from_str(&json).expect("deserialize grid prefs");
        assert_eq!(back, prefs);
    }

    // --- C3: east-as-flip-of-west at Land -----------------------------------

    use super::{PixelBuffer, flip_frames_horizontal};
    use pixhaus_core::project::Rgba;

    /// A left-right-asymmetric test frame: a colour ramp across X so a mirror is
    /// visibly distinct from the original (a symmetric pattern would flip onto
    /// itself and prove nothing).
    fn ramp_frame(w: u32, h: u32, tag: u8) -> PixelBuffer {
        let mut buf = PixelBuffer::new(w, h).expect("buffer");
        for y in 0..h {
            for x in 0..w {
                let r = u8::try_from(x * 255 / w.max(1)).unwrap_or(255);
                buf.set_pixel(x, y, Rgba::opaque(r, tag, u8::try_from(y % 256).unwrap_or(0)));
            }
        }
        buf
    }

    /// Mirrors one buffer across X by hand — the reference the flip must match.
    fn mirror_x(buf: &PixelBuffer) -> PixelBuffer {
        let (w, h) = (buf.width(), buf.height());
        let mut out = PixelBuffer::new(w, h).expect("buffer");
        for y in 0..h {
            for x in 0..w {
                let px = buf.pixel(w - 1 - x, y).expect("pixel");
                out.set_pixel(x, y, px);
            }
        }
        out
    }

    fn to_rgba_image(b: &PixelBuffer) -> image::RgbaImage {
        let mut bytes = Vec::with_capacity((b.width() as usize) * (b.height() as usize) * 4);
        for y in 0..b.height() {
            bytes.extend_from_slice(b.row(y).expect("row"));
        }
        image::RgbaImage::from_raw(b.width(), b.height(), bytes).expect("rgba image")
    }

    #[test]
    fn east_land_flips_each_west_frame_across_x() {
        // The west loop the Normalize stage reviewed.
        let west = vec![ramp_frame(12, 8, 1), ramp_frame(12, 8, 2), ramp_frame(12, 8, 3)];
        // East is the flip of west: Land mirrors the reviewed frames before
        // integrate, preserving frame count and order.
        let east = flip_frames_horizontal(west.clone());

        assert_eq!(east.len(), west.len(), "the flip keeps every frame, in order");
        for (i, (east_frame, west_frame)) in east.iter().zip(&west).enumerate() {
            let expected = mirror_x(west_frame);
            // Byte-for-byte equality, stronger than an image-compare score.
            assert_eq!(east_frame.as_bytes(), expected.as_bytes(), "east frame {i} is the west frame mirrored across X");
            // And confirm via image-compare for the visual-regression record.
            let score = image_compare::rgba_hybrid_compare(&to_rgba_image(east_frame), &to_rgba_image(&expected))
                .expect("compare")
                .score;
            assert!((score - 1.0).abs() < f64::EPSILON, "east frame {i} matches the mirror exactly: {score}");
        }
        // The mirror genuinely changes an asymmetric frame — a no-op flip would
        // pass the equality above against itself and prove nothing.
        assert_ne!(east[0].as_bytes(), west[0].as_bytes(), "the mirror is not the identity for an asymmetric frame");
    }

    #[test]
    fn east_land_passes_an_empty_frame_through_unchanged() {
        // A zero-sized buffer can't flip; it must pass through, not vanish, so the
        // frame count and order stay stable.
        let frames = vec![PixelBuffer::new(0, 0).expect("empty buffer")];
        let flipped = flip_frames_horizontal(frames);
        assert_eq!(flipped.len(), 1, "the empty frame survives the flip pass");
        assert!(flipped[0].is_empty(), "the empty frame is unchanged");
    }

    // --- Brief 8: anchor-palette over-quantising guard on Land --------------

    use super::landing_finish_options;
    use pixhaus_core::project::PaletteEntry;
    use pixhaus_core::transforms::finisher::{PaletteSource, finish_frames};

    #[test]
    fn landing_finish_options_prefers_the_anchor_palette_when_present() {
        // No anchor palette -> Extract (for_canvas); a present anchor palette ->
        // Fixed, so a derived loop stays on the established anchor colours rather
        // than re-extracting a fresh per-frame palette (over-quantising guard).
        let none = landing_finish_options(16, 16, &[]);
        assert!(matches!(none.palette, PaletteSource::Extract { .. }), "empty anchor palette falls back to Extract");
        let anchor = [PaletteEntry::new(Rgba::opaque(0, 0, 0)), PaletteEntry::new(Rgba::opaque(255, 255, 255))];
        let fixed = landing_finish_options(16, 16, &anchor);
        match fixed.palette {
            PaletteSource::Fixed(p) => assert_eq!(p.colors.len(), 2, "the anchor swatches drive the fixed palette"),
            other => panic!("expected a fixed anchor palette, got {other:?}"),
        }
    }

    #[test]
    fn landing_with_an_anchor_palette_snaps_frames_to_the_anchor_colours() {
        // A derivation with a present anchor palette snaps every opaque pixel to
        // the anchor's two colours, not a freshly extracted set — so a landed
        // loop does not drift off the established anchor palette.
        let anchor = [PaletteEntry::new(Rgba::opaque(0, 0, 0)), PaletteEntry::new(Rgba::opaque(255, 255, 255))];
        let opts = landing_finish_options(8, 8, &anchor);
        // A colour ramp gives the snap a spread to reduce; a flat fill would prove
        // nothing about drift.
        let finished = finish_frames(&[ramp_frame(8, 8, 4)], &opts).expect("finish");
        assert_eq!(finished.len(), 1);
        let palette: Vec<[u8; 3]> = finished[0].palette.colors.iter().map(|e| [e.color.r, e.color.g, e.color.b]).collect();
        assert_eq!(palette, vec![[0, 0, 0], [255, 255, 255]], "the snap palette is exactly the anchor swatches");
        let allowed: std::collections::HashSet<[u8; 3]> = [[0, 0, 0], [255, 255, 255]].into_iter().collect();
        let img = to_rgba_image(&finished[0].buffer);
        for px in img.pixels() {
            if px.0[3] == 0 {
                continue;
            }
            assert!(allowed.contains(&[px.0[0], px.0[1], px.0[2]]), "snapped pixel {:?} must be an anchor colour", px.0);
        }
    }

    // --- sync to audio: beat → frame mapping --------------------------------

    #[test]
    fn map_beats_fewer_beats_holds_across_frames() {
        // Two beats over four frames: each beat covers two frames.
        let mapped = map_beats_to_frames(&[100, 200], 4);
        assert_eq!(mapped, vec![100, 100, 200, 200]);
    }

    #[test]
    fn map_beats_one_per_frame_when_counts_match() {
        let mapped = map_beats_to_frames(&[10, 20, 30], 3);
        assert_eq!(mapped, vec![10, 20, 30]);
    }

    #[test]
    fn map_beats_more_beats_than_frames_keeps_leading_beats() {
        // Four beats over two frames: only the beats the frames index into are
        // used; the tail beats drop.
        let mapped = map_beats_to_frames(&[10, 20, 30, 40], 2);
        assert_eq!(mapped, vec![10, 30]);
    }

    #[test]
    fn map_beats_empty_inputs_yield_empty() {
        assert!(map_beats_to_frames(&[], 4).is_empty());
        assert!(map_beats_to_frames(&[100], 0).is_empty());
    }
}
