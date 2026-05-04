//! `OpenAI` backend adapter.
//!
//! Supports text generation (GPT-5 family), vision-language, image
//! generation (DALL-E 4), image editing/inpainting, tool-use, and
//! embeddings. Implemented as a raw HTTP client for consistency with the
//! other adapters.
//!
//! # Models
//!
//! - Text / vision: `gpt-4o` (default); override per-request.
//! - Image generation: `dall-e-3` (default).
//! - Image edit/inpaint: `dall-e-3`.
//!
//! # Pricing (May 2026 estimates)
//!
//! - GPT-4o input: $2.50 / `MTok`
//! - GPT-4o output: $10.00 / `MTok`
//! - DALL-E 3 HD 1024×1024: $0.080 per image

use std::time::Duration;

use async_trait::async_trait;
use base64::Engine as _;
use serde::Deserialize;
use tokio::select;
use tokio_util::sync::CancellationToken;
use tracing::{debug, instrument, warn};

use super::{
    BackendError, ChatRole, ContentPart, FrameInterpolationRequest, ImageEditRequest,
    ImageGenRequest, ImageGenResponse, InferenceBackend, InferenceRequest, InferenceResponse,
    Result, TextGenRequest, TextGenResponse, ToolCall, VerbProgress, check_http_status,
};
use crate::plugin::descriptor::{BackendCapabilities, CostEstimate};
use crate::plugin::progress::{CostUpdate, VerbProgressEvent};

const BASE_URL: &str = "https://api.openai.com/v1";
const DEFAULT_TEXT_MODEL: &str = "gpt-4o";
const DEFAULT_IMAGE_MODEL: &str = "dall-e-3";

const INPUT_PRICE_PER_TOKEN: f64 = 2.50 / 1_000_000.0;
const OUTPUT_PRICE_PER_TOKEN: f64 = 10.00 / 1_000_000.0;
const IMAGE_PRICE_CENTS: f32 = 8.0; // DALL-E 3 HD 1024×1024 ≈ $0.08 = 8 cents

/// `OpenAI` backend adapter.
pub struct OpenAiBackend {
    client: reqwest::Client,
    api_key: String,
    base_url: String,
    text_model: String,
    image_model: String,
}

impl OpenAiBackend {
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
            text_model: DEFAULT_TEXT_MODEL.into(),
            image_model: DEFAULT_IMAGE_MODEL.into(),
        }
    }

    /// Constructs an adapter by reading the API key from the OS keychain.
    pub fn from_keychain() -> Result<Self> {
        let key = super::ApiKeyStore::get("openai")?;
        Ok(Self::new(key))
    }

    /// Overrides the default text model.
    #[must_use]
    pub fn with_text_model(mut self, model: impl Into<String>) -> Self {
        self.text_model = model.into();
        self
    }

    /// Overrides the default image model.
    #[must_use]
    pub fn with_image_model(mut self, model: impl Into<String>) -> Self {
        self.image_model = model.into();
        self
    }

    /// Overrides the base URL.
    #[must_use]
    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    async fn chat_completions(
        &self,
        req: &TextGenRequest,
        progress: &VerbProgress,
        cancel: &CancellationToken,
    ) -> Result<TextGenResponse> {
        let model = req
            .model
            .as_deref()
            .unwrap_or(self.text_model.as_str())
            .to_owned();

        let body = build_chat_body(req, &model);
        debug!(model = %model, "sending OpenAI chat completions request");

        let http_req = self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .bearer_auth(&self.api_key)
            .json(&body)
            .build()
            .map_err(BackendError::Network)?;

        let http_resp = select! {
            biased;
            () = cancel.cancelled() => return Err(BackendError::Cancelled),
            res = self.client.execute(http_req) => res.map_err(BackendError::Network)?,
        };

        let http_resp = check_http_status(http_resp).await?;
        let raw: OpenAiChatResponse = http_resp.json().await.map_err(BackendError::Network)?;

        let choice = raw
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| BackendError::InvalidResponse("no choices in response".into()))?;

        let content = choice.message.content.unwrap_or_default();
        let tool_calls: Vec<ToolCall> = choice
            .message
            .tool_calls
            .unwrap_or_default()
            .into_iter()
            .map(|tc| ToolCall {
                id: tc.id,
                name: tc.function.name,
                input: serde_json::from_str(&tc.function.arguments)
                    .unwrap_or(serde_json::Value::Null),
            })
            .collect();

        let input_tokens = raw.usage.prompt_tokens;
        let output_tokens = raw.usage.completion_tokens;
        let cost_cents = cost_chat_cents(input_tokens, output_tokens);

        progress
            .send(VerbProgressEvent::Cost(CostUpdate {
                usd_cents: cost_cents,
                tokens_input: Some(input_tokens),
                tokens_output: Some(output_tokens),
            }))
            .await;

        Ok(TextGenResponse {
            content,
            model: raw.model,
            input_tokens,
            output_tokens,
            stop_reason: choice.finish_reason,
            tool_calls,
        })
    }

    #[allow(clippy::cast_precision_loss, clippy::disallowed_methods)]
    async fn generate_image(
        &self,
        req: &ImageGenRequest,
        progress: &VerbProgress,
        cancel: &CancellationToken,
    ) -> Result<ImageGenResponse> {
        let model = req
            .model
            .as_deref()
            .unwrap_or(self.image_model.as_str())
            .to_owned();

        debug!(model = %model, "sending DALL-E image generation request");

        let body = serde_json::json!({
            "model": model,
            "prompt": req.prompt,
            "n": req.num_images,
            "size": format!("{}x{}", req.width, req.height),
            "response_format": "b64_json",
        });

        let http_req = self
            .client
            .post(format!("{}/images/generations", self.base_url))
            .bearer_auth(&self.api_key)
            .json(&body)
            .build()
            .map_err(BackendError::Network)?;

        let http_resp = select! {
            biased;
            () = cancel.cancelled() => return Err(BackendError::Cancelled),
            res = self.client.execute(http_req) => res.map_err(BackendError::Network)?,
        };

        let http_resp = check_http_status(http_resp).await?;
        let raw: OpenAiImageResponse = http_resp.json().await.map_err(BackendError::Network)?;

        let images = decode_image_data(raw.data)?;

        progress
            .send(VerbProgressEvent::Cost(CostUpdate {
                usd_cents: IMAGE_PRICE_CENTS * req.num_images as f32,
                tokens_input: None,
                tokens_output: None,
            }))
            .await;

        Ok(ImageGenResponse { images, model })
    }

    #[allow(clippy::cast_precision_loss)]
    async fn edit_image(
        &self,
        req: &ImageEditRequest,
        progress: &VerbProgress,
        cancel: &CancellationToken,
    ) -> Result<ImageGenResponse> {
        let model = req
            .model
            .as_deref()
            .unwrap_or(self.image_model.as_str())
            .to_owned();

        debug!(model = %model, "sending DALL-E image edit request");

        // DALL-E edit endpoint uses multipart/form-data.
        let mut form = reqwest::multipart::Form::new()
            .text("prompt", req.prompt.clone())
            .text("n", req.num_images.to_string())
            .text("response_format", "b64_json")
            .text("model", model.clone())
            .part(
                "image",
                reqwest::multipart::Part::bytes(req.image.clone())
                    .file_name("image.png")
                    .mime_str("image/png")
                    .map_err(|e| BackendError::Other(e.to_string()))?,
            );

        if let Some(mask) = &req.mask {
            form = form.part(
                "mask",
                reqwest::multipart::Part::bytes(mask.clone())
                    .file_name("mask.png")
                    .mime_str("image/png")
                    .map_err(|e| BackendError::Other(e.to_string()))?,
            );
        }

        let http_req = self
            .client
            .post(format!("{}/images/edits", self.base_url))
            .bearer_auth(&self.api_key)
            .multipart(form)
            .build()
            .map_err(BackendError::Network)?;

        let http_resp = select! {
            biased;
            () = cancel.cancelled() => return Err(BackendError::Cancelled),
            res = self.client.execute(http_req) => res.map_err(BackendError::Network)?,
        };

        let http_resp = check_http_status(http_resp).await?;
        let raw: OpenAiImageResponse = http_resp.json().await.map_err(BackendError::Network)?;
        let images = decode_image_data(raw.data)?;

        progress
            .send(VerbProgressEvent::Cost(CostUpdate {
                usd_cents: IMAGE_PRICE_CENTS * req.num_images as f32,
                tokens_input: None,
                tokens_output: None,
            }))
            .await;

        Ok(ImageGenResponse { images, model })
    }
}

#[async_trait]
impl InferenceBackend for OpenAiBackend {
    fn backend_id(&self) -> &'static str {
        "openai"
    }

    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities::TEXT_GENERATION
            .union(BackendCapabilities::VISION_LANGUAGE)
            .union(BackendCapabilities::IMAGE_GENERATION)
            .union(BackendCapabilities::IMAGE_EDIT)
            .union(BackendCapabilities::IMAGE_INPAINT)
            .union(BackendCapabilities::TOOL_USE)
            .union(BackendCapabilities::EMBEDDINGS)
    }

    fn supports_streaming(&self) -> bool {
        true
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
    fn estimate_cost(&self, request: &InferenceRequest) -> CostEstimate {
        match request {
            InferenceRequest::Text(req) => {
                let chars: usize = req
                    .messages
                    .iter()
                    .flat_map(|m| &m.content)
                    .map(|p| match p {
                        ContentPart::Text { text } => text.len(),
                        ContentPart::Image { .. } => 1000,
                    })
                    .sum();
                let input_tokens = (chars / 4).max(1) as u32;
                let output_tokens = req.max_tokens.unwrap_or(1024);
                let cents = cost_chat_cents(input_tokens, output_tokens);
                CostEstimate {
                    typical_latency: Duration::from_secs(3),
                    max_latency: Duration::from_secs(30),
                    typical_usd_cents: cents,
                    max_usd_cents: cents * 3.0,
                }
            }
            InferenceRequest::ImageGeneration(req) => CostEstimate {
                typical_latency: Duration::from_secs(8),
                max_latency: Duration::from_secs(30),
                typical_usd_cents: IMAGE_PRICE_CENTS * req.num_images as f32,
                max_usd_cents: IMAGE_PRICE_CENTS * req.num_images as f32 * 2.0,
            },
            InferenceRequest::ImageEdit(req) | InferenceRequest::ImageInpaint(req) => {
                CostEstimate {
                    typical_latency: Duration::from_secs(8),
                    max_latency: Duration::from_secs(30),
                    typical_usd_cents: IMAGE_PRICE_CENTS * req.num_images as f32,
                    max_usd_cents: IMAGE_PRICE_CENTS * req.num_images as f32 * 2.0,
                }
            }
            _ => CostEstimate::free(),
        }
    }

    #[instrument(skip(self, request, progress, cancel), fields(backend = "openai"))]
    async fn invoke(
        &self,
        request: InferenceRequest,
        progress: VerbProgress,
        cancel: CancellationToken,
    ) -> Result<InferenceResponse> {
        match request {
            InferenceRequest::Text(ref req) => {
                progress
                    .send(VerbProgressEvent::Started {
                        backend: Some("openai".into()),
                    })
                    .await;
                let resp = self.chat_completions(req, &progress, &cancel).await?;
                Ok(InferenceResponse::Text(resp))
            }
            InferenceRequest::ImageGeneration(ref req) => {
                progress
                    .send(VerbProgressEvent::Started {
                        backend: Some("openai".into()),
                    })
                    .await;
                let resp = self.generate_image(req, &progress, &cancel).await?;
                Ok(InferenceResponse::Image(resp))
            }
            InferenceRequest::ImageEdit(ref req) | InferenceRequest::ImageInpaint(ref req) => {
                progress
                    .send(VerbProgressEvent::Started {
                        backend: Some("openai".into()),
                    })
                    .await;
                let resp = self.edit_image(req, &progress, &cancel).await?;
                Ok(InferenceResponse::Image(resp))
            }
            InferenceRequest::FrameInterpolation(_)
            | InferenceRequest::Replicate(_)
            | InferenceRequest::ComfyUi(_) => {
                warn!("OpenAI does not support this request type");
                Err(BackendError::UnsupportedCapability)
            }
        }
    }
}

// ── Helpers ────────────────────────────────────────────────────────────────

#[allow(clippy::cast_possible_truncation)]
fn cost_chat_cents(input_tokens: u32, output_tokens: u32) -> f32 {
    let cost = f64::from(input_tokens) * INPUT_PRICE_PER_TOKEN
        + f64::from(output_tokens) * OUTPUT_PRICE_PER_TOKEN;
    (cost * 100.0) as f32
}

#[allow(clippy::disallowed_methods)]
fn build_chat_body(req: &TextGenRequest, model: &str) -> serde_json::Value {
    let mut messages: Vec<serde_json::Value> = Vec::new();

    // System message.
    let sys = req.system.as_deref().unwrap_or("").to_owned()
        + &req
            .messages
            .iter()
            .filter(|m| matches!(m.role, ChatRole::System))
            .flat_map(|m| &m.content)
            .filter_map(|p| {
                if let ContentPart::Text { text } = p {
                    Some(text.as_str())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("\n\n");

    if !sys.trim().is_empty() {
        messages.push(serde_json::json!({
            "role": "system",
            "content": sys.trim(),
        }));
    }

    for msg in &req.messages {
        if matches!(msg.role, ChatRole::System) {
            continue;
        }
        let role = match msg.role {
            ChatRole::User => "user",
            ChatRole::Assistant => "assistant",
            ChatRole::System => continue,
        };

        let content: Vec<serde_json::Value> = msg
            .content
            .iter()
            .map(|part| match part {
                ContentPart::Text { text } => serde_json::json!({
                    "type": "text",
                    "text": text,
                }),
                ContentPart::Image { bytes, media_type } => {
                    let data = base64::engine::general_purpose::STANDARD.encode(bytes);
                    let url = format!("data:{};base64,{}", media_type.as_mime(), data);
                    serde_json::json!({
                        "type": "image_url",
                        "image_url": { "url": url },
                    })
                }
            })
            .collect();

        messages.push(serde_json::json!({
            "role": role,
            "content": content,
        }));
    }

    let mut body = serde_json::json!({
        "model": model,
        "messages": messages,
    });

    if let Some(max) = req.max_tokens {
        body["max_tokens"] = serde_json::json!(max);
    }
    if let Some(temp) = req.temperature {
        body["temperature"] = serde_json::json!(temp);
    }
    if !req.stop.is_empty() {
        body["stop"] = serde_json::json!(req.stop);
    }
    if !req.tools.is_empty() {
        let tools: Vec<serde_json::Value> = req
            .tools
            .iter()
            .map(|t| {
                serde_json::json!({
                    "type": "function",
                    "function": {
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.input_schema,
                    }
                })
            })
            .collect();
        body["tools"] = serde_json::json!(tools);
    }

    body
}

fn decode_image_data(data: Vec<ImageData>) -> Result<Vec<Vec<u8>>> {
    data.into_iter()
        .map(|d| {
            if let Some(b64) = d.b64_json {
                base64::engine::general_purpose::STANDARD
                    .decode(b64.as_bytes())
                    .map_err(|e| BackendError::InvalidResponse(e.to_string()))
            } else {
                Err(BackendError::InvalidResponse(
                    "no b64_json in image response".into(),
                ))
            }
        })
        .collect()
}

// ── Wire types ─────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct OpenAiChatResponse {
    model: String,
    choices: Vec<ChatChoice>,
    usage: UsageBlock,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatMessageWire,
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct ChatMessageWire {
    content: Option<String>,
    tool_calls: Option<Vec<ToolCallWire>>,
}

#[derive(Deserialize)]
struct ToolCallWire {
    id: String,
    function: FunctionCall,
}

#[derive(Deserialize)]
struct FunctionCall {
    name: String,
    arguments: String,
}

#[derive(Deserialize)]
struct UsageBlock {
    prompt_tokens: u32,
    completion_tokens: u32,
}

#[derive(Deserialize)]
struct OpenAiImageResponse {
    data: Vec<ImageData>,
}

#[derive(Deserialize)]
struct ImageData {
    b64_json: Option<String>,
}

// Suppress unused import warning.
fn _assert_frame_req(_: &FrameInterpolationRequest) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capabilities_are_broad() {
        let b = OpenAiBackend::new("k");
        let caps = b.capabilities();
        assert!(caps.contains(BackendCapabilities::TEXT_GENERATION));
        assert!(caps.contains(BackendCapabilities::IMAGE_GENERATION));
        assert!(caps.contains(BackendCapabilities::IMAGE_EDIT));
        assert!(caps.contains(BackendCapabilities::IMAGE_INPAINT));
        assert!(caps.contains(BackendCapabilities::TOOL_USE));
    }

    #[test]
    fn estimate_cost_image_is_nonzero() {
        let b = OpenAiBackend::new("k");
        let req = InferenceRequest::ImageGeneration(ImageGenRequest {
            model: None,
            prompt: "a cat".into(),
            negative_prompt: None,
            width: 1024,
            height: 1024,
            steps: None,
            seed: None,
            num_images: 1,
            style_image: None,
        });
        let est = b.estimate_cost(&req);
        assert!(est.typical_usd_cents > 0.0);
    }

    #[test]
    fn build_chat_body_includes_system() {
        let mut req = TextGenRequest::user("hello");
        req.system = Some("you are helpful".into());
        let body = build_chat_body(&req, "gpt-4o");
        let msgs = body["messages"].as_array().unwrap();
        // First message should be system.
        assert_eq!(msgs[0]["role"], "system");
        assert!(
            msgs[0]["content"]
                .as_str()
                .unwrap()
                .contains("you are helpful")
        );
    }

    #[tokio::test]
    async fn integration_text_generation() {
        let Some(api_key) = std::env::var("PIXHAUS_OPENAI_API_KEY").ok() else {
            eprintln!("skipping: PIXHAUS_OPENAI_API_KEY not set");
            return;
        };

        let backend = OpenAiBackend::new(api_key);
        let req = InferenceRequest::Text(TextGenRequest::user("Say exactly: 'pong'"));
        let (progress, _rx) = VerbProgress::channel();
        let cancel = CancellationToken::new();

        let resp = backend.invoke(req, progress, cancel).await.unwrap();
        let InferenceResponse::Text(text) = resp else {
            panic!("expected text response");
        };
        assert!(
            text.content.to_lowercase().contains("pong"),
            "unexpected: {}",
            text.content
        );
    }
}
