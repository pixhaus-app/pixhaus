//! Verb: `GenerateReferenceSheet` — AI-generated character / item / tileset
//! reference sheets.
//!
//! Takes a target sprite entity plus a picked composition Structure (and
//! optional Style and saved Prompt), runs them through the `ai::compose`
//! resolver, and produces 1–4 `SheetVariant` candidates delivered to the
//! host as a `VerbEffect::Custom` payload. None of the candidates are
//! canonical until the user approves one.
//!
//! The layout, dimensions, panel slice geometry, and prompt prose all derive
//! from the resolved Structure (built-in or project-tier), so the verb itself
//! holds no hardcoded templates — see [`crate::compose`].
//!
//! Implements: docs/planning/work/b10-reference-sheets.md#b101

use std::collections::BTreeMap;
use std::time::Instant;

use async_trait::async_trait;
use base64::Engine as _;
use serde::{Deserialize, Serialize};
use tokio::select;
use tokio_util::sync::CancellationToken;

use pixhaus_core::project::library::composition::{PromptId, StructureId, StyleId};
use pixhaus_core::project::{EntityId, GenerationProvenance, SheetVariantId};

use crate::backends::{ImageGenRequest, ImageQuality, InferenceRequest, InferenceResponse};
use crate::compose::builtins::BUILTIN_DEFAULT_BASELINE;
use crate::compose::{ComposeRequest, compose};
use crate::plugin::context::VerbContext;
use crate::plugin::descriptor::{
    BackendCapabilities, CostEstimate, EffectKind, VerbDescriptor, VerbId,
};
use crate::plugin::error::{Result, VerbError};
use crate::plugin::inputs::VerbInputs;
use crate::plugin::output::{ActualCost, VerbEffect, VerbOutput};
use crate::plugin::progress::{VerbProgress, VerbProgressEvent};
use crate::plugin::verb::Verb;

/// Stable ID for the built-in generate-reference-sheet verb.
pub const GENERATE_REFERENCE_SHEET_VERB_ID: &str = "pixhaus.builtin.generate_reference_sheet";

/// Effect name used in the `VerbEffect::Custom` payload this verb produces.
///
/// Hosts that apply this effect deserialise [`GenerateSheetPayload`] from
/// the `payload` field and insert the returned variants into the target
/// entity's history.
pub const GENERATE_SHEET_EFFECT_NAME: &str = "pixhaus.builtin.generate_reference_sheet.sheets";

// ── Input types ───────────────────────────────────────────────────────────────

/// Inputs for [`GenerateReferenceSheetVerb`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GenerateReferenceSheetInputs {
    /// Target sprite entity in the project library. The host inserts the
    /// generated variants into this entity's embedded `ReferenceSheet::history`.
    pub entity_id: EntityId,

    /// Composition Structure id. Resolved against project records first, then
    /// the built-in registry; determines panel layout, dimensions, and the
    /// layout prose.
    pub structure_id: StructureId,

    /// Optional Style id applying reusable look modifiers and look negatives.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style_id: Option<StyleId>,

    /// Optional saved Prompt id whose text (with variables substituted) is
    /// composed into the request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_id: Option<PromptId>,

    /// Explicit values for the picked Prompt's `{variables}`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub variable_values: BTreeMap<String, String>,

    /// Free-typed prompt additions (replaces the old single `prompt` field
    /// for the inline case; a saved Prompt is referenced by `prompt_id`).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub inline_text: String,

    /// Free-typed negative additions, merged with the Structure and Style
    /// negatives at compose time.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub inline_negatives: String,

    /// Number of candidate sheets to generate. Clamped to `1..=4`.
    #[serde(default = "default_num_variants")]
    pub num_variants: u32,

    /// Optional quality hint for providers that support it.
    #[serde(default = "default_quality", skip_serializing_if = "Option::is_none")]
    pub quality: Option<ImageQuality>,

    /// RNG seed for reproducible generation. `None` uses a random seed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<u64>,
}

fn default_num_variants() -> u32 {
    1
}

#[allow(clippy::unnecessary_wraps)]
fn default_quality() -> Option<ImageQuality> {
    Some(ImageQuality::Auto)
}

// ── Output payload ────────────────────────────────────────────────────────────

/// The payload carried in the `VerbEffect::Custom` this verb produces.
///
/// Hosts handling `pixhaus.builtin.generate_reference_sheet.sheets`
/// deserialise this from the effect's `payload` field and convert each
/// [`SheetVariantOutput`] into a [`pixhaus_core::project::SheetVariant`]
/// added to the target entity's `history`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GenerateSheetPayload {
    /// Target sprite entity to receive the new variants.
    pub entity_id: EntityId,
    /// Generated sheet variants, one per requested candidate.
    pub variants: Vec<SheetVariantOutput>,
}

/// One generated sheet variant in a form ready for host storage.
///
/// Image bytes are base64-encoded PNG so the payload is valid UTF-8 JSON.
/// The host decodes `image_b64` back to bytes and wraps them in a
/// [`pixhaus_core::project::ReferenceImage`] before inserting the variant
/// into the entity's history.
///
/// `id` is a placeholder: the host assigns a real [`SheetVariantId`] from
/// the project's id-minting sequence on commit.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SheetVariantOutput {
    /// Placeholder variant id. The host replaces this with a real id.
    pub id: SheetVariantId,
    /// UTC seconds-since-epoch at which this variant was generated.
    pub generated_at: i64,
    /// Base64-encoded PNG sheet image.
    pub image_b64: String,
    /// Panel layout reflecting the template's expected composition.
    pub composition: pixhaus_core::project::SheetComposition,
    /// Generation provenance for reproducibility and audit.
    pub generation: GenerationProvenance,
}

// ── Verb implementation ───────────────────────────────────────────────────────

/// AI verb that generates reference sheet candidates for a library entity.
///
/// Construct with [`GenerateReferenceSheetVerb::new`] (no arguments). The
/// runtime selects an `IMAGE_GENERATION`-capable backend per invocation and
/// injects it into `VerbContext::backend`.
pub struct GenerateReferenceSheetVerb {
    descriptor: VerbDescriptor,
}

impl GenerateReferenceSheetVerb {
    /// Constructs the verb. The runtime injects the backend per invocation.
    #[must_use]
    #[allow(clippy::disallowed_methods)]
    pub fn new() -> Self {
        let input_schema = serde_json::json!({
            "type": "object",
            "properties": {
                "entity_id": {
                    "type": "integer",
                    "minimum": 0,
                    "description": "ID of the target sprite entity in the project library"
                },
                "structure_id": {
                    "type": "string",
                    "minLength": 1,
                    "description": "Composition Structure id (built-in or project-tier) that controls panel layout and dimensions"
                },
                "style_id": {
                    "type": ["string", "null"],
                    "description": "Optional Style id applying look modifiers and look negatives"
                },
                "prompt_id": {
                    "type": ["string", "null"],
                    "description": "Optional saved Prompt id whose text is composed into the request"
                },
                "variable_values": {
                    "type": "object",
                    "additionalProperties": { "type": "string" },
                    "description": "Values for the picked Prompt's {variables}"
                },
                "inline_text": {
                    "type": "string",
                    "description": "Free-typed prompt additions (subject description)"
                },
                "inline_negatives": {
                    "type": "string",
                    "description": "Free-typed negative additions"
                },
                "num_variants": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 4,
                    "default": 1,
                    "description": "Number of candidate sheets to generate"
                },
                "quality": {
                    "type": ["string", "null"],
                    "enum": ["auto", "low", "medium", "high", null],
                    "default": "auto",
                    "description": "Image quality for GPT image backends"
                },
                "seed": {
                    "type": ["integer", "null"],
                    "minimum": 0,
                    "description": "Optional RNG seed for reproducibility"
                }
            },
            "required": ["entity_id", "structure_id"]
        });

        Self {
            descriptor: VerbDescriptor {
                id: VerbId::new(GENERATE_REFERENCE_SHEET_VERB_ID),
                display_name: "Generate Reference Sheet".into(),
                description: "Generates 1–4 reference-sheet candidate images for a \
                              sprite library entity from a picked composition Structure \
                              (built-in or project-tier), with an optional Style and saved \
                              Prompt. Candidates land in the entity's embedded sheet history; \
                              the user approves one as canonical."
                    .into(),
                version: env!("CARGO_PKG_VERSION").into(),
                required_capabilities: BackendCapabilities::IMAGE_GENERATION,
                input_schema,
                output_schema: None,
                output_kinds: vec![EffectKind::Custom(GENERATE_SHEET_EFFECT_NAME.into())],
                cost_estimate: CostEstimate {
                    typical_latency: std::time::Duration::from_secs(45),
                    max_latency: std::time::Duration::from_secs(300),
                    typical_usd_cents: 2.0,
                    max_usd_cents: 20.0,
                },
                streaming: true,
                cancellable: true,
                documentation_url: Some("docs/verbs/generate-reference-sheet.md".into()),
            },
        }
    }
}

impl Default for GenerateReferenceSheetVerb {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for GenerateReferenceSheetVerb {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GenerateReferenceSheetVerb")
            .field("id", &self.descriptor.id)
            .finish_non_exhaustive()
    }
}

#[async_trait]
impl Verb for GenerateReferenceSheetVerb {
    fn descriptor(&self) -> &VerbDescriptor {
        &self.descriptor
    }

    fn validate(&self, inputs: &VerbInputs) -> Result<()> {
        let parsed: GenerateReferenceSheetInputs = inputs.deserialize()?;
        if parsed.structure_id.0.trim().is_empty() {
            return Err(VerbError::Schema(
                "generate-reference-sheet: structure_id must not be blank".into(),
            ));
        }
        Ok(())
    }

    // Verb invoke is a long state machine: progress events, cancellation
    // checkpoints, backend dispatch, typed response unpacking, and output
    // assembly. Refactoring would split the cancel/progress contract across
    // helpers and obscure the flow.
    #[allow(clippy::too_many_lines)]
    async fn invoke(
        &self,
        ctx: VerbContext,
        inputs: VerbInputs,
        progress: VerbProgress,
        cancel: CancellationToken,
    ) -> Result<VerbOutput> {
        let inputs: GenerateReferenceSheetInputs = inputs.deserialize_owned()?;
        let backend = crate::verbs::ctx_fat_backend(&ctx)?;
        let started = Instant::now();

        let num_variants = inputs.num_variants.clamp(1, 4);

        // Resolve the picked records and run the composition resolver. The
        // resolved view borrows `ctx`; we extract owned values before the
        // backend await so nothing is held across the suspension point.
        let (full_prompt, negative, composition, width, height, structure_name) = {
            let library = ctx.library_view();
            let structure = library.structure(&inputs.structure_id).ok_or_else(|| {
                VerbError::Schema(format!(
                    "generate-reference-sheet: unknown structure `{}`",
                    inputs.structure_id.0
                ))
            })?;
            // A stale style/prompt id is an error, not a silent no-op — otherwise
            // the verb would generate a different composition than was picked.
            let style = match inputs.style_id.as_ref() {
                Some(id) => Some(library.style(id).ok_or_else(|| {
                    VerbError::Schema(format!(
                        "generate-reference-sheet: unknown style `{}`",
                        id.0
                    ))
                })?),
                None => None,
            };
            let prompt = match inputs.prompt_id.as_ref() {
                Some(id) => Some(library.prompt(id).ok_or_else(|| {
                    VerbError::Schema(format!(
                        "generate-reference-sheet: unknown prompt `{}`",
                        id.0
                    ))
                })?),
                None => None,
            };
            let entity_info: BTreeMap<String, String> = BTreeMap::new();
            let req = ComposeRequest {
                baseline: BUILTIN_DEFAULT_BASELINE,
                structure,
                style,
                prompt,
                variable_values: &inputs.variable_values,
                entity_info: &entity_info,
                inline_text: &inputs.inline_text,
                inline_negatives: &inputs.inline_negatives,
                operation_hint: None,
                context_fragments: &[],
            };
            let composed = compose(&req).map_err(|e| VerbError::Schema(e.to_string()))?;
            let (width, height) = composed
                .canvas
                .map_or((1024, 1024), |c| (c.width, c.height));
            (
                composed.positive,
                composed.negative,
                composed.composition,
                width,
                height,
                structure.name.clone(),
            )
        };

        let backend_id = backend.backend_id().to_owned();
        progress
            .send(VerbProgressEvent::Started {
                backend: Some(backend_id.clone()),
            })
            .await;

        if cancel.is_cancelled() {
            return Err(VerbError::Cancelled);
        }

        progress
            .step(Some(0.1), "sending to image generation backend")
            .await;

        let negative_opt = (!negative.trim().is_empty()).then(|| negative.clone());
        let req = ImageGenRequest {
            model: None,
            prompt: full_prompt.clone(),
            negative_prompt: negative_opt.clone(),
            width,
            height,
            steps: None,
            seed: inputs.seed,
            num_images: num_variants,
            quality: inputs.quality,
            style_image: None,
            reference_images: Vec::new(),
        };

        let response = select! {
            biased;
            () = cancel.cancelled() => return Err(VerbError::Cancelled),
            res = backend.invoke(
                InferenceRequest::ImageGeneration(req),
                VerbProgress::discard(),
                cancel.clone(),
            ) => res.map_err(|e| VerbError::Backend(e.to_string()))?,
        };

        if cancel.is_cancelled() {
            return Err(VerbError::Cancelled);
        }

        let (images, backend_model) = match response {
            InferenceResponse::Image(r) => (r.images, r.model),
            InferenceResponse::Text(_) => {
                return Err(VerbError::Backend(
                    "backend returned text for an image-generation request".into(),
                ));
            }
            InferenceResponse::Frames(_) | InferenceResponse::Video(_) => {
                return Err(VerbError::Backend(
                    "backend returned frames/video for an image-generation request".into(),
                ));
            }
            InferenceResponse::Raw(_) => {
                return Err(VerbError::Backend(
                    "backend returned raw JSON for an image-generation request; \
                     use a typed image-gen adapter"
                        .into(),
                ));
            }
        };

        if images.is_empty() {
            return Err(VerbError::Backend(
                "backend returned zero images for image generation request".into(),
            ));
        }

        progress.step(Some(0.9), "encoding variants").await;

        let variants = build_variants(
            unix_now(),
            images,
            &backend_id,
            &backend_model,
            &full_prompt,
            negative_opt.as_deref(),
            inputs.seed,
            &composition,
        );
        let variant_count = variants.len();
        let payload = GenerateSheetPayload {
            entity_id: inputs.entity_id,
            variants,
        };
        let payload_json = serde_json::to_value(&payload)
            .map_err(|e| VerbError::Backend(format!("failed to serialise sheet payload: {e}")))?;

        let elapsed = started.elapsed();
        let summary = format!(
            "Generate {variant_count} reference sheet candidate{} for entity {} ({structure_name})",
            if variant_count == 1 { "" } else { "s" },
            inputs.entity_id.get(),
        );

        progress.step(Some(1.0), "done").await;

        Ok(VerbOutput {
            summary,
            effects: vec![VerbEffect::Custom {
                name: GENERATE_SHEET_EFFECT_NAME.into(),
                payload: payload_json,
            }],
            thumbnail: None,
            actual_cost: ActualCost {
                elapsed,
                usd_cents: 0.0,
                backend: Some(backend_id),
                tokens_input: None,
                tokens_output: None,
            },
            notes: vec![],
        })
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

// i is always 0..=3 (num_variants clamped to 1..=4); the truncation cannot fire.
#[allow(clippy::cast_possible_truncation, clippy::too_many_arguments)]
fn build_variants(
    now: i64,
    images: Vec<Vec<u8>>,
    backend_id: &str,
    model: &str,
    prompt: &str,
    negative_prompt: Option<&str>,
    seed: Option<u64>,
    composition: &pixhaus_core::project::SheetComposition,
) -> Vec<SheetVariantOutput> {
    images
        .into_iter()
        .enumerate()
        .map(|(i, png_bytes)| SheetVariantOutput {
            id: SheetVariantId::new(i as u32),
            generated_at: now,
            image_b64: base64::engine::general_purpose::STANDARD.encode(png_bytes),
            composition: composition.clone(),
            generation: GenerationProvenance {
                backend: backend_id.to_owned(),
                model: model.to_owned(),
                prompt: prompt.to_owned(),
                seed,
                negative_prompt: negative_prompt.map(ToOwned::to_owned),
            },
        })
        .collect()
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use async_trait::async_trait;
    use tokio_util::sync::CancellationToken;

    use pixhaus_core::project::ProjectMetadata;

    use crate::backends::bridge::BackendProxy;
    use crate::backends::{
        BackendError, ImageGenResponse, InferenceBackend, InferenceRequest, InferenceResponse,
    };
    use crate::plugin::context::VerbContext;
    use crate::plugin::descriptor::{BackendCapabilities, CostEstimate, VerbId};
    use crate::plugin::error::VerbError;
    use crate::plugin::inputs::VerbInputs;
    use crate::plugin::output::VerbEffect;
    use crate::plugin::progress::VerbProgress;
    use crate::plugin::runtime::VerbRuntime;
    use crate::plugin::verb::Verb;

    use super::*;

    fn meta() -> ProjectMetadata {
        ProjectMetadata {
            name: "sheet-test".into(),
            description: None,
            author: None,
            created_at: 0,
            updated_at: 0,
            editor_version: "0".into(),
        }
    }

    /// Stub backend: returns fixed 1×1 white PNG images for any
    /// `ImageGeneration` request.
    #[derive(Debug)]
    struct WhiteStub {
        /// How many images to return per request.
        num_images: u32,
    }

    impl WhiteStub {
        fn new(num_images: u32) -> Self {
            Self { num_images }
        }

        fn white_png() -> Vec<u8> {
            // Minimal 1×1 white RGBA PNG.
            use image::{ImageBuffer, ImageFormat, RgbaImage};
            use std::io::Cursor;
            let img: RgbaImage = ImageBuffer::from_pixel(1, 1, image::Rgba([255u8, 255, 255, 255]));
            let mut buf = Vec::new();
            img.write_to(&mut Cursor::new(&mut buf), ImageFormat::Png)
                .expect("encode stub PNG");
            buf
        }
    }

    #[async_trait]
    impl InferenceBackend for WhiteStub {
        fn backend_id(&self) -> &'static str {
            "stub.white"
        }

        fn capabilities(&self) -> BackendCapabilities {
            BackendCapabilities::IMAGE_GENERATION
        }

        fn supports_streaming(&self) -> bool {
            false
        }

        fn estimate_cost(&self, _req: &InferenceRequest) -> CostEstimate {
            CostEstimate::free()
        }

        async fn invoke(
            &self,
            request: InferenceRequest,
            _progress: VerbProgress,
            _cancel: CancellationToken,
        ) -> std::result::Result<InferenceResponse, BackendError> {
            match request {
                InferenceRequest::ImageGeneration(req) => {
                    let count = req.num_images.max(1).min(self.num_images);
                    let images = (0..count).map(|_| Self::white_png()).collect();
                    Ok(InferenceResponse::Image(ImageGenResponse {
                        images,
                        model: "stub.white".into(),
                    }))
                }
                _ => Err(BackendError::UnsupportedCapability),
            }
        }
    }

    const CHARACTER: &str = "pixhaus.builtin.structure.character";
    const ITEM: &str = "pixhaus.builtin.structure.item";
    const TILESET: &str = "pixhaus.builtin.structure.tileset";
    const CUSTOM: &str = "pixhaus.builtin.structure.custom";

    fn inputs_for(structure_id: &str, num_variants: u32) -> VerbInputs {
        VerbInputs::from_struct(&GenerateReferenceSheetInputs {
            entity_id: EntityId::new(42),
            structure_id: StructureId(structure_id.into()),
            style_id: None,
            prompt_id: None,
            variable_values: BTreeMap::new(),
            inline_text: "a fantasy hero with a sword".into(),
            inline_negatives: String::new(),
            num_variants,
            quality: None,
            seed: None,
        })
        .unwrap()
    }

    // ── Descriptor ────────────────────────────────────────────────────────────

    #[test]
    fn verb_id_matches_constant() {
        let verb = GenerateReferenceSheetVerb::new();
        assert_eq!(
            verb.descriptor().id,
            VerbId::new(GENERATE_REFERENCE_SHEET_VERB_ID)
        );
    }

    #[test]
    fn verb_requires_image_generation_capability() {
        let verb = GenerateReferenceSheetVerb::new();
        assert!(
            verb.descriptor()
                .required_capabilities
                .contains(BackendCapabilities::IMAGE_GENERATION),
            "verb must advertise IMAGE_GENERATION"
        );
    }

    #[test]
    fn verb_is_streaming_and_cancellable() {
        let verb = GenerateReferenceSheetVerb::new();
        assert!(verb.descriptor().streaming);
        assert!(verb.descriptor().cancellable);
    }

    #[test]
    fn output_kind_is_custom_sheet_effect() {
        let verb = GenerateReferenceSheetVerb::new();
        let kinds = &verb.descriptor().output_kinds;
        assert_eq!(kinds.len(), 1);
        match &kinds[0] {
            crate::plugin::descriptor::EffectKind::Custom(name) => {
                assert_eq!(name, GENERATE_SHEET_EFFECT_NAME);
            }
            other => panic!("expected Custom effect kind, got {other:?}"),
        }
    }

    // ── Validation ────────────────────────────────────────────────────────────

    #[test]
    fn validate_rejects_blank_structure_id() {
        let verb = GenerateReferenceSheetVerb::new();
        let inputs = VerbInputs::from_struct(&GenerateReferenceSheetInputs {
            entity_id: EntityId::new(1),
            structure_id: StructureId("   ".into()),
            style_id: None,
            prompt_id: None,
            variable_values: BTreeMap::new(),
            inline_text: String::new(),
            inline_negatives: String::new(),
            num_variants: 1,
            quality: None,
            seed: None,
        })
        .unwrap();
        assert!(matches!(verb.validate(&inputs), Err(VerbError::Schema(_))));
    }

    #[test]
    fn validate_accepts_well_formed_inputs() {
        let verb = GenerateReferenceSheetVerb::new();
        assert!(verb.validate(&inputs_for(CHARACTER, 2)).is_ok());
    }

    #[test]
    fn omitted_quality_defaults_to_auto() {
        let inputs: GenerateReferenceSheetInputs = serde_json::from_value(serde_json::json!({
            "entity_id": 1,
            "structure_id": "pixhaus.builtin.structure.character",
            "inline_text": "a fantasy hero"
        }))
        .unwrap();

        assert_eq!(inputs.quality, Some(ImageQuality::Auto));
        assert!(inputs.style_id.is_none());
        assert_eq!(inputs.inline_text, "a fantasy hero");
    }

    // ── Full invocation via runtime ───────────────────────────────────────────

    #[tokio::test]
    async fn invocation_produces_custom_effect_with_payload() {
        let runtime = VerbRuntime::new();
        runtime
            .register_backend(BackendProxy::new(WhiteStub::new(4)), 0)
            .unwrap();
        runtime.register(GenerateReferenceSheetVerb::new()).unwrap();

        let inv = runtime
            .invoke(
                &VerbId::new(GENERATE_REFERENCE_SHEET_VERB_ID),
                VerbContext::empty(meta()),
                inputs_for(CHARACTER, 2),
            )
            .unwrap();
        let preview = inv.finish().await.unwrap();

        assert_eq!(preview.verb.as_str(), GENERATE_REFERENCE_SHEET_VERB_ID);
        assert_eq!(preview.output.effects.len(), 1);

        match &preview.output.effects[0] {
            VerbEffect::Custom { name, payload } => {
                assert_eq!(name, GENERATE_SHEET_EFFECT_NAME);
                let decoded: GenerateSheetPayload =
                    serde_json::from_value(payload.clone()).unwrap();
                assert_eq!(decoded.entity_id, EntityId::new(42));
                assert_eq!(decoded.variants.len(), 2);
            }
            other => panic!("expected Custom effect, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn variants_carry_composition_panels() {
        let runtime = VerbRuntime::new();
        runtime
            .register_backend(BackendProxy::new(WhiteStub::new(4)), 0)
            .unwrap();
        runtime.register(GenerateReferenceSheetVerb::new()).unwrap();

        let inv = runtime
            .invoke(
                &VerbId::new(GENERATE_REFERENCE_SHEET_VERB_ID),
                VerbContext::empty(meta()),
                inputs_for(CHARACTER, 1),
            )
            .unwrap();
        let preview = inv.finish().await.unwrap();

        if let VerbEffect::Custom { payload, .. } = &preview.output.effects[0] {
            let decoded: GenerateSheetPayload = serde_json::from_value(payload.clone()).unwrap();
            let variant = &decoded.variants[0];
            assert_eq!(
                variant.composition.views.len(),
                5,
                "character sheet must have 5 turnaround views"
            );
            assert_eq!(
                variant.composition.expressions.len(),
                3,
                "character sheet must have 3 expressions"
            );
            assert!(
                variant.composition.palette_swatch.is_some(),
                "character sheet must have a palette swatch"
            );
        }
    }

    #[tokio::test]
    async fn variants_carry_valid_base64_png() {
        let runtime = VerbRuntime::new();
        runtime
            .register_backend(BackendProxy::new(WhiteStub::new(4)), 0)
            .unwrap();
        runtime.register(GenerateReferenceSheetVerb::new()).unwrap();

        let inv = runtime
            .invoke(
                &VerbId::new(GENERATE_REFERENCE_SHEET_VERB_ID),
                VerbContext::empty(meta()),
                inputs_for(CUSTOM, 1),
            )
            .unwrap();
        let preview = inv.finish().await.unwrap();

        if let VerbEffect::Custom { payload, .. } = &preview.output.effects[0] {
            let decoded: GenerateSheetPayload = serde_json::from_value(payload.clone()).unwrap();
            let variant = &decoded.variants[0];
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(&variant.image_b64)
                .expect("image_b64 must be valid base64");
            assert!(!bytes.is_empty(), "decoded image bytes must be non-empty");
            // Verify PNG signature (first 8 bytes).
            assert_eq!(
                &bytes[..8],
                &[137u8, 80, 78, 71, 13, 10, 26, 10],
                "decoded bytes must start with PNG signature"
            );
        }
    }

    #[tokio::test]
    async fn num_variants_clamped_to_max_four() {
        let runtime = VerbRuntime::new();
        runtime
            .register_backend(BackendProxy::new(WhiteStub::new(4)), 0)
            .unwrap();
        runtime.register(GenerateReferenceSheetVerb::new()).unwrap();

        // Request 10 — the verb should clamp to 4.
        let inputs = VerbInputs::from_struct(&GenerateReferenceSheetInputs {
            entity_id: EntityId::new(1),
            structure_id: StructureId(ITEM.into()),
            style_id: None,
            prompt_id: None,
            variable_values: BTreeMap::new(),
            inline_text: "treasure chest".into(),
            inline_negatives: String::new(),
            num_variants: 10,
            quality: None,
            seed: None,
        })
        .unwrap();

        let inv = runtime
            .invoke(
                &VerbId::new(GENERATE_REFERENCE_SHEET_VERB_ID),
                VerbContext::empty(meta()),
                inputs,
            )
            .unwrap();
        let preview = inv.finish().await.unwrap();

        if let VerbEffect::Custom { payload, .. } = &preview.output.effects[0] {
            let decoded: GenerateSheetPayload = serde_json::from_value(payload.clone()).unwrap();
            assert!(
                decoded.variants.len() <= 4,
                "must not produce more than 4 variants"
            );
        }
    }

    #[tokio::test]
    async fn cancellation_before_backend_returns_error() {
        let verb = GenerateReferenceSheetVerb::new();
        let cancel = CancellationToken::new();
        cancel.cancel();

        let result = verb
            .invoke(
                VerbContext::empty(meta()),
                inputs_for(CUSTOM, 1),
                VerbProgress::discard(),
                cancel,
            )
            .await;
        // Cancelled before the backend lookup because ctx has no backend,
        // so we expect either Cancelled or Backend error depending on order.
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn fails_without_image_generation_backend() {
        let runtime = VerbRuntime::new();
        // Register only a text backend — IMAGE_GENERATION is unsatisfied.
        runtime.register(GenerateReferenceSheetVerb::new()).unwrap();

        let err = runtime
            .invoke(
                &VerbId::new(GENERATE_REFERENCE_SHEET_VERB_ID),
                VerbContext::empty(meta()),
                inputs_for(CUSTOM, 1),
            )
            .unwrap_err();

        assert!(
            matches!(
                err,
                VerbError::UnsupportedCapability { .. } | VerbError::BackendUnavailable { .. }
            ),
            "expected pre-flight failure when IMAGE_GENERATION backend is absent, got {err:?}"
        );
    }

    #[tokio::test]
    async fn generation_provenance_carries_backend_id() {
        let runtime = VerbRuntime::new();
        runtime
            .register_backend(BackendProxy::new(WhiteStub::new(4)), 0)
            .unwrap();
        runtime.register(GenerateReferenceSheetVerb::new()).unwrap();

        let inv = runtime
            .invoke(
                &VerbId::new(GENERATE_REFERENCE_SHEET_VERB_ID),
                VerbContext::empty(meta()),
                inputs_for(TILESET, 1),
            )
            .unwrap();
        let preview = inv.finish().await.unwrap();

        if let VerbEffect::Custom { payload, .. } = &preview.output.effects[0] {
            let decoded: GenerateSheetPayload = serde_json::from_value(payload.clone()).unwrap();
            let prov = &decoded.variants[0].generation;
            assert_eq!(prov.backend, "stub.white");
            assert!(!prov.prompt.is_empty(), "prompt must be non-empty");
        }
    }

    #[tokio::test]
    async fn all_four_templates_produce_valid_output() {
        let runtime = VerbRuntime::new();
        runtime
            .register_backend(BackendProxy::new(WhiteStub::new(4)), 0)
            .unwrap();
        runtime.register(GenerateReferenceSheetVerb::new()).unwrap();

        for structure_id in [CHARACTER, ITEM, TILESET, CUSTOM] {
            let inv = runtime
                .invoke(
                    &VerbId::new(GENERATE_REFERENCE_SHEET_VERB_ID),
                    VerbContext::empty(meta()),
                    inputs_for(structure_id, 1),
                )
                .unwrap();
            let preview = inv.finish().await.unwrap();

            assert_eq!(
                preview.output.effects.len(),
                1,
                "each template must produce exactly one effect"
            );
            assert!(
                matches!(preview.output.effects[0], VerbEffect::Custom { .. }),
                "output must be a Custom effect"
            );
        }
    }

    #[tokio::test]
    async fn empty_images_from_backend_returns_backend_error() {
        // WhiteStub::new(0) returns an empty image list: num_images=0 causes
        // count = req.num_images.max(1).min(0) = 0.
        let runtime = VerbRuntime::new();
        runtime
            .register_backend(BackendProxy::new(WhiteStub::new(0)), 0)
            .unwrap();
        runtime.register(GenerateReferenceSheetVerb::new()).unwrap();

        let inv = runtime
            .invoke(
                &VerbId::new(GENERATE_REFERENCE_SHEET_VERB_ID),
                VerbContext::empty(meta()),
                inputs_for(CHARACTER, 1),
            )
            .unwrap();
        let err = inv.finish().await.unwrap_err();

        assert!(
            matches!(err, VerbError::Backend(_)),
            "expected Backend error when backend returns zero images, got {err:?}"
        );
    }

    #[tokio::test]
    async fn generation_provenance_carries_model_from_backend() {
        let runtime = VerbRuntime::new();
        runtime
            .register_backend(BackendProxy::new(WhiteStub::new(1)), 0)
            .unwrap();
        runtime.register(GenerateReferenceSheetVerb::new()).unwrap();

        let inv = runtime
            .invoke(
                &VerbId::new(GENERATE_REFERENCE_SHEET_VERB_ID),
                VerbContext::empty(meta()),
                inputs_for(CHARACTER, 1),
            )
            .unwrap();
        let preview = inv.finish().await.unwrap();

        if let VerbEffect::Custom { payload, .. } = &preview.output.effects[0] {
            let decoded: GenerateSheetPayload = serde_json::from_value(payload.clone()).unwrap();
            let prov = &decoded.variants[0].generation;
            assert_eq!(
                prov.model, "stub.white",
                "model must be captured from backend response, not hardcoded"
            );
        }
    }

    #[tokio::test]
    async fn generation_provenance_carries_negative_prompt() {
        let runtime = VerbRuntime::new();
        runtime
            .register_backend(BackendProxy::new(WhiteStub::new(1)), 0)
            .unwrap();
        runtime.register(GenerateReferenceSheetVerb::new()).unwrap();

        let inputs = VerbInputs::from_struct(&GenerateReferenceSheetInputs {
            entity_id: EntityId::new(7),
            structure_id: StructureId(CHARACTER.into()),
            style_id: None,
            prompt_id: None,
            variable_values: BTreeMap::new(),
            inline_text: "test subject".into(),
            inline_negatives: "user_neg".into(),
            num_variants: 1,
            quality: None,
            seed: None,
        })
        .unwrap();

        let inv = runtime
            .invoke(
                &VerbId::new(GENERATE_REFERENCE_SHEET_VERB_ID),
                VerbContext::empty(meta()),
                inputs,
            )
            .unwrap();
        let preview = inv.finish().await.unwrap();

        if let VerbEffect::Custom { payload, .. } = &preview.output.effects[0] {
            let decoded: GenerateSheetPayload = serde_json::from_value(payload.clone()).unwrap();
            let prov = &decoded.variants[0].generation;
            let neg = prov
                .negative_prompt
                .as_deref()
                .expect("negative_prompt must be Some");
            assert!(
                neg.contains("user_neg"),
                "user negative prompt must appear in provenance: {neg}"
            );
            assert!(
                !neg.starts_with(','),
                "negative prompt must not start with a bare comma"
            );
        }
    }

    #[tokio::test]
    async fn unknown_style_id_is_rejected() {
        let runtime = VerbRuntime::new();
        runtime
            .register_backend(BackendProxy::new(WhiteStub::new(1)), 0)
            .unwrap();
        runtime.register(GenerateReferenceSheetVerb::new()).unwrap();

        let inputs = VerbInputs::from_struct(&GenerateReferenceSheetInputs {
            entity_id: EntityId::new(1),
            structure_id: StructureId(CHARACTER.into()),
            style_id: Some(StyleId("does.not.exist".into())),
            prompt_id: None,
            variable_values: BTreeMap::new(),
            inline_text: "a hero".into(),
            inline_negatives: String::new(),
            num_variants: 1,
            quality: None,
            seed: None,
        })
        .unwrap();

        let inv = runtime
            .invoke(
                &VerbId::new(GENERATE_REFERENCE_SHEET_VERB_ID),
                VerbContext::empty(meta()),
                inputs,
            )
            .unwrap();
        let err = inv.finish().await.unwrap_err();
        assert!(
            matches!(err, VerbError::Schema(_)),
            "an unknown style id must be rejected, got {err:?}"
        );
    }

    // Ensure WhiteStub image and Duration are visible in this test scope.
    #[allow(dead_code)]
    fn _ensure_imports_visible() -> Duration {
        Duration::ZERO
    }
}
