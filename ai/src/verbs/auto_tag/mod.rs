//! Verb: `AutoTag` — proposes library tags for a sprite entity from its
//! canonical reference-sheet image.
//!
//! This is a stub. Auto-tagging reads an image and emits text labels, so it
//! needs a vision-language backend (`BackendCapabilities::VISION_LANGUAGE`).
//! V2 ships no such backend (the FAL slice is image-generation only, `OpenAI`
//! is image-only here), so the verb is backend-gated: its descriptor declares the
//! capability, and `invoke` returns a clear "needs a vision backend" error
//! rather than a fake result.
//!
//! What ships now is the surface around it: the descriptor (so a future vision
//! backend selection finds the verb), the typed inputs/output payload, and the
//! gated invoke. The accept/reject side — promoting a suggested `TagId` into an
//! entity's confirmed tags or dropping it — is the shell's job and lands today
//! against any `AiMetadata::suggested_tags` already present, independent of this
//! verb. When a vision backend registers, fill in `invoke`'s body to call it.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use pixhaus_core::project::EntityId;

use crate::plugin::context::VerbContext;
use crate::plugin::descriptor::{BackendCapabilities, CostEstimate, EffectKind, VerbDescriptor, VerbId};
use crate::plugin::error::{Result, VerbError};
use crate::plugin::inputs::VerbInputs;
use crate::plugin::output::VerbOutput;
use crate::plugin::progress::VerbProgress;
use crate::plugin::verb::Verb;

/// Stable ID for the built-in auto-tag verb.
pub const AUTO_TAG_VERB_ID: &str = "pixhaus.builtin.auto_tag";

/// Effect name used in the `VerbEffect::Custom` payload this verb produces once
/// a vision backend is available.
pub const AUTO_TAG_EFFECT_NAME: &str = "pixhaus.builtin.auto_tag.suggestions";

// ── Input / output types ────────────────────────────────────────────────────

/// Inputs for [`AutoTagVerb`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AutoTagInputs {
    /// Target sprite entity. Auto-tagging reads its canonical sheet image and
    /// proposes tag names; the host writes the resolved `TagId`s into the
    /// entity's `AiMetadata::suggested_tags` for the artist to accept or reject.
    pub entity_id: EntityId,

    /// Base64-encoded PNG of the image to analyse — typically the entity's
    /// canonical reference-sheet variant.
    pub image_b64: String,

    /// Upper bound on proposed tags. Clamped to `1..=12` on invoke.
    #[serde(default = "default_max_tags")]
    pub max_tags: u32,
}

const fn default_max_tags() -> u32 {
    8
}

/// The payload an `AutoTag` invocation produces once a vision backend exists:
/// the proposed tag names for the host to resolve into `TagId`s and append to
/// the entity's `suggested_tags`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AutoTagSuggestions {
    /// Target entity the suggestions apply to.
    pub entity_id: EntityId,
    /// Proposed tag names, deduplicated and trimmed by the verb.
    pub tag_names: Vec<String>,
}

// ── Verb implementation ─────────────────────────────────────────────────────

/// AI verb that proposes library tags for an entity from its image.
///
/// Construct with [`AutoTagVerb::new`]. The verb is backend-gated: its
/// descriptor declares `VISION_LANGUAGE`, and until a vision backend registers,
/// [`Verb::invoke`] returns [`VerbError::BackendUnavailable`].
pub struct AutoTagVerb {
    descriptor: VerbDescriptor,
}

impl AutoTagVerb {
    /// Constructs the verb. A vision backend is selected per invocation once one
    /// is registered.
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
                "image_b64": {
                    "type": "string",
                    "minLength": 1,
                    "description": "Base64-encoded PNG of the image to analyse"
                },
                "max_tags": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 12,
                    "default": 8,
                    "description": "Upper bound on the number of proposed tags"
                }
            },
            "required": ["entity_id", "image_b64"]
        });
        Self {
            descriptor: VerbDescriptor {
                id: VerbId::new(AUTO_TAG_VERB_ID),
                display_name: "Auto-tag".into(),
                description: "Propose library tags for an entity from its reference image (needs a vision backend).".into(),
                version: "0.1.0".into(),
                required_capabilities: BackendCapabilities::VISION_LANGUAGE,
                input_schema,
                output_schema: None,
                output_kinds: vec![EffectKind::Custom(AUTO_TAG_EFFECT_NAME.into())],
                cost_estimate: CostEstimate {
                    typical_latency: std::time::Duration::from_secs(4),
                    max_latency: std::time::Duration::from_secs(30),
                    typical_usd_cents: 0.2,
                    max_usd_cents: 2.0,
                },
                streaming: false,
                cancellable: true,
                documentation_url: None,
            },
        }
    }
}

impl Default for AutoTagVerb {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for AutoTagVerb {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AutoTagVerb").field("id", &self.descriptor.id).finish_non_exhaustive()
    }
}

#[async_trait]
impl Verb for AutoTagVerb {
    fn descriptor(&self) -> &VerbDescriptor {
        &self.descriptor
    }

    fn validate(&self, inputs: &VerbInputs) -> Result<()> {
        let parsed: AutoTagInputs = inputs.deserialize()?;
        if parsed.image_b64.trim().is_empty() {
            return Err(VerbError::Schema("auto-tag: image_b64 must not be blank".into()));
        }
        Ok(())
    }

    async fn invoke(&self, _ctx: VerbContext, inputs: VerbInputs, _progress: VerbProgress, _cancel: CancellationToken) -> Result<VerbOutput> {
        // Deserialise so a malformed payload still surfaces as a schema error
        // rather than the gate message, keeping the failure mode honest.
        let _inputs: AutoTagInputs = inputs.deserialize_owned()?;
        // Backend-gated: v2 ships no vision-language backend, so the verb cannot
        // run yet. Report it as unavailable (a matching backend would satisfy
        // the descriptor's VISION_LANGUAGE capability) rather than fabricating
        // tags. When a vision backend lands, replace this with the real call:
        // analyse `image_b64`, collect tag names, and emit an `AUTO_TAG_EFFECT_NAME`
        // custom effect carrying `AutoTagSuggestions`.
        Err(VerbError::BackendUnavailable {
            id: "auto-tag needs a vision-language backend; none is configured in this build".into(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::context::VerbContext;
    use crate::plugin::progress::VerbProgress;
    use pixhaus_core::project::ProjectMetadata;

    fn meta() -> ProjectMetadata {
        ProjectMetadata {
            name: "auto-tag-test".into(),
            description: None,
            author: None,
            created_at: 0,
            updated_at: 0,
            editor_version: "0".into(),
        }
    }

    fn sample_inputs() -> VerbInputs {
        VerbInputs::from_struct(&AutoTagInputs {
            entity_id: EntityId::new(1),
            image_b64: "iVBORw0KGgo=".into(),
            max_tags: 8,
        })
        .expect("serialize inputs")
    }

    #[test]
    fn descriptor_declares_vision_language() {
        let verb = AutoTagVerb::new();
        assert_eq!(verb.descriptor().id.as_str(), AUTO_TAG_VERB_ID);
        assert!(
            verb.descriptor().required_capabilities.contains(BackendCapabilities::VISION_LANGUAGE),
            "auto-tag is gated on a vision-language backend"
        );
    }

    #[test]
    fn validate_rejects_blank_image() {
        let verb = AutoTagVerb::new();
        let inputs = VerbInputs::from_struct(&AutoTagInputs {
            entity_id: EntityId::new(1),
            image_b64: "   ".into(),
            max_tags: 8,
        })
        .expect("serialize");
        assert!(matches!(verb.validate(&inputs), Err(VerbError::Schema(_))));
    }

    #[test]
    fn validate_accepts_a_real_image() {
        let verb = AutoTagVerb::new();
        assert!(verb.validate(&sample_inputs()).is_ok());
    }

    #[tokio::test]
    async fn invoke_is_gated_without_a_vision_backend() {
        let verb = AutoTagVerb::new();
        let err = verb
            .invoke(VerbContext::empty(meta()), sample_inputs(), VerbProgress::discard(), CancellationToken::new())
            .await
            .expect_err("no vision backend, so invoke must fail");
        assert!(
            matches!(err, VerbError::BackendUnavailable { .. }),
            "the gate reports the backend as unavailable, not a fabricated result: {err:?}"
        );
    }

    #[tokio::test]
    async fn invoke_surfaces_a_malformed_payload_as_schema_error() {
        let verb = AutoTagVerb::new();
        let bad = VerbInputs::new(serde_json::json!({ "entity_id": "not-a-number" }));
        let err = verb
            .invoke(VerbContext::empty(meta()), bad, VerbProgress::discard(), CancellationToken::new())
            .await
            .expect_err("malformed inputs fail");
        assert!(matches!(err, VerbError::Schema(_)), "a bad payload fails as schema, not the gate: {err:?}");
    }

    #[test]
    fn suggestions_payload_round_trips() {
        let payload = AutoTagSuggestions {
            entity_id: EntityId::new(4),
            tag_names: vec!["hero".into(), "armored".into()],
        };
        let json = serde_json::to_value(&payload).expect("serialize");
        let back: AutoTagSuggestions = serde_json::from_value(json).expect("deserialize");
        assert_eq!(back, payload);
    }
}
