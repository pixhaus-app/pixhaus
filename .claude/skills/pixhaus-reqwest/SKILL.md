---
name: pixhaus-reqwest
description: >
  Use when making any HTTP request from the Pixhaus binary — above all the AI
  multi-backend runtime (`Anthropic`, `OpenAI`, `Replicate`, `Ollama`, `ComfyUI`,
  `Stability`), where each backend adapter talks to its provider's REST API over
  reqwest. Trigger this for ANY "call the AI backend", "send a request to the
  API", "POST this JSON / GET that endpoint", "upload an image to the model",
  "stream tokens / stream the response", "set the auth header / bearer token /
  API key", "build the HTTP client", "add a timeout / retry", "parse the JSON
  response", "the request hangs / times out", "TLS / certificate error", or
  "which reqwest feature do I enable" task, even when the user doesn't say
  "reqwest". reqwest is the async HTTP client for the binary; it runs on the one
  tokio runtime and its result returns to the egui loop over a channel — it must
  never touch the UI thread. reqwest 0.13 broke hard from the 0.11/0.12 API in
  training data (rustls is the DEFAULT TLS now, the `rustls-tls` feature is gone,
  `query`/`form` became opt-in features), and the default crypto provider's
  license footprint can trip the MIT `cargo deny` gate — so reach for this skill
  rather than relying on memory. For where the request future is spawned and how
  its result reaches the frame, that's [[pixhaus-tokio]]; for the `Backend` trait
  the adapters implement, [[pixhaus-async-trait]].
---

# reqwest for Pixhaus

reqwest is the async HTTP client for the Pixhaus binary. Its one job here is the
AI multi-backend runtime: each backend adapter — `Anthropic`, `OpenAI`,
`Replicate`, `Ollama`, `ComfyUI`, `Stability` — talks to a remote REST API, and
reqwest is how. POST a prompt, upload a reference image, stream tokens back,
download a generated sprite. That's the whole surface area; Pixhaus is a desktop
editor, not a server, so there is no inbound HTTP and no web framework.

reqwest sits on top of hyper and **requires a tokio runtime**. That means every
rule from [[pixhaus-tokio]] applies: the request future is spawned on the binary's
one runtime, never run on the egui update thread, and its result comes back to the
frame over a channel. reqwest is the *what* (the HTTP call); tokio is the *where*
(the runtime and the boundary). Get the boundary wrong and the window freezes
mid-request.

## Versions and license

| Crate | Version | License |
|---|---|---|
| `reqwest` | 0.13.x (docs verified at 0.13.4) | MIT OR Apache-2.0 |

reqwest itself is MIT/Apache and passes the workspace MIT lock. The catch is the
TLS stack it pulls in — see "TLS and the MIT license" below, the one decision that
needs care before you add reqwest to a crate. The workspace does not pin reqwest
yet. The recommended `[workspace.dependencies]` entry:

```toml
reqwest = { version = "0.13", default-features = false, features = [
    "rustls",   # rustls TLS, verifying against the OS trust store; see TLS section
    "json",     # RequestBuilder::json / Response::json
    "stream",   # Response::bytes_stream for streaming AI responses
    "charset",  # robust non-UTF-8 text decoding
    "http2",    # most AI APIs are HTTP/2
] }
```

`default-features = false` is deliberate. The defaults turn on `default-tls`,
which in 0.13 resolves to rustls with the **aws-lc-rs** crypto provider — and that
provider's license footprint needs a `cargo deny` check before you trust it. Turn
defaults off and name TLS explicitly with the `rustls` feature so the choice is
visible. When you bump reqwest, re-verify against docs.rs — see
[[feedback_dep_upgrades]] — because 0.13 already moved the feature names once.

## reqwest 0.13 broke from the 0.11/0.12 API in your memory

These changed in 0.13 and will bite anyone working from training data. Check them
before writing code:

- **rustls is the default TLS backend now, not native-tls.** `default-tls` enables
  rustls (aws-lc-rs provider), not the system OpenSSL/Schannel stack.
- **The whole `rustls-tls*` feature family is gone.** `rustls-tls`,
  `rustls-tls-webpki-roots`, `rustls-tls-native-roots`, and `rustls-tls-manual-roots`
  no longer resolve. The TLS feature is now plain `rustls`, and certificate roots go
  through `rustls-platform-verifier` (the OS trust store) instead of a
  feature-selected bundle. `rustls-no-provider` is the escape hatch for supplying your
  own crypto provider.
- **`query` and `form` are opt-in crate features now.** `RequestBuilder::query` and
  `::form` used to be always available; in 0.13 they fail to compile unless you
  enable the matching feature. `json` and `multipart` have always been opt-in.
- **`use_rustls_tls()`, `use_native_tls()`, `add_root_certificate()`, and
  `danger_accept_invalid_certs()` are deprecated.** Configure TLS through features,
  not these builder methods.
- **`trust-dns` was renamed `hickory-dns`.** You almost never need it here.

## The mental model: one shared Client, on the tokio runtime, result over a channel

Three facts drive correct reqwest use in Pixhaus.

1. **Build one `Client` and share it.** The `Client` holds a connection pool and is
   `Arc` internally — the docs say outright you do not wrap it in `Arc`/`Rc`,
   because cloning is already a cheap handle to the shared pool. Building a client
   per request throws away connection reuse and TLS session resumption, which on a
   chatty AI session is the difference between snappy and sluggish. Construct it
   once (in the AI module that owns the backends) and clone it into each adapter.

2. **The request future runs on the runtime, never the UI thread.** A backend's
   async `run(...)` does `client.post(url)...send().await`. That future is spawned
   on the binary's one tokio runtime ([[pixhaus-tokio]]). The egui loop only *starts*
   the request and *drains* the result; any `.await` on the UI thread freezes the
   window.

3. **The result returns over a channel, and a repaint wakes the loop.** Same pattern
   as every other background job: the spawned task sends its `Result` down an mpsc/
   oneshot channel and calls `ctx.request_repaint()`. `logic` drains with `try_recv`
   each frame. reqwest changes nothing about this — it's just what fills the `async`
   block.

```
  egui update thread                         tokio runtime
  ------------------                         -------------
  ui: user clicks "Generate"
    handle.spawn(async move {                  ┌─ task runs on the runtime
        backend.run(req).await   ───────────▶  │   client.post(url)
    })                                          │     .bearer_auth(key)
                                                │     .json(&body)
                                                │     .send().await?
                                                │     .json::<Resp>().await?
                                                │  tx.send(result)
                                                └─ ctx.request_repaint()  ◀── wakes loop
  logic (next frame):
    while let Ok(r) = rx.try_recv() { apply(r) }
```

## The canonical request

```rust
use std::time::Duration;
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
struct GenerateRequest { prompt: String, width: u32, height: u32 }

#[derive(Deserialize)]
struct GenerateResponse { image_url: String }

// Built once, cloned into each backend adapter.
fn build_client() -> reqwest::Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent(concat!("pixhaus/", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(120))      // generation can be slow; pick per backend
        .connect_timeout(Duration::from_secs(10))
        .build()
}

// Inside a backend's async run(...). Spawned on the tokio runtime, never the UI thread.
async fn generate(
    client: &reqwest::Client,
    api_key: &str,
    req: GenerateRequest,
) -> Result<GenerateResponse, BackendError> {     // BackendError is a thiserror enum
    let resp = client
        .post("https://api.example.com/v1/generate")
        .bearer_auth(api_key)                     // Authorization: Bearer <key>
        .json(&req)                               // serializes + sets Content-Type
        .send()
        .await?
        .error_for_status()?                      // turn 4xx/5xx into an Err
        .json::<GenerateResponse>()               // deserialize the body
        .await?;
    Ok(resp)
}
```

Note the order: `send().await?` gets you headers, then `error_for_status()?` checks
the status code, then `.json().await?` reads and deserializes the body. Calling
`.json()` *before* `error_for_status()` tries to parse an error page as your success
type and gives a confusing decode error instead of the real status. JSON needs the
`json` feature and pairs with serde — see [[pixhaus-serde]] and [[pixhaus-serde-json]].

## TLS and the MIT license — the decision to get right first

This is the load-bearing call, because the workspace's `cargo deny` enforces a
strict MIT-compatible license set and reqwest's TLS choice changes which crates
land in the tree.

- **`rustls` (recommended) → rustls + the OS trust store.** This is the same TLS
  implementation on every platform, verifying certificates against the system trust
  store via `rustls-platform-verifier` (Schannel store on Windows, Keychain on macOS,
  system bundle on Linux). It is what `default-tls` resolves to in 0.13, just named
  explicitly so the choice is visible. No web framework, no inbound TLS — this is all
  Pixhaus needs.
- **`native-tls` → system TLS implementation.** Links Schannel (Windows), Secure
  Transport (macOS), and OpenSSL (Linux). The Linux OpenSSL dependency is exactly the
  cross-platform and licensing friction the rest of the stack avoids. Skip it.
- **The crypto provider is the license question.** The `rustls` feature pulls the
  **aws-lc-rs** provider by default, whose license includes an OpenSSL-licensed
  component that is not in `.cargo/deny.toml`'s allow list. Whether that fails the
  gate depends on how `cargo deny` resolves the multi-part expression — don't assume
  either way. Verify, and if it fails, swap to the `ring` provider (see below).

The verification step is not optional: after you add reqwest to a crate, run
`cargo deny check --config .cargo/deny.toml` and confirm the license gate is green.
If the aws-lc-rs provider trips it, the honest fixes are to enable `rustls-no-provider`
and install `ring` as the process default crypto provider
(`rustls::crypto::ring::default_provider().install_default()` once at startup), or to
add a documented `exceptions` entry in deny.toml with a reason. Surface the choice;
don't silently weaken the gate (see CLAUDE.md). The deny.toml already allow-lists
`webpki-roots` (CDLA-Permissive-2.0) and the rest of the rustls stack, so the rustls
path was anticipated — the crypto provider is the one piece left to confirm. Provider
and root-store details are in `references/features-tls-multipart.md`.

## Errors: map at the boundary, classify, strip the URL

reqwest returns `reqwest::Error` for everything — connect failures, timeouts,
non-2xx (via `error_for_status`), decode errors. In a library crate (`io`, or the
AI/`core` module that owns backends) that becomes a `thiserror` enum at the crate
boundary; `anyhow` only in the binary, never `Box<dyn Error>` in a public API (see
[[pixhaus-rust-conventions]] and [[pixhaus-thiserror]]).

```rust
#[derive(Debug, thiserror::Error)]
pub enum BackendError {
    #[error("AI backend request timed out")]
    Timeout,
    #[error("AI backend returned HTTP {0}")]
    Status(reqwest::StatusCode),
    #[error("AI backend transport error")]
    Transport(#[source] reqwest::Error),
}

impl From<reqwest::Error> for BackendError {
    fn from(e: reqwest::Error) -> Self {
        if e.is_timeout() {
            Self::Timeout
        } else if let Some(status) = e.status() {
            Self::Status(status)
        } else {
            // strip the URL: it can carry an API key in the query string,
            // and this error may be logged or shown to the user.
            Self::Transport(e.without_url())
        }
    }
}
```

Two things worth internalizing. First, classify with the `is_*` predicates and
`status()` rather than matching on the `Display` string — `is_timeout()`,
`is_connect()`, `is_status()`, `is_decode()` are the stable API. Second,
`without_url()` exists precisely because reqwest errors embed the request URL, and
an AI endpoint URL can contain a key as a query parameter; strip it before the error
crosses into anything logged or surfaced. Full predicate list in
`references/responses-and-streaming.md`.

## Streaming responses (the `stream` feature)

Some AI backends stream tokens or progress as the response body arrives (SSE-style).
`Response::bytes_stream()` (needs the `stream` feature) turns the body into a
`futures::Stream` of `Bytes` chunks you drive with `StreamExt::next()` — see
[[pixhaus-futures]] for the stream combinators.

```rust
use futures::StreamExt;

let mut stream = client.post(url).bearer_auth(key).json(&body).send().await?
    .error_for_status()?
    .bytes_stream();

while let Some(chunk) = stream.next().await {
    let bytes = chunk?;                 // each item is Result<Bytes, reqwest::Error>
    // parse SSE lines / accumulate tokens, then send a partial update to the UI
    let _ = tx.send(Partial::from(&bytes)).await;
    ctx.request_repaint();              // wake the loop so the user sees progress
}
```

Streaming is where the channel-per-chunk pattern earns its keep: each chunk sends a
partial result and requests a repaint, so generated text or a progress bar updates
live instead of appearing all at once at the end. The lower-level `chunk(&mut self)`
pull loop needs no feature flag if you'd rather not depend on `futures`.

## What not to reach for

- **`reqwest::blocking` inside the runtime.** The blocking client panics if called
  from within an async context. Pixhaus has a runtime, so use the async `Client`. If
  you truly have a sync call site, it belongs in `spawn_blocking` ([[pixhaus-tokio]]),
  not the blocking client.
- **`Response::copy_to`.** It does not exist on the async `Response` — it's
  blocking-only. To save a downloaded asset, `bytes().await?` then write via
  `spawn_blocking` + `std::fs`, or stream chunks. Reaching for `copy_to` on an async
  response is a ported-from-blocking-code mistake that won't compile.
- **A fresh `Client` per request.** Throws away the pool. Build once, clone the
  handle. See fact 1 above.
- **`tokio::net` / raw sockets.** reqwest owns its networking on top of hyper; you
  spawn the request future, you don't open connections.
- **`danger_accept_invalid_certs(true)` to "fix" a TLS error.** That disables
  verification for everyone. A cert error against a real AI endpoint is a real
  problem (clock skew, a proxy, a missing root) — diagnose it, don't disable
  security. For a self-hosted Ollama/ComfyUI on localhost over plain HTTP, no TLS is
  involved at all.

## References

Open the file for the area you're working in. Each is a dense reqwest 0.13 reference
verified against docs.rs.

| File | Covers |
|---|---|
| `references/client-and-requests.md` | `Client` and the connection pool, `ClientBuilder` (timeouts, default headers, user agent, redirect, proxy, decompression), `RequestBuilder` (`header`/`headers`, `basic_auth`/`bearer_auth`, `json`/`form`/`query`, `body`, `timeout`, `build`/`send`/`try_clone`), the `header` module, building a URL |
| `references/responses-and-streaming.md` | `Response` (`status`/`headers`/`url`/`content_length`, `bytes`/`text`/`text_with_charset`/`json`, `error_for_status`(`_ref`)), the streaming APIs (`chunk`, `bytes_stream`), and the full `Error` inspection surface (every `is_*` predicate, `status`, `url`, `without_url`) |
| `references/features-tls-multipart.md` | the complete cargo feature table, the TLS/crypto-provider/root-store matrix and its `cargo deny` consequences, the `multipart` module (`Form`, `Part`, the async `Form::file`), `redirect::Policy`, and the `blocking` module caveat |

A standing caution: signatures were verified at reqwest 0.13.4. The 0.13 line moved
feature names once already, so when a feature flag or a deep signature is
load-bearing, confirm it against https://docs.rs/reqwest/latest/ before depending on
it — and always re-run `cargo deny` after touching the dependency.
