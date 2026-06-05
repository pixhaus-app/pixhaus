//! The OpenRouter provider: real sprite-anchor and idle-animation generation.
//!
//! Ports the proven two-pass pipeline onto OpenRouter (bible 14.2). An anchor is a
//! text-only request asking for a neutral character on a flat magenta key; an idle
//! animation is an image-to-image request that attaches the anchor as a reference and
//! asks for an N-cell sheet. Both set `modalities = [image, text]` and an
//! `image_config` aspect ratio so the model returns an image. The returned PNG is
//! chroma-keyed and (for animation) sliced into frames by `super::postprocess`.
//!
//! The worker contract holds (bible 13.6): the provider receives immutable input and
//! returns a result; it never touches the live document. The API key is read by the
//! app from the environment and passed in here - never logged, never stored in a
//! result. Per-pixel decode and slicing run on a blocking thread, off the reactor.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use tokio_util::sync::CancellationToken;

use openrouter_rs::OpenRouterClient;
use openrouter_rs::api::chat::{ChatCompletionRequest, ContentPart, Message, Modality};
use openrouter_rs::types::Role;
use openrouter_rs::types::completion::{Choice, CompletionsResponse};

use pixhaus_core::LoopMode;
use pixhaus_services::generated::{GeneratedAnimation, GeneratedAsset, GeneratedResult, GenerationProvenance};
use pixhaus_services::job::{GenerationJobInput, GenerationKind, Grid, ReferenceImage};
use pixhaus_services::provider::{GenerateFuture, Provider, ProviderCapability, ProviderError, ProviderId};

use crate::postprocess::{chroma_key_magenta, slice_sheet};

/// The provider's stable id and provenance label.
const PROVIDER_ID: &str = "openrouter";

/// The default model id. OpenRouter image-output model slugs drift; override with the
/// `PIXHAUS_OPENROUTER_MODEL` environment variable to pin a current one.
const DEFAULT_MODEL: &str = "google/gemini-2.5-flash-image";

const CAPABILITIES: &[ProviderCapability] = &[
    ProviderCapability::TextToSprite,
    ProviderCapability::GenerateAnchor,
    ProviderCapability::GenerateIdleAnimation,
];

/// Generates sprites through the OpenRouter image-output chat models.
pub struct OpenRouterProvider {
    client: OpenRouterClient,
    model: String,
}

impl OpenRouterProvider {
    /// Builds a provider from an API key. The model id comes from
    /// `PIXHAUS_OPENROUTER_MODEL`, falling back to the default model.
    ///
    /// The key is a transient `String` handed in from the app boundary. Today the app
    /// reads it from the environment; the destination is the OS credential vault per
    /// `modules/providers/CLAUDE.md` and bible 22.4. The env var is the foundation-stage
    /// seam, not the final source — the provider takes the key as data either way, so the
    /// move to a stored key reference does not touch this signature.
    ///
    /// # Errors
    /// Returns [`ProviderError::Unavailable`] if the underlying client cannot build.
    pub fn new(api_key: String) -> Result<Self, ProviderError> {
        let model = std::env::var("PIXHAUS_OPENROUTER_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_owned());
        let client = OpenRouterClient::builder()
            .api_key(api_key)
            .build()
            .map_err(|e| ProviderError::Unavailable(format!("openrouter client: {e}")))?;
        Ok(Self { client, model })
    }

    /// Builds the chat request for one generation, choosing the messages, temperature,
    /// and aspect ratio from the generation kind.
    fn build_request(&self, input: &GenerationJobInput) -> Result<ChatCompletionRequest, ProviderError> {
        let (messages, temperature, aspect) = match &input.kind {
            // The anchor is text-only: a single neutral character on a flat key.
            GenerationKind::Anchor => (vec![Message::new(Role::User, input.prompt.clone())], 0.6_f64, "1:1"),
            // The idle pass attaches the anchor as the identity reference (image-to-image)
            // and runs cool so the eight cells stay on-model. Gemini only accepts a fixed
            // set of aspect ratios (1:1, 4:3, 16:9, 21:9, ...) - NOT the 2:1 a 4x2
            // square-cell sheet would want - so request 16:9, the closest supported; for
            // a 4x2 grid that gives slightly portrait cells, which suit a standing figure.
            GenerationKind::IdleAnimation { reference, .. } => {
                let data_url = encode_reference(reference)?;
                let parts = vec![ContentPart::text(input.prompt.clone()), ContentPart::image_url(data_url)];
                (vec![Message::with_parts(Role::User, parts)], 0.2_f64, "16:9")
            }
        };
        // input.size is advisory for OpenRouter: output size is constrained by
        // aspect_ratio only (Gemini ignores a pixel size, and a configurable model
        // slug may 400 on an image_size key), so it is intentionally not forwarded -
        // the result is taken at the size the model picks for the aspect ratio.
        ChatCompletionRequest::builder()
            .model(self.model.clone())
            .messages(messages)
            .modalities([Modality::Image, Modality::Text])
            .image_config([("aspect_ratio", aspect)])
            .temperature(temperature)
            .build()
            .map_err(|e| ProviderError::BadOutput(format!("request build: {e}")))
    }
}

impl Provider for OpenRouterProvider {
    fn id(&self) -> ProviderId {
        ProviderId(PROVIDER_ID.to_owned())
    }

    fn label_key(&self) -> &'static str {
        "provider.openrouter.label"
    }

    fn capabilities(&self) -> &[ProviderCapability] {
        CAPABILITIES
    }

    fn generate<'a>(&'a self, input: &'a GenerationJobInput, cancel: CancellationToken) -> GenerateFuture<'a> {
        Box::pin(async move {
            let request = self.build_request(input)?;
            // The cancel branch wins ties (biased) so a cancel during a slow request
            // resolves promptly.
            let response = tokio::select! {
                biased;
                () = cancel.cancelled() => return Err(ProviderError::Cancelled),
                result = self.client.send_chat_completion(&request) => result.map_err(|e| ProviderError::Unavailable(e.to_string()))?,
            };
            let data_url = first_image_data_url(&response).ok_or_else(|| ProviderError::BadOutput("response contained no image".to_owned()))?;

            // Decode and post-process off the reactor (per-pixel work).
            let decode = DecodeKind::from(&input.kind);
            let provenance = GenerationProvenance {
                prompt: input.prompt.clone(),
                seed: input.seed,
                provider_id: PROVIDER_ID.to_owned(),
                model: self.model.clone(),
                created_unix_ms: crate::now_ms(),
            };
            tokio::task::spawn_blocking(move || decode_result(&data_url, decode, provenance))
                .await
                .map_err(|e| ProviderError::BadOutput(format!("decode task failed: {e}")))?
        })
    }
}

/// What the response should be decoded into - mirrors [`GenerationKind`] without the
/// reference image (which the request already used).
enum DecodeKind {
    Anchor,
    Idle { grid: Grid, fps: u16, clip_name: String },
}

impl DecodeKind {
    fn from(kind: &GenerationKind) -> Self {
        match kind {
            GenerationKind::Anchor => Self::Anchor,
            GenerationKind::IdleAnimation { grid, fps, animation_id, .. } => Self::Idle {
                grid: *grid,
                fps: *fps,
                clip_name: animation_id.clone(),
            },
        }
    }
}

/// Finds the first generated image in the response and returns its data URL.
fn first_image_data_url(response: &CompletionsResponse) -> Option<String> {
    for choice in &response.choices {
        if let Choice::NonStreaming(non_streaming) = choice {
            let images = non_streaming.message.images.as_ref()?;
            for image in images {
                if let Some(url) = image.get("image_url").and_then(|inner| inner.get("url")).and_then(serde_json::Value::as_str) {
                    return Some(url.to_owned());
                }
            }
        }
    }
    None
}

/// Decodes the returned image, chroma-keys it, and shapes the result by kind. Pure
/// CPU work, run on a blocking thread.
fn decode_result(data_url: &str, kind: DecodeKind, provenance: GenerationProvenance) -> Result<GeneratedResult, ProviderError> {
    let (mut rgba, width, height) = decode_png_data_url(data_url)?;
    chroma_key_magenta(&mut rgba);
    match kind {
        DecodeKind::Anchor => Ok(GeneratedResult::Sprite(GeneratedAsset {
            width,
            height,
            stride: width * 4,
            rgba,
            provenance,
        })),
        DecodeKind::Idle { grid, fps, clip_name } => {
            let frames = slice_sheet(&rgba, width, height, grid).map_err(|e| ProviderError::BadOutput(e.to_string()))?;
            Ok(GeneratedResult::Animation(GeneratedAnimation {
                frames,
                clip_name,
                fps,
                loop_mode: LoopMode::Loop,
                provenance,
            }))
        }
    }
}

/// Upper bound, per side in pixels, on a decoded provider image. Generated anchors
/// (512px) and idle sheets sit far below this; the cap exists only to reject a hostile
/// or malformed response (a decompression bomb) before it allocates, so a bad job
/// surfaces as an error instead of OOM-ing the app (bible 24.3). Matches the project's
/// 8192-px canvas ceiling.
const MAX_DECODE_DIM: u32 = 8192;

/// Strips the `data:image/png;base64,` prefix, base64-decodes, and decodes the PNG to
/// tightly-packed RGBA8, returning `(rgba, width, height)`.
///
/// The PNG is untrusted network input, so it is decoded through [`image::ImageReader`]
/// with an explicit dimension [`image::Limits`] rather than the bare
/// `load_from_memory*` free function: a response declaring enormous dimensions is
/// rejected as [`ProviderError::BadOutput`] instead of driving a huge allocation. This
/// already runs on a blocking thread (see [`OpenRouterProvider::generate`]).
fn decode_png_data_url(data_url: &str) -> Result<(Vec<u8>, u32, u32), ProviderError> {
    let b64 = data_url.split_once(',').map_or(data_url, |(_, payload)| payload);
    let bytes = STANDARD.decode(b64).map_err(|e| ProviderError::BadOutput(format!("base64 decode: {e}")))?;

    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_DECODE_DIM);
    limits.max_image_height = Some(MAX_DECODE_DIM);
    let mut reader = image::ImageReader::with_format(std::io::Cursor::new(bytes), image::ImageFormat::Png);
    reader.limits(limits);
    let image = reader.decode().map_err(|e| ProviderError::BadOutput(format!("png decode: {e}")))?;

    let rgba = image.to_rgba8();
    let (width, height) = rgba.dimensions();
    Ok((rgba.into_raw(), width, height))
}

/// PNG-encodes a reference image and wraps it as a base64 data URL for attachment.
fn encode_reference(reference: &ReferenceImage) -> Result<String, ProviderError> {
    use image::ImageEncoder as _;
    let mut png = Vec::new();
    image::codecs::png::PngEncoder::new(&mut png)
        .write_image(&reference.rgba, reference.width, reference.height, image::ExtendedColorType::Rgba8)
        .map_err(|e| ProviderError::BadOutput(format!("reference png encode: {e}")))?;
    Ok(format!("data:image/png;base64,{}", STANDARD.encode(&png)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pixhaus_services::job::GenerationContext;

    fn anchor_input() -> GenerationJobInput {
        GenerationJobInput {
            prompt: "Bit, side view".to_owned(),
            seed: 1,
            size: (512, 512),
            context: GenerationContext::NewAsset,
            kind: GenerationKind::Anchor,
        }
    }

    fn idle_input() -> GenerationJobInput {
        GenerationJobInput {
            prompt: "Bit idle".to_owned(),
            seed: 1,
            size: (512, 512),
            context: GenerationContext::NewAsset,
            kind: GenerationKind::IdleAnimation {
                reference: ReferenceImage {
                    width: 2,
                    height: 2,
                    stride: 8,
                    rgba: vec![0u8; 2 * 2 * 4],
                },
                animation_id: "idle".to_owned(),
                grid: Grid { cols: 4, rows: 2 },
                fps: 12,
            },
        }
    }

    // A small valid provider so request shaping can be exercised without a network key.
    fn provider() -> OpenRouterProvider {
        OpenRouterProvider::new("test-key".to_owned()).expect("client builds from a key")
    }

    #[test]
    fn builds_an_anchor_request_without_panicking() {
        // The request shapes (model/messages/modalities/aspect) without a live call.
        assert!(provider().build_request(&anchor_input()).is_ok());
    }

    #[test]
    fn builds_an_idle_request_attaching_the_reference() {
        assert!(provider().build_request(&idle_input()).is_ok());
    }

    #[test]
    fn decodes_a_known_png_data_url() {
        // A 1x1 opaque-red PNG, base64-encoded, as a data URL.
        let mut png = Vec::new();
        {
            use image::ImageEncoder as _;
            image::codecs::png::PngEncoder::new(&mut png)
                .write_image(&[255, 0, 0, 255], 1, 1, image::ExtendedColorType::Rgba8)
                .unwrap();
        }
        let url = format!("data:image/png;base64,{}", STANDARD.encode(&png));
        let (rgba, w, h) = decode_png_data_url(&url).unwrap();
        assert_eq!((w, h), (1, 1));
        assert_eq!(rgba, vec![255, 0, 0, 255]);
    }

    // openrouter-rs 0.10 marks CompletionsResponse/Choice/Message #[non_exhaustive]
    // with no public constructors, so a synthetic response is built by deserializing
    // JSON, not by a struct literal. `object` must read "chat.completion" and the
    // untagged Choice enum resolves to NonStreaming when a `message` object is present
    // (NonChat needs a `text` field, which this omits).
    fn response_with_image(url: &str) -> CompletionsResponse {
        let json = serde_json::json!({
            "id": "x",
            "created": 0,
            "model": "m",
            "object": "chat.completion",
            "choices": [{
                "message": {
                    "role": "assistant",
                    "images": [{ "image_url": { "url": url } }]
                }
            }]
        });
        serde_json::from_value(json).expect("synthetic chat.completion deserializes")
    }

    fn response_without_image() -> CompletionsResponse {
        let json = serde_json::json!({
            "id": "x",
            "created": 0,
            "model": "m",
            "object": "chat.completion",
            "choices": [{ "message": { "role": "assistant" } }]
        });
        serde_json::from_value(json).expect("synthetic chat.completion deserializes")
    }

    #[test]
    fn first_image_data_url_reads_the_first_image() {
        let response = response_with_image("data:image/png;base64,AAA");
        assert_eq!(first_image_data_url(&response), Some("data:image/png;base64,AAA".to_owned()));
    }

    #[test]
    fn first_image_data_url_is_none_without_images() {
        assert_eq!(first_image_data_url(&response_without_image()), None);
        // An empty choices array also yields no image.
        let json = serde_json::json!({
            "id": "x", "created": 0, "model": "m", "object": "chat.completion", "choices": []
        });
        let empty: CompletionsResponse = serde_json::from_value(json).expect("deserializes");
        assert_eq!(first_image_data_url(&empty), None);
    }

    // A small RGBA image PNG-encoded to a base64 data URL, the same shape a provider
    // returns. Used to exercise decode and post-process off the network.
    fn png_data_url(rgba: &[u8], width: u32, height: u32) -> String {
        use image::ImageEncoder as _;
        let mut png = Vec::new();
        image::codecs::png::PngEncoder::new(&mut png)
            .write_image(rgba, width, height, image::ExtendedColorType::Rgba8)
            .expect("png encodes");
        format!("data:image/png;base64,{}", STANDARD.encode(&png))
    }

    #[test]
    fn encode_reference_round_trips_through_decode() {
        // 2x2 known pixels through encode_reference -> decode_png_data_url.
        let pixels = vec![
            10, 20, 30, 255, 40, 50, 60, 255, // top row
            70, 80, 90, 255, 100, 110, 120, 255, // bottom row
        ];
        let reference = ReferenceImage {
            width: 2,
            height: 2,
            stride: 8,
            rgba: pixels.clone(),
        };
        let url = encode_reference(&reference).expect("reference encodes");
        let (rgba, w, h) = decode_png_data_url(&url).expect("round-trips");
        assert_eq!((w, h), (2, 2));
        assert_eq!(rgba, pixels);
    }

    #[test]
    fn decode_result_anchor_keys_magenta_and_returns_a_sprite() {
        // A 2x1 sheet: one magenta key pixel, one opaque character pixel.
        let url = png_data_url(&[255, 0, 255, 255, 12, 34, 56, 255], 2, 1);
        let result = decode_result(&url, DecodeKind::Anchor, provenance()).expect("decodes");
        match result {
            GeneratedResult::Sprite(asset) => {
                assert_eq!((asset.width, asset.height, asset.stride), (2, 1, 8));
                assert_eq!(&asset.rgba[0..4], &[0, 0, 0, 0], "magenta keyed to transparent");
                assert_eq!(&asset.rgba[4..8], &[12, 34, 56, 255], "character pixel kept");
            }
            GeneratedResult::Animation(_) => panic!("anchor must decode to a sprite"),
        }
    }

    #[test]
    fn decode_result_idle_slices_a_keyed_sheet_into_frames() {
        // A 4x1 sheet: each cell one pixel; the first is the magenta key.
        let sheet = vec![
            255, 0, 255, 255, // magenta key
            10, 0, 0, 255, // cell 1
            20, 0, 0, 255, // cell 2
            30, 0, 0, 255, // cell 3
        ];
        let url = png_data_url(&sheet, 4, 1);
        let grid = Grid { cols: 4, rows: 1 };
        let kind = DecodeKind::Idle {
            grid,
            fps: 12,
            clip_name: "idle".to_owned(),
        };
        let result = decode_result(&url, kind, provenance()).expect("decodes");
        match result {
            GeneratedResult::Animation(animation) => {
                assert_eq!(animation.frames.len(), grid.cell_count() as usize);
                assert_eq!(animation.fps, 12);
                assert_eq!(animation.clip_name, "idle");
                assert_eq!(animation.frames[0].rgba, vec![0, 0, 0, 0], "first frame is the keyed pixel");
                assert_eq!(animation.frames[1].rgba, vec![10, 0, 0, 255]);
            }
            GeneratedResult::Sprite(_) => panic!("idle must decode to an animation"),
        }
    }

    // A provenance fixture for decode tests; the value is irrelevant to decode shaping.
    fn provenance() -> GenerationProvenance {
        GenerationProvenance {
            prompt: "Bit".to_owned(),
            seed: 1,
            provider_id: PROVIDER_ID.to_owned(),
            model: "m".to_owned(),
            created_unix_ms: 0,
        }
    }

    #[test]
    fn decode_png_data_url_rejects_invalid_base64() {
        // The base64 step (not the PNG decode) must fail loudly, not silently.
        let result = decode_png_data_url("data:image/png;base64,!!!not_base64!!!");
        assert!(matches!(result, Err(ProviderError::BadOutput(_))), "invalid base64 is a BadOutput error");
    }

    #[test]
    fn decode_png_data_url_rejects_an_over_limit_image() {
        // An image one pixel past MAX_DECODE_DIM must trip the Limits guard rather than
        // allocating. Encode a genuinely oversized PNG so the check fires deterministically
        // (a crafted header alone is not the safest trigger).
        let width = MAX_DECODE_DIM + 1;
        let rgba = vec![0u8; width as usize * 4];
        let url = png_data_url(&rgba, width, 1);
        let result = decode_png_data_url(&url);
        assert!(matches!(result, Err(ProviderError::BadOutput(_))), "an over-limit image is a BadOutput error");
    }
}
