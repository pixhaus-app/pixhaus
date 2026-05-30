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

use pixhaus_core::project::library::composition::{ArtStyleKind, PromptId, StructureId, StyleId};
use pixhaus_core::project::{EntityId, GenerationProvenance, ReferenceRole, SheetVariantId};

use crate::backends::{ImageGenRequest, ImageQuality, InferenceRequest, InferenceResponse};
use crate::compose::builtins::baseline_for;
use crate::compose::{ComposeRequest, VariableAxis, compose, identity_lock_clause};
use crate::plugin::context::VerbContext;
use crate::plugin::descriptor::{BackendCapabilities, CostEstimate, EffectKind, VerbDescriptor, VerbId};
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

    /// Ordered conditioning references (drag-in anchors). Each carries a role
    /// and weight; the verb routes a `Style`-role reference to the backend's
    /// `style_image` and the rest to `reference_images`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub references: Vec<ReferenceInput>,

    /// Hand-edited positive prompt. When `Some`, it is sent to the backend
    /// verbatim in place of the composed positive — the structure is still
    /// resolved for canvas size and panel geometry, so a hand-written prompt
    /// still slices into a correct sheet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_override: Option<String>,

    /// Hand-edited negative prompt. When `Some`, it replaces the composed
    /// negative verbatim.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub negative_override: Option<String>,

    /// Optional explicit output size `(width, height)`. Applies only to
    /// free-form (`Single`) output, which has no structure-declared canvas;
    /// Paneled structures keep their own canvas because the layout prose bakes
    /// in pixel dimensions. Lets the host request a high-resolution anchor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_size: Option<(u32, u32)>,
}

/// One conditioning reference handed to a generation request.
///
/// The image is a base64-encoded PNG so the inputs stay valid UTF-8 JSON, in
/// line with the rest of this verb's wire format. `role` and `weight` are kept
/// for provenance and for routing the reference to the right backend slot.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReferenceInput {
    /// Base64-encoded PNG bytes.
    pub image_b64: String,
    /// What this reference contributes (Subject / Style / Pose / …).
    #[serde(default)]
    pub role: ReferenceRole,
    /// Relative influence of this reference.
    #[serde(default = "default_reference_weight")]
    pub weight: f32,
}

fn default_reference_weight() -> f32 {
    1.0
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
    /// The resolved art-style kind plus an optional target grid, so the host's
    /// Land/finisher path can gate the pixel finisher on `kind.is_pixel()`
    /// (Brief 8). `#[serde(default)]` keeps pre-`finisher` payloads decodable.
    #[serde(default)]
    pub finisher: SheetFinisherSpec,
}

/// What the host needs to gate the post-generation finisher.
///
/// Minimal by design (Brief 1): the resolved [`ArtStyleKind`] plus an optional
/// `(cols, rows)` target grid hint. The finisher itself (palette-snap +
/// downscale) lands in Brief 8 and reads `kind.is_pixel()`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SheetFinisherSpec {
    /// The resolved art-style kind. Defaults to `PixelArt` when no style is
    /// picked, matching the cascading-baseline default.
    #[serde(default)]
    pub kind: ArtStyleKind,
    /// Optional `(cols, rows)` grid hint for a static grid-sheet path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_grid: Option<(u32, u32)>,
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
    /// The subject text the artist typed, kept distinct from the composed
    /// prompt so the gallery can show both.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub user_prompt: String,
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
            return Err(VerbError::Schema("generate-reference-sheet: structure_id must not be blank".into()));
        }
        Ok(())
    }

    // Verb invoke is a long state machine: progress events, cancellation
    // checkpoints, backend dispatch, typed response unpacking, and output
    // assembly. Refactoring would split the cancel/progress contract across
    // helpers and obscure the flow.
    #[allow(clippy::too_many_lines)]
    async fn invoke(&self, ctx: VerbContext, inputs: VerbInputs, progress: VerbProgress, cancel: CancellationToken) -> Result<VerbOutput> {
        let inputs: GenerateReferenceSheetInputs = inputs.deserialize_owned()?;
        let backend = crate::verbs::ctx_fat_backend(&ctx)?;
        let started = Instant::now();

        let num_variants = inputs.num_variants.clamp(1, 4);

        // Split the conditioning references up front: the result feeds both the
        // identity-lock decision below (a non-empty `reference_images` means an
        // anchor is in play) and the backend request later, so we decode once.
        let (style_image, reference_images) = decode_references(&inputs.references);
        // An anchor reference conditions the generation, so name what stays fixed
        // (silhouette, palette, face, costume, scale) and free only the pose.
        // A lone Style-role reference routes to `style_image`, not an anchor, so
        // it does not lock identity. The `prompt_override` path stays verbatim.
        let identity_hint = (!reference_images.is_empty()).then(|| identity_lock_clause(VariableAxis::Pose));

        // Resolve the picked records and run the composition resolver. The
        // resolved view borrows `ctx`; we extract owned values before the
        // backend await so nothing is held across the suspension point. The
        // style resolves first so the baseline can derive from its art-style
        // kind (a non-pixel style no longer prefixes "pixel art").
        let (composed_positive, composed_negative, composition, width, height, structure_name, resolved_kind) = {
            let library = ctx.library_view();
            let structure = library
                .structure(&inputs.structure_id)
                .ok_or_else(|| VerbError::Schema(format!("generate-reference-sheet: unknown structure `{}`", inputs.structure_id.0)))?;
            // A stale style/prompt id is an error, not a silent no-op — otherwise
            // the verb would generate a different composition than was picked.
            let style = match inputs.style_id.as_ref() {
                Some(id) => Some(
                    library
                        .style(id)
                        .ok_or_else(|| VerbError::Schema(format!("generate-reference-sheet: unknown style `{}`", id.0)))?,
                ),
                None => None,
            };
            // PixelArt by default when no style is picked, matching the cascading
            // baseline default and the host's finisher gate.
            let resolved_kind = style.map_or(ArtStyleKind::default(), |s| s.kind);
            // The project's style_notes are the prompt baseline when present, so
            // the house style folds into every generation; otherwise the
            // kind-specific built-in baseline.
            let baseline: &str = if ctx.composition_library.style_notes.trim().is_empty() {
                baseline_for(resolved_kind)
            } else {
                ctx.composition_library.style_notes.as_str()
            };
            let prompt = match inputs.prompt_id.as_ref() {
                Some(id) => Some(
                    library
                        .prompt(id)
                        .ok_or_else(|| VerbError::Schema(format!("generate-reference-sheet: unknown prompt `{}`", id.0)))?,
                ),
                None => None,
            };
            let entity_info: BTreeMap<String, String> = BTreeMap::new();
            let req = ComposeRequest {
                baseline,
                structure,
                style,
                prompt,
                variable_values: &inputs.variable_values,
                entity_info: &entity_info,
                inline_text: &inputs.inline_text,
                inline_negatives: &inputs.inline_negatives,
                operation_hint: identity_hint.as_deref(),
                context_fragments: &[],
            };
            let composed = compose(&req).map_err(|e| VerbError::Schema(e.to_string()))?;
            // Paneled output keeps its declared canvas (the prose bakes in pixel
            // dims); free-form output honours a host-requested size, else 1024².
            let (width, height) = composed
                .canvas
                .map_or_else(|| inputs.target_size.unwrap_or((1024, 1024)), |c| (c.width, c.height));
            (
                composed.positive,
                composed.negative,
                composed.composition,
                width,
                height,
                structure.name.clone(),
                resolved_kind,
            )
        };

        // A hand-edited prompt is sent verbatim; otherwise the composed text.
        // Either way the structure already fixed canvas size and panel geometry
        // above, so a hand-written prompt still slices into a correct sheet.
        let full_prompt = inputs.prompt_override.clone().filter(|s| !s.trim().is_empty()).unwrap_or(composed_positive);
        let negative = inputs.negative_override.clone().filter(|s| !s.trim().is_empty()).unwrap_or(composed_negative);

        let backend_id = backend.backend_id().to_owned();
        progress
            .send(VerbProgressEvent::Started {
                backend: Some(backend_id.clone()),
            })
            .await;

        if cancel.is_cancelled() {
            return Err(VerbError::Cancelled);
        }

        progress.step(Some(0.1), "sending to image generation backend").await;

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
            style_image,
            reference_images,
        };

        let response = select! {
            biased;
            () = cancel.cancelled() => return Err(VerbError::Cancelled),
            res = backend.invoke(
                InferenceRequest::ImageGeneration(req),
                // Forward the verb's progress channel so the backend's
                // streamed partial frames reach the UI for a live preview.
                progress.clone(),
                cancel.clone(),
            ) => res.map_err(|e| VerbError::Backend(e.to_string()))?,
        };

        if cancel.is_cancelled() {
            return Err(VerbError::Cancelled);
        }

        let (images, backend_model) = match response {
            InferenceResponse::Image(r) => (r.images, r.model),
            InferenceResponse::Text(_) => {
                return Err(VerbError::Backend("backend returned text for an image-generation request".into()));
            }
            InferenceResponse::Frames(_) | InferenceResponse::Video(_) => {
                return Err(VerbError::Backend("backend returned frames/video for an image-generation request".into()));
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
            return Err(VerbError::Backend("backend returned zero images for image generation request".into()));
        }

        progress.step(Some(0.9), "encoding variants").await;

        let variants = build_variants(
            unix_now(),
            images,
            &backend_id,
            &backend_model,
            &full_prompt,
            &inputs.inline_text,
            negative_opt.as_deref(),
            inputs.seed,
            &composition,
        );
        let variant_count = variants.len();
        let payload = GenerateSheetPayload {
            entity_id: inputs.entity_id,
            variants,
            finisher: SheetFinisherSpec {
                kind: resolved_kind,
                target_grid: None,
            },
        };
        let payload_json = serde_json::to_value(&payload).map_err(|e| VerbError::Backend(format!("failed to serialise sheet payload: {e}")))?;

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

/// Splits conditioning references into the backend's two slots: the first
/// `Style`-role reference becomes `style_image`, every other decodable
/// reference becomes a `reference_images` entry, preserving order. Undecodable
/// base64 is skipped rather than failing the whole generation.
fn decode_references(refs: &[ReferenceInput]) -> (Option<Vec<u8>>, Vec<Vec<u8>>) {
    let mut style_image = None;
    let mut reference_images = Vec::new();
    for r in refs {
        let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(&r.image_b64) else {
            continue;
        };
        if r.role == ReferenceRole::Style && style_image.is_none() {
            style_image = Some(bytes);
        } else {
            reference_images.push(bytes);
        }
    }
    (style_image, reference_images)
}

// i is always 0..=3 (num_variants clamped to 1..=4); the truncation cannot fire.
#[allow(clippy::cast_possible_truncation, clippy::too_many_arguments)]
fn build_variants(
    now: i64,
    images: Vec<Vec<u8>>,
    backend_id: &str,
    model: &str,
    prompt: &str,
    user_prompt: &str,
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
            user_prompt: user_prompt.to_owned(),
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
    use crate::backends::{BackendError, ImageGenResponse, InferenceBackend, InferenceRequest, InferenceResponse};
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
            img.write_to(&mut Cursor::new(&mut buf), ImageFormat::Png).expect("encode stub PNG");
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
    const SINGLE: &str = "pixhaus.builtin.structure.single";
    const CUSTOM: &str = "pixhaus.builtin.structure.custom";

    fn inputs_for(structure_id: &str, num_variants: u32) -> VerbInputs {
        VerbInputs::from_struct(&GenerateReferenceSheetInputs {
            entity_id: EntityId::new(42),
            structure_id: StructureId(structure_id.into()),
            style_id: None,
            prompt_id: None,
            variable_values: BTreeMap::new(),
            target_size: None,
            inline_text: "a fantasy hero with a sword".into(),
            inline_negatives: String::new(),
            num_variants,
            quality: None,
            seed: None,
            references: Vec::new(),
            prompt_override: None,
            negative_override: None,
        })
        .unwrap()
    }

    // ── Descriptor ────────────────────────────────────────────────────────────

    #[test]
    fn verb_id_matches_constant() {
        let verb = GenerateReferenceSheetVerb::new();
        assert_eq!(verb.descriptor().id, VerbId::new(GENERATE_REFERENCE_SHEET_VERB_ID));
    }

    #[test]
    fn verb_requires_image_generation_capability() {
        let verb = GenerateReferenceSheetVerb::new();
        assert!(
            verb.descriptor().required_capabilities.contains(BackendCapabilities::IMAGE_GENERATION),
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
            target_size: None,
            inline_text: String::new(),
            inline_negatives: String::new(),
            num_variants: 1,
            quality: None,
            seed: None,
            references: Vec::new(),
            prompt_override: None,
            negative_override: None,
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
        runtime.register_backend(BackendProxy::new(WhiteStub::new(4)), 0).unwrap();
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
                let decoded: GenerateSheetPayload = serde_json::from_value(payload.clone()).unwrap();
                assert_eq!(decoded.entity_id, EntityId::new(42));
                assert_eq!(decoded.variants.len(), 2);
            }
            other => panic!("expected Custom effect, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn variants_carry_composition_panels() {
        let runtime = VerbRuntime::new();
        runtime.register_backend(BackendProxy::new(WhiteStub::new(4)), 0).unwrap();
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
            assert_eq!(variant.composition.views.len(), 5, "character sheet must have 5 turnaround views");
            assert_eq!(variant.composition.expressions.len(), 3, "character sheet must have 3 expressions");
            assert!(variant.composition.palette_swatch.is_some(), "character sheet must have a palette swatch");
        }
    }

    #[tokio::test]
    async fn variants_carry_valid_base64_png() {
        let runtime = VerbRuntime::new();
        runtime.register_backend(BackendProxy::new(WhiteStub::new(4)), 0).unwrap();
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
            assert_eq!(&bytes[..8], &[137u8, 80, 78, 71, 13, 10, 26, 10], "decoded bytes must start with PNG signature");
        }
    }

    #[tokio::test]
    async fn num_variants_clamped_to_max_four() {
        let runtime = VerbRuntime::new();
        runtime.register_backend(BackendProxy::new(WhiteStub::new(4)), 0).unwrap();
        runtime.register(GenerateReferenceSheetVerb::new()).unwrap();

        // Request 10 — the verb should clamp to 4.
        let inputs = VerbInputs::from_struct(&GenerateReferenceSheetInputs {
            entity_id: EntityId::new(1),
            structure_id: StructureId(ITEM.into()),
            style_id: None,
            prompt_id: None,
            variable_values: BTreeMap::new(),
            target_size: None,
            inline_text: "treasure chest".into(),
            inline_negatives: String::new(),
            num_variants: 10,
            quality: None,
            seed: None,
            references: Vec::new(),
            prompt_override: None,
            negative_override: None,
        })
        .unwrap();

        let inv = runtime
            .invoke(&VerbId::new(GENERATE_REFERENCE_SHEET_VERB_ID), VerbContext::empty(meta()), inputs)
            .unwrap();
        let preview = inv.finish().await.unwrap();

        if let VerbEffect::Custom { payload, .. } = &preview.output.effects[0] {
            let decoded: GenerateSheetPayload = serde_json::from_value(payload.clone()).unwrap();
            assert!(decoded.variants.len() <= 4, "must not produce more than 4 variants");
        }
    }

    #[tokio::test]
    async fn cancellation_before_backend_returns_error() {
        let verb = GenerateReferenceSheetVerb::new();
        let cancel = CancellationToken::new();
        cancel.cancel();

        let result = verb
            .invoke(VerbContext::empty(meta()), inputs_for(CUSTOM, 1), VerbProgress::discard(), cancel)
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
            matches!(err, VerbError::UnsupportedCapability { .. } | VerbError::BackendUnavailable { .. }),
            "expected pre-flight failure when IMAGE_GENERATION backend is absent, got {err:?}"
        );
    }

    #[tokio::test]
    async fn generation_provenance_carries_backend_id() {
        let runtime = VerbRuntime::new();
        runtime.register_backend(BackendProxy::new(WhiteStub::new(4)), 0).unwrap();
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
        runtime.register_backend(BackendProxy::new(WhiteStub::new(4)), 0).unwrap();
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

            assert_eq!(preview.output.effects.len(), 1, "each template must produce exactly one effect");
            assert!(matches!(preview.output.effects[0], VerbEffect::Custom { .. }), "output must be a Custom effect");
        }
    }

    #[tokio::test]
    async fn empty_images_from_backend_returns_backend_error() {
        // WhiteStub::new(0) returns an empty image list: num_images=0 causes
        // count = req.num_images.max(1).min(0) = 0.
        let runtime = VerbRuntime::new();
        runtime.register_backend(BackendProxy::new(WhiteStub::new(0)), 0).unwrap();
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
        runtime.register_backend(BackendProxy::new(WhiteStub::new(1)), 0).unwrap();
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
            assert_eq!(prov.model, "stub.white", "model must be captured from backend response, not hardcoded");
        }
    }

    #[tokio::test]
    async fn generation_provenance_carries_negative_prompt() {
        let runtime = VerbRuntime::new();
        runtime.register_backend(BackendProxy::new(WhiteStub::new(1)), 0).unwrap();
        runtime.register(GenerateReferenceSheetVerb::new()).unwrap();

        let inputs = VerbInputs::from_struct(&GenerateReferenceSheetInputs {
            entity_id: EntityId::new(7),
            structure_id: StructureId(CHARACTER.into()),
            style_id: None,
            prompt_id: None,
            variable_values: BTreeMap::new(),
            target_size: None,
            inline_text: "test subject".into(),
            inline_negatives: "user_neg".into(),
            num_variants: 1,
            quality: None,
            seed: None,
            references: Vec::new(),
            prompt_override: None,
            negative_override: None,
        })
        .unwrap();

        let inv = runtime
            .invoke(&VerbId::new(GENERATE_REFERENCE_SHEET_VERB_ID), VerbContext::empty(meta()), inputs)
            .unwrap();
        let preview = inv.finish().await.unwrap();

        if let VerbEffect::Custom { payload, .. } = &preview.output.effects[0] {
            let decoded: GenerateSheetPayload = serde_json::from_value(payload.clone()).unwrap();
            let prov = &decoded.variants[0].generation;
            let neg = prov.negative_prompt.as_deref().expect("negative_prompt must be Some");
            assert!(neg.contains("user_neg"), "user negative prompt must appear in provenance: {neg}");
            assert!(!neg.starts_with(','), "negative prompt must not start with a bare comma");
        }
    }

    #[tokio::test]
    async fn unknown_style_id_is_rejected() {
        let runtime = VerbRuntime::new();
        runtime.register_backend(BackendProxy::new(WhiteStub::new(1)), 0).unwrap();
        runtime.register(GenerateReferenceSheetVerb::new()).unwrap();

        let inputs = VerbInputs::from_struct(&GenerateReferenceSheetInputs {
            entity_id: EntityId::new(1),
            structure_id: StructureId(CHARACTER.into()),
            style_id: Some(StyleId("does.not.exist".into())),
            prompt_id: None,
            variable_values: BTreeMap::new(),
            target_size: None,
            inline_text: "a hero".into(),
            inline_negatives: String::new(),
            num_variants: 1,
            quality: None,
            seed: None,
            references: Vec::new(),
            prompt_override: None,
            negative_override: None,
        })
        .unwrap();

        let inv = runtime
            .invoke(&VerbId::new(GENERATE_REFERENCE_SHEET_VERB_ID), VerbContext::empty(meta()), inputs)
            .unwrap();
        let err = inv.finish().await.unwrap_err();
        assert!(matches!(err, VerbError::Schema(_)), "an unknown style id must be rejected, got {err:?}");
    }

    // ── Cockpit contract: overrides, style_notes baseline, references ────────

    use std::sync::{Arc, Mutex};

    use pixhaus_core::project::ReferenceRole;

    use crate::backends::ImageGenRequest;
    use crate::plugin::context::ProjectCompositionLibrary;

    /// Backend that records the last image-gen request it received into a shared
    /// slot, so tests can assert what the verb actually sent. Returns one white PNG.
    #[derive(Debug)]
    struct CapturingStub {
        last: Arc<Mutex<Option<ImageGenRequest>>>,
    }

    #[async_trait]
    impl InferenceBackend for CapturingStub {
        fn backend_id(&self) -> &'static str {
            "stub.capture"
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
                    *self.last.lock().expect("lock") = Some(req);
                    Ok(InferenceResponse::Image(ImageGenResponse {
                        images: vec![WhiteStub::white_png()],
                        model: "stub.capture".into(),
                    }))
                }
                _ => Err(BackendError::UnsupportedCapability),
            }
        }
    }

    fn decode_payload(output: &crate::plugin::output::VerbOutput) -> GenerateSheetPayload {
        match &output.effects[0] {
            VerbEffect::Custom { payload, .. } => serde_json::from_value(payload.clone()).expect("decode payload"),
            other => panic!("expected Custom effect, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn prompt_override_is_sent_verbatim() {
        let runtime = VerbRuntime::new();
        runtime.register_backend(BackendProxy::new(WhiteStub::new(1)), 0).unwrap();
        runtime.register(GenerateReferenceSheetVerb::new()).unwrap();

        let inputs = VerbInputs::from_struct(&GenerateReferenceSheetInputs {
            entity_id: EntityId::new(1),
            structure_id: StructureId(CHARACTER.into()),
            style_id: None,
            prompt_id: None,
            variable_values: BTreeMap::new(),
            target_size: None,
            inline_text: "ignored when overridden".into(),
            inline_negatives: String::new(),
            num_variants: 1,
            quality: None,
            seed: None,
            references: Vec::new(),
            prompt_override: Some("EXACTLY THESE WORDS".into()),
            negative_override: None,
        })
        .unwrap();

        let inv = runtime
            .invoke(&VerbId::new(GENERATE_REFERENCE_SHEET_VERB_ID), VerbContext::empty(meta()), inputs)
            .unwrap();
        let payload = decode_payload(&inv.finish().await.unwrap().output);
        // The override is sent verbatim — no baseline/layout prose wraps it.
        assert_eq!(payload.variants[0].generation.prompt, "EXACTLY THESE WORDS");
        // The composition (panel geometry) still comes from the structure.
        assert_eq!(payload.variants[0].composition.views.len(), 5, "structure geometry survives an override");
    }

    #[tokio::test]
    async fn negative_override_is_sent_verbatim() {
        let runtime = VerbRuntime::new();
        runtime.register_backend(BackendProxy::new(WhiteStub::new(1)), 0).unwrap();
        runtime.register(GenerateReferenceSheetVerb::new()).unwrap();

        let inputs = VerbInputs::from_struct(&GenerateReferenceSheetInputs {
            entity_id: EntityId::new(1),
            structure_id: StructureId(CHARACTER.into()),
            style_id: None,
            prompt_id: None,
            variable_values: BTreeMap::new(),
            target_size: None,
            inline_text: "hero".into(),
            inline_negatives: "ignored".into(),
            num_variants: 1,
            quality: None,
            seed: None,
            references: Vec::new(),
            prompt_override: None,
            negative_override: Some("only this negative".into()),
        })
        .unwrap();

        let inv = runtime
            .invoke(&VerbId::new(GENERATE_REFERENCE_SHEET_VERB_ID), VerbContext::empty(meta()), inputs)
            .unwrap();
        let payload = decode_payload(&inv.finish().await.unwrap().output);
        assert_eq!(payload.variants[0].generation.negative_prompt.as_deref(), Some("only this negative"));
    }

    #[tokio::test]
    async fn style_notes_become_the_prompt_baseline() {
        let runtime = VerbRuntime::new();
        runtime.register_backend(BackendProxy::new(WhiteStub::new(1)), 0).unwrap();
        runtime.register(GenerateReferenceSheetVerb::new()).unwrap();

        let ctx = VerbContext::builder(meta())
            .with_composition_library(ProjectCompositionLibrary {
                style_notes: "HOUSE STYLE: muted gameboy palette".into(),
                ..Default::default()
            })
            .build();

        let inv = runtime
            .invoke(&VerbId::new(GENERATE_REFERENCE_SHEET_VERB_ID), ctx, inputs_for(CHARACTER, 1))
            .unwrap();
        let payload = decode_payload(&inv.finish().await.unwrap().output);
        assert!(
            payload.variants[0].generation.prompt.starts_with("HOUSE STYLE: muted gameboy palette"),
            "style_notes must lead the composed prompt: {}",
            payload.variants[0].generation.prompt
        );
    }

    #[tokio::test]
    async fn generated_prompt_carries_panel_discipline() {
        let runtime = VerbRuntime::new();
        runtime.register_backend(BackendProxy::new(WhiteStub::new(1)), 0).unwrap();
        runtime.register(GenerateReferenceSheetVerb::new()).unwrap();

        let inv = runtime
            .invoke(
                &VerbId::new(GENERATE_REFERENCE_SHEET_VERB_ID),
                VerbContext::empty(meta()),
                inputs_for(CHARACTER, 1),
            )
            .unwrap();
        let payload = decode_payload(&inv.finish().await.unwrap().output);
        let variant = &payload.variants[0];
        // Positive prompt carries the universal containment/scale-lock clause.
        assert!(
            variant.generation.prompt.contains("equal empty margin on all four sides"),
            "composed prompt must carry the containment clause: {}",
            variant.generation.prompt
        );
        // Negative carries the no-border rule even with no style picked.
        let neg = variant.generation.negative_prompt.as_deref().expect("negative_prompt must be Some");
        assert!(neg.contains("drawn cell borders"), "composed negative must forbid drawn cell borders: {neg}");
        assert!(!neg.starts_with(','), "negative prompt must not start with a bare comma: {neg}");
    }

    #[tokio::test]
    async fn pixel_art_style_gives_pixel_baseline() {
        use crate::compose::builtins::STYLE_PIXEL_ART_ID;

        let runtime = VerbRuntime::new();
        runtime.register_backend(BackendProxy::new(WhiteStub::new(1)), 0).unwrap();
        runtime.register(GenerateReferenceSheetVerb::new()).unwrap();

        // No style_notes, so the baseline derives from the picked style's kind.
        let inputs = VerbInputs::from_struct(&GenerateReferenceSheetInputs {
            entity_id: EntityId::new(1),
            structure_id: StructureId(CHARACTER.into()),
            style_id: Some(StyleId(STYLE_PIXEL_ART_ID.into())),
            prompt_id: None,
            variable_values: BTreeMap::new(),
            target_size: None,
            inline_text: "a hero".into(),
            inline_negatives: String::new(),
            num_variants: 1,
            quality: None,
            seed: None,
            references: Vec::new(),
            prompt_override: None,
            negative_override: None,
        })
        .unwrap();

        let inv = runtime
            .invoke(&VerbId::new(GENERATE_REFERENCE_SHEET_VERB_ID), VerbContext::empty(meta()), inputs)
            .unwrap();
        let payload = decode_payload(&inv.finish().await.unwrap().output);
        assert!(
            payload.variants[0].generation.prompt.starts_with("pixel art reference sheet"),
            "pixel-art style must lead with the pixel baseline: {}",
            payload.variants[0].generation.prompt
        );
    }

    #[tokio::test]
    async fn clean_hd_style_gives_non_pixel_baseline() {
        use crate::compose::builtins::STYLE_CLEAN_HD_ID;

        let runtime = VerbRuntime::new();
        runtime.register_backend(BackendProxy::new(WhiteStub::new(1)), 0).unwrap();
        runtime.register(GenerateReferenceSheetVerb::new()).unwrap();

        let inputs = VerbInputs::from_struct(&GenerateReferenceSheetInputs {
            entity_id: EntityId::new(1),
            structure_id: StructureId(CHARACTER.into()),
            style_id: Some(StyleId(STYLE_CLEAN_HD_ID.into())),
            prompt_id: None,
            variable_values: BTreeMap::new(),
            target_size: None,
            inline_text: "a hero".into(),
            inline_negatives: String::new(),
            num_variants: 1,
            quality: None,
            seed: None,
            references: Vec::new(),
            prompt_override: None,
            negative_override: None,
        })
        .unwrap();

        let inv = runtime
            .invoke(&VerbId::new(GENERATE_REFERENCE_SHEET_VERB_ID), VerbContext::empty(meta()), inputs)
            .unwrap();
        let payload = decode_payload(&inv.finish().await.unwrap().output);
        let prompt = &payload.variants[0].generation.prompt;
        assert!(!prompt.starts_with("pixel art"), "a clean-HD style must not prefix pixel art: {prompt}");
        assert!(
            prompt.starts_with("high-detail reference sheet"),
            "clean-HD style must lead with its baseline: {prompt}"
        );
    }

    #[tokio::test]
    async fn payload_finisher_carries_resolved_kind() {
        use crate::compose::builtins::STYLE_CLEAN_HD_ID;
        use pixhaus_core::project::library::composition::ArtStyleKind;

        let runtime = VerbRuntime::new();
        runtime.register_backend(BackendProxy::new(WhiteStub::new(1)), 0).unwrap();
        runtime.register(GenerateReferenceSheetVerb::new()).unwrap();

        let inputs = VerbInputs::from_struct(&GenerateReferenceSheetInputs {
            entity_id: EntityId::new(1),
            structure_id: StructureId(CHARACTER.into()),
            style_id: Some(StyleId(STYLE_CLEAN_HD_ID.into())),
            prompt_id: None,
            variable_values: BTreeMap::new(),
            target_size: None,
            inline_text: "a hero".into(),
            inline_negatives: String::new(),
            num_variants: 1,
            quality: None,
            seed: None,
            references: Vec::new(),
            prompt_override: None,
            negative_override: None,
        })
        .unwrap();

        let inv = runtime
            .invoke(&VerbId::new(GENERATE_REFERENCE_SHEET_VERB_ID), VerbContext::empty(meta()), inputs)
            .unwrap();
        let payload = decode_payload(&inv.finish().await.unwrap().output);
        assert_eq!(
            payload.finisher.kind,
            ArtStyleKind::CleanHd,
            "the resolved style kind rides on the payload finisher"
        );
        assert!(!payload.finisher.kind.is_pixel(), "clean-HD must not gate the pixel finisher");
    }

    #[tokio::test]
    async fn payload_finisher_defaults_to_pixel_art_without_a_style() {
        use pixhaus_core::project::library::composition::ArtStyleKind;

        let runtime = VerbRuntime::new();
        runtime.register_backend(BackendProxy::new(WhiteStub::new(1)), 0).unwrap();
        runtime.register(GenerateReferenceSheetVerb::new()).unwrap();

        let inv = runtime
            .invoke(
                &VerbId::new(GENERATE_REFERENCE_SHEET_VERB_ID),
                VerbContext::empty(meta()),
                inputs_for(CHARACTER, 1),
            )
            .unwrap();
        let payload = decode_payload(&inv.finish().await.unwrap().output);
        assert_eq!(payload.finisher.kind, ArtStyleKind::PixelArt, "no style resolves to the default pixel-art kind");
    }

    #[tokio::test]
    async fn references_route_to_style_image_and_reference_images() {
        let captured = Arc::new(Mutex::new(None));
        let stub = CapturingStub { last: captured.clone() };
        let runtime = VerbRuntime::new();
        runtime.register_backend(BackendProxy::new(stub), 0).unwrap();
        runtime.register(GenerateReferenceSheetVerb::new()).unwrap();

        let png_b64 = base64::engine::general_purpose::STANDARD.encode(WhiteStub::white_png());
        let inputs = VerbInputs::from_struct(&GenerateReferenceSheetInputs {
            entity_id: EntityId::new(1),
            structure_id: StructureId(CHARACTER.into()),
            style_id: None,
            prompt_id: None,
            variable_values: BTreeMap::new(),
            target_size: None,
            inline_text: "hero".into(),
            inline_negatives: String::new(),
            num_variants: 1,
            quality: None,
            seed: None,
            references: vec![
                ReferenceInput {
                    image_b64: png_b64.clone(),
                    role: ReferenceRole::Style,
                    weight: 1.0,
                },
                ReferenceInput {
                    image_b64: png_b64.clone(),
                    role: ReferenceRole::Subject,
                    weight: 1.0,
                },
            ],
            prompt_override: None,
            negative_override: None,
        })
        .unwrap();

        runtime
            .invoke(&VerbId::new(GENERATE_REFERENCE_SHEET_VERB_ID), VerbContext::empty(meta()), inputs)
            .unwrap()
            .finish()
            .await
            .unwrap();

        let captured = captured.lock().unwrap().clone().expect("backend was invoked");
        assert!(captured.style_image.is_some(), "the Style-role reference routes to style_image");
        assert_eq!(captured.reference_images.len(), 1, "the Subject reference routes to reference_images");
    }

    #[tokio::test]
    async fn anchor_reference_adds_identity_lock_to_prompt() {
        let captured = Arc::new(Mutex::new(None));
        let stub = CapturingStub { last: captured.clone() };
        let runtime = VerbRuntime::new();
        runtime.register_backend(BackendProxy::new(stub), 0).unwrap();
        runtime.register(GenerateReferenceSheetVerb::new()).unwrap();

        let png_b64 = base64::engine::general_purpose::STANDARD.encode(WhiteStub::white_png());
        let inputs = VerbInputs::from_struct(&GenerateReferenceSheetInputs {
            entity_id: EntityId::new(1),
            structure_id: StructureId(CHARACTER.into()),
            style_id: None,
            prompt_id: None,
            variable_values: BTreeMap::new(),
            target_size: None,
            inline_text: "hero".into(),
            inline_negatives: String::new(),
            num_variants: 1,
            quality: None,
            seed: None,
            // A Subject-role anchor routes to reference_images, so the verb adds
            // the identity-lock clause.
            references: vec![ReferenceInput {
                image_b64: png_b64,
                role: ReferenceRole::Subject,
                weight: 1.0,
            }],
            prompt_override: None,
            negative_override: None,
        })
        .unwrap();

        runtime
            .invoke(&VerbId::new(GENERATE_REFERENCE_SHEET_VERB_ID), VerbContext::empty(meta()), inputs)
            .unwrap()
            .finish()
            .await
            .unwrap();

        let captured = captured.lock().unwrap().clone().expect("backend was invoked");
        let prompt = &captured.prompt;
        assert!(
            prompt.contains("keep the same character from the reference image"),
            "an anchor reference must add the identity-lock clause: {prompt}"
        );
        assert!(
            prompt.contains("identical silhouette, palette, face, costume, and scale"),
            "the identity-lock clause must list the locked attributes: {prompt}"
        );
    }

    #[tokio::test]
    async fn no_reference_omits_identity_lock() {
        let captured = Arc::new(Mutex::new(None));
        let stub = CapturingStub { last: captured.clone() };
        let runtime = VerbRuntime::new();
        runtime.register_backend(BackendProxy::new(stub), 0).unwrap();
        runtime.register(GenerateReferenceSheetVerb::new()).unwrap();

        // A from-scratch sheet (no references) must not carry the lock clause.
        runtime
            .invoke(
                &VerbId::new(GENERATE_REFERENCE_SHEET_VERB_ID),
                VerbContext::empty(meta()),
                inputs_for(CHARACTER, 1),
            )
            .unwrap()
            .finish()
            .await
            .unwrap();

        let captured = captured.lock().unwrap().clone().expect("backend was invoked");
        assert!(
            !captured.prompt.contains("keep the same character from the reference image"),
            "a from-scratch sheet must not carry the identity-lock clause: {}",
            captured.prompt
        );
    }

    #[tokio::test]
    async fn style_only_reference_omits_identity_lock() {
        let captured = Arc::new(Mutex::new(None));
        let stub = CapturingStub { last: captured.clone() };
        let runtime = VerbRuntime::new();
        runtime.register_backend(BackendProxy::new(stub), 0).unwrap();
        runtime.register(GenerateReferenceSheetVerb::new()).unwrap();

        // A lone Style-role reference routes to style_image, not reference_images,
        // so it is not an anchor and must not trigger the identity lock.
        let png_b64 = base64::engine::general_purpose::STANDARD.encode(WhiteStub::white_png());
        let inputs = VerbInputs::from_struct(&GenerateReferenceSheetInputs {
            entity_id: EntityId::new(1),
            structure_id: StructureId(CHARACTER.into()),
            style_id: None,
            prompt_id: None,
            variable_values: BTreeMap::new(),
            target_size: None,
            inline_text: "hero".into(),
            inline_negatives: String::new(),
            num_variants: 1,
            quality: None,
            seed: None,
            references: vec![ReferenceInput {
                image_b64: png_b64,
                role: ReferenceRole::Style,
                weight: 1.0,
            }],
            prompt_override: None,
            negative_override: None,
        })
        .unwrap();

        runtime
            .invoke(&VerbId::new(GENERATE_REFERENCE_SHEET_VERB_ID), VerbContext::empty(meta()), inputs)
            .unwrap()
            .finish()
            .await
            .unwrap();

        let captured = captured.lock().unwrap().clone().expect("backend was invoked");
        assert!(
            !captured.prompt.contains("keep the same character from the reference image"),
            "a Style-only reference is not an anchor and must not lock identity: {}",
            captured.prompt
        );
    }

    #[tokio::test]
    async fn target_size_sets_free_form_output_dimensions() {
        let captured = Arc::new(Mutex::new(None));
        let runtime = VerbRuntime::new();
        runtime
            .register_backend(BackendProxy::new(CapturingStub { last: captured.clone() }), 0)
            .unwrap();
        runtime.register(GenerateReferenceSheetVerb::new()).unwrap();

        let inputs = VerbInputs::from_struct(&GenerateReferenceSheetInputs {
            entity_id: EntityId::new(1),
            structure_id: StructureId(SINGLE.into()),
            style_id: None,
            prompt_id: None,
            variable_values: BTreeMap::new(),
            target_size: Some((1536, 1536)),
            inline_text: "a hero".into(),
            inline_negatives: String::new(),
            num_variants: 1,
            quality: None,
            seed: None,
            references: Vec::new(),
            prompt_override: None,
            negative_override: None,
        })
        .unwrap();

        runtime
            .invoke(&VerbId::new(GENERATE_REFERENCE_SHEET_VERB_ID), VerbContext::empty(meta()), inputs)
            .unwrap()
            .finish()
            .await
            .unwrap();

        let captured = captured.lock().unwrap().clone().expect("backend was invoked");
        assert_eq!((captured.width, captured.height), (1536, 1536), "free-form output honours the target size");
    }

    #[tokio::test]
    async fn variants_carry_the_user_prompt_distinct_from_composed() {
        let runtime = VerbRuntime::new();
        runtime.register_backend(BackendProxy::new(WhiteStub::new(1)), 0).unwrap();
        runtime.register(GenerateReferenceSheetVerb::new()).unwrap();

        let inv = runtime
            .invoke(
                &VerbId::new(GENERATE_REFERENCE_SHEET_VERB_ID),
                VerbContext::empty(meta()),
                inputs_for(CHARACTER, 1),
            )
            .unwrap();
        let payload = decode_payload(&inv.finish().await.unwrap().output);
        assert_eq!(payload.variants[0].user_prompt, "a fantasy hero with a sword");
        assert_ne!(
            payload.variants[0].user_prompt, payload.variants[0].generation.prompt,
            "composed prompt wraps the user prompt with baseline/layout prose"
        );
    }

    // Ensure WhiteStub image and Duration are visible in this test scope.
    #[allow(dead_code)]
    fn _ensure_imports_visible() -> Duration {
        Duration::ZERO
    }
}
