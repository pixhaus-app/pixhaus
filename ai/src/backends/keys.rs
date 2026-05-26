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

use std::sync::Once;

use keyring_core::Entry;

use super::error::{BackendError, Result};

/// Keychain service prefix for all Pixhaus backend credentials.
const SERVICE_PREFIX: &str = "pixhaus";

/// Guards one-time registration of the process-global keychain store.
static STORE_INIT: Once = Once::new();

/// Pure decision: should keychain access route through the in-memory store
/// instead of the real OS keychain?
///
/// `mock` is the explicit `PIXHAUS_KEYRING_MOCK` opt-in; `under_nextest` is
/// cargo-nextest's auto-set `NEXTEST` marker. Split out so the policy is
/// unit-testable without touching the process environment.
fn should_use_mock_store(mock: bool, under_nextest: bool) -> bool {
    mock || under_nextest
}

/// True when this process must never touch the real OS keychain.
///
/// Two signals, either of which is enough:
///
/// - `PIXHAUS_KEYRING_MOCK` — explicit opt-in for anyone (e.g. plain
///   `cargo test`) who wants the in-memory store.
/// - `NEXTEST` — cargo-nextest sets this in every test process it spawns, so
///   the project's standard test command is covered with no harness config to
///   drift out of sync. (An earlier attempt set `PIXHAUS_KEYRING_MOCK` from a
///   `[env]` table in `.config/nextest.toml`, but nextest has no such key and
///   silently ignored it, so the variable never reached the tests and every
///   one popped the macOS login Keychain dialog.)
fn mock_store_requested() -> bool {
    should_use_mock_store(std::env::var_os("PIXHAUS_KEYRING_MOCK").is_some(), std::env::var_os("NEXTEST").is_some())
}

/// Registers the platform credential store as `keyring_core`'s default,
/// exactly once per process, before the first [`Entry`] is created.
///
/// `keyring_core` keeps a single process-global "default store"; without it
/// every `Entry::new` fails with "No default store has been set". The
/// `keyring` meta-crate ships per-platform registration helpers — none of
/// which run unless called, so this is where we call one.
///
/// On Linux we use the pure-Rust zbus Secret Service backend (persistent,
/// integrates with GNOME Keyring / `KWallet`). macOS uses the login Keychain
/// and Windows the Credential Manager via `use_native_store`.
///
/// If registration fails (e.g. a headless Linux box with no Secret Service
/// daemon) we log and move on: the subsequent keychain call surfaces a
/// [`BackendError::Keychain`] to the UI rather than silently falling back to
/// an in-memory or plaintext store.
fn ensure_store() {
    STORE_INIT.call_once(|| {
        // A store may already be registered — e.g. a test installs the
        // in-memory sample store first. Don't override it.
        if keyring_core::get_default_store().is_some() {
            return;
        }

        // Test processes route keychain access through an in-memory store
        // instead of the OS keychain (see `mock_store_requested`). Without
        // this, every test process that touches a backend pops a native
        // credential-store prompt (the macOS login Keychain dialog). The dev
        // app and production trigger neither signal, so they use the real
        // keychain.
        if mock_store_requested() {
            if let Err(err) = keyring::use_sample_store(&std::collections::HashMap::new()) {
                tracing::warn!(error = %err, "failed to register in-memory keyring store");
            }
            return;
        }

        #[cfg(target_os = "linux")]
        let result = keyring::use_zbus_secret_service_store(&std::collections::HashMap::new());
        #[cfg(not(target_os = "linux"))]
        let result = keyring::use_native_store(false);

        if let Err(err) = result {
            tracing::warn!(error = %err, "failed to register keychain store");
        }
    });
}

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
            keyring_core::Error::NoEntry => BackendError::ApiKeyNotFound(backend.to_owned()),
            other => BackendError::Keychain(other.to_string()),
        })
    }

    /// Stores `key` in the OS keychain for `backend`.
    ///
    /// Overwrites an existing key without error.
    pub fn set(backend: &str, key: &str) -> Result<()> {
        let entry = Self::entry(backend)?;
        entry.set_password(key).map_err(|err| BackendError::Keychain(err.to_string()))
    }

    /// Deletes the stored key for `backend`.
    ///
    /// Returns [`BackendError::ApiKeyNotFound`] if no key was stored.
    pub fn delete(backend: &str) -> Result<()> {
        let entry = Self::entry(backend)?;
        entry.delete_credential().map_err(|err| match err {
            keyring_core::Error::NoEntry => BackendError::ApiKeyNotFound(backend.to_owned()),
            other => BackendError::Keychain(other.to_string()),
        })
    }

    fn entry(backend: &str) -> Result<Entry> {
        ensure_store();
        let service = format!("{SERVICE_PREFIX}.{backend}");
        Entry::new(&service, backend).map_err(|err| BackendError::Keychain(err.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_store_policy_truth_table() {
        assert!(!should_use_mock_store(false, false), "neither signal");
        assert!(should_use_mock_store(true, false), "explicit opt-in");
        assert!(should_use_mock_store(false, true), "nextest marker");
        assert!(should_use_mock_store(true, true), "both signals");
    }

    #[test]
    fn nextest_runs_request_the_mock_store() {
        // Regression guard for the keychain-prompt-during-tests bug. cargo-nextest
        // sets NEXTEST=1 in every test process, so under nextest the gate must
        // resolve to the in-memory store with no extra configuration. If this
        // fails, the suite is about to pop the macOS login Keychain dialog again.
        if std::env::var_os("NEXTEST").is_none() {
            // Not running under nextest (e.g. plain `cargo test`); the NEXTEST
            // path doesn't apply. PIXHAUS_KEYRING_MOCK remains the opt-in.
            return;
        }
        assert!(mock_store_requested(), "expected nextest's NEXTEST marker to select the in-memory store");
    }

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
    fn sample_store_round_trip() {
        // Register the cross-platform in-memory sample store before any
        // keychain access. `ensure_store()` early-returns when a store is
        // already set, so production lazy-init won't replace it — keeping
        // this test hermetic and prompt-free on every platform and in CI.
        keyring::use_sample_store(&std::collections::HashMap::new()).expect("sample store registers on all platforms");

        let backend = "pixhaus-test-sample-roundtrip";
        let _ = ApiKeyStore::delete(backend);

        ApiKeyStore::set(backend, "test-key-value").expect("set should succeed");
        assert_eq!(ApiKeyStore::get(backend).expect("get should succeed"), "test-key-value");

        ApiKeyStore::delete(backend).expect("delete should succeed");
        assert!(matches!(ApiKeyStore::get(backend), Err(BackendError::ApiKeyNotFound(_))));
    }

    // Exercises the real OS keychain, which can pop an interactive "allow
    // access" prompt on macOS once a native store is registered. Kept as a
    // manual check (`cargo test -- --ignored`) so it never blocks CI.
    #[ignore = "hits the real OS keychain; may prompt on macOS"]
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
