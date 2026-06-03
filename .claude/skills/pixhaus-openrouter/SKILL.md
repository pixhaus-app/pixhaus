---
name: pixhaus-openrouter
description: >
  Use when working with the OpenRouter generation provider in Pixhaus — the first
  real AI backend behind the capability registry (`pixhaus-mod-providers`). Trigger
  this for any "generate a sprite / anchor / idle animation for real", "call the
  OpenRouter API", "the real provider", "image generation request", "set modalities
  / image_config / aspect_ratio", "attach the anchor as a reference image",
  "image-to-image", "decode the generated PNG", "read `message.images`", "build the
  `ChatCompletionRequest`", "pick the model slug / Gemini Nano Banana", or "the
  OpenRouter key" task — even when the user doesn't say OpenRouter. The provider
  lives in `modules/providers/src/openrouter.rs`, implements the `Provider` trait,
  and returns a `GeneratedResult`. `openrouter-rs` 0.10's image-generation surface
  is NOT in most training data (image output via `modalities` + `image_config` +
  `message.images` is recent), so reach for this rather than memory. For the HTTP
  layer underneath, that's [[pixhaus-reqwest]]; for decoding the returned PNG,
  [[pixhaus-image]]; for where the request future runs, [[pixhaus-tokio]]; for
  moving the key off the environment into the OS vault later, [[pixhaus-keyring]].
---

# OpenRouter for Pixhaus

OpenRouter is the first real generation backend. It sits behind the capability
registry: the app asks "who can generate an anchor / an idle animation?" and the
`OpenRouterProvider` answers when an API key is present, otherwise the offline
`MockProvider` does. The provider ports the proven two-pass sprite pipeline — an
anchor (a neutral character on a flat magenta key), then an idle-animation sheet
conditioned on that anchor — onto OpenRouter's image-output chat models (Gemini).

It is a `Provider` like any other (object-safe, boxed-future `generate`, a
`CancellationToken`), so every rule from the provider boundary applies: it receives
immutable input and returns a `GeneratedResult`, never touches the live document,
and runs on the binary's one tokio runtime. `openrouter-rs` wraps reqwest, so
[[pixhaus-reqwest]] and [[pixhaus-tokio]] both apply underneath.

## Versions — pin in lockstep

| Crate | Version | License |
|---|---|---|
| `openrouter-rs` | 0.10.x (API verified at 0.10.0) | MIT |
| `base64` | 0.22.x | MIT OR Apache-2.0 |
| `image` | 0.25 (png feature) | MIT OR Apache-2.0 |

`openrouter-rs` 0.10's image-generation surface drifts across minors — confirm any
signature against docs.rs at the pinned version before depending on it. It pulls
`reqwest` + a `rustls` TLS stack transitively; `cargo deny` (MIT lock) passes at the
pinned versions, but re-check on a bump. The model slug is the other moving part:
image-output Gemini slugs change, so the provider takes the slug from
`PIXHAUS_OPENROUTER_MODEL` and falls back to a default — do not hardcode a slug as
the only option.

## The verified 0.10 image-generation surface

This is the exact shape `modules/providers/src/openrouter.rs` uses. It compiled and
the request-shaping is unit-tested.

### Build the client

```rust
use openrouter_rs::OpenRouterClient;

let client = OpenRouterClient::builder()
    .api_key(api_key)          // String; the app reads OPENROUTER_API_KEY, never logs it
    .build()?;                 // Result<_, OpenRouterError>
```

### Build the request (text-only anchor, and image-to-image idle)

```rust
use openrouter_rs::api::chat::{ChatCompletionRequest, ContentPart, Message, Modality};
use openrouter_rs::types::Role;

// Anchor: text only.
let messages = vec![Message::new(Role::User, prompt)];

// Idle: multi-part — the prompt plus the anchor as a base64 data URL (image-to-image).
let messages = vec![Message::with_parts(Role::User, vec![
    ContentPart::text(prompt),
    ContentPart::image_url(format!("data:image/png;base64,{b64}")),
])];

let request = ChatCompletionRequest::builder()
    .model(model_slug)                                   // impl Into<String>
    .messages(messages)
    .modalities([Modality::Image, Modality::Text])       // REQUIRED for image output
    .image_config([("aspect_ratio", "16:9")])            // see the allowed-ratios trap below
    .temperature(0.2_f64)                                // cool for sheet consistency
    .build()?;
```

`modalities` is the switch that makes the model return an image — omit it and you
get text. `image_config` takes `IntoIterator<Item = (impl Into<String>, impl
Into<serde_json::Value>)>`; `aspect_ratio` and `image_size` ("1K".."4K") are the
useful keys.

**Aspect-ratio trap (verified against a 400).** Gemini image models accept only a
fixed set of `aspect_ratio` values and reject anything else with a 400 listing the
valid options: `1:1, 1:4, 1:8, 2:3, 3:2, 3:4, 4:1, 4:3, 4:5, 5:4, 8:1, 9:16, 16:9,
21:9`. Notably **`2:1` is not allowed** — the value a 4x2 square-cell sprite sheet
naturally wants. Pixhaus requests `16:9` for the idle sheet (the closest supported;
a 4x2 grid then tiles into slightly portrait cells, fine for a standing figure) and
`1:1` for the anchor. Pick from that list, and prefer `1:1`/`16:9` since they are the
most broadly supported across models (the slug is configurable, so don't assume a
ratio one model accepts works on another).

### Send and read the generated image

```rust
let response = client.send_chat_completion(&request).await?;   // CompletionsResponse
```

`Choice` is a `#[non_exhaustive]` enum — there is no `.message()` accessor, so match
it. The generated images live on the non-streaming choice's message as raw JSON
values, each `{ "type": "image_url", "image_url": { "url": "data:image/png;base64,..." } }`:

```rust
use openrouter_rs::types::completion::Choice;

for choice in &response.choices {
    if let Choice::NonStreaming(c) = choice {
        if let Some(images) = c.message.images.as_ref() {        // Option<Vec<serde_json::Value>>
            for image in images {
                if let Some(url) = image.get("image_url")
                    .and_then(|i| i.get("url"))
                    .and_then(serde_json::Value::as_str)
                {
                    // url is "data:image/png;base64,...."
                }
            }
        }
    }
}
```

### Decode and encode the data URL

Strip the `data:...,` prefix, base64-decode, then decode the PNG with `image`:

```rust
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;

let b64 = url.split_once(',').map_or(url, |(_, payload)| payload);
let bytes = STANDARD.decode(b64)?;
let rgba = image::load_from_memory_with_format(&bytes, image::ImageFormat::Png)?.to_rgba8();
// rgba.dimensions() -> (w, h); rgba.into_raw() -> tight RGBA8 Vec<u8>
```

Encode direction (attach the anchor): PNG-encode the RGBA via
`image::codecs::png::PngEncoder::write_image(buf, w, h, ExtendedColorType::Rgba8)`,
then `STANDARD.encode(&png)` and prepend `data:image/png;base64,`.

## The Pixhaus boundary

- The provider lives in `modules/providers`; provider-specific logic stays there,
  never in the Generate workspace. The app asks by capability, never by name.
- The app owns the key: it reads `OPENROUTER_API_KEY` from the environment and calls
  `register_openrouter`. The module never reads the environment. The key is **never
  logged** and never stored in a `GeneratedResult` or provenance. Env is the interim
  home; the OS vault ([[pixhaus-keyring]]) is the follow-up.
- Register OpenRouter **before** the mock so capability lookups prefer the real
  backend; a build failure is logged and skipped, never fatal (the mock answers).
- Per-pixel work — base64-decode, PNG decode, chroma-key, slice — runs in
  `tokio::task::spawn_blocking`, off the reactor. Wrap the request in a `biased`
  `tokio::select!` against the `CancellationToken` so a cancel wins ties.
- Post-processing (chroma-key the magenta, slice the sheet) is
  `pixhaus_mod_providers::{chroma_key_magenta, slice_sheet}` — shared with any
  provider that follows the magenta-key convention.

## Common mistakes

- Forgetting `modalities([Modality::Image, Modality::Text])` — the model returns
  text and `message.images` is `None`.
- Assuming `Choice` has a `.message` field — it is a non-exhaustive enum; match
  `Choice::NonStreaming(c)` and read `c.message.images`. Always handle `images ==
  None` (a refusal or a text-only response) with a clean `BadOutput` error, never an
  unwrap.
- Decoding the PNG on the async reactor instead of `spawn_blocking`.
- Logging or storing the API key. It is data, never an i18n key, and never a log
  field.
- Floating `openrouter-rs` (`"0.10"` unpinned is fine for a patch, but verify the
  image surface against docs.rs on any minor bump).
- Hardcoding a model slug as the only option — they drift; honor
  `PIXHAUS_OPENROUTER_MODEL`.
