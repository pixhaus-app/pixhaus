//! Verb runtime: registry, dispatch, backend selection, preview/commit
//! lifecycle.
//!
//! The runtime splits into two cooperating types across submodules:
//!
//! - [`VerbRuntime`] (in the `registry` submodule) is the host-side
//!   coordinator. It owns the verb registry (`Arc<dyn Verb>` by ID), the
//!   priority-ordered backend registry, and the preview ID minter, and
//!   exposes `invoke`.
//! - [`VerbInvocation`] (in the `invocation` submodule) is the `'static`
//!   handle the caller drives to completion: cancellation, progress
//!   draining, and the join-handle state machine.
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
//! There is **no** pending-preview map. [`VerbPreview`](super::preview::VerbPreview)s
//! are values the caller carries between `finish` and `commit` /
//! `discard`. Handing the value back to the caller sidesteps the whole
//! "what if the dialog closes?" class of bugs.
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
//!    [`VerbError`](super::error::VerbError)`::UnsupportedCapability`.
//!
//! The selected backend is attached to the `VerbContext` before it
//! reaches the verb. Verbs downcast `ctx.backend` to a more specific
//! sub-trait when they need to make inference calls.

mod invocation;
mod registry;

pub use invocation::VerbInvocation;
pub use registry::VerbRuntime;

#[cfg(test)]
mod tests {
    use super::VerbRuntime;
    use crate::plugin::backend::InferenceBackend;
    use crate::plugin::context::VerbContext;
    use crate::plugin::descriptor::{BackendCapabilities, CostEstimate, EffectKind, VerbDescriptor, VerbId};
    use crate::plugin::error::{Result, VerbError};
    use crate::plugin::inputs::VerbInputs;
    use crate::plugin::output::{ActualCost, VerbOutput};
    use crate::plugin::preview::{PreviewId, VerbPreview};
    use crate::plugin::progress::{VerbProgress, VerbProgressEvent};
    use crate::plugin::verb::Verb;
    use async_trait::async_trait;
    use pixhaus_core::project::ProjectMetadata;
    use std::time::Duration;
    use tokio_util::sync::CancellationToken;

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
        async fn invoke(&self, _ctx: VerbContext, _inputs: VerbInputs, progress: VerbProgress, _cancel: CancellationToken) -> Result<VerbOutput> {
            progress.send(VerbProgressEvent::Started { backend: None }).await;
            Ok(empty_output(Duration::ZERO))
        }
    }

    struct ObservesCancellation(VerbDescriptor);
    #[async_trait]
    impl Verb for ObservesCancellation {
        fn descriptor(&self) -> &VerbDescriptor {
            &self.0
        }
        async fn invoke(&self, _ctx: VerbContext, _inputs: VerbInputs, _progress: VerbProgress, cancel: CancellationToken) -> Result<VerbOutput> {
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
        async fn invoke(&self, _ctx: VerbContext, _inputs: VerbInputs, _progress: VerbProgress, _cancel: CancellationToken) -> Result<VerbOutput> {
            Ok(empty_output(Duration::ZERO))
        }
    }

    #[tokio::test]
    async fn register_and_invoke_round_trip() {
        let runtime = VerbRuntime::new();
        runtime.register(ImmediateOk(descriptor("test.ok", true, true))).unwrap();

        assert_eq!(runtime.len(), 1);
        assert!(!runtime.is_empty());
        let descs = runtime.list();
        assert_eq!(descs.len(), 1);
        assert_eq!(descs[0].id.as_str(), "test.ok");

        let mut inv = runtime
            .invoke(&VerbId::new("test.ok"), VerbContext::empty(metadata()), VerbInputs::empty())
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
        runtime.register(ImmediateOk(descriptor("test.dup", true, false))).unwrap();
        let err = runtime.register(ImmediateOk(descriptor("test.dup", true, false))).unwrap_err();
        assert!(matches!(err, VerbError::AlreadyRegistered(_)));
    }

    #[tokio::test]
    async fn unknown_verb_returns_not_found() {
        let runtime = VerbRuntime::new();
        let err = runtime
            .invoke(&VerbId::new("test.missing"), VerbContext::empty(metadata()), VerbInputs::empty())
            .unwrap_err();
        assert!(matches!(err, VerbError::NotFound(_)));
    }

    #[tokio::test]
    async fn validate_failure_propagates() {
        let runtime = VerbRuntime::new();
        runtime.register(Validates(descriptor("test.validates", true, false))).unwrap();
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
        runtime.register(ObservesCancellation(descriptor("test.cancel", true, false))).unwrap();
        let inv = runtime
            .invoke(&VerbId::new("test.cancel"), VerbContext::empty(metadata()), VerbInputs::empty())
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
            async fn invoke(&self, _ctx: VerbContext, _inputs: VerbInputs, _progress: VerbProgress, cancel: CancellationToken) -> Result<VerbOutput> {
                cancel.cancel();
                Ok(empty_output(Duration::ZERO))
            }
        }

        let runtime = VerbRuntime::new();
        runtime.register(OkButCancels(descriptor("test.ok-cancels", true, false))).unwrap();
        let inv = runtime
            .invoke(&VerbId::new("test.ok-cancels"), VerbContext::empty(metadata()), VerbInputs::empty())
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
            async fn invoke(&self, _ctx: VerbContext, _inputs: VerbInputs, _progress: VerbProgress, cancel: CancellationToken) -> Result<VerbOutput> {
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
                .invoke(&VerbId::new("test.drop-cancels"), VerbContext::empty(metadata()), VerbInputs::empty())
                .unwrap();
            // Drop at end of scope; Drop impl fires the cancel token.
        }

        // Worker observes cancellation and sets `observed`. Poll briefly
        // with a deadline rather than a fixed sleep.
        let deadline = std::time::Instant::now() + Duration::from_secs(1);
        while !observed.load(Ordering::SeqCst) && std::time::Instant::now() < deadline {
            tokio::task::yield_now().await;
        }
        assert!(observed.load(Ordering::SeqCst), "drop did not cancel the in-flight verb");
    }

    #[tokio::test]
    async fn abandoned_finish_future_cancels_worker() {
        // A `finish()` future dropped mid-await (e.g. a timeout) must still
        // cancel the worker. `finish` takes the join handle out before
        // awaiting, so this only holds because Drop keys off the `finished`
        // flag rather than `join.is_some()`.
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
            async fn invoke(&self, _ctx: VerbContext, _inputs: VerbInputs, _progress: VerbProgress, cancel: CancellationToken) -> Result<VerbOutput> {
                cancel.cancelled().await;
                self.observed.store(true, Ordering::SeqCst);
                Err(VerbError::Cancelled)
            }
        }

        let observed = Arc::new(AtomicBool::new(false));
        let runtime = VerbRuntime::new();
        runtime
            .register(SignalsOnCancel {
                descriptor: descriptor("test.abandon-finish", true, false),
                observed: observed.clone(),
            })
            .unwrap();

        let inv = runtime
            .invoke(&VerbId::new("test.abandon-finish"), VerbContext::empty(metadata()), VerbInputs::empty())
            .unwrap();

        // finish() hangs because the worker waits on cancellation. Time out
        // so the finish future is dropped mid-await.
        let timed_out = tokio::time::timeout(Duration::from_millis(50), inv.finish()).await;
        assert!(timed_out.is_err(), "finish should not have completed");

        let deadline = std::time::Instant::now() + Duration::from_secs(1);
        while !observed.load(Ordering::SeqCst) && std::time::Instant::now() < deadline {
            tokio::task::yield_now().await;
        }
        assert!(observed.load(Ordering::SeqCst), "abandoning the finish future must cancel the worker");
    }

    #[tokio::test]
    async fn unregister_removes_verb() {
        let runtime = VerbRuntime::new();
        runtime.register(ImmediateOk(descriptor("test.un", true, false))).unwrap();
        runtime.unregister(&VerbId::new("test.un")).unwrap();
        assert!(runtime.is_empty());
        let err = runtime.unregister(&VerbId::new("test.un")).unwrap_err();
        assert!(matches!(err, VerbError::NotFound(_)));
    }

    // ── Backend registry tests ───────────────────────────────────────────────

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
        StubBackend { id, caps, available: true }
    }

    #[test]
    fn register_and_list_backends() {
        let rt = VerbRuntime::new();
        rt.register_backend(caps_backend("local", BackendCapabilities::TEXT_GENERATION), 0).unwrap();
        rt.register_backend(caps_backend("cloud", BackendCapabilities::IMAGE_GENERATION), 10).unwrap();

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
        rt.register_backend(caps_backend("b", BackendCapabilities::empty()), 0).unwrap();
        let err = rt.register_backend(caps_backend("b", BackendCapabilities::empty()), 1).unwrap_err();
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
        rt.register_backend(caps_backend("b", BackendCapabilities::empty()), 0).unwrap();
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
        rt.register_backend(caps_backend("text-only", BackendCapabilities::TEXT_GENERATION), 0).unwrap();
        rt.register_backend(
            caps_backend("multimodal", BackendCapabilities::TEXT_GENERATION.union(BackendCapabilities::IMAGE_GENERATION)),
            10,
        )
        .unwrap();

        // text-only is tried first and satisfies TEXT_GENERATION.
        let b = rt.select_backend(BackendCapabilities::TEXT_GENERATION, &VerbId::new("v")).unwrap();
        assert_eq!(b.id(), "text-only");

        // IMAGE_GENERATION only available in multimodal.
        let b = rt.select_backend(BackendCapabilities::IMAGE_GENERATION, &VerbId::new("v")).unwrap();
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

        let b = rt.select_backend(BackendCapabilities::TEXT_GENERATION, &VerbId::new("v")).unwrap();
        assert_eq!(b.id(), "up");
    }

    #[test]
    fn select_backend_returns_unsupported_when_none_match() {
        let rt = VerbRuntime::new();
        rt.register_backend(caps_backend("text-only", BackendCapabilities::TEXT_GENERATION), 0).unwrap();

        let err = rt.select_backend(BackendCapabilities::IMAGE_GENERATION, &VerbId::new("v")).unwrap_err();
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

        let err = rt.select_backend(BackendCapabilities::TEXT_GENERATION, &VerbId::new("v")).unwrap_err();
        assert!(
            matches!(&err, VerbError::BackendUnavailable { id } if id == "ollama"),
            "expected BackendUnavailable, got {err:?}"
        );
    }

    #[test]
    fn select_backend_by_id_returns_named_backend() {
        let rt = VerbRuntime::new();
        rt.register_backend(caps_backend("google", BackendCapabilities::IMAGE_GENERATION), 0).unwrap();

        let backend = rt
            .select_backend_by_id("google", BackendCapabilities::IMAGE_GENERATION, &VerbId::new("v"))
            .unwrap();

        assert_eq!(backend.id(), "google");
    }

    #[test]
    fn select_backend_by_id_rejects_unknown_id() {
        let rt = VerbRuntime::new();

        let err = rt
            .select_backend_by_id("missing", BackendCapabilities::IMAGE_GENERATION, &VerbId::new("v"))
            .unwrap_err();

        assert!(
            matches!(&err, VerbError::BackendNotFound(id) if id == "missing"),
            "expected BackendNotFound, got {err:?}"
        );
    }

    #[test]
    fn select_backend_by_id_checks_capabilities() {
        let rt = VerbRuntime::new();
        rt.register_backend(caps_backend("text-only", BackendCapabilities::TEXT_GENERATION), 0).unwrap();

        let err = rt
            .select_backend_by_id("text-only", BackendCapabilities::IMAGE_GENERATION, &VerbId::new("v"))
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
            .select_backend_by_id("down", BackendCapabilities::IMAGE_GENERATION, &VerbId::new("v"))
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
            async fn invoke(&self, ctx: VerbContext, _inputs: VerbInputs, _progress: VerbProgress, _cancel: CancellationToken) -> Result<VerbOutput> {
                self.saw_backend.store(ctx.backend.is_some(), Ordering::SeqCst);
                Ok(empty_output(Duration::ZERO))
            }
        }

        let saw = Arc::new(AtomicBool::new(false));
        let rt = VerbRuntime::new();
        rt.register_backend(caps_backend("text", BackendCapabilities::TEXT_GENERATION), 0).unwrap();

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
            .invoke(&VerbId::new("test.needs-backend"), VerbContext::empty(metadata()), VerbInputs::empty())
            .unwrap();
        inv.finish().await.unwrap();

        assert!(saw.load(Ordering::SeqCst), "verb did not receive a backend in ctx");
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
            async fn invoke(&self, _ctx: VerbContext, _inputs: VerbInputs, _progress: VerbProgress, _cancel: CancellationToken) -> Result<VerbOutput> {
                Ok(empty_output(Duration::ZERO))
            }
        }

        let rt = VerbRuntime::new();
        rt.register_backend(caps_backend("text-only", BackendCapabilities::TEXT_GENERATION), 0).unwrap();

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
            .invoke(&VerbId::new("test.img"), VerbContext::empty(metadata()), VerbInputs::empty())
            .unwrap_err();
        assert!(
            matches!(err, VerbError::UnsupportedCapability { .. }),
            "expected UnsupportedCapability, got {err:?}"
        );
    }
}
