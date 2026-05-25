//! Ollama backend adapter.
//!
//! Ollama runs open-weight LLMs locally via a simple HTTP API. No API key
//! is required; the server is assumed to be running on the host machine.
//! Vision models (e.g. `llava`, `bakllava`) are supported when images are
//! included in the request content.
//!
//! # Default base URL
//!
//! `http://localhost:11434`. Override via [`OllamaBackend::with_base_url`]
//! for remote Ollama servers.
//!
//! # Cost
//!
//! Local inference; zero API cost.

use std::time::Duration;

use async_trait::async_trait;
use base64::Engine as _;
use serde::Deserialize;
use tokio::select;
use tokio_util::sync::CancellationToken;
use tracing::{debug, instrument, warn};

use super::{
    BackendError, ContentPart, ImageEditRequest, ImageGenRequest, InferenceBackend,
    InferenceRequest, InferenceResponse, Result, TextGenRequest, TextGenResponse, VerbProgress,
    check_http_status,
};
use crate::plugin::descriptor::{BackendCapabilities, CostEstimate};
use crate::plugin::progress::VerbProgressEvent;

const DEFAULT_BASE_URL: &str = "http://localhost:11434";
const DEFAULT_MODEL: &str = "llama3.2";

/// Ollama backend adapter.
///
/// Ollama is a local LLM runner. Construct via [`OllamaBackend::new`].
/// No keychain integration is needed since Ollama has no API key concept.
pub struct OllamaBackend {
    client: reqwest::Client,
    base_url: String,
    default_model: String,
    /// Whether the configured default model has vision support.
    ///
    /// Controls whether `VISION_LANGUAGE` appears in [`Self::capabilities`].
    /// Verbs that require vision must verify the model supports images.
    vision_capable: bool,
}

impl OllamaBackend {
    /// Constructs an adapter pointing at a local Ollama server.
    ///
    /// `vision_capable` should be `true` for models like `llava` or
    /// `bakllava` that accept image inputs.
    #[must_use]
    pub fn new(vision_capable: bool) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(300))
                .build()
                .unwrap_or_default(),
            base_url: DEFAULT_BASE_URL.into(),
            default_model: DEFAULT_MODEL.into(),
            vision_capable,
        }
    }

    /// Overrides the base URL (for remote Ollama servers or testing).
    #[must_use]
    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    /// Overrides the default model.
    #[must_use]
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.default_model = model.into();
        self
    }

    #[allow(clippy::disallowed_methods)]
    async fn call_chat(
        &self,
        req: &TextGenRequest,
        cancel: &CancellationToken,
    ) -> Result<TextGenResponse> {
        let model = req
            .model
            .as_deref()
            .unwrap_or(self.default_model.as_str())
            .to_owned();

        debug!(model = %model, "sending Ollama chat request");

        let messages = build_ollama_messages(req);

        let mut body = serde_json::json!({
            "model": model,
            "messages": messages,
            "stream": false,
        });

        if let Some(temp) = req.temperature {
            body["options"] = serde_json::json!({ "temperature": temp });
        }

        let http_req = self
            .client
            .post(format!("{}/api/chat", self.base_url))
            .json(&body)
            .build()
            .map_err(BackendError::Network)?;

        let http_resp = select! {
            biased;
            () = cancel.cancelled() => return Err(BackendError::Cancelled),
            res = self.client.execute(http_req) => res.map_err(BackendError::Network)?,
        };

        let http_resp = check_http_status(http_resp).await?;
        let raw: OllamaChatResponse = http_resp.json().await.map_err(BackendError::Network)?;

        Ok(TextGenResponse {
            content: raw.message.content,
            model: raw.model,
            input_tokens: raw.prompt_eval_count.unwrap_or(0),
            output_tokens: raw.eval_count.unwrap_or(0),
            stop_reason: if raw.done {
                Some("end_turn".into())
            } else {
                None
            },
            tool_calls: Vec::new(),
        })
    }
}

#[async_trait]
impl InferenceBackend for OllamaBackend {
    fn backend_id(&self) -> &'static str {
        "ollama"
    }

    fn capabilities(&self) -> BackendCapabilities {
        let base = BackendCapabilities::TEXT_GENERATION;
        if self.vision_capable {
            base.union(BackendCapabilities::VISION_LANGUAGE)
        } else {
            base
        }
    }

    fn supports_streaming(&self) -> bool {
        true
    }

    fn estimate_cost(&self, _request: &InferenceRequest) -> CostEstimate {
        // Local inference — no monetary cost.
        CostEstimate {
            typical_latency: Duration::from_secs(5),
            max_latency: Duration::from_secs(60),
            typical_usd_cents: 0.0,
            max_usd_cents: 0.0,
        }
    }

    #[instrument(skip(self, request, progress, cancel), fields(backend = "ollama"))]
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
                        backend: Some("ollama".into()),
                    })
                    .await;
                let resp = self.call_chat(req, &cancel).await?;
                Ok(InferenceResponse::Text(resp))
            }
            InferenceRequest::ImageGeneration(_)
            | InferenceRequest::ImageEdit(_)
            | InferenceRequest::ImageInpaint(_)
            | InferenceRequest::FrameInterpolation(_)
            | InferenceRequest::ImageToVideo(_)
            | InferenceRequest::BackgroundRemoval(_)
            | InferenceRequest::Replicate(_)
            | InferenceRequest::ComfyUi(_) => {
                warn!("Ollama does not support this request type");
                Err(BackendError::UnsupportedCapability)
            }
        }
    }
}

// ── Helpers ────────────────────────────────────────────────────────────────

#[allow(clippy::disallowed_methods)]
fn build_ollama_messages(req: &TextGenRequest) -> Vec<serde_json::Value> {
    let mut messages: Vec<serde_json::Value> = Vec::new();

    // Merge system content.
    let system_text: String = std::iter::once(req.system.as_deref().unwrap_or(""))
        .chain(
            req.messages
                .iter()
                .filter(|m| matches!(m.role, super::ChatRole::System))
                .flat_map(|m| &m.content)
                .filter_map(|p| {
                    if let ContentPart::Text { text } = p {
                        Some(text.as_str())
                    } else {
                        None
                    }
                }),
        )
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");

    if !system_text.is_empty() {
        messages.push(serde_json::json!({
            "role": "system",
            "content": system_text,
        }));
    }

    for msg in &req.messages {
        if matches!(msg.role, super::ChatRole::System) {
            continue;
        }

        let role = match msg.role {
            super::ChatRole::User => "user",
            super::ChatRole::Assistant => "assistant",
            super::ChatRole::System => continue,
        };

        // Ollama accepts images as a list of base64 strings alongside text.
        let text: String = msg
            .content
            .iter()
            .filter_map(|p| {
                if let ContentPart::Text { text } = p {
                    Some(text.as_str())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("\n");

        let images: Vec<String> = msg
            .content
            .iter()
            .filter_map(|p| {
                if let ContentPart::Image { bytes, .. } = p {
                    Some(base64::engine::general_purpose::STANDARD.encode(bytes))
                } else {
                    None
                }
            })
            .collect();

        let mut msg_json = serde_json::json!({
            "role": role,
            "content": text,
        });
        if !images.is_empty() {
            msg_json["images"] = serde_json::json!(images);
        }
        messages.push(msg_json);
    }

    messages
}

// ── Wire types ─────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct OllamaChatResponse {
    model: String,
    message: OllamaMessage,
    done: bool,
    prompt_eval_count: Option<u32>,
    eval_count: Option<u32>,
}

#[derive(Deserialize)]
struct OllamaMessage {
    content: String,
}

// Suppress unused import warnings.
fn _assert_image_gen(_: &ImageGenRequest) {}
fn _assert_image_edit(_: &ImageEditRequest) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_only_capabilities() {
        let b = OllamaBackend::new(false);
        let caps = b.capabilities();
        assert!(caps.contains(BackendCapabilities::TEXT_GENERATION));
        assert!(!caps.contains(BackendCapabilities::VISION_LANGUAGE));
    }

    #[test]
    fn vision_capable_adds_vision_flag() {
        let b = OllamaBackend::new(true);
        let caps = b.capabilities();
        assert!(caps.contains(BackendCapabilities::VISION_LANGUAGE));
    }

    #[test]
    fn estimate_cost_is_free() {
        let b = OllamaBackend::new(false);
        let req = InferenceRequest::Text(TextGenRequest::user("hello"));
        let est = b.estimate_cost(&req);
        assert_eq!(est.typical_usd_cents, 0.0);
        assert_eq!(est.max_usd_cents, 0.0);
    }

    #[test]
    fn build_ollama_messages_merges_system() {
        let mut req = TextGenRequest::user("hello");
        req.system = Some("be concise".into());
        let msgs = build_ollama_messages(&req);
        assert_eq!(msgs[0]["role"], "system");
        assert!(msgs[0]["content"].as_str().unwrap().contains("be concise"));
    }

    #[test]
    fn build_ollama_messages_attaches_images() {
        use crate::backends::{ChatMessage, ChatRole, ContentPart, ImageMediaType};
        let req = TextGenRequest {
            model: None,
            system: None,
            messages: vec![ChatMessage {
                role: ChatRole::User,
                content: vec![
                    ContentPart::text("describe this"),
                    ContentPart::Image {
                        bytes: vec![1, 2, 3],
                        media_type: ImageMediaType::Png,
                    },
                ],
            }],
            max_tokens: None,
            temperature: None,
            tools: Vec::new(),
            stop: Vec::new(),
        };
        let msgs = build_ollama_messages(&req);
        // No system message; only the user message.
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0]["role"], "user");
        let images = msgs[0]["images"].as_array().unwrap();
        assert_eq!(images.len(), 1);
        // Verify the image is valid base64.
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(images[0].as_str().unwrap())
            .unwrap();
        assert_eq!(decoded, vec![1, 2, 3]);
    }

    #[tokio::test]
    async fn integration_text_generation() {
        // This test requires a running local Ollama server.
        let base_url =
            std::env::var("PIXHAUS_OLLAMA_URL").unwrap_or_else(|_| "http://localhost:11434".into());
        let model = std::env::var("PIXHAUS_OLLAMA_MODEL").unwrap_or_else(|_| "llama3.2".into());

        if std::env::var("PIXHAUS_OLLAMA_RUN_INTEGRATION").is_err() {
            eprintln!("skipping: PIXHAUS_OLLAMA_RUN_INTEGRATION not set");
            return;
        }

        let backend = OllamaBackend::new(false)
            .with_base_url(base_url)
            .with_model(model);
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
