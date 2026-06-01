---
name: pixhaus-keyring
description: >
  Use when Pixhaus needs to store or read a secret in the operating system's own
  credential vault with the `keyring` crate — above all the AI backend API keys
  (Anthropic, OpenAI, Replicate, Stability, …) the user pastes into settings, plus
  any other token or password Pixhaus must keep off disk in plaintext. Trigger
  this for ANY "save the API key", "where do I put the user's secret / token /
  password", "read the stored key back", "OS keychain / Credential Manager /
  Secret Service / GNOME Keyring / KWallet", "Entry", "set_password / get_password
  / set_secret / get_secret / delete_credential", "set_default_store", or "the key
  shouldn't live in the config file" task, even when the user never says
  "keyring". Two traps make this worth stopping for. First, keyring 4.x is a
  ground-up redesign: a library now depends on `keyring-core` plus a per-platform
  provider crate and registers a store at runtime with `set_default_store` — the
  v3 "pick a backend via Cargo feature" pattern and most online examples are wrong
  now. Second, a keyring call is blocking I/O that on Linux can pop a system
  unlock dialog, so it must never run on the egui update thread. Reach for this
  skill rather than guessing the API from memory, which is a major version behind.
---

# keyring for Pixhaus

`keyring` stores secrets in the platform's own secure credential vault — the macOS
Keychain, the Windows Credential Manager, the Linux Secret Service (GNOME Keyring
/ KWallet) — instead of in a file Pixhaus controls. In Pixhaus the job is narrow
and important: the **AI backend API keys**. The user pastes an Anthropic, OpenAI,
Replicate, or Stability key into settings; that key is a long-lived secret that
must survive restarts but must not sit in plaintext in the MessagePack config
([[pixhaus-directories]] is for config/cache *paths*, not for secrets). keyring is
the bridge to the OS vault.

It is a small API around `Entry`. The reason this skill exists is that the crate
was rewritten at 4.0 and the threading model bites if you ignore it.

## Trap 1: keyring 4.x is not the keyring you remember

If you learned this crate before — or from most blog posts and Stack Overflow
answers — what you know is the v3 model: one `keyring` dependency, the platform
backend chosen by a **Cargo feature** at compile time, `Entry::new` reads it
directly. That is gone. The maintainers say plainly: do not carry v3 code into v4.

The 4.x model splits into three pieces:

- **`keyring-core`** — the actual library: the `Entry` type, the `Error` type, and
  the `set_default_store` / `get_default_store` / `unset_default_store` functions.
  This is what a library or app depends on.
- **provider store crates** — one crate per backend (`windows-native-keyring-store`,
  `apple-native-keyring-store`, `zbus-secret-service-keyring-store`, …). You pick
  the ones for your target platforms.
- **`keyring`** — the CLI binary, sample code, and a thin aggregator with
  `use_native_store()` convenience functions. It pulls in `clap`, `rpassword`, and
  `rprompt` as non-optional dependencies, so depending on it from a GUI drags a
  command-line argument parser into the build for no reason.

The backend is now chosen and registered **at runtime**, not by a feature. You
allocate a store at startup and install it as the default with
`set_default_store`. Every `Entry` created afterward uses it.

**For Pixhaus: depend on `keyring-core` plus the per-platform provider crates, not
on the `keyring` aggregator.** Pixhaus is a GUI, not a CLI — it has no use for
clap/rpassword/rprompt, and wiring three small provider crates by `cfg` is less
weight than the aggregator. (The aggregator's `use_native_store(not_keyutils)` is
fine for a throwaway script; it is the wrong default for the shipped app.)

## Dependencies, license, and the Linux choice

| Crate | Version | License | cargo deny |
|---|---|---|---|
| `keyring-core` | 1.0 | `MIT OR Apache-2.0` | passes the MIT lock |
| `windows-native-keyring-store` | 1.0 | `MIT OR Apache-2.0` | passes |
| `apple-native-keyring-store` | 1.0 | `MIT OR Apache-2.0` | passes |
| `zbus-secret-service-keyring-store` | 1.0 | `MIT OR Apache-2.0` | passes |

```toml
# keyring-core is the library; the stores are per-platform and target-gated so
# each only builds where it applies. Pixhaus is desktop-only (Windows/macOS/Linux).
keyring-core = "1"

[target.'cfg(target_os = "windows")'.dependencies]
windows-native-keyring-store = "1"

[target.'cfg(target_os = "macos")'.dependencies]
apple-native-keyring-store = "1"

[target.'cfg(target_os = "linux")'.dependencies]
zbus-secret-service-keyring-store = "1"
```

The Linux choice is a real decision, not a coin flip. There are three Linux
providers; pick the Secret Service one, via **zbus**:

- **`zbus-secret-service-keyring-store` (use this).** Talks to the Secret Service
  over D-Bus using pure-Rust `zbus`. It is persistent — credentials live in GNOME
  Keyring / KWallet and survive logout and reboot — which is exactly what a
  long-lived API key needs. Pure Rust means no system C library and no GPL-tinged
  `libdbus` link to argue with `cargo deny` about.
- **`linux-keyutils-keyring-store` (don't, for this).** Stores in the kernel
  keyutils keyring, which is session/process scoped and does **not** persist across
  reboot (often not across logout). Wrong lifetime for a saved API key. This is
  also what the aggregator's `use_native_store(false)` picks on Linux by default —
  another reason to skip the aggregator.
- **`dbus-secret-service-keyring-store` (don't).** Same Secret Service, but through
  the C `libdbus` library. Adds a system C dependency and a licensing question the
  zbus variant simply avoids.

The zbus store exposes `crypto-rust` and `crypto-openssl` features for the session
transport encryption; prefer `crypto-rust` (no OpenSSL system dependency, stays
all-Rust). Confirm the current default in that crate's docs before pinning
features.

## Register the store once, at startup

The default store is process-global, so it has exactly one owner: the shell binary
sets it during startup, before any `Entry` is created, and the rest of the code
never touches store setup again. This is the single-owner rule from CLAUDE.md
applied to a global.

```rust
/// Install the OS credential store as keyring's default. Call once, early in
/// shell startup, before creating any Entry. Returns a thiserror variant rather
/// than panicking so a headless/locked environment is reportable, not fatal.
fn install_credential_store() -> Result<(), SecretsError> {
    #[cfg(target_os = "windows")]
    let store = windows_native_keyring_store::Store::new()?;
    #[cfg(target_os = "macos")]
    let store = apple_native_keyring_store::Store::new()?;
    #[cfg(target_os = "linux")]
    let store = zbus_secret_service_keyring_store::Store::new()?;

    keyring_core::set_default_store(store);
    Ok(())
}
```

`Store::new()` returns the `Arc<CredentialStore>` that `set_default_store` wants, so
there is nothing to wrap. If you never call `set_default_store`, every `Entry`
operation fails with `Error::NoDefaultStore` — that error means "you forgot
startup wiring," not "the user has no key."

You generally don't need `unset_default_store` in a desktop app that runs until the
user quits; the aggregator's CLI uses it for clean teardown between commands.

## The Entry API — store, read, delete a secret

An `Entry` names a slot by `(service, user)`. For Pixhaus, make `service` a stable
constant and `user` the backend name, so each provider's key is its own entry:

```rust
use keyring_core::{Entry, Error};

const SERVICE: &str = "dev.pixhaus.ai-keys";

// Save the key the user pasted into settings.
fn store_api_key(backend: &str, key: &str) -> Result<(), SecretsError> {
    let entry = Entry::new(SERVICE, backend)?;
    entry.set_password(key)?;
    Ok(())
}

// Read it back when constructing a backend client. NoEntry == "user hasn't set
// this key yet", which is an ordinary state, not an error to surface loudly.
fn load_api_key(backend: &str) -> Result<Option<String>, SecretsError> {
    let entry = Entry::new(SERVICE, backend)?;
    match entry.get_password() {
        Ok(key) => Ok(Some(key)),
        Err(Error::NoEntry) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

// "Forget this key" in settings.
fn clear_api_key(backend: &str) -> Result<(), SecretsError> {
    let entry = Entry::new(SERVICE, backend)?;
    match entry.delete_credential() {
        Ok(()) | Err(Error::NoEntry) => Ok(()), // already gone is success
        Err(e) => Err(e.into()),
    }
}
```

The key moves that matter:

- **`set_password` / `get_password`** for UTF-8 strings (API keys). Use
  **`set_secret` / `get_secret`** (`&[u8]` / `Vec<u8>`) only for genuinely binary
  secrets — calling `get_password` on bytes that aren't UTF-8 returns
  `Error::BadEncoding`, not a decoded string.
- **`Error::NoEntry` is the normal "not set yet" branch.** Match it and return
  `Ok(None)` / treat delete as success. Never log it as a failure — a user who
  hasn't entered an OpenAI key isn't an error condition.
- **`Entry::new_with_modifiers(service, user, &modifiers)`** replaces v3's
  `new_with_target`; `target` is now just one possible key in the modifiers map,
  and which modifiers a store accepts is store-specific. Plain `Entry::new` is what
  you want for Pixhaus unless a provider's docs tell you otherwise.

Full method and error reference: [references/api.md](references/api.md).

## Trap 2: keyring calls block — keep them off the egui thread

Every `set_password` / `get_password` / `delete_credential` is synchronous I/O
into the OS vault. On Linux that is a D-Bus round-trip to the Secret Service, and
the **first** access in a session can pop a system dialog asking the user to unlock
their keyring — which blocks until they respond, possibly for seconds, possibly
forever if they walk away. macOS Keychain can do the same. If that call sits on the
egui update thread, the whole Pixhaus window freezes (see [[pixhaus-egui]]).

So treat a keyring call exactly like a file dialog (see [[pixhaus-rfd]]): run it off
the UI thread and deliver the result back over a channel the update loop drains.
Because it is blocking CPU/IO work rather than an async future, the right tool is
`tokio::task::spawn_blocking`, per the async rules in [[pixhaus-rust-conventions]]
and [[pixhaus-tokio]].

```rust
use tokio::sync::oneshot;

// Fired from a "Save key" button handler inside `ui`. Returns immediately.
fn save_key_async(
    ctx: &egui::Context,
    backend: String,
    key: String,
) -> oneshot::Receiver<Result<(), SecretsError>> {
    let (tx, rx) = oneshot::channel();
    let ctx = ctx.clone(); // cheap: egui::Context is an Arc handle
    tokio::task::spawn_blocking(move || {
        let result = store_api_key(&backend, &key); // the blocking keyring call
        let _ = tx.send(result);
        ctx.request_repaint(); // wake the loop so it drains the channel now
    });
    rx
}
```

Drain `rx` each frame the same way [[pixhaus-rfd]] drains its dialog receiver. The
`request_repaint()` after sending is the same must-not-forget line — without it the
"saved" confirmation lags until the next mouse move.

Reading keys at startup is the one place blocking is acceptable: if you must have
the key before constructing a backend client, do it on a background task during
init, not inline on the first frame.

## Where secrets belong (and where they don't)

- **API keys, tokens, passwords → keyring (the OS vault).** Encrypted at rest,
  protected by the OS login, never in your files.
- **Everything else → the normal config/cache/data paths** from
  [[pixhaus-directories]]. The MessagePack config can hold *which* backends are
  enabled, the model names, endpoints, and a flag like `has_api_key: bool` — but
  never the key itself.

A frequent mistake is serializing the whole settings struct, key included, to the
config file "for now." Don't. A secret in the MessagePack project/config file is a
secret on disk in plaintext, syncable and backup-able by accident. Keep the key in
keyring and the metadata in config, and have settings load the key from keyring on
demand.

## Errors and the no-unwrap rule

`Entry::new` and every operation return `keyring_core::Result<T>` (alias for
`Result<T, keyring_core::Error>`). In Pixhaus's io/core layer, wrap that in a
`thiserror` enum with `#[from] keyring_core::Error`, and let the egui layer surface
a calm message ("Couldn't reach the system keychain"). `anyhow` stays in the binary.
No `unwrap`/`expect` outside tests — a locked keyring or a missing default store is
a user-facing condition to report, not a panic. `NoEntry` in particular is an
expected branch, not an unwrap site. The full `Error` variant list and what each
means is in [references/api.md](references/api.md#error-variants).

## Testing without touching the real vault

Tests must never read or write the developer's real Keychain — that is flaky,
machine-specific, and can prompt for a password mid-test. `keyring-core` ships a
**mock store** (`keyring_core::mock`) with no persistence and optional error
injection. Install it as the default store in the test, exactly like a real store:

```rust
#[test]
fn round_trips_a_key() {
    keyring_core::set_default_store(keyring_core::mock::Store::new().unwrap());
    let entry = Entry::new("test-service", "openai").unwrap();
    entry.set_password("sk-test-123").unwrap();
    assert_eq!(entry.get_password().unwrap(), "sk-test-123");
    entry.delete_credential().unwrap();
}
```

The mock can be configured to return specific errors, so it is how you test the
`NoEntry`, `NoStorageAccess`, and `PlatformFailure` branches of your wrapper
without a real vault. `unwrap` is fine here — this is test code. See
[[pixhaus-testing-conventions]]. Note the default store is process-global, so tests
that set it can interfere when run in parallel; keep store setup inside each test
and avoid sharing one `service` name across tests that run concurrently.

## Decision shortcut

```
Need to keep a secret (API key / token / password) in Pixhaus?
├─ Depend on keyring-core + the per-platform provider crate.
│    NOT the `keyring` aggregator (CLI baggage), NOT a v3 Cargo-feature backend.
├─ Linux backend? → zbus-secret-service-keyring-store (persistent, pure-Rust).
│    NOT keyutils (non-persistent), NOT the libdbus variant (C dep).
├─ At startup, ONCE, in the shell: set_default_store(Store::new()?).
├─ Per secret: Entry::new(SERVICE, backend) then set_password / get_password / delete_credential.
│    NoEntry == "not set yet" → Ok(None) / treat delete as success. Never an error to log.
├─ Calling from anywhere reachable by the egui `ui`? → spawn_blocking + channel + request_repaint.
│    A keyring call blocks and can pop an OS unlock dialog. Never on the update thread.
└─ The metadata (which backends, model names) → config file (pixhaus-directories).
   The key itself → keyring ONLY. Never serialize a secret into the MessagePack config.
```
