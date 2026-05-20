//! Verb runtime: registry, dispatch, backend selection, preview/commit
//! lifecycle.
//!
//! [`VerbRuntime`] is the host-side coordinator. It owns:
//!
//! - the verb registry (`Arc<dyn Verb>` by ID),
//! - the backend registry (`Arc<dyn InferenceBackend>` ordered by
//!   priority), and
//! - the preview ID minter.
//!
//! # State model
//!
//! The runtime keeps two read-mostly registries behind `RwLock`s, and
//! one lock-free atomic counter:
//!
//! - `verbs` — writes only on plugin load / unload.
//! - `backends` — writes only when the user changes backend config.
//! - `id_minter` — lock-free atomic increment; never blocks.
//!
//! There is **no** pending-preview map. [`VerbPreview`]s are values the
//! caller carries between `finish` and `commit` / `discard`. Handing
//! the value back to the caller sidesteps the whole "what if the dialog
//! closes?" class of bugs.
//!
//! # Backend selection and fallback chains
//!
//! Before spawning a verb worker, [`VerbRuntime::invoke`] resolves a
//! backend:
//!
//! 1. If the verb's `required_capabilities` are empty, no backend is
//!    needed — `ctx.backend` is `None`.
//! 2. Otherwise the runtime walks `backends` in ascending priority order
//!    and picks the first entry that is available *and* whose
//!    `capabilities` are a superset of the required set.
//! 3. If no backend qualifies, the invocation fails with
//!    [`VerbError::UnsupportedCapability`].
//!
//! The selected backend is attached to the `VerbContext` before it
//! reaches the verb. Verbs downcast `ctx.backend` to a more specific
//! sub-trait when they need to make inference calls.

use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use std::time::Instant;

use parking_lot::RwLock;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{debug, instrument, trace};

use super::backend::{BackendInfo, InferenceBackend};
use super::context::VerbContext;
use super::descriptor::{BackendCapabilities, VerbDescriptor, VerbId};
use super::error::{Result, VerbError};
use super::inputs::VerbInputs;
use super::output::VerbOutput;
use super::preview::{
    PreviewId, PreviewIdMinter, VerbCommit, VerbDiscard, VerbPreview, now_unix_seconds,
};
use super::progress::{VerbProgress, VerbProgressEvent};
use super::verb::Verb;

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
    /// [`super::output::VerbEffect`] into a reversible command.
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

/// Active invocation handle returned by [`VerbRuntime::invoke`].
///
/// Owns everything it needs to drive the verb to completion:
/// - a [`CancellationToken`] the caller fires to cancel the verb;
/// - the receiver half of the progress channel;
/// - the join handle of the spawned worker;
/// - the start instant for elapsed-time bookkeeping;
/// - a [`PreviewId`] minted at invocation time, so the handle is
///   `'static` and can be moved into a tokio task or stored in
///   long-lived app state without borrowing the runtime.
pub struct VerbInvocation {
    verb_id: VerbId,
    descriptor: VerbDescriptor,
    cancel: CancellationToken,
    progress_rx: mpsc::Receiver<VerbProgressEvent>,
    /// `Some` until [`Self::finish`] consumes it. The `Option` lets
    /// `Drop` distinguish a consumed handle (no-op) from an abandoned
    /// one (fire cancellation).
    join: Option<JoinHandle<Result<VerbOutput>>>,
    started: Instant,
    preview_id: PreviewId,
}

impl fmt::Debug for VerbInvocation {
    /// Lists the fields useful for diagnostics; the descriptor, the
    /// progress receiver, and the join handle are intentionally
    /// omitted (verbose, not informative in a debug dump).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VerbInvocation")
            .field("verb", &self.verb_id)
            .field("preview_id", &self.preview_id)
            .field("started", &self.started)
            .field("cancelled", &self.cancel.is_cancelled())
            .finish_non_exhaustive()
    }
}

impl Drop for VerbInvocation {
    /// Fires the cancellation token when the handle is dropped before
    /// [`Self::finish`] consumed it.
    ///
    /// Without this, dropping a `VerbInvocation` (closing a UI dialog,
    /// `?` propagation in a host method) leaves the spawned worker
    /// running with no observer — an LLM call has real cost on the
    /// line. Cooperative cancellation is the protocol's contract;
    /// authors who want forcible abort should hold their own
    /// `JoinHandle` and `abort()` explicitly.
    ///
    /// `finish` takes the join handle out, so its absence here means
    /// the invocation already completed naturally and cancelling would
    /// be a redundant signal (also unhelpful when introspecting
    /// `cancel.is_cancelled()` after a successful run).
    fn drop(&mut self) {
        if self.join.is_some() {
            self.cancel.cancel();
        }
    }
}

impl VerbInvocation {
    /// Returns the verb ID this invocation is running.
    #[must_use]
    pub fn verb(&self) -> &VerbId {
        &self.verb_id
    }

    /// Returns the verb's descriptor.
    #[must_use]
    pub fn descriptor(&self) -> &VerbDescriptor {
        &self.descriptor
    }

    /// Returns the [`PreviewId`] minted for this invocation. Stable
    /// for the lifetime of the invocation; reused as the resulting
    /// [`VerbPreview::id`].
    #[must_use]
    pub fn preview_id(&self) -> PreviewId {
        self.preview_id
    }

    /// Fires the cancellation token. The verb observes it via
    /// `cancel.is_cancelled()` between expensive operations.
    pub fn cancel(&self) {
        self.cancel.cancel();
    }

    /// Returns a clone of the cancellation token. The caller can
    /// share it with auxiliary tasks (e.g. a timeout watchdog).
    #[must_use]
    pub fn cancellation(&self) -> CancellationToken {
        self.cancel.clone()
    }

    /// Awaits the next progress event, or `None` once the verb has
    /// finished and the channel is drained.
    pub async fn next_progress(&mut self) -> Option<VerbProgressEvent> {
        self.progress_rx.recv().await
    }

    /// Awaits the verb's completion and packages the result as a
    /// [`VerbPreview`].
    ///
    /// On verb error, the invocation propagates the error. On a
    /// panicked or aborted worker, returns
    /// [`VerbError::Aborted`].
    pub async fn finish(mut self) -> Result<VerbPreview> {
        // `take` leaves `self.join` as `None`; the `Drop` impl uses
        // that to skip its cancel-on-drop signal, since natural
        // completion already drove the worker to exit. A `None` here
        // means a programmer called `finish` twice; surface that as
        // `Aborted` rather than panic — the no-unwrap/no-expect rule.
        let join = self
            .join
            .take()
            .ok_or_else(|| VerbError::Aborted("VerbInvocation::finish called twice".into()))?;
        let join_result = join.await;
        let elapsed = self.started.elapsed();
        let output = match join_result {
            Ok(inner) => inner?,
            Err(join_err) => return Err(VerbError::Aborted(join_err.to_string())),
        };
        Ok(VerbPreview::new(
            self.preview_id,
            self.verb_id.clone(),
            output,
            elapsed,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::descriptor::{
        BackendCapabilities, CostEstimate, EffectKind, VerbDescriptor, VerbId,
    };
    use crate::plugin::output::{ActualCost, VerbOutput};
    use crate::plugin::preview::PreviewId;
    use crate::plugin::progress::VerbProgressEvent;
    use async_trait::async_trait;
    use pixhaus_core::project::ProjectMetadata;
    use std::time::Duration;

    fn metadata() -> ProjectMetadata {
        ProjectMetadata {
            name: "t".into(),
            description: None,
            author: None,
            created_at: 0,
            updated_at: 0,
            editor_version: "0".into(),
        }
    }

    fn descriptor(id: &str, cancellable: bool, streaming: bool) -> VerbDescriptor {
        VerbDescriptor {
            id: VerbId::new(id),
            display_name: id.into(),
            description: id.into(),
            version: "0.0.1".into(),
            required_capabilities: BackendCapabilities::empty(),
            input_schema: serde_json::json!({}),
            output_schema: None,
            output_kinds: vec![EffectKind::AddLayer],
            cost_estimate: CostEstimate::free(),
            streaming,
            cancellable,
            documentation_url: None,
        }
    }

    fn empty_output(elapsed: Duration) -> VerbOutput {
        VerbOutput {
            summary: "ok".into(),
            effects: vec![],
            thumbnail: None,
            actual_cost: ActualCost::free(elapsed),
            notes: vec![],
        }
    }

    struct ImmediateOk(VerbDescriptor);
    #[async_trait]
    impl Verb for ImmediateOk {
        fn descriptor(&self) -> &VerbDescriptor {
            &self.0
        }
        async fn invoke(
            &self,
            _ctx: VerbContext,
            _inputs: VerbInputs,
            progress: VerbProgress,
            _cancel: CancellationToken,
        ) -> Result<VerbOutput> {
            progress
                .send(VerbProgressEvent::Started { backend: None })
                .await;
            Ok(empty_output(Duration::ZERO))
        }
    }

    struct ObservesCancellation(VerbDescriptor);
    #[async_trait]
    impl Verb for ObservesCancellation {
        fn descriptor(&self) -> &VerbDescriptor {
            &self.0
        }
        async fn invoke(
            &self,
            _ctx: VerbContext,
            _inputs: VerbInputs,
            _progress: VerbProgress,
            cancel: CancellationToken,
        ) -> Result<VerbOutput> {
            cancel.cancelled().await;
            Err(VerbError::Cancelled)
        }
    }

    struct Validates(VerbDescriptor);
    #[async_trait]
    impl Verb for Validates {
        fn descriptor(&self) -> &VerbDescriptor {
            &self.0
        }
        fn validate(&self, inputs: &VerbInputs) -> Result<()> {
            if inputs.as_value().is_object() {
                Ok(())
            } else {
                Err(VerbError::Schema("inputs must be an object".into()))
            }
        }
        async fn invoke(
            &self,
            _ctx: VerbContext,
            _inputs: VerbInputs,
            _progress: VerbProgress,
            _cancel: CancellationToken,
        ) -> Result<VerbOutput> {
            Ok(empty_output(Duration::ZERO))
        }
    }

    #[tokio::test]
    async fn register_and_invoke_round_trip() {
        let runtime = VerbRuntime::new();
        runtime
            .register(ImmediateOk(descriptor("test.ok", true, true)))
            .unwrap();

        assert_eq!(runtime.len(), 1);
        assert!(!runtime.is_empty());
        let descs = runtime.list();
        assert_eq!(descs.len(), 1);
        assert_eq!(descs[0].id.as_str(), "test.ok");

        let mut inv = runtime
            .invoke(
                &VerbId::new("test.ok"),
                VerbContext::empty(metadata()),
                VerbInputs::empty(),
            )
            .unwrap();

        let first = inv.next_progress().await.unwrap();
        assert!(matches!(first, VerbProgressEvent::Started { .. }));

        let preview = inv.finish().await.unwrap();
        assert_eq!(preview.verb.as_str(), "test.ok");
        assert_eq!(preview.id.get(), 1);
    }

    #[tokio::test]
    async fn duplicate_registration_fails() {
        let runtime = VerbRuntime::new();
        runtime
            .register(ImmediateOk(descriptor("test.dup", true, false)))
            .unwrap();
        let err = runtime
            .register(ImmediateOk(descriptor("test.dup", true, false)))
            .unwrap_err();
        assert!(matches!(err, VerbError::AlreadyRegistered(_)));
    }

    #[tokio::test]
    async fn unknown_verb_returns_not_found() {
        let runtime = VerbRuntime::new();
        let err = runtime
            .invoke(
                &VerbId::new("test.missing"),
                VerbContext::empty(metadata()),
                VerbInputs::empty(),
            )
            .unwrap_err();
        assert!(matches!(err, VerbError::NotFound(_)));
    }

    #[tokio::test]
    async fn validate_failure_propagates() {
        let runtime = VerbRuntime::new();
        runtime
            .register(Validates(descriptor("test.validates", true, false)))
            .unwrap();
        let err = runtime
            .invoke(
                &VerbId::new("test.validates"),
                VerbContext::empty(metadata()),
                VerbInputs::new(serde_json::Value::Null),
            )
            .unwrap_err();
        assert!(matches!(err, VerbError::Schema(_)));
    }

    #[tokio::test]
    async fn cancellation_propagates_to_verb() {
        let runtime = VerbRuntime::new();
        runtime
            .register(ObservesCancellation(descriptor("test.cancel", true, false)))
            .unwrap();
        let inv = runtime
            .invoke(
                &VerbId::new("test.cancel"),
                VerbContext::empty(metadata()),
                VerbInputs::empty(),
            )
            .unwrap();
        inv.cancel();
        let res = inv.finish().await;
        assert!(matches!(res, Err(VerbError::Cancelled)));
    }

    #[tokio::test]
    async fn ok_with_cancelled_token_surfaces_cancelled() {
        struct OkButCancels(VerbDescriptor);
        #[async_trait]
        impl Verb for OkButCancels {
            fn descriptor(&self) -> &VerbDescriptor {
                &self.0
            }
            async fn invoke(
                &self,
                _ctx: VerbContext,
                _inputs: VerbInputs,
                _progress: VerbProgress,
                cancel: CancellationToken,
            ) -> Result<VerbOutput> {
                cancel.cancel();
                Ok(empty_output(Duration::ZERO))
            }
        }

        let runtime = VerbRuntime::new();
        runtime
            .register(OkButCancels(descriptor("test.ok-cancels", true, false)))
            .unwrap();
        let inv = runtime
            .invoke(
                &VerbId::new("test.ok-cancels"),
                VerbContext::empty(metadata()),
                VerbInputs::empty(),
            )
            .unwrap();
        let res = inv.finish().await;
        assert!(matches!(res, Err(VerbError::Cancelled)));
    }

    #[tokio::test]
    async fn commit_and_discard_are_pure() {
        let runtime = VerbRuntime::new();
        let preview = VerbPreview::new(
            PreviewId::new(7),
            VerbId::new("test.x"),
            empty_output(Duration::from_millis(3)),
            Duration::from_millis(3),
        );
        let commit = runtime.commit(preview.clone());
        assert_eq!(commit.preview, preview.id);
        assert_eq!(commit.elapsed, preview.elapsed);

        let discard = runtime.discard(preview.clone(), Some("user rejected".into()));
        assert_eq!(discard.preview, preview.id);
        assert_eq!(discard.reason.as_deref(), Some("user rejected"));
    }

    #[tokio::test]
    async fn drop_cancels_in_flight_invocation() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicBool, Ordering};

        struct SignalsOnCancel {
            descriptor: VerbDescriptor,
            observed: Arc<AtomicBool>,
        }
        #[async_trait]
        impl Verb for SignalsOnCancel {
            fn descriptor(&self) -> &VerbDescriptor {
                &self.descriptor
            }
            async fn invoke(
                &self,
                _ctx: VerbContext,
                _inputs: VerbInputs,
                _progress: VerbProgress,
                cancel: CancellationToken,
            ) -> Result<VerbOutput> {
                cancel.cancelled().await;
                self.observed.store(true, Ordering::SeqCst);
                Err(VerbError::Cancelled)
            }
        }

        let observed = Arc::new(AtomicBool::new(false));
        let runtime = VerbRuntime::new();
        runtime
            .register(SignalsOnCancel {
                descriptor: descriptor("test.drop-cancels", true, false),
                observed: observed.clone(),
            })
            .unwrap();

        {
            let _inv = runtime
                .invoke(
                    &VerbId::new("test.drop-cancels"),
                    VerbContext::empty(metadata()),
                    VerbInputs::empty(),
                )
                .unwrap();
            // Drop at end of scope; Drop impl fires the cancel token.
        }

        // Worker observes cancellation and sets `observed`. Poll briefly
        // with a deadline rather than a fixed sleep.
        let deadline = std::time::Instant::now() + Duration::from_secs(1);
        while !observed.load(Ordering::SeqCst) && std::time::Instant::now() < deadline {
            tokio::task::yield_now().await;
        }
        assert!(
            observed.load(Ordering::SeqCst),
            "drop did not cancel the in-flight verb"
        );
    }

    #[tokio::test]
    async fn unregister_removes_verb() {
        let runtime = VerbRuntime::new();
        runtime
            .register(ImmediateOk(descriptor("test.un", true, false)))
            .unwrap();
        runtime.unregister(&VerbId::new("test.un")).unwrap();
        assert!(runtime.is_empty());
        let err = runtime.unregister(&VerbId::new("test.un")).unwrap_err();
        assert!(matches!(err, VerbError::NotFound(_)));
    }

    // ── Backend registry tests ───────────────────────────────────────────────

    use crate::plugin::backend::InferenceBackend;

    #[derive(Debug)]
    struct StubBackend {
        id: &'static str,
        caps: BackendCapabilities,
        available: bool,
    }

    impl InferenceBackend for StubBackend {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
        fn id(&self) -> &'static str {
            self.id
        }
        fn capabilities(&self) -> BackendCapabilities {
            self.caps
        }
        fn cost_estimate(&self, _: BackendCapabilities) -> CostEstimate {
            CostEstimate::free()
        }
        fn is_available(&self) -> bool {
            self.available
        }
    }

    fn caps_backend(id: &'static str, caps: BackendCapabilities) -> StubBackend {
        StubBackend {
            id,
            caps,
            available: true,
        }
    }

    #[test]
    fn register_and_list_backends() {
        let rt = VerbRuntime::new();
        rt.register_backend(
            caps_backend("local", BackendCapabilities::TEXT_GENERATION),
            0,
        )
        .unwrap();
        rt.register_backend(
            caps_backend("cloud", BackendCapabilities::IMAGE_GENERATION),
            10,
        )
        .unwrap();

        let list = rt.list_backends();
        assert_eq!(list.len(), 2);
        // Priority-sorted: local (0) before cloud (10).
        assert_eq!(list[0].id, "local");
        assert_eq!(list[1].id, "cloud");
        assert_eq!(rt.backend_count(), 2);
    }

    #[test]
    fn duplicate_backend_registration_fails() {
        let rt = VerbRuntime::new();
        rt.register_backend(caps_backend("b", BackendCapabilities::empty()), 0)
            .unwrap();
        let err = rt
            .register_backend(caps_backend("b", BackendCapabilities::empty()), 1)
            .unwrap_err();
        // Dedicated variant — the backend-management UI must distinguish
        // backend duplicates from verb duplicates.
        assert!(
            matches!(&err, VerbError::BackendAlreadyRegistered(id) if id == "b"),
            "expected BackendAlreadyRegistered, got {err:?}"
        );
    }

    #[test]
    fn unregister_backend_removes_entry() {
        let rt = VerbRuntime::new();
        rt.register_backend(caps_backend("b", BackendCapabilities::empty()), 0)
            .unwrap();
        rt.unregister_backend("b").unwrap();
        assert_eq!(rt.backend_count(), 0);

        let err = rt.unregister_backend("b").unwrap_err();
        assert!(
            matches!(&err, VerbError::BackendNotFound(id) if id == "b"),
            "expected BackendNotFound, got {err:?}"
        );
    }

    #[test]
    fn select_backend_respects_capabilities() {
        let rt = VerbRuntime::new();
        rt.register_backend(
            caps_backend("text-only", BackendCapabilities::TEXT_GENERATION),
            0,
        )
        .unwrap();
        rt.register_backend(
            caps_backend(
                "multimodal",
                BackendCapabilities::TEXT_GENERATION.union(BackendCapabilities::IMAGE_GENERATION),
            ),
            10,
        )
        .unwrap();

        // text-only is tried first and satisfies TEXT_GENERATION.
        let b = rt
            .select_backend(BackendCapabilities::TEXT_GENERATION, &VerbId::new("v"))
            .unwrap();
        assert_eq!(b.id(), "text-only");

        // IMAGE_GENERATION only available in multimodal.
        let b = rt
            .select_backend(BackendCapabilities::IMAGE_GENERATION, &VerbId::new("v"))
            .unwrap();
        assert_eq!(b.id(), "multimodal");
    }

    #[test]
    fn select_backend_skips_unavailable() {
        let rt = VerbRuntime::new();
        // High-priority backend that is down.
        rt.register_backend(
            StubBackend {
                id: "down",
                caps: BackendCapabilities::TEXT_GENERATION,
                available: false,
            },
            0,
        )
        .unwrap();
        // Lower-priority backend that is up.
        rt.register_backend(
            StubBackend {
                id: "up",
                caps: BackendCapabilities::TEXT_GENERATION,
                available: true,
            },
            10,
        )
        .unwrap();

        let b = rt
            .select_backend(BackendCapabilities::TEXT_GENERATION, &VerbId::new("v"))
            .unwrap();
        assert_eq!(b.id(), "up");
    }

    #[test]
    fn select_backend_returns_unsupported_when_none_match() {
        let rt = VerbRuntime::new();
        rt.register_backend(
            caps_backend("text-only", BackendCapabilities::TEXT_GENERATION),
            0,
        )
        .unwrap();

        let err = rt
            .select_backend(BackendCapabilities::IMAGE_GENERATION, &VerbId::new("v"))
            .unwrap_err();
        assert!(matches!(err, VerbError::UnsupportedCapability { .. }));
    }

    #[test]
    fn select_backend_distinguishes_unavailable_from_unsupported() {
        // Regression for thread 2: when the only registered backend
        // matching the required capabilities is unavailable, the runtime
        // must surface `BackendUnavailable` (so the UI can prompt the
        // user to start the backend) instead of `UnsupportedCapability`
        // (which would imply no backend exists at all).
        let rt = VerbRuntime::new();
        rt.register_backend(
            StubBackend {
                id: "ollama",
                caps: BackendCapabilities::TEXT_GENERATION,
                available: false,
            },
            0,
        )
        .unwrap();

        let err = rt
            .select_backend(BackendCapabilities::TEXT_GENERATION, &VerbId::new("v"))
            .unwrap_err();
        assert!(
            matches!(&err, VerbError::BackendUnavailable { id } if id == "ollama"),
            "expected BackendUnavailable, got {err:?}"
        );
    }

    #[test]
    fn select_backend_by_id_returns_named_backend() {
        let rt = VerbRuntime::new();
        rt.register_backend(
            caps_backend("google", BackendCapabilities::IMAGE_GENERATION),
            0,
        )
        .unwrap();

        let backend = rt
            .select_backend_by_id(
                "google",
                BackendCapabilities::IMAGE_GENERATION,
                &VerbId::new("v"),
            )
            .unwrap();

        assert_eq!(backend.id(), "google");
    }

    #[test]
    fn select_backend_by_id_rejects_unknown_id() {
        let rt = VerbRuntime::new();

        let err = rt
            .select_backend_by_id(
                "missing",
                BackendCapabilities::IMAGE_GENERATION,
                &VerbId::new("v"),
            )
            .unwrap_err();

        assert!(
            matches!(&err, VerbError::BackendNotFound(id) if id == "missing"),
            "expected BackendNotFound, got {err:?}"
        );
    }

    #[test]
    fn select_backend_by_id_checks_capabilities() {
        let rt = VerbRuntime::new();
        rt.register_backend(
            caps_backend("text-only", BackendCapabilities::TEXT_GENERATION),
            0,
        )
        .unwrap();

        let err = rt
            .select_backend_by_id(
                "text-only",
                BackendCapabilities::IMAGE_GENERATION,
                &VerbId::new("v"),
            )
            .unwrap_err();

        assert!(
            matches!(err, VerbError::UnsupportedCapability { .. }),
            "expected UnsupportedCapability, got {err:?}"
        );
    }

    #[test]
    fn select_backend_by_id_reports_unavailable_backend() {
        let rt = VerbRuntime::new();
        rt.register_backend(
            StubBackend {
                id: "down",
                caps: BackendCapabilities::IMAGE_GENERATION,
                available: false,
            },
            0,
        )
        .unwrap();

        let err = rt
            .select_backend_by_id(
                "down",
                BackendCapabilities::IMAGE_GENERATION,
                &VerbId::new("v"),
            )
            .unwrap_err();

        assert!(
            matches!(&err, VerbError::BackendUnavailable { id } if id == "down"),
            "expected BackendUnavailable, got {err:?}"
        );
    }

    #[tokio::test]
    async fn invoke_injects_backend_into_ctx() {
        // A verb that requires TEXT_GENERATION; its invoke records which
        // backend was selected.
        use std::sync::Arc;
        use std::sync::atomic::{AtomicBool, Ordering};

        struct RecordsBackend {
            descriptor: VerbDescriptor,
            saw_backend: Arc<AtomicBool>,
        }
        #[async_trait]
        impl Verb for RecordsBackend {
            fn descriptor(&self) -> &VerbDescriptor {
                &self.descriptor
            }
            async fn invoke(
                &self,
                ctx: VerbContext,
                _inputs: VerbInputs,
                _progress: VerbProgress,
                _cancel: CancellationToken,
            ) -> Result<VerbOutput> {
                self.saw_backend
                    .store(ctx.backend.is_some(), Ordering::SeqCst);
                Ok(empty_output(Duration::ZERO))
            }
        }

        let saw = Arc::new(AtomicBool::new(false));
        let rt = VerbRuntime::new();
        rt.register_backend(
            caps_backend("text", BackendCapabilities::TEXT_GENERATION),
            0,
        )
        .unwrap();

        let desc = VerbDescriptor {
            id: VerbId::new("test.needs-backend"),
            display_name: "NeedsBackend".into(),
            description: String::new(),
            version: "0.0.1".into(),
            required_capabilities: BackendCapabilities::TEXT_GENERATION,
            input_schema: serde_json::json!({}),
            output_schema: None,
            output_kinds: vec![EffectKind::AddLayer],
            cost_estimate: CostEstimate::free(),
            streaming: false,
            cancellable: false,
            documentation_url: None,
        };
        rt.register(RecordsBackend {
            descriptor: desc,
            saw_backend: saw.clone(),
        })
        .unwrap();

        let inv = rt
            .invoke(
                &VerbId::new("test.needs-backend"),
                VerbContext::empty(metadata()),
                VerbInputs::empty(),
            )
            .unwrap();
        inv.finish().await.unwrap();

        assert!(
            saw.load(Ordering::SeqCst),
            "verb did not receive a backend in ctx"
        );
    }

    #[tokio::test]
    async fn invoke_without_matching_backend_fails() {
        // Verb requires IMAGE_GENERATION but only TEXT_GENERATION is registered.
        struct NeedsImage(VerbDescriptor);
        #[async_trait]
        impl Verb for NeedsImage {
            fn descriptor(&self) -> &VerbDescriptor {
                &self.0
            }
            async fn invoke(
                &self,
                _ctx: VerbContext,
                _inputs: VerbInputs,
                _progress: VerbProgress,
                _cancel: CancellationToken,
            ) -> Result<VerbOutput> {
                Ok(empty_output(Duration::ZERO))
            }
        }

        let rt = VerbRuntime::new();
        rt.register_backend(
            caps_backend("text-only", BackendCapabilities::TEXT_GENERATION),
            0,
        )
        .unwrap();

        let desc = VerbDescriptor {
            id: VerbId::new("test.img"),
            display_name: "NeedsImage".into(),
            description: String::new(),
            version: "0.0.1".into(),
            required_capabilities: BackendCapabilities::IMAGE_GENERATION,
            input_schema: serde_json::json!({}),
            output_schema: None,
            output_kinds: vec![EffectKind::AddLayer],
            cost_estimate: CostEstimate::free(),
            streaming: false,
            cancellable: false,
            documentation_url: None,
        };
        rt.register(NeedsImage(desc)).unwrap();

        let err = rt
            .invoke(
                &VerbId::new("test.img"),
                VerbContext::empty(metadata()),
                VerbInputs::empty(),
            )
            .unwrap_err();
        assert!(
            matches!(err, VerbError::UnsupportedCapability { .. }),
            "expected UnsupportedCapability, got {err:?}"
        );
    }
}
