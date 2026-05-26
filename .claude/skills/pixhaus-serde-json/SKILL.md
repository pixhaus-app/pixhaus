---
name: pixhaus-serde-json
description: >
  Use when reading or writing JSON in Pixhaus with `serde_json` — above all the
  AI backend adapters (Anthropic, OpenAI, Replicate, Ollama, ComfyUI, Stability),
  whose request and response bodies are JSON over HTTP, plus any JSON config,
  plugin manifest, or interop blob. Trigger this for ANY "build the request body",
  "parse the API response", "deserialize this JSON", "what does the model return",
  "handle the streaming SSE chunks", "read this config file", or "round-trip this
  to JSON" request, and whenever you see `serde_json`, `from_str`, `from_slice`,
  `to_string`, `to_vec`, `json!`, `Value`, `RawValue`, or a `#[derive(Deserialize)]`
  on an API payload type. JSON is the AI wire format; it is NOT the `.pixhaus`
  project file (that is MessagePack via rmp-serde — see [[pixhaus-rmp-serde]]).
  serde_json has traps that bite the adapter layer — `json!` panics, square-bracket
  indexing silently returns Null, `from_reader` blocks until EOF — so reach for this
  skill rather than relying on memory.
---

# serde_json for Pixhaus

serde_json is the Serde data format for JSON. In Pixhaus it has one dominant job:
talking to the AI backends. Every adapter — Anthropic, OpenAI, Replicate, Ollama,
ComfyUI, Stability — speaks JSON over HTTP, so serde_json is how the adapter layer
builds request bodies and parses responses. It also reads any JSON config or plugin
manifest. It is the most ergonomic Serde format because JSON is human-readable and
the `json!` macro lets you write literals inline.

It is **not** the project file format. The `.pixhaus` file is MessagePack + zstd
(`rmp-serde`, see [[pixhaus-rmp-serde]] and [[pixhaus-zstd]]). Don't reach for
serde_json to save a document — JSON is bulkier, lossy on binary (pixel buffers
become integer arrays), and slower at the 8K scale the project targets
([[8k-perf-constraint]]). serde_json is for the wire and for human-facing config,
not for bulk persistence.

For the exhaustive API — every function signature, the full `Value`/`Number`/`Map`
surface, the error and streaming types, the custom-formatter machinery — read
`references/api-reference.md`. This file is the decisions and the patterns.

## Version and license

| Crate | Version | License | MSRV |
|---|---|---|---|
| `serde_json` | 1.0.150 | `MIT OR Apache-2.0` | 1.56 (crate policy) |

The dual license includes MIT, so serde_json passes the workspace MIT lock and
`cargo deny`. Pair it with `serde` for the derive macros.

```toml
serde_json = "1"
serde      = { version = "1", features = ["derive"] }
```

Only `std` is on by default. The optional features that matter in Pixhaus —
`preserve_order`, `raw_value`, `arbitrary_precision` — are off until you ask for
them (see [Features](#features-turn-on-only-what-you-need) below).

## Decision 1: typed structs first, `Value` only for genuinely dynamic JSON

This is the call you make on every adapter. serde_json gives you two ways to handle
a payload, and the convenient one is the wrong default for code you maintain:

- **Typed structs** — define a `#[derive(Serialize, Deserialize)]` struct that
  mirrors the API shape, then `from_slice`/`to_vec` against it. The compiler checks
  field names and types, missing required fields fail loudly with a line/column, and
  the struct *is* the documentation of what that endpoint returns.
- **`Value` / `json!`** — the untyped tree. Quick to write, but every field access
  is a runtime `Option` or a silent `Null`, typos compile, and the shape lives only
  in your head.

**Model the parts of the API you depend on as structs.** An Anthropic or OpenAI
response has a stable, documented shape; encode it once and every adapter reads it
type-safely:

```rust
use serde::Deserialize;

#[derive(Deserialize)]
struct ChatResponse {
    id: String,
    model: String,
    #[serde(default)]                 // tolerate older/leaner responses
    usage: Usage,
    content: Vec<ContentBlock>,
}

// Body is already in memory from reqwest — borrow from it, don't stream it.
let resp: ChatResponse = serde_json::from_slice(&body_bytes)?;
```

**Reach for `Value` only where the JSON is genuinely open-ended** and you can't know
the shape ahead of time: a ComfyUI workflow graph whose node set the user defines, a
provider's free-form `metadata` bag, or a field that differs per backend. Even then,
prefer a typed struct with one `Value` field rather than making the whole payload
untyped:

```rust
#[derive(Deserialize)]
struct NodeResult {
    node_id: String,
    outputs: serde_json::Value,       // shape varies per node type — keep this dynamic
}
```

If you find yourself writing `resp["choices"][0]["message"]["content"]`, that's the
signal you should have defined a struct. The mixed approach — typed envelope, `Value`
for the one unknowable field — is almost always the right shape in this codebase.

## Decision 2: build request bodies without `json!` in library code

`json!` is delightful in tests and examples and a hazard in library code, because it
**panics**. It panics if an interpolated value's `Serialize` impl fails, and if you
interpolate a map with non-string keys. Pixhaus forbids `panic!`/`unwrap` outside
tests ([[pixhaus-rust-conventions]]), so a `json!` on a request-building path is a
clippy-and-review problem waiting to happen.

In the adapter crates, build bodies from typed structs and serialize fallibly:

```rust
#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    max_tokens: u32,
    messages: &'a [Message],
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<&'a str>,          // omit the key entirely when None
}

let body = serde_json::to_vec(&req)?; // Result, not a panic; hand straight to reqwest
```

If you truly need to assemble a `Value` at runtime, use `serde_json::to_value(x)?` —
it returns a `Result` where `json!` would have panicked. Save `json!` for test
fixtures and the rare const-ish literal where you've reasoned that nothing can fail.

The other serialization trap to remember: **a map with non-string keys fails at
runtime, not compile time.** `serde_json::to_string(&HashMap<i32, _>)` returns an
`Err` (and `json!` with such a map panics). JSON object keys are strings; key your
maps with `String`/`&str`, or use a `Vec` of pairs.

## Decision 3: borrow the body, don't stream it — `from_slice`, not `from_reader`

The three deserialize entry points are not interchangeable:

- **`from_str` / `from_slice`** — parse from a `&str` / `&[u8]` already in memory.
  Can borrow directly from the input (zero-copy `&str` fields when the target allows).
  This is what you want for an HTTP response: reqwest hands you the full body as
  `Bytes`/`String`, so `from_slice(&bytes)` is both the simplest and the fastest path.
- **`from_reader`** — parse from an `impl Read`. The docs are explicit that it is
  **usually slower** than reading to a `String`/`Vec` and calling `from_slice`, and it
  **does not return until the stream hits EOF**. On a socket that stays open (a
  streaming response) it will hang forever. Use it only for a file or pipe you read to
  completion, and wrap unbuffered sources in `BufReader`.

```rust
// Right: body already buffered by the HTTP client.
let parsed: T = serde_json::from_slice(resp_bytes)?;

// Wrong on a streaming endpoint: from_reader waits for EOF that never comes.
```

**Streaming responses (SSE) are line-by-line, not one big parse.** Anthropic and
OpenAI stream tokens as Server-Sent Events: each chunk is a `data: {json}\n` line.
Strip the `data: ` prefix and parse each line's JSON with `from_str`/`from_slice`
individually — one `Deserialize` call per event. Don't point `from_reader` at the SSE
body, and don't reach for `StreamDeserializer` (that's for *concatenated* JSON values
with no framing; SSE has its own `data:`/`event:` framing that you must peel off
first). See the streaming section of the reference for the resumable
`StreamDeserializer` pattern when you do have raw concatenated JSON (NDJSON logs).

## Decision 4: `.get()` to probe, `[ ]` only for known-present chained access

When you do hold a `Value`, square-bracket indexing has a sharp edge worth internalizing:

> Indexing a `Value` with `[ ]` returns `Value::Null` wherever `.get()` would have
> returned `None` — a missing key, an out-of-bounds index, or indexing into a `Null`.
> It does **not** panic for those. (It *does* panic if you index into a scalar — a
> string, number, or bool — since those can't be indexed at all.)

That means `v["a"]["b"]["c"]` never tells you *where* it went wrong; any miss in the
chain collapses the rest to `Null`, and a present-but-`null` field is indistinguishable
from an absent one. So:

- Use **`.get("key")`** (returns `Option<&Value>`) when you need to know whether a
  field is present, or to branch on optional data. This is the safe default for
  probing an untyped response.
- Use **`[ ]`** only for ergonomic traversal of structure you're confident exists, and
  where "missing collapses to Null" is the behavior you want.

```rust
// Probing optional/unknown structure → get(), so absent is distinguishable.
if let Some(usage) = value.get("usage").and_then(|u| u.get("output_tokens")) {
    record_tokens(usage.as_u64().unwrap_or(0));
}

// Known-present chain where a miss should just be Null → indexing is fine.
let first = &response["content"][0];
```

The `as_*` accessors (`as_str`, `as_u64`, `as_f64`, `as_array`, `as_object`, …) all
return `Option` and return `None` on a type mismatch — thread them with `?`/`ok_or`
into a `thiserror` variant rather than unwrapping. Numbers have a subtlety:
`Value::is_f64` is true only for *non-integer* numbers, so test with `as_f64()` (which
yields a value for integers too) rather than `is_f64()` when you just need a float.

## Errors: wrap `serde_json::Error`, surface line and column

`serde_json::Error` carries the **line and column** where parsing failed and a
`classify()` into `Category::{Io, Syntax, Data, Eof}`. That position is gold when an
adapter gets a body it didn't expect — log it. Wrap it in your crate's `thiserror`
enum and propagate with `?`; never `unwrap` a parse on a network boundary, where
malformed input is a *when*, not an *if*.

```rust
#[derive(Debug, thiserror::Error)]
pub enum AdapterError {
    #[error("HTTP transport")]
    Http(#[from] reqwest::Error),
    #[error("decoding {provider} response as JSON")]
    Json {
        provider: &'static str,
        #[source]
        source: serde_json::Error,
    },
}
```

`Category::Data` means "valid JSON, wrong shape for your type" (a required field was
missing or a number didn't fit) — usually a struct/API drift bug on your side.
`Category::Syntax` means the bytes weren't JSON at all — often an HTML error page or a
rate-limit body the provider returned with a 200. Distinguishing them in logs saves a
debugging session. See the reference for the full method list (`line`, `column`,
`classify`, `is_data`, `io_error_kind`, …).

## Features: turn on only what you need

Default (`std` only) is right for almost all adapter code. Reach for an optional
feature only when a specific need forces it:

| Feature | Turn on when | Cost |
|---|---|---|
| `preserve_order` | You must hand a JSON object back in its original key order — e.g. round-tripping a ComfyUI workflow you received and must return unchanged. | Pulls in `indexmap`; backs `Map` with it instead of `BTreeMap`. Affects the whole crate. |
| `raw_value` | You want to forward a JSON subtree verbatim without parsing it (capture a provider's opaque blob, pass a node graph through untouched). Enables `RawValue`. | `RawValue` is `!Sized` — always `&RawValue` or `Box<RawValue>`. |
| `arbitrary_precision` | A number must round-trip with exact precision beyond i64/u64/f64 range. Rare for AI APIs. | Changes `Number` internals; known to interact badly with `#[serde(flatten)]` and untagged enums. |
| `float_roundtrip` | f64 values must reparse to the identical bits. | Slightly slower parsing. |

`unbounded_depth` removes the recursion-depth guard — **do not enable it for adapter
input.** Untrusted deeply-nested JSON is a stack-overflow DoS; the default depth limit
is the protection.

By default, JSON object keys come out **sorted** (the `Map` is a `BTreeMap`). If you
serialize a `Value` and the key order looks "wrong", that's why — enable
`preserve_order` only if order is actually load-bearing.

## Modeling payloads: lean on serde derive attributes

The shape work happens in `serde` derive attributes, not in serde_json itself — see
[[pixhaus-serde]] for the full attribute set and the derive patterns. The ones you'll
use constantly on AI payloads:

- `#[serde(rename_all = "snake_case")]` / `"camelCase"` — match the provider's casing
  without renaming every Rust field.
- `#[serde(default)]` — tolerate fields a leaner or older response omits.
- `#[serde(skip_serializing_if = "Option::is_none")]` — omit optional request fields
  rather than sending `null`.
- `#[serde(tag = "type")]` on an enum — internally-tagged variants, the natural fit
  for SSE event streams (`{"type": "content_block_delta", ...}`) and for content
  blocks that vary by `type`.
- `#[serde(rename = "...")]` — pin a field name that isn't a legal Rust identifier
  (`"in"`, `"type"`) or doesn't follow the casing rule.

These belong to serde, not serde_json — but they're how every well-typed adapter
payload is built, so write the struct with them and let serde_json do the encoding.

## Testing JSON code

- **Round-trip** every payload type: `to_vec` then `from_slice` and assert equality.
  Catches a `rename`/casing mistake immediately.
- **Pin the wire format** with `insta` snapshots of `to_string_pretty` output, so an
  accidental field rename or reorder shows up as a reviewable diff
  ([[pixhaus-testing-conventions]]).
- **Decode a real captured response.** Check in a fixture of an actual provider body
  and assert your struct parses it. When the struct drifts from the API, that test
  fails loudly instead of in production.
- `json!` is welcome *in tests* — it's the cleanest way to write an expected `Value`.

## Decision shortcut

```
Working with JSON via serde_json?
├─ Is this the .pixhaus project file or a bulk/binary blob?
│    └─ yes → wrong tool. Use rmp-serde + zstd ([[pixhaus-rmp-serde]], [[pixhaus-zstd]]).
├─ Do you know the payload's shape (a documented API request/response)?
│    └─ yes → #[derive(Serialize/Deserialize)] struct + from_slice / to_vec. Typed wins.
├─ Is the shape genuinely open-ended (ComfyUI graph, free-form metadata)?
│    └─ yes → Value, but keep it to the one dynamic field inside a typed envelope.
├─ Building a request body in library code?
│    └─ to_vec(&struct)? — never json! (it panics; library code is no-panic).
├─ Parsing an HTTP response body already in memory?
│    └─ from_slice(&bytes). NOT from_reader (slower, blocks until EOF).
├─ Consuming a streaming SSE response?
│    └─ split on lines, strip "data: ", from_str each event. Not from_reader, not StreamDeserializer.
├─ Probing whether a field exists on a Value?
│    └─ .get() (Option), not [ ] (collapses misses to Null, can't tell absent from null).
└─ Forwarding a subtree verbatim without parsing? → RawValue (feature "raw_value").
```
