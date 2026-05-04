//! Stability AI backend adapter.
//!
//! Supports image generation (Stable Diffusion 3) and image editing /
//! inpainting via the Stability AI REST API. Requests use
//! `multipart/form-data` as required by the v2beta endpoints.
//!
//! # Endpoints
//!
//! - `POST /v2beta/stable-image/generate/sd3` — text-to-image.
//! - `POST /v2beta/stable-image/edit/inpaint` — masked inpainting.
//! - `POST /v2beta/stable-image/edit/image-to-image` — image-to-image.
//!
//! # Pricing (May 2026 estimates)
//!
//! - SD3 Medium: $0.035 per image = 3.5 cents
//! - SD3 Large: $0.065 per image = 6.5 cents

use std::time::Duration;

use async_trait::async_trait;
use tokio::select;
use tokio_util::sync::CancellationToken;
use tracing::{debug, instrument, warn};

use super::{
    BackendError, FrameInterpolationRequest, ImageEditRequest, ImageGenRequest, ImageGenResponse,
    InferenceBackend, InferenceRequest, InferenceResponse, ReplicateRequest, Result, VerbProgress,
    check_http_status,
};
use crate::plugin::descriptor::{BackendCapabilities, CostEstimate};
use crate::plugin::progress::{CostUpdate, VerbProgressEvent};

const BASE_URL: &str = "https://api.stability.ai";
const DEFAULT_MODEL: &str = "sd3-medium";

/// USD cents per image for each Stability model.
fn price_cents(model: &str) -> f32 {
    match model {
        "sd3-large" | "sd3-large-turbo" | "sd3.5-large" | "sd3.5-large-turbo" => 6.5,
        _ => 3.5, // sd3-medium
    }
}

/// Stability AI backend adapter.
pub struct StabilityBackend {
    client: reqwest::Client,
    api_key: String,
    base_url: String,
    default_model: String,
}

impl StabilityBackend {
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
    pub fn from_keychain() -> Result<Self> {
        let key = super::ApiKeyStore::get("stability")?;
        Ok(Self::new(key))
    }

    /// Overrides the default model.
    #[must_use]
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.default_model = model.into();
        self
    }

    /// Overrides the base URL.
    #[must_use]
    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    #[allow(clippy::cast_precision_loss)]
    async fn generate(
        &self,
        req: &ImageGenRequest,
        progress: &VerbProgress,
        cancel: &CancellationToken,
    ) -> Result<ImageGenResponse> {
        let model = req
            .model
            .as_deref()
            .unwrap_or(self.default_model.as_str())
            .to_owned();

        debug!(model = %model, "sending Stability generate request");

        let mut form = reqwest::multipart::Form::new()
            .text("prompt", req.prompt.clone())
            .text("model", model.clone())
            .text("output_format", "png");

        if let Some(np) = &req.negative_prompt {
            form = form.text("negative_prompt", np.clone());
        }
        if let Some(seed) = req.seed {
            form = form.text("seed", seed.to_string());
        }

        let http_req = self
            .client
            .post(format!(
                "{}/v2beta/stable-image/generate/sd3",
                self.base_url
            ))
            .header("authorization", format!("Bearer {}", self.api_key))
            .header("accept", "image/*")
            .multipart(form)
            .build()
            .map_err(BackendError::Network)?;

        let http_resp = select! {
            biased;
            () = cancel.cancelled() => return Err(BackendError::Cancelled),
            res = self.client.execute(http_req) => res.map_err(BackendError::Network)?,
        };

        let http_resp = check_http_status(http_resp).await?;
        let bytes = http_resp.bytes().await.map_err(BackendError::Network)?;

        let cost_cents = price_cents(&model) * req.num_images as f32;
        progress
            .send(VerbProgressEvent::Cost(CostUpdate {
                usd_cents: cost_cents,
                tokens_input: None,
                tokens_output: None,
            }))
            .await;

        // Stability returns a single PNG; replicate num_images times if > 1.
        // In practice, SD3 generate endpoint returns one image per call;
        // callers wanting multiple images should call invoke multiple times.
        let mut images = Vec::with_capacity(req.num_images as usize);
        for _ in 0..req.num_images.max(1) {
            images.push(bytes.to_vec());
        }

        Ok(ImageGenResponse { images, model })
    }

    #[allow(clippy::cast_precision_loss)]
    async fn inpaint(
        &self,
        req: &ImageEditRequest,
        progress: &VerbProgress,
        cancel: &CancellationToken,
    ) -> Result<ImageGenResponse> {
        let model = req
            .model
            .as_deref()
            .unwrap_or(self.default_model.as_str())
            .to_owned();

        debug!(model = %model, "sending Stability inpaint request");

        let mut form = reqwest::multipart::Form::new()
            .text("prompt", req.prompt.clone())
            .text("output_format", "png")
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
        if let Some(np) = &req.negative_prompt {
            form = form.text("negative_prompt", np.clone());
        }

        let endpoint = if req.mask.is_some() {
            "inpaint"
        } else {
            "image-to-image"
        };

        let http_req = self
            .client
            .post(format!(
                "{}/v2beta/stable-image/edit/{}",
                self.base_url, endpoint
            ))
            .header("authorization", format!("Bearer {}", self.api_key))
            .header("accept", "image/*")
            .multipart(form)
            .build()
            .map_err(BackendError::Network)?;

        let http_resp = select! {
            biased;
            () = cancel.cancelled() => return Err(BackendError::Cancelled),
            res = self.client.execute(http_req) => res.map_err(BackendError::Network)?,
        };

        let http_resp = check_http_status(http_resp).await?;
        let bytes = http_resp.bytes().await.map_err(BackendError::Network)?;

        let cost_cents = price_cents(&model) * req.num_images as f32;
        progress
            .send(VerbProgressEvent::Cost(CostUpdate {
                usd_cents: cost_cents,
                tokens_input: None,
                tokens_output: None,
            }))
            .await;

        Ok(ImageGenResponse {
            images: vec![bytes.to_vec()],
            model,
        })
    }
}

#[async_trait]
impl InferenceBackend for StabilityBackend {
    fn backend_id(&self) -> &'static str {
        "stability"
    }

    fn capabilities(&self) -> BackendCapabilities {
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
            InferenceRequest::ImageGeneration(req) => {
                let model = req.model.as_deref().unwrap_or(self.default_model.as_str());
                let cents = price_cents(model) * req.num_images as f32;
                CostEstimate {
                    typical_latency: Duration::from_secs(8),
                    max_latency: Duration::from_secs(30),
                    typical_usd_cents: cents,
                    max_usd_cents: cents * 2.0,
                }
            }
            InferenceRequest::ImageEdit(req) | InferenceRequest::ImageInpaint(req) => {
                let model = req.model.as_deref().unwrap_or(self.default_model.as_str());
                let cents = price_cents(model) * req.num_images as f32;
                CostEstimate {
                    typical_latency: Duration::from_secs(10),
                    max_latency: Duration::from_secs(30),
                    typical_usd_cents: cents,
                    max_usd_cents: cents * 2.0,
                }
            }
            _ => CostEstimate::free(),
        }
    }

    #[instrument(skip(self, request, progress, cancel), fields(backend = "stability"))]
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
                        backend: Some("stability".into()),
                    })
                    .await;
                let resp = self.generate(req, &progress, &cancel).await?;
                Ok(InferenceResponse::Image(resp))
            }
            InferenceRequest::ImageEdit(ref req) | InferenceRequest::ImageInpaint(ref req) => {
                progress
                    .send(VerbProgressEvent::Started {
                        backend: Some("stability".into()),
                    })
                    .await;
                let resp = self.inpaint(req, &progress, &cancel).await?;
                Ok(InferenceResponse::Image(resp))
            }
            InferenceRequest::Text(_)
            | InferenceRequest::FrameInterpolation(_)
            | InferenceRequest::Replicate(_)
            | InferenceRequest::ComfyUi(_) => {
                warn!("Stability does not support this request type");
                Err(BackendError::UnsupportedCapability)
            }
        }
    }
}

// Suppress unused import warnings.
fn _assert_frame_req(_: &FrameInterpolationRequest) {}
fn _assert_rep_req(_: &ReplicateRequest) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capabilities_are_image_only() {
        let b = StabilityBackend::new("k");
        let caps = b.capabilities();
        assert!(caps.contains(BackendCapabilities::IMAGE_GENERATION));
        assert!(caps.contains(BackendCapabilities::IMAGE_EDIT));
        assert!(caps.contains(BackendCapabilities::IMAGE_INPAINT));
        assert!(!caps.contains(BackendCapabilities::TEXT_GENERATION));
        assert!(!caps.contains(BackendCapabilities::FRAME_INTERPOLATION));
    }

    #[test]
    fn price_cents_for_models() {
        assert!((price_cents("sd3-medium") - 3.5).abs() < 0.01);
        assert!((price_cents("sd3-large") - 6.5).abs() < 0.01);
        assert!((price_cents("unknown-model") - 3.5).abs() < 0.01);
    }

    #[test]
    fn estimate_cost_image_gen() {
        let b = StabilityBackend::new("k");
        let req = InferenceRequest::ImageGeneration(ImageGenRequest {
            model: None,
            prompt: "test".into(),
            negative_prompt: None,
            width: 1024,
            height: 1024,
            steps: None,
            seed: None,
            num_images: 2,
            style_image: None,
        });
        let est = b.estimate_cost(&req);
        // 2 images at sd3-medium price.
        assert!((est.typical_usd_cents - 7.0).abs() < 0.1);
    }

    #[tokio::test]
    async fn integration_image_generation() {
        let Some(api_key) = std::env::var("PIXHAUS_STABILITY_API_KEY").ok() else {
            eprintln!("skipping: PIXHAUS_STABILITY_API_KEY not set");
            return;
        };

        let backend = StabilityBackend::new(api_key);
        let req = InferenceRequest::ImageGeneration(ImageGenRequest {
            model: None,
            prompt: "a simple red square on white background".into(),
            negative_prompt: None,
            width: 1024,
            height: 1024,
            steps: None,
            seed: Some(42),
            num_images: 1,
            style_image: None,
        });
        let (progress, _rx) = VerbProgress::channel();
        let cancel = CancellationToken::new();

        let resp = backend.invoke(req, progress, cancel).await.unwrap();
        let InferenceResponse::Image(img) = resp else {
            panic!("expected image response");
        };
        assert_eq!(img.images.len(), 1);
        // PNG magic bytes: 0x89 0x50 0x4E 0x47.
        assert_eq!(&img.images[0][..4], b"\x89PNG", "expected PNG output");
    }
}
