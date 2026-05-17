//! fal.ai image backend for Flux, Recraft, and utility image models.

use std::time::Duration;

use async_trait::async_trait;
use base64::Engine as _;
use futures::StreamExt;
use serde::Deserialize;
use tokio::select;
use tokio_util::sync::CancellationToken;

use super::{
    BackendError, ImageEditRequest, ImageGenRequest, ImageGenResponse, InferenceBackend,
    InferenceRequest, InferenceResponse, ReplicateRequest, Result, VerbProgress, check_http_status,
};
use crate::plugin::context::PixelData;
use crate::plugin::descriptor::{BackendCapabilities, CostEstimate};
use crate::plugin::progress::{CostUpdate, LogLevel, VerbProgressEvent};

const BASE_URL: &str = "https://fal.run";
const QUEUE_BASE_URL: &str = "https://queue.fal.run";
/// fal Flux Kontext endpoint.
pub const FAL_FLUX_KONTEXT: &str = "fal-ai/flux-pro/kontext";
/// fal Flux LoRA endpoint used for Flux.1 dev plus LoRA-style generation.
pub const FAL_FLUX_LORA: &str = "fal-ai/flux-lora";
/// fal Recraft vectorization endpoint.
pub const FAL_RECRAFT_VECTORIZE: &str = "fal-ai/recraft/vectorize";
/// fal Real-ESRGAN endpoint.
pub const FAL_REAL_ESRGAN: &str = "fal-ai/real-esrgan";
/// fal Flux LoRA fast training endpoint.
pub const FAL_FLUX_LORA_FAST_TRAINING: &str = "fal-ai/flux-lora-fast-training";

/// Result of a fal LoRA training run.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FalLoraTrainingResult {
    /// URL to the trained LoRA weights.
    pub lora_url: String,
}

/// fal.ai backend adapter.
pub struct FalBackend {
    client: reqwest::Client,
    api_key: String,
    base_url: String,
    queue_base_url: String,
}

impl std::fmt::Debug for FalBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FalBackend")
            .field("base_url", &self.base_url)
            .field("queue_base_url", &self.queue_base_url)
            .field("api_key", &"[redacted]")
            .finish_non_exhaustive()
    }
}

impl FalBackend {
    /// Constructs an adapter with an explicit API key.
    #[must_use]
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(180))
                .build()
                .unwrap_or_default(),
            api_key: api_key.into(),
            base_url: BASE_URL.into(),
            queue_base_url: QUEUE_BASE_URL.into(),
        }
    }

    /// Constructs an adapter by reading the API key from the OS keychain.
    pub fn from_keychain() -> Result<Self> {
        let key = super::ApiKeyStore::get("fal")?;
        Ok(Self::new(key))
    }

    /// Overrides the API base URL for tests.
    #[must_use]
    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        let url = url.into();
        self.base_url = url.clone();
        self.queue_base_url = url;
        self
    }

    /// Submits a zipped image dataset to fal's Flux LoRA fast trainer.
    pub async fn train_lora_archive(
        &self,
        archive_zip: Vec<u8>,
        trigger_word: &str,
        is_style: bool,
        cancel: CancellationToken,
    ) -> Result<FalLoraTrainingResult> {
        let body = build_lora_training_body(&archive_zip, trigger_word, is_style);
        let http_req = self
            .client
            .post(format!("{}/{}", self.base_url, FAL_FLUX_LORA_FAST_TRAINING))
            .header("Authorization", format!("Key {}", self.api_key))
            .json(&body)
            .build()
            .map_err(BackendError::Network)?;
        let http_resp = select! {
            biased;
            () = cancel.cancelled() => return Err(BackendError::Cancelled),
            res = self.client.execute(http_req) => res.map_err(BackendError::Network)?,
        };
        let http_resp = check_http_status(http_resp).await?;
        let raw: FalTrainingResponse = http_resp.json().await.map_err(BackendError::Network)?;
        let lora_url = raw
            .diffusers_lora_file
            .or(raw.lora_file)
            .map(|file| file.url)
            .ok_or_else(|| {
                BackendError::InvalidResponse(
                    "fal training response did not include a LoRA file URL".into(),
                )
            })?;
        Ok(FalLoraTrainingResult { lora_url })
    }

    async fn call_image_endpoint(
        &self,
        endpoint: &str,
        input: serde_json::Value,
        progress: &VerbProgress,
        cancel: &CancellationToken,
    ) -> Result<ImageGenResponse> {
        if let Some(resp) = self
            .try_stream_image_endpoint(endpoint, input.clone(), progress, cancel)
            .await?
        {
            return Ok(resp);
        }
        if self.base_url == BASE_URL {
            return self
                .call_queue_image_endpoint(endpoint, input, progress, cancel)
                .await;
        }
        let http_req = self
            .client
            .post(format!("{}/{}", self.base_url, endpoint))
            .header("Authorization", format!("Key {}", self.api_key))
            .json(&input)
            .build()
            .map_err(BackendError::Network)?;
        let http_resp = select! {
            biased;
            () = cancel.cancelled() => return Err(BackendError::Cancelled),
            res = self.client.execute(http_req) => res.map_err(BackendError::Network)?,
        };
        let http_resp = check_http_status(http_resp).await?;
        let raw: serde_json::Value = http_resp.json().await.map_err(BackendError::Network)?;
        let images = decode_fal_images(&self.client, raw, cancel).await?;
        progress
            .send(VerbProgressEvent::Cost(CostUpdate {
                usd_cents: 0.0,
                tokens_input: None,
                tokens_output: None,
            }))
            .await;
        Ok(ImageGenResponse {
            images,
            model: endpoint.into(),
        })
    }

    async fn try_stream_image_endpoint(
        &self,
        endpoint: &str,
        input: serde_json::Value,
        progress: &VerbProgress,
        cancel: &CancellationToken,
    ) -> Result<Option<ImageGenResponse>> {
        let stream_url = format!("{}/{}/stream", self.base_url, endpoint);
        let http_req = self
            .client
            .post(stream_url)
            .header("Authorization", format!("Key {}", self.api_key))
            .header("Accept", "text/event-stream")
            .json(&input)
            .build()
            .map_err(BackendError::Network)?;
        let http_resp = select! {
            biased;
            () = cancel.cancelled() => return Err(BackendError::Cancelled),
            res = self.client.execute(http_req) => res.map_err(BackendError::Network)?,
        };
        let status = http_resp.status();
        if status == reqwest::StatusCode::NOT_FOUND
            || status == reqwest::StatusCode::METHOD_NOT_ALLOWED
        {
            return Ok(None);
        }
        let http_resp = check_http_status(http_resp).await?;
        let images = parse_fal_sse_images(&self.client, http_resp, progress, cancel).await?;
        progress
            .send(VerbProgressEvent::Cost(CostUpdate {
                usd_cents: 0.0,
                tokens_input: None,
                tokens_output: None,
            }))
            .await;
        Ok(Some(ImageGenResponse {
            images,
            model: endpoint.into(),
        }))
    }

    async fn call_queue_image_endpoint(
        &self,
        endpoint: &str,
        input: serde_json::Value,
        progress: &VerbProgress,
        cancel: &CancellationToken,
    ) -> Result<ImageGenResponse> {
        let submit_req = self
            .client
            .post(format!("{}/{}", self.queue_base_url, endpoint))
            .header("Authorization", format!("Key {}", self.api_key))
            .json(&input)
            .build()
            .map_err(BackendError::Network)?;
        let submit_resp = select! {
            biased;
            () = cancel.cancelled() => return Err(BackendError::Cancelled),
            res = self.client.execute(submit_req) => res.map_err(BackendError::Network)?,
        };
        let submit_resp = check_http_status(submit_resp).await?;
        let raw: FalQueueSubmitResponse =
            submit_resp.json().await.map_err(BackendError::Network)?;
        let request_id = raw.request_id;
        let status_url = raw.status_url.unwrap_or_else(|| {
            format!(
                "{}/{}/requests/{}/status/stream?logs=1",
                self.queue_base_url, endpoint, request_id
            )
        });
        let result_url = raw.response_url.unwrap_or_else(|| {
            format!(
                "{}/{}/requests/{}",
                self.queue_base_url, endpoint, request_id
            )
        });
        let cancel_url = raw.cancel_url.unwrap_or_else(|| {
            format!(
                "{}/{}/requests/{}",
                self.queue_base_url, endpoint, request_id
            )
        });

        let status_req = self
            .client
            .get(status_url)
            .header("Authorization", format!("Key {}", self.api_key))
            .header("Accept", "text/event-stream")
            .build()
            .map_err(BackendError::Network)?;
        let status_resp = select! {
            biased;
            () = cancel.cancelled() => {
                self.cancel_queue_request(&cancel_url).await;
                return Err(BackendError::Cancelled);
            },
            res = self.client.execute(status_req) => res.map_err(BackendError::Network)?,
        };
        let status_resp = check_http_status(status_resp).await?;
        wait_for_fal_queue_completion(status_resp, progress, cancel, || {
            let client = self.client.clone();
            let api_key = self.api_key.clone();
            let cancel_url = cancel_url.clone();
            async move {
                let _ = client
                    .delete(cancel_url)
                    .header("Authorization", format!("Key {}", api_key))
                    .send()
                    .await;
            }
        })
        .await?;

        let result_req = self
            .client
            .get(result_url)
            .header("Authorization", format!("Key {}", self.api_key))
            .build()
            .map_err(BackendError::Network)?;
        let result_resp = select! {
            biased;
            () = cancel.cancelled() => {
                self.cancel_queue_request(&cancel_url).await;
                return Err(BackendError::Cancelled);
            },
            res = self.client.execute(result_req) => res.map_err(BackendError::Network)?,
        };
        let result_resp = check_http_status(result_resp).await?;
        let raw: serde_json::Value = result_resp.json().await.map_err(BackendError::Network)?;
        let images = decode_fal_images(&self.client, raw, cancel).await?;
        progress
            .send(VerbProgressEvent::Cost(CostUpdate {
                usd_cents: 0.0,
                tokens_input: None,
                tokens_output: None,
            }))
            .await;
        Ok(ImageGenResponse {
            images,
            model: endpoint.into(),
        })
    }

    async fn cancel_queue_request(&self, cancel_url: &str) {
        let _ = self
            .client
            .delete(cancel_url)
            .header("Authorization", format!("Key {}", self.api_key))
            .send()
            .await;
    }
}

#[async_trait]
impl InferenceBackend for FalBackend {
    fn backend_id(&self) -> &'static str {
        "fal"
    }

    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities::IMAGE_GENERATION
            .union(BackendCapabilities::IMAGE_EDIT)
            .union(BackendCapabilities::IMAGE_INPAINT)
    }

    fn supports_streaming(&self) -> bool {
        true
    }

    fn estimate_cost(&self, _request: &InferenceRequest) -> CostEstimate {
        CostEstimate {
            typical_latency: Duration::from_secs(8),
            max_latency: Duration::from_secs(180),
            typical_usd_cents: 8.0,
            max_usd_cents: 30.0,
        }
    }

    async fn invoke(
        &self,
        request: InferenceRequest,
        progress: VerbProgress,
        cancel: CancellationToken,
    ) -> Result<InferenceResponse> {
        progress
            .send(VerbProgressEvent::Started {
                backend: Some("fal".into()),
            })
            .await;
        match request {
            InferenceRequest::ImageGeneration(req) => {
                let endpoint = req.model.as_deref().unwrap_or(FAL_FLUX_LORA);
                let body = build_fal_generation_body(&req);
                let resp = self
                    .call_image_endpoint(endpoint, body, &progress, &cancel)
                    .await?;
                Ok(InferenceResponse::Image(resp))
            }
            InferenceRequest::ImageEdit(req) | InferenceRequest::ImageInpaint(req) => {
                let endpoint = req.model.as_deref().unwrap_or(FAL_FLUX_KONTEXT);
                let body = build_fal_edit_body(&req);
                let resp = self
                    .call_image_endpoint(endpoint, body, &progress, &cancel)
                    .await?;
                Ok(InferenceResponse::Image(resp))
            }
            InferenceRequest::Replicate(req) => {
                let resp = self
                    .call_image_endpoint(&req.model, req.input, &progress, &cancel)
                    .await?;
                Ok(InferenceResponse::Image(resp))
            }
            _ => Err(BackendError::UnsupportedCapability),
        }
    }
}

impl crate::plugin::backend::InferenceBackend for FalBackend {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn id(&self) -> &'static str {
        "fal"
    }

    fn capabilities(&self) -> BackendCapabilities {
        <Self as InferenceBackend>::capabilities(self)
    }

    fn cost_estimate(&self, _required: BackendCapabilities) -> CostEstimate {
        CostEstimate {
            typical_latency: Duration::from_secs(8),
            max_latency: Duration::from_secs(180),
            typical_usd_cents: 8.0,
            max_usd_cents: 30.0,
        }
    }

    fn is_available(&self) -> bool {
        !self.api_key.is_empty()
    }
}

#[allow(clippy::disallowed_methods)]
fn build_fal_generation_body(req: &ImageGenRequest) -> serde_json::Value {
    let mut input = serde_json::json!({
        "prompt": req.prompt,
        "num_images": req.num_images,
        "sync_mode": true,
        "image_size": {
            "width": req.width,
            "height": req.height,
        },
    });
    if let Some(style) = req.style_image.as_deref() {
        input["image_url"] = serde_json::json!(data_uri(style, "image/png"));
    }
    if !req.reference_images.is_empty() {
        let refs = req
            .reference_images
            .iter()
            .map(|bytes| data_uri(bytes, "image/png"))
            .collect::<Vec<_>>();
        input["reference_image_urls"] = serde_json::json!(refs);
    }
    input
}

#[allow(clippy::disallowed_methods)]
fn build_fal_edit_body(req: &ImageEditRequest) -> serde_json::Value {
    let mut input = serde_json::json!({
        "prompt": req.prompt,
        "image_url": data_uri(&req.image, "image/png"),
        "num_images": req.num_images,
        "sync_mode": true,
    });
    if let Some(mask) = req.mask.as_deref() {
        input["mask_url"] = serde_json::json!(data_uri(mask, "image/png"));
    }
    if let Some(style) = req.style_image.as_deref() {
        input["reference_image_url"] = serde_json::json!(data_uri(style, "image/png"));
    }
    if !req.reference_images.is_empty() {
        let refs = req
            .reference_images
            .iter()
            .map(|bytes| data_uri(bytes, "image/png"))
            .collect::<Vec<_>>();
        input["reference_image_urls"] = serde_json::json!(refs);
    }
    input
}

#[allow(clippy::disallowed_methods)]
fn build_lora_training_body(
    archive_zip: &[u8],
    trigger_word: &str,
    is_style: bool,
) -> serde_json::Value {
    serde_json::json!({
        "images_data_url": format!(
            "data:application/zip;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(archive_zip)
        ),
        "trigger_word": trigger_word,
        "is_style": is_style,
        "create_masks": !is_style,
        "steps": 1000,
    })
}

fn data_uri(bytes: &[u8], mime: &str) -> String {
    format!(
        "data:{mime};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    )
}

async fn decode_fal_images(
    client: &reqwest::Client,
    raw: serde_json::Value,
    cancel: &CancellationToken,
) -> Result<Vec<Vec<u8>>> {
    if let Some(svg) = raw
        .get("svg")
        .and_then(serde_json::Value::as_str)
        .or_else(|| raw.get("data").and_then(serde_json::Value::as_str))
        .filter(|value| value.trim_start().starts_with('<'))
    {
        return Ok(vec![svg.as_bytes().to_vec()]);
    }

    let mut refs = Vec::new();
    collect_image_refs(&raw, &mut refs);
    let mut out = Vec::new();
    for image_ref in refs {
        if let Some(bytes) = decode_data_uri(&image_ref)? {
            out.push(bytes);
        } else {
            let req = client
                .get(&image_ref)
                .build()
                .map_err(BackendError::Network)?;
            let resp = select! {
                biased;
                () = cancel.cancelled() => return Err(BackendError::Cancelled),
                res = client.execute(req) => res.map_err(BackendError::Network)?,
            };
            let resp = check_http_status(resp).await?;
            out.push(resp.bytes().await.map_err(BackendError::Network)?.to_vec());
        }
    }
    if out.is_empty() {
        return Err(BackendError::InvalidResponse(
            "fal response contained no image URL, data URI, or SVG".into(),
        ));
    }
    Ok(out)
}

async fn parse_fal_sse_images(
    client: &reqwest::Client,
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
            handle_fal_image_sse_frame(client, &frame, progress, cancel, &mut final_images).await?;
        }
    }
    if !buffer.trim().is_empty() {
        handle_fal_image_sse_frame(client, &buffer, progress, cancel, &mut final_images).await?;
    }
    if final_images.is_empty() {
        return Err(BackendError::InvalidResponse(
            "fal stream ended without a final image".into(),
        ));
    }
    Ok(final_images)
}

async fn handle_fal_image_sse_frame(
    client: &reqwest::Client,
    frame: &str,
    progress: &VerbProgress,
    cancel: &CancellationToken,
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
    if let Some(logs) = value.get("logs").and_then(serde_json::Value::as_array) {
        for log in logs {
            if let Some(message) = log
                .get("message")
                .and_then(serde_json::Value::as_str)
                .or_else(|| log.as_str())
            {
                progress
                    .send(VerbProgressEvent::Log {
                        level: LogLevel::Info,
                        message: message.to_owned(),
                    })
                    .await;
            }
        }
    }
    let status = value
        .get("status")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    if matches!(status, "IN_QUEUE" | "IN_PROGRESS") {
        progress
            .send(VerbProgressEvent::Step {
                fraction: None,
                message: status.to_owned(),
            })
            .await;
        return Ok(());
    }
    let images = decode_fal_images(client, value, cancel).await?;
    for (index, image) in images.iter().enumerate() {
        if let Some(pixels) = image_bytes_to_pixel_data(image) {
            progress
                .send(VerbProgressEvent::PartialPixels {
                    effect_index: u32::try_from(index).unwrap_or(u32::MAX),
                    pixels,
                })
                .await;
        }
    }
    final_images.extend(images);
    Ok(())
}

async fn wait_for_fal_queue_completion<F, Fut>(
    response: reqwest::Response,
    progress: &VerbProgress,
    cancel: &CancellationToken,
    cancel_request: F,
) -> Result<()>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = ()>,
{
    let mut cancel_request = Some(cancel_request);
    let mut stream = response.bytes_stream();
    let mut buffer = String::new();
    while let Some(chunk) = select! {
        biased;
        () = cancel.cancelled() => {
            if let Some(cancel_request) = cancel_request.take() {
                cancel_request().await;
            }
            return Err(BackendError::Cancelled);
        },
        next = stream.next() => next,
    } {
        let chunk = chunk.map_err(BackendError::Network)?;
        buffer.push_str(&String::from_utf8_lossy(&chunk));
        while let Some(index) = buffer.find("\n\n") {
            let frame = buffer[..index].to_owned();
            buffer.drain(..index + 2);
            if handle_fal_queue_status_frame(&frame, progress).await? {
                return Ok(());
            }
        }
    }
    if !buffer.trim().is_empty() && handle_fal_queue_status_frame(&buffer, progress).await? {
        return Ok(());
    }
    Err(BackendError::InvalidResponse(
        "fal queue status stream ended before completion".into(),
    ))
}

async fn handle_fal_queue_status_frame(frame: &str, progress: &VerbProgress) -> Result<bool> {
    let data = frame
        .lines()
        .filter_map(|line| line.strip_prefix("data:"))
        .map(str::trim)
        .collect::<Vec<_>>()
        .join("\n");
    if data.is_empty() || data == "[DONE]" {
        return Ok(false);
    }
    let value: serde_json::Value = serde_json::from_str(&data)?;
    let status = value
        .get("status")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("IN_PROGRESS");
    progress
        .send(VerbProgressEvent::Step {
            fraction: None,
            message: status.to_owned(),
        })
        .await;
    if let Some(logs) = value.get("logs").and_then(serde_json::Value::as_array) {
        for log in logs {
            if let Some(message) = log
                .get("message")
                .and_then(serde_json::Value::as_str)
                .or_else(|| log.as_str())
            {
                progress
                    .send(VerbProgressEvent::Log {
                        level: LogLevel::Info,
                        message: message.to_owned(),
                    })
                    .await;
            }
        }
    }
    match status {
        "COMPLETED" => Ok(true),
        "FAILED" | "ERROR" => Err(BackendError::Other(
            value
                .get("error")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("fal queue request failed")
                .to_owned(),
        )),
        _ => Ok(false),
    }
}

fn image_bytes_to_pixel_data(bytes: &[u8]) -> Option<PixelData> {
    let image = image::load_from_memory(bytes).ok()?.to_rgba8();
    let (width, height) = image.dimensions();
    Some(PixelData::rgba8(width, height, image.into_raw()))
}

fn collect_image_refs(value: &serde_json::Value, refs: &mut Vec<String>) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, value) in map {
                if matches!(key.as_str(), "url" | "image_url" | "content")
                    && let Some(s) = value.as_str()
                    && (s.starts_with("http://")
                        || s.starts_with("https://")
                        || s.starts_with("data:image/"))
                {
                    refs.push(s.to_owned());
                }
                collect_image_refs(value, refs);
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                collect_image_refs(value, refs);
            }
        }
        _ => {}
    }
}

fn decode_data_uri(uri: &str) -> Result<Option<Vec<u8>>> {
    let Some((_, data)) = uri.split_once(",") else {
        return Ok(None);
    };
    if !uri.starts_with("data:image/") {
        return Ok(None);
    }
    base64::engine::general_purpose::STANDARD
        .decode(data.as_bytes())
        .map(Some)
        .map_err(|err| BackendError::InvalidResponse(err.to_string()))
}

#[allow(dead_code)]
#[derive(Deserialize)]
struct FalQueueSubmitResponse {
    request_id: String,
    #[serde(default)]
    status_url: Option<String>,
    #[serde(default)]
    response_url: Option<String>,
    #[serde(default)]
    cancel_url: Option<String>,
}

#[derive(Deserialize)]
struct FalTrainingResponse {
    diffusers_lora_file: Option<FalFile>,
    lora_file: Option<FalFile>,
}

#[derive(Deserialize)]
struct FalFile {
    url: String,
}

// Preserve the raw request type in this module's public adapter surface.
fn _assert_raw_req(_: &ReplicateRequest) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edit_body_uses_data_uri() {
        let req = ImageEditRequest {
            model: None,
            image: b"img".to_vec(),
            mask: Some(b"mask".to_vec()),
            prompt: "edit".into(),
            negative_prompt: None,
            num_images: 1,
            style_image: None,
            reference_images: Vec::new(),
        };
        let body = build_fal_edit_body(&req);
        assert_eq!(body["prompt"], "edit");
        assert!(
            body["image_url"]
                .as_str()
                .unwrap_or_default()
                .starts_with("data:image/png;base64,")
        );
        assert!(
            body["mask_url"]
                .as_str()
                .unwrap_or_default()
                .contains("bWFzaw")
        );
    }

    #[test]
    fn collect_refs_finds_nested_urls() {
        let raw = serde_json::json!({ "images": [{ "url": "https://example.com/a.png" }] });
        let mut refs = Vec::new();
        collect_image_refs(&raw, &mut refs);
        assert_eq!(refs, vec!["https://example.com/a.png"]);
    }

    #[test]
    fn lora_training_body_uses_zip_data_uri() {
        let body = build_lora_training_body(b"zip", "wiztok", false);
        assert_eq!(body["trigger_word"], "wiztok");
        assert_eq!(body["create_masks"], true);
        assert!(
            body["images_data_url"]
                .as_str()
                .unwrap_or_default()
                .starts_with("data:application/zip;base64,")
        );
    }

    #[tokio::test]
    async fn queue_status_frame_reports_completion() {
        let (progress, mut rx) = VerbProgress::channel();
        let done = handle_fal_queue_status_frame(
            "data: {\"status\":\"COMPLETED\",\"logs\":[{\"message\":\"done\"}]}\n\n",
            &progress,
        )
        .await
        .unwrap();
        assert!(done);
        assert!(matches!(
            rx.recv().await,
            Some(VerbProgressEvent::Step { .. })
        ));
        assert!(matches!(
            rx.recv().await,
            Some(VerbProgressEvent::Log { .. })
        ));
    }

    #[tokio::test]
    async fn stream_frame_emits_partial_and_final_image() {
        let (progress, mut rx) = VerbProgress::channel();
        let png = one_pixel_png();
        let encoded = base64::engine::general_purpose::STANDARD.encode(&png);
        let data_uri = format!("data:image/png;base64,{encoded}");
        let raw = format!("data: {{\"images\":[{{\"url\":\"{data_uri}\"}}]}}\n\n");
        let cancel = CancellationToken::new();
        let client = reqwest::Client::new();
        let mut finals = Vec::new();
        handle_fal_image_sse_frame(&client, &raw, &progress, &cancel, &mut finals)
            .await
            .unwrap();
        assert_eq!(finals, vec![png]);
        assert!(matches!(
            rx.recv().await,
            Some(VerbProgressEvent::PartialPixels { .. })
        ));
    }

    fn one_pixel_png() -> Vec<u8> {
        let img = image::RgbaImage::from_pixel(1, 1, image::Rgba([0, 255, 0, 255]));
        let mut bytes = Vec::new();
        img.write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Png,
        )
        .unwrap();
        bytes
    }
}
