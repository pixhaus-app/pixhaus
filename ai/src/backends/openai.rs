//! `OpenAI` image backend adapter.
//!
//! Scoped to image generation and editing over `/v1/images/generations`
//! and `/v1/images/edits`. Text, vision, and embedding paths are not
//! ported in this slice — the request types they need live elsewhere, and
//! advertising those capabilities would make this backend selectable for a
//! request it cannot satisfy. Implemented as a raw HTTP client for
//! consistency with the other adapters.
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
use serde::Deserialize;
use tokio::select;
use tokio_util::sync::CancellationToken;
use tracing::{debug, instrument, warn};

use super::{
    BackendError, ImageEditRequest, ImageGenRequest, ImageGenResponse, ImageQuality,
    InferenceBackend, InferenceRequest, InferenceResponse, Result, VerbProgress, check_http_status,
};
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
    #[must_use]
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::builder()
                // Image generation can run for minutes; the reference-sheet
                // verb advertises a 300s max latency. 120s aborted mid-gen.
                .timeout(Duration::from_secs(300))
                .build()
                .unwrap_or_default(),
            api_key: api_key.into(),
            base_url: BASE_URL.into(),
            image_model: DEFAULT_IMAGE_MODEL.into(),
            image_edit_model: DEFAULT_IMAGE_EDIT_MODEL.into(),
        }
    }

    /// Constructs an adapter by reading the API key from the OS keychain.
    pub fn from_keychain() -> Result<Self> {
        let key = super::ApiKeyStore::get("openai")?;
        Ok(Self::new(key))
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

    #[allow(clippy::cast_precision_loss)]
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
        let raw: OpenAiImageResponse = http_resp.json().await.map_err(BackendError::Network)?;
        let images = decode_image_data(raw.data)?;
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
    async fn invoke(
        &self,
        request: InferenceRequest,
        progress: VerbProgress,
        cancel: CancellationToken,
    ) -> Result<InferenceResponse> {
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
    let img = image::load_from_memory(bytes)
        .map_err(|e| BackendError::InvalidResponse(format!("image decode failed: {e}")))?;
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
fn fit_images_to_request(
    images: Vec<Vec<u8>>,
    req: &ImageGenRequest,
    model: &str,
) -> Result<Vec<Vec<u8>>> {
    if !is_gpt_image_model(model) {
        return Ok(images);
    }
    let (snapped_w, snapped_h) = snap_gpt_image_size(req.width, req.height);
    if snapped_w == req.width && snapped_h == req.height {
        return Ok(images);
    }
    images
        .into_iter()
        .map(|bytes| resize_png_to(&bytes, req.width, req.height))
        .collect()
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
struct OpenAiImageResponse {
    data: Vec<ImageData>,
}

#[derive(Deserialize)]
struct ImageData {
    b64_json: Option<String>,
}

// Preserve the quality enum in this module's adapter surface.
fn _assert_quality(_: ImageQuality) {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backends::ImageQuality;

    #[test]
    fn capabilities_are_image_only() {
        let b = OpenAiBackend::new("k");
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
        assert!(OpenAiBackend::new("k").is_available());
        assert!(!OpenAiBackend::new("").is_available());
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
    fn gpt_image_body_is_non_streaming_png() {
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
        // Streaming is not used on the image path; the buffered JSON image
        // response is the only path.
        assert!(body.get("stream").is_none());
        assert!(body.get("partial_images").is_none());
        assert!(body.get("response_format").is_none());
        // Unsupported gpt-image fields are dropped, not forwarded.
        assert!(body.get("negative_prompt").is_none());
        assert!(body.get("seed").is_none());
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
        let err = decode_image_data(vec![ImageData { b64_json: None }])
            .expect_err("missing b64_json must error");
        assert!(matches!(err, BackendError::InvalidResponse(_)));
    }

    #[test]
    fn decode_image_data_round_trips_base64() {
        let png = one_pixel_png();
        let encoded = base64::engine::general_purpose::STANDARD.encode(&png);
        let decoded = decode_image_data(vec![ImageData {
            b64_json: Some(encoded),
        }])
        .expect("valid base64 decodes");
        assert_eq!(decoded, vec![png]);
    }

    fn one_pixel_png() -> Vec<u8> {
        let img = image::RgbaImage::from_pixel(1, 1, image::Rgba([255, 0, 0, 255]));
        let mut bytes = Vec::new();
        img.write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Png,
        )
        .expect("encode one-pixel png");
        bytes
    }
}
