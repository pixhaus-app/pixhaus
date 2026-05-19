//! Built-in AI verbs (S23–S36).
//!
//! Each verb lives in its own submodule. The modules are public so the
//! app crate can instantiate verbs with whatever
//! [`crate::backends::BackendRegistry`] the user has configured.
//!
//! Most built-ins are registered with the
//! [`crate::plugin::runtime::VerbRuntime`] at startup in
//! `app::state::AppState::new`. A handful are intentionally not
//! registered there — they ship as part of this crate (so plugins,
//! scripting, and tests can construct them) but require either a
//! configured backend at construction time or a follow-up refactor
//! before the runtime can host them. [`ConversationalVerb`] is the
//! current example: its `::new` takes a concrete `Arc<dyn
//! InferenceBackend>` rather than the shared `BackendRegistry` other
//! verbs use, so registration must wait until the backend-config
//! flow lands or its constructor is reworked.
//!
//! # Verb ID namespace
//!
//! Most built-in verbs use the prefix `pixhaus.builtin.`. Third-party
//! plugins use their own reverse-DNS namespace; the runtime does not
//! enforce namespacing but the convention prevents collisions.
//!
//! Known drift:
//! - [`SketchFinishingVerb`] advertises `pixhaus.ai.sketch_finishing`
//!   (legacy prefix instead of `pixhaus.builtin.`).
//! - [`AutoMeshDeformationVerb`] advertises
//!   `pixhaus.builtin.auto-mesh-deformation` (kebab-case instead of the
//!   `snake_case` every other built-in uses).
//!
//! Both are public surface (logged, scriptable, plugin-addressable, baked
//! into stored projects), so neither is renamed. Future built-ins should
//! stick to `pixhaus.builtin.<name>` in `snake_case`.
//!
//! # Shared helpers
//!
//! Verbs that need an inference call go through one of these helpers,
//! which each centralise the downcast from
//! `Arc<dyn plugin::backend::InferenceBackend>` to a concrete adapter:
//!
//! - `call_text_vlm` — for `TextGenRequest` (Anthropic / `OpenAI`).
//! - `call_image_edit` — for `ImageEditRequest` (Stability / `OpenAI` /
//!   Replicate).
//!
//! `ctx_fat_backend` is the no-request-type escape hatch for verbs that
//! need the fat trait reference but don't fit one of the typed helpers
//! (today: `tileset_from_description`, `animated_sprite_sheet`).
//!
//! Adding a new concrete adapter means updating the helper(s) that
//! accept it, not the verbs themselves. Two verbs still open-code their
//! own downcast chains because their backend method is not part of the
//! trait (`project_style_learning` and `train_entity_lora` call
//! `replicate.run_style_training`, a Replicate-specific method) or has
//! domain-specific response unpacking (`continue_verb` accepts both
//! `Frames` and `Image` responses for `ComfyUI` compatibility). See the
//! follow-up notes in `docs/planning/work/architecture-cleanup-s53.md`.

pub mod animated_sprite_sheet;
pub mod audio_timing;
pub mod auto_mesh_deformation;
pub mod cleanup;
pub mod continue_verb;
pub mod conversational;
pub mod critique;
pub mod extend;
pub mod inbetween;
pub mod iterate_reference_sheet;
pub mod motion_from_video;
pub mod project_style_learning;
pub mod reference_sheet;
pub mod sketch_finishing;
pub mod tile;
pub mod tileset_from_description;
pub mod train_entity_lora;
pub mod variant;

pub use animated_sprite_sheet::AnimatedSpriteSheetVerb;
pub use audio_timing::AudioTimingVerb;
pub use auto_mesh_deformation::AutoMeshDeformationVerb;
pub use cleanup::CleanupVerb;
pub use continue_verb::ContinueVerb;
pub use conversational::ConversationalVerb;
pub use critique::CritiqueVerb;
pub use extend::ExtendVerb;
pub use inbetween::InbetweenVerb;
pub use iterate_reference_sheet::IterateReferenceSheetVerb;
pub use motion_from_video::MotionFromVideoVerb;
pub use project_style_learning::ProjectStyleLearningVerb;
pub use reference_sheet::GenerateReferenceSheetVerb;
pub use sketch_finishing::SketchFinishingVerb;
pub use tile::TileVerb;
pub use tileset_from_description::TilesetFromDescriptionVerb;
pub use train_entity_lora::TrainEntityLoraVerb;
pub use variant::VariantVerb;

use tokio_util::sync::CancellationToken;

use crate::backends::{
    ImageEditRequest, InferenceBackend as OpsBackend, InferenceRequest, InferenceResponse,
    TextGenRequest, TextGenResponse, anthropic::AnthropicBackend, bridge::BackendProxy,
    openai::OpenAiBackend, replicate::ReplicateBackend, stability::StabilityBackend,
};
use crate::plugin::backend::InferenceBackend as PluginBackend;
use crate::plugin::context::VerbContext;
use crate::plugin::error::{Result, VerbError};
use crate::plugin::progress::VerbProgress;

/// Returns the anchor sheet's image bytes for an `ImageGenRequest`'s
/// `style_image` slot, or `None` when the active entity has no anchor.
///
/// Use this from image-generation verbs that already have a user-
/// supplied `style_image: Option<Vec<u8>>` input — the anchor is the
/// fallback when the user doesn't override:
///
/// ```ignore
/// let style_image = inputs.style_image.or_else(|| anchor_style_image_bytes(&ctx));
/// ```
///
/// Verbs that don't run image generation (Critique, Conversational,
/// Audio-driven, ...) can still reach the anchor via `ctx.anchor`
/// directly; this helper is just the common path.
#[must_use]
pub(crate) fn anchor_style_image_bytes(ctx: &VerbContext) -> Option<Vec<u8>> {
    ctx.anchor
        .as_ref()
        .and_then(crate::plugin::anchor::AnchorPayload::style_image_bytes)
}

/// Returns the fat operational backend attached to `ctx`, downcasting
/// from the thin trait the runtime stores.
///
/// The runtime selects a backend matching the verb's `required_capabilities`
/// and stores it in `ctx.backend` as a `Arc<dyn plugin::backend::InferenceBackend>`.
/// Verbs that need to make actual inference calls reach the operational
/// `crate::backends::InferenceBackend` trait through this helper. Add a
/// new branch when a new concrete adapter lands; verbs themselves don't
/// need to change.
///
/// # Errors
///
/// - [`VerbError::Backend`] if `ctx.backend` is `None` (the runtime
///   skips selection for verbs whose `required_capabilities` are empty
///   — none of the backend-using verbs hit this in practice) or holds
///   a backend type this helper doesn't recognise (a future adapter that
///   forgot to register here, or a test stub that isn't wrapped in
///   [`BackendProxy`]).
pub(crate) fn ctx_fat_backend(ctx: &VerbContext) -> Result<&dyn OpsBackend> {
    let plug = ctx
        .backend
        .as_ref()
        .ok_or_else(|| VerbError::Backend("no backend attached to verb context".into()))?;
    let any = plug.as_any();
    if let Some(b) = any.downcast_ref::<AnthropicBackend>() {
        Ok(b as &dyn OpsBackend)
    } else if let Some(b) = any.downcast_ref::<OpenAiBackend>() {
        Ok(b as &dyn OpsBackend)
    } else if let Some(b) = any.downcast_ref::<ReplicateBackend>() {
        Ok(b as &dyn OpsBackend)
    } else if let Some(b) = any.downcast_ref::<StabilityBackend>() {
        Ok(b as &dyn OpsBackend)
    } else if let Some(p) = any.downcast_ref::<BackendProxy>() {
        Ok(p.fat())
    } else {
        Err(VerbError::Backend(
            "ctx.backend type not recognised; register a known adapter or wrap in BackendProxy"
                .into(),
        ))
    }
}

/// Sends a text-generation (optionally vision-language) request through
/// whichever concrete backend is attached to the verb context.
///
/// The `plugin::backend::InferenceBackend` trait is intentionally thin —
/// verbs that need to make inference calls downcast to a concrete adapter.
/// This helper consolidates the downcast logic so individual verbs don't
/// repeat it. It tries each known VLM-capable adapter in declaration order
/// and returns [`VerbError::Backend`] if none match.
///
/// # Supported backends
///
/// - [`AnthropicBackend`] — Claude Sonnet 4.6 (default model)
/// - [`OpenAiBackend`] — GPT-4o (default model)
///
/// Additional adapters are added here as they land; verbs do not need to
/// change when a new adapter is added.
pub(crate) async fn call_text_vlm(
    backend: &dyn PluginBackend,
    request: TextGenRequest,
    progress: VerbProgress,
    cancel: CancellationToken,
) -> Result<TextGenResponse> {
    let req = InferenceRequest::Text(request);

    let resp = if let Some(b) = backend.as_any().downcast_ref::<AnthropicBackend>() {
        b.invoke(req, progress, cancel)
            .await
            .map_err(|e| VerbError::Backend(e.to_string()))?
    } else if let Some(b) = backend.as_any().downcast_ref::<OpenAiBackend>() {
        b.invoke(req, progress, cancel)
            .await
            .map_err(|e| VerbError::Backend(e.to_string()))?
    } else if let Some(p) = backend.as_any().downcast_ref::<BackendProxy>() {
        p.fat()
            .invoke(req, progress, cancel)
            .await
            .map_err(|e| VerbError::Backend(e.to_string()))?
    } else {
        return Err(VerbError::Backend(
            "no supported VLM adapter attached to context; \
             register an Anthropic or OpenAI backend with the VerbRuntime"
                .into(),
        ));
    };

    match resp {
        InferenceResponse::Text(t) => Ok(t),
        _ => Err(VerbError::Backend(
            "backend returned a non-text response to a text request".into(),
        )),
    }
}

/// Sends an image-edit request through whichever concrete image-edit
/// adapter is attached to the verb context, returning the raw PNG bytes
/// of each generated image.
///
/// Tries Stability → `OpenAI` → Replicate in declaration order, then
/// [`BackendProxy`] for test backends. Returns [`VerbError::Backend`]
/// when none match.
///
/// # Supported backends
///
/// - [`StabilityBackend`]
/// - [`OpenAiBackend`]
/// - [`ReplicateBackend`]
///
/// Additional adapters are added here as they land; verbs do not need to
/// change when a new adapter is added.
pub(crate) async fn call_image_edit(
    backend: &dyn PluginBackend,
    request: ImageEditRequest,
    progress: VerbProgress,
    cancel: CancellationToken,
) -> Result<Vec<Vec<u8>>> {
    let req = InferenceRequest::ImageEdit(request);
    let any = backend.as_any();

    let resp = if let Some(b) = any.downcast_ref::<StabilityBackend>() {
        b.invoke(req, progress, cancel)
            .await
            .map_err(|e| VerbError::Backend(e.to_string()))?
    } else if let Some(b) = any.downcast_ref::<OpenAiBackend>() {
        b.invoke(req, progress, cancel)
            .await
            .map_err(|e| VerbError::Backend(e.to_string()))?
    } else if let Some(b) = any.downcast_ref::<ReplicateBackend>() {
        b.invoke(req, progress, cancel)
            .await
            .map_err(|e| VerbError::Backend(e.to_string()))?
    } else if let Some(p) = any.downcast_ref::<BackendProxy>() {
        p.fat()
            .invoke(req, progress, cancel)
            .await
            .map_err(|e| VerbError::Backend(e.to_string()))?
    } else {
        return Err(VerbError::Backend(
            "no supported image-edit adapter attached to context; \
             register a Stability, OpenAI, or Replicate backend with the VerbRuntime"
                .into(),
        ));
    };

    match resp {
        InferenceResponse::Image(img) => Ok(img.images),
        _ => Err(VerbError::Backend(
            "image-edit backend returned a non-image response".into(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::verb::Verb;

    #[test]
    fn critique_verb_is_exported() {
        let verb = CritiqueVerb::new();
        assert_eq!(verb.descriptor().id.as_str(), critique::CRITIQUE_VERB_ID);
    }

    #[test]
    fn generate_reference_sheet_verb_is_exported() {
        let verb = GenerateReferenceSheetVerb::new();
        assert_eq!(
            verb.descriptor().id.as_str(),
            reference_sheet::GENERATE_REFERENCE_SHEET_VERB_ID
        );
    }

    #[test]
    fn anchor_style_image_bytes_returns_none_without_anchor() {
        use pixhaus_core::project::ProjectMetadata;

        let ctx = VerbContext::empty(ProjectMetadata {
            name: "no-anchor".into(),
            description: None,
            author: None,
            created_at: 0,
            updated_at: 0,
            editor_version: "0".into(),
        });
        assert!(anchor_style_image_bytes(&ctx).is_none());
    }

    #[test]
    fn anchor_style_image_bytes_returns_image_when_anchor_set() {
        use pixhaus_core::project::{
            AiMetadata, AssetInfo, Entity, EntityContent, EntityDefaults, EntityId, EntityKind,
            ProjectMetadata, ReferenceImage, ReferenceSheet, SheetVariant, SheetVariantId,
            UserData,
        };
        use std::collections::BTreeMap;

        let entity = Entity {
            id: EntityId::new(1),
            kind: EntityKind::Custom("Character".into()),
            name: "Hero".into(),
            group_id: None,
            tags: Vec::new(),
            defaults: EntityDefaults::default(),
            content: EntityContent::Sprites {
                states: Vec::new(),
                reference_sheet: Some(Box::new(ReferenceSheet {
                    canonical: Some(SheetVariant::from_image(
                        SheetVariantId::new(1),
                        0,
                        ReferenceImage {
                            bytes: vec![1, 2, 3, 4, 5],
                            mime: "image/png".into(),
                        },
                    )),
                    variants: Vec::new(),
                    prompts: Vec::new(),
                    info: AssetInfo {
                        fields: BTreeMap::new(),
                        notes: Vec::new(),
                    },
                })),
            },
            ai: AiMetadata::default(),
            user_data: UserData::default(),
            created_at: 0,
            updated_at: 0,
        };
        let anchor =
            crate::plugin::anchor::AnchorPayload::from_sprite_entity(&entity, 0.7, None).unwrap();

        let mut ctx = VerbContext::empty(ProjectMetadata {
            name: "anchored".into(),
            description: None,
            author: None,
            created_at: 0,
            updated_at: 0,
            editor_version: "0".into(),
        });
        ctx.anchor = Some(anchor);

        assert_eq!(anchor_style_image_bytes(&ctx), Some(vec![1, 2, 3, 4, 5]));
    }

    #[test]
    fn iterate_reference_sheet_verb_is_exported() {
        let verb = IterateReferenceSheetVerb::new();
        assert_eq!(
            verb.descriptor().id.as_str(),
            iterate_reference_sheet::ITERATE_REFERENCE_SHEET_VERB_ID
        );
    }
}
