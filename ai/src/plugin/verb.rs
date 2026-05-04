//! The [`Verb`] trait — the core of the plugin protocol.
//!
//! Every AI verb implements this trait. The trait is `Send + Sync +
//! 'static` so the runtime can register verbs as
//! `Arc<dyn Verb>` and dispatch them across worker threads.
//!
//! # Why `async-trait`
//!
//! Rust 1.95 supports native async fns in traits, but
//! `async fn` returning `impl Future` cannot be used through `dyn
//! Trait` without erasure. Verbs are inherently heterogeneous (the
//! registry holds `Arc<dyn Verb>`), so the protocol uses
//! [`async_trait::async_trait`] to box the returned future. The boxing
//! cost is one allocation per invocation — negligible against any verb
//! whose work touches an inference backend or a pixel buffer.

use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

use super::context::VerbContext;
use super::descriptor::VerbDescriptor;
use super::error::Result;
use super::inputs::VerbInputs;
use super::output::VerbOutput;
use super::progress::VerbProgress;

/// The static-method-only trait every AI verb implements.
///
/// # Lifecycle
///
/// 1. The host registers the verb with the runtime via
///    [`super::runtime::VerbRuntime::register`].
/// 2. On invocation, the runtime calls [`Verb::validate`] (which the
///    default impl makes a no-op).
/// 3. The runtime spawns [`Verb::invoke`] on the tokio runtime,
///    handing it a [`VerbContext`], the [`VerbInputs`], a
///    [`VerbProgress`] sender, and a cancellation token.
/// 4. The verb returns a [`VerbOutput`] *or* a
///    [`super::error::VerbError::Cancelled`] if it observed the token
///    firing.
/// 5. The runtime wraps the output in a
///    [`super::preview::VerbPreview`] which the host shows to the
///    user; on accept, the host calls
///    [`super::runtime::VerbRuntime::commit`].
///
/// # Cancellation
///
/// Verbs that declare `cancellable: true` in their descriptor are
/// expected to observe `cancel.is_cancelled()` between expensive
/// operations and return [`super::error::VerbError::Cancelled`] when
/// it fires. The runtime applies a hard timeout if the verb ignores
/// cancellation indefinitely; ignoring it is a bug.
///
/// # Threading
///
/// `invoke` runs on whichever runtime called it. CPU-bound verbs
/// should self-schedule onto `tokio::task::spawn_blocking`; the
/// runtime does not blanket-wrap invocations because doing so would
/// double-spawn for the common backend-driven case.
#[async_trait]
pub trait Verb: Send + Sync + 'static {
    /// Returns the static descriptor for this verb. The descriptor is
    /// expected to be stable for the lifetime of the verb instance —
    /// the runtime caches it on registration.
    fn descriptor(&self) -> &VerbDescriptor;

    /// Validates inputs against the verb's expectations *before*
    /// invocation. Default: always `Ok`. Override to check
    /// schema-shaped invariants the runtime cannot.
    ///
    /// Validation runs synchronously — keep it cheap. Anything that
    /// requires reading the project (e.g. "this layer ID exists")
    /// belongs in [`Self::invoke`] instead.
    fn validate(&self, _inputs: &VerbInputs) -> Result<()> {
        Ok(())
    }

    /// Runs the verb.
    ///
    /// `ctx` is a snapshot of the project state at invocation time.
    /// `inputs` is the typed payload the caller built. `progress` is
    /// the sender half of a bounded `mpsc` channel; the verb sends
    /// progress events through it. `cancel` fires when the user
    /// cancels.
    ///
    /// Returns the verb's [`VerbOutput`] — the description of effects
    /// the host will commit on accept.
    async fn invoke(
        &self,
        ctx: VerbContext,
        inputs: VerbInputs,
        progress: VerbProgress,
        cancel: CancellationToken,
    ) -> Result<VerbOutput>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::descriptor::{
        BackendCapabilities, CostEstimate, EffectKind, VerbDescriptor, VerbId,
    };
    use crate::plugin::error::VerbError;
    use crate::plugin::output::ActualCost;
    use std::time::Duration;

    struct AlwaysFails {
        descriptor: VerbDescriptor,
    }

    impl AlwaysFails {
        fn new() -> Self {
            Self {
                descriptor: VerbDescriptor {
                    id: VerbId::new("test.fails"),
                    display_name: "Fails".into(),
                    description: "Always fails".into(),
                    version: "0.0.0".into(),
                    required_capabilities: BackendCapabilities::empty(),
                    input_schema: serde_json::json!({}),
                    output_schema: None,
                    output_kinds: vec![EffectKind::AddLayer],
                    cost_estimate: CostEstimate::free(),
                    streaming: false,
                    cancellable: false,
                    documentation_url: None,
                },
            }
        }
    }

    #[async_trait]
    impl Verb for AlwaysFails {
        fn descriptor(&self) -> &VerbDescriptor {
            &self.descriptor
        }

        async fn invoke(
            &self,
            _ctx: VerbContext,
            _inputs: VerbInputs,
            _progress: VerbProgress,
            _cancel: CancellationToken,
        ) -> Result<VerbOutput> {
            Err(VerbError::Backend("always fails".into()))
        }
    }

    #[tokio::test]
    async fn verb_can_be_used_through_dyn_trait() {
        let verb: std::sync::Arc<dyn Verb> = std::sync::Arc::new(AlwaysFails::new());
        assert_eq!(verb.descriptor().id, VerbId::new("test.fails"));

        let ctx = VerbContext::empty(pixhaus_core::project::ProjectMetadata {
            name: "t".into(),
            description: None,
            author: None,
            created_at: 0,
            updated_at: 0,
            editor_version: "0".into(),
        });
        let res = verb
            .invoke(
                ctx,
                VerbInputs::empty(),
                VerbProgress::discard(),
                CancellationToken::new(),
            )
            .await;
        assert!(matches!(res, Err(VerbError::Backend(_))));
    }

    #[test]
    fn default_validate_accepts_anything() {
        let verb = AlwaysFails::new();
        let res = verb.validate(&VerbInputs::empty());
        assert!(res.is_ok());
    }

    // Ensures the trait is dyn-safe — won't compile if it isn't.
    fn _assert_dyn_safe(_v: &dyn Verb) {}

    // Suppress dead-code warnings about ActualCost in this test module
    // by referencing it; the default impl tests below need it to be
    // visible across the integration test boundary.
    #[allow(dead_code)]
    fn _ensure_actual_cost_is_visible() -> ActualCost {
        ActualCost::free(Duration::ZERO)
    }
}
