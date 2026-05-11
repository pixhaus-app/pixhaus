//! Application-level state threaded through Tauri commands.
//!
//! [`AppState`] is registered with `.manage()` during startup and received by
//! every command that needs to read or modify the active document, the verb
//! runtime, or the plugin registry.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use dashmap::DashMap;
use pixhaus_ai::plugin::AnchorPayload;
use pixhaus_ai::plugin::runtime::VerbRuntime;
use pixhaus_ai::verbs::{
    AudioTimingVerb, AutoMeshDeformationVerb, CleanupVerb, ContinueVerb, ConversationalVerb,
    CritiqueVerb, ExtendVerb, InbetweenVerb, MotionFromVideoVerb, ProjectStyleLearningVerb,
    SketchFinishingVerb, TileVerb, TilesetFromDescriptionVerb, VariantVerb,
};
use pixhaus_core::project::{LayerId, PixelBufferId, Project, Rgba, SpriteId};
use pixhaus_core::undo::History;
use pixhaus_io::pixhaus::PixelBufferEntry;
use tokio_util::sync::CancellationToken;

use pixhaus_plugins::PluginRegistry;

use crate::pixel_history::PixelHistory;

/// In-memory document and editor session. Internal to the app crate.
pub(crate) struct DocumentStore {
    /// Active project, or `None` when no document is open.
    pub(crate) project: Option<Project>,
    /// Path from which the project was last saved or opened.
    pub(crate) path: Option<PathBuf>,
    /// Monotonically increasing counter used to mint entity IDs.
    /// Starts at 1; 0 is reserved for "null / no entity".
    pub(crate) next_id: u32,
    /// `true` when the in-memory state differs from the last save.
    pub(crate) dirty: bool,
    /// Undo/redo history for the active document.
    pub(crate) history: History,
    /// Pixel buffers loaded from or to be written to the `.pixhaus` archive.
    ///
    /// Cels reference buffers by [`PixelBufferId`]; the bytes live here.
    /// Retained across the decode round-trip so `project_save` can write
    /// them back without losing content.
    pub(crate) pixel_buffers: Vec<PixelBufferEntry>,
    /// Undo/redo stack for pixel drawing ops.
    ///
    /// Pixel ops mutate `pixel_buffers` directly rather than going through
    /// `History` (which only sees `&mut Project`). This parallel stack
    /// stores before/after snapshots so undo/redo can restore them.
    pub(crate) pixel_history: PixelHistory,
    /// In-flight freehand strokes keyed by session id.
    ///
    /// Populated by `canvas_begin_stroke`, mutated by `canvas_extend_stroke`,
    /// and drained by `canvas_end_stroke`. Sessions live entirely in memory
    /// — they are not persisted in `.pixhaus` and don't survive a project
    /// close.
    pub(crate) active_strokes: HashMap<u32, StrokeSession>,
    /// Counter for stroke session ids. Separate from `next_id` because
    /// sessions are ephemeral — they live for the duration of one drag
    /// and never appear in the project file. Sharing `next_id` would
    /// inflate it on every stroke and waste id space for layers,
    /// sprites, palettes, and pixel buffers that DO survive.
    pub(crate) next_session_id: u32,
}

/// One in-flight stroke. Lives in `DocumentStore::active_strokes` between
/// the begin and end IPC calls, so extends can re-rasterize from the
/// pre-stroke pixels and the final commit records exactly one undo entry
/// for the whole drag.
///
/// `Clone` is implemented because the rasterize path needs an owned
/// snapshot to read after the `DocumentStore` write-lock has been
/// released. `initial_pixels` is wrapped in `Arc` so each clone is
/// O(1) regardless of canvas size — at RAF cadence on large canvases
/// the cost of a per-extend `Vec::clone` of `width * height * 4` bytes
/// would be prohibitive.
#[derive(Clone)]
pub(crate) struct StrokeSession {
    pub(crate) sprite_id: SpriteId,
    /// Captured so extend / end can re-verify the lock and so the
    /// rasterize path can re-locate the cel buffer if needed.
    pub(crate) layer_id: LayerId,
    pub(crate) frame_index: u32,
    pub(crate) buffer_id: PixelBufferId,
    /// Buffer dimensions captured at begin so the rasterize path can
    /// run without re-reading the buffer entry under the doc lock.
    pub(crate) buf_width: u32,
    pub(crate) buf_height: u32,
    pub(crate) buf_stride: u32,
    /// Buffer pixels at the moment the stroke began. Re-rasterized from
    /// on every extend so partial strokes never accumulate. Wrapped in
    /// `Arc` so cloning the session is constant-time.
    pub(crate) initial_pixels: Arc<Vec<u8>>,
    /// All points received via begin + extend so far, in order.
    pub(crate) points: Vec<[f32; 2]>,
    pub(crate) color: Rgba,
    pub(crate) brush_shape: String,
    pub(crate) brush_size: u32,
    pub(crate) pixel_perfect: bool,
    pub(crate) erase: bool,
    /// Undo label for the eventual `PixelOpBatch` ("stroke", "eraser",
    /// etc.). Captured at begin so a tool change mid-stroke (which the
    /// frontend won't issue but defense-in-depth) can't relabel an
    /// in-flight session.
    pub(crate) label: String,
}

impl Default for DocumentStore {
    fn default() -> Self {
        Self {
            project: None,
            path: None,
            next_id: 1,
            dirty: false,
            history: History::new(),
            pixel_buffers: Vec::new(),
            pixel_history: PixelHistory::new(),
            active_strokes: HashMap::new(),
            next_session_id: 1,
        }
    }
}

/// Shared application state registered with `tauri::Builder::manage`.
///
/// Commands receive `tauri::State<'_, AppState>` and lock `doc` to
/// read or mutate the document. `RwLock` lets read-only commands
/// (`project_get`, `sprite_list`, `frame_list`, `layer_list`,
/// `palette_list`) take a shared read guard so they don't block each
/// other; mutating commands take the write guard.
///
/// `verbs` and `plugins` are separate top-level fields (not inside `doc`)
/// because they are not document state — they persist across project opens
/// and closes.
pub struct AppState {
    /// Document store guarded by a tokio `RwLock`. Lock, work, release
    /// — never hold across an unrelated async suspension.
    pub(crate) doc: tokio::sync::RwLock<DocumentStore>,
    /// Verb runtime with built-in verbs registered. Shared with the plugin
    /// registry so plugins register their verbs into the same runtime.
    /// Wrapped in `Arc` so commands can invoke verbs concurrently without
    /// holding the doc lock.
    pub(crate) verb_runtime: Arc<VerbRuntime>,
    /// In-flight verb invocations keyed by [`pixhaus_ai::plugin::preview::PreviewId`]
    /// (stored as `u64`). `verb_invoke` registers the cancel token before
    /// awaiting the verb body and removes it on completion; `verb_cancel`
    /// looks up by id and fires the token. `DashMap` avoids contention on
    /// the doc lock so concurrent invocations cancel independently.
    pub(crate) invocations: Arc<DashMap<u64, CancellationToken>>,
    /// Plugin registry. Populated on startup by scanning
    /// `~/.pixhaus/plugins` and again on hot-reload events.
    pub(crate) plugins: Arc<PluginRegistry>,
    /// Anchor-payload cache keyed by `EntityId` of the Reference
    /// entity (B10.3).
    ///
    /// Each entry holds the latest [`AnchorPayload`] built from the
    /// canonical sheet. On verb dispatch the host checks
    /// `payload.canonical_hash` against the current canonical bytes; a
    /// stale entry triggers a rebuild. The approval and set-anchor
    /// commands invalidate eagerly by removing the entry. Cleared on
    /// `project_close` / `project_new` / `project_open` so a stale
    /// payload from a previous project never bleeds into a fresh one.
    pub(crate) anchor_cache: Arc<DashMap<u32, AnchorPayload>>,
}

impl AppState {
    /// Constructs the initial state with no open project and no loaded plugins.
    ///
    /// Registers all built-in verbs with the verb runtime, then hands an
    /// `Arc` of that runtime to the plugin registry so plugin-registered
    /// verbs land in the same runtime as the built-ins. Backend
    /// registration happens separately after the user configures API keys
    /// in the settings panel; the registry's `scan()` is called in the
    /// Tauri setup closure to load user-installed plugins asynchronously.
    pub fn new() -> Self {
        let runtime = VerbRuntime::new();

        // Duplicate IDs would be a programmer bug — every verb in this
        // list is distinct by construction. Failures are logged at error
        // level rather than discarded so a future regression (someone
        // double-registers, a verb's id constant changes to overlap)
        // surfaces in logs and the crash-report sink instead of silently
        // dropping a verb from the runtime. `panic!` would be tighter
        // but the workspace bans it outside tests.
        //
        // Verbs no longer hold their own backend reference: the runtime
        // selects a capability-matching backend per invocation and
        // injects it into `VerbContext::backend`. Configuring backends
        // happens via `runtime.register_backend(...)` from the (planned)
        // settings flow.
        register_builtin(&runtime, AudioTimingVerb::new());
        register_builtin(&runtime, AutoMeshDeformationVerb::new());
        register_builtin(&runtime, CleanupVerb::new());
        register_builtin(&runtime, ContinueVerb::new());
        register_builtin(&runtime, ConversationalVerb::new());
        register_builtin(&runtime, CritiqueVerb::new());
        register_builtin(&runtime, ExtendVerb::new());
        register_builtin(&runtime, InbetweenVerb::new());
        register_builtin(&runtime, MotionFromVideoVerb::new());
        register_builtin(&runtime, ProjectStyleLearningVerb::new());
        register_builtin(&runtime, SketchFinishingVerb::new());
        register_builtin(&runtime, TileVerb::new());
        register_builtin(&runtime, TilesetFromDescriptionVerb::new());
        register_builtin(&runtime, VariantVerb::new());

        let verb_runtime = Arc::new(runtime);
        let plugins = Arc::new(PluginRegistry::new(verb_runtime.clone()));

        Self {
            doc: tokio::sync::RwLock::new(DocumentStore::default()),
            verb_runtime,
            invocations: Arc::new(DashMap::new()),
            plugins,
            anchor_cache: Arc::new(DashMap::new()),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

/// Registers a built-in verb with `runtime` and logs at error level if
/// registration fails. Failure means a duplicate id, which is a
/// programmer error rather than a runtime condition; logging keeps the
/// app booting (without that verb) and surfaces the regression in the
/// crash-report sink.
fn register_builtin<V: pixhaus_ai::plugin::verb::Verb>(runtime: &VerbRuntime, verb: V) {
    let id = verb.descriptor().id.as_str().to_owned();
    if let Err(err) = runtime.register(verb) {
        tracing::error!(verb_id = %id, error = %err, "failed to register built-in verb");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_registers_every_built_in_verb() {
        let state = AppState::new();
        let descriptors = state.verb_runtime.list();
        let mut ids: Vec<&str> = descriptors.iter().map(|d| d.id.as_str()).collect();
        ids.sort_unstable();

        // Verb IDs as advertised by their descriptors. Note the namespace
        // drift: most verbs use `pixhaus.builtin.*` but `sketch_finishing`
        // uses `pixhaus.ai.*`. Tracked as a follow-up — left as-is here so
        // the test reflects the real surface.
        let mut expected = [
            "pixhaus.ai.sketch_finishing",
            "pixhaus.builtin.audio_timing",
            "pixhaus.builtin.auto-mesh-deformation",
            "pixhaus.builtin.cleanup",
            "pixhaus.builtin.continue",
            "pixhaus.builtin.conversational",
            "pixhaus.builtin.critique",
            "pixhaus.builtin.extend",
            "pixhaus.builtin.inbetween",
            "pixhaus.builtin.motion_from_video",
            "pixhaus.builtin.project_style_learning",
            "pixhaus.builtin.tile",
            "pixhaus.builtin.tileset_from_description",
            "pixhaus.builtin.variant",
        ];
        expected.sort_unstable();

        assert_eq!(ids, expected);
    }
}
