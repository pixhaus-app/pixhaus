//! `OpenAI` backend adapter.
//!
//! Supports text generation, vision-language queries, image generation,
//! image editing/inpainting, and tool-use over `/v1/chat/completions` and
//! `/v1/images/generations`. Embeddings are not yet wired through the
//! verb runtime — the request type lands alongside the first verb that
//! needs it. Implemented as a raw HTTP client for consistency with the
//! other adapters.
//!
//! # Models
//!
//! - Text / vision: `gpt-4o` (default); override per-request.
//! - Image generation: `gpt-image-2` (default).
//! - Image edit/inpaint: `gpt-image-2`.
//!
//! # Pricing (May 2026 estimates)
//!
//! - GPT-4o input: $2.50 / `MTok`
//! - GPT-4o output: $10.00 / `MTok`
//! - GPT image generation varies by output size and quality.

use std::time::Duration;

use async_trait::async_trait;
use base64::Engine as _;
use futures::StreamExt;
use serde::Deserialize;
use tokio::select;
use tokio_util::sync::CancellationToken;
use tracing::{debug, instrument, warn};

use super::{
    BackendError, ChatRole, ContentPart, FrameInterpolationRequest, ImageEditRequest,
    ImageGenRequest, ImageGenResponse, InferenceBackend, InferenceRequest, InferenceResponse,
    Result, TextGenRequest, TextGenResponse, ToolCall, VerbProgress, check_http_status,
};
use crate::plugin::context::PixelData;
use crate::plugin::descriptor::{BackendCapabilities, CostEstimate};
use crate::plugin::progress::{CostUpdate, VerbProgressEvent};

const BASE_URL: &str = "https://api.openai.com/v1";
const DEFAULT_TEXT_MODEL: &str = "gpt-4o";
/// Default `OpenAI` image model for Pixhaus image generation.
pub const DEFAULT_IMAGE_MODEL: &str = "gpt-image-2";
const DEFAULT_IMAGE_EDIT_MODEL: &str = "gpt-image-2";

const INPUT_PRICE_PER_TOKEN: f64 = 2.50 / 1_000_000.0;
const OUTPUT_PRICE_PER_TOKEN: f64 = 10.00 / 1_000_000.0;
const IMAGE_PRICE_CENTS: f32 = 8.0; // Placeholder estimate; provider pricing varies by model/size/quality.

/// `OpenAI` backend adapter.
pub struct OpenAiBackend {
    client: reqwest::Client,
    api_key: String,
    base_url: String,
    text_model: String,
    image_model: String,
    image_edit_model: String,
}

impl std::fmt::Debug for OpenAiBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenAiBackend")
            .field("base_url", &self.base_url)
            .field("text_model", &self.text_model)
            .field("image_model", &self.image_model)
            .field("image_edit_model", &self.image_edit_model)
            .field("api_key", &"[redacted]")
            .finish_non_exhaustive()
    }
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
            image_edit_model: DEFAULT_IMAGE_EDIT_MODEL.into(),
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

    /// Overrides the default image generation model.
    #[must_use]
    pub fn with_image_model(mut self, model: impl Into<String>) -> Self {
        self.image_model = model.into();
        self
    }

    /// Overrides the default image edit/inpaint model.
    #[must_use]
    pub fn with_image_edit_model(mut self, model: impl Into<String>) -> Self {
        self.image_edit_model = model.into();
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
            .map(parse_tool_call)
            .collect::<Result<Vec<_>>>()?;

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

        debug!(model = %model, "sending OpenAI image generation request");

        if !req.reference_images.is_empty() {
            return self
                .generate_image_with_references(req, &model, progress, cancel)
                .await;
        }

        let body = build_image_generation_body(req, &model);

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
        if use_image_stream(&model, req.num_images) {
            let images = parse_openai_image_stream(http_resp, progress, cancel).await?;
            progress
                .send(VerbProgressEvent::Cost(CostUpdate {
                    usd_cents: IMAGE_PRICE_CENTS * req.num_images as f32,
                    tokens_input: None,
                    tokens_output: None,
                }))
                .await;
            return Ok(ImageGenResponse { images, model });
        }
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
    async fn generate_image_with_references(
        &self,
        req: &ImageGenRequest,
        model: &str,
        progress: &VerbProgress,
        cancel: &CancellationToken,
    ) -> Result<ImageGenResponse> {
        let image_field = if is_gpt_image_model(model) {
            "image[]"
        } else {
            "image"
        };
        let mut form = reqwest::multipart::Form::new()
            .text("prompt", req.prompt.clone())
            .text("n", req.num_images.to_string())
            .text("model", model.to_owned())
            .text("size", format!("{}x{}", req.width, req.height));
        if is_gpt_image_model(model) {
            form = form.text("output_format", "png");
            if use_image_stream(model, req.num_images) {
                form = form.text("stream", "true");
                form = form.text("partial_images", "2");
            }
            if let Some(quality) = req.quality {
                form = form.text("quality", quality.as_openai());
            }
        } else {
            form = form.text("response_format", "b64_json");
        }
        for (index, image) in req.reference_images.iter().enumerate() {
            form = form.part(
                image_field,
                reqwest::multipart::Part::bytes(image.clone())
                    .file_name(format!("reference-{index}.png"))
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
        if use_image_stream(model, req.num_images) {
            let images = parse_openai_image_stream(http_resp, progress, cancel).await?;
            progress
                .send(VerbProgressEvent::Cost(CostUpdate {
                    usd_cents: IMAGE_PRICE_CENTS * req.num_images as f32,
                    tokens_input: None,
                    tokens_output: None,
                }))
                .await;
            return Ok(ImageGenResponse {
                images,
                model: model.to_owned(),
            });
        }
        let raw: OpenAiImageResponse = http_resp.json().await.map_err(BackendError::Network)?;
        let images = decode_image_data(raw.data)?;
        progress
            .send(VerbProgressEvent::Cost(CostUpdate {
                usd_cents: IMAGE_PRICE_CENTS * req.num_images as f32,
                tokens_input: None,
                tokens_output: None,
            }))
            .await;
        Ok(ImageGenResponse {
            images,
            model: model.to_owned(),
        })
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
            .unwrap_or(self.image_edit_model.as_str())
            .to_owned();

        debug!(model = %model, "sending OpenAI image edit request");

        // Image edits use multipart/form-data. GPT image models return
        // base64 by default and do not accept the legacy
        // `response_format` field.
        let is_gpt_image = is_gpt_image_model(&model);
        let image_field = if is_gpt_image { "image[]" } else { "image" };
        let mut form = reqwest::multipart::Form::new()
            .text("prompt", req.prompt.clone())
            .text("n", req.num_images.to_string())
            .text("model", model.clone())
            .part(
                image_field,
                reqwest::multipart::Part::bytes(req.image.clone())
                    .file_name("image.png")
                    .mime_str("image/png")
                    .map_err(|e| BackendError::Other(e.to_string()))?,
            );
        if is_gpt_image {
            form = form.text("output_format", "png");
            if use_image_stream(&model, req.num_images) {
                form = form.text("stream", "true");
                form = form.text("partial_images", "2");
            }
        } else {
            form = form.text("response_format", "b64_json");
        }

        if let Some(mask) = &req.mask {
            form = form.part(
                "mask",
                reqwest::multipart::Part::bytes(mask.clone())
                    .file_name("mask.png")
                    .mime_str("image/png")
                    .map_err(|e| BackendError::Other(e.to_string()))?,
            );
        }
        for (index, image) in req.reference_images.iter().enumerate() {
            form = form.part(
                image_field,
                reqwest::multipart::Part::bytes(image.clone())
                    .file_name(format!("reference-{index}.png"))
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
        if use_image_stream(&model, req.num_images) {
            let images = parse_openai_image_stream(http_resp, progress, cancel).await?;
            progress
                .send(VerbProgressEvent::Cost(CostUpdate {
                    usd_cents: IMAGE_PRICE_CENTS * req.num_images as f32,
                    tokens_input: None,
                    tokens_output: None,
                }))
                .await;
            return Ok(ImageGenResponse { images, model });
        }
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
        // EMBEDDINGS intentionally not advertised: there is no
        // embeddings request type yet, so claiming the capability would
        // make this backend selectable for a request it cannot satisfy.
        // Wire it back in when the verb runtime grows an embeddings
        // request variant.
        BackendCapabilities::TEXT_GENERATION
            .union(BackendCapabilities::VISION_LANGUAGE)
            .union(BackendCapabilities::IMAGE_GENERATION)
            .union(BackendCapabilities::IMAGE_EDIT)
            .union(BackendCapabilities::IMAGE_INPAINT)
            .union(BackendCapabilities::TOOL_USE)
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

impl crate::plugin::backend::InferenceBackend for OpenAiBackend {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn id(&self) -> &'static str {
        "openai"
    }

    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities::TEXT_GENERATION
            .union(BackendCapabilities::VISION_LANGUAGE)
            .union(BackendCapabilities::IMAGE_GENERATION)
            .union(BackendCapabilities::IMAGE_EDIT)
            .union(BackendCapabilities::IMAGE_INPAINT)
            .union(BackendCapabilities::TOOL_USE)
    }

    fn cost_estimate(&self, required: BackendCapabilities) -> CostEstimate {
        let image_caps = BackendCapabilities::IMAGE_GENERATION
            .union(BackendCapabilities::IMAGE_EDIT)
            .union(BackendCapabilities::IMAGE_INPAINT);
        if required.intersection(image_caps).is_empty() {
            CostEstimate {
                typical_latency: Duration::from_secs(3),
                max_latency: Duration::from_secs(30),
                typical_usd_cents: 0.5,
                max_usd_cents: 5.0,
            }
        } else {
            CostEstimate {
                typical_latency: Duration::from_secs(8),
                max_latency: Duration::from_secs(30),
                typical_usd_cents: IMAGE_PRICE_CENTS,
                max_usd_cents: IMAGE_PRICE_CENTS * 2.0,
            }
        }
    }

    fn is_available(&self) -> bool {
        !self.api_key.is_empty()
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

    // System message: merge the explicit `req.system` with any system-role
    // messages from the conversation. Both sources are joined with "\n\n"
    // so a non-empty `req.system` followed by a system-role message reads
    // as two paragraphs rather than running together.
    let mut sys_parts: Vec<&str> = Vec::new();
    if let Some(s) = req.system.as_deref()
        && !s.is_empty()
    {
        sys_parts.push(s);
    }
    sys_parts.extend(
        req.messages
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
            .filter(|s| !s.is_empty()),
    );
    let sys = sys_parts.join("\n\n");

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

#[allow(clippy::disallowed_methods)]
fn build_image_generation_body(req: &ImageGenRequest, model: &str) -> serde_json::Value {
    let mut body = serde_json::json!({
        "model": model,
        "prompt": req.prompt,
        "n": req.num_images,
        "size": format!("{}x{}", req.width, req.height),
    });

    if is_gpt_image_model(model) {
        body["output_format"] = serde_json::json!("png");
        if use_image_stream(model, req.num_images) {
            body["stream"] = serde_json::json!(true);
            body["partial_images"] = serde_json::json!(2);
        }
        if let Some(quality) = req.quality {
            body["quality"] = serde_json::json!(quality.as_openai());
        }
    } else {
        body["response_format"] = serde_json::json!("b64_json");
    }

    body
}

fn is_gpt_image_model(model: &str) -> bool {
    model.starts_with("gpt-image-") || model == "chatgpt-image-latest"
}

/// Whether to request server-sent partial frames for an image call.
///
/// `OpenAI` only streams gpt-image responses when `n == 1`; asking for
/// `stream=true` alongside `n > 1` is rejected with HTTP 400
/// ("Streaming is only supported with n=1."). Multi-candidate requests
/// therefore fall back to the buffered JSON response.
fn use_image_stream(model: &str, num_images: u32) -> bool {
    is_gpt_image_model(model) && num_images == 1
}

/// Converts one wire-level tool call into the public [`ToolCall`].
///
/// Returns [`BackendError::InvalidResponse`] when the model produced an
/// `arguments` payload that isn't valid JSON. Without this guard the
/// runtime would receive `null` arguments and silently misroute the
/// tool — a parser failure is the kind of corruption callers must see.
fn parse_tool_call(tc: ToolCallWire) -> Result<ToolCall> {
    let input = serde_json::from_str(&tc.function.arguments).map_err(|e| {
        BackendError::InvalidResponse(format!(
            "OpenAI tool_call '{}' arguments not valid JSON: {e}",
            tc.function.name
        ))
    })?;
    Ok(ToolCall {
        id: tc.id,
        name: tc.function.name,
        input,
    })
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

async fn parse_openai_image_stream(
    response: reqwest::Response,
    progress: &VerbProgress,
    cancel: &CancellationToken,
) -> Result<Vec<Vec<u8>>> {
    let mut stream = response.bytes_stream();
    let mut buffer = String::new();
    let mut final_images = Vec::new();
    while let Some(chunk) = select! {
        biased;
        () = cancel.cancelled() => return Err(BackendError::Cancelled),
        next = stream.next() => next,
    } {
        let chunk = chunk.map_err(BackendError::Network)?;
        buffer.push_str(&String::from_utf8_lossy(&chunk));
        while let Some(index) = buffer.find("\n\n") {
            let frame = buffer[..index].to_owned();
            buffer.drain(..index + 2);
            handle_openai_sse_frame(&frame, progress, &mut final_images).await?;
        }
    }
    if !buffer.trim().is_empty() {
        handle_openai_sse_frame(&buffer, progress, &mut final_images).await?;
    }
    if final_images.is_empty() {
        return Err(BackendError::InvalidResponse(
            "OpenAI image stream ended without a final image".into(),
        ));
    }
    Ok(final_images)
}

async fn handle_openai_sse_frame(
    frame: &str,
    progress: &VerbProgress,
    final_images: &mut Vec<Vec<u8>>,
) -> Result<()> {
    let data = frame
        .lines()
        .filter_map(|line| line.strip_prefix("data:"))
        .map(str::trim)
        .collect::<Vec<_>>()
        .join("\n");
    if data.is_empty() || data == "[DONE]" {
        return Ok(());
    }
    let value: serde_json::Value = serde_json::from_str(&data)?;
    let event_type = value
        .get("type")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let mut images = Vec::new();
    collect_base64_images(&value, &mut images);
    if images.is_empty() {
        return Ok(());
    }
    if event_type.contains("partial") {
        for (index, image) in images.into_iter().enumerate() {
            if let Some(pixels) = image_bytes_to_pixel_data(&image) {
                progress
                    .send(VerbProgressEvent::PartialPixels {
                        effect_index: u32::try_from(index).unwrap_or(u32::MAX),
                        pixels,
                    })
                    .await;
            }
        }
    } else {
        final_images.extend(images);
    }
    Ok(())
}

fn collect_base64_images(value: &serde_json::Value, images: &mut Vec<Vec<u8>>) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, value) in map {
                if key.contains("b64")
                    && let Some(encoded) = value.as_str()
                    && let Ok(bytes) =
                        base64::engine::general_purpose::STANDARD.decode(encoded.as_bytes())
                {
                    images.push(bytes);
                    continue;
                }
                collect_base64_images(value, images);
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                collect_base64_images(value, images);
            }
        }
        _ => {}
    }
}

fn image_bytes_to_pixel_data(bytes: &[u8]) -> Option<PixelData> {
    let image = image::load_from_memory(bytes).ok()?.to_rgba8();
    let (width, height) = image.dimensions();
    Some(PixelData::rgba8(width, height, image.into_raw()))
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
    use crate::backends::ImageQuality;

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
        // EMBEDDINGS is intentionally not advertised — there is no
        // embeddings request type yet.
        assert!(!caps.contains(BackendCapabilities::EMBEDDINGS));
    }

    #[test]
    fn parse_tool_call_returns_invalid_response_on_malformed_json() {
        let tc = ToolCallWire {
            id: "call_1".into(),
            function: FunctionCall {
                name: "set_pixel".into(),
                arguments: "{not_valid_json".into(),
            },
        };
        let err = parse_tool_call(tc).expect_err("malformed JSON must error");
        match err {
            BackendError::InvalidResponse(msg) => {
                assert!(
                    msg.contains("set_pixel"),
                    "error should mention the offending tool name: {msg}"
                );
            }
            other => panic!("expected InvalidResponse, got {other:?}"),
        }
    }

    #[test]
    fn parse_tool_call_passes_valid_arguments_through() {
        let tc = ToolCallWire {
            id: "call_2".into(),
            function: FunctionCall {
                name: "set_pixel".into(),
                arguments: r##"{"x":3,"y":4,"color":"#ff0000"}"##.into(),
            },
        };
        let parsed = parse_tool_call(tc).expect("valid JSON must parse");
        assert_eq!(parsed.id, "call_2");
        assert_eq!(parsed.name, "set_pixel");
        assert_eq!(parsed.input["x"], 3);
        assert_eq!(parsed.input["color"], "#ff0000");
    }

    #[test]
    fn build_chat_body_merges_system_with_separator() {
        use super::super::ChatMessage;
        // Both `req.system` and a system-role message in `req.messages`
        // must be joined with "\n\n" so they read as separate paragraphs
        // rather than running together.
        let mut req = TextGenRequest::user("hello");
        req.system = Some("you are helpful".into());
        req.messages.insert(
            0,
            ChatMessage {
                role: ChatRole::System,
                content: vec![ContentPart::text("be terse")],
            },
        );
        let body = build_chat_body(&req, "gpt-4o");
        let msgs = body["messages"].as_array().unwrap();
        assert_eq!(msgs[0]["role"], "system");
        let combined = msgs[0]["content"].as_str().unwrap();
        assert!(
            combined.contains("you are helpful\n\nbe terse"),
            "system parts must be joined with two newlines, got: {combined:?}"
        );
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
            quality: None,
            style_image: None,
            reference_images: Vec::new(),
        });
        let est = b.estimate_cost(&req);
        assert!(est.typical_usd_cents > 0.0);
    }

    #[test]
    fn gpt_image_single_candidate_body_streams() {
        let req = ImageGenRequest {
            model: None,
            prompt: "a reference sheet".into(),
            negative_prompt: Some("blurry".into()),
            width: 1024,
            height: 1536,
            steps: None,
            seed: None,
            num_images: 1,
            quality: Some(ImageQuality::Medium),
            style_image: None,
            reference_images: Vec::new(),
        };

        let body = build_image_generation_body(&req, "gpt-image-2");

        assert_eq!(body["model"], "gpt-image-2");
        assert_eq!(body["size"], "1024x1536");
        assert_eq!(body["quality"], "medium");
        assert_eq!(body["output_format"], "png");
        assert_eq!(body["stream"], true);
        assert_eq!(body["partial_images"], 2);
        assert!(body.get("response_format").is_none());
        assert!(body.get("background").is_none());
        assert!(body.get("negative_prompt").is_none());
    }

    // OpenAI rejects `stream=true` when `n > 1`
    // ("Streaming is only supported with n=1."), so multi-candidate
    // requests must drop streaming and fall back to the JSON response.
    #[test]
    fn gpt_image_multi_candidate_body_drops_streaming() {
        let req = ImageGenRequest {
            model: None,
            prompt: "a reference sheet".into(),
            negative_prompt: None,
            width: 1024,
            height: 1536,
            steps: None,
            seed: None,
            num_images: 2,
            quality: Some(ImageQuality::Medium),
            style_image: None,
            reference_images: Vec::new(),
        };

        let body = build_image_generation_body(&req, "gpt-image-2");

        assert_eq!(body["n"], 2);
        assert_eq!(body["output_format"], "png");
        assert_eq!(body["quality"], "medium");
        assert!(body.get("stream").is_none());
        assert!(body.get("partial_images").is_none());
        assert!(body.get("response_format").is_none());
    }

    #[tokio::test]
    async fn openai_image_stream_frame_separates_partial_from_final() {
        let (progress, mut rx) = VerbProgress::channel();
        let png = one_pixel_png();
        let encoded = base64::engine::general_purpose::STANDARD.encode(&png);
        let mut finals = Vec::new();
        handle_openai_sse_frame(
            &format!("data: {{\"type\":\"image_generation.partial_image\",\"b64_json\":\"{encoded}\"}}\n\n"),
            &progress,
            &mut finals,
        )
        .await
        .unwrap();
        assert!(finals.is_empty());
        assert!(matches!(
            rx.recv().await,
            Some(VerbProgressEvent::PartialPixels { .. })
        ));
        handle_openai_sse_frame(
            &format!("data: {{\"type\":\"image_generation.completed\",\"data\":[{{\"b64_json\":\"{encoded}\"}}]}}\n\n"),
            &progress,
            &mut finals,
        )
        .await
        .unwrap();
        assert_eq!(finals, vec![png]);
    }

    fn one_pixel_png() -> Vec<u8> {
        let img = image::RgbaImage::from_pixel(1, 1, image::Rgba([255, 0, 0, 255]));
        let mut bytes = Vec::new();
        img.write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Png,
        )
        .unwrap();
        bytes
    }

    #[test]
    fn dall_e_generation_body_keeps_b64_response_format() {
        let req = ImageGenRequest {
            model: None,
            prompt: "a cat".into(),
            negative_prompt: None,
            width: 1024,
            height: 1024,
            steps: None,
            seed: None,
            num_images: 1,
            quality: Some(ImageQuality::High),
            style_image: None,
            reference_images: Vec::new(),
        };

        let body = build_image_generation_body(&req, "dall-e-3");

        assert_eq!(body["response_format"], "b64_json");
        assert!(body.get("output_format").is_none());
        assert!(body.get("quality").is_none());
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
