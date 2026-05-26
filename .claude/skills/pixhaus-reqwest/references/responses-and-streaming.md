# Responses, streaming, and errors

Verified against reqwest 0.13.4. This file covers reading a `Response`, streaming a
body chunk by chunk, and the full `Error` inspection surface. For building the request
see `client-and-requests.md`; for features and TLS see `features-tls-multipart.md`.

## Contents

- `Response` — status, headers, metadata (sync)
- Reading the body: `bytes`, `text`, `json`
- `error_for_status` — turning 4xx/5xx into `Err`
- Streaming: `chunk` and `bytes_stream`
- `Error` — every inspection method
- The error-handling pattern for Pixhaus

## `Response` — status, headers, metadata (sync)

These do not touch the body and are not async:

```rust
pub fn status(&self) -> StatusCode               // the HTTP status code
pub fn headers(&self) -> &HeaderMap              // response headers
pub fn content_length(&self) -> Option<u64>      // None if the server didn't advertise one
pub fn url(&self) -> &Url                        // the FINAL url, after any redirects
pub fn version(&self) -> Version                 // HTTP version negotiated
```

`status()` returns a `StatusCode` you can compare (`resp.status().is_success()`,
`== StatusCode::TOO_MANY_REQUESTS`). Reading a header like `retry-after` before deciding
to back off is a common pattern with rate-limited AI APIs:

```rust
if resp.status() == StatusCode::TOO_MANY_REQUESTS {
    let retry_after = resp.headers()
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok());
    // schedule a retry after `retry_after` seconds
}
```

## Reading the body: `bytes`, `text`, `json`

Each consumes the `Response` (you get the body once) and is async:

```rust
pub async fn bytes(self) -> Result<Bytes>                       // raw body as bytes::Bytes
pub async fn text(self) -> Result<String>                       // UTF-8 (or charset-detected) string
pub async fn text_with_charset(self, default: &str) -> Result<String>  // feature: charset
pub async fn json<T: DeserializeOwned>(self) -> Result<T>       // feature: json; deserialize
```

- **`json::<T>()`** is the AI-response workhorse: deserialize straight into a struct
  deriving `Deserialize` (see [[pixhaus-serde]]). It reads the whole body, then parses.
- **`bytes()`** for binary payloads — a generated PNG/sprite returned inline. Hand the
  `Bytes` to the image decode path; if you write it to disk, do the write in
  `spawn_blocking` with `std::fs` (see [[pixhaus-tokio]]), not on the request task.
- **`text()`** for plain-text or when you want to log/inspect a raw body. With the
  `charset` feature it honors the `Content-Type` charset; `text_with_charset` lets you
  name a fallback when the server omits one.

## `error_for_status` — turning 4xx/5xx into `Err`

A 4xx or 5xx response is a *successful* HTTP exchange as far as `send()` is concerned —
the `Result` from `send().await` is `Ok`. To treat a bad status as an error, call:

```rust
pub fn error_for_status(self) -> Result<Self>        // consumes; Err on 4xx/5xx
pub fn error_for_status_ref(&self) -> Result<&Self>  // borrows; same check, keeps the body
```

Order matters. Check status *before* reading the body as your success type:

```rust
let resp = client.post(url).bearer_auth(key).json(&req).send().await?;
let parsed: GenResponse = resp
    .error_for_status()?          // 4xx/5xx → Err here, with the status attached
    .json()                       // only parse the body if status was 2xx
    .await?;
```

If you call `.json::<GenResponse>()` on a 400 response, reqwest tries to deserialize the
*error page* into `GenResponse` and you get a confusing decode error instead of "HTTP
400". When you need the error body for diagnostics (many AI APIs return a useful JSON
error), use `error_for_status_ref()` so the `Response` survives, then read the body on
the error path:

```rust
let resp = client.post(url).json(&req).send().await?;
if let Err(status_err) = resp.error_for_status_ref() {
    let detail = resp.text().await.unwrap_or_default();  // read the error body
    return Err(BackendError::Api { status: status_err.status(), detail });
}
let parsed: GenResponse = resp.json().await?;
```

## Streaming: `chunk` and `bytes_stream`

Two ways to consume a body incrementally — for SSE-style token streaming or a large
download where you want progress.

```rust
pub async fn chunk(&mut self) -> Result<Option<Bytes>>      // pull one chunk; Ok(None) at EOF; no feature
pub fn bytes_stream(self) -> impl Stream<Item = Result<Bytes>>  // feature: stream
```

`chunk(&mut self)` is the no-dependency pull loop — it needs no feature flag and no
`futures` import:

```rust
let mut resp = client.post(url).json(&req).send().await?.error_for_status()?;
while let Some(chunk) = resp.chunk().await? {
    // chunk is bytes::Bytes; parse SSE lines, accumulate, push a partial to the UI
}
```

`bytes_stream()` (feature `stream`) turns the body into a `futures::Stream` you drive
with the combinators from [[pixhaus-futures]]:

```rust
use futures::StreamExt;
let mut stream = resp.bytes_stream();
while let Some(item) = stream.next().await {
    let chunk = item?;                 // each item is Result<Bytes>
    let _ = tx.send(Partial::from(&chunk)).await;
    ctx.request_repaint();             // wake the egui loop so progress shows live
}
```

Both deliver `bytes::Bytes`. Use `bytes_stream` when you want to compose with other
stream combinators (buffering, `map`, `take_until` a cancellation); use `chunk` for a
plain loop with no extra dependency. Either way, each chunk should send a partial result
over the channel and request a repaint, so streamed text or a progress bar updates as it
arrives rather than appearing all at once at the end.

Note: `Response::copy_to` does **not** exist on the async response — it is blocking-only
(see `features-tls-multipart.md`). To save a stream to disk, accumulate or forward
chunks and write via `spawn_blocking` + `std::fs`.

## `Error` — every inspection method

`reqwest::Error` covers the whole failure surface. Classify with the predicates, not by
matching on the `Display` string:

```rust
pub fn is_body(&self) -> bool
pub fn is_builder(&self) -> bool      // malformed URL/header at build time
pub fn is_connect(&self) -> bool      // could not establish a connection
pub fn is_decode(&self) -> bool       // body failed to decode/deserialize
pub fn is_redirect(&self) -> bool     // redirect policy error (e.g. too many)
pub fn is_request(&self) -> bool
pub fn is_status(&self) -> bool       // came from error_for_status (4xx/5xx)
pub fn is_timeout(&self) -> bool      // request/connect/read timed out
pub fn is_upgrade(&self) -> bool

pub fn status(&self) -> Option<StatusCode>   // Some only when is_status()
pub fn url(&self) -> Option<&Url>
pub fn url_mut(&mut self) -> Option<&mut Url>
pub fn with_url(self, url: Url) -> Self
pub fn without_url(self) -> Self             // strip the URL (it may carry an API key)
```

There is **no** `is_client()` / `is_server()` split — classify a status error with
`is_status()` and then read `status()` to see whether it's 4xx or 5xx. The nine `is_*`
predicates above are the complete set.

## The error-handling pattern for Pixhaus

Map `reqwest::Error` into a `thiserror` enum at the crate boundary (see
[[pixhaus-thiserror]] and [[pixhaus-rust-conventions]]); never expose `reqwest::Error`
or `Box<dyn Error>` in a public API of `io`/`core`.

```rust
#[derive(Debug, thiserror::Error)]
pub enum BackendError {
    #[error("request to the AI backend timed out")]
    Timeout,
    #[error("could not reach the AI backend")]
    Connect,
    #[error("AI backend returned HTTP {0}")]
    Status(reqwest::StatusCode),
    #[error("could not decode the AI backend response")]
    Decode(#[source] reqwest::Error),
    #[error("AI backend transport error")]
    Transport(#[source] reqwest::Error),
}

impl From<reqwest::Error> for BackendError {
    fn from(e: reqwest::Error) -> Self {
        if e.is_timeout() {
            Self::Timeout
        } else if e.is_connect() {
            Self::Connect
        } else if let Some(status) = e.status() {
            Self::Status(status)
        } else if e.is_decode() {
            Self::Decode(e.without_url())
        } else {
            // strip the URL before storing/logging: an endpoint URL can carry a key
            // in its query string, and this error may be surfaced to the user.
            Self::Transport(e.without_url())
        }
    }
}
```

The two habits that matter: classify with the predicates (a timeout and a 500 are
different user-facing problems and deserve different messages and retry behavior), and
call `without_url()` before any error crosses into logging or the UI, because the
request URL can embed a secret. Map distinct cases to distinct variants so the egui
layer can show "the model is busy, try again" for a 503 and "check your API key" for a
401, rather than one opaque "request failed".
