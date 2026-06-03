//! The live editing/services bundle the [`Host`](crate::state::Host) owns.
//!
//! `EditSession` holds the `core::Document` and the `services`-owned containers that
//! act on it — the undo [`History`], the [`JobManager`], the [`ProviderRegistry`],
//! and the shared [`ResultStore`]. It is the concrete home for the bible's
//! services-owned containers (section 22.7). It is mutated ONLY in `apply_intent`,
//! `drain_background`, and `canvas_stage` — never borrowed into a `&self` panel, so
//! the deferred-intent invariant (panels read `SessionState`, push `Intent`s) holds.

use std::sync::Arc;
use std::sync::mpsc::{self, Receiver};

use pixhaus_core::{Document, FrameId};
use pixhaus_services::{History, JobManager, JobMsg, ProviderRegistry, ResultStore};

/// The live document plus the services that operate on it.
pub struct EditSession {
    /// The authoritative project state and the single command target.
    pub document: Document,
    /// The command executor and undo/redo stack.
    pub history: History,
    /// Completed generation results, shared with the job tasks that write them.
    pub results: Arc<ResultStore>,
    /// Submits and cancels generation jobs.
    pub jobs: JobManager,
    /// The capability-based provider registry; populated at boot.
    pub providers: ProviderRegistry,
    /// Receives [`JobMsg`] notifications from running jobs; drained each frame.
    pub job_rx: Receiver<JobMsg>,
    /// The document revision last composited and uploaded to the GPU; the
    /// recomposition dirty gate. Starts at the empty document's revision (0), so the
    /// first real edit forces a composite.
    pub last_uploaded_revision: u64,
    /// The frame last composited for display, paired with `last_uploaded_revision`
    /// as the dirty gate: playback advances the displayed frame without bumping the
    /// revision, so the canvas must also re-upload when this changes. `None` until
    /// the first upload.
    pub last_uploaded_frame: Option<FrameId>,
}

impl Default for EditSession {
    fn default() -> Self {
        let (job_tx, job_rx) = mpsc::channel();
        Self {
            document: Document::new(),
            history: History::new(),
            results: Arc::new(ResultStore::new()),
            jobs: JobManager::new(job_tx),
            providers: ProviderRegistry::new(),
            job_rx,
            last_uploaded_revision: 0,
            last_uploaded_frame: None,
        }
    }
}
