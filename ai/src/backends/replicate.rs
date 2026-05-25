//! Replicate backend adapter.
//!
//! Replicate is a model marketplace where any model can be run by
//! submitting a prediction and polling for results. This adapter exposes
//! the generic [`InferenceRequest::Replicate`] request type so verbs can
//! run arbitrary Replicate models, plus standard image generation through
//! a configurable default model.
//!
//! # Polling
//!
//! Replicate predictions are asynchronous. This adapter submits the job,
//! then polls `/v1/predictions/{id}` at 1-second intervals until the
//! status reaches `succeeded` or `failed`. The `cancel` token fires the
//! polling loop early.
//!
//! # Pricing
//!
//! Replicate charges per-second of GPU compute. Estimates are
//! model-specific; this adapter returns a fixed rough estimate.

use std::time::Duration;

use async_trait::async_trait;
use base64::Engine as _;
use serde::Deserialize;
use tokio::select;
use tokio_util::sync::CancellationToken;
use tracing::{debug, instrument, warn};

use super::{
    BackendError, FrameInterpolationRequest, FrameInterpolationResponse, ImageEditRequest,
    ImageGenRequest, ImageGenResponse, ImageToVideoRequest, ImageToVideoResponse, InferenceBackend,
    InferenceRequest, InferenceResponse, ReplicateRequest, Result, VerbProgress, check_http_status,
};
use crate::plugin::descriptor::{BackendCapabilities, CostEstimate};
use crate::plugin::progress::VerbProgressEvent;

const BASE_URL: &str = "https://api.replicate.com/v1";
const POLL_INTERVAL: Duration = Duration::from_secs(1);
const DEFAULT_IMAGE_MODEL: &str = "stability-ai/stable-diffusion-3";

/// Replicate backend adapter.
#[derive(Debug)]
pub struct ReplicateBackend {
    client: reqwest::Client,
    api_key: String,
    base_url: String,
    image_model: String,
}

impl ReplicateBackend {
    /// Constructs an adapter with an explicit API key.
    #[must_use]
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(300))
                .build()
                .unwrap_or_default(),
            api_key: api_key.into(),
            base_url: BASE_URL.into(),
            image_model: DEFAULT_IMAGE_MODEL.into(),
        }
    }

    /// Constructs an adapter by reading the API key from the OS keychain.
    pub fn from_keychain() -> Result<Self> {
        let key = super::ApiKeyStore::get("replicate")?;
        Ok(Self::new(key))
    }

    /// Overrides the default image generation model.
    #[must_use]
    pub fn with_image_model(mut self, model: impl Into<String>) -> Self {
        self.image_model = model.into();
        self
    }

    /// Overrides the base URL (useful for testing).
    #[must_use]
    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    /// Submits a raw prediction and polls until it completes or is cancelled.
    #[allow(clippy::disallowed_methods)]
    async fn run_prediction(
        &self,
        request: ReplicateRequest,
        progress: &VerbProgress,
        cancel: &CancellationToken,
    ) -> Result<serde_json::Value> {
        // Build submission payload.
        let mut body = serde_json::json!({ "input": request.input });
        if let Some(v) = &request.version {
            body["version"] = serde_json::json!(v);
        }

        let submit_url = if request.version.is_some() {
            format!("{}/predictions", self.base_url)
        } else {
            format!("{}/models/{}/predictions", self.base_url, request.model)
        };

        debug!(model = %request.model, "submitting Replicate prediction");

        let submit_req = self
            .client
            .post(&submit_url)
            .bearer_auth(&self.api_key)
            .json(&body)
            .build()
            .map_err(BackendError::Network)?;

        let submit_resp = select! {
            biased;
            () = cancel.cancelled() => return Err(BackendError::Cancelled),
            res = self.client.execute(submit_req) => res.map_err(BackendError::Network)?,
        };

        let submit_resp = check_http_status(submit_resp).await?;
        let prediction: PredictionResponse =
            submit_resp.json().await.map_err(BackendError::Network)?;
        let id = prediction.id;

        progress
            .send(VerbProgressEvent::Step {
                fraction: None,
                message: format!("Replicate prediction {id} submitted"),
            })
            .await;

        // Poll until done.
        let poll_url = format!("{}/predictions/{}", self.base_url, id);
        loop {
            select! {
                biased;
                () = cancel.cancelled() => {
                    // Best-effort cancel the prediction on the server side.
                    let _ = self.client
                        .post(format!("{}/predictions/{}/cancel", self.base_url, id))
                        .bearer_auth(&self.api_key)
                        .send()
                        .await;
                    return Err(BackendError::Cancelled);
                }
                () = tokio::time::sleep(POLL_INTERVAL) => {}
            }

            let poll_req = self
                .client
                .get(&poll_url)
                .bearer_auth(&self.api_key)
                .build()
                .map_err(BackendError::Network)?;

            let poll_resp = select! {
                biased;
                () = cancel.cancelled() => return Err(BackendError::Cancelled),
                res = self.client.execute(poll_req) => res.map_err(BackendError::Network)?,
            };

            let poll_resp = check_http_status(poll_resp).await?;
            let status: PredictionStatus = poll_resp.json().await.map_err(BackendError::Network)?;

            match status.status.as_deref() {
                Some("succeeded") => {
                    return status.output.ok_or_else(|| {
                        BackendError::InvalidResponse("succeeded but no output".into())
                    });
                }
                Some("failed" | "canceled") => {
                    let err = status.error.unwrap_or_else(|| "unknown error".into());
                    return Err(BackendError::Other(format!(
                        "Replicate prediction failed: {err}"
                    )));
                }
                _ => {
                    let logs = status.logs.as_deref().unwrap_or("");
                    if !logs.is_empty() {
                        progress
                            .send(VerbProgressEvent::Step {
                                fraction: None,
                                message: logs.lines().last().unwrap_or("").to_owned(),
                            })
                            .await;
                    }
                }
            }
        }
    }
}

#[async_trait]
impl InferenceBackend for ReplicateBackend {
    fn backend_id(&self) -> &'static str {
        "replicate"
    }

    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities::IMAGE_GENERATION
            .union(BackendCapabilities::IMAGE_EDIT)
            .union(BackendCapabilities::FRAME_INTERPOLATION)
            .union(BackendCapabilities::VIEW_SYNTHESIS)
            .union(BackendCapabilities::SEGMENTATION)
            .union(BackendCapabilities::POSE_ESTIMATION)
            .union(BackendCapabilities::STYLE_TRAINING)
            .union(BackendCapabilities::IMAGE_TO_VIDEO)
    }

    fn supports_streaming(&self) -> bool {
        false
    }

    #[allow(clippy::cast_precision_loss)]
    fn estimate_cost(&self, request: &InferenceRequest) -> CostEstimate {
        match request {
            InferenceRequest::ImageGeneration(_)
            | InferenceRequest::ImageEdit(_)
            | InferenceRequest::ImageInpaint(_) => CostEstimate {
                typical_latency: Duration::from_secs(15),
                max_latency: Duration::from_secs(120),
                // SD3 on Replicate: ~$0.035 per image = 3.5 cents.
                typical_usd_cents: 3.5,
                max_usd_cents: 10.0,
            },
            InferenceRequest::FrameInterpolation(req) => CostEstimate {
                typical_latency: Duration::from_secs(30),
                max_latency: Duration::from_secs(300),
                typical_usd_cents: 5.0 * req.num_outputs as f32,
                max_usd_cents: 20.0 * req.num_outputs as f32,
            },
            InferenceRequest::Replicate(_) => CostEstimate {
                typical_latency: Duration::from_secs(20),
                max_latency: Duration::from_secs(300),
                typical_usd_cents: 5.0,
                max_usd_cents: 50.0,
            },
            InferenceRequest::ImageToVideo(_) => CostEstimate {
                // i2v is the slow, pricey path the studio flags prominently.
                typical_latency: Duration::from_secs(60),
                max_latency: Duration::from_secs(300),
                typical_usd_cents: 12.0,
                max_usd_cents: 40.0,
            },
            _ => CostEstimate::free(),
        }
    }

    #[instrument(skip(self, request, progress, cancel), fields(backend = "replicate"))]
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
                        backend: Some("replicate".into()),
                    })
                    .await;
                let model = req
                    .model
                    .clone()
                    .unwrap_or_else(|| self.image_model.clone());
                let input = build_image_gen_input(req);
                let rep_req = ReplicateRequest {
                    model: model.clone(),
                    version: None,
                    input,
                };
                let output = self.run_prediction(rep_req, &progress, &cancel).await?;
                let images = extract_url_images(&output).await?;
                Ok(InferenceResponse::Image(ImageGenResponse { images, model }))
            }
            InferenceRequest::FrameInterpolation(ref req) => {
                progress
                    .send(VerbProgressEvent::Started {
                        backend: Some("replicate".into()),
                    })
                    .await;
                let model = req
                    .model
                    .clone()
                    .unwrap_or_else(|| "google-deepmind/frame-interpolation".into());
                let input = build_interpolation_input(req);
                let rep_req = ReplicateRequest {
                    model: model.clone(),
                    version: None,
                    input,
                };
                let output = self.run_prediction(rep_req, &progress, &cancel).await?;
                let frames = extract_url_images(&output).await?;
                Ok(InferenceResponse::Frames(FrameInterpolationResponse {
                    frames,
                    model,
                }))
            }
            InferenceRequest::Replicate(req) => {
                progress
                    .send(VerbProgressEvent::Started {
                        backend: Some("replicate".into()),
                    })
                    .await;
                let output = self.run_prediction(req, &progress, &cancel).await?;
                Ok(InferenceResponse::Raw(output))
            }
            InferenceRequest::ImageEdit(ref req) | InferenceRequest::ImageInpaint(ref req) => {
                progress
                    .send(VerbProgressEvent::Started {
                        backend: Some("replicate".into()),
                    })
                    .await;
                let model = req
                    .model
                    .clone()
                    .unwrap_or_else(|| "stability-ai/stable-diffusion-3".into());
                let input = build_image_edit_input(req);
                let rep_req = ReplicateRequest {
                    model: model.clone(),
                    version: None,
                    input,
                };
                let output = self.run_prediction(rep_req, &progress, &cancel).await?;
                let images = extract_url_images(&output).await?;
                Ok(InferenceResponse::Image(ImageGenResponse { images, model }))
            }
            InferenceRequest::ImageToVideo(ref req) => {
                progress
                    .send(VerbProgressEvent::Started {
                        backend: Some("replicate".into()),
                    })
                    .await;
                let model = req
                    .model
                    .clone()
                    .unwrap_or_else(|| "stability-ai/stable-video-diffusion".into());
                let input = build_i2v_input(req);
                let rep_req = ReplicateRequest {
                    model: model.clone(),
                    version: None,
                    input,
                };
                let output = self.run_prediction(rep_req, &progress, &cancel).await?;
                let (clip, mime) = extract_url_clip(&output).await?;
                Ok(InferenceResponse::Video(ImageToVideoResponse {
                    clip,
                    mime,
                    model,
                }))
            }
            InferenceRequest::Text(_)
            | InferenceRequest::BackgroundRemoval(_)
            | InferenceRequest::ComfyUi(_) => {
                warn!("Replicate adapter does not support this request type directly");
                Err(BackendError::UnsupportedCapability)
            }
        }
    }
}

// ── Helpers ────────────────────────────────────────────────────────────────

#[allow(clippy::disallowed_methods)]
fn build_image_gen_input(req: &ImageGenRequest) -> serde_json::Value {
    let mut input = serde_json::json!({
        "prompt": req.prompt,
        "width": req.width,
        "height": req.height,
        "num_outputs": req.num_images,
    });
    if let Some(np) = &req.negative_prompt {
        input["negative_prompt"] = serde_json::json!(np);
    }
    if let Some(steps) = req.steps {
        input["num_inference_steps"] = serde_json::json!(steps);
    }
    if let Some(seed) = req.seed {
        input["seed"] = serde_json::json!(seed);
    }
    input
}

#[allow(clippy::disallowed_methods)]
fn build_image_edit_input(req: &ImageEditRequest) -> serde_json::Value {
    let img_b64 = base64::engine::general_purpose::STANDARD.encode(&req.image);
    let mut input = serde_json::json!({
        "prompt": req.prompt,
        "image": format!("data:image/png;base64,{img_b64}"),
        "num_outputs": req.num_images,
    });
    if let Some(mask) = &req.mask {
        let mask_b64 = base64::engine::general_purpose::STANDARD.encode(mask);
        input["mask"] = serde_json::json!(format!("data:image/png;base64,{mask_b64}"));
    }
    if let Some(np) = &req.negative_prompt {
        input["negative_prompt"] = serde_json::json!(np);
    }
    input
}

#[allow(clippy::disallowed_methods)]
fn build_interpolation_input(req: &FrameInterpolationRequest) -> serde_json::Value {
    let frames: Vec<String> = req
        .frames
        .iter()
        .map(|f| {
            let b64 = base64::engine::general_purpose::STANDARD.encode(f);
            format!("data:image/png;base64,{b64}")
        })
        .collect();
    serde_json::json!({
        "frames": frames,
        "num_outputs": req.num_outputs,
    })
}

#[allow(clippy::disallowed_methods)]
fn build_i2v_input(req: &ImageToVideoRequest) -> serde_json::Value {
    let img_b64 = base64::engine::general_purpose::STANDARD.encode(&req.image);
    let mut input = serde_json::json!({
        "input_image": format!("data:image/png;base64,{img_b64}"),
        "prompt": req.prompt,
        "frames_per_second": req.fps,
        "video_length": req.num_frames,
    });
    if let Some(np) = &req.negative_prompt {
        input["negative_prompt"] = serde_json::json!(np);
    }
    if let Some(seed) = req.seed {
        input["seed"] = serde_json::json!(seed);
    }
    input
}

/// Fetches a single video clip from the URL returned by Replicate, inferring
/// the MIME type from the URL extension (default `video/mp4`).
// The URL is lower-cased before comparison, so the case-sensitivity lint is a
// false positive here.
#[allow(clippy::case_sensitive_file_extension_comparisons)]
async fn extract_url_clip(output: &serde_json::Value) -> Result<(Vec<u8>, String)> {
    let url = match output {
        serde_json::Value::String(s) => s.as_str(),
        serde_json::Value::Array(arr) => arr
            .iter()
            .find_map(serde_json::Value::as_str)
            .ok_or_else(|| BackendError::InvalidResponse("i2v output array is empty".into()))?,
        _ => {
            return Err(BackendError::InvalidResponse(
                "Replicate i2v output is not a URL".into(),
            ));
        }
    };
    let lower = url.to_ascii_lowercase();
    // Fallback when the response carries no Content-Type — signed URLs often
    // lack a useful extension, so the header is preferred when present.
    let ext_mime = if lower.ends_with(".webm") {
        "video/webm"
    } else if lower.ends_with(".gif") {
        "image/gif"
    } else {
        "video/mp4"
    };
    let client = reqwest::Client::new();
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(BackendError::Network)?;
    let resp = check_http_status(resp).await?;
    let mime = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.split(';').next().unwrap_or(s).trim().to_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| ext_mime.to_owned());
    let bytes = resp.bytes().await.map_err(BackendError::Network)?;
    Ok((bytes.to_vec(), mime))
}

/// Fetches images from URLs returned by Replicate.
async fn extract_url_images(output: &serde_json::Value) -> Result<Vec<Vec<u8>>> {
    let client = reqwest::Client::new();
    let urls: Vec<&str> = match output {
        serde_json::Value::Array(arr) => arr.iter().filter_map(|v| v.as_str()).collect(),
        serde_json::Value::String(s) => vec![s.as_str()],
        _ => {
            return Err(BackendError::InvalidResponse(
                "Replicate output is not a URL or array of URLs".into(),
            ));
        }
    };

    let mut images = Vec::with_capacity(urls.len());
    for url in urls {
        let resp = client
            .get(url)
            .send()
            .await
            .map_err(BackendError::Network)?;
        // Validate status before reading the body so an HTML/JSON error
        // page doesn't sneak through as image bytes.
        let resp = check_http_status(resp).await?;
        let bytes = resp.bytes().await.map_err(BackendError::Network)?;
        images.push(bytes.to_vec());
    }
    Ok(images)
}

// ── Style training ─────────────────────────────────────────────────────────

/// Result of a completed `LoRA` training job.
#[derive(Debug)]
pub struct StyleTrainingResult {
    /// URL to download the trained `LoRA` weights (safetensors).
    pub weights_url: String,
    /// Opaque Replicate training ID for reference.
    pub training_id: String,
    /// Rough cost in USD cents.
    pub usd_cents: f32,
}

/// Parameters for [`ReplicateBackend::run_style_training`].
pub struct StyleTrainingParams<'a> {
    /// Fully-qualified Replicate model name (e.g. `"ostris/flux-dev-lora-trainer"`).
    pub training_model: &'a str,
    /// `data:application/zip;base64,...` data URI of the training image archive.
    pub images_uri: String,
    /// Training step count.
    pub steps: u32,
    /// `LoRA` rank.
    pub lora_rank: u32,
    /// Human-readable label used as the `LoRA` trigger word.
    pub label: &'a str,
}

impl ReplicateBackend {
    /// Submits a `LoRA` training job using Replicate's training API and
    /// polls until it completes or is cancelled.
    ///
    /// `params.training_model` is the fully-qualified Replicate model name
    /// (e.g. `"ostris/flux-dev-lora-trainer"`). `params.images_uri` must be a
    /// `data:application/zip;base64,...` data URI containing a zip archive
    /// of PNG training images (one file per image, any filename).
    #[allow(clippy::disallowed_methods)]
    pub async fn run_style_training(
        &self,
        params: StyleTrainingParams<'_>,
        progress: &VerbProgress,
        cancel: &CancellationToken,
    ) -> std::result::Result<StyleTrainingResult, BackendError> {
        let StyleTrainingParams {
            training_model,
            images_uri,
            steps,
            lora_rank,
            label,
        } = params;

        let (owner, name) = training_model.split_once('/').ok_or_else(|| {
            BackendError::Other(format!(
                "training model '{training_model}' is not in owner/name format"
            ))
        })?;

        let submit_url = format!("{}/models/{owner}/{name}/trainings", self.base_url);

        let body = serde_json::json!({
            "input": {
                "input_images": images_uri,
                "steps": steps,
                "lora_rank": lora_rank,
                "trigger_word": label,
            }
        });

        debug!(model = %training_model, steps, lora_rank, "submitting LoRA training job");

        let submit_req = self
            .client
            .post(&submit_url)
            .bearer_auth(&self.api_key)
            .json(&body)
            .build()
            .map_err(BackendError::Network)?;

        let submit_resp = tokio::select! {
            biased;
            () = cancel.cancelled() => return Err(BackendError::Cancelled),
            res = self.client.execute(submit_req) => res.map_err(BackendError::Network)?,
        };

        let submit_resp = check_http_status(submit_resp).await?;
        let training: TrainingResponse = submit_resp.json().await.map_err(BackendError::Network)?;
        let id = training.id;

        progress
            .send(VerbProgressEvent::Step {
                fraction: None,
                message: format!("training job {id} submitted — this may take 15-30 minutes"),
            })
            .await;

        let weights_url = self.poll_style_training(&id, progress, cancel).await?;
        Ok(StyleTrainingResult {
            weights_url,
            training_id: id,
            usd_cents: 0.0,
        })
    }

    /// Polls the Replicate training endpoint until the job succeeds, fails,
    /// or is cancelled. Returns the weights download URL on success.
    async fn poll_style_training(
        &self,
        id: &str,
        progress: &VerbProgress,
        cancel: &CancellationToken,
    ) -> std::result::Result<String, BackendError> {
        let poll_url = format!("{}/trainings/{}", self.base_url, id);
        loop {
            tokio::select! {
                biased;
                () = cancel.cancelled() => {
                    // Best-effort cancel on the server side.
                    let _ = self
                        .client
                        .post(format!("{}/trainings/{}/cancel", self.base_url, id))
                        .bearer_auth(&self.api_key)
                        .send()
                        .await;
                    return Err(BackendError::Cancelled);
                }
                () = tokio::time::sleep(POLL_INTERVAL) => {}
            }

            let poll_req = self
                .client
                .get(&poll_url)
                .bearer_auth(&self.api_key)
                .build()
                .map_err(BackendError::Network)?;

            let poll_resp = tokio::select! {
                biased;
                () = cancel.cancelled() => return Err(BackendError::Cancelled),
                res = self.client.execute(poll_req) => res.map_err(BackendError::Network)?,
            };

            let poll_resp = check_http_status(poll_resp).await?;
            let status: TrainingStatus = poll_resp.json().await.map_err(BackendError::Network)?;

            match status.status.as_deref() {
                Some("succeeded") => {
                    return status
                        .output
                        .as_ref()
                        .and_then(|o| o.get("weights"))
                        .and_then(|w| w.as_str())
                        .map(String::from)
                        .ok_or_else(|| {
                            BackendError::InvalidResponse(
                                "training succeeded but output has no weights URL".into(),
                            )
                        });
                }
                Some("failed" | "canceled") => {
                    let err = status.error.unwrap_or_else(|| "unknown error".into());
                    return Err(BackendError::Other(format!(
                        "Replicate training failed: {err}"
                    )));
                }
                _ => {
                    let logs = status.logs.as_deref().unwrap_or("");
                    if !logs.is_empty() {
                        progress
                            .send(VerbProgressEvent::Step {
                                fraction: None,
                                message: logs.lines().last().unwrap_or("").to_owned(),
                            })
                            .await;
                    }
                }
            }
        }
    }
}

// ── plugin::InferenceBackend bridge ────────────────────────────────────────

// `ReplicateBackend` implements `crate::plugin::backend::InferenceBackend` so
// it can be registered in the verb runtime's backend registry and injected
// into `VerbContext::backend`. Verbs that need Replicate-specific calls
// (like S30) downcast via `as_any()` to the concrete type and call the
// `backends::InferenceBackend` methods directly.
impl crate::plugin::backend::InferenceBackend for ReplicateBackend {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn id(&self) -> &'static str {
        "replicate"
    }

    fn capabilities(&self) -> BackendCapabilities {
        // Delegate to the S22 InferenceBackend impl.
        <Self as InferenceBackend>::capabilities(self)
    }

    fn cost_estimate(
        &self,
        required: BackendCapabilities,
    ) -> crate::plugin::descriptor::CostEstimate {
        use std::time::Duration;
        if required.contains(BackendCapabilities::STYLE_TRAINING) {
            return crate::plugin::descriptor::CostEstimate {
                typical_latency: Duration::from_secs(900),
                max_latency: Duration::from_secs(1800),
                typical_usd_cents: 200.0,
                max_usd_cents: 500.0,
            };
        }
        CostEstimate {
            typical_latency: Duration::from_secs(20),
            max_latency: Duration::from_secs(300),
            typical_usd_cents: 5.0,
            max_usd_cents: 50.0,
        }
    }
}

// ── Wire types ─────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct PredictionResponse {
    id: String,
}

#[derive(Deserialize)]
struct PredictionStatus {
    status: Option<String>,
    output: Option<serde_json::Value>,
    error: Option<String>,
    logs: Option<String>,
}

#[derive(Deserialize)]
struct TrainingResponse {
    id: String,
}

#[derive(Deserialize)]
struct TrainingStatus {
    status: Option<String>,
    output: Option<serde_json::Value>,
    error: Option<String>,
    logs: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backends::InferenceRequest;

    #[test]
    fn plugin_backend_id_matches_backend_id() {
        use crate::plugin::backend::InferenceBackend as PluginBackend;
        let b = ReplicateBackend::new("k");
        assert_eq!(<ReplicateBackend as PluginBackend>::id(&b), "replicate");
    }

    #[test]
    fn plugin_backend_cost_estimate_style_training_is_expensive() {
        use crate::plugin::backend::InferenceBackend as PluginBackend;
        use crate::plugin::descriptor::BackendCapabilities;
        let b = ReplicateBackend::new("k");
        let est = <ReplicateBackend as PluginBackend>::cost_estimate(
            &b,
            BackendCapabilities::STYLE_TRAINING,
        );
        assert!(
            est.typical_latency.as_secs() > 60,
            "style training estimate should reflect long training time"
        );
    }

    #[test]
    fn plugin_backend_downcasts_via_as_any() {
        use crate::plugin::backend::InferenceBackend as PluginBackend;
        use std::sync::Arc;
        let concrete = Arc::new(ReplicateBackend::new("k"));
        let dynamic: Arc<dyn PluginBackend> = concrete.clone();
        let recovered = dynamic.as_any().downcast_ref::<ReplicateBackend>();
        assert!(
            recovered.is_some(),
            "downcast from plugin backend to ReplicateBackend must succeed"
        );
    }

    #[test]
    fn capabilities_include_image_and_interpolation() {
        let b = ReplicateBackend::new("k");
        let caps = b.capabilities();
        assert!(caps.contains(BackendCapabilities::IMAGE_GENERATION));
        assert!(caps.contains(BackendCapabilities::FRAME_INTERPOLATION));
        assert!(caps.contains(BackendCapabilities::STYLE_TRAINING));
        assert!(!caps.contains(BackendCapabilities::TEXT_GENERATION));
    }

    #[test]
    fn build_image_gen_input_includes_prompt() {
        let req = ImageGenRequest {
            model: None,
            prompt: "a cat in space".into(),
            negative_prompt: None,
            width: 512,
            height: 512,
            steps: Some(30),
            seed: Some(42),
            num_images: 1,
            quality: None,
            style_image: None,
            reference_images: Vec::new(),
        };
        let input = build_image_gen_input(&req);
        assert_eq!(input["prompt"], "a cat in space");
        assert_eq!(input["seed"], 42);
        assert_eq!(input["num_inference_steps"], 30);
    }

    #[test]
    fn estimate_cost_image_gen_nonzero() {
        let b = ReplicateBackend::new("k");
        let req = InferenceRequest::ImageGeneration(ImageGenRequest {
            model: None,
            prompt: "test".into(),
            negative_prompt: None,
            width: 512,
            height: 512,
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

    #[tokio::test]
    async fn integration_image_generation() {
        let Some(api_key) = std::env::var("PIXHAUS_REPLICATE_API_KEY").ok() else {
            eprintln!("skipping: PIXHAUS_REPLICATE_API_KEY not set");
            return;
        };

        let backend =
            ReplicateBackend::new(api_key).with_image_model("stability-ai/stable-diffusion-3");
        let req = InferenceRequest::ImageGeneration(ImageGenRequest {
            model: None,
            prompt: "a simple red square on white background".into(),
            negative_prompt: None,
            width: 64,
            height: 64,
            steps: Some(10),
            seed: Some(42),
            num_images: 1,
            quality: None,
            style_image: None,
            reference_images: Vec::new(),
        });
        let (progress, _rx) = VerbProgress::channel();
        let cancel = CancellationToken::new();

        let resp = backend.invoke(req, progress, cancel).await.unwrap();
        let InferenceResponse::Image(img) = resp else {
            panic!("expected image response");
        };
        assert_eq!(img.images.len(), 1);
        assert!(!img.images[0].is_empty());
    }
}
