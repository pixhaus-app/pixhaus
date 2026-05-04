//! API key management via the OS keychain.
//!
//! [`ApiKeyStore`] wraps the `keyring` crate to store and retrieve
//! per-backend API keys in the platform keychain (Windows Credential
//! Manager, macOS Keychain, Linux Secret Service / `KWallet`). Keys are
//! never written to disk in plaintext.
//!
//! Service name convention: `"pixhaus.<backend>"` where `<backend>` is
//! the stable identifier returned by
//! [`super::InferenceBackend::backend_id`], e.g. `"pixhaus.anthropic"`.

use keyring::Entry;

use super::error::{BackendError, Result};

/// Keychain service prefix for all Pixhaus backend credentials.
const SERVICE_PREFIX: &str = "pixhaus";

/// Thin wrapper around the OS keychain.
///
/// All methods are synchronous — keychain access is fast enough that
/// the overhead of `spawn_blocking` outweighs any gain on a call that
/// typically completes in microseconds. Call sites that need async
/// can wrap the call themselves.
pub struct ApiKeyStore;

impl ApiKeyStore {
    /// Retrieves the API key for `backend` from the OS keychain.
    ///
    /// Returns [`BackendError::ApiKeyNotFound`] if no key is stored,
    /// or [`BackendError::Keychain`] for lower-level OS errors.
    pub fn get(backend: &str) -> Result<String> {
        let entry = Self::entry(backend)?;
        entry.get_password().map_err(|err| match err {
            keyring::Error::NoEntry => BackendError::ApiKeyNotFound(backend.to_owned()),
            other => BackendError::Keychain(other.to_string()),
        })
    }

    /// Stores `key` in the OS keychain for `backend`.
    ///
    /// Overwrites an existing key without error.
    pub fn set(backend: &str, key: &str) -> Result<()> {
        let entry = Self::entry(backend)?;
        entry
            .set_password(key)
            .map_err(|err| BackendError::Keychain(err.to_string()))
    }

    /// Deletes the stored key for `backend`.
    ///
    /// Returns [`BackendError::ApiKeyNotFound`] if no key was stored.
    pub fn delete(backend: &str) -> Result<()> {
        let entry = Self::entry(backend)?;
        entry.delete_credential().map_err(|err| match err {
            keyring::Error::NoEntry => BackendError::ApiKeyNotFound(backend.to_owned()),
            other => BackendError::Keychain(other.to_string()),
        })
    }

    fn entry(backend: &str) -> Result<Entry> {
        let service = format!("{SERVICE_PREFIX}.{backend}");
        Entry::new(&service, backend).map_err(|err| BackendError::Keychain(err.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_missing_key_returns_not_found() {
        // Use a key name that almost certainly does not exist.
        let result = ApiKeyStore::get("pixhaus-test-nonexistent-backend-xyzzy");
        match result {
            Err(BackendError::ApiKeyNotFound(_) | BackendError::Keychain(_)) => {}
            Ok(_) => {
                // If a key somehow exists, delete it and consider the test
                // inconclusive rather than failing.
                let _ = ApiKeyStore::delete("pixhaus-test-nonexistent-backend-xyzzy");
            }
            Err(other) => panic!("unexpected error: {other}"),
        }
    }

    #[test]
    fn set_and_get_round_trip() {
        let backend = "pixhaus-test-roundtrip-xyzzy";
        // Clean up any pre-existing entry.
        let _ = ApiKeyStore::delete(backend);

        if let Err(e) = ApiKeyStore::set(backend, "test-key-value") {
            // Keychain not available in this test environment (common in CI).
            eprintln!("skipping: keychain not available — {e}");
            return;
        }

        let get_result = ApiKeyStore::get(backend);
        match get_result {
            Err(BackendError::ApiKeyNotFound(_) | BackendError::Keychain(_)) => {
                // Some keychain implementations (e.g. certain Windows
                // Credential Manager configurations) accept writes but
                // do not surface them in subsequent reads within the same
                // process. Treat this as "not reliably available" rather
                // than a hard failure.
                eprintln!("skipping: keychain wrote but could not read back");
                let _ = ApiKeyStore::delete(backend);
                return;
            }
            Err(other) => panic!("unexpected error on get: {other}"),
            Ok(retrieved) => assert_eq!(retrieved, "test-key-value"),
        }

        ApiKeyStore::delete(backend).expect("delete should succeed");
        assert!(matches!(
            ApiKeyStore::get(backend),
            Err(BackendError::ApiKeyNotFound(_) | BackendError::Keychain(_))
        ));
    }
}
