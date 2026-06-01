# Features, TLS, multipart, redirects, blocking

Verified against reqwest 0.13.4. This file is the version-sensitive one: 0.13 renamed
the TLS features and changed the defaults, so confirm any feature line against
https://docs.rs/crate/reqwest/latest/features before depending on it. For the request
and response APIs see the other two reference files.

## Contents

- The complete feature table
- Default features and the 0.13 changes
- TLS: backend, crypto provider, root store
- `cargo deny` and the MIT license gate
- `multipart` — uploading images to a backend
- `redirect::Policy`
- The `blocking` module (and why not to use it here)

## The complete feature table

| Feature | Default | What it enables |
|---|---|---|
| `charset` | yes | non-UTF-8 text decoding (`text_with_charset`) |
| `default-tls` | yes | TLS; in 0.13 resolves to `rustls` (aws-lc-rs provider) |
| `http2` | yes | HTTP/2 support |
| `system-proxy` | yes | read OS proxy settings (Windows/macOS) |
| `rustls` | via default-tls | rustls TLS with `rustls-platform-verifier` (OS trust store) |
| `rustls-no-provider` | no | rustls without a bundled crypto provider — you install one |
| `native-tls` | no | system-native TLS (OpenSSL / Schannel / Secure Transport) |
| `native-tls-vendored` | no | native-tls with vendored OpenSSL |
| `json` | no | `RequestBuilder::json`, `Response::json` |
| `query` | no | `RequestBuilder::query` |
| `form` | no | `RequestBuilder::form` |
| `multipart` | no | the `multipart` module (`Form`, `Part`) |
| `stream` | no | `Response::bytes_stream`, `Part::stream`, async `Form::file` |
| `gzip` / `brotli` / `deflate` / `zstd` | no | response-body decompression per algorithm |
| `blocking` | no | the `reqwest::blocking` module |
| `cookies` | no | cookie store / session support |
| `socks` | no | SOCKS5 proxy support |
| `http3` | no | HTTP/3 (unstable) |
| `hickory-dns` | no | Hickory async DNS resolver (the renamed `trust-dns`) |

## Default features and the 0.13 changes

Defaults on: `charset`, `default-tls`, `http2`, `system-proxy` (which transitively pull
`rustls` and the aws-lc-rs provider). The 0.13 line broke several things that older
examples and training data still assume:

- `default-tls` now means **rustls**, not native-tls. Pre-0.13 it meant native-tls.
- The `rustls-tls`, `rustls-tls-webpki-roots`, `rustls-tls-native-roots`, and
  `rustls-tls-manual-roots` features **no longer exist**. The feature is plain `rustls`.
- `query` and `form` are now opt-in features. They used to be always available.
- `trust-dns` is now `hickory-dns`.
- `use_rustls_tls()`, `use_native_tls()`, `add_root_certificate()`, and
  `danger_accept_invalid_certs()` on `ClientBuilder` are deprecated.

The Pixhaus baseline, defaults off and TLS named explicitly:

```toml
reqwest = { version = "0.13", default-features = false, features = [
    "rustls", "json", "stream", "charset", "http2",
] }
```

Add `multipart` for image-upload backends, `form`/`query` if a backend needs urlencoded
bodies or query params, and a decompression feature only if a backend sends compressed
bodies you want transparently decoded.

## TLS: backend, crypto provider, root store

Three independent choices, and 0.13 collapses the common path into one feature:

1. **TLS implementation** — `rustls` (pure Rust) vs `native-tls` (OS library). Pixhaus
   uses `rustls`: one implementation across Windows/macOS/Linux, no system OpenSSL
   dependency, predictable for a shipped desktop binary.
2. **Crypto provider** — with the `rustls` feature this is **aws-lc-rs** by default.
   The alternative is **ring**, selected via `rustls-no-provider` plus installing ring
   as the process default. The provider is the part with license implications (below).
3. **Root certificates** — `rustls` uses `rustls-platform-verifier`, which verifies
   against the OS trust store (Schannel store, macOS Keychain, the Linux system bundle).
   There is no longer a reqwest feature that selects the bundled Mozilla `webpki-roots`
   set; that would now be configured at the rustls layer with a preconfigured client,
   which Pixhaus does not need — the platform verifier is the right default for a
   desktop app talking to well-known AI endpoints.

If `cargo deny` rejects aws-lc-rs, switch the provider to ring:

```toml
reqwest = { version = "0.13", default-features = false, features = [
    "rustls-no-provider", "json", "stream", "charset", "http2",
] }
rustls = { version = "0.23", features = ["ring"] }
```

```rust
// once, at startup in main, before the first request:
rustls::crypto::ring::default_provider()
    .install_default()
    .expect("install rustls ring provider");   // startup setup may expect
```

## `cargo deny` and the MIT license gate

The workspace enforces a strict MIT-compatible license set (`.cargo/deny.toml`). reqwest
itself is `MIT OR Apache-2.0` and passes. The risk is in the TLS stack:

- aws-lc-rs carries an `OpenSSL`-license component. Whether `cargo deny` accepts the
  full `ISC AND (Apache-2.0 OR ISC) AND OpenSSL` expression depends on how it resolves
  the bare `OpenSSL` term against the allow list — verify, don't assume.
- The deny.toml already allow-lists `webpki-roots` (CDLA-Permissive-2.0) and the rest of
  the rustls dependencies, so the rustls path was anticipated.

The non-optional step after adding reqwest to any crate:

```bash
cargo deny check --config .cargo/deny.toml
```

If the license gate is red, the honest fixes are (a) the ring provider above, or (b) a
documented `exceptions` entry in deny.toml with a written reason. Do not silence the gate
by loosening the global allow list (see CLAUDE.md) — that hides every future license
regression, not just this one.

## `multipart` — uploading images to a backend

Feature `multipart`. Some backends (img2img, ControlNet, upscaling) take a reference
image as a multipart upload rather than base64-in-JSON.

```rust
// Form
pub fn new() -> Form
pub fn text<T, U>(self, name: T, value: U) -> Form
    where T: Into<Cow<'static, str>>, U: Into<Cow<'static, str>>
pub fn part<T: Into<Cow<'static, str>>>(self, name: T, part: Part) -> Form
pub async fn file<T, U>(self, name: T, path: U) -> Result<Form>   // ASYNC; feature: stream
    where T: Into<Cow<'static, str>>, U: AsRef<Path>

// Part — constructors are associated fns (no self)
pub fn bytes<T: Into<Cow<'static, [u8]>>>(value: T) -> Part
pub fn text<T: Into<Cow<'static, str>>>(value: T) -> Part
pub fn stream<T: Into<Body>>(value: T) -> Part                    // feature: stream
pub fn stream_with_length<T: Into<Body>>(value: T, length: u64) -> Part  // feature: stream
// builder methods on a Part (consume self)
pub fn file_name<T: Into<Cow<'static, str>>>(self, filename: T) -> Part
pub fn mime_str(self, mime: &str) -> Result<Part>                 // Result — invalid MIME errors
```

In Pixhaus the image to upload is usually already in memory as RGBA/PNG bytes, not a file
on disk, so build the `Part` from bytes:

```rust
let png: Vec<u8> = encode_png(&pixels)?;     // encode on a blocking task, see pixhaus-png
let part = reqwest::multipart::Part::bytes(png)
    .file_name("reference.png")
    .mime_str("image/png")?;                 // mime_str is fallible; propagate
let form = reqwest::multipart::Form::new()
    .text("prompt", prompt)
    .part("image", part);
let resp = client.post(url).bearer_auth(key).multipart(form).send().await?;
```

Two traps from memory: `Form::file` is **async and requires the `stream` feature** (not
just `multipart`) because it reads the file off disk as a stream — and you rarely want it
here since the pixels are already in memory. `Part::bytes`/`text`/`stream` are associated
functions (`Part::bytes(...)`), while `file_name`/`mime_str` are builder methods on an
existing `Part`.

## `redirect::Policy`

Set on the client with `ClientBuilder::redirect(policy)`.

```rust
pub fn none() -> Policy                  // never follow a redirect
pub fn limited(max: usize) -> Policy     // follow up to max hops, then error
pub fn custom<T>(policy: T) -> Policy
    where T: Fn(Attempt) -> Action + Send + Sync + 'static
impl Default for Policy                  // the default follows up to ~10 redirects
```

The default (~10 hops) is fine for almost every AI API. Reach for `none()` only when you
specifically want to read a `3xx` `Location` yourself — for example a backend that
returns a `302` to a signed CDN URL for the generated asset and you'd rather fetch that
URL on your own terms. A custom policy receives an `Attempt` and returns
`attempt.follow()`, `attempt.stop()`, or `attempt.error(...)`.

## The `blocking` module (and why not to use it here)

Feature `blocking`. It mirrors the async API with synchronous bodies, and `Response`
implements `std::io::Read`, so `copy_to` exists here:

```rust
pub fn bytes(self) -> Result<Bytes>                       // NOT async
pub fn text(self) -> Result<String>                       // NOT async
pub fn json<T: DeserializeOwned>(self) -> Result<T>       // feature: json; NOT async
pub fn copy_to<W: Write + ?Sized>(&mut self, w: &mut W) -> Result<u64>  // stream to a writer
```

**Do not use it in Pixhaus.** The docs are explicit: *"the functionality in
`reqwest::blocking` must not be executed within an async runtime, or it will panic when
attempting to block."* Pixhaus owns a tokio runtime, so the blocking client will panic if
called from a task, and calling it on the egui thread freezes the window. Use the async
`Client` everywhere. If you genuinely have a synchronous-only call site (you almost never
do), the correct shape is `tokio::task::spawn_blocking` around the work (see
[[pixhaus-tokio]]) — but reach for that for sync libraries you can't avoid, not as a way
to use the blocking HTTP client.
