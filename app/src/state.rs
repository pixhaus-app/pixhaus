//! Application-level state threaded through Tauri commands.
//!
//! [`AppState`] is registered with `.manage()` during startup and received by
//! every command that needs to read or modify the active document.

use std::path::PathBuf;
use std::sync::Arc;

use pixhaus_ai::plugin::runtime::VerbRuntime;
use pixhaus_ai::verbs::CritiqueVerb;
#[cfg(doc)]
use pixhaus_core::project::PixelBufferId;
use pixhaus_core::project::Project;
use pixhaus_core::undo::History;
use pixhaus_io::pixhaus::PixelBufferEntry;

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
pub struct AppState {
    /// Document store guarded by a tokio `RwLock`. Lock, work, release
    /// — never hold across an unrelated async suspension.
    pub(crate) doc: tokio::sync::RwLock<DocumentStore>,
    /// Verb runtime with built-in verbs registered. Shared across all
    /// IPC command handlers via `Arc` so commands can invoke verbs
    /// concurrently without holding the doc lock.
    pub(crate) verb_runtime: Arc<VerbRuntime>,
}

impl AppState {
    /// Constructs the initial state with no open project.
    ///
    /// Registers all built-in verbs with the verb runtime. Backend
    /// registration happens separately through `verb_runtime` after the
    /// user configures API keys in the settings panel.
    pub fn new() -> Self {
        let runtime = VerbRuntime::new();
        // Registration fails only on a duplicate ID, which cannot happen
        // with the hardcoded verb set below. Discard the Ok(()) result.
        let _ = runtime.register(CritiqueVerb::new());

        Self {
            doc: tokio::sync::RwLock::new(DocumentStore::default()),
            verb_runtime: Arc::new(runtime),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
