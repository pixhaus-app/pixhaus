//! Google AI Studio image backend.

use std::time::Duration;

use async_trait::async_trait;
use base64::Engine as _;
use serde::Deserialize;
use tokio::select;
use tokio_util::sync::CancellationToken;

use super::{
    BackendError, ImageGenResponse, InferenceBackend, InferenceRequest, InferenceResponse, Result,
    VerbProgress, check_http_status,
};
use crate::plugin::descriptor::{BackendCapabilities, CostEstimate};
use crate::plugin::progress::{CostUpdate, VerbProgressEvent};

const BASE_URL: &str = "https://generativelanguage.googleapis.com/v1beta/models";
/// Gemini Pro image model used for the product-facing Nano Banana Pro label.
pub const GOOGLE_PRO_IMAGE_MODEL: &str = "gemini-3-pro-image-preview";
/// Gemini Flash image model used for lower-latency image generation.
pub const GOOGLE_FLASH_IMAGE_MODEL: &str = "gemini-3.1-flash-image-preview";

/// Google AI Studio backend adapter for Gemini image generation/editing.
pub struct GoogleAiBackend {
    client: reqwest::Client,
    api_key: String,
    base_url: String,
    image_model: String,
}

impl std::fmt::Debug for GoogleAiBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GoogleAiBackend")
            .field("base_url", &self.base_url)
            .field("image_model", &self.image_model)
            .field("api_key", &"[redacted]")
            .finish_non_exhaustive()
    }
}

impl GoogleAiBackend {
    /// Constructs an adapter with an explicit API key.
    #[must_use]
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(120))
                .build()
                .unwrap_or_default(),
            api_key: api_key.into(),
            base_url: BASE_URL.into(),
            image_model: GOOGLE_PRO_IMAGE_MODEL.into(),
        }
    }

    /// Constructs an adapter by reading the API key from the OS keychain.
    pub fn from_keychain() -> Result<Self> {
        let key = super::ApiKeyStore::get("google_ai")?;
        Ok(Self::new(key))
    }

    /// Overrides the default image model.
    #[must_use]
    pub fn with_image_model(mut self, model: impl Into<String>) -> Self {
        self.image_model = model.into();
        self
    }

    /// Overrides the API base URL for tests.
    #[must_use]
    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    async fn generate_or_edit(
        &self,
        model_override: Option<&str>,
        prompt: &str,
        images: Vec<(&[u8], &str)>,
        progress: &VerbProgress,
        cancel: &CancellationToken,
    ) -> Result<ImageGenResponse> {
        let model = model_override.unwrap_or(&self.image_model).to_owned();
        let body = build_generate_content_body(prompt, images);
        let http_req = self
            .client
            .post(format!("{}/{}:generateContent", self.base_url, model))
            .header("x-goog-api-key", &self.api_key)
            .json(&body)
            .build()
            .map_err(BackendError::Network)?;

        let http_resp = select! {
            biased;
            () = cancel.cancelled() => return Err(BackendError::Cancelled),
            res = self.client.execute(http_req) => res.map_err(BackendError::Network)?,
        };
        let http_resp = check_http_status(http_resp).await?;
        let raw: GeminiGenerateContentResponse =
            http_resp.json().await.map_err(BackendError::Network)?;
        let images = decode_inline_images(raw)?;
        progress
            .send(VerbProgressEvent::Cost(CostUpdate {
                usd_cents: 0.0,
                tokens_input: None,
                tokens_output: None,
            }))
            .await;
        Ok(ImageGenResponse { images, model })
    }
}

#[async_trait]
impl InferenceBackend for GoogleAiBackend {
    fn backend_id(&self) -> &'static str {
        "google_ai"
    }

    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities::IMAGE_GENERATION.union(BackendCapabilities::IMAGE_EDIT)
    }

    fn supports_streaming(&self) -> bool {
        false
    }

    fn estimate_cost(&self, _request: &InferenceRequest) -> CostEstimate {
        CostEstimate {
            typical_latency: Duration::from_secs(5),
            max_latency: Duration::from_secs(60),
            typical_usd_cents: 12.0,
            max_usd_cents: 30.0,
        }
    }

    async fn invoke(
        &self,
        request: InferenceRequest,
        progress: VerbProgress,
        cancel: CancellationToken,
    ) -> Result<InferenceResponse> {
        progress
            .send(VerbProgressEvent::Started {
                backend: Some("google_ai".into()),
            })
            .await;
        match request {
            InferenceRequest::ImageGeneration(req) => {
                let mut refs = req
                    .reference_images
                    .iter()
                    .map(|bytes| (bytes.as_slice(), "image/png"))
                    .collect::<Vec<_>>();
                if refs.is_empty()
                    && let Some(bytes) = req.style_image.as_deref()
                {
                    refs.push((bytes, "image/png"));
                }
                let resp = self
                    .generate_or_edit(req.model.as_deref(), &req.prompt, refs, &progress, &cancel)
                    .await?;
                Ok(InferenceResponse::Image(resp))
            }
            InferenceRequest::ImageEdit(req) => {
                let mut images = vec![(req.image.as_slice(), "image/png")];
                if let Some(mask) = req.mask.as_deref() {
                    images.push((mask, "image/png"));
                }
                images.extend(
                    req.reference_images
                        .iter()
                        .map(|bytes| (bytes.as_slice(), "image/png")),
                );
                if let Some(style) = req.style_image.as_deref() {
                    images.push((style, "image/png"));
                }
                let resp = self
                    .generate_or_edit(
                        req.model.as_deref(),
                        &req.prompt,
                        images,
                        &progress,
                        &cancel,
                    )
                    .await?;
                Ok(InferenceResponse::Image(resp))
            }
            _ => Err(BackendError::UnsupportedCapability),
        }
    }
}

impl crate::plugin::backend::InferenceBackend for GoogleAiBackend {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn id(&self) -> &'static str {
        "google_ai"
    }

    fn capabilities(&self) -> BackendCapabilities {
        <Self as InferenceBackend>::capabilities(self)
    }

    fn cost_estimate(&self, _required: BackendCapabilities) -> CostEstimate {
        CostEstimate {
            typical_latency: Duration::from_secs(5),
            max_latency: Duration::from_secs(60),
            typical_usd_cents: 12.0,
            max_usd_cents: 30.0,
        }
    }

    fn is_available(&self) -> bool {
        !self.api_key.is_empty()
    }
}

#[allow(clippy::disallowed_methods)]
fn build_generate_content_body(prompt: &str, images: Vec<(&[u8], &str)>) -> serde_json::Value {
    let mut parts = vec![serde_json::json!({ "text": prompt })];
    parts.extend(images.into_iter().map(|(bytes, mime)| {
        serde_json::json!({
            "inline_data": {
                "mime_type": mime,
                "data": base64::engine::general_purpose::STANDARD.encode(bytes),
            }
        })
    }));
    serde_json::json!({
        "contents": [{ "parts": parts }],
        "generationConfig": { "responseModalities": ["IMAGE"] },
    })
}

fn decode_inline_images(raw: GeminiGenerateContentResponse) -> Result<Vec<Vec<u8>>> {
    let mut images = Vec::new();
    for candidate in raw.candidates {
        for part in candidate.content.parts {
            if let Some(inline) = part.inline_data.or(part.inline_data_camel) {
                images.push(
                    base64::engine::general_purpose::STANDARD
                        .decode(inline.data.as_bytes())
                        .map_err(|err| BackendError::InvalidResponse(err.to_string()))?,
                );
            }
        }
    }
    if images.is_empty() {
        return Err(BackendError::InvalidResponse(
            "Gemini image response contained no inline image data".into(),
        ));
    }
    Ok(images)
}

#[derive(Deserialize)]
struct GeminiGenerateContentResponse {
    #[serde(default)]
    candidates: Vec<GeminiCandidate>,
}

#[derive(Deserialize)]
struct GeminiCandidate {
    content: GeminiContent,
}

#[derive(Deserialize)]
struct GeminiContent {
    #[serde(default)]
    parts: Vec<GeminiPart>,
}

#[derive(Deserialize)]
struct GeminiPart {
    #[serde(default)]
    inline_data: Option<GeminiInlineData>,
    #[serde(default, rename = "inlineData")]
    inline_data_camel: Option<GeminiInlineData>,
}

#[derive(Deserialize)]
struct GeminiInlineData {
    data: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn body_contains_text_and_inline_image() {
        let body = build_generate_content_body("draw", vec![(b"abc".as_slice(), "image/png")]);
        assert_eq!(body["contents"][0]["parts"][0]["text"], "draw");
        assert_eq!(
            body["contents"][0]["parts"][1]["inline_data"]["mime_type"],
            "image/png"
        );
        assert_eq!(body["generationConfig"]["responseModalities"][0], "IMAGE");
    }

    #[test]
    fn capabilities_match_implemented_request_paths() {
        let caps = GoogleAiBackend::new("key").capabilities();
        assert!(caps.contains(BackendCapabilities::IMAGE_GENERATION));
        assert!(caps.contains(BackendCapabilities::IMAGE_EDIT));
        assert!(!caps.contains(BackendCapabilities::IMAGE_INPAINT));
        assert!(!caps.contains(BackendCapabilities::VISION_LANGUAGE));
    }

    #[test]
    fn decodes_inline_data() {
        let raw = GeminiGenerateContentResponse {
            candidates: vec![GeminiCandidate {
                content: GeminiContent {
                    parts: vec![GeminiPart {
                        inline_data: Some(GeminiInlineData {
                            data: base64::engine::general_purpose::STANDARD.encode(b"png"),
                        }),
                        inline_data_camel: None,
                    }],
                },
            }],
        };
        assert_eq!(decode_inline_images(raw).unwrap(), vec![b"png".to_vec()]);
    }
}
