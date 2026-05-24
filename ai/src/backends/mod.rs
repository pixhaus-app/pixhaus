//! Inference backend adapters — S22.
//!
//! An [`InferenceBackend`] wraps one inference provider (Anthropic,
//! `OpenAI`, Replicate, Ollama, `ComfyUI`, Stability) and exposes a single
//! [`InferenceBackend::invoke`] method that accepts an
//! [`InferenceRequest`] and returns an [`InferenceResponse`]. Verbs
//! (S23–S36) receive an [`Arc<BackendRegistry>`] at construction time
//! and call into it to reach the configured backend.
//!
//! # Capability matching
//!
//! Each backend declares a [`BackendCapabilities`] bitfield via
//! [`InferenceBackend::capabilities`]. The [`BackendRegistry`] exposes
//! [`BackendRegistry::find_for`] so verbs can ask "give me *any* backend
//! that can do `IMAGE_GENERATION | VISION_LANGUAGE`" without caring which
//! provider satisfies the request.
//!
//! The verb runtime (S21) checks the union of all configured backends
//! against a verb's `required_capabilities` at dispatch time. If no
//! backend satisfies the union, the invocation fails with
//! [`crate::plugin::error::VerbError::UnsupportedCapability`] before the
//! verb's `invoke` is ever called.
//!
//! # API key management
//!
//! Cloud backends retrieve their API keys from the OS keychain via
//! [`keys::ApiKeyStore`] in production. The configuration UI (S13)
//! writes keys into the keychain; the adapter reads them on first use.
//! Integration tests bypass the keychain and read from environment
//! variables (e.g. `OPENAI_API_KEY`) so CI can exercise the adapters
//! without touching the developer's system keyring.

pub mod anthropic;
pub mod bridge;
pub mod comfyui;
pub mod error;
pub mod fal;
pub mod google;
pub mod keys;
pub mod ollama;
pub mod openai;
pub mod replicate;
pub mod stability;

pub use bridge::BackendProxy;
pub use error::{BackendError, Result};
pub use keys::ApiKeyStore;

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use crate::plugin::descriptor::{BackendCapabilities, CostEstimate};
use crate::plugin::progress::VerbProgress;

// ── Request types ──────────────────────────────────────────────────────────

/// Role of a message in a chat thread.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChatRole {
    /// A message framing the model's behaviour (system prompt).
    System,
    /// A turn from the human caller.
    User,
    /// A turn from the model.
    Assistant,
}

/// MIME type of an inline image in a message.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImageMediaType {
    /// `image/png`
    Png,
    /// `image/jpeg`
    Jpeg,
    /// `image/gif`
    Gif,
    /// `image/webp`
    Webp,
}

impl ImageMediaType {
    /// Returns the MIME type string.
    #[must_use]
    pub fn as_mime(&self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
            Self::Gif => "image/gif",
            Self::Webp => "image/webp",
        }
    }
}

/// One part of a chat message — either plain text or an embedded image.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentPart {
    /// Plain UTF-8 text.
    Text {
        /// The text content.
        text: String,
    },
    /// An inline image sent as raw bytes.
    Image {
        /// Raw image bytes (PNG, JPEG, etc.).
        bytes: Vec<u8>,
        /// MIME type the bytes represent.
        media_type: ImageMediaType,
    },
}

impl ContentPart {
    /// Constructs a text part.
    #[must_use]
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text { text: text.into() }
    }

    /// Constructs a PNG image part.
    #[must_use]
    pub fn png(bytes: Vec<u8>) -> Self {
        Self::Image {
            bytes,
            media_type: ImageMediaType::Png,
        }
    }
}

/// A single turn in a chat conversation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ChatMessage {
    /// Who sent this message.
    pub role: ChatRole,
    /// Ordered content parts; backends that do not support multi-modal
    /// messages will concatenate text parts and error on image parts.
    pub content: Vec<ContentPart>,
}

impl ChatMessage {
    /// Constructs a user message with a single text part.
    #[must_use]
    pub fn user(text: impl Into<String>) -> Self {
        Self {
            role: ChatRole::User,
            content: vec![ContentPart::text(text)],
        }
    }

    /// Constructs an assistant message with a single text part.
    #[must_use]
    pub fn assistant(text: impl Into<String>) -> Self {
        Self {
            role: ChatRole::Assistant,
            content: vec![ContentPart::text(text)],
        }
    }
}

/// JSON-Schema-described tool the model may call.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ToolDef {
    /// Stable, unique tool name (`snake_case`).
    pub name: String,
    /// One-sentence description the model uses to decide when to call this.
    pub description: String,
    /// JSON Schema for the tool's `input` object.
    pub input_schema: serde_json::Value,
}

/// Text generation request (covers `TEXT_GENERATION` and `VISION_LANGUAGE`).
///
/// Pass images in message `content` parts to exercise vision capability;
/// backends that lack `VISION_LANGUAGE` will return
/// [`BackendError::UnsupportedCapability`] if they encounter an image part.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TextGenRequest {
    /// Override the backend's default model (e.g. `"claude-opus-4-7"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Optional system prompt. Backends that have no concept of a system
    /// prompt will prepend this as the first user message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    /// Conversation history in order. Must contain at least one message.
    pub messages: Vec<ChatMessage>,
    /// Maximum tokens to generate. `None` uses the backend's default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    /// Sampling temperature in `[0.0, 2.0]`. `None` uses the backend's
    /// default (typically 1.0 for Claude, 1.0 for GPT).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    /// Tools the model may call.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<ToolDef>,
    /// Stop sequences. Generation halts when any of these strings is produced.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stop: Vec<String>,
}

impl TextGenRequest {
    /// Constructs a minimal request with a single user message.
    #[must_use]
    pub fn user(text: impl Into<String>) -> Self {
        Self {
            model: None,
            system: None,
            messages: vec![ChatMessage::user(text)],
            max_tokens: None,
            temperature: None,
            tools: Vec::new(),
            stop: Vec::new(),
        }
    }
}

/// Image generation request (`IMAGE_GENERATION` capability).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImageGenRequest {
    /// Override the backend's default model.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Positive prompt describing the desired image.
    pub prompt: String,
    /// Negative prompt describing content to avoid.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub negative_prompt: Option<String>,
    /// Output width in pixels.
    pub width: u32,
    /// Output height in pixels.
    pub height: u32,
    /// Number of diffusion steps. `None` uses the backend's default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub steps: Option<u32>,
    /// RNG seed for reproducibility. `None` uses a random seed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<u64>,
    /// Number of images to generate.
    pub num_images: u32,
    /// Optional quality hint. Backends that do not expose a quality
    /// parameter ignore this field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quality: Option<ImageQuality>,
    /// Reference image for style conditioning (raw PNG bytes).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style_image: Option<Vec<u8>>,
    /// Ordered reference images for multi-reference generation.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reference_images: Vec<Vec<u8>>,
}

/// Quality hint for image generation backends.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImageQuality {
    /// Let the provider choose a default.
    Auto,
    /// Lower-cost/lower-latency generation.
    Low,
    /// Balanced quality.
    Medium,
    /// Highest provider-supported quality.
    High,
}

impl ImageQuality {
    /// Returns the `OpenAI` wire value.
    #[must_use]
    pub fn as_openai(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }
}

/// Image editing request (`IMAGE_EDIT` / `IMAGE_INPAINT` capability).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImageEditRequest {
    /// Override the backend's default model.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Source image as raw PNG bytes.
    pub image: Vec<u8>,
    /// Edit mask as raw PNG bytes. White pixels are edited, black pixels
    /// are kept. Required for inpaint; optional for instruction-based edit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mask: Option<Vec<u8>>,
    /// Instruction describing the desired edit.
    pub prompt: String,
    /// Negative prompt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub negative_prompt: Option<String>,
    /// Number of images to return.
    pub num_images: u32,
    /// Optional style reference image (raw PNG). Used by `ImageEdit`
    /// verbs that participate in the anchor mechanic to inherit a sprite
    /// entity's canonical reference sheet as a style condition.
    /// Backends without a corresponding API parameter destructure-and-
    /// ignore this field — same posture they take toward
    /// `ImageGenRequest::style_image` today.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style_image: Option<Vec<u8>>,
    /// Ordered additional references for image edits.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reference_images: Vec<Vec<u8>>,
}

/// Frame interpolation request (`FRAME_INTERPOLATION` capability).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FrameInterpolationRequest {
    /// Override the backend's default model.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Input frames as raw PNG bytes, in display order.
    pub frames: Vec<Vec<u8>>,
    /// How many frames to generate between each adjacent pair of inputs.
    pub num_outputs: u32,
}

/// Image-to-video request (`IMAGE_TO_VIDEO` capability).
///
/// Animates a single still into a short clip. Used by the walk-cycle path of
/// the animated-sprite-sheet verb: a direction-locked clip is generated, then
/// frame-picked into a clean loop. The prompt bakes in the methodology's hard
/// negatives (no pivots, no background, no particles) at the call site.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImageToVideoRequest {
    /// Override the backend's default i2v model.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Source still as raw PNG bytes — the first frame the motion derives from.
    pub image: Vec<u8>,
    /// Positive prompt describing the desired motion.
    pub prompt: String,
    /// Negative prompt: pivots, quarter-turns, backgrounds, particles, glow.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub negative_prompt: Option<String>,
    /// Target clip length in frames. Backends that only take a duration
    /// convert via [`Self::fps`].
    pub num_frames: u32,
    /// Target frames per second of the generated clip.
    pub fps: u32,
    /// RNG seed for reproducibility. `None` uses a random seed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<u64>,
}

/// Raw Replicate prediction request for model-marketplace access.
///
/// Used by verbs that need a specific Replicate model not covered by a
/// built-in capability type.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReplicateRequest {
    /// Fully qualified model name, e.g. `"stability-ai/stable-diffusion-3"`.
    pub model: String,
    /// Specific version hash. `None` uses the model's latest deployment.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Model-specific input payload.
    pub input: serde_json::Value,
}

/// Raw `ComfyUI` workflow request.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ComfyUiRequest {
    /// Full `ComfyUI` workflow graph as produced by the `ComfyUI` web UI.
    pub workflow: serde_json::Value,
    /// Maximum time to wait for the job to complete.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout: Option<Duration>,
}

/// Dispatched to an [`InferenceBackend`].
///
/// Each variant maps to one or more [`BackendCapabilities`] bits. The
/// backend's `invoke` implementation returns
/// [`BackendError::UnsupportedCapability`] if a variant does not match its
/// declared capabilities.
#[derive(Clone, Debug)]
pub enum InferenceRequest {
    /// Plain text generation or vision-language query.
    Text(TextGenRequest),
    /// Render a new image from a text prompt.
    ImageGeneration(ImageGenRequest),
    /// Edit an existing image with an instruction and optional mask.
    ImageEdit(ImageEditRequest),
    /// Inpaint a masked region of an existing image.
    ImageInpaint(ImageEditRequest),
    /// Generate intermediate frames between key frames.
    FrameInterpolation(FrameInterpolationRequest),
    /// Animate a still image into a short video clip.
    ImageToVideo(ImageToVideoRequest),
    /// Raw Replicate API prediction (any registered model).
    Replicate(ReplicateRequest),
    /// Raw `ComfyUI` workflow execution.
    ComfyUi(ComfyUiRequest),
}

// ── Response types ─────────────────────────────────────────────────────────

/// One function call the model wants to make.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolCall {
    /// Opaque call identifier returned by the backend.
    pub id: String,
    /// Name of the tool to call.
    pub name: String,
    /// Arguments the model passed.
    pub input: serde_json::Value,
}

/// Response to a [`TextGenRequest`].
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TextGenResponse {
    /// Generated text. Empty when the model chose to call tools instead.
    pub content: String,
    /// Model identifier that produced this response.
    pub model: String,
    /// Input tokens consumed.
    pub input_tokens: u32,
    /// Output tokens generated.
    pub output_tokens: u32,
    /// Why generation stopped (e.g. `"end_turn"`, `"max_tokens"`,
    /// `"stop_sequence"`, `"tool_use"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,
    /// Tool calls the model wants to make. Non-empty only when
    /// `stop_reason == "tool_use"`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolCall>,
}

/// Response carrying one or more generated or edited images.
#[derive(Clone, Debug)]
pub struct ImageGenResponse {
    /// Generated images as raw PNG bytes, one per requested image.
    pub images: Vec<Vec<u8>>,
    /// Model identifier that produced this response.
    pub model: String,
}

/// Response carrying interpolated frames.
#[derive(Clone, Debug)]
pub struct FrameInterpolationResponse {
    /// Generated frames as raw PNG bytes, in display order.
    pub frames: Vec<Vec<u8>>,
    /// Model identifier that produced this response.
    pub model: String,
}

/// Response carrying a generated video clip.
#[derive(Clone, Debug)]
pub struct ImageToVideoResponse {
    /// The clip as raw container bytes (typically `MP4` / `WebM`).
    pub clip: Vec<u8>,
    /// MIME type of the clip (e.g. `video/mp4`). Drives the decode path.
    pub mime: String,
    /// Model identifier that produced this response.
    pub model: String,
}

/// Returned by [`InferenceBackend::invoke`].
pub enum InferenceResponse {
    /// Text or tool-call response.
    Text(TextGenResponse),
    /// Image or image-edit response.
    Image(ImageGenResponse),
    /// Frame interpolation response.
    Frames(FrameInterpolationResponse),
    /// Image-to-video clip response.
    Video(ImageToVideoResponse),
    /// Raw JSON value used by Replicate when the prediction output
    /// doesn't fit a typed variant. `ComfyUI` returns
    /// [`InferenceResponse::Image`] for its raw workflow execution; the
    /// only current producer of this variant is the Replicate adapter's
    /// [`InferenceRequest::Replicate`] arm.
    Raw(serde_json::Value),
}

// ── InferenceBackend trait ─────────────────────────────────────────────────

/// The trait every inference backend adapter implements.
///
/// Backends are stored as `Arc<dyn InferenceBackend>` in the
/// [`BackendRegistry`]; the `Send + Sync + 'static` bounds allow sharing
/// across async tasks and across the IPC command boundary.
///
/// # Cost estimation
///
/// [`InferenceBackend::estimate_cost`] returns a *pre-invocation* guess.
/// The runtime surfaces this in the UI before the user clicks "Run". The
/// actual cost is reported via the [`VerbProgress`] channel during and
/// after execution. Estimates do not need to be exact — within 2× is
/// acceptable.
#[async_trait]
pub trait InferenceBackend: Send + Sync + 'static {
    /// Stable, lower-case backend identifier.
    ///
    /// Used as the keychain service suffix and as the `backend` field in
    /// [`crate::plugin::output::ActualCost`]. Must be unique within a
    /// [`BackendRegistry`].
    fn backend_id(&self) -> &'static str;

    /// Capabilities this backend exposes.
    fn capabilities(&self) -> BackendCapabilities;

    /// Whether this backend can stream text tokens progressively.
    fn supports_streaming(&self) -> bool;

    /// Rough cost estimate for the given request, computed without making
    /// any network calls.
    fn estimate_cost(&self, request: &InferenceRequest) -> CostEstimate;

    /// Execute an inference request.
    ///
    /// Implementations must observe `cancel` between expensive operations
    /// (streaming loops, polling loops) and return
    /// [`BackendError::Cancelled`] promptly when it fires.
    ///
    /// Progress (cost updates, step messages) is emitted via `progress`.
    async fn invoke(
        &self,
        request: InferenceRequest,
        progress: VerbProgress,
        cancel: CancellationToken,
    ) -> Result<InferenceResponse>;
}

// ── BackendRegistry ────────────────────────────────────────────────────────

/// Registry of configured [`InferenceBackend`]s.
///
/// Verbs receive an `Arc<BackendRegistry>` at construction time and use
/// [`Self::find_for`] to retrieve a backend that satisfies their
/// capability requirements. The registry is immutable after construction
/// — if the user reconfigures backends, the app builds a new registry and
/// re-constructs verbs.
///
/// The registry is also consumed by the verb runtime (S21) to perform
/// capability pre-flight checks at dispatch time without invoking the verb
/// itself.
#[derive(Default)]
pub struct BackendRegistry {
    backends: Vec<Arc<dyn InferenceBackend>>,
}

impl BackendRegistry {
    /// Creates an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a backend to the registry.
    pub fn add(&mut self, backend: impl InferenceBackend) {
        self.backends.push(Arc::new(backend));
    }

    /// Adds an already-boxed backend. Useful when the caller already holds
    /// an `Arc<dyn InferenceBackend>`.
    pub fn add_arc(&mut self, backend: Arc<dyn InferenceBackend>) {
        self.backends.push(backend);
    }

    /// Returns the number of registered backends.
    #[must_use]
    pub fn len(&self) -> usize {
        self.backends.len()
    }

    /// Returns `true` if no backends are registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.backends.is_empty()
    }

    /// Returns the union of capabilities across all registered backends.
    #[must_use]
    pub fn all_capabilities(&self) -> BackendCapabilities {
        self.backends
            .iter()
            .fold(BackendCapabilities::empty(), |acc, b| {
                BackendCapabilities(acc.0 | b.capabilities().0)
            })
    }

    /// Returns `true` if *any* registered backend satisfies all of `required`.
    #[must_use]
    pub fn can_satisfy(&self, required: BackendCapabilities) -> bool {
        if required.is_empty() {
            return true;
        }
        self.backends
            .iter()
            .any(|b| b.capabilities().contains(required))
    }

    /// Returns the first backend whose capabilities contain all of `required`.
    ///
    /// Returns `None` if no backend satisfies the requirements.
    #[must_use]
    pub fn find_for(&self, required: BackendCapabilities) -> Option<&dyn InferenceBackend> {
        if required.is_empty() {
            return self.backends.first().map(std::convert::AsRef::as_ref);
        }
        self.backends
            .iter()
            .find(|b| b.capabilities().contains(required))
            .map(std::convert::AsRef::as_ref)
    }

    /// Returns all backends that satisfy `required`.
    #[must_use]
    pub fn find_all_for(&self, required: BackendCapabilities) -> Vec<&dyn InferenceBackend> {
        self.backends
            .iter()
            .filter(|b| b.capabilities().contains(required))
            .map(std::convert::AsRef::as_ref)
            .collect()
    }

    /// Returns all registered backends, in registration order.
    #[must_use]
    pub fn all(&self) -> Vec<&dyn InferenceBackend> {
        self.backends
            .iter()
            .map(std::convert::AsRef::as_ref)
            .collect()
    }
}

// ── Helpers shared across adapters ─────────────────────────────────────────

/// Checks an HTTP response status and converts errors to [`BackendError`].
///
/// On non-2xx, reads the response body and returns [`BackendError::Http`].
/// On 401/403, wraps in [`BackendError::Auth`] instead for clearer
/// error messages. On 429, returns [`BackendError::RateLimited`] with the
/// `Retry-After` header value if present.
pub(crate) async fn check_http_status(
    response: reqwest::Response,
) -> std::result::Result<reqwest::Response, BackendError> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }

    let status_u16 = status.as_u16();

    // Extract Retry-After before consuming the response.
    let retry_after = if status_u16 == 429 {
        response
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok())
    } else {
        None
    };

    let body = response.text().await.unwrap_or_default();

    if status_u16 == 429 {
        return Err(BackendError::RateLimited {
            retry_after_secs: retry_after.unwrap_or(60),
        });
    }

    if status_u16 == 401 || status_u16 == 403 {
        return Err(BackendError::Auth(body));
    }

    Err(BackendError::http(status_u16, body))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::descriptor::BackendCapabilities;
    use std::time::Duration;

    struct FakeBackend {
        id: &'static str,
        caps: BackendCapabilities,
    }

    #[async_trait]
    impl InferenceBackend for FakeBackend {
        fn backend_id(&self) -> &'static str {
            self.id
        }
        fn capabilities(&self) -> BackendCapabilities {
            self.caps
        }
        fn supports_streaming(&self) -> bool {
            false
        }
        fn estimate_cost(&self, _req: &InferenceRequest) -> CostEstimate {
            CostEstimate::free()
        }
        async fn invoke(
            &self,
            _req: InferenceRequest,
            _progress: VerbProgress,
            _cancel: CancellationToken,
        ) -> Result<InferenceResponse> {
            Err(BackendError::UnsupportedCapability)
        }
    }

    fn text_backend() -> FakeBackend {
        FakeBackend {
            id: "text",
            caps: BackendCapabilities::TEXT_GENERATION.union(BackendCapabilities::VISION_LANGUAGE),
        }
    }

    fn image_backend() -> FakeBackend {
        FakeBackend {
            id: "image",
            caps: BackendCapabilities::IMAGE_GENERATION.union(BackendCapabilities::IMAGE_EDIT),
        }
    }

    #[test]
    fn registry_all_capabilities_is_union() {
        let mut reg = BackendRegistry::new();
        reg.add(text_backend());
        reg.add(image_backend());

        let all = reg.all_capabilities();
        assert!(all.contains(BackendCapabilities::TEXT_GENERATION));
        assert!(all.contains(BackendCapabilities::VISION_LANGUAGE));
        assert!(all.contains(BackendCapabilities::IMAGE_GENERATION));
        assert!(all.contains(BackendCapabilities::IMAGE_EDIT));
        assert!(!all.contains(BackendCapabilities::AUDIO_ANALYSIS));
    }

    #[test]
    fn registry_find_for_returns_matching_backend() {
        let mut reg = BackendRegistry::new();
        reg.add(text_backend());
        reg.add(image_backend());

        let found = reg.find_for(BackendCapabilities::IMAGE_GENERATION);
        assert!(found.is_some());
        assert_eq!(found.unwrap().backend_id(), "image");

        let text = reg.find_for(BackendCapabilities::TEXT_GENERATION);
        assert_eq!(text.unwrap().backend_id(), "text");
    }

    #[test]
    fn registry_find_for_returns_none_when_no_match() {
        let mut reg = BackendRegistry::new();
        reg.add(text_backend());

        let found = reg.find_for(BackendCapabilities::AUDIO_ANALYSIS);
        assert!(found.is_none());
    }

    #[test]
    fn registry_can_satisfy_empty_is_always_true() {
        let reg = BackendRegistry::new();
        assert!(reg.can_satisfy(BackendCapabilities::empty()));
    }

    #[test]
    fn registry_can_satisfy_checks_union() {
        let mut reg = BackendRegistry::new();
        reg.add(text_backend());

        assert!(reg.can_satisfy(BackendCapabilities::TEXT_GENERATION));
        assert!(!reg.can_satisfy(BackendCapabilities::IMAGE_GENERATION));
    }

    #[test]
    fn content_part_constructors() {
        let t = ContentPart::text("hello");
        assert!(matches!(t, ContentPart::Text { .. }));

        let i = ContentPart::png(vec![1, 2, 3]);
        assert!(matches!(
            i,
            ContentPart::Image {
                media_type: ImageMediaType::Png,
                ..
            }
        ));
    }

    #[test]
    fn chat_message_user_role() {
        let msg = ChatMessage::user("hello");
        assert!(matches!(msg.role, ChatRole::User));
        assert_eq!(msg.content.len(), 1);
    }

    #[test]
    fn image_media_type_mime_strings() {
        assert_eq!(ImageMediaType::Png.as_mime(), "image/png");
        assert_eq!(ImageMediaType::Jpeg.as_mime(), "image/jpeg");
        assert_eq!(ImageMediaType::Gif.as_mime(), "image/gif");
        assert_eq!(ImageMediaType::Webp.as_mime(), "image/webp");
    }

    #[test]
    fn text_gen_request_user_constructor() {
        let req = TextGenRequest::user("write a haiku");
        assert_eq!(req.messages.len(), 1);
        assert!(req.tools.is_empty());
        assert!(req.model.is_none());
    }

    #[test]
    fn registry_len_and_empty() {
        let mut reg = BackendRegistry::new();
        assert!(reg.is_empty());
        reg.add(text_backend());
        assert_eq!(reg.len(), 1);
        assert!(!reg.is_empty());
    }

    #[test]
    fn registry_find_all_for_multi_match() {
        let mut reg = BackendRegistry::new();
        let b1 = FakeBackend {
            id: "b1",
            caps: BackendCapabilities::TEXT_GENERATION,
        };
        let b2 = FakeBackend {
            id: "b2",
            caps: BackendCapabilities::TEXT_GENERATION,
        };
        reg.add(b1);
        reg.add(b2);

        let all = reg.find_all_for(BackendCapabilities::TEXT_GENERATION);
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn cost_estimate_free_is_zero() {
        let c = CostEstimate::free();
        assert_eq!(c.typical_usd_cents, 0.0);
        assert_eq!(c.typical_latency, Duration::ZERO);
    }
}
