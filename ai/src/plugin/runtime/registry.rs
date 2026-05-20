//! The host-side verb runtime: registries, backend selection, dispatch.
//!
//! [`VerbRuntime`] owns the verb registry, the priority-ordered backend
//! registry, and the preview ID minter. [`VerbRuntime::invoke`] resolves
//! a backend (see the backend-selection rules on
//! [`VerbRuntime::select_backend`]), spawns the verb worker, and returns
//! a [`VerbInvocation`] the caller drives to completion.

use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use std::time::Instant;

use parking_lot::RwLock;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{debug, instrument, trace};

use crate::plugin::backend::{BackendInfo, InferenceBackend};
use crate::plugin::context::VerbContext;
use crate::plugin::descriptor::{BackendCapabilities, VerbDescriptor, VerbId};
use crate::plugin::error::{Result, VerbError};
use crate::plugin::inputs::VerbInputs;
use crate::plugin::output::VerbOutput;
use crate::plugin::preview::{
    PreviewIdMinter, VerbCommit, VerbDiscard, VerbPreview, now_unix_seconds,
};
use crate::plugin::progress::VerbProgress;
use crate::plugin::verb::Verb;

use super::invocation::VerbInvocation;

/// One entry in the backend priority list.
struct BackendEntry {
    backend: Arc<dyn InferenceBackend>,
    /// Lower value = tried first during capability matching.
    priority: u16,
}

/// Coordinator for verb registration, backend selection, and dispatch.
///
/// One `VerbRuntime` lives per editor session. Construct via
/// [`VerbRuntime::new`], wrap in an `Arc`, and share across the
/// command handlers.
pub struct VerbRuntime {
    verbs: RwLock<HashMap<VerbId, Arc<dyn Verb>>>,
    /// Priority-ordered list of registered backends. The list is kept
    /// sorted by `priority` ascending whenever an entry is inserted so
    /// [`VerbRuntime::select_backend`] can short-circuit on first match
    /// without sorting on every call.
    backends: RwLock<Vec<BackendEntry>>,
    id_minter: PreviewIdMinter,
}

impl Default for VerbRuntime {
    fn default() -> Self {
        Self {
            verbs: RwLock::new(HashMap::new()),
            backends: RwLock::new(Vec::new()),
            id_minter: PreviewIdMinter::new(),
        }
    }
}

impl fmt::Debug for VerbRuntime {
    /// Lists registered verb IDs and backend IDs. Neither `dyn Verb`
    /// nor `dyn InferenceBackend` is `Debug`; we surface the stable
    /// identifiers instead. `id_minter` is intentionally omitted.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let verbs = self.verbs.read();
        let verb_ids: Vec<&VerbId> = verbs.keys().collect();
        let backends = self.backends.read();
        let backend_ids: Vec<&str> = backends.iter().map(|e| e.backend.id()).collect();
        f.debug_struct("VerbRuntime")
            .field("verbs", &verb_ids)
            .field("backends", &backend_ids)
            .finish_non_exhaustive()
    }
}

impl VerbRuntime {
    /// Constructs an empty runtime. No verbs are registered until the
    /// host calls [`Self::register`].
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a verb. Returns
    /// [`VerbError::AlreadyRegistered`] if a verb with the same ID is
    /// already present.
    pub fn register<V: Verb>(&self, verb: V) -> Result<()> {
        let id = verb.descriptor().id.clone();
        let mut verbs = self.verbs.write();
        if verbs.contains_key(&id) {
            return Err(VerbError::AlreadyRegistered(id));
        }
        debug!(id = %id, "registering verb");
        verbs.insert(id, Arc::new(verb));
        Ok(())
    }

    /// Removes a verb from the registry. Returns
    /// [`VerbError::NotFound`] if no such verb exists. Pending
    /// invocations of the verb continue to run — unregistering is
    /// shallow.
    pub fn unregister(&self, id: &VerbId) -> Result<()> {
        let mut verbs = self.verbs.write();
        if verbs.remove(id).is_none() {
            return Err(VerbError::NotFound(id.clone()));
        }
        debug!(id = %id, "unregistered verb");
        Ok(())
    }

    /// Returns the descriptors of all registered verbs, sorted by ID.
    /// Useful for command-palette population.
    #[must_use]
    pub fn list(&self) -> Vec<VerbDescriptor> {
        let verbs = self.verbs.read();
        let mut out: Vec<VerbDescriptor> = verbs.values().map(|v| v.descriptor().clone()).collect();
        out.sort_by(|a, b| a.id.cmp(&b.id));
        out
    }

    /// Looks up a verb's descriptor by ID without invoking it.
    #[must_use]
    pub fn descriptor(&self, id: &VerbId) -> Option<VerbDescriptor> {
        self.verbs.read().get(id).map(|v| v.descriptor().clone())
    }

    /// Returns the number of registered verbs.
    #[must_use]
    pub fn len(&self) -> usize {
        self.verbs.read().len()
    }

    /// Returns `true` if no verbs are registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.verbs.read().is_empty()
    }

    // ── Backend registry ────────────────────────────────────────────────────

    /// Registers an inference backend.
    ///
    /// Backends are ordered by `priority`; lower value = tried first
    /// when selecting a backend for a verb invocation. The list is kept
    /// sorted on insertion so [`Self::select_backend`] is a linear scan
    /// without an extra sort pass.
    ///
    /// Returns [`VerbError::BackendAlreadyRegistered`] if a backend
    /// with the same id already exists. The dedicated variant (rather
    /// than `AlreadyRegistered`, which is for verbs) lets backend-
    /// management UIs surface "backend X already registered" instead of
    /// "verb X already registered".
    pub fn register_backend<B: InferenceBackend>(&self, backend: B, priority: u16) -> Result<()> {
        let id_str = backend.id().to_owned();
        let arc: Arc<dyn InferenceBackend> = Arc::new(backend);
        let mut backends = self.backends.write();
        if backends.iter().any(|e| e.backend.id() == id_str) {
            return Err(VerbError::BackendAlreadyRegistered(id_str));
        }
        debug!(id = %id_str, priority, "registering backend");
        // Insert in sorted position so the list stays ordered.
        let pos = backends
            .iter()
            .position(|e| e.priority > priority)
            .unwrap_or(backends.len());
        backends.insert(
            pos,
            BackendEntry {
                backend: arc,
                priority,
            },
        );
        Ok(())
    }

    /// Removes a backend from the registry. Returns
    /// [`VerbError::BackendNotFound`] if no such backend exists.
    ///
    /// In-flight invocations that already received a reference to this
    /// backend continue to run — unregistering is shallow.
    pub fn unregister_backend(&self, id: &str) -> Result<()> {
        let mut backends = self.backends.write();
        let pos = backends
            .iter()
            .position(|e| e.backend.id() == id)
            .ok_or_else(|| VerbError::BackendNotFound(id.to_owned()))?;
        debug!(id, "unregistered backend");
        backends.remove(pos);
        Ok(())
    }

    /// Returns metadata snapshots for all registered backends, in
    /// priority order.
    #[must_use]
    pub fn list_backends(&self) -> Vec<BackendInfo> {
        self.backends
            .read()
            .iter()
            .map(|e| BackendInfo {
                id: e.backend.id().to_owned(),
                display_name: e.backend.display_name().to_owned(),
                capabilities: e.backend.capabilities(),
                available: e.backend.is_available(),
                priority: e.priority,
            })
            .collect()
    }

    /// Returns the number of registered backends.
    #[must_use]
    pub fn backend_count(&self) -> usize {
        self.backends.read().len()
    }

    /// Selects the highest-priority backend that is available *and*
    /// whose capabilities are a superset of `required`.
    ///
    /// Distinguishes two failure modes the UI needs to render
    /// differently:
    ///
    /// - [`VerbError::UnsupportedCapability`] — no registered backend
    ///   advertises the required capabilities at all. The user must
    ///   configure a suitable backend.
    /// - [`VerbError::BackendUnavailable`] — at least one registered
    ///   backend matches the capability set, but every match reports
    ///   `is_available() == false`. The user should start the backend
    ///   (e.g. launch the Ollama process) and retry.
    ///
    /// The search is linear over the priority-sorted list; for the
    /// expected registry size (< 10 backends) a sorted scan is faster
    /// than a heap.
    pub fn select_backend(
        &self,
        required: BackendCapabilities,
        verb: &VerbId,
    ) -> Result<Arc<dyn InferenceBackend>> {
        let backends = self.backends.read();
        // First pass: an available backend that matches.
        if let Some(entry) = backends
            .iter()
            .find(|e| e.backend.is_available() && e.backend.capabilities().contains(required))
        {
            return Ok(entry.backend.clone());
        }
        // Second pass: any backend matches but isn't available — surface
        // the highest-priority match's id so the UI can guide the user.
        if let Some(entry) = backends
            .iter()
            .find(|e| e.backend.capabilities().contains(required))
        {
            return Err(VerbError::BackendUnavailable {
                id: entry.backend.id().to_owned(),
            });
        }
        // No registered backend matches at all.
        Err(VerbError::UnsupportedCapability {
            verb: verb.clone(),
            required,
        })
    }

    /// Selects a specific available backend by id and verifies that it
    /// satisfies the requested capabilities.
    pub fn select_backend_by_id(
        &self,
        id: &str,
        required: BackendCapabilities,
        verb: &VerbId,
    ) -> Result<Arc<dyn InferenceBackend>> {
        let backends = self.backends.read();
        let Some(entry) = backends.iter().find(|entry| entry.backend.id() == id) else {
            return Err(VerbError::BackendNotFound(id.to_owned()));
        };
        if !entry.backend.capabilities().contains(required) {
            return Err(VerbError::UnsupportedCapability {
                verb: verb.clone(),
                required,
            });
        }
        if !entry.backend.is_available() {
            return Err(VerbError::BackendUnavailable { id: id.to_owned() });
        }
        Ok(entry.backend.clone())
    }

    // ── Dispatch ────────────────────────────────────────────────────────────

    /// Starts an invocation. Returns a [`VerbInvocation`] the caller
    /// drives to completion.
    ///
    /// Before spawning, the runtime:
    ///
    /// 1. Looks up the verb by `id` — `VerbError::NotFound` if absent.
    /// 2. Validates `inputs` via the verb's `validate` — `VerbError::Schema`
    ///    on failure.
    /// 3. If the verb's `required_capabilities` are non-empty, selects the
    ///    highest-priority available backend that satisfies them, and
    ///    attaches it to `ctx`. Returns `VerbError::UnsupportedCapability`
    ///    if no backend qualifies.
    /// 4. Spawns the verb on the tokio runtime with a fresh cancellation
    ///    token and progress channel.
    ///
    /// The caller drains progress through [`VerbInvocation::next_progress`]
    /// and awaits [`VerbInvocation::finish`] for the [`VerbPreview`].
    #[instrument(skip(self, ctx, inputs), fields(verb = %id))]
    pub fn invoke(
        &self,
        id: &VerbId,
        ctx: VerbContext,
        inputs: VerbInputs,
    ) -> Result<VerbInvocation> {
        let verb = self
            .verbs
            .read()
            .get(id)
            .cloned()
            .ok_or_else(|| VerbError::NotFound(id.clone()))?;
        verb.validate(&inputs)?;

        // Capability check and backend injection. Verbs that need no
        // backend (empty capabilities) skip this; ctx.backend stays None.
        // The capability requirement is input-dependent — a verb may
        // need no backend for some inputs (e.g. a procedural mode).
        let mut ctx = ctx;
        let required = verb.required_capabilities_for(&inputs);
        if !required.is_empty() {
            let backend = self.select_backend(required, id)?;
            debug!(
                backend = %backend.id(),
                "selected backend for verb"
            );
            ctx.backend = Some(backend);
        }

        let cancel = CancellationToken::new();
        let (progress, progress_rx) = VerbProgress::channel();
        let started = Instant::now();
        let descriptor = verb.descriptor().clone();
        // Mint the preview ID up front so the invocation is `'static`
        // and can be moved into a tokio::spawn or stored in app state
        // without tying it to a borrow of the runtime. IDs are cheap
        // (a single atomic increment); spending one on a verb that
        // ends up failing is fine.
        let preview_id = self.id_minter.issue();

        let cancel_for_task = cancel.clone();
        let join: JoinHandle<Result<VerbOutput>> = tokio::spawn(async move {
            trace!("verb worker entered");
            let res = verb
                .invoke(ctx, inputs, progress, cancel_for_task.clone())
                .await;
            trace!(
                ok = res.is_ok(),
                cancelled = cancel_for_task.is_cancelled(),
                "verb worker exiting"
            );
            // If the verb returned Ok but the token has fired, surface
            // cancellation rather than committing a partial preview.
            match res {
                Ok(_) if cancel_for_task.is_cancelled() => Err(VerbError::Cancelled),
                other => other,
            }
        });

        Ok(VerbInvocation {
            verb_id: id.clone(),
            descriptor,
            cancel,
            progress_rx,
            join: Some(join),
            started,
            preview_id,
        })
    }

    /// Materialises a `VerbCommit` from an accepted preview.
    ///
    /// The runtime is stateless about previews; this method only
    /// re-shapes the value with a fresh timestamp. The undo system
    /// (S05) consumes the result and turns each
    /// [`super::super::output::VerbEffect`] into a reversible command.
    ///
    /// `&self` is included for API symmetry with the rest of the
    /// runtime's methods and to leave room for future state — e.g.
    /// per-session telemetry counters — without breaking call sites.
    #[allow(clippy::unused_self)]
    #[instrument(skip(self, preview), fields(verb = %preview.verb, preview = preview.id.get()))]
    pub fn commit(&self, preview: VerbPreview) -> VerbCommit {
        debug!("committing preview");
        VerbCommit {
            preview: preview.id,
            verb: preview.verb,
            effects: preview.output.effects,
            committed_at: now_unix_seconds(),
            elapsed: preview.elapsed,
        }
    }

    /// Records a discard. Mirrors [`Self::commit`] for the rejected
    /// path; the returned record is for telemetry / analytics.
    ///
    /// `&self` is included for the same reason as
    /// [`Self::commit`] — symmetry plus room for future state.
    #[allow(clippy::unused_self)]
    #[instrument(skip(self, preview, reason), fields(verb = %preview.verb, preview = preview.id.get()))]
    pub fn discard(&self, preview: VerbPreview, reason: Option<String>) -> VerbDiscard {
        debug!(?reason, "discarding preview");
        VerbDiscard {
            preview: preview.id,
            verb: preview.verb,
            reason,
            discarded_at: now_unix_seconds(),
        }
    }
}
