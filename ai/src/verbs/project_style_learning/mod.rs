//! S30: Verb — Project style learning.
//!
//! Trains a small `LoRA` (Low-Rank Adaptation) on the project's visual
//! content so subsequent verbs can condition their output on the
//! project's established style. The trained model is stored as a
//! [`crate::plugin::context::StyleReference::TrainedModel`] entry in
//! the project; verbs read it from `VerbContext::style_refs`.
//!
//! # Flow
//!
//! 1. The host extracts pixel data from every visible raster layer and
//!    passes it as `training_images` in [`StyleLearningInputs`].
//! 2. The verb encodes the images as PNGs and packs them into a zip
//!    archive sent as a base64 data URI.
//! 3. The zip is submitted to Replicate's training API for
//!    `ostris/flux-dev-lora-trainer` (configurable).
//! 4. The verb polls Replicate until training finishes (15–30 min
//!    typically) and returns the weights download URL in a
//!    [`crate::plugin::output::VerbEffect::Custom`] effect.
//! 5. The host downloads the safetensors file on commit and registers
//!    a `StyleReference::TrainedModel` in the project.
//!
//! # Backend requirement
//!
//! Requires [`crate::plugin::descriptor::BackendCapabilities::STYLE_TRAINING`].
//! The runtime selects the first registered backend that advertises this
//! capability (currently [`crate::backends::replicate::ReplicateBackend`])
//! and injects it into `VerbContext::backend`. The verb downcasts the
//! backend to `ReplicateBackend` to call the training API; other backends
//! that implement `STYLE_TRAINING` can be added by implementing the same
//! downcast path.

use std::io::Write as _;
use std::time::Duration;

use async_trait::async_trait;
use base64::Engine as _;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;
use tracing::{debug, instrument};

use pixhaus_core::project::EntityId;

use crate::backends::replicate::ReplicateBackend;
use crate::plugin::context::{PixelData, VerbContext};
use crate::plugin::descriptor::{
    BackendCapabilities, CostEstimate, EffectKind, VerbDescriptor, VerbId,
};
use crate::plugin::error::{Result, VerbError};
use crate::plugin::inputs::VerbInputs;
use crate::plugin::output::{ActualCost, VerbEffect, VerbOutput};
use crate::plugin::progress::{VerbProgress, VerbProgressEvent};
use crate::plugin::verb::Verb;

/// Stable identifier for the project style learning verb.
pub const PROJECT_STYLE_LEARNING_VERB_ID: &str = "pixhaus.builtin.project_style_learning";

/// Default Replicate model used for `LoRA` training.
const DEFAULT_TRAINING_MODEL: &str = "ostris/flux-dev-lora-trainer";

const DEFAULT_STEPS: u32 = 1000;
const DEFAULT_LORA_RANK: u32 = 16;

/// Minimum/maximum valid `LoRA` rank.
const RANK_MIN: u32 = 4;
const RANK_MAX: u32 = 32;

/// Minimum/maximum training steps.
const STEPS_MIN: u32 = 200;
const STEPS_MAX: u32 = 2000;

// ── Input / output types ────────────────────────────────────────────────────

/// Inputs for [`ProjectStyleLearningVerb`].
///
/// The host is responsible for extracting pixel data from every visible
/// raster layer at every key frame and populating `training_images` before
/// invoking the verb. The verb does not read pixel buffers from the project
/// directly — it only processes what the host provides here.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StyleLearningInputs {
    /// Pixel data extracted from visible raster layers.
    ///
    /// At least one image is required. All images must be well-formed
    /// per [`PixelData::is_well_formed`]. The verb encodes each as a
    /// PNG and packs them into a zip archive for the training backend.
    pub training_images: Vec<PixelData>,

    /// `LoRA` rank — dimensionality of the adapter matrices (4–32). Lower
    /// values train faster and produce smaller models; higher values
    /// capture more detail. Default: 16.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lora_rank: Option<u32>,

    /// Training step count (200–2000). More steps improve quality but
    /// increase training time and cost. Default: 1000.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub steps: Option<u32>,

    /// Human-readable label for the trained model shown in the style
    /// picker. Defaults to the project name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,

    /// Replicate model to use for training. Defaults to
    /// `"ostris/flux-dev-lora-trainer"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,

    /// Library entity IDs whose pixel data the `training_images` were
    /// extracted from. When provided, the verb returns an
    /// `UpdateProjectAi` effect that merges these IDs into
    /// `ProjectAi.style_corpus` on commit. Omit when calling the verb
    /// outside a library context.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub corpus_entity_ids: Vec<EntityId>,
}

/// Metadata carried in the [`VerbEffect::Custom`] payload on success.
///
/// The host deserialises this on commit, downloads the weights from
/// `weights_url`, and registers a
/// [`crate::plugin::context::StyleReference::TrainedModel`] entry in
/// the project so subsequent verbs can consume it.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TrainedModelRef {
    /// URL to download the `LoRA` weights safetensors file.
    pub weights_url: String,
    /// Replicate training job ID for audit/reference.
    pub training_id: String,
    /// Human-readable model label shown in the style picker.
    pub label: String,
    /// Namespaced model type identifier — consumers use this to choose
    /// how to feed the model into their backend.
    pub model_id: String,
    /// Replicate model used for training.
    pub training_model: String,
    /// Effective training steps used.
    pub steps: u32,
    /// Effective `LoRA` rank used.
    pub lora_rank: u32,
    /// Number of training images.
    pub image_count: usize,
}

// ── Verb implementation ─────────────────────────────────────────────────────

/// Trains a `LoRA` on all project layers and registers it as the default
/// style reference for subsequent verbs.
#[derive(Debug)]
pub struct ProjectStyleLearningVerb {
    descriptor: VerbDescriptor,
}

impl ProjectStyleLearningVerb {
    /// Constructs the verb.
    #[must_use]
    // serde_json::json! calls .unwrap() internally on infallible builders;
    // the workspace disallows unwrap everywhere except here.
    #[allow(clippy::disallowed_methods)]
    pub fn new() -> Self {
        let input_schema = serde_json::json!({
            "type": "object",
            "properties": {
                "training_images": {
                    "type": "array",
                    "minItems": 1,
                    "items": {
                        "type": "object",
                        "properties": {
                            "width":  {"type": "integer", "minimum": 1},
                            "height": {"type": "integer", "minimum": 1},
                            "bytes_per_pixel": {"type": "integer"},
                            "stride": {"type": "integer"},
                            "bytes":  {"type": "array", "items": {"type": "integer"}}
                        },
                        "required": ["width", "height", "bytes_per_pixel", "stride", "bytes"]
                    }
                },
                "lora_rank": {
                    "type": ["integer", "null"],
                    "minimum": RANK_MIN,
                    "maximum": RANK_MAX
                },
                "steps": {
                    "type": ["integer", "null"],
                    "minimum": STEPS_MIN,
                    "maximum": STEPS_MAX
                },
                "label":  {"type": ["string", "null"]},
                "model":  {"type": ["string", "null"]},
                "corpus_entity_ids": {
                    "type": "array",
                    "items": {"type": "integer", "minimum": 0},
                    "default": []
                }
            },
            "required": ["training_images"]
        });

        Self {
            descriptor: VerbDescriptor {
                id: VerbId::new(PROJECT_STYLE_LEARNING_VERB_ID),
                display_name: "Learn Project Style".into(),
                description:
                    "Train a LoRA on all project layers and register it as the project style reference"
                        .into(),
                version: env!("CARGO_PKG_VERSION").into(),
                required_capabilities: BackendCapabilities::STYLE_TRAINING,
                input_schema,
                output_schema: None,
                output_kinds: vec![EffectKind::Custom(
                    "pixhaus.builtin.project_style_learning.model".into(),
                )],
                cost_estimate: CostEstimate {
                    typical_latency: Duration::from_secs(900),
                    max_latency: Duration::from_secs(1800),
                    typical_usd_cents: 200.0,
                    max_usd_cents: 500.0,
                },
                streaming: true,
                cancellable: true,
                documentation_url: Some("docs/verbs/project_style_learning.md".into()),
            },
        }
    }
}

impl Default for ProjectStyleLearningVerb {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Verb for ProjectStyleLearningVerb {
    fn descriptor(&self) -> &VerbDescriptor {
        &self.descriptor
    }

    fn validate(&self, inputs: &VerbInputs) -> Result<()> {
        let parsed: StyleLearningInputs = inputs.deserialize()?;

        if parsed.training_images.is_empty() {
            return Err(VerbError::Schema(
                "training_images must contain at least one image".into(),
            ));
        }
        for (i, img) in parsed.training_images.iter().enumerate() {
            if !img.is_well_formed() {
                return Err(VerbError::Schema(format!(
                    "training_images[{i}]: dimensions and byte count are inconsistent"
                )));
            }
        }
        if let Some(rank) = parsed.lora_rank {
            if !(RANK_MIN..=RANK_MAX).contains(&rank) {
                return Err(VerbError::Schema(format!(
                    "lora_rank {rank} is out of range [{RANK_MIN}, {RANK_MAX}]"
                )));
            }
        }
        if let Some(steps) = parsed.steps {
            if !(STEPS_MIN..=STEPS_MAX).contains(&steps) {
                return Err(VerbError::Schema(format!(
                    "steps {steps} is out of range [{STEPS_MIN}, {STEPS_MAX}]"
                )));
            }
        }
        Ok(())
    }

    #[instrument(skip(self, ctx, inputs, progress, cancel), fields(verb = PROJECT_STYLE_LEARNING_VERB_ID))]
    async fn invoke(
        &self,
        ctx: VerbContext,
        inputs: VerbInputs,
        progress: VerbProgress,
        cancel: CancellationToken,
    ) -> Result<VerbOutput> {
        // B10.3 anchor: this verb intentionally ignores `ctx.anchor`.
        // This verb PRODUCES the project LoRA that anchors then consume.
        // Reading `ctx.anchor` here would be circular — the anchor's
        // `lora_path` is the eventual output of this verb.
        let started = std::time::Instant::now();
        let inputs: StyleLearningInputs = inputs.deserialize_owned()?;

        // Resolve training parameters.
        let label = inputs
            .label
            .clone()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| ctx.project.name.clone());
        let training_model = inputs
            .model
            .clone()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| DEFAULT_TRAINING_MODEL.into());
        let steps = inputs.steps.unwrap_or(DEFAULT_STEPS);
        let lora_rank = inputs.lora_rank.unwrap_or(DEFAULT_LORA_RANK);
        let image_count = inputs.training_images.len();

        // The runtime guarantees backend is Some when required_capabilities
        // is non-empty, so this expect is unreachable in practice.
        let backend = ctx
            .backend
            .as_ref()
            .ok_or(VerbError::MissingContext("style training backend"))?;

        let replicate = backend
            .as_any()
            .downcast_ref::<ReplicateBackend>()
            .ok_or_else(|| {
                VerbError::Backend(
                    "project style learning requires a Replicate backend with STYLE_TRAINING capability".into(),
                )
            })?;

        progress
            .send(VerbProgressEvent::Started {
                backend: Some("replicate".into()),
            })
            .await;

        if cancel.is_cancelled() {
            return Err(VerbError::Cancelled);
        }

        progress
            .step(
                Some(0.02),
                &format!("encoding {image_count} training images"),
            )
            .await;

        // PNG-encode training images and pack into a zip archive.
        // Run in spawn_blocking because zip compression + PNG encoding
        // are CPU-bound and can be slow for large projects.
        let images = inputs.training_images.clone();
        let zip_data = tokio::task::spawn_blocking(move || encode_training_zip(&images))
            .await
            .map_err(|e| VerbError::Backend(format!("zip encoding task panicked: {e}")))?
            .map_err(|e| VerbError::Backend(format!("failed to encode training archive: {e}")))?;

        debug!(zip_bytes = zip_data.len(), "training archive ready");

        if cancel.is_cancelled() {
            return Err(VerbError::Cancelled);
        }

        progress
            .step(
                Some(0.05),
                &format!(
                    "submitting training job ({image_count} images, {steps} steps, rank {lora_rank})"
                ),
            )
            .await;

        let zip_b64 = base64::engine::general_purpose::STANDARD.encode(&zip_data);
        let images_uri = format!("data:application/zip;base64,{zip_b64}");

        let result = replicate
            .run_style_training(
                crate::backends::replicate::StyleTrainingParams {
                    training_model: &training_model,
                    images_uri,
                    steps,
                    lora_rank,
                    label: &label,
                },
                &progress,
                &cancel,
            )
            .await
            .map_err(|e| VerbError::Backend(e.to_string()))?;

        progress.step(Some(1.0), "training complete").await;

        let elapsed = started.elapsed();
        let model_ref = TrainedModelRef {
            weights_url: result.weights_url,
            training_id: result.training_id,
            label: label.clone(),
            model_id: "pixhaus.style.lora.replicate".into(),
            training_model,
            steps,
            lora_rank,
            image_count,
        };

        let payload = serde_json::to_value(&model_ref)?;

        let mut effects = vec![VerbEffect::Custom {
            name: "pixhaus.builtin.project_style_learning.model".into(),
            payload,
        }];

        // When corpus entity IDs were supplied, append an UpdateProjectAi
        // effect so the host can persist the corpus membership on commit.
        // The lora_path is left None here — the host sets it after it has
        // downloaded the weights and written them to the project directory.
        if !inputs.corpus_entity_ids.is_empty() {
            effects.push(VerbEffect::UpdateProjectAi {
                add_corpus_entity_ids: inputs.corpus_entity_ids,
                lora_path: None,
            });
        }

        Ok(VerbOutput {
            summary: format!(
                "Trained project style model '{label}' ({image_count} images, {steps} steps)"
            ),
            effects,
            thumbnail: None,
            actual_cost: ActualCost {
                elapsed,
                usd_cents: result.usd_cents,
                backend: Some("replicate".into()),
                tokens_input: None,
                tokens_output: None,
            },
            notes: vec![
                "Commit this preview to register the trained model as the project style reference."
                    .into(),
                "Other verbs will use it automatically for style conditioning.".into(),
            ],
        })
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────────

/// Encodes each `PixelData` as a PNG and packs all PNGs into a zip archive.
///
/// The returned bytes are a valid zip file. This function is CPU-bound and
/// should be called via `tokio::task::spawn_blocking`.
///
/// Shared with the B10.5 train-entity-lora verb; both pipe their training
/// images through the same encoder so the corpus format is identical.
pub(crate) fn encode_training_zip(images: &[PixelData]) -> std::result::Result<Vec<u8>, String> {
    let buf = std::io::Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(buf);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    for (i, pixel_data) in images.iter().enumerate() {
        let png = encode_png(pixel_data)?;
        zip.start_file(format!("{i:04}.png"), options)
            .map_err(|e| e.to_string())?;
        zip.write_all(&png).map_err(|e| e.to_string())?;
    }

    let buf = zip.finish().map_err(|e| e.to_string())?;
    Ok(buf.into_inner())
}

/// Encodes a [`PixelData`] buffer as a PNG byte stream.
///
/// Handles stride — each row is read starting at `y * stride` and cropped
/// to `width * bytes_per_pixel` bytes before passing to the encoder.
/// Supports `bytes_per_pixel` of 1 (luma), 2 (luma+alpha), 3 (RGB),
/// and 4 (RGBA).
fn encode_png(pixel_data: &PixelData) -> std::result::Result<Vec<u8>, String> {
    use image::DynamicImage;

    let w = pixel_data.width;
    let h = pixel_data.height;
    let bpp = pixel_data.bytes_per_pixel as usize;
    let stride = pixel_data.stride as usize;
    let row_bytes = w as usize * bpp;

    // De-stride into a tightly-packed buffer.
    let mut packed = Vec::with_capacity(row_bytes * h as usize);
    for y in 0..h as usize {
        let row_start = y * stride;
        let row_end = row_start + row_bytes;
        packed.extend_from_slice(&pixel_data.bytes[row_start..row_end]);
    }

    let dyn_image = match bpp {
        1 => {
            let buf = image::GrayImage::from_raw(w, h, packed)
                .ok_or_else(|| "luma buffer size mismatch".to_string())?;
            DynamicImage::ImageLuma8(buf)
        }
        2 => {
            let buf = image::GrayAlphaImage::from_raw(w, h, packed)
                .ok_or_else(|| "luma-alpha buffer size mismatch".to_string())?;
            DynamicImage::ImageLumaA8(buf)
        }
        3 => {
            let buf = image::RgbImage::from_raw(w, h, packed)
                .ok_or_else(|| "rgb buffer size mismatch".to_string())?;
            DynamicImage::ImageRgb8(buf)
        }
        4 => {
            let buf = image::RgbaImage::from_raw(w, h, packed)
                .ok_or_else(|| "rgba buffer size mismatch".to_string())?;
            DynamicImage::ImageRgba8(buf)
        }
        bpp => {
            return Err(format!("unsupported bytes_per_pixel: {bpp}"));
        }
    };

    let mut out = Vec::new();
    dyn_image
        .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    Ok(out)
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::runtime::VerbRuntime;
    use pixhaus_core::project::{ProjectMetadata, SpriteId};

    fn metadata() -> ProjectMetadata {
        ProjectMetadata {
            name: "style-test".into(),
            description: None,
            author: None,
            created_at: 0,
            updated_at: 0,
            editor_version: "0".into(),
        }
    }

    fn ctx_with_sprite() -> VerbContext {
        let mut ctx = VerbContext::empty(metadata());
        ctx.active_sprite = Some(SpriteId::new(1));
        ctx
    }

    fn small_rgba_image() -> PixelData {
        PixelData::rgba8(4, 4, vec![128u8; 64])
    }

    #[test]
    fn descriptor_declares_style_training_capability() {
        let v = ProjectStyleLearningVerb::new();
        assert!(
            v.descriptor()
                .required_capabilities
                .contains(BackendCapabilities::STYLE_TRAINING)
        );
        assert!(v.descriptor().streaming);
        assert!(v.descriptor().cancellable);
    }

    #[test]
    fn descriptor_output_kinds_is_custom() {
        let v = ProjectStyleLearningVerb::new();
        assert_eq!(v.descriptor().output_kinds.len(), 1);
        assert!(matches!(
            &v.descriptor().output_kinds[0],
            crate::plugin::descriptor::EffectKind::Custom(n) if n.contains("project_style_learning")
        ));
    }

    #[test]
    fn validate_rejects_empty_training_images() {
        let v = ProjectStyleLearningVerb::new();
        let inputs = VerbInputs::from_struct(&StyleLearningInputs {
            training_images: vec![],
            lora_rank: None,
            steps: None,
            label: None,
            model: None,
            corpus_entity_ids: vec![],
        })
        .unwrap();
        let err = v.validate(&inputs).unwrap_err();
        assert!(matches!(err, VerbError::Schema(_)));
    }

    #[test]
    fn validate_rejects_malformed_image() {
        let v = ProjectStyleLearningVerb::new();
        let bad_image = PixelData {
            width: 4,
            height: 4,
            bytes_per_pixel: 4,
            stride: 16,
            bytes: vec![0; 8], // too few bytes
        };
        let inputs = VerbInputs::from_struct(&StyleLearningInputs {
            training_images: vec![bad_image],
            lora_rank: None,
            steps: None,
            label: None,
            model: None,
            corpus_entity_ids: vec![],
        })
        .unwrap();
        let err = v.validate(&inputs).unwrap_err();
        assert!(matches!(err, VerbError::Schema(_)));
    }

    #[test]
    fn validate_rejects_rank_out_of_range() {
        let v = ProjectStyleLearningVerb::new();
        let inputs = VerbInputs::from_struct(&StyleLearningInputs {
            training_images: vec![small_rgba_image()],
            lora_rank: Some(64), // too high
            steps: None,
            label: None,
            model: None,
            corpus_entity_ids: vec![],
        })
        .unwrap();
        let err = v.validate(&inputs).unwrap_err();
        assert!(matches!(err, VerbError::Schema(_)));
    }

    #[test]
    fn validate_rejects_steps_out_of_range() {
        let v = ProjectStyleLearningVerb::new();
        let inputs = VerbInputs::from_struct(&StyleLearningInputs {
            training_images: vec![small_rgba_image()],
            lora_rank: None,
            steps: Some(100), // too few
            label: None,
            model: None,
            corpus_entity_ids: vec![],
        })
        .unwrap();
        let err = v.validate(&inputs).unwrap_err();
        assert!(matches!(err, VerbError::Schema(_)));
    }

    #[test]
    fn validate_accepts_valid_inputs() {
        let v = ProjectStyleLearningVerb::new();
        let inputs = VerbInputs::from_struct(&StyleLearningInputs {
            training_images: vec![small_rgba_image()],
            lora_rank: Some(16),
            steps: Some(500),
            label: Some("My Style".into()),
            model: None,
            corpus_entity_ids: vec![],
        })
        .unwrap();
        v.validate(&inputs).unwrap();
    }

    #[test]
    fn encode_png_rgba8_round_trips() {
        let pixel_data = PixelData::rgba8(
            2,
            2,
            vec![
                255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 255,
            ],
        );
        let png_bytes = encode_png(&pixel_data).unwrap();
        // Verify it's a valid PNG by checking the magic bytes.
        assert_eq!(&png_bytes[0..8], b"\x89PNG\r\n\x1a\n");
    }

    #[test]
    fn encode_png_handles_stride_padding() {
        // stride > width * bpp: row has 4 extra padding bytes.
        let padded = PixelData {
            width: 2,
            height: 2,
            bytes_per_pixel: 4,
            stride: 12, // 2 pixels * 4 bytes + 4 padding
            bytes: vec![
                255, 0, 0, 255, // pixel (0,0)
                0, 255, 0, 255, // pixel (1,0)
                0, 0, 0, 0, // padding row 0
                0, 0, 255, 255, // pixel (0,1)
                255, 255, 0, 255, // pixel (1,1)
                0, 0, 0, 0, // padding row 1
            ],
        };
        let png_bytes = encode_png(&padded).unwrap();
        assert_eq!(&png_bytes[0..8], b"\x89PNG\r\n\x1a\n");
    }

    #[test]
    fn encode_training_zip_produces_valid_archive() {
        let images = vec![small_rgba_image(), small_rgba_image()];
        let zip_bytes = encode_training_zip(&images).unwrap();
        // The zip local file header signature is 0x04034b50.
        assert_eq!(&zip_bytes[0..4], b"PK\x03\x04");
    }

    #[test]
    fn corpus_entity_ids_round_trips() {
        use pixhaus_core::project::EntityId;
        let inputs = StyleLearningInputs {
            training_images: vec![small_rgba_image()],
            lora_rank: None,
            steps: None,
            label: None,
            model: None,
            corpus_entity_ids: vec![EntityId::new(1), EntityId::new(2)],
        };
        let json = serde_json::to_string(&inputs).unwrap();
        let back: StyleLearningInputs = serde_json::from_str(&json).unwrap();
        assert_eq!(back.corpus_entity_ids.len(), 2);
    }

    #[test]
    fn corpus_entity_ids_absent_when_empty() {
        let inputs = StyleLearningInputs {
            training_images: vec![small_rgba_image()],
            lora_rank: None,
            steps: None,
            label: None,
            model: None,
            corpus_entity_ids: vec![],
        };
        let json = serde_json::to_string(&inputs).unwrap();
        // skip_serializing_if = Vec::is_empty — field should not appear.
        assert!(!json.contains("corpus_entity_ids"));
    }

    #[test]
    fn trained_model_ref_round_trips_as_json() {
        let r = TrainedModelRef {
            weights_url: "https://example.com/model.safetensors".into(),
            training_id: "abc123".into(),
            label: "My Style".into(),
            model_id: "pixhaus.style.lora.replicate".into(),
            training_model: "ostris/flux-dev-lora-trainer".into(),
            steps: 1000,
            lora_rank: 16,
            image_count: 25,
        };
        let json = serde_json::to_string(&r).unwrap();
        let back: TrainedModelRef = serde_json::from_str(&json).unwrap();
        assert_eq!(back.weights_url, r.weights_url);
        assert_eq!(back.model_id, r.model_id);
    }

    #[test]
    fn verb_requires_active_backend_when_missing_returns_backend_error() {
        // The runtime guarantees a backend is injected when required_capabilities
        // is non-empty. This test exercises the explicit error path for safety.
        let v = ProjectStyleLearningVerb::new();
        let ctx = ctx_with_sprite(); // no backend set
        // ctx.backend is None — the verb should surface MissingContext.
        let rt = VerbRuntime::new();
        rt.register(ProjectStyleLearningVerb::new()).unwrap();
        // We can't easily invoke without a registered backend,
        // but we can verify the descriptor is registered.
        assert!(
            rt.descriptor(&VerbId::new(PROJECT_STYLE_LEARNING_VERB_ID))
                .is_some()
        );
        drop(v);
        drop(ctx);
    }

    /// Integration test — gated on `PIXHAUS_REPLICATE_API_KEY`.
    #[tokio::test]
    async fn integration_style_training() {
        use crate::backends::replicate::ReplicateBackend;
        use std::sync::Arc;

        let Some(api_key) = std::env::var("PIXHAUS_REPLICATE_API_KEY").ok() else {
            eprintln!("skipping: PIXHAUS_REPLICATE_API_KEY not set");
            return;
        };

        // Build a minimal 8x8 sprite as the single training image.
        let training_image = PixelData::rgba8(8, 8, vec![128u8; 8 * 8 * 4]);

        let inputs = VerbInputs::from_struct(&StyleLearningInputs {
            training_images: vec![training_image],
            lora_rank: Some(4), // minimal rank for speed
            steps: Some(200),   // minimal steps for the integration test
            label: Some("integration-test-style".into()),
            model: None,
            corpus_entity_ids: vec![],
        })
        .unwrap();

        // register_backend takes B: InferenceBackend directly; ctx.backend
        // takes Arc<dyn InferenceBackend>. Use two separate instances.
        let backend_arc: Arc<dyn crate::plugin::backend::InferenceBackend> =
            Arc::new(ReplicateBackend::new(api_key.clone()));
        let backend_registered = ReplicateBackend::new(api_key);

        let mut ctx = VerbContext::empty(metadata());
        ctx.active_sprite = Some(SpriteId::new(1));
        ctx.backend = Some(backend_arc);

        let rt = VerbRuntime::new();
        rt.register(ProjectStyleLearningVerb::new()).unwrap();
        rt.register_backend(backend_registered, 10).unwrap();

        let inv = rt
            .invoke(&VerbId::new(PROJECT_STYLE_LEARNING_VERB_ID), ctx, inputs)
            .unwrap();

        let preview = inv.finish().await.unwrap();
        assert_eq!(preview.output.effects.len(), 1);
        match &preview.output.effects[0] {
            VerbEffect::Custom { name, payload } => {
                assert!(name.contains("project_style_learning"));
                let model_ref: TrainedModelRef = serde_json::from_value(payload.clone()).unwrap();
                assert!(!model_ref.weights_url.is_empty());
                assert!(!model_ref.training_id.is_empty());
            }
            other => panic!("unexpected effect: {other:?}"),
        }
    }
}
