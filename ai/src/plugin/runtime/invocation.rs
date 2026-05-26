//! The in-flight invocation handle returned by
//! [`VerbRuntime::invoke`](super::VerbRuntime::invoke).
//!
//! [`VerbInvocation`] owns everything needed to drive a spawned verb
//! worker to completion: the cancellation token, the progress receiver,
//! the join handle, the start instant, and the preview ID minted at
//! invocation time. Keeping the handle `'static` (the preview ID is
//! minted up front rather than borrowed from the runtime) lets the caller
//! move it into a tokio task or store it in long-lived app state.

use std::fmt;
use std::time::Instant;

use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::plugin::descriptor::{VerbDescriptor, VerbId};
use crate::plugin::error::{Result, VerbError};
use crate::plugin::output::VerbOutput;
use crate::plugin::preview::{PreviewId, VerbPreview};
use crate::plugin::progress::VerbProgressEvent;

/// Active invocation handle returned by
/// [`VerbRuntime::invoke`](super::VerbRuntime::invoke).
///
/// Owns everything it needs to drive the verb to completion:
/// - a [`CancellationToken`] the caller fires to cancel the verb;
/// - the receiver half of the progress channel;
/// - the join handle of the spawned worker;
/// - the start instant for elapsed-time bookkeeping;
/// - a [`PreviewId`] minted at invocation time, so the handle is
///   `'static` and can be moved into a tokio task or stored in
///   long-lived app state without borrowing the runtime.
///
/// The fields are `pub(super)` so the runtime's `invoke` (in the sibling
/// `registry` module) can construct the handle after spawning the worker.
pub struct VerbInvocation {
    pub(super) verb_id: VerbId,
    pub(super) descriptor: VerbDescriptor,
    pub(super) cancel: CancellationToken,
    pub(super) progress_rx: mpsc::Receiver<VerbProgressEvent>,
    /// `Some` until [`Self::finish`] takes it to await the worker.
    pub(super) join: Option<JoinHandle<Result<VerbOutput>>>,
    pub(super) started: Instant,
    pub(super) preview_id: PreviewId,
    /// Set to `true` only once [`Self::finish`]'s await on the worker has
    /// returned. `Drop` keys cancellation off this rather than off
    /// `join.is_some()`: `finish` takes `join` *before* awaiting, so a
    /// `finish` future abandoned mid-await would otherwise leave
    /// `join == None` and silently skip the cancel.
    pub(super) finished: bool,
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
    /// `finish` sets `finished` only after its await on the worker
    /// returns, so an unset flag here means the invocation never reached
    /// completion — whether it was never awaited or its `finish` future
    /// was dropped mid-await. Either way the worker may still be running,
    /// so fire the cancel. Cancelling an already-completed token is a
    /// harmless no-op.
    fn drop(&mut self) {
        if !self.finished {
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
        // Take the handle out to await it. A `None` here would mean a
        // programmer called `finish` twice (impossible by-value, but
        // defended anyway); surface that as `Aborted` rather than panic —
        // the no-unwrap/no-expect rule.
        let join = self
            .join
            .take()
            .ok_or_else(|| VerbError::Aborted("VerbInvocation::finish called twice".into()))?;
        let join_result = join.await;
        // The worker has resolved. Mark finished so `Drop` does not fire a
        // redundant cancel. Crucially, if this future is dropped *before*
        // this line (abandoned mid-await), `finished` stays false and
        // `Drop` cancels the still-running worker.
        self.finished = true;
        let elapsed = self.started.elapsed();
        let output = match join_result {
            Ok(inner) => inner?,
            Err(join_err) => return Err(VerbError::Aborted(join_err.to_string())),
        };
        Ok(VerbPreview::new(self.preview_id, self.verb_id.clone(), output, elapsed))
    }
}
