# Client and requests

Verified against reqwest 0.13.4. This file covers building and reusing the `Client`,
configuring it with `ClientBuilder`, and assembling a request with `RequestBuilder`.
For reading the response and handling errors see `responses-and-streaming.md`; for
feature flags, TLS, multipart, and redirects see `features-tls-multipart.md`.

## Contents

- `Client` — the shared connection pool
- `ClientBuilder` — configuration
- `RequestBuilder` — assembling a request
- Headers and auth
- Bodies: `json`, `form`, `query`, `body`
- `build`, `send`, `try_clone`
- The `header` module
- Building a URL

## `Client` — the shared connection pool

```rust
pub fn new() -> Client                                   // convenience; panics on TLS/init failure
pub fn builder() -> ClientBuilder                        // fallible, configurable construction
pub fn get<U: IntoUrl>(&self, url: U) -> RequestBuilder
pub fn post<U: IntoUrl>(&self, url: U) -> RequestBuilder
pub fn put<U: IntoUrl>(&self, url: U) -> RequestBuilder
pub fn patch<U: IntoUrl>(&self, url: U) -> RequestBuilder
pub fn delete<U: IntoUrl>(&self, url: U) -> RequestBuilder
pub fn head<U: IntoUrl>(&self, url: U) -> RequestBuilder
pub fn request<U: IntoUrl>(&self, method: Method, url: U) -> RequestBuilder
pub fn execute(&self, request: Request) -> impl Future<Output = Result<Response>>
```

The docs are explicit on reuse: *"You do not have to wrap the `Client` in an `Rc` or
`Arc` to reuse it, because it already uses an `Arc` internally."* And: *"The `Client`
holds a connection pool internally, so it is advised that you create one and reuse
it."*

In Pixhaus that means: build one `Client` in the module that owns the AI backends,
store it there, and hand a `.clone()` to each backend adapter. Cloning is a cheap
`Arc` bump that shares the same pool, keep-alive connections, and TLS session cache.
Never build a client per request — you throw away connection reuse and pay a fresh
TCP + TLS handshake every time, which on a multi-turn AI session is the difference
between snappy and sluggish.

`Client::new()` panics if the TLS backend fails to initialize. Prefer
`Client::builder().…​.build()?` so that failure is a `Result` you handle, in keeping
with the no-`unwrap`/no-`panic` rule (see [[pixhaus-rust-conventions]]) — the one
allowed exception is binary `main` setup code, where an `.expect("build http client")`
at startup is defensible.

`IntoUrl` is implemented for `&str`, `String`, and `url::Url`. A malformed URL does
not panic — it surfaces as a builder error when you `send()`.

## `ClientBuilder` — configuration

Every method consumes `self` and returns `ClientBuilder`, so they chain. `build()`
returns a `Result`.

```rust
pub fn build(self) -> Result<Client>

// timeouts — set both; an AI generation call can legitimately run for a minute,
// but a stuck connect should fail fast.
pub fn timeout(self, timeout: Duration) -> ClientBuilder          // whole request (connect + body)
pub fn connect_timeout(self, timeout: Duration) -> ClientBuilder  // connect phase only
pub fn read_timeout(self, timeout: Duration) -> ClientBuilder     // per-read inactivity

// connection pool
pub fn pool_idle_timeout<D: Into<Option<Duration>>>(self, val: D) -> ClientBuilder
pub fn pool_max_idle_per_host(self, max: usize) -> ClientBuilder

// headers applied to every request from this client
pub fn default_headers(self, headers: HeaderMap) -> ClientBuilder
pub fn user_agent<V: TryInto<HeaderValue>>(self, value: V) -> ClientBuilder

// behavior
pub fn redirect(self, policy: redirect::Policy) -> ClientBuilder  // see features-tls-multipart.md
pub fn https_only(self, enabled: bool) -> ClientBuilder
pub fn http1_only(self) -> ClientBuilder
pub fn http2_prior_knowledge(self) -> ClientBuilder               // feature: http2

// response decompression (each feature-gated; off unless enabled)
pub fn gzip(self, enable: bool) -> ClientBuilder                  // feature: gzip
pub fn brotli(self, enable: bool) -> ClientBuilder               // feature: brotli
pub fn deflate(self, enable: bool) -> ClientBuilder              // feature: deflate
pub fn zstd(self, enable: bool) -> ClientBuilder                 // feature: zstd

// proxy
pub fn proxy(self, proxy: Proxy) -> ClientBuilder
pub fn no_proxy(self) -> ClientBuilder

// DEPRECATED in 0.13 — do not reach for these; configure TLS via features instead
pub fn use_rustls_tls(self) -> ClientBuilder                     // deprecated
pub fn use_native_tls(self) -> ClientBuilder                     // deprecated
pub fn add_root_certificate(self, cert: Certificate) -> ClientBuilder  // deprecated
pub fn danger_accept_invalid_certs(self, v: bool) -> ClientBuilder     // deprecated; never use
```

Pixhaus guidance:

- **Set a per-backend timeout.** A `Stability` image generation can take 30–60s; a
  `Ollama` token stream may run longer. Pick the `timeout` to match the slowest sane
  response for that backend, and always pair it with a short `connect_timeout` so a
  dead host fails in seconds, not after the full request budget.
- **Put the API key in `default_headers` only if it's per-client.** If one client
  serves one backend with one key, `default_headers` is clean. If a client is shared
  across keys, set `bearer_auth` per request instead.
- **Decompression is opt-in.** If a backend sends gzip/brotli bodies and you want them
  transparently decoded, enable the matching feature *and* call `.gzip(true)`. Without
  the feature flag the builder method does not exist.

## `RequestBuilder` — assembling a request

```rust
pub fn header<K, V>(self, key: K, value: V) -> RequestBuilder
    where HeaderName: TryFrom<K>, HeaderValue: TryFrom<V>
pub fn headers(self, headers: HeaderMap) -> RequestBuilder
pub fn basic_auth<U, P>(self, username: U, password: Option<P>) -> RequestBuilder
    where U: fmt::Display, P: fmt::Display
pub fn bearer_auth<T: fmt::Display>(self, token: T) -> RequestBuilder
pub fn body<T: Into<Body>>(self, body: T) -> RequestBuilder
pub fn timeout(self, timeout: Duration) -> RequestBuilder        // per-request override
pub fn version(self, version: Version) -> RequestBuilder
pub fn build(self) -> Result<Request>                            // build without sending
pub fn send(self) -> impl Future<Output = Result<Response>>      // consume + send; await it
pub fn try_clone(&self) -> Option<RequestBuilder>                // None if the body can't be cloned

// feature-gated body/query helpers
pub fn json<T: Serialize + ?Sized>(self, json: &T) -> RequestBuilder    // feature: json
pub fn form<T: Serialize + ?Sized>(self, form: &T) -> RequestBuilder    // feature: form
pub fn query<T: Serialize + ?Sized>(self, query: &T) -> RequestBuilder  // feature: query
pub fn multipart(self, multipart: Form) -> RequestBuilder               // feature: multipart
```

## Headers and auth

For one or two headers, chain `.header(...)`. The key accepts anything convertible to
`HeaderName` (a `&str` or a `header::` constant), the value anything convertible to
`HeaderValue`.

```rust
use reqwest::header::{CONTENT_TYPE, ACCEPT};

client.post(url)
    .header(CONTENT_TYPE, "application/json")
    .header("anthropic-version", "2023-06-01")   // custom header by &str name
    .header(ACCEPT, "application/json")
```

Auth helpers set the `Authorization` header for you:

- `bearer_auth(token)` → `Authorization: Bearer <token>`. This is the right call for
  almost every AI backend (OpenAI, Anthropic via `x-api-key` is the exception —
  Anthropic uses a custom header, so use `.header("x-api-key", key)` there).
- `basic_auth(user, Some(pass))` → `Authorization: Basic <base64>`. Rare for AI APIs;
  shows up for some self-hosted ComfyUI behind a reverse proxy.

For headers that apply to *every* request from a client (a fixed API version, a user
agent), build a `HeaderMap` once and pass it to `ClientBuilder::default_headers` rather
than repeating `.header(...)` at each call site.

## Bodies: `json`, `form`, `query`, `body`

- **`json(&T)`** (feature `json`): serializes `T` with serde_json and sets
  `Content-Type: application/json`. Takes a *reference*. This is the workhorse for AI
  request payloads — see [[pixhaus-serde]] for deriving `Serialize` on the request
  struct and [[pixhaus-serde-json]] for the serializer.
- **`form(&T)`** (feature `form`): serializes `T` as
  `application/x-www-form-urlencoded`. Needed by a few backends' auth/token endpoints.
- **`query(&T)`** (feature `query`): appends serialized fields to the URL query string.
  Use for pagination/filter params, not secrets — query strings leak into logs.
- **`body(impl Into<Body>)`**: a raw body — a `String`, `Vec<u8>`, `&'static [u8]`, or
  (with `stream`) a streaming `Body`. Use when you've already serialized, or for
  non-JSON payloads.

`json`, `form`, and `query` each require their feature flag in 0.13. If a call to
`.query(...)` or `.form(...)` fails to compile with "no method named", the feature is
the cause, not your code.

## `build`, `send`, `try_clone`

- **`send().await`** consumes the builder and returns `Result<Response>`. The `Err` at
  this point is a transport/connect/timeout error; a 4xx/5xx is *not* an error yet —
  the request succeeded, the server answered. Convert status to an error with
  `Response::error_for_status()` (see `responses-and-streaming.md`).
- **`build()`** produces a `Request` without sending, for inspection or for
  `Client::execute`. Rarely needed.
- **`try_clone()`** returns `Option<RequestBuilder>` — `None` when the body is a
  non-replayable stream. This is the building block for a retry: clone before the first
  `send`, and on a retryable failure (timeout, 503) send the clone. A streaming body
  can't be cloned, so retry logic over a streaming upload has to rebuild the request.

## The `header` module

`reqwest::header` re-exports `http::header`. Standard names are constants; build a map
when you need several:

```rust
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};

let mut headers = HeaderMap::new();
headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
// from_static is const-checked and never allocates; for a runtime string:
headers.insert(AUTHORIZATION, HeaderValue::from_str(&format!("Bearer {key}"))?);
```

`HeaderValue::from_static` panics on an invalid value, but only accepts `&'static str`,
so the check is effectively at compile time for literals. For runtime strings use
`from_str`, which returns `Result<HeaderValue, InvalidHeaderValue>` — propagate the
error, don't unwrap.

## Building a URL

reqwest re-exports `url::Url`. For a fixed endpoint, pass the `&str` straight to
`client.post("https://…")`. When you assemble a path from parts, build a `Url` and use
`join`/`query_pairs_mut` rather than `format!`, so escaping is handled:

```rust
let mut url = reqwest::Url::parse(base)?;        // base endpoint
url.set_path("/v1/images/generations");
url.query_pairs_mut().append_pair("model", model);
client.post(url)                                  // Url implements IntoUrl
```

`format!`-ing a URL with a user-supplied segment risks an injection or a broken escape;
`Url` does the right thing.
