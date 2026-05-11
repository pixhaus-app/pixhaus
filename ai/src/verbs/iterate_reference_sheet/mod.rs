//! Verb: `IterateReferenceSheet` — AI-driven inpainting pass on an existing
//! reference sheet variant.
//!
//! Takes an existing `SheetVariant` (supplied as a base64-encoded PNG by the
//! caller), an optional panel label to scope the edit to one panel rectangle,
//! and a refinement prompt. Produces a new `SheetVariantOutput` derived from
//! the source variant via an `IMAGE_INPAINT` backend call.
//!
//! Panel scoping keeps every pixel outside the target panel pixel-stable.
//! Without a panel label the backend edits the whole sheet based on the
//! instruction prompt alone.
//!
//! Implements: docs/planning/work/b10-reference-sheets.md#b102

use std::io::Cursor;
use std::time::Instant;

use async_trait::async_trait;
use base64::Engine as _;
use image::{GrayImage, ImageFormat};
use serde::{Deserialize, Serialize};
use tokio::select;
use tokio_util::sync::CancellationToken;

use pixhaus_core::project::{
    EntityId, GenerationProvenance, Rect, SheetComposition, SheetVariantId,
};

use crate::backends::{ImageEditRequest, InferenceRequest, InferenceResponse};
use crate::plugin::context::VerbContext;
use crate::plugin::descriptor::{
    BackendCapabilities, CostEstimate, EffectKind, VerbDescriptor, VerbId,
};
use crate::plugin::error::{Result, VerbError};
use crate::plugin::inputs::VerbInputs;
use crate::plugin::output::{ActualCost, VerbEffect, VerbOutput};
use crate::plugin::progress::{VerbProgress, VerbProgressEvent};
use crate::plugin::verb::Verb;

use crate::verbs::reference_sheet::SheetVariantOutput;

/// Stable ID for the built-in iterate-reference-sheet verb.
pub const ITERATE_REFERENCE_SHEET_VERB_ID: &str = "pixhaus.builtin.iterate_reference_sheet";

/// PNG file signature: the first 8 bytes of every well-formed PNG.
const PNG_MAGIC: &[u8] = b"\x89PNG\r\n\x1a\n";

/// Effect name used in the `VerbEffect::Custom` payload this verb produces.
///
/// Hosts handling this effect deserialise [`IterateSheetPayload`] from the
/// `payload` field and insert the returned variant into the target entity's
/// history.
pub const ITERATE_SHEET_EFFECT_NAME: &str = "pixhaus.builtin.iterate_reference_sheet.variant";

// ── Input types ───────────────────────────────────────────────────────────────

/// Inputs for [`IterateReferenceSheetVerb`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct IterateReferenceSheetInputs {
    /// Target Reference entity in the project library. The host inserts the
    /// returned variant into this entity's history.
    pub entity_id: EntityId,

    /// ID of the source variant being refined. Stored in provenance so the
    /// host can link the iteration chain; the source variant is not mutated.
    pub source_variant_id: SheetVariantId,

    /// Full sheet image as a base64-encoded PNG. The verb decodes this for
    /// the inpainting call and re-encodes the result.
    pub sheet_image_b64: String,

    /// Panel layout recorded when the source variant was generated. Used to
    /// resolve `panel_label` to a pixel rectangle for mask generation.
    pub composition: SheetComposition,

    /// Label of the panel to refine (e.g. `"happy"`, `"detail-1"`,
    /// `"side-left"`). Inpainting is scoped to that panel's rectangle;
    /// every pixel outside it stays pixel-stable. When `None`, the backend
    /// edits the whole sheet based on the prompt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub panel_label: Option<String>,

    /// Refinement instruction (e.g. `"make the hair longer"`, `"add a scar
    /// over the left eye"`). Sent as the inpainting prompt.
    pub prompt: String,

    /// Optional user-supplied negative prompt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub negative_prompt: Option<String>,
}

// ── Output payload ────────────────────────────────────────────────────────────

/// The payload carried in the `VerbEffect::Custom` this verb produces.
///
/// Hosts handling `pixhaus.builtin.iterate_reference_sheet.variant`
/// deserialise this from the effect's `payload` field and convert the
/// [`SheetVariantOutput`] into a [`pixhaus_core::project::SheetVariant`]
/// added to the target entity's history.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IterateSheetPayload {
    /// Target Reference entity to receive the new variant.
    pub entity_id: EntityId,
    /// ID of the source variant this was derived from. Enables the host to
    /// display the iteration lineage in the Sheet view history strip.
    pub source_variant_id: SheetVariantId,
    /// The new derived variant.
    pub variant: SheetVariantOutput,
}

// ── Verb implementation ───────────────────────────────────────────────────────

/// AI verb that produces a new reference sheet variant via inpainting.
///
/// Construct with [`IterateReferenceSheetVerb::new`] (no arguments). The
/// runtime selects an `IMAGE_INPAINT`-capable backend per invocation and
/// injects it into `VerbContext::backend`.
pub struct IterateReferenceSheetVerb {
    descriptor: VerbDescriptor,
}

impl IterateReferenceSheetVerb {
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
                    "description": "ID of the target Reference entity in the project library"
                },
                "source_variant_id": {
                    "type": "integer",
                    "minimum": 0,
                    "description": "ID of the sheet variant being refined"
                },
                "sheet_image_b64": {
                    "type": "string",
                    "description": "Base64-encoded PNG of the full sheet to iterate on"
                },
                "composition": {
                    "type": "object",
                    "description": "Panel layout from the source variant, used to resolve panel_label"
                },
                "panel_label": {
                    "type": ["string", "null"],
                    "description": "Panel to scope inpainting to (e.g. 'happy', 'detail-1'). Null edits the whole sheet"
                },
                "prompt": {
                    "type": "string",
                    "minLength": 1,
                    "description": "Refinement instruction describing the desired change"
                },
                "negative_prompt": {
                    "type": ["string", "null"],
                    "description": "Optional user-supplied negative prompt"
                }
            },
            "required": ["entity_id", "source_variant_id", "sheet_image_b64", "composition", "prompt"]
        });

        Self {
            descriptor: VerbDescriptor {
                id: VerbId::new(ITERATE_REFERENCE_SHEET_VERB_ID),
                display_name: "Iterate Reference Sheet".into(),
                description: "Produces a new reference sheet variant by running an inpainting pass \
                              on an existing variant. A panel label scopes the edit to one panel \
                              rectangle; every pixel outside it stays pixel-stable. Without a label \
                              the backend edits the whole sheet. The new variant lands in the \
                              entity's history alongside the source."
                    .into(),
                version: env!("CARGO_PKG_VERSION").into(),
                required_capabilities: BackendCapabilities::IMAGE_INPAINT,
                input_schema,
                output_schema: None,
                output_kinds: vec![EffectKind::Custom(ITERATE_SHEET_EFFECT_NAME.into())],
                cost_estimate: CostEstimate {
                    typical_latency: std::time::Duration::from_secs(20),
                    max_latency: std::time::Duration::from_secs(120),
                    typical_usd_cents: 1.0,
                    max_usd_cents: 8.0,
                },
                streaming: true,
                cancellable: true,
                documentation_url: Some("docs/verbs/iterate-reference-sheet.md".into()),
            },
        }
    }
}

impl Default for IterateReferenceSheetVerb {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for IterateReferenceSheetVerb {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IterateReferenceSheetVerb")
            .field("id", &self.descriptor.id)
            .finish_non_exhaustive()
    }
}

#[async_trait]
impl Verb for IterateReferenceSheetVerb {
    fn descriptor(&self) -> &VerbDescriptor {
        &self.descriptor
    }

    fn validate(&self, inputs: &VerbInputs) -> Result<()> {
        let parsed: IterateReferenceSheetInputs = inputs.deserialize()?;
        if parsed.prompt.trim().is_empty() {
            return Err(VerbError::Schema(
                "iterate-reference-sheet: prompt must not be blank".into(),
            ));
        }
        if parsed.sheet_image_b64.trim().is_empty() {
            return Err(VerbError::Schema(
                "iterate-reference-sheet: sheet_image_b64 must not be blank".into(),
            ));
        }
        Ok(())
    }

    // Long state machine: progress events, cancellation checkpoints, mask
    // generation, backend dispatch, and output assembly. Splitting would
    // obscure the cancel/progress flow.
    #[allow(clippy::too_many_lines)]
    async fn invoke(
        &self,
        ctx: VerbContext,
        inputs: VerbInputs,
        progress: VerbProgress,
        cancel: CancellationToken,
    ) -> Result<VerbOutput> {
        let inputs: IterateReferenceSheetInputs = inputs.deserialize_owned()?;
        let backend = crate::verbs::ctx_fat_backend(&ctx)?;
        let started = Instant::now();

        let backend_id = backend.backend_id().to_owned();
        progress
            .send(VerbProgressEvent::Started {
                backend: Some(backend_id.clone()),
            })
            .await;

        if cancel.is_cancelled() {
            return Err(VerbError::Cancelled);
        }

        // Decode the caller-supplied sheet image.
        progress
            .step(Some(0.1), "decoding source sheet image")
            .await;

        let sheet_bytes = base64::engine::general_purpose::STANDARD
            .decode(&inputs.sheet_image_b64)
            .map_err(|e| {
                VerbError::Schema(format!(
                    "iterate-reference-sheet: sheet_image_b64 is not valid base64: {e}"
                ))
            })?;

        if !sheet_bytes.starts_with(PNG_MAGIC) {
            return Err(VerbError::Schema(
                "iterate-reference-sheet: sheet_image_b64 is not a PNG (magic prefix mismatch)"
                    .into(),
            ));
        }

        if cancel.is_cancelled() {
            return Err(VerbError::Cancelled);
        }

        // Build the inpaint mask if a panel label was given.
        progress.step(Some(0.2), "building inpaint mask").await;

        let mask = match &inputs.panel_label {
            Some(label) => {
                let panel = *find_panel_rect(&inputs.composition, label).ok_or_else(|| {
                    VerbError::Schema(format!(
                        "iterate-reference-sheet: panel label {label:?} not found in composition"
                    ))
                })?;
                // PNG decode + per-pixel mask writes + PNG encode are CPU-bound;
                // self-schedule per docs/verb-protocol.md (Threading).
                let sheet_for_mask = sheet_bytes.clone();
                let mask_bytes =
                    tokio::task::spawn_blocking(move || build_panel_mask(&sheet_for_mask, &panel))
                        .await
                        .map_err(|e| VerbError::Aborted(e.to_string()))??;
                Some(mask_bytes)
            }
            None => None,
        };

        if cancel.is_cancelled() {
            return Err(VerbError::Cancelled);
        }

        let negative = inputs
            .negative_prompt
            .as_deref()
            .map(str::to_owned)
            .unwrap_or_default();

        let req = ImageEditRequest {
            model: None,
            image: sheet_bytes,
            mask,
            prompt: inputs.prompt.clone(),
            negative_prompt: if negative.is_empty() {
                None
            } else {
                Some(negative.clone())
            },
            num_images: 1,
        };

        progress
            .step(Some(0.3), "sending to inpainting backend")
            .await;

        let response = select! {
            biased;
            () = cancel.cancelled() => return Err(VerbError::Cancelled),
            res = backend.invoke(
                InferenceRequest::ImageInpaint(req),
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
                    "backend returned text for an inpainting request".into(),
                ));
            }
            InferenceResponse::Frames(_) => {
                return Err(VerbError::Backend(
                    "backend returned frames for an inpainting request".into(),
                ));
            }
            InferenceResponse::Raw(_) => {
                return Err(VerbError::Backend(
                    "backend returned raw JSON for an inpainting request; \
                     use a typed image-edit adapter"
                        .into(),
                ));
            }
        };

        let png_bytes = images.into_iter().next().ok_or_else(|| {
            VerbError::Backend("backend returned zero images for inpainting request".into())
        })?;

        progress.step(Some(0.9), "encoding variant").await;

        let panel_scope = inputs.panel_label.as_deref().unwrap_or("whole-sheet");
        let variant = SheetVariantOutput {
            id: SheetVariantId::new(0),
            generated_at: unix_now(),
            image_b64: base64::engine::general_purpose::STANDARD.encode(&png_bytes),
            composition: inputs.composition,
            generation: GenerationProvenance {
                backend: backend_id.clone(),
                model: backend_model,
                prompt: inputs.prompt.clone(),
                // `ImageEditRequest` has no `seed` field, so the iterate verb
                // can't actually reproduce inpainting runs. Record `None`
                // rather than claim a contract we don't fulfil.
                seed: None,
                negative_prompt: if negative.is_empty() {
                    None
                } else {
                    Some(negative)
                },
            },
        };

        let payload = IterateSheetPayload {
            entity_id: inputs.entity_id,
            source_variant_id: inputs.source_variant_id,
            variant,
        };
        let payload_json = serde_json::to_value(&payload)
            .map_err(|e| VerbError::Backend(format!("failed to serialise iterate payload: {e}")))?;

        let elapsed = started.elapsed();
        let summary = format!(
            "Iterate reference sheet for entity {} (panel: {panel_scope})",
            inputs.entity_id.get(),
        );

        progress.step(Some(1.0), "done").await;

        Ok(VerbOutput {
            summary,
            effects: vec![VerbEffect::Custom {
                name: ITERATE_SHEET_EFFECT_NAME.into(),
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

/// Searches all panel lists in `composition` for the first panel whose label
/// matches `label`. Also handles the `"palette-swatch"` sentinel for the
/// palette region. Returns `None` if no panel carries that label.
fn find_panel_rect<'a>(composition: &'a SheetComposition, label: &str) -> Option<&'a Rect> {
    if label == "palette-swatch" {
        return composition.palette_swatch.as_ref();
    }

    composition
        .views
        .iter()
        .chain(composition.expressions.iter())
        .chain(composition.callouts.iter())
        .chain(composition.outfits.iter())
        .find(|p| p.label == label)
        .map(|p| &p.region)
}

/// Creates a greyscale mask PNG sized to match `sheet_bytes`, with the
/// `panel` rectangle filled white (edit) and everything else black (keep).
///
/// The mask is passed to the inpainting backend to restrict pixel changes
/// to the panel region, keeping the rest of the sheet pixel-stable.
fn build_panel_mask(sheet_bytes: &[u8], panel: &Rect) -> Result<Vec<u8>> {
    let sheet = image::load_from_memory(sheet_bytes).map_err(|e| {
        VerbError::Backend(format!("failed to decode sheet for mask generation: {e}"))
    })?;

    let width = sheet.width();
    let height = sheet.height();
    // GrayImage::new fills with zero (black = keep).
    let mut mask = GrayImage::new(width, height);

    // Clamp the panel region to the image bounds to avoid out-of-bounds writes.
    // .max(0) ensures the value is non-negative before the infallible conversion.
    let px = u32::try_from(panel.origin.x.max(0)).unwrap_or(0);
    let py = u32::try_from(panel.origin.y.max(0)).unwrap_or(0);
    let x_end = px.saturating_add(panel.size.width).min(width);
    let y_end = py.saturating_add(panel.size.height).min(height);

    for row in py..y_end {
        for col in px..x_end {
            mask.put_pixel(col, row, image::Luma([255u8]));
        }
    }

    let mut buf = Vec::new();
    mask.write_to(&mut Cursor::new(&mut buf), ImageFormat::Png)
        .map_err(|e| VerbError::Backend(format!("failed to encode panel mask: {e}")))?;

    Ok(buf)
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
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
    use crate::verbs::reference_sheet::CompositionTemplate;

    use super::*;

    fn meta() -> ProjectMetadata {
        ProjectMetadata {
            name: "iterate-test".into(),
            description: None,
            author: None,
            created_at: 0,
            updated_at: 0,
            editor_version: "0".into(),
        }
    }

    /// Stub backend: handles `ImageInpaint` by returning a 1×1 white PNG.
    #[derive(Debug)]
    struct InpaintStub;

    impl InpaintStub {
        fn white_png() -> Vec<u8> {
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
    impl InferenceBackend for InpaintStub {
        fn backend_id(&self) -> &'static str {
            "stub.inpaint"
        }

        fn capabilities(&self) -> BackendCapabilities {
            BackendCapabilities::IMAGE_INPAINT
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
                InferenceRequest::ImageInpaint(_) => {
                    Ok(InferenceResponse::Image(ImageGenResponse {
                        images: vec![Self::white_png()],
                        model: "stub.inpaint".into(),
                    }))
                }
                _ => Err(BackendError::UnsupportedCapability),
            }
        }
    }

    fn white_png_b64() -> String {
        base64::engine::general_purpose::STANDARD.encode(InpaintStub::white_png())
    }

    fn inputs_whole_sheet(prompt: &str) -> VerbInputs {
        VerbInputs::from_struct(&IterateReferenceSheetInputs {
            entity_id: EntityId::new(1),
            source_variant_id: SheetVariantId::new(0),
            sheet_image_b64: white_png_b64(),
            composition: CompositionTemplate::Custom.composition(),
            panel_label: None,
            prompt: prompt.into(),
            negative_prompt: None,
        })
        .unwrap()
    }

    fn inputs_panel_scoped(label: &str) -> VerbInputs {
        VerbInputs::from_struct(&IterateReferenceSheetInputs {
            entity_id: EntityId::new(2),
            source_variant_id: SheetVariantId::new(5),
            sheet_image_b64: white_png_b64(),
            composition: CompositionTemplate::Character.composition(),
            panel_label: Some(label.into()),
            prompt: "make the hair longer".into(),
            negative_prompt: None,
        })
        .unwrap()
    }

    // ── Descriptor ────────────────────────────────────────────────────────────

    #[test]
    fn verb_id_matches_constant() {
        let verb = IterateReferenceSheetVerb::new();
        assert_eq!(
            verb.descriptor().id,
            VerbId::new(ITERATE_REFERENCE_SHEET_VERB_ID)
        );
    }

    #[test]
    fn verb_requires_image_inpaint_capability() {
        let verb = IterateReferenceSheetVerb::new();
        assert!(
            verb.descriptor()
                .required_capabilities
                .contains(BackendCapabilities::IMAGE_INPAINT),
            "verb must advertise IMAGE_INPAINT"
        );
    }

    #[test]
    fn verb_is_streaming_and_cancellable() {
        let verb = IterateReferenceSheetVerb::new();
        assert!(verb.descriptor().streaming);
        assert!(verb.descriptor().cancellable);
    }

    #[test]
    fn output_kind_is_custom_iterate_effect() {
        let verb = IterateReferenceSheetVerb::new();
        let kinds = &verb.descriptor().output_kinds;
        assert_eq!(kinds.len(), 1);
        match &kinds[0] {
            crate::plugin::descriptor::EffectKind::Custom(name) => {
                assert_eq!(name, ITERATE_SHEET_EFFECT_NAME);
            }
            other => panic!("expected Custom effect kind, got {other:?}"),
        }
    }

    // ── Validation ────────────────────────────────────────────────────────────

    #[test]
    fn validate_rejects_blank_prompt() {
        let verb = IterateReferenceSheetVerb::new();
        let inputs = VerbInputs::from_struct(&IterateReferenceSheetInputs {
            entity_id: EntityId::new(1),
            source_variant_id: SheetVariantId::new(0),
            sheet_image_b64: white_png_b64(),
            composition: CompositionTemplate::Custom.composition(),
            panel_label: None,
            prompt: "   ".into(),
            negative_prompt: None,
        })
        .unwrap();
        assert!(matches!(verb.validate(&inputs), Err(VerbError::Schema(_))));
    }

    #[test]
    fn validate_rejects_blank_sheet_image() {
        let verb = IterateReferenceSheetVerb::new();
        let inputs = VerbInputs::from_struct(&IterateReferenceSheetInputs {
            entity_id: EntityId::new(1),
            source_variant_id: SheetVariantId::new(0),
            sheet_image_b64: "   ".into(),
            composition: CompositionTemplate::Custom.composition(),
            panel_label: None,
            prompt: "make the eyes blue".into(),
            negative_prompt: None,
        })
        .unwrap();
        assert!(matches!(verb.validate(&inputs), Err(VerbError::Schema(_))));
    }

    #[test]
    fn validate_accepts_well_formed_inputs() {
        let verb = IterateReferenceSheetVerb::new();
        assert!(
            verb.validate(&inputs_whole_sheet("make the hair longer"))
                .is_ok()
        );
        assert!(verb.validate(&inputs_panel_scoped("happy")).is_ok());
    }

    // ── Full invocation via runtime ───────────────────────────────────────────

    #[tokio::test]
    async fn whole_sheet_iteration_produces_custom_effect() {
        let runtime = VerbRuntime::new();
        runtime
            .register_backend(BackendProxy::new(InpaintStub), 0)
            .unwrap();
        runtime.register(IterateReferenceSheetVerb::new()).unwrap();

        let inv = runtime
            .invoke(
                &VerbId::new(ITERATE_REFERENCE_SHEET_VERB_ID),
                VerbContext::empty(meta()),
                inputs_whole_sheet("make the cloak red"),
            )
            .unwrap();
        let preview = inv.finish().await.unwrap();

        assert_eq!(preview.verb.as_str(), ITERATE_REFERENCE_SHEET_VERB_ID);
        assert_eq!(preview.output.effects.len(), 1);

        match &preview.output.effects[0] {
            VerbEffect::Custom { name, payload } => {
                assert_eq!(name, ITERATE_SHEET_EFFECT_NAME);
                let decoded: IterateSheetPayload = serde_json::from_value(payload.clone()).unwrap();
                assert_eq!(decoded.entity_id, EntityId::new(1));
                assert_eq!(decoded.source_variant_id, SheetVariantId::new(0));
            }
            other => panic!("expected Custom effect, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn panel_scoped_iteration_produces_valid_payload() {
        let runtime = VerbRuntime::new();
        runtime
            .register_backend(BackendProxy::new(InpaintStub), 0)
            .unwrap();
        runtime.register(IterateReferenceSheetVerb::new()).unwrap();

        let inv = runtime
            .invoke(
                &VerbId::new(ITERATE_REFERENCE_SHEET_VERB_ID),
                VerbContext::empty(meta()),
                inputs_panel_scoped("happy"),
            )
            .unwrap();
        let preview = inv.finish().await.unwrap();

        match &preview.output.effects[0] {
            VerbEffect::Custom { payload, .. } => {
                let decoded: IterateSheetPayload = serde_json::from_value(payload.clone()).unwrap();
                assert_eq!(decoded.entity_id, EntityId::new(2));
                assert_eq!(decoded.source_variant_id, SheetVariantId::new(5));
                assert_eq!(
                    decoded.variant.composition.expressions.len(),
                    3,
                    "character composition must have three expressions"
                );
            }
            other => panic!("expected Custom effect, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn output_variant_carries_valid_base64_png() {
        let runtime = VerbRuntime::new();
        runtime
            .register_backend(BackendProxy::new(InpaintStub), 0)
            .unwrap();
        runtime.register(IterateReferenceSheetVerb::new()).unwrap();

        let inv = runtime
            .invoke(
                &VerbId::new(ITERATE_REFERENCE_SHEET_VERB_ID),
                VerbContext::empty(meta()),
                inputs_whole_sheet("darken the palette"),
            )
            .unwrap();
        let preview = inv.finish().await.unwrap();

        if let VerbEffect::Custom { payload, .. } = &preview.output.effects[0] {
            let decoded: IterateSheetPayload = serde_json::from_value(payload.clone()).unwrap();
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(&decoded.variant.image_b64)
                .expect("image_b64 must be valid base64");
            assert!(!bytes.is_empty());
            assert_eq!(
                &bytes[..8],
                &[137u8, 80, 78, 71, 13, 10, 26, 10],
                "decoded bytes must start with PNG signature"
            );
        }
    }

    #[tokio::test]
    async fn provenance_records_backend_and_prompt() {
        let runtime = VerbRuntime::new();
        runtime
            .register_backend(BackendProxy::new(InpaintStub), 0)
            .unwrap();
        runtime.register(IterateReferenceSheetVerb::new()).unwrap();

        let inv = runtime
            .invoke(
                &VerbId::new(ITERATE_REFERENCE_SHEET_VERB_ID),
                VerbContext::empty(meta()),
                inputs_whole_sheet("add freckles"),
            )
            .unwrap();
        let preview = inv.finish().await.unwrap();

        if let VerbEffect::Custom { payload, .. } = &preview.output.effects[0] {
            let decoded: IterateSheetPayload = serde_json::from_value(payload.clone()).unwrap();
            let prov = &decoded.variant.generation;
            assert_eq!(prov.backend, "stub.inpaint");
            assert_eq!(prov.model, "stub.inpaint");
            assert!(prov.prompt.contains("add freckles"));
        }
    }

    #[tokio::test]
    async fn unknown_panel_label_returns_schema_error() {
        let runtime = VerbRuntime::new();
        runtime
            .register_backend(BackendProxy::new(InpaintStub), 0)
            .unwrap();
        runtime.register(IterateReferenceSheetVerb::new()).unwrap();

        let inputs = VerbInputs::from_struct(&IterateReferenceSheetInputs {
            entity_id: EntityId::new(1),
            source_variant_id: SheetVariantId::new(0),
            sheet_image_b64: white_png_b64(),
            composition: CompositionTemplate::Custom.composition(),
            panel_label: Some("does-not-exist".into()),
            prompt: "make it green".into(),
            negative_prompt: None,
        })
        .unwrap();

        let inv = runtime
            .invoke(
                &VerbId::new(ITERATE_REFERENCE_SHEET_VERB_ID),
                VerbContext::empty(meta()),
                inputs,
            )
            .unwrap();
        let err = inv.finish().await.unwrap_err();

        assert!(
            matches!(err, VerbError::Schema(_)),
            "expected Schema error for unknown panel label, got {err:?}"
        );
    }

    #[tokio::test]
    async fn non_png_sheet_bytes_return_schema_error() {
        // Valid base64 that decodes to "not a png" — backend would otherwise
        // get garbage bytes; the verb must reject this up-front.
        let bogus = base64::engine::general_purpose::STANDARD.encode(b"not a png at all");

        let runtime = VerbRuntime::new();
        runtime
            .register_backend(BackendProxy::new(InpaintStub), 0)
            .unwrap();
        runtime.register(IterateReferenceSheetVerb::new()).unwrap();

        let inputs = VerbInputs::from_struct(&IterateReferenceSheetInputs {
            entity_id: EntityId::new(1),
            source_variant_id: SheetVariantId::new(0),
            sheet_image_b64: bogus,
            composition: CompositionTemplate::Custom.composition(),
            panel_label: None,
            prompt: "make it green".into(),
            negative_prompt: None,
        })
        .unwrap();

        let inv = runtime
            .invoke(
                &VerbId::new(ITERATE_REFERENCE_SHEET_VERB_ID),
                VerbContext::empty(meta()),
                inputs,
            )
            .unwrap();
        let err = inv.finish().await.unwrap_err();

        assert!(
            matches!(&err, VerbError::Schema(msg) if msg.contains("not a PNG")),
            "expected Schema error for non-PNG sheet bytes, got {err:?}"
        );
    }

    #[tokio::test]
    async fn fails_without_image_inpaint_backend() {
        let runtime = VerbRuntime::new();
        // No backend registered — IMAGE_INPAINT unsatisfied.
        runtime.register(IterateReferenceSheetVerb::new()).unwrap();

        let err = runtime
            .invoke(
                &VerbId::new(ITERATE_REFERENCE_SHEET_VERB_ID),
                VerbContext::empty(meta()),
                inputs_whole_sheet("test"),
            )
            .unwrap_err();

        assert!(
            matches!(
                err,
                VerbError::UnsupportedCapability { .. } | VerbError::BackendUnavailable { .. }
            ),
            "expected pre-flight failure when IMAGE_INPAINT backend is absent, got {err:?}"
        );
    }

    #[tokio::test]
    async fn cancellation_before_backend_returns_error() {
        let verb = IterateReferenceSheetVerb::new();
        let cancel = CancellationToken::new();
        cancel.cancel();

        let result = verb
            .invoke(
                VerbContext::empty(meta()),
                inputs_whole_sheet("test"),
                VerbProgress::discard(),
                cancel,
            )
            .await;
        assert!(result.is_err());
    }

    // ── Panel lookup helper ───────────────────────────────────────────────────

    #[test]
    fn find_panel_rect_returns_view_by_label() {
        let comp = CompositionTemplate::Character.composition();
        let rect = find_panel_rect(&comp, "front");
        assert!(
            rect.is_some(),
            "character composition must have a 'front' view"
        );
        let rect = rect.unwrap();
        assert_eq!(rect.origin.x, 0);
        assert_eq!(rect.size.width, 200);
    }

    #[test]
    fn find_panel_rect_returns_expression_by_label() {
        let comp = CompositionTemplate::Character.composition();
        let rect = find_panel_rect(&comp, "happy");
        assert!(
            rect.is_some(),
            "character composition must have a 'happy' expression"
        );
    }

    #[test]
    fn find_panel_rect_returns_palette_swatch() {
        let comp = CompositionTemplate::Custom.composition();
        let rect = find_panel_rect(&comp, "palette-swatch");
        assert!(
            rect.is_some(),
            "custom composition must have a palette swatch"
        );
    }

    #[test]
    fn find_panel_rect_returns_none_for_unknown_label() {
        let comp = CompositionTemplate::Custom.composition();
        assert!(find_panel_rect(&comp, "does-not-exist").is_none());
    }

    // ── Mask generation ───────────────────────────────────────────────────────

    #[test]
    fn build_panel_mask_produces_valid_png() {
        let sheet_png = InpaintStub::white_png();
        let panel = Rect::from_xywh(0, 0, 1, 1);
        let mask_bytes = build_panel_mask(&sheet_png, &panel).unwrap();
        // PNG signature check.
        assert_eq!(
            &mask_bytes[..8],
            &[137u8, 80, 78, 71, 13, 10, 26, 10],
            "mask must be a valid PNG"
        );
    }

    #[test]
    fn build_panel_mask_clamps_out_of_bounds_region() {
        let sheet_png = InpaintStub::white_png();
        // Panel extends well beyond the 1×1 image — should not panic.
        let panel = Rect::from_xywh(0, 0, 100, 100);
        let result = build_panel_mask(&sheet_png, &panel);
        assert!(
            result.is_ok(),
            "mask generation must not fail for oversized panel"
        );
    }
}
