//! Application-level state threaded through Tauri commands.
//!
//! [`AppState`] is registered with `.manage()` during startup and received by
//! every command that needs to read or modify the active document.

use std::path::PathBuf;

use pixhaus_core::project::Project;

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
}

impl Default for DocumentStore {
    fn default() -> Self {
        Self {
            project: None,
            path: None,
            next_id: 1,
            dirty: false,
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
