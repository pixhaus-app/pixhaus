//! Error type for the verb plugin protocol.
//!
//! `VerbError` is the closed set of failure modes that flow through the
//! runtime. Verbs return this from [`super::verb::Verb::invoke`]; the
//! runtime maps its own internal failures (lookup miss, double-register)
//! onto the same enum. Crossing into the `app` crate, errors are wrapped
//! by `anyhow` so the IPC bridge sees a single string chain.

use thiserror::Error;

use super::descriptor::VerbId;

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
    /// requires. `required` is the full bitfield from the descriptor.
    #[error("no backend satisfies capabilities {required:#010x} required by verb `{verb}`")]
    UnsupportedCapability {
        /// The verb whose descriptor declared the required capabilities.
        verb: VerbId,
        /// The unsatisfied capability bitfield.
        required: u32,
    },

    /// A backend that was selected for the verb reported it is not
    /// currently reachable (e.g. Ollama process is down).
    #[error("backend `{id}` is not available")]
    BackendUnavailable {
        /// Identifier of the backend that is down.
        id: String,
    },

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
        assert_eq!(
            err.to_string(),
            "no verb registered with id `pixhaus.builtin.echo`"
        );
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
    fn unsupported_capability_renders_bits() {
        let err = VerbError::UnsupportedCapability {
            verb: VerbId::new("pixhaus.builtin.inbetween"),
            required: 0x0000_0024, // IMAGE_GENERATION | FRAME_INTERPOLATION
        };
        let msg = err.to_string();
        assert!(msg.contains("0x00000024"));
        assert!(msg.contains("inbetween"));
    }

    #[test]
    fn backend_unavailable_renders_id() {
        let err = VerbError::BackendUnavailable {
            id: "ollama.llama3".into(),
        };
        assert_eq!(err.to_string(), "backend `ollama.llama3` is not available");
    }
}
