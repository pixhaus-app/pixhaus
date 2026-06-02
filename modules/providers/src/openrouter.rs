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

use std::time::{SystemTime, UNIX_EPOCH};

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
            // and runs cool so the eight cells stay on-model.
            GenerationKind::IdleAnimation { reference, .. } => {
                let data_url = encode_reference(reference)?;
                let parts = vec![ContentPart::text(input.prompt.clone()), ContentPart::image_url(data_url)];
                (vec![Message::with_parts(Role::User, parts)], 0.2_f64, "2:1")
            }
        };
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
                created_unix_ms: now_ms(),
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

/// Strips the `data:image/png;base64,` prefix, base64-decodes, and decodes the PNG to
/// tightly-packed RGBA8, returning `(rgba, width, height)`.
fn decode_png_data_url(data_url: &str) -> Result<(Vec<u8>, u32, u32), ProviderError> {
    let b64 = data_url.split_once(',').map_or(data_url, |(_, payload)| payload);
    let bytes = STANDARD.decode(b64).map_err(|e| ProviderError::BadOutput(format!("base64 decode: {e}")))?;
    let image = image::load_from_memory_with_format(&bytes, image::ImageFormat::Png).map_err(|e| ProviderError::BadOutput(format!("png decode: {e}")))?;
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

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|d| u64::try_from(d.as_millis()).ok())
        .unwrap_or(0)
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
}
