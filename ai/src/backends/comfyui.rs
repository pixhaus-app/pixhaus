//! `ComfyUI` backend adapter.
//!
//! `ComfyUI` is a node-based diffusion pipeline runner. This adapter
//! submits workflow JSON to a local (or remote) `ComfyUI` server, polls
//! history until the job completes, and downloads the output images.
//!
//! # API surface
//!
//! - `POST /prompt` — queue a workflow.
//! - `GET /history/{prompt_id}` — poll job status.
//! - `GET /view?filename=...&type=output` — download output images.
//! - `GET /system_stats` — verify the server is alive.
//!
//! # Default base URL
//!
//! `http://localhost:8188`
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
    BackendError, FrameInterpolationRequest, ImageEditRequest, ImageGenRequest, ImageGenResponse,
    InferenceBackend, InferenceRequest, InferenceResponse, Result, VerbProgress, check_http_status,
};
use crate::plugin::descriptor::{BackendCapabilities, CostEstimate};
use crate::plugin::progress::VerbProgressEvent;

const DEFAULT_BASE_URL: &str = "http://localhost:8188";
const DEFAULT_POLL_INTERVAL: Duration = Duration::from_millis(500);
const DEFAULT_JOB_TIMEOUT: Duration = Duration::from_secs(300);

/// `ComfyUI` backend adapter.
///
/// Construct via [`ComfyUiBackend::new`]. No API key required.
pub struct ComfyUiBackend {
    client: reqwest::Client,
    base_url: String,
    poll_interval: Duration,
    job_timeout: Duration,
    /// Client ID sent with each job, used to track WebSocket events in
    /// future (currently unused; kept for forward compatibility).
    client_id: String,
}

impl ComfyUiBackend {
    /// Constructs an adapter pointing at a local `ComfyUI` server.
    #[must_use]
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
            base_url: DEFAULT_BASE_URL.into(),
            poll_interval: DEFAULT_POLL_INTERVAL,
            job_timeout: DEFAULT_JOB_TIMEOUT,
            client_id: uuid_v4_simple(),
        }
    }

    /// Overrides the base URL.
    #[must_use]
    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    /// Overrides the polling interval.
    #[must_use]
    pub fn with_poll_interval(mut self, interval: Duration) -> Self {
        self.poll_interval = interval;
        self
    }

    /// Overrides the per-job timeout.
    #[must_use]
    pub fn with_job_timeout(mut self, timeout: Duration) -> Self {
        self.job_timeout = timeout;
        self
    }

    /// Submits a workflow and returns the images produced.
    #[allow(
        clippy::too_many_lines,
        clippy::disallowed_methods,
        clippy::cast_precision_loss
    )]
    async fn run_workflow(
        &self,
        workflow: serde_json::Value,
        timeout_override: Option<Duration>,
        progress: &VerbProgress,
        cancel: &CancellationToken,
    ) -> Result<Vec<Vec<u8>>> {
        let body = serde_json::json!({
            "prompt": workflow,
            "client_id": self.client_id,
        });

        debug!("submitting ComfyUI workflow");

        let submit_req = self
            .client
            .post(format!("{}/prompt", self.base_url))
            .json(&body)
            .build()
            .map_err(BackendError::Network)?;

        let submit_resp = select! {
            biased;
            () = cancel.cancelled() => return Err(BackendError::Cancelled),
            res = self.client.execute(submit_req) => res.map_err(BackendError::Network)?,
        };

        let submit_resp = check_http_status(submit_resp).await?;
        let queued: QueueResponse = submit_resp.json().await.map_err(BackendError::Network)?;
        let prompt_id = queued.prompt_id;

        progress
            .send(VerbProgressEvent::Step {
                fraction: None,
                message: format!("ComfyUI job {prompt_id} queued"),
            })
            .await;

        // Poll history until the job appears and has outputs. Per-call
        // override (from `ComfyUiRequest.timeout`) wins over the
        // backend-level default so callers can dial the deadline up for
        // long jobs or down for quick previews.
        let job_timeout = timeout_override.unwrap_or(self.job_timeout);
        let started = std::time::Instant::now();
        loop {
            if started.elapsed() > job_timeout {
                return Err(BackendError::Other("ComfyUI job timed out".into()));
            }

            select! {
                biased;
                () = cancel.cancelled() => return Err(BackendError::Cancelled),
                () = tokio::time::sleep(self.poll_interval) => {}
            }

            let history_req = self
                .client
                .get(format!("{}/history/{}", self.base_url, prompt_id))
                .build()
                .map_err(BackendError::Network)?;

            let history_resp = select! {
                biased;
                () = cancel.cancelled() => return Err(BackendError::Cancelled),
                res = self.client.execute(history_req) => res.map_err(BackendError::Network)?,
            };

            if !history_resp.status().is_success() {
                // Job not in history yet; keep polling.
                continue;
            }

            let history: serde_json::Value =
                history_resp.json().await.map_err(BackendError::Network)?;

            let Some(entry) = history.get(&prompt_id) else {
                continue;
            };

            // Check for errors.
            if let Some(status) = entry.get("status") {
                if let Some(msgs) = status.get("messages").and_then(|m| m.as_array()) {
                    let failed = msgs.iter().any(|m| {
                        m.as_array().is_some_and(|a| {
                            a.first().and_then(|v| v.as_str()) == Some("execution_error")
                        })
                    });
                    if failed {
                        return Err(BackendError::Other("ComfyUI execution error".into()));
                    }
                }
            }

            // Extract output filenames.
            let Some(outputs) = entry.get("outputs") else {
                continue;
            };

            let filenames: Vec<(String, String)> = outputs
                .as_object()
                .into_iter()
                .flat_map(|map| map.values())
                .flat_map(|node| {
                    node.get("images")
                        .and_then(|imgs| imgs.as_array())
                        .into_iter()
                        .flatten()
                })
                .filter_map(|img| {
                    let filename = img.get("filename").and_then(|f| f.as_str())?.to_owned();
                    let subfolder = img
                        .get("subfolder")
                        .and_then(|s| s.as_str())
                        .unwrap_or("")
                        .to_owned();
                    Some((filename, subfolder))
                })
                .collect();

            if filenames.is_empty() {
                continue;
            }

            // Download images.
            let total = filenames.len();
            let mut images = Vec::with_capacity(total);
            for (filename, subfolder) in &filenames {
                // Build the /view URL with `query_pairs_mut` so any `&`,
                // `#`, space, or non-ASCII character in the filename or
                // subfolder gets percent-encoded — `format!` would let
                // those break the request.
                let mut url =
                    reqwest::Url::parse(&format!("{}/view", self.base_url)).map_err(|e| {
                        BackendError::InvalidResponse(format!(
                            "ComfyUI base URL is not parseable: {e}"
                        ))
                    })?;
                {
                    let mut q = url.query_pairs_mut();
                    q.append_pair("filename", filename);
                    q.append_pair("type", "output");
                    if !subfolder.is_empty() {
                        q.append_pair("subfolder", subfolder);
                    }
                }

                let img_req = self
                    .client
                    .get(url)
                    .build()
                    .map_err(BackendError::Network)?;

                let img_resp = select! {
                    biased;
                    () = cancel.cancelled() => return Err(BackendError::Cancelled),
                    res = self.client.execute(img_req) => res.map_err(BackendError::Network)?,
                };
                let img_resp = check_http_status(img_resp).await?;
                let bytes = img_resp.bytes().await.map_err(BackendError::Network)?;
                images.push(bytes.to_vec());

                progress
                    .send(VerbProgressEvent::Step {
                        fraction: Some(images.len() as f32 / total as f32),
                        message: format!("Downloaded output image {filename}"),
                    })
                    .await;
            }

            return Ok(images);
        }
    }

    /// Builds a minimal txt2img workflow for SD-based models.
    #[allow(clippy::disallowed_methods)]
    fn txt2img_workflow(req: &ImageGenRequest) -> serde_json::Value {
        // A minimal ComfyUI workflow for Stable Diffusion txt2img.
        // Node IDs are arbitrary; this is a simplified representation.
        let seed = req.seed.unwrap_or(0);
        let steps = req.steps.unwrap_or(20);
        let negative = req.negative_prompt.as_deref().unwrap_or("ugly, blurry");
        serde_json::json!({
            "3": {
                "class_type": "KSampler",
                "inputs": {
                    "seed": seed,
                    "steps": steps,
                    "cfg": 7.0,
                    "sampler_name": "euler",
                    "scheduler": "normal",
                    "denoise": 1.0,
                    "model": ["4", 0],
                    "positive": ["6", 0],
                    "negative": ["7", 0],
                    "latent_image": ["5", 0]
                }
            },
            "4": {
                "class_type": "CheckpointLoaderSimple",
                "inputs": { "ckpt_name": "v1-5-pruned-emaonly.ckpt" }
            },
            "5": {
                "class_type": "EmptyLatentImage",
                "inputs": {
                    "width": req.width,
                    "height": req.height,
                    "batch_size": req.num_images.max(1),
                }
            },
            "6": {
                "class_type": "CLIPTextEncode",
                "inputs": { "text": req.prompt, "clip": ["4", 1] }
            },
            "7": {
                "class_type": "CLIPTextEncode",
                "inputs": { "text": negative, "clip": ["4", 1] }
            },
            "8": {
                "class_type": "VAEDecode",
                "inputs": { "samples": ["3", 0], "vae": ["4", 2] }
            },
            "9": {
                "class_type": "SaveImage",
                "inputs": { "filename_prefix": "pixhaus", "images": ["8", 0] }
            }
        })
    }

    /// Builds a minimal img2img workflow.
    #[allow(clippy::disallowed_methods)]
    fn img2img_workflow(req: &ImageEditRequest) -> serde_json::Value {
        let img_b64 = base64::engine::general_purpose::STANDARD.encode(&req.image);
        let negative = req.negative_prompt.as_deref().unwrap_or("ugly, blurry");
        serde_json::json!({
            "1": {
                "class_type": "ETN_LoadImageBase64",
                "inputs": { "image": img_b64 }
            },
            "2": {
                "class_type": "CheckpointLoaderSimple",
                "inputs": { "ckpt_name": "v1-5-pruned-emaonly.ckpt" }
            },
            "3": {
                "class_type": "CLIPTextEncode",
                "inputs": { "text": req.prompt, "clip": ["2", 1] }
            },
            "4": {
                "class_type": "CLIPTextEncode",
                "inputs": { "text": negative, "clip": ["2", 1] }
            },
            "5": {
                "class_type": "VAEEncode",
                "inputs": { "pixels": ["1", 0], "vae": ["2", 2] }
            },
            "6": {
                "class_type": "KSampler",
                "inputs": {
                    "seed": 0,
                    "steps": 20,
                    "cfg": 7.0,
                    "sampler_name": "euler",
                    "scheduler": "normal",
                    "denoise": 0.7,
                    "model": ["2", 0],
                    "positive": ["3", 0],
                    "negative": ["4", 0],
                    "latent_image": ["5", 0]
                }
            },
            "7": {
                "class_type": "VAEDecode",
                "inputs": { "samples": ["6", 0], "vae": ["2", 2] }
            },
            "8": {
                "class_type": "SaveImage",
                "inputs": { "filename_prefix": "pixhaus", "images": ["7", 0] }
            }
        })
    }
}

impl Default for ComfyUiBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl InferenceBackend for ComfyUiBackend {
    fn backend_id(&self) -> &'static str {
        "comfyui"
    }

    fn capabilities(&self) -> BackendCapabilities {
        // IMAGE_INPAINT and FRAME_INTERPOLATION are not advertised: both
        // require dedicated ComfyUI workflows (mask + VAEEncodeForInpaint
        // for inpaint, AnimateDiff for interpolation) which are not yet
        // bundled. Callers can still drive those workflows manually via
        // `InferenceRequest::ComfyUi` with their own graph.
        BackendCapabilities::IMAGE_GENERATION
            .union(BackendCapabilities::IMAGE_EDIT)
            .union(BackendCapabilities::STYLE_TRAINING)
    }

    fn supports_streaming(&self) -> bool {
        false
    }

    fn estimate_cost(&self, _request: &InferenceRequest) -> CostEstimate {
        // Local inference — no monetary cost.
        CostEstimate {
            typical_latency: Duration::from_secs(20),
            max_latency: Duration::from_secs(300),
            typical_usd_cents: 0.0,
            max_usd_cents: 0.0,
        }
    }

    #[instrument(skip(self, request, progress, cancel), fields(backend = "comfyui"))]
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
                        backend: Some("comfyui".into()),
                    })
                    .await;
                let workflow = Self::txt2img_workflow(req);
                let images = self
                    .run_workflow(workflow, None, &progress, &cancel)
                    .await?;
                Ok(InferenceResponse::Image(ImageGenResponse {
                    images,
                    model: "comfyui/txt2img".into(),
                }))
            }
            InferenceRequest::ImageEdit(ref req) => {
                progress
                    .send(VerbProgressEvent::Started {
                        backend: Some("comfyui".into()),
                    })
                    .await;
                let workflow = Self::img2img_workflow(req);
                let images = self
                    .run_workflow(workflow, None, &progress, &cancel)
                    .await?;
                Ok(InferenceResponse::Image(ImageGenResponse {
                    images,
                    model: "comfyui/img2img".into(),
                }))
            }
            InferenceRequest::ImageInpaint(_) => {
                // Inpainting needs a dedicated workflow with `LoadImageMask`
                // + `VAEEncodeForInpaint` nodes wired to the request mask.
                // The default img2img workflow has no mask handling, so
                // dispatching to it would silently drop the mask. Drive a
                // bespoke graph through `InferenceRequest::ComfyUi`
                // instead until the workflow ships in a follow-up.
                Err(BackendError::UnsupportedCapability)
            }
            InferenceRequest::ComfyUi(ref req) => {
                progress
                    .send(VerbProgressEvent::Started {
                        backend: Some("comfyui".into()),
                    })
                    .await;
                let workflow = req.workflow.clone();
                let images = self
                    .run_workflow(workflow, req.timeout, &progress, &cancel)
                    .await?;
                Ok(InferenceResponse::Image(ImageGenResponse {
                    images,
                    model: "comfyui/custom".into(),
                }))
            }
            InferenceRequest::FrameInterpolation(_) => {
                // Frame interpolation via ComfyUI requires a specific
                // workflow (e.g. AnimateDiff). Surface this as a
                // capability mismatch so callers can branch on it the
                // same way they do for other unsupported request types,
                // and run a bespoke graph via `InferenceRequest::ComfyUi`.
                Err(BackendError::UnsupportedCapability)
            }
            InferenceRequest::Text(_) | InferenceRequest::Replicate(_) => {
                warn!("ComfyUI does not support this request type");
                Err(BackendError::UnsupportedCapability)
            }
        }
    }
}

// ── Helpers ────────────────────────────────────────────────────────────────

/// Generates a short pseudo-random client ID.
///
/// Uses a simple `XorShift` seeded with system time rather than pulling in
/// a UUID crate — the ID is used for `ComfyUI` session tracking, not
/// cryptography.
#[allow(clippy::cast_possible_truncation)]
fn uuid_v4_simple() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos()) as u64;
    // XorShift64.
    let mut x = seed ^ 0x9e37_79b9_7f4a_7c15;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    format!("{x:016x}")
}

// ── Wire types ─────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct QueueResponse {
    prompt_id: String,
}

// Suppress unused import warning.
fn _assert_frame_req(_: &FrameInterpolationRequest) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capabilities_include_image_and_edit() {
        let b = ComfyUiBackend::new();
        let caps = b.capabilities();
        assert!(caps.contains(BackendCapabilities::IMAGE_GENERATION));
        assert!(caps.contains(BackendCapabilities::IMAGE_EDIT));
        // Inpaint and frame interpolation are intentionally not
        // advertised — they need bespoke workflows the caller must
        // provide via `InferenceRequest::ComfyUi`.
        assert!(!caps.contains(BackendCapabilities::IMAGE_INPAINT));
        assert!(!caps.contains(BackendCapabilities::FRAME_INTERPOLATION));
        assert!(!caps.contains(BackendCapabilities::TEXT_GENERATION));
    }

    #[test]
    fn estimate_cost_is_free() {
        let b = ComfyUiBackend::new();
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
        });
        let est = b.estimate_cost(&req);
        assert_eq!(est.typical_usd_cents, 0.0);
    }

    #[test]
    fn txt2img_workflow_includes_prompt() {
        let req = ImageGenRequest {
            model: None,
            prompt: "a sunny meadow".into(),
            negative_prompt: None,
            width: 512,
            height: 512,
            steps: Some(20),
            seed: Some(1),
            num_images: 1,
            quality: None,
            style_image: None,
        };
        let wf = ComfyUiBackend::txt2img_workflow(&req);
        let positive_text = wf["6"]["inputs"]["text"].as_str().unwrap();
        assert_eq!(positive_text, "a sunny meadow");
        assert_eq!(wf["5"]["inputs"]["width"], 512);
    }

    #[test]
    fn txt2img_workflow_threads_num_images_into_batch_size() {
        let req = ImageGenRequest {
            model: None,
            prompt: "test".into(),
            negative_prompt: None,
            width: 512,
            height: 512,
            steps: None,
            seed: None,
            num_images: 4,
            quality: None,
            style_image: None,
        };
        let wf = ComfyUiBackend::txt2img_workflow(&req);
        assert_eq!(wf["5"]["inputs"]["batch_size"], 4);
    }

    #[test]
    fn txt2img_workflow_clamps_zero_num_images_to_one() {
        let req = ImageGenRequest {
            model: None,
            prompt: "test".into(),
            negative_prompt: None,
            width: 512,
            height: 512,
            steps: None,
            seed: None,
            num_images: 0,
            quality: None,
            style_image: None,
        };
        let wf = ComfyUiBackend::txt2img_workflow(&req);
        // ComfyUI rejects batch_size = 0; clamp keeps the workflow runnable.
        assert_eq!(wf["5"]["inputs"]["batch_size"], 1);
    }

    #[test]
    fn view_url_percent_encodes_special_characters() {
        // The `/view` URL is built with `query_pairs_mut` so any `&`,
        // space, or non-ASCII filename gets percent-encoded. Without
        // this the request would fragment on raw `&` and ComfyUI would
        // see a different filename than was generated.
        let mut url = reqwest::Url::parse("http://localhost:8188/view").unwrap();
        url.query_pairs_mut()
            .append_pair("filename", "x y&z=1.png")
            .append_pair("type", "output")
            .append_pair("subfolder", "outputs/2026 May");
        let s = url.as_str();
        assert!(s.contains("filename=x+y%26z%3D1.png"), "got: {s}");
        assert!(s.contains("subfolder=outputs%2F2026+May"), "got: {s}");
    }

    #[test]
    fn uuid_v4_simple_is_16_hex_chars() {
        let id = uuid_v4_simple();
        assert_eq!(id.len(), 16);
        assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[tokio::test]
    async fn integration_image_generation() {
        if std::env::var("PIXHAUS_COMFYUI_RUN_INTEGRATION").is_err() {
            eprintln!("skipping: PIXHAUS_COMFYUI_RUN_INTEGRATION not set");
            return;
        }

        let base_url =
            std::env::var("PIXHAUS_COMFYUI_URL").unwrap_or_else(|_| DEFAULT_BASE_URL.into());

        let backend = ComfyUiBackend::new()
            .with_base_url(base_url)
            .with_job_timeout(Duration::from_secs(120));

        let req = InferenceRequest::ImageGeneration(ImageGenRequest {
            model: None,
            prompt: "a simple red square".into(),
            negative_prompt: None,
            width: 512,
            height: 512,
            steps: Some(10),
            seed: Some(42),
            num_images: 1,
            quality: None,
            style_image: None,
        });
        let (progress, _rx) = VerbProgress::channel();
        let cancel = CancellationToken::new();

        let resp = backend.invoke(req, progress, cancel).await.unwrap();
        let InferenceResponse::Image(img) = resp else {
            panic!("expected image response");
        };
        assert!(!img.images.is_empty());
        assert!(!img.images[0].is_empty());
    }
}
