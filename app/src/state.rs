//! Application-level state threaded through Tauri commands.
//!
//! [`AppState`] is registered with `.manage()` during startup and received by
//! every command that needs to read or modify the active document.

use std::path::PathBuf;

#[cfg(doc)]
use pixhaus_core::project::PixelBufferId;
use pixhaus_core::project::Project;
use pixhaus_core::undo::History;
use pixhaus_io::pixhaus::PixelBufferEntry;

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
}

impl AppState {
    /// Constructs the initial state with no open project.
    pub fn new() -> Self {
        Self {
            doc: tokio::sync::RwLock::new(DocumentStore::default()),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
