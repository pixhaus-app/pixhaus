//! `OpenAI` image backend adapter.
//!
//! Scoped to image generation and editing over `/v1/images/generations`
//! and `/v1/images/edits`. Text, vision, and embedding paths are not
//! ported in this slice — the request types they need live elsewhere, and
//! advertising those capabilities would make this backend selectable for a
//! request it cannot satisfy. Implemented as a raw HTTP client for
//! consistency with the other adapters.
//!
//! # How the OpenAI image API works
//!
//! Two endpoints, and the split is the thing to remember:
//!
//! - `/v1/images/generations` is text-only: prompt in, image out. It has **no
//!   reference-image input at all**, and its `style` field is the unrelated
//!   dall-e-3 "vivid"/"natural" knob — not our `ImageGenRequest::style_image`.
//! - `/v1/images/edits` is the only endpoint that accepts input images: a
//!   multipart `image[]` array (up to 16 for gpt-image), an optional `mask`, and
//!   a prompt. It serves both literal edits and "generate conditioned on a
//!   reference".
//!
//! So this adapter routes by *content*, not by request type: an `ImageGenRequest`
//! carrying **any** `reference_images` is sent to `/images/edits` (the anchor
//! rides in `image[]`); a bare prompt goes to `/images/generations`. See
//! `generate_image`. That is why the studio's first-frame generation — which
//! passes the approved anchor as a reference — actually reaches the model even
//! though `/images/generations` has no reference parameter.
//!
//! Fields with no OpenAI wire equivalent are dropped: `style_image` (a
//! FAL/IP-adapter concept), `negative_prompt`, `seed`, and `steps`.
//!
//! ## gpt-image vs dall-e
//!
//! - gpt-image (`gpt-image-1`/`-2`/`-mini`, `chatgpt-image-latest`): returns
//!   base64 by default (no `response_format`), takes `output_format` (we send
//!   `png`), and supports streaming (`stream: true` + `partial_images: 0..=3`).
//! - dall-e (`dall-e-2`/`-3`): uses `response_format: b64_json`, no streaming.
//!
//! `is_gpt_image_model` is the switch; the differences are applied in
//! `build_image_generation_body` and the multipart edit/reference paths.
//!
//! ## Sizes
//!
//! gpt-image-1 accepts only `1024x1024`, `1536x1024`, `1024x1536`. gpt-image-2
//! also accepts arbitrary `WIDTHxHEIGHT` as long as both are divisible by 16, the
//! aspect is within 1:3..3:1, and it is <= 3840x2160. To stay safe across models
//! this adapter snaps every gpt-image request to the nearest of the three
//! standard sizes by aspect (`snap_gpt_image_size`), then downscales the result
//! back to the caller's exact width/height (`fit_images_to_request`) so the
//! `ImageGenRequest` width/height contract holds. dall-e sizes pass through
//! verbatim. (To honor true arbitrary gpt-image-2 sizes, relax the snap — it is
//! deliberately conservative today.)
//!
//! ## Streaming
//!
//! Only a single gpt-image request streams (`use_image_stream`): the SSE events
//! are `image_generation.partial_image` / `.completed` (and `image_edit.*` on the
//! edit path), parsed by `parse_openai_image_stream`. Multi-image (`n > 1`) and
//! dall-e stay on the buffered JSON path.
//!
//! # Models
//!
//! - Image generation: `gpt-image-2` (default); override per-request.
//! - Image edit/inpaint: `gpt-image-2`.
//!
//! # Pricing (May 2026 estimates)
//!
//! GPT image generation varies by output size and quality; the per-image
//! cents figure here is a placeholder the runtime surfaces before a run.

use std::time::Duration;

use async_trait::async_trait;
use base64::Engine as _;
use futures::StreamExt as _;
use serde::Deserialize;
use tokio::select;
use tokio_util::sync::CancellationToken;
use tracing::{debug, instrument, warn};

use super::{
    BackendError, ImageEditRequest, ImageGenRequest, ImageGenResponse, InferenceBackend, InferenceRequest, InferenceResponse, Result, VerbProgress,
    build_http_client, check_http_status,
};
use crate::plugin::context::PixelData;
use crate::plugin::descriptor::{BackendCapabilities, CostEstimate};
use crate::plugin::progress::{CostUpdate, VerbProgressEvent};

const BASE_URL: &str = "https://api.openai.com/v1";
/// Default `OpenAI` image model for Pixhaus image generation.
pub const DEFAULT_IMAGE_MODEL: &str = "gpt-image-2";
const DEFAULT_IMAGE_EDIT_MODEL: &str = "gpt-image-2";

/// Placeholder per-image estimate; provider pricing varies by model, size,
/// and quality.
const IMAGE_PRICE_CENTS: f32 = 8.0;

/// `OpenAI` image backend adapter.
pub struct OpenAiBackend {
    client: reqwest::Client,
    api_key: String,
    base_url: String,
    image_model: String,
    image_edit_model: String,
}

impl std::fmt::Debug for OpenAiBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenAiBackend")
            .field("base_url", &self.base_url)
            .field("image_model", &self.image_model)
            .field("image_edit_model", &self.image_edit_model)
            .field("api_key", &"[redacted]")
            .finish_non_exhaustive()
    }
}

impl OpenAiBackend {
    /// Constructs an adapter with an explicit API key.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError::Network`] if the HTTP client cannot be built
    /// (for instance, the TLS backend fails to initialize).
    pub fn new(api_key: impl Into<String>) -> Result<Self> {
        Ok(Self {
            // Image generation can run for minutes; the reference-sheet verb
            // advertises a 300s max latency. 120s aborted mid-gen.
            client: build_http_client(Duration::from_secs(300))?,
            api_key: api_key.into(),
            base_url: BASE_URL.into(),
            image_model: DEFAULT_IMAGE_MODEL.into(),
            image_edit_model: DEFAULT_IMAGE_EDIT_MODEL.into(),
        })
    }

    /// Constructs an adapter by reading the API key from the OS keychain.
    pub fn from_keychain() -> Result<Self> {
        let key = super::ApiKeyStore::get("openai")?;
        Self::new(key)
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

    /// Overrides the base URL for tests.
    #[must_use]
    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    /// Generates an image from `req`. Routes by content (see the module docs):
    /// with any `reference_images` the request goes to `/images/edits`
    /// (`generate_image_with_references`) because `/images/generations` has no
    /// reference-image input; a bare prompt uses `/images/generations`.
    #[allow(clippy::cast_precision_loss)]
    async fn generate_image(&self, req: &ImageGenRequest, progress: &VerbProgress, cancel: &CancellationToken) -> Result<ImageGenResponse> {
        let model = req.model.as_deref().unwrap_or(self.image_model.as_str()).to_owned();

        debug!(model = %model, "sending OpenAI image generation request");

        if !req.reference_images.is_empty() {
            return self.generate_image_with_references(req, &model, progress, cancel).await;
        }

        let body = build_image_generation_body(req, &model);
        let streaming = use_image_stream(&model, req.num_images);

        let mut builder = self
            .client
            .post(format!("{}/images/generations", self.base_url))
            .bearer_auth(&self.api_key)
            .json(&body);
        if streaming {
            // The client negotiates gzip/brotli; reqwest then tries to
            // decompress incrementally, and a chunked compressed
            // text/event-stream fails mid-body with "error decoding response
            // body". Opt this one request out of compression so bytes_stream
            // yields raw SSE frames.
            builder = builder.header(reqwest::header::ACCEPT_ENCODING, "identity");
        }
        let http_req = builder.build().map_err(BackendError::Network)?;

        let http_resp = select! {
            biased;
            () = cancel.cancelled() => return Err(BackendError::Cancelled),
            res = self.client.execute(http_req) => res.map_err(BackendError::Network)?,
        };

        let http_resp = check_http_status(http_resp).await?;
        let images = if streaming {
            parse_openai_image_stream(http_resp, progress, cancel).await?
        } else {
            let raw: OpenAiImageResponse = http_resp.json().await.map_err(BackendError::Network)?;
            decode_image_data(raw.data)?
        };
        // The request size may have been snapped to a valid gpt-image size;
        // downscale back to what the caller asked for.
        let images = fit_images_to_request(images, req, &model)?;

        progress
            .send(VerbProgressEvent::Cost(CostUpdate {
                usd_cents: IMAGE_PRICE_CENTS * req.num_images as f32,
                tokens_input: None,
                tokens_output: None,
            }))
            .await;

        Ok(ImageGenResponse { images, model })
    }

    /// Generates an image conditioned on `req.reference_images` via the
    /// `/images/edits` endpoint — the only OpenAI image endpoint that accepts
    /// input images. There is no reference parameter on `/images/generations`, so
    /// the references are sent as multipart `image[]` parts (gpt-image) / `image`
    /// (dall-e). `style_image` is intentionally not sent (no OpenAI equivalent).
    #[allow(clippy::cast_precision_loss)]
    async fn generate_image_with_references(
        &self,
        req: &ImageGenRequest,
        model: &str,
        progress: &VerbProgress,
        cancel: &CancellationToken,
    ) -> Result<ImageGenResponse> {
        let image_field = if is_gpt_image_model(model) { "image[]" } else { "image" };
        let mut form = reqwest::multipart::Form::new()
            .text("prompt", req.prompt.clone())
            .text("n", req.num_images.to_string())
            .text("model", model.to_owned())
            .text("size", openai_size_param(req, model));
        if is_gpt_image_model(model) {
            form = form.text("output_format", "png");
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
        let raw: OpenAiImageResponse = http_resp.json().await.map_err(BackendError::Network)?;
        let images = decode_image_data(raw.data)?;
        let images = fit_images_to_request(images, req, model)?;
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

    /// Edits/inpaints `req.image` via `/images/edits`: the base image plus an
    /// optional `mask` (white = repaint) and any extra `reference_images`, all as
    /// multipart `image[]` parts. gpt-image returns base64 `png`; dall-e takes
    /// `response_format: b64_json`. `style_image` has no OpenAI equivalent.
    #[allow(clippy::cast_precision_loss)]
    async fn edit_image(&self, req: &ImageEditRequest, progress: &VerbProgress, cancel: &CancellationToken) -> Result<ImageGenResponse> {
        let model = req.model.as_deref().unwrap_or(self.image_edit_model.as_str()).to_owned();

        debug!(model = %model, "sending OpenAI image edit request");

        // Image edits use multipart/form-data. GPT image models return
        // base64 by default and do not accept the legacy `response_format`
        // field.
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
        // Image-only slice. TEXT_GENERATION, VISION_LANGUAGE, TOOL_USE, and
        // EMBEDDINGS are intentionally not advertised: their request paths
        // are not ported here, so claiming the capability would make this
        // backend selectable for a request it cannot satisfy.
        BackendCapabilities::IMAGE_GENERATION
            .union(BackendCapabilities::IMAGE_EDIT)
            .union(BackendCapabilities::IMAGE_INPAINT)
    }

    fn supports_streaming(&self) -> bool {
        false
    }

    #[allow(clippy::cast_precision_loss)]
    fn estimate_cost(&self, request: &InferenceRequest) -> CostEstimate {
        match request {
            InferenceRequest::ImageGeneration(req) => CostEstimate {
                typical_latency: Duration::from_secs(8),
                max_latency: Duration::from_secs(30),
                typical_usd_cents: IMAGE_PRICE_CENTS * req.num_images as f32,
                max_usd_cents: IMAGE_PRICE_CENTS * req.num_images as f32 * 2.0,
            },
            InferenceRequest::ImageEdit(req) | InferenceRequest::ImageInpaint(req) => CostEstimate {
                typical_latency: Duration::from_secs(8),
                max_latency: Duration::from_secs(30),
                typical_usd_cents: IMAGE_PRICE_CENTS * req.num_images as f32,
                max_usd_cents: IMAGE_PRICE_CENTS * req.num_images as f32 * 2.0,
            },
            _ => CostEstimate::free(),
        }
    }

    #[instrument(skip(self, request, progress, cancel), fields(backend = "openai"))]
    async fn invoke(&self, request: InferenceRequest, progress: VerbProgress, cancel: CancellationToken) -> Result<InferenceResponse> {
        match request {
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
            InferenceRequest::Text(_)
            | InferenceRequest::FrameInterpolation(_)
            | InferenceRequest::ImageToVideo(_)
            | InferenceRequest::BackgroundRemoval(_)
            | InferenceRequest::Replicate(_)
            | InferenceRequest::ComfyUi(_) => {
                warn!("OpenAI image backend does not support this request type");
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
        <Self as InferenceBackend>::capabilities(self)
    }

    fn cost_estimate(&self, _required: BackendCapabilities) -> CostEstimate {
        CostEstimate {
            typical_latency: Duration::from_secs(8),
            max_latency: Duration::from_secs(30),
            typical_usd_cents: IMAGE_PRICE_CENTS,
            max_usd_cents: IMAGE_PRICE_CENTS * 2.0,
        }
    }

    fn is_available(&self) -> bool {
        !self.api_key.is_empty()
    }
}

// ── Helpers ────────────────────────────────────────────────────────────────

/// Maps a requested image size to the nearest size `gpt-image-*` actually
/// accepts (`1024x1024`, `1024x1536`, `1536x1024`), chosen by aspect ratio.
///
/// `OpenAI` rejects any other size (e.g. the animation verb's `256x256`) with
/// an error event. Callers snap for the API call, then downscale the returned
/// image back to the requested dimensions via [`resize_png_to`] so the
/// `ImageGenRequest` width/height contract still holds.
#[must_use]
fn snap_gpt_image_size(width: u32, height: u32) -> (u32, u32) {
    if height == 0 {
        return (1024, 1024);
    }
    #[allow(clippy::cast_precision_loss)]
    let ratio = width as f32 / height as f32;
    if ratio > 1.2 {
        (1536, 1024)
    } else if ratio < 0.83 {
        (1024, 1536)
    } else {
        (1024, 1024)
    }
}

/// The `OpenAI` `size` string for a request: snapped to a valid gpt-image size
/// for gpt-image models, or the verbatim request size otherwise (`dall-e`
/// accepts a wider range and is left as-is).
fn openai_size_param(req: &ImageGenRequest, model: &str) -> String {
    if is_gpt_image_model(model) {
        let (w, h) = snap_gpt_image_size(req.width, req.height);
        format!("{w}x{h}")
    } else {
        format!("{}x{}", req.width, req.height)
    }
}

/// Resizes a PNG to `(width, height)` if it differs, returning re-encoded PNG
/// bytes. Used to downscale a snapped gpt-image result back to the requested
/// size. A no-op (re-encode aside) when the image already matches.
fn resize_png_to(bytes: &[u8], width: u32, height: u32) -> Result<Vec<u8>> {
    let img = image::load_from_memory(bytes).map_err(|e| BackendError::InvalidResponse(format!("image decode failed: {e}")))?;
    if img.width() == width && img.height() == height {
        return Ok(bytes.to_vec());
    }
    let resized = img.resize_exact(width, height, image::imageops::FilterType::Lanczos3);
    let mut out = Vec::new();
    resized
        .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
        .map_err(|e| BackendError::InvalidResponse(format!("image encode failed: {e}")))?;
    Ok(out)
}

/// Downscales each image back to the requested size when the API call used a
/// snapped gpt-image size. No-op for non-gpt models or when sizes already
/// match.
fn fit_images_to_request(images: Vec<Vec<u8>>, req: &ImageGenRequest, model: &str) -> Result<Vec<Vec<u8>>> {
    if !is_gpt_image_model(model) {
        return Ok(images);
    }
    let (snapped_w, snapped_h) = snap_gpt_image_size(req.width, req.height);
    if snapped_w == req.width && snapped_h == req.height {
        return Ok(images);
    }
    images.into_iter().map(|bytes| resize_png_to(&bytes, req.width, req.height)).collect()
}

/// Builds the JSON body for an `/images/generations` call.
///
/// `negative_prompt`, `seed`, and `steps` have no `OpenAI` image-generation
/// wire equivalent and are dropped (matching the original adapter's
/// behaviour). `reference_images` are handled out of band via the multipart
/// `/images/edits` path before this is ever called.
#[allow(clippy::disallowed_methods)]
fn build_image_generation_body(req: &ImageGenRequest, model: &str) -> serde_json::Value {
    let mut body = serde_json::json!({
        "model": model,
        "prompt": req.prompt,
        "n": req.num_images,
        "size": openai_size_param(req, model),
    });

    if is_gpt_image_model(model) {
        body["output_format"] = serde_json::json!("png");
        if use_image_stream(model, req.num_images) {
            body["stream"] = serde_json::json!(true);
            // 0 yields only the final image; 2 gives two progressively
            // sharper previews before it, which is plenty for a live canvas.
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
/// Limited to a single gpt-image request: multi-image streaming interleaves
/// `partial_image_index` across images, which the canvas preview has no way to
/// disentangle, and only gpt-image models support `stream: true` (dall-e does
/// not). The buffered JSON path stays the default for every other case.
fn use_image_stream(model: &str, num_images: u32) -> bool {
    is_gpt_image_model(model) && num_images == 1
}

fn decode_image_data(data: Vec<ImageData>) -> Result<Vec<Vec<u8>>> {
    data.into_iter()
        .map(|d| {
            if let Some(b64) = d.b64_json {
                base64::engine::general_purpose::STANDARD
                    .decode(b64.as_bytes())
                    .map_err(|e| BackendError::InvalidResponse(e.to_string()))
            } else {
                Err(BackendError::InvalidResponse("no b64_json in image response".into()))
            }
        })
        .collect()
}

/// Reads an `OpenAI` image generation server-sent event stream, emitting each
/// partial frame as [`VerbProgressEvent::PartialPixels`] and collecting the
/// completed image(s) for the final result.
///
/// # Errors
///
/// Returns [`BackendError::Network`] on a transport failure, parse errors from
/// [`handle_openai_sse_frame`], and [`BackendError::InvalidResponse`] if the
/// stream ends without a completed image.
async fn parse_openai_image_stream(response: reqwest::Response, progress: &VerbProgress, cancel: &CancellationToken) -> Result<Vec<Vec<u8>>> {
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
        return Err(BackendError::InvalidResponse("OpenAI image stream ended without a final image".into()));
    }
    Ok(final_images)
}

/// Parses one SSE frame: routes `*partial*` events to the progress channel as
/// live previews and `*completed*` events into `final_images`.
///
/// # Errors
///
/// Returns [`BackendError::InvalidResponse`] when the frame is not valid JSON
/// or carries a provider-side error event.
async fn handle_openai_sse_frame(frame: &str, progress: &VerbProgress, final_images: &mut Vec<Vec<u8>>) -> Result<()> {
    let data = frame
        .lines()
        .filter_map(|line| line.strip_prefix("data:"))
        .map(str::trim)
        .collect::<Vec<_>>()
        .join("\n");
    if data.is_empty() || data == "[DONE]" {
        return Ok(());
    }
    let value: serde_json::Value = serde_json::from_str(&data).map_err(|e| BackendError::InvalidResponse(e.to_string()))?;
    let event_type = value.get("type").and_then(serde_json::Value::as_str).unwrap_or_default();
    // Surface a provider-side error event instead of swallowing it (it carries
    // no `b64` key, so it would otherwise end as the generic "no final image").
    if event_type.contains("error") || value.get("error").is_some() {
        let msg = value
            .get("error")
            .and_then(|e| e.get("message"))
            .and_then(serde_json::Value::as_str)
            .or_else(|| value.get("message").and_then(serde_json::Value::as_str))
            .unwrap_or(event_type);
        return Err(BackendError::InvalidResponse(format!("OpenAI image error: {msg}")));
    }
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

/// Walks a JSON value collecting every base64-decodable `*b64*` string as image
/// bytes. `OpenAI` puts the image under `b64_json` at the top level for partial
/// events and inside `data[]` for completed events; this finds both.
fn collect_base64_images(value: &serde_json::Value, images: &mut Vec<Vec<u8>>) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, value) in map {
                if key.contains("b64")
                    && let Some(encoded) = value.as_str()
                    && let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(encoded.as_bytes())
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

/// Decodes PNG (or any image) bytes into tightly-packed RGBA8 [`PixelData`].
/// Returns `None` if the bytes don't decode, so a malformed partial frame is
/// skipped rather than aborting the stream.
fn image_bytes_to_pixel_data(bytes: &[u8]) -> Option<PixelData> {
    let image = image::load_from_memory(bytes).ok()?.to_rgba8();
    let (width, height) = image.dimensions();
    Some(PixelData::rgba8(width, height, image.into_raw()))
}

// ── Wire types ─────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct OpenAiImageResponse {
    data: Vec<ImageData>,
}

#[derive(Deserialize)]
struct ImageData {
    b64_json: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backends::ImageQuality;

    #[test]
    fn capabilities_are_image_only() {
        let b = OpenAiBackend::new("k").unwrap();
        let caps = <OpenAiBackend as InferenceBackend>::capabilities(&b);
        assert!(caps.contains(BackendCapabilities::IMAGE_GENERATION));
        assert!(caps.contains(BackendCapabilities::IMAGE_EDIT));
        assert!(caps.contains(BackendCapabilities::IMAGE_INPAINT));
        // Text/vision/embeddings paths are not ported in this slice.
        assert!(!caps.contains(BackendCapabilities::TEXT_GENERATION));
        assert!(!caps.contains(BackendCapabilities::VISION_LANGUAGE));
        assert!(!caps.contains(BackendCapabilities::EMBEDDINGS));
    }

    #[test]
    fn is_available_tracks_api_key() {
        use crate::plugin::backend::InferenceBackend as _;
        assert!(OpenAiBackend::new("k").unwrap().is_available());
        assert!(!OpenAiBackend::new("").unwrap().is_available());
    }

    #[test]
    fn estimate_cost_image_is_nonzero() {
        let b = OpenAiBackend::new("k").unwrap();
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
    fn gpt_image_body_streams_single_image_png() {
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
        // A single gpt-image request streams partial previews for a live canvas.
        assert_eq!(body["stream"], true);
        assert_eq!(body["partial_images"], 2);
        assert!(body.get("response_format").is_none());
        // Unsupported gpt-image fields are dropped, not forwarded.
        assert!(body.get("negative_prompt").is_none());
        assert!(body.get("seed").is_none());
    }

    #[test]
    fn gpt_image_body_multi_image_is_non_streaming() {
        let req = ImageGenRequest {
            model: None,
            prompt: "a reference sheet".into(),
            negative_prompt: None,
            width: 1024,
            height: 1024,
            steps: None,
            seed: None,
            num_images: 4,
            quality: None,
            style_image: None,
            reference_images: Vec::new(),
        };

        let body = build_image_generation_body(&req, "gpt-image-2");

        // Multi-image streaming interleaves partials across images; stay buffered.
        assert!(body.get("stream").is_none());
        assert!(body.get("partial_images").is_none());
        assert_eq!(body["output_format"], "png");
    }

    #[test]
    fn snap_gpt_image_size_picks_valid_dims_by_aspect() {
        assert_eq!(snap_gpt_image_size(256, 256), (1024, 1024));
        assert_eq!(snap_gpt_image_size(800, 1200), (1024, 1536));
        assert_eq!(snap_gpt_image_size(1600, 900), (1536, 1024));
        assert_eq!(snap_gpt_image_size(1024, 1024), (1024, 1024));
        // Degenerate height falls back to the square size.
        assert_eq!(snap_gpt_image_size(256, 0), (1024, 1024));
    }

    #[test]
    fn gpt_image_body_snaps_small_square_to_1024() {
        let req = ImageGenRequest {
            model: None,
            prompt: "sprite sheet".into(),
            negative_prompt: None,
            width: 256,
            height: 256,
            steps: None,
            seed: None,
            num_images: 1,
            quality: None,
            style_image: None,
            reference_images: Vec::new(),
        };
        // gpt-image snaps the rejected 256x256 up to a valid size...
        let gpt = build_image_generation_body(&req, "gpt-image-2");
        assert_eq!(gpt["size"], "1024x1024");
        // ...while dall-e keeps the verbatim request size.
        let dalle = build_image_generation_body(&req, "dall-e-2");
        assert_eq!(dalle["size"], "256x256");
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
    fn resize_png_to_changes_and_preserves_dims() {
        let src = one_pixel_png();
        let resized = resize_png_to(&src, 8, 8).expect("resize should succeed");
        let img = image::load_from_memory(&resized).expect("decode resized");
        assert_eq!((img.width(), img.height()), (8, 8));
        // Same-size request returns a same-dimension image.
        let same = resize_png_to(&resized, 8, 8).expect("same-size resize");
        let img2 = image::load_from_memory(&same).expect("decode same");
        assert_eq!((img2.width(), img2.height()), (8, 8));
    }

    #[test]
    fn decode_image_data_errors_without_b64() {
        let err = decode_image_data(vec![ImageData { b64_json: None }]).expect_err("missing b64_json must error");
        assert!(matches!(err, BackendError::InvalidResponse(_)));
    }

    #[test]
    fn decode_image_data_round_trips_base64() {
        let png = one_pixel_png();
        let encoded = base64::engine::general_purpose::STANDARD.encode(&png);
        let decoded = decode_image_data(vec![ImageData { b64_json: Some(encoded) }]).expect("valid base64 decodes");
        assert_eq!(decoded, vec![png]);
    }

    #[test]
    fn use_image_stream_only_for_single_gpt_image() {
        assert!(use_image_stream("gpt-image-2", 1));
        assert!(!use_image_stream("gpt-image-2", 4));
        assert!(!use_image_stream("dall-e-3", 1));
    }

    /// Live smoke test against the real `OpenAI` image API. Ignored by default: it
    /// needs `OPENAI_API_KEY` in the environment and spends real money on one
    /// gpt-image generation. Confirms the streaming SSE path decodes (no "error
    /// decoding response body") and that partial previews arrive before the
    /// final image. Run with:
    ///   `OPENAI_API_KEY=sk-... cargo nextest run -p pixhaus-ai --run-ignored all openai_stream_live`
    #[tokio::test]
    #[ignore = "hits the live OpenAI API; needs OPENAI_API_KEY and costs money"]
    async fn openai_stream_live_emits_partials_then_final() {
        let Ok(key) = std::env::var("OPENAI_API_KEY") else {
            eprintln!("OPENAI_API_KEY not set; skipping live streaming test");
            return;
        };
        let backend = OpenAiBackend::new(key).unwrap();
        let (progress, mut rx) = VerbProgress::channel();

        // Drain progress on a task so a full channel never blocks the request.
        let drain = tokio::spawn(async move {
            let mut partials = 0_usize;
            while let Some(event) = rx.recv().await {
                if matches!(event, VerbProgressEvent::PartialPixels { .. }) {
                    partials += 1;
                }
            }
            partials
        });

        let req = ImageGenRequest {
            model: None,
            prompt: "a small pixel-art mushroom on a white background".to_owned(),
            negative_prompt: None,
            width: 1024,
            height: 1024,
            steps: None,
            seed: None,
            num_images: 1,
            quality: Some(ImageQuality::Medium),
            style_image: None,
            reference_images: Vec::new(),
        };

        let resp = backend
            .invoke(InferenceRequest::ImageGeneration(req), progress, CancellationToken::new())
            .await
            .expect("live image generation succeeds");

        let InferenceResponse::Image(image) = resp else {
            panic!("expected an image response from an image-generation request");
        };
        assert!(!image.images.is_empty(), "a final image must be returned");

        let partials = drain.await.expect("drain task joins");
        assert!(partials > 0, "at least one partial preview frame should stream in");
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
        .expect("partial frame parses");
        assert!(finals.is_empty());
        assert!(matches!(rx.recv().await, Some(VerbProgressEvent::PartialPixels { .. })));
        handle_openai_sse_frame(
            &format!("data: {{\"type\":\"image_generation.completed\",\"data\":[{{\"b64_json\":\"{encoded}\"}}]}}\n\n"),
            &progress,
            &mut finals,
        )
        .await
        .expect("completed frame parses");
        assert_eq!(finals, vec![png]);
    }

    #[tokio::test]
    async fn openai_image_stream_frame_surfaces_error_event() {
        let (progress, _rx) = VerbProgress::channel();
        let mut finals = Vec::new();
        let err = handle_openai_sse_frame(
            "data: {\"type\":\"error\",\"error\":{\"message\":\"Invalid size 256x256\"}}\n\n",
            &progress,
            &mut finals,
        )
        .await
        .expect_err("error event must surface");
        assert!(matches!(err, BackendError::InvalidResponse(ref m) if m.contains("Invalid size 256x256")));
        assert!(finals.is_empty());
    }

    fn one_pixel_png() -> Vec<u8> {
        let img = image::RgbaImage::from_pixel(1, 1, image::Rgba([255, 0, 0, 255]));
        let mut bytes = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png)
            .expect("encode one-pixel png");
        bytes
    }
}
