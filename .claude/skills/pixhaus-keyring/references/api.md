# keyring API reference

Verified against `keyring-core` 1.0 and the `keyring` 4.0.1 aggregator (the
open-source-cooperative fork). The 4.x line is a redesign; signatures here do not
match v3 examples. See `SKILL.md` for the Pixhaus patterns — this file is the
exhaustive lookup.

## Contents

- [Crate map](#crate-map)
- [keyring-core: store registration](#keyring-core-store-registration)
- [keyring-core: Entry](#keyring-core-entry)
- [keyring-core: Error variants](#error-variants)
- [keyring-core: traits and type aliases](#traits-and-type-aliases)
- [keyring-core: CredentialPersistence](#credentialpersistence)
- [keyring-core: mock and sample stores](#mock-and-sample-stores)
- [Provider store crates](#provider-store-crates)
- [The keyring aggregator (use only for a CLI/script)](#the-keyring-aggregator)

## Crate map

| Crate | Role | Depend on it? |
|---|---|---|
| `keyring-core` 1.0 | The library: `Entry`, `Error`, store registration | Yes — this is the API |
| `windows-native-keyring-store` 1.0 | Windows Credential Manager backend | Yes (Windows target) |
| `apple-native-keyring-store` 1.0 | macOS/iOS Keychain backend | Yes (macOS target) |
| `zbus-secret-service-keyring-store` 1.0 | Linux Secret Service via pure-Rust zbus | Yes (Linux target) |
| `linux-keyutils-keyring-store` 1.0 | Linux kernel keyutils (non-persistent) | No — wrong lifetime for a saved key |
| `dbus-secret-service-keyring-store` 1.0 | Linux Secret Service via C libdbus | No — C dep, prefer zbus |
| `db-keystore` ~0.4 | Cross-platform encrypted SQLite (Turso) | Only if you need a portable file vault |
| `keyring` 4.0.1 | CLI + sample + aggregator (`use_*` fns) | No for the GUI — pulls clap/rpassword/rprompt |

All of the above are `MIT OR Apache-2.0`, so they clear the workspace MIT lock; let
`cargo deny check` confirm the full transitive tree when you add them.

## keyring-core: store registration

The default credential store is a process-global. These free functions manage it.
Re-exported from the `keyring_core` crate root.

```rust
/// Set the credential store used by default to create entries. Meant for clients
/// who use one credential store. Blocks waiting for other threads to finish
/// creating entries; meant to be called at startup before creating any entries.
pub fn set_default_store(new: Arc<CredentialStore>)

/// Get the default credential store.
pub fn get_default_store() -> Option<Arc<CredentialStore>>

/// Release the default credential store. Returns the old value and forgets it.
/// Not releasing the store may have unintended side effects.
pub fn unset_default_store() -> Option<Arc<CredentialStore>>
```

A provider's `Store::new()` already returns the `Arc<CredentialStore>` that
`set_default_store` wants — no manual `Arc::new`. Creating an `Entry` before any
`set_default_store` call yields `Error::NoDefaultStore`.

## keyring-core: Entry

`Entry` names a credential slot by `(service, user)` in the default store. Its
methods delegate to the store's `CredentialApi`, so the signatures below mirror
that trait.

```rust
/// Create an entry for the given service and user, using the default store's
/// default configuration. Fails with NoDefaultStore if none is set.
pub fn new(service: &str, user: &str) -> Result<Entry>

/// Like `new`, but pass store-specific modifiers (e.g. {"target": "..."} on
/// Windows). Replaces v3's `new_with_target`. Which keys are honored is
/// store-specific — check the provider's docs.
pub fn new_with_modifiers(
    service: &str,
    user: &str,
    modifiers: &HashMap<&str, &str>,
) -> Result<Entry>

/// Store a UTF-8 string secret (e.g. an API key).
pub fn set_password(&self, password: &str) -> Result<()>

/// Store a binary secret.
pub fn set_secret(&self, secret: &[u8]) -> Result<()>

/// Read the secret back as a UTF-8 string. Returns BadEncoding if the stored
/// bytes are not valid UTF-8; NoEntry if nothing was ever stored / it was deleted.
pub fn get_password(&self) -> Result<String>

/// Read the secret back as raw bytes.
pub fn get_secret(&self) -> Result<Vec<u8>>

/// Read store-specific attributes attached to the credential.
pub fn get_attributes(&self) -> Result<HashMap<String, String>>

/// Update store-specific attributes. Stores that don't support this return
/// NotSupportedByStore.
pub fn update_attributes(&self, attributes: &HashMap<&str, &str>) -> Result<()>

/// Delete the underlying credential. NoEntry if there was nothing to delete.
pub fn delete_credential(&self) -> Result<()>

/// The (service, user) this entry resolves to, if the store reports them.
pub fn get_specifiers(&self) -> Option<(String, String)>
```

Pixhaus uses `new`, `set_password`, `get_password`, and `delete_credential`. The
secret/attribute/specifier methods are there for completeness — reach for
`set_secret`/`get_secret` only when the secret is genuinely binary.

## Error variants

`keyring_core::Error`, `#[non_exhaustive]`. Verbatim from source, with the doc text:

```rust
#[non_exhaustive]
#[derive(Debug)]
pub enum Error {
    /// Runtime failure in the underlying platform storage system. Details are in
    /// the attached platform error.
    PlatformFailure(PlatformError),

    /// The underlying secure storage could not be accessed — typically platform
    /// access rules, e.g. the credential store is locked. The platform error
    /// usually gives the reason.
    NoStorageAccess(PlatformError),

    /// There is no underlying credential entry for this entry. Either one was
    /// never set, or it was deleted.
    NoEntry,

    /// The retrieved password blob was not a UTF-8 string. The raw bytes are
    /// attached.
    BadEncoding(Vec<u8>),

    /// The retrieved secret blob was not formatted as the store expected (some
    /// stores encrypt/transform). The raw blob and an underlying error are attached.
    BadDataFormat(Vec<u8>, PlatformError),

    /// The store itself was not formatted as expected. The value describes the problem.
    BadStoreFormat(String),

    /// One of the entry's attributes exceeded a platform length limit. The values
    /// give the attribute name and the limit exceeded.
    TooLong(String, u32),

    /// A parameter passed to the operation was invalid. The values give the
    /// parameter and describe the problem.
    Invalid(String, String),

    /// More than one credential in the store matches this entry. The value is a
    /// vector of entries wrapping the matching credentials.
    Ambiguous(Vec<Entry>),

    /// There was no default credential store; set one before creating entries.
    NoDefaultStore,

    /// The requested operation is unsupported by the handling store. The value
    /// describes why.
    NotSupportedByStore(String),
}

pub type Result<T> = std::result::Result<T, Error>;
```

How Pixhaus should treat each:

- **`NoEntry`** — expected. "User hasn't set this key." Map to `Ok(None)` on read,
  treat as success on delete. Never log as an error.
- **`NoDefaultStore`** — a programming bug: startup never called
  `set_default_store`. Fix the wiring; don't show it to the user.
- **`NoStorageAccess`** — the vault is locked or the user declined to unlock it.
  Surface a calm, retryable message.
- **`PlatformFailure`**, **`BadStoreFormat`**, **`BadDataFormat`** — genuine
  backend failures; report and, where sensible, let the user re-enter the key.
- **`BadEncoding`** — you stored bytes with `set_secret` and read with
  `get_password`. Use matching methods.
- **`TooLong`**, **`Invalid`** — bad inputs you constructed; fix the call site.
- **`Ambiguous`** — multiple matching credentials. Rare for a fixed
  `(service, user)`; the variant carries the matching entries if you must
  disambiguate.
- **`NotSupportedByStore`** — the chosen backend doesn't support that operation
  (e.g. attribute updates). Avoid relying on optional operations.

## Traits and type aliases

You only implement these to write a custom store, which Pixhaus does not need.
They are documented here so the `Arc<CredentialStore>` / `Arc<Credential>` types in
signatures make sense.

```rust
pub type Credential      = dyn CredentialApi + Send + Sync;
pub type CredentialStore = dyn CredentialStoreApi + Send + Sync;
```

`CredentialApi` (what an `Entry` delegates to): `set_password`, `set_secret`,
`get_password`, `get_secret`, `get_attributes`, `update_attributes`,
`delete_credential`, `get_credential`, `get_specifiers`, `as_any`, `debug_fmt`.

`CredentialStoreApi` (what a provider's `Store` implements): `vendor`, `id`,
`build(service, user, modifiers)` (this is what `Entry::new` calls), `search`,
`as_any`, `persistence`, `debug_fmt`.

## CredentialPersistence

Reports how long a store keeps a credential. Useful if Pixhaus ever wants to warn
"this backend won't remember your key after reboot."

```rust
#[non_exhaustive]
pub enum CredentialPersistence {
    EntryOnly,    // lives only as long as the Entry
    ProcessOnly,  // lost when the process exits
    UntilLogout,  // lost at logout (keyutils-like)
    UntilReboot,  // lost at reboot
    UntilDelete,  // persists until explicitly deleted (Keychain / Secret Service)
    Unspecified,
}
```

The OS vaults Pixhaus targets (Keychain, Credential Manager, Secret Service) report
`UntilDelete`. The keyutils backend reports a shorter lifetime — which is exactly
why it is the wrong choice for a saved API key.

## mock and sample stores

`keyring-core` bundles two test-only stores. Neither is warranted secure or
robust; never ship either as the real backend.

- **`keyring_core::mock`** — `mock::Store::new()` returns a store with no
  persistence and supports injecting specific errors, so you can exercise the
  `NoEntry` / `NoStorageAccess` / `PlatformFailure` branches of a wrapper in unit
  tests. This is the one Pixhaus tests use.
- **`keyring_core::sample`** — behind the `sample` feature; file-based persistence,
  intended as a worked example for writing a custom provider.

```rust
// Test setup — install the mock as the default store, then use Entry normally.
keyring_core::set_default_store(keyring_core::mock::Store::new().unwrap());
```

## Provider store crates

Each provider exposes a `Store` whose `new()` returns the `Arc<CredentialStore>`
you hand to `set_default_store`. Verified constructors:

```rust
// Windows Credential Manager
use windows_native_keyring_store::Store;
keyring_core::set_default_store(Store::new()?);

// Linux Secret Service (GNOME Keyring / KWallet) via pure-Rust zbus
use zbus_secret_service_keyring_store::Store;
keyring_core::set_default_store(Store::new()?);

// macOS / iOS Keychain — same shape (apple-native-keyring-store 1.0)
use apple_native_keyring_store::Store;
keyring_core::set_default_store(Store::new()?);
```

Some providers also offer a configured constructor (e.g. a
`Store::new_with_configuration` / builder that takes modifiers) — check the
specific crate's `examples/example.rs` and docs before using anything beyond
`new()`. The `zbus-secret-service-keyring-store` crate exposes `crypto-rust` and
`crypto-openssl` features for its session transport; prefer `crypto-rust` to stay
off OpenSSL.

## The keyring aggregator

The `keyring` 4.0.1 crate is the CLI binary plus a thin convenience layer. Pixhaus
should **not** depend on it (it pulls `clap`, `rpassword`, `rprompt`), but the
functions are documented here for completeness and for the occasional throwaway
script. They allocate a store and install it as the default in one call.

```rust
pub const NAMED_STORES: [&str; 9] = [
    "android", "keychain", "keyutils", "protected", "sample",
    "secret-service", "secret-service-async", "sqlite", "windows",
];

/// Pick the OS-native store. On Linux, uses keyutils unless not_keyutils is true,
/// in which case it uses the synchronous Secret Service store. Falls back to the
/// sample store on platforms with no native vault.
pub fn use_native_store(not_keyutils: bool) -> Result<()>

pub fn use_named_store(name: &str) -> Result<()>
pub fn use_named_store_with_modifiers(name: &str, modifiers: &HashMap<&str, &str>) -> Result<()>

pub fn use_apple_keychain_store(config: &HashMap<&str, &str>) -> Result<()>
pub fn use_apple_protected_store(config: &HashMap<&str, &str>) -> Result<()>
pub fn use_windows_native_store(config: &HashMap<&str, &str>) -> Result<()>
pub fn use_linux_keyutils_store(config: &HashMap<&str, &str>) -> Result<()>
pub fn use_dbus_secret_service_store(config: &HashMap<&str, &str>) -> Result<()>
pub fn use_zbus_secret_service_store(config: &HashMap<&str, &str>) -> Result<()>
pub fn use_sqlite_store(config: &HashMap<&str, &str>) -> Result<()>
pub fn use_android_native_store(config: &HashMap<&str, &str>) -> Result<()>
pub fn use_sample_store(config: &HashMap<&str, &str>) -> Result<()>

pub fn release_store()
pub fn store_info() -> String
/// Turn an owned String map (e.g. collected from user input) into the borrowed
/// map the `use_*` config functions want.
pub fn internalize(config: Option<&HashMap<String, String>>) -> HashMap<&str, &str>
```

Note `use_native_store(false)` picks **keyutils** on Linux — non-persistent — which
is another reason the GUI wires the zbus Secret Service provider explicitly rather
than leaning on this convenience.
