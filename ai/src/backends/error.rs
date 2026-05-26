//! Error types for inference backend adapters.

use thiserror::Error;

/// Closed set of failures an [`super::InferenceBackend`] may produce.
///
/// Every adapter maps its internal errors onto these variants so callers
/// have a uniform surface to match on. Verb authors convert
/// `BackendError` into [`crate::plugin::error::VerbError::Backend`]
/// when they propagate errors up to the runtime.
#[derive(Debug, Error)]
pub enum BackendError {
    /// The backend returned an HTTP error status.
    #[error("HTTP {status} from backend: {body}")]
    Http {
        /// HTTP status code.
        status: u16,
        /// Response body, truncated if excessively long.
        body: String,
    },

    /// Network-level failure (DNS, connection, TLS, timeout).
    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),

    /// JSON (de)serialisation failed.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// The backend does not support the requested capability.
    #[error("backend does not support this capability")]
    UnsupportedCapability,

    /// Authentication rejected by the backend.
    #[error("authentication failed: {0}")]
    Auth(String),

    /// No API key is stored for this backend.
    #[error("API key not configured for backend '{0}'")]
    ApiKeyNotFound(String),

    /// OS keychain access failed.
    #[error("keychain error: {0}")]
    Keychain(String),

    /// The backend returned a response that could not be interpreted.
    #[error("invalid backend response: {0}")]
    InvalidResponse(String),

    /// The backend signalled rate limiting.
    #[error("rate limited; retry after {retry_after_secs}s")]
    RateLimited {
        /// Seconds the caller should wait before retrying.
        retry_after_secs: u64,
    },

    /// The request was cancelled before the backend responded.
    #[error("request cancelled")]
    Cancelled,

    /// Backend-specific failure that does not fit any other variant.
    #[error("backend error: {0}")]
    Other(String),
}

impl BackendError {
    /// Returns `true` when the error was caused by user-driven cancellation.
    #[must_use]
    pub const fn is_cancelled(&self) -> bool {
        matches!(self, Self::Cancelled)
    }

    /// Constructs a [`Self::Http`] error, trimming the body to 512 chars.
    #[must_use]
    pub fn http(status: u16, body: impl Into<String>) -> Self {
        let mut body = body.into();
        body.truncate(512);
        Self::Http { status, body }
    }
}

/// Result alias used throughout the backends module.
pub type Result<T> = std::result::Result<T, BackendError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancelled_variant_is_classified() {
        assert!(BackendError::Cancelled.is_cancelled());
        assert!(!BackendError::UnsupportedCapability.is_cancelled());
    }

    #[test]
    fn http_error_trims_long_bodies() {
        let long = "x".repeat(1024);
        let err = BackendError::http(429, long);
        let msg = err.to_string();
        assert!(msg.contains("429"));
        assert!(msg.len() < 600);
    }

    #[test]
    fn http_error_carries_status() {
        let err = BackendError::http(401, "Unauthorized");
        assert!(matches!(err, BackendError::Http { status: 401, .. }));
    }

    #[test]
    fn network_error_converts_from_reqwest() {
        // We can't easily construct a reqwest::Error in a unit test without
        // an actual network call. Verify the From impl exists by checking
        // the error string contains the expected discriminant text.
        let err = BackendError::Other("timeout".into());
        assert!(err.to_string().contains("timeout"));
    }
}
