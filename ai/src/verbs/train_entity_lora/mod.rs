//! B10.5: Verb — Train per-entity `LoRA` from a Reference sheet.
//!
//! Extends the Project Style Learning pipeline (S30) so the same Replicate
//! `flux-dev-lora-trainer` flow can target a single Reference entity rather
//! than the project corpus. The trained safetensors becomes
//! `Entity.ai.lora_path`; anchor payloads built for that entity prefer it
//! over the project-wide `LoRA`, so generations against the anchor get
//! consistency by mechanism instead of by prompt engineering hope.
//!
//! # Flow
//!
//! 1. The host reads the Reference entity's canonical sheet (plus any
//!    archive variants the user opted into), decodes each PNG to a
//!    [`PixelData`], and passes the bundle in [`TrainEntityLoraInputs`].
//! 2. The verb encodes the images as PNGs and packs them into a zip
//!    archive submitted as a `data:` URI.
//! 3. Replicate trains the `LoRA`. The verb polls until completion (the
//!    same 15-30 min window as [`crate::verbs::ProjectStyleLearningVerb`])
//!    and returns the weights URL plus an [`EntityLoraResult`] inside a
//!    [`VerbEffect::Custom`].
//! 4. The host downloads the safetensors on commit, writes it under the
//!    project directory, and applies the second effect —
//!    [`VerbEffect::UpdateEntityAi`] — to set `Entity.ai.lora_path` and
//!    invalidate the anchor cache.
//!
//! # Backend requirement
//!
//! Requires [`BackendCapabilities::STYLE_TRAINING`]. The runtime selects
//! the first registered backend that advertises it (currently
//! [`ReplicateBackend`]) and injects it into `VerbContext::backend`.

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

use crate::verbs::project_style_learning::encode_training_zip;

/// Stable identifier for the train-entity-lora verb.
pub const TRAIN_ENTITY_LORA_VERB_ID: &str = "pixhaus.builtin.train_entity_lora";

/// Effect name namespacing the custom payload this verb produces.
pub const TRAIN_ENTITY_LORA_EFFECT_NAME: &str = "pixhaus.builtin.train_entity_lora.model";

/// Default Replicate model used for per-entity `LoRA` training.
///
/// Mirrors [`crate::verbs::project_style_learning`] — the two verbs share
/// the same backend trainer so trained outputs are byte-comparable when a
/// project trains both project-wide and per-entity weights.
const DEFAULT_TRAINING_MODEL: &str = "ostris/flux-dev-lora-trainer";

const DEFAULT_STEPS: u32 = 1000;
const DEFAULT_LORA_RANK: u32 = 16;

const RANK_MIN: u32 = 4;
const RANK_MAX: u32 = 32;

const STEPS_MIN: u32 = 200;
const STEPS_MAX: u32 = 2000;

// ── Input / output types ────────────────────────────────────────────────────

/// Inputs for [`TrainEntityLoraVerb`].
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TrainEntityLoraInputs {
    /// Target Reference entity. The trained `LoRA` lands on
    /// `Entity.ai.lora_path` of this entity.
    pub entity_id: EntityId,

    /// Training corpus. At least the canonical sheet image (one
    /// [`PixelData`]); the host may opt to include archive variants for
    /// richer fine-tuning. Every image must be well-formed per
    /// [`PixelData::is_well_formed`].
    ///
    /// Per-entity training typically wants 1-5 images — far fewer than
    /// the project corpus, because the entity's identity is concentrated
    /// in the canonical sheet rather than spread across many sprites.
    pub training_images: Vec<PixelData>,

    /// `LoRA` rank — dimensionality of the adapter matrices (4-32).
    /// Higher captures more detail at the cost of training time and
    /// model size. Default: 16.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lora_rank: Option<u32>,

    /// Training step count (200-2000). Default: 1000.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub steps: Option<u32>,

    /// Human-readable label and trigger word for the trained `LoRA`.
    ///
    /// Defaults to `entity-{id}` inside the verb when omitted. The host
    /// wrapper (`library_train_entity_lora` in `app/`) substitutes a slug
    /// derived from the entity name before invocation, so end-user
    /// trainings get a friendlier label; non-host callers (tests, direct
    /// invocation) get the id-based fallback.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,

    /// Replicate model to use for training. Defaults to
    /// `"ostris/flux-dev-lora-trainer"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

/// Payload returned in the [`VerbEffect::Custom`] this verb produces.
///
/// The host deserialises this on commit, downloads `weights_url` to the
/// project directory, then writes the resulting path to
/// `Entity.ai.lora_path` via the [`VerbEffect::UpdateEntityAi`] effect
/// the same `VerbOutput` carries.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct EntityLoraResult {
    /// Entity the weights were trained against.
    pub entity_id: EntityId,
    /// Download URL for the trained `LoRA` safetensors.
    pub weights_url: String,
    /// Replicate training job ID, for audit traces and re-fetches.
    pub training_id: String,
    /// Human-readable label / trigger word used during training.
    pub label: String,
    /// Replicate model used for training.
    pub training_model: String,
    /// Effective training steps.
    pub steps: u32,
    /// Effective `LoRA` rank.
    pub lora_rank: u32,
    /// Number of training images submitted.
    pub image_count: usize,
}

// ── Verb implementation ─────────────────────────────────────────────────────

/// Trains a `LoRA` against a Reference entity's canonical sheet and binds
/// the result to the entity via [`VerbEffect::UpdateEntityAi`].
#[derive(Debug)]
pub struct TrainEntityLoraVerb {
    descriptor: VerbDescriptor,
}

impl TrainEntityLoraVerb {
    /// Constructs the verb.
    #[must_use]
    // serde_json::json! calls .unwrap() internally on infallible builders;
    // the workspace disallows unwrap everywhere except here.
    #[allow(clippy::disallowed_methods)]
    pub fn new() -> Self {
        let input_schema = serde_json::json!({
            "type": "object",
            "properties": {
                "entity_id": {
                    "type": "integer",
                    "minimum": 0,
                    "description": "ID of the Reference entity to train against"
                },
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
                "model":  {"type": ["string", "null"]}
            },
            "required": ["entity_id", "training_images"]
        });

        Self {
            descriptor: VerbDescriptor {
                id: VerbId::new(TRAIN_ENTITY_LORA_VERB_ID),
                display_name: "Train Consistency LoRA".into(),
                description:
                    "Train a per-entity LoRA from a Reference sheet and bind the weights to the entity so anchor payloads carry them through to subsequent generations"
                        .into(),
                version: env!("CARGO_PKG_VERSION").into(),
                required_capabilities: BackendCapabilities::STYLE_TRAINING,
                input_schema,
                output_schema: None,
                output_kinds: vec![EffectKind::Custom(TRAIN_ENTITY_LORA_EFFECT_NAME.into())],
                cost_estimate: CostEstimate {
                    typical_latency: Duration::from_secs(900),
                    max_latency: Duration::from_secs(1800),
                    typical_usd_cents: 200.0,
                    max_usd_cents: 500.0,
                },
                streaming: true,
                cancellable: true,
                documentation_url: Some("docs/verbs/train-entity-lora.md".into()),
            },
        }
    }
}

impl Default for TrainEntityLoraVerb {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Verb for TrainEntityLoraVerb {
    fn descriptor(&self) -> &VerbDescriptor {
        &self.descriptor
    }

    fn validate(&self, inputs: &VerbInputs) -> Result<()> {
        let parsed: TrainEntityLoraInputs = inputs.deserialize()?;

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
        if let Some(rank) = parsed.lora_rank
            && !(RANK_MIN..=RANK_MAX).contains(&rank)
        {
            return Err(VerbError::Schema(format!(
                "lora_rank {rank} is out of range [{RANK_MIN}, {RANK_MAX}]"
            )));
        }
        if let Some(steps) = parsed.steps
            && !(STEPS_MIN..=STEPS_MAX).contains(&steps)
        {
            return Err(VerbError::Schema(format!(
                "steps {steps} is out of range [{STEPS_MIN}, {STEPS_MAX}]"
            )));
        }
        Ok(())
    }

    #[instrument(
        skip(self, ctx, inputs, progress, cancel),
        fields(verb = TRAIN_ENTITY_LORA_VERB_ID),
    )]
    async fn invoke(
        &self,
        ctx: VerbContext,
        inputs: VerbInputs,
        progress: VerbProgress,
        cancel: CancellationToken,
    ) -> Result<VerbOutput> {
        // B10.3 anchor: this verb intentionally ignores `ctx.anchor`.
        // It PRODUCES the per-entity LoRA that subsequent anchors carry;
        // reading the anchor here would be circular.
        let started = std::time::Instant::now();
        let mut inputs: TrainEntityLoraInputs = inputs.deserialize_owned()?;

        let label = inputs
            .label
            .clone()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| format!("entity-{}", inputs.entity_id.get()));
        let training_model = inputs
            .model
            .clone()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| DEFAULT_TRAINING_MODEL.into());
        let steps = inputs.steps.unwrap_or(DEFAULT_STEPS);
        let lora_rank = inputs.lora_rank.unwrap_or(DEFAULT_LORA_RANK);
        let image_count = inputs.training_images.len();

        let backend = ctx
            .backend
            .as_ref()
            .ok_or(VerbError::MissingContext("style training backend"))?;

        let replicate = backend
            .as_any()
            .downcast_ref::<ReplicateBackend>()
            .ok_or_else(|| {
                VerbError::Backend(
                    "train-entity-lora requires a Replicate backend with STYLE_TRAINING capability"
                        .into(),
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

        // Zip + PNG encoding is CPU-bound — move it off the runtime.
        // Move (don't clone) the training images: they're multi-MB RGBA
        // buffers and `inputs.training_images` isn't read again after this.
        let images = std::mem::take(&mut inputs.training_images);
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
        let payload = EntityLoraResult {
            entity_id: inputs.entity_id,
            weights_url: result.weights_url,
            training_id: result.training_id,
            label: label.clone(),
            training_model,
            steps,
            lora_rank,
            image_count,
        };

        let custom_payload = serde_json::to_value(&payload)?;

        // Two effects: the custom payload the host downloads from, and
        // an UpdateEntityAi placeholder so the host invalidates the anchor
        // cache for the entity after writing the safetensors path. The
        // path itself is filled by the host after download; the verb
        // returns None here because the safetensors lives on Replicate's
        // CDN until the host fetches it.
        let effects = vec![
            VerbEffect::Custom {
                name: TRAIN_ENTITY_LORA_EFFECT_NAME.into(),
                payload: custom_payload,
            },
            VerbEffect::UpdateEntityAi {
                entity_id: inputs.entity_id,
                lora_path: None,
            },
        ];

        Ok(VerbOutput {
            summary: format!(
                "Trained consistency LoRA '{label}' for entity {} ({image_count} images, {steps} steps)",
                inputs.entity_id.get()
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
                "Commit this preview to bind the trained LoRA to the entity.".into(),
                "Anchor payloads for this entity will carry the weights through to subsequent verb invocations.".into(),
            ],
        })
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::runtime::VerbRuntime;
    use pixhaus_core::project::{ProjectMetadata, SpriteId};

    fn metadata() -> ProjectMetadata {
        ProjectMetadata {
            name: "lora-test".into(),
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
        let v = TrainEntityLoraVerb::new();
        assert!(
            v.descriptor()
                .required_capabilities
                .contains(BackendCapabilities::STYLE_TRAINING)
        );
        assert!(v.descriptor().streaming);
        assert!(v.descriptor().cancellable);
    }

    #[test]
    fn descriptor_advertises_custom_effect_kind() {
        let v = TrainEntityLoraVerb::new();
        let kinds = &v.descriptor().output_kinds;
        assert_eq!(kinds.len(), 1);
        assert!(matches!(
            &kinds[0],
            EffectKind::Custom(name) if name == TRAIN_ENTITY_LORA_EFFECT_NAME,
        ));
    }

    #[test]
    fn validate_rejects_empty_training_images() {
        let v = TrainEntityLoraVerb::new();
        let inputs = VerbInputs::from_struct(&TrainEntityLoraInputs {
            entity_id: EntityId::new(7),
            training_images: vec![],
            lora_rank: None,
            steps: None,
            label: None,
            model: None,
        })
        .unwrap();
        let err = v.validate(&inputs).unwrap_err();
        assert!(matches!(err, VerbError::Schema(_)));
    }

    #[test]
    fn validate_rejects_malformed_image() {
        let v = TrainEntityLoraVerb::new();
        let bad = PixelData {
            width: 4,
            height: 4,
            bytes_per_pixel: 4,
            stride: 16,
            bytes: vec![0; 8], // too few bytes
        };
        let inputs = VerbInputs::from_struct(&TrainEntityLoraInputs {
            entity_id: EntityId::new(7),
            training_images: vec![bad],
            lora_rank: None,
            steps: None,
            label: None,
            model: None,
        })
        .unwrap();
        let err = v.validate(&inputs).unwrap_err();
        assert!(matches!(err, VerbError::Schema(_)));
    }

    #[test]
    fn validate_rejects_rank_out_of_range() {
        let v = TrainEntityLoraVerb::new();
        let inputs = VerbInputs::from_struct(&TrainEntityLoraInputs {
            entity_id: EntityId::new(7),
            training_images: vec![small_rgba_image()],
            lora_rank: Some(64),
            steps: None,
            label: None,
            model: None,
        })
        .unwrap();
        let err = v.validate(&inputs).unwrap_err();
        assert!(matches!(err, VerbError::Schema(_)));
    }

    #[test]
    fn validate_rejects_steps_out_of_range() {
        let v = TrainEntityLoraVerb::new();
        let inputs = VerbInputs::from_struct(&TrainEntityLoraInputs {
            entity_id: EntityId::new(7),
            training_images: vec![small_rgba_image()],
            lora_rank: None,
            steps: Some(100),
            label: None,
            model: None,
        })
        .unwrap();
        let err = v.validate(&inputs).unwrap_err();
        assert!(matches!(err, VerbError::Schema(_)));
    }

    #[test]
    fn validate_accepts_valid_inputs() {
        let v = TrainEntityLoraVerb::new();
        let inputs = VerbInputs::from_struct(&TrainEntityLoraInputs {
            entity_id: EntityId::new(7),
            training_images: vec![small_rgba_image()],
            lora_rank: Some(16),
            steps: Some(500),
            label: Some("Hero".into()),
            model: None,
        })
        .unwrap();
        v.validate(&inputs).unwrap();
    }

    #[test]
    fn entity_lora_result_round_trips_as_json() {
        let r = EntityLoraResult {
            entity_id: EntityId::new(42),
            weights_url: "https://replicate.delivery/abc/model.safetensors".into(),
            training_id: "tr-xyz".into(),
            label: "Hero".into(),
            training_model: "ostris/flux-dev-lora-trainer".into(),
            steps: 1000,
            lora_rank: 16,
            image_count: 1,
        };
        let json = serde_json::to_string(&r).unwrap();
        let back: EntityLoraResult = serde_json::from_str(&json).unwrap();
        assert_eq!(r, back);
    }

    #[test]
    fn verb_registers_with_runtime() {
        let v = TrainEntityLoraVerb::new();
        let ctx = ctx_with_sprite();
        let rt = VerbRuntime::new();
        rt.register(TrainEntityLoraVerb::new()).unwrap();
        assert!(
            rt.descriptor(&VerbId::new(TRAIN_ENTITY_LORA_VERB_ID))
                .is_some()
        );
        drop(v);
        drop(ctx);
    }

    /// Integration test — gated on `PIXHAUS_REPLICATE_API_KEY`.
    #[tokio::test]
    async fn integration_train_entity_lora() {
        use crate::backends::replicate::ReplicateBackend;
        use std::sync::Arc;

        let Some(api_key) = std::env::var("PIXHAUS_REPLICATE_API_KEY").ok() else {
            eprintln!("skipping: PIXHAUS_REPLICATE_API_KEY not set");
            return;
        };

        let img = PixelData::rgba8(8, 8, vec![128u8; 8 * 8 * 4]);
        let inputs = VerbInputs::from_struct(&TrainEntityLoraInputs {
            entity_id: EntityId::new(1),
            training_images: vec![img],
            lora_rank: Some(4),
            steps: Some(200),
            label: Some("integration-entity".into()),
            model: None,
        })
        .unwrap();

        let backend_arc: Arc<dyn crate::plugin::backend::InferenceBackend> =
            Arc::new(ReplicateBackend::new(api_key.clone()));
        let backend_registered = ReplicateBackend::new(api_key);

        let mut ctx = VerbContext::empty(metadata());
        ctx.active_sprite = Some(SpriteId::new(1));
        ctx.backend = Some(backend_arc);

        let rt = VerbRuntime::new();
        rt.register(TrainEntityLoraVerb::new()).unwrap();
        rt.register_backend(backend_registered, 10).unwrap();

        let inv = rt
            .invoke(&VerbId::new(TRAIN_ENTITY_LORA_VERB_ID), ctx, inputs)
            .unwrap();

        let preview = inv.finish().await.unwrap();
        assert_eq!(preview.output.effects.len(), 2);
        let custom = preview
            .output
            .effects
            .iter()
            .find_map(|e| match e {
                VerbEffect::Custom { name, payload } if name == TRAIN_ENTITY_LORA_EFFECT_NAME => {
                    Some(payload.clone())
                }
                _ => None,
            })
            .expect("custom effect");
        let result: EntityLoraResult = serde_json::from_value(custom).unwrap();
        assert_eq!(result.entity_id, EntityId::new(1));
        assert!(!result.weights_url.is_empty());
        assert!(!result.training_id.is_empty());

        assert!(preview.output.effects.iter().any(|e| matches!(
            e,
            VerbEffect::UpdateEntityAi { entity_id, lora_path: None }
                if *entity_id == EntityId::new(1),
        )));
    }
}
