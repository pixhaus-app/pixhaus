//! Inference backend trait and discovery types.
//!
//! Every backend adapter (Anthropic, `OpenAI`, Replicate, Ollama,
//! `ComfyUI`, Stability — shipped by S22) implements [`InferenceBackend`].
//! The runtime uses the trait for two purposes:
//!
//! 1. **Capability checking.** Before invoking a verb, the runtime walks
//!    the registered backends in priority order and selects the first one
//!    whose [`InferenceBackend::capabilities`] satisfy the verb's
//!    [`super::descriptor::VerbDescriptor::required_capabilities`].
//!
//! 2. **Backend injection.** The selected backend is attached to the
//!    [`super::context::VerbContext`] handed to the verb. Verbs that need
//!    to call an inference service downcast the `Arc<dyn InferenceBackend>`
//!    to their concrete type (or a more specific sub-trait defined in their
//!    own crate).
//!
//! # Fallback chains
//!
//! Backends are registered with a priority (`u16`). Lower value = higher
//! priority. `select_backend` iterates in ascending priority order and
//! returns the first backend that is available *and* provides all of the
//! requested capabilities. The common idiom is:
//!
//! ```text
//! priority 0  Ollama (local, free, low latency, limited capabilities)
//! priority 10 Anthropic Claude (cloud, cost, high capability)
//! priority 20 OpenAI GPT-5 (fallback for anything Anthropic can't do)
//! ```
//!
//! An unavailable backend (e.g. Ollama is not running) is skipped and the
//! next candidate in the chain is tried.
//!
//! # Extending the trait
//!
//! `InferenceBackend` is intentionally thin. Verbs that need specific call
//! semantics (text generation, image editing, etc.) downcast `Arc<dyn
//! InferenceBackend>` to a more specific trait they define in their own
//! module. The core protocol does not enumerate every possible inference
//! method; S22 adapters implement whatever sub-traits their capabilities
//! advertise.
//!
//! New methods on `InferenceBackend` itself require a semver minor bump of
//! `pixhaus-ai`.

use serde::{Deserialize, Serialize};

use super::descriptor::{BackendCapabilities, CostEstimate};

/// Inference backend that an AI verb can call to do its work.
///
/// Every concrete backend (Anthropic, `OpenAI`, Replicate, Ollama,
/// `ComfyUI`, Stability) implements this trait. The runtime queries
/// [`Self::capabilities`] and [`Self::is_available`] before selecting
/// the backend for an invocation; verb code accesses the backend through
/// `ctx.backend` and typically downcasts to a more specific sub-trait.
///
/// # Object safety
///
/// `InferenceBackend` is object-safe: all methods take `&self` with no
/// generic parameters, so the runtime can store `Arc<dyn
/// InferenceBackend>`.
///
/// # Debug
///
/// Implementors must derive or implement [`std::fmt::Debug`] so
/// `VerbContext` can derive `Debug`. A minimal impl that prints the
/// backend's ID is usually sufficient.
pub trait InferenceBackend: std::fmt::Debug + Send + Sync + 'static {
    /// Stable machine-readable identifier for this backend instance.
    ///
    /// Recommended format: `"<provider>.<model>"` (e.g.
    /// `"anthropic.claude-sonnet-4-6"`, `"ollama.llama3-8b"`). The
    /// identifier is used in telemetry and in
    /// [`super::progress::VerbProgressEvent::Started::backend`].
    fn id(&self) -> &str;

    /// Human-readable name shown in the preferences UI and progress
    /// events. Defaults to `id()`.
    fn display_name(&self) -> &str {
        self.id()
    }

    /// The set of inference capabilities this backend can satisfy.
    ///
    /// The runtime ANDs the verb's `required_capabilities` with this
    /// set to decide whether the backend is eligible. Return
    /// [`BackendCapabilities::empty`] for a backend that cannot satisfy
    /// any AI capability (unusual but allowed).
    fn capabilities(&self) -> BackendCapabilities;

    /// Pre-invocation cost / latency estimate for the given capability
    /// set. Called by the UI before the user confirms "Run verb" to
    /// surface expected spend.
    ///
    /// `required` is the verb's full `required_capabilities` bitfield, not
    /// a subset. Implementations that cannot estimate the cost for some
    /// capabilities may return a conservative upper bound.
    fn cost_estimate(&self, required: BackendCapabilities) -> CostEstimate;

    /// Returns `true` if this backend is currently reachable.
    ///
    /// The runtime calls this during backend selection to skip backends
    /// that are configured but not running (e.g. Ollama is installed
    /// but the process is not started). Implementations may make a
    /// lightweight health-check call or check a cached flag; they must
    /// not block for more than a few milliseconds. Default: always
    /// `true`.
    fn is_available(&self) -> bool {
        true
    }
}

/// Snapshot of a registered backend's metadata, without the `Arc<dyn
/// InferenceBackend>` handle.
///
/// Returned by [`super::runtime::VerbRuntime::list_backends`] so the
/// UI can render the backend list and preference panel without holding
/// a reference into the runtime.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BackendInfo {
    /// Stable identifier — mirrors [`InferenceBackend::id`].
    pub id: String,
    /// Human-readable name — mirrors [`InferenceBackend::display_name`].
    pub display_name: String,
    /// Capabilities this backend advertises.
    pub capabilities: BackendCapabilities,
    /// Snapshot of [`InferenceBackend::is_available`] at list time.
    pub available: bool,
    /// Registration priority. Lower value = tried first in the
    /// fallback chain.
    pub priority: u16,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::descriptor::CostEstimate;
    use std::time::Duration;

    #[derive(Debug)]
    struct AlwaysOn {
        id: &'static str,
        caps: BackendCapabilities,
    }

    impl InferenceBackend for AlwaysOn {
        fn id(&self) -> &str {
            self.id
        }
        fn capabilities(&self) -> BackendCapabilities {
            self.caps
        }
        fn cost_estimate(&self, _required: BackendCapabilities) -> CostEstimate {
            CostEstimate {
                typical_latency: Duration::from_millis(100),
                max_latency: Duration::from_secs(5),
                typical_usd_cents: 0.1,
                max_usd_cents: 1.0,
            }
        }
    }

    #[test]
    fn backend_info_round_trips_as_json() {
        let info = BackendInfo {
            id: "anthropic.claude-sonnet-4-6".into(),
            display_name: "Anthropic Claude Sonnet".into(),
            capabilities: BackendCapabilities::TEXT_GENERATION
                .union(BackendCapabilities::VISION_LANGUAGE),
            available: true,
            priority: 10,
        };
        let json = serde_json::to_string(&info).unwrap();
        let back: BackendInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(back, info);
    }

    #[test]
    fn default_display_name_falls_back_to_id() {
        let b = AlwaysOn {
            id: "ollama.llama3",
            caps: BackendCapabilities::TEXT_GENERATION,
        };
        assert_eq!(b.display_name(), b.id());
    }

    #[test]
    fn default_is_available_returns_true() {
        let b = AlwaysOn {
            id: "test",
            caps: BackendCapabilities::empty(),
        };
        assert!(b.is_available());
    }
}
