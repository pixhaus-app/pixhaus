//! Anthropic Claude backend adapter.
//!
//! Supports text generation, vision-language queries, and tool-use
//! (function calling) over the non-streaming `/v1/messages` endpoint.
//! SSE streaming will land alongside the verb runtime's stream sink
//! (S23+); today every call awaits the complete response.
//!
//! No official Rust SDK exists as of May 2026; this is a hand-rolled
//! HTTP client as recommended in `docs/planning/ecosystem/03-ai-ml.md`.
//!
//! # Models
//!
//! Default model: `claude-sonnet-4-6`. Override per-request via
//! [`TextGenRequest::model`].
//!
//! # Pricing (May 2026 estimates)
//!
//! - Input: $3.00 / `MTok` (Sonnet 4.6)
//! - Output: $15.00 / `MTok` (Sonnet 4.6)

use std::time::Duration;

use async_trait::async_trait;
use base64::Engine as _;
use serde::Deserialize;
use tokio::select;
use tokio_util::sync::CancellationToken;
use tracing::{debug, instrument, warn};

use super::{
    BackendError, ChatRole, ContentPart, ImageGenRequest, ImageGenResponse, InferenceBackend,
    InferenceRequest, InferenceResponse, Result, TextGenRequest, TextGenResponse, ToolCall,
    VerbProgress, check_http_status,
};
use crate::plugin::descriptor::{BackendCapabilities, CostEstimate};
use crate::plugin::progress::{CostUpdate, VerbProgressEvent};

/// Anthropic API base URL.
const BASE_URL: &str = "https://api.anthropic.com/v1";

/// Default model to use when the request does not specify one.
const DEFAULT_MODEL: &str = "claude-sonnet-4-6";

/// Anthropic API version header value.
const API_VERSION: &str = "2023-06-01";

/// Input token price in USD per token (Sonnet 4.6 estimate).
const INPUT_PRICE_PER_TOKEN: f64 = 3.0 / 1_000_000.0;

/// Output token price in USD per token (Sonnet 4.6 estimate).
const OUTPUT_PRICE_PER_TOKEN: f64 = 15.0 / 1_000_000.0;

/// Anthropic backend adapter.
///
/// Construct via [`AnthropicBackend::new`] (explicit key) or
/// [`AnthropicBackend::from_keychain`] (OS keychain).
pub struct AnthropicBackend {
    client: reqwest::Client,
    api_key: String,
    base_url: String,
    default_model: String,
}

impl std::fmt::Debug for AnthropicBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AnthropicBackend")
            .field("base_url", &self.base_url)
            .field("default_model", &self.default_model)
            .field("api_key", &"[redacted]")
            .finish_non_exhaustive()
    }
}

impl AnthropicBackend {
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
            default_model: DEFAULT_MODEL.into(),
        }
    }

    /// Constructs an adapter by reading the API key from the OS keychain.
    ///
    /// Keychain entry: `"pixhaus.anthropic"`.
    pub fn from_keychain() -> Result<Self> {
        let key = super::ApiKeyStore::get("anthropic")?;
        Ok(Self::new(key))
    }

    /// Overrides the default model. Returns `self` for chaining.
    #[must_use]
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.default_model = model.into();
        self
    }

    /// Overrides the base URL. Useful for testing against a proxy or mock.
    #[must_use]
    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    async fn call_messages(
        &self,
        req: &TextGenRequest,
        progress: &VerbProgress,
        cancel: &CancellationToken,
    ) -> Result<TextGenResponse> {
        let model = req
            .model
            .as_deref()
            .unwrap_or(self.default_model.as_str())
            .to_owned();

        let body = build_messages_body(req, &model);
        debug!(model = %model, "sending Anthropic messages request");

        let http_req = self
            .client
            .post(format!("{}/messages", self.base_url))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", API_VERSION)
            .header("content-type", "application/json")
            .json(&body)
            .build()
            .map_err(BackendError::Network)?;

        let http_resp = select! {
            biased;
            () = cancel.cancelled() => return Err(BackendError::Cancelled),
            res = self.client.execute(http_req) => res.map_err(BackendError::Network)?,
        };

        let http_resp = check_http_status(http_resp).await?;
        let raw: AnthropicResponse = http_resp.json().await.map_err(BackendError::Network)?;

        let content = raw
            .content
            .iter()
            .filter_map(|block| {
                if block.block_type == "text" {
                    block.text.as_deref()
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("");

        let tool_calls = raw
            .content
            .iter()
            .filter(|b| b.block_type == "tool_use")
            .filter_map(|b| {
                Some(ToolCall {
                    id: b.id.clone()?,
                    name: b.name.clone()?,
                    input: b.input.clone().unwrap_or(serde_json::Value::Null),
                })
            })
            .collect();

        let input_tokens = raw.usage.as_ref().map_or(0, |u| u.input_tokens);
        let output_tokens = raw.usage.as_ref().map_or(0, |u| u.output_tokens);

        let cost_cents = cost_usd_cents(input_tokens, output_tokens);
        progress
            .send(VerbProgressEvent::Cost(CostUpdate {
                usd_cents: cost_cents,
                tokens_input: Some(input_tokens),
                tokens_output: Some(output_tokens),
            }))
            .await;

        Ok(TextGenResponse {
            content,
            model: raw.model.unwrap_or_else(|| model.clone()),
            input_tokens,
            output_tokens,
            stop_reason: raw.stop_reason,
            tool_calls,
        })
    }
}

#[async_trait]
impl InferenceBackend for AnthropicBackend {
    fn backend_id(&self) -> &'static str {
        "anthropic"
    }

    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities::TEXT_GENERATION
            .union(BackendCapabilities::VISION_LANGUAGE)
            .union(BackendCapabilities::TOOL_USE)
    }

    fn supports_streaming(&self) -> bool {
        true
    }

    fn estimate_cost(&self, request: &InferenceRequest) -> CostEstimate {
        match request {
            InferenceRequest::Text(req) => {
                // Rough character-count heuristic: ~4 chars per token.
                let chars: usize = req
                    .messages
                    .iter()
                    .flat_map(|m| &m.content)
                    .map(|p| match p {
                        ContentPart::Text { text } => text.len(),
                        ContentPart::Image { .. } => 1000, // vision tokens ≈ 1K each
                    })
                    .sum();
                #[allow(clippy::cast_possible_truncation)]
                let input_tokens = (chars / 4).max(1) as u32;
                let output_tokens = req.max_tokens.unwrap_or(1024);
                let cents = cost_usd_cents(input_tokens, output_tokens);
                CostEstimate {
                    typical_latency: Duration::from_secs(3),
                    max_latency: Duration::from_secs(30),
                    typical_usd_cents: cents,
                    max_usd_cents: cents * 3.0,
                }
            }
            _ => CostEstimate {
                typical_latency: Duration::ZERO,
                max_latency: Duration::ZERO,
                typical_usd_cents: 0.0,
                max_usd_cents: 0.0,
            },
        }
    }

    #[instrument(skip(self, request, progress, cancel), fields(backend = "anthropic"))]
    async fn invoke(
        &self,
        request: InferenceRequest,
        progress: VerbProgress,
        cancel: CancellationToken,
    ) -> Result<InferenceResponse> {
        match request {
            InferenceRequest::Text(req) => {
                progress
                    .send(VerbProgressEvent::Started {
                        backend: Some("anthropic".into()),
                    })
                    .await;
                let resp = self.call_messages(&req, &progress, &cancel).await?;
                Ok(InferenceResponse::Text(resp))
            }
            InferenceRequest::ImageGeneration(_)
            | InferenceRequest::ImageEdit(_)
            | InferenceRequest::ImageInpaint(_)
            | InferenceRequest::FrameInterpolation(_)
            | InferenceRequest::Replicate(_)
            | InferenceRequest::ComfyUi(_) => {
                warn!("Anthropic does not support this request type");
                Err(BackendError::UnsupportedCapability)
            }
        }
    }
}

// ── Wire types ─────────────────────────────────────────────────────────────

#[allow(clippy::cast_possible_truncation)]
fn cost_usd_cents(input_tokens: u32, output_tokens: u32) -> f32 {
    let cost = f64::from(input_tokens) * INPUT_PRICE_PER_TOKEN
        + f64::from(output_tokens) * OUTPUT_PRICE_PER_TOKEN;
    // Convert USD to cents.
    (cost * 100.0) as f32
}

/// Builds the request body for the Anthropic `/v1/messages` endpoint.
#[allow(clippy::disallowed_methods)]
fn build_messages_body(req: &TextGenRequest, model: &str) -> serde_json::Value {
    let mut messages = Vec::new();

    for msg in &req.messages {
        // Skip system messages — they go in the top-level `system` field.
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
                    serde_json::json!({
                        "type": "image",
                        "source": {
                            "type": "base64",
                            "media_type": media_type.as_mime(),
                            "data": data,
                        }
                    })
                }
            })
            .collect();

        messages.push(serde_json::json!({
            "role": role,
            "content": content,
        }));
    }

    // Collect system text from system-role messages.
    let system_text: Option<String> = {
        let parts: Vec<&str> = req
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
            .collect();

        // Merge with the top-level system field, if any.
        let combined = [req.system.as_deref().unwrap_or("")]
            .iter()
            .chain(parts.iter())
            .filter(|s| !s.is_empty())
            .copied()
            .collect::<Vec<_>>()
            .join("\n\n");

        if combined.is_empty() {
            None
        } else {
            Some(combined)
        }
    };

    let mut body = serde_json::json!({
        "model": model,
        "max_tokens": req.max_tokens.unwrap_or(4096),
        "messages": messages,
    });

    if let Some(system) = system_text {
        body["system"] = serde_json::Value::String(system);
    }

    if let Some(temp) = req.temperature {
        body["temperature"] = serde_json::json!(temp);
    }

    if !req.stop.is_empty() {
        body["stop_sequences"] = serde_json::json!(req.stop);
    }

    if !req.tools.is_empty() {
        let tools: Vec<serde_json::Value> = req
            .tools
            .iter()
            .map(|t| {
                serde_json::json!({
                    "name": t.name,
                    "description": t.description,
                    "input_schema": t.input_schema,
                })
            })
            .collect();
        body["tools"] = serde_json::json!(tools);
    }

    body
}

// Anthropic response shapes ─────────────────────────────────────────────────

#[derive(Deserialize)]
struct AnthropicResponse {
    model: Option<String>,
    stop_reason: Option<String>,
    content: Vec<ContentBlock>,
    usage: Option<UsageBlock>,
}

#[derive(Deserialize)]
struct ContentBlock {
    #[serde(rename = "type")]
    block_type: String,
    text: Option<String>,
    // Tool-use fields.
    id: Option<String>,
    name: Option<String>,
    input: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct UsageBlock {
    input_tokens: u32,
    output_tokens: u32,
}

// Unused here but kept for symmetry and future streaming implementation.
#[allow(dead_code)]
#[derive(Deserialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: u32,
    messages: Vec<serde_json::Value>,
}

// Suppress unused import warning for ImageGenRequest which is referenced in
// the match arm types.
fn _assert_image_gen_req_visible(_: &ImageGenRequest) {}
fn _assert_image_gen_resp_visible(_: &ImageGenResponse) {}

// ── VerbRuntime bridge ──────────────────────────────────────────────────────
//
// The VerbRuntime (S21) stores backends as `Arc<dyn
// plugin::backend::InferenceBackend>` — a thin discovery trait. Adapters
// also implement the operational `backends::InferenceBackend` with full
// `invoke` semantics. This bridge impl lets AnthropicBackend be registered
// in the VerbRuntime while remaining callable from verb code via
// `as_any().downcast_ref::<AnthropicBackend>()`.

impl crate::plugin::backend::InferenceBackend for AnthropicBackend {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn id(&self) -> &str {
        self.backend_id()
    }

    fn capabilities(&self) -> BackendCapabilities {
        <Self as super::InferenceBackend>::capabilities(self)
    }

    fn cost_estimate(
        &self,
        _required: BackendCapabilities,
    ) -> crate::plugin::descriptor::CostEstimate {
        crate::plugin::descriptor::CostEstimate {
            typical_latency: std::time::Duration::from_secs(5),
            max_latency: std::time::Duration::from_secs(60),
            typical_usd_cents: 1.0,
            max_usd_cents: 10.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backends::{ChatMessage, TextGenRequest};

    fn make_req(text: &str) -> TextGenRequest {
        TextGenRequest::user(text)
    }

    #[test]
    fn cost_usd_cents_scales_linearly() {
        let c = cost_usd_cents(1_000_000, 0);
        // 1M input tokens at $3/MTok = $3 = 300 cents.
        assert!((c - 300.0).abs() < 1.0, "expected ~300 cents, got {c}");

        let c = cost_usd_cents(0, 1_000_000);
        // 1M output tokens at $15/MTok = $15 = 1500 cents.
        assert!((c - 1500.0).abs() < 1.0, "expected ~1500 cents, got {c}");
    }

    #[test]
    fn build_messages_body_includes_model_and_messages() {
        let req = make_req("hello");
        let body = build_messages_body(&req, "claude-sonnet-4-6");
        assert_eq!(body["model"], "claude-sonnet-4-6");
        assert!(body["messages"].is_array());
        let msgs = body["messages"].as_array().unwrap();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0]["role"], "user");
    }

    #[test]
    fn build_messages_body_merges_system_field_and_role() {
        let mut req = make_req("user turn");
        req.system = Some("top-level system".into());
        // Also add a system-role message.
        req.messages.insert(
            0,
            ChatMessage {
                role: ChatRole::System,
                content: vec![ContentPart::text("role system")],
            },
        );
        let body = build_messages_body(&req, "m");
        let system = body["system"].as_str().unwrap();
        assert!(system.contains("top-level system"));
        assert!(system.contains("role system"));
        // System messages should not appear in the messages array.
        let msgs = body["messages"].as_array().unwrap();
        assert!(msgs.iter().all(|m| m["role"] != "system"));
    }

    #[test]
    fn build_messages_body_encodes_image_as_base64() {
        use crate::backends::ImageMediaType;
        let mut req = make_req("describe this");
        req.messages[0].content.push(ContentPart::Image {
            bytes: vec![1, 2, 3],
            media_type: ImageMediaType::Png,
        });
        let body = build_messages_body(&req, "m");
        let msgs = body["messages"].as_array().unwrap();
        let content = msgs[0]["content"].as_array().unwrap();
        assert_eq!(content.len(), 2);
        assert_eq!(content[1]["type"], "image");
        assert_eq!(content[1]["source"]["media_type"], "image/png");
        let data = content[1]["source"]["data"].as_str().unwrap();
        // Verify it's valid base64 that decodes to our bytes.
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(data)
            .unwrap();
        assert_eq!(decoded, vec![1, 2, 3]);
    }

    #[test]
    fn capabilities_include_text_vision_tool() {
        let backend = AnthropicBackend::new("key");
        let caps = backend.capabilities();
        assert!(caps.contains(BackendCapabilities::TEXT_GENERATION));
        assert!(caps.contains(BackendCapabilities::VISION_LANGUAGE));
        assert!(caps.contains(BackendCapabilities::TOOL_USE));
        assert!(!caps.contains(BackendCapabilities::IMAGE_GENERATION));
    }

    #[test]
    fn estimate_cost_text_is_nonzero() {
        let backend = AnthropicBackend::new("key");
        let req = InferenceRequest::Text(TextGenRequest::user("a long prompt for estimation"));
        let est = backend.estimate_cost(&req);
        assert!(est.typical_usd_cents >= 0.0);
        assert!(est.typical_latency > Duration::ZERO);
    }

    #[tokio::test]
    async fn integration_text_generation() {
        let Some(api_key) = std::env::var("PIXHAUS_ANTHROPIC_API_KEY").ok() else {
            eprintln!("skipping: PIXHAUS_ANTHROPIC_API_KEY not set");
            return;
        };

        let backend = AnthropicBackend::new(api_key);
        let req = InferenceRequest::Text(TextGenRequest::user("Say exactly: 'pong'"));
        let (progress, _rx) = VerbProgress::channel();
        let cancel = CancellationToken::new();

        let resp = backend.invoke(req, progress, cancel).await.unwrap();
        let InferenceResponse::Text(text) = resp else {
            panic!("expected text response");
        };
        assert!(
            text.content.to_lowercase().contains("pong"),
            "unexpected response: {}",
            text.content
        );
        assert!(text.input_tokens > 0);
        assert!(text.output_tokens > 0);
    }

    #[tokio::test]
    async fn integration_unsupported_capability_returns_error() {
        let Some(api_key) = std::env::var("PIXHAUS_ANTHROPIC_API_KEY").ok() else {
            eprintln!("skipping: PIXHAUS_ANTHROPIC_API_KEY not set");
            return;
        };

        let backend = AnthropicBackend::new(api_key);
        let req = InferenceRequest::ImageGeneration(ImageGenRequest {
            model: None,
            prompt: "a cat".into(),
            negative_prompt: None,
            width: 64,
            height: 64,
            steps: None,
            seed: None,
            num_images: 1,
            style_image: None,
        });
        let (progress, _rx) = VerbProgress::channel();
        let cancel = CancellationToken::new();

        let result = backend.invoke(req, progress, cancel).await;
        assert!(matches!(result, Err(BackendError::UnsupportedCapability)));
    }
}
