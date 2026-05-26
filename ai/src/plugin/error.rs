//! Error type for the verb plugin protocol.
//!
//! `VerbError` is the closed set of failure modes that flow through the
//! runtime. Verbs return this from [`super::verb::Verb::invoke`]; the
//! runtime maps its own internal failures (lookup miss, double-register)
//! onto the same enum. Crossing into the `app` crate, errors are wrapped
//! by `anyhow` so the IPC bridge sees a single string chain.

use thiserror::Error;

use super::descriptor::{BackendCapabilities, VerbId};

/// Closed set of verb-protocol failures.
///
/// Each variant is shaped to be actionable on the UI side: lookup
/// misses get the offending `VerbId` so the UI can show "no such verb"
/// without parsing a string; schema failures carry the parser message
/// for inline form validation.
#[derive(Debug, Error)]
pub enum VerbError {
    /// No verb is registered with the supplied ID.
    #[error("no verb registered with id `{0}`")]
    NotFound(VerbId),

    /// A verb with this ID is already registered.
    #[error("verb `{0}` is already registered")]
    AlreadyRegistered(VerbId),

    /// Inputs failed schema validation.
    #[error("schema validation failed: {0}")]
    Schema(String),

    /// The verb requires an active sprite, layer, or frame that the
    /// caller did not provide.
    #[error("missing required context: {0}")]
    MissingContext(&'static str),

    /// No registered backend satisfies all of the capabilities the verb
    /// requires. `required` is the full capability set from the descriptor.
    ///
    /// Distinct from [`Self::BackendUnavailable`]: this variant means
    /// no backend in the registry advertises the capabilities at all.
    /// `BackendUnavailable` means a matching backend exists but is
    /// down; the UI surfaces them differently (the former asks the
    /// user to configure a backend, the latter to start one).
    #[error("no backend satisfies capabilities [{required}] required by verb `{verb}`")]
    UnsupportedCapability {
        /// The verb whose descriptor declared the required capabilities.
        verb: VerbId,
        /// The unsatisfied capability set, rendered by name in the
        /// error message via [`BackendCapabilities`]'s `Display` impl.
        required: BackendCapabilities,
    },

    /// One or more backends advertise the required capabilities but
    /// none are currently reachable (e.g. Ollama process is down,
    /// API key revoked). The string is the id of the highest-priority
    /// matching backend.
    #[error("backend `{id}` advertises the required capabilities but is not currently available")]
    BackendUnavailable {
        /// Identifier of the backend that matched but is down.
        id: String,
    },

    /// A backend with this id is already registered.
    ///
    /// Distinct from [`Self::AlreadyRegistered`] (which is for verbs)
    /// so backend-management UIs surface the right object type.
    #[error("backend `{0}` is already registered")]
    BackendAlreadyRegistered(String),

    /// No backend with this id is registered.
    ///
    /// Distinct from [`Self::NotFound`] (which is for verbs) so
    /// backend-management UIs surface the right object type.
    #[error("no backend registered with id `{0}`")]
    BackendNotFound(String),

    /// The invocation was cancelled before producing a preview.
    #[error("verb invocation was cancelled")]
    Cancelled,

    /// The verb-side worker panicked or was aborted by the executor.
    #[error("verb worker terminated abnormally: {0}")]
    Aborted(String),

    /// The backend or the verb itself returned an error message that
    /// does not fit any other variant.
    #[error("backend error: {0}")]
    Backend(String),

    /// Wraps a JSON (de)serialization failure when packaging
    /// `VerbInputs` or `VerbOutput`.
    #[error("payload serialization failed: {0}")]
    Payload(#[from] serde_json::Error),
}

impl VerbError {
    /// Returns `true` if the failure was caused by user-driven
    /// cancellation. The UI uses this to suppress error toasts for the
    /// "I clicked cancel" path.
    #[must_use]
    pub const fn is_cancelled(&self) -> bool {
        matches!(self, Self::Cancelled)
    }
}

/// Result alias used throughout the plugin protocol.
pub type Result<T> = std::result::Result<T, VerbError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn not_found_renders_id() {
        let err = VerbError::NotFound(VerbId::new("pixhaus.builtin.echo"));
        assert_eq!(err.to_string(), "no verb registered with id `pixhaus.builtin.echo`");
    }

    #[test]
    fn cancelled_is_classified() {
        assert!(VerbError::Cancelled.is_cancelled());
        assert!(!VerbError::NotFound(VerbId::new("x")).is_cancelled());
    }

    #[test]
    fn json_error_converts() {
        let bad: serde_json::Error = serde_json::from_str::<u32>("nope").unwrap_err();
        let err: VerbError = bad.into();
        assert!(matches!(err, VerbError::Payload(_)));
    }

    #[test]
    fn unsupported_capability_renders_named_bits() {
        // Regression for thread 3: prior message rendered the
        // capability mask as `0x00000024`. Users couldn't tell which
        // capabilities were missing. Now the names appear directly.
        let err = VerbError::UnsupportedCapability {
            verb: VerbId::new("pixhaus.builtin.inbetween"),
            required: BackendCapabilities::IMAGE_GENERATION.union(BackendCapabilities::FRAME_INTERPOLATION),
        };
        let msg = err.to_string();
        assert!(msg.contains("IMAGE_GENERATION"), "msg = {msg}");
        assert!(msg.contains("FRAME_INTERPOLATION"), "msg = {msg}");
        assert!(!msg.contains("0x"), "named output must replace the hex bitfield: {msg}");
        assert!(msg.contains("inbetween"));
    }

    #[test]
    fn backend_unavailable_renders_id() {
        let err = VerbError::BackendUnavailable { id: "ollama.llama3".into() };
        assert!(err.to_string().contains("ollama.llama3"));
    }

    #[test]
    fn backend_already_registered_renders_id() {
        let err = VerbError::BackendAlreadyRegistered("anthropic.claude".into());
        assert_eq!(err.to_string(), "backend `anthropic.claude` is already registered");
    }

    #[test]
    fn backend_not_found_renders_id() {
        let err = VerbError::BackendNotFound("ollama.llama3".into());
        assert_eq!(err.to_string(), "no backend registered with id `ollama.llama3`");
    }
}
