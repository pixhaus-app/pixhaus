//! Verb runtime: registry, dispatch, preview/commit lifecycle.
//!
//! [`VerbRuntime`] is the host-side coordinator. It owns the registry
//! of `Arc<dyn Verb>` instances, mints [`super::preview::PreviewId`]s,
//! and converts an invocation request into a
//! [`VerbInvocation`] handle the caller drives.
//!
//! # State model
//!
//! The runtime is intentionally lightweight:
//!
//! - `verbs` — read-mostly registry behind a `parking_lot::RwLock`.
//!   Reads (lookup by ID, list descriptors) take the read lock; the
//!   only writes happen during plugin load / unload.
//! - `id_minter` — atomic counter for preview IDs. Lock-free.
//!
//! There is **no** pending-preview map. [`VerbPreview`]s are values
//! the caller carries between `finish` and `commit` / `discard`. This
//! is by design — every preview-tracking system that lived inside the
//! runtime had to handle "what if the preview is dropped?", "what if
//! the user closes the dialog?", "what if the document closes?". By
//! handing the preview back to the caller, the runtime sidesteps the
//! whole class of bugs.

use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use std::time::{Instant, SystemTime};

use parking_lot::RwLock;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{debug, instrument, trace};

use super::context::VerbContext;
use super::descriptor::{VerbDescriptor, VerbId};
use super::error::{Result, VerbError};
use super::inputs::VerbInputs;
use super::output::VerbOutput;
use super::preview::{PreviewIdMinter, VerbCommit, VerbDiscard, VerbPreview};
use super::progress::{VerbProgress, VerbProgressEvent};
use super::verb::Verb;

/// Returns the current UTC time as seconds since the Unix epoch,
/// clamped at `i64::MAX` if the system clock is far enough in the
/// future to overflow `i64`.
fn now_unix_seconds() -> i64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .ok()
        .and_then(|d| i64::try_from(d.as_secs()).ok())
        .unwrap_or(i64::MAX)
}

/// Coordinator for verb registration and dispatch.
///
/// One `VerbRuntime` lives per editor session. Construct via
/// [`VerbRuntime::new`], wrap in an `Arc`, and share across the
/// command handlers.
#[derive(Default)]
pub struct VerbRuntime {
    verbs: RwLock<HashMap<VerbId, Arc<dyn Verb>>>,
    id_minter: PreviewIdMinter,
}

impl fmt::Debug for VerbRuntime {
    /// Lists registered verb IDs only — `dyn Verb` is not `Debug`, and
    /// dumping descriptors at every `dbg!` would be noisy. The
    /// `id_minter` field is intentionally omitted; its monotonic
    /// counter is internal scaffolding and not interesting in a debug
    /// dump.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let verbs = self.verbs.read();
        let ids: Vec<&VerbId> = verbs.keys().collect();
        f.debug_struct("VerbRuntime")
            .field("registered", &ids)
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

    /// Starts an invocation. Returns a [`VerbInvocation`] the caller
    /// drives to completion.
    ///
    /// The verb is looked up, validated against `inputs`, and spawned
    /// as a tokio task with a fresh cancellation token and progress
    /// channel. The caller drains progress through
    /// [`VerbInvocation::next_progress`] and ultimately awaits
    /// [`VerbInvocation::finish`] for the [`VerbPreview`].
    #[instrument(skip(self, ctx, inputs), fields(verb = %id))]
    pub fn invoke(
        &self,
        id: &VerbId,
        ctx: VerbContext,
        inputs: VerbInputs,
    ) -> Result<VerbInvocation<'_>> {
        let verb = self
            .verbs
            .read()
            .get(id)
            .cloned()
            .ok_or_else(|| VerbError::NotFound(id.clone()))?;
        verb.validate(&inputs)?;

        let cancel = CancellationToken::new();
        let (progress, progress_rx) = VerbProgress::channel();
        let started = Instant::now();
        let descriptor = verb.descriptor().clone();

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
            join,
            started,
            id_minter: &self.id_minter,
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
/// Holds:
/// - a [`CancellationToken`] the caller fires to cancel the verb;
/// - the receiver half of the progress channel;
/// - the join handle of the spawned worker;
/// - the start instant for elapsed-time bookkeeping.
///
/// The handle borrows the runtime's [`PreviewIdMinter`] so
/// [`Self::finish`] can mint the resulting preview's ID without
/// reaching back into the runtime. The borrow lives only as long as
/// the invocation itself.
pub struct VerbInvocation<'a> {
    verb_id: VerbId,
    descriptor: VerbDescriptor,
    cancel: CancellationToken,
    progress_rx: mpsc::Receiver<VerbProgressEvent>,
    join: JoinHandle<Result<VerbOutput>>,
    started: Instant,
    id_minter: &'a PreviewIdMinter,
}

impl fmt::Debug for VerbInvocation<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VerbInvocation")
            .field("verb", &self.verb_id)
            .field("started", &self.started)
            .field("cancelled", &self.cancel.is_cancelled())
            .finish()
    }
}

impl VerbInvocation<'_> {
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
    pub async fn finish(self) -> Result<VerbPreview> {
        let Self {
            join,
            started,
            id_minter,
            verb_id,
            ..
        } = self;
        let join_result = join.await;
        let elapsed = started.elapsed();
        let output = match join_result {
            Ok(inner) => inner?,
            Err(join_err) => return Err(VerbError::Aborted(join_err.to_string())),
        };
        let preview_id = id_minter.issue();
        Ok(VerbPreview::new(preview_id, verb_id, output, elapsed))
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
}
