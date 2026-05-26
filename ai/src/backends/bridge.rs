//! [`BackendProxy`]: bridge between [`crate::backends::InferenceBackend`]
//! (the fat async trait S22 adapters implement) and
//! [`crate::plugin::backend::InferenceBackend`] (the thin trait the verb
//! runtime stores).
//!
//! # Why two traits?
//!
//! The verb runtime (`plugin::runtime`) works with a thin, object-safe,
//! sync trait for capability checking and backend selection. The S22
//! inference adapters implement a separate async trait with the actual
//! `invoke` method. `BackendProxy` is the glue: it wraps any S22 adapter
//! in a single concrete type that:
//!
//! - satisfies `plugin::backend::InferenceBackend` so the runtime can
//!   register and select it;
//! - exposes the underlying adapter via [`BackendProxy::fat`] so verbs
//!   can call `invoke` after downcasting through `as_any`.
//!
//! # Usage in a verb
//!
//! ```ignore
//! let proxy = ctx.backend
//!     .as_ref()
//!     .and_then(|b| b.as_any().downcast_ref::<BackendProxy>())
//!     .ok_or(VerbError::Backend("expected BackendProxy".into()))?;
//!
//! let response = proxy.fat()
//!     .invoke(InferenceRequest::FrameInterpolation(req), progress, cancel)
//!     .await
//!     .map_err(|e| VerbError::Backend(e.to_string()))?;
//! ```
//!
//! # Registration in the app
//!
//! The `app` crate (or integration tests) wraps S22 adapters before
//! registering them with the runtime:
//!
//! ```ignore
//! let proxy = BackendProxy::new(ReplicateBackend::from_keychain()?);
//! runtime.register_backend(proxy, 10)?;
//! ```

use std::any::Any;
use std::fmt;
use std::sync::Arc;

use crate::backends::InferenceBackend as FatBackend;
use crate::plugin::backend::InferenceBackend as ThinBackend;
use crate::plugin::descriptor::{BackendCapabilities, CostEstimate};

/// Wraps a fat [`FatBackend`] so it can be stored in the verb runtime.
///
/// The verb runtime registers backends as `Arc<dyn ThinBackend>`. This
/// proxy implements `ThinBackend` by delegating identity and capability
/// queries to the inner fat backend. Verbs downcast to `BackendProxy`
/// via `as_any()` and then call [`BackendProxy::fat`] to reach the
/// actual inference surface.
pub struct BackendProxy {
    inner: Arc<dyn FatBackend>,
    cached_id: &'static str,
    cached_caps: BackendCapabilities,
}

impl BackendProxy {
    /// Wraps a fat backend in a proxy. The `backend_id` and
    /// `capabilities` are captured at construction time so calls into
    /// the thin trait never touch async code.
    pub fn new(backend: impl FatBackend) -> Self {
        let cached_id = backend.backend_id();
        let cached_caps = backend.capabilities();
        Self {
            inner: Arc::new(backend),
            cached_id,
            cached_caps,
        }
    }

    /// Returns a reference to the underlying fat backend.
    ///
    /// Callers invoke `fat().invoke(InferenceRequest::...)` to make
    /// actual inference calls. Use the cancellation token and progress
    /// handle from the enclosing `VerbInvocation`.
    #[must_use]
    pub fn fat(&self) -> &dyn FatBackend {
        self.inner.as_ref()
    }
}

impl fmt::Debug for BackendProxy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BackendProxy")
            .field("id", &self.cached_id)
            .finish_non_exhaustive()
    }
}

impl ThinBackend for BackendProxy {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn id(&self) -> &'static str {
        self.cached_id
    }

    fn capabilities(&self) -> BackendCapabilities {
        self.cached_caps
    }

    fn cost_estimate(&self, _required: BackendCapabilities) -> CostEstimate {
        // Per-request cost estimation lives on the fat backend and
        // requires an `InferenceRequest` to compute. The thin trait only
        // gets the capability set, not the full request, so we return a
        // zero estimate here. The verb's `VerbDescriptor::cost_estimate`
        // carries the user-visible estimate; the fat backend's
        // `estimate_cost` is queried during invocation and surfaced via
        // `ActualCost`.
        CostEstimate::free()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backends::{
        BackendError, InferenceRequest, InferenceResponse, Result, VerbProgress,
    };
    use async_trait::async_trait;
    use tokio_util::sync::CancellationToken;

    #[derive(Debug)]
    struct FakeBackend {
        id: &'static str,
        caps: BackendCapabilities,
    }

    #[async_trait]
    impl FatBackend for FakeBackend {
        fn backend_id(&self) -> &'static str {
            self.id
        }
        fn capabilities(&self) -> BackendCapabilities {
            self.caps
        }
        fn supports_streaming(&self) -> bool {
            false
        }
        fn estimate_cost(
            &self,
            _req: &InferenceRequest,
        ) -> crate::plugin::descriptor::CostEstimate {
            CostEstimate::free()
        }
        async fn invoke(
            &self,
            _req: InferenceRequest,
            _progress: VerbProgress,
            _cancel: CancellationToken,
        ) -> Result<InferenceResponse> {
            Err(BackendError::UnsupportedCapability)
        }
    }

    #[test]
    fn proxy_delegates_id_and_caps() {
        let proxy = BackendProxy::new(FakeBackend {
            id: "test.fake",
            caps: BackendCapabilities::FRAME_INTERPOLATION,
        });
        assert_eq!(proxy.id(), "test.fake");
        assert!(
            proxy
                .capabilities()
                .contains(BackendCapabilities::FRAME_INTERPOLATION)
        );
    }

    #[test]
    fn proxy_as_any_downcasts_to_self() {
        let proxy = BackendProxy::new(FakeBackend {
            id: "t",
            caps: BackendCapabilities::empty(),
        });
        let thin: &dyn ThinBackend = &proxy;
        let recovered = thin.as_any().downcast_ref::<BackendProxy>();
        assert!(recovered.is_some());
    }

    #[test]
    fn proxy_fat_returns_inner() {
        let proxy = BackendProxy::new(FakeBackend {
            id: "t",
            caps: BackendCapabilities::IMAGE_GENERATION,
        });
        assert_eq!(proxy.fat().backend_id(), "t");
    }
}
