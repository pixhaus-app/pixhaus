//! Project- and entity-level AI metadata: style learning, model routing
//! preferences, prompt history, and asynchronous `LoRA` training jobs.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::project::color::Rgba;
use crate::project::id::{AssetId, EntityId, TagId, TrainingJobId};

use super::assets::{AssetLibrary, LoraKind};

/// Project-level AI memory and defaults.
///
/// Per-entity AI metadata lives on [`Entity::ai`](super::Entity::ai); this
/// struct collects the project-wide pieces used by the v1 reference-sheet
/// workflow: prompt style notes, reusable assets, provider-routing
/// preferences, project defaults, and asynchronous `LoRA` training jobs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ProjectAi {
    /// Project-level notes prepended to every AI prompt composition.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub style_notes: String,

    /// Browsable project-scoped AI asset library.
    #[serde(default, skip_serializing_if = "AssetLibrary::is_empty")]
    pub asset_library: AssetLibrary,

    /// Per-operation model overrides. If absent, the router uses its
    /// operation defaults and configured-provider fallback.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub per_operation_model_prefs: BTreeMap<OperationKind, ModelId>,

    /// Default chroma-key color for new sheet generations.
    #[serde(
        default = "default_reference_chroma",
        skip_serializing_if = "is_default_reference_chroma"
    )]
    pub default_chroma: Rgba,

    /// Default quality tier for new generation forms.
    #[serde(
        default = "default_project_quality",
        skip_serializing_if = "Quality::is_medium"
    )]
    pub default_quality: Quality,

    /// Default candidate count for new generation forms.
    #[serde(
        default = "default_candidate_count",
        skip_serializing_if = "is_default_candidate_count"
    )]
    pub default_candidate_count: u8,

    /// `LoRA` training jobs, including completed and failed jobs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub training_jobs: Vec<TrainingJob>,

    /// Entity ids included in the project's style reference corpus. The
    /// Project Style Learning verb (S30) trains a `LoRA` from these.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub style_corpus: Vec<EntityId>,

    /// Path to the trained `LoRA` file relative to the project directory.
    /// Optional — projects without a learned style still work.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_lora_path: Option<String>,

    /// Recent prompts for resume-where-you-left-off semantic search.
    /// Capped — the implementation details land with S21's follow-up.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub prompt_history: Vec<PromptHistoryEntry>,

    /// Project-tier composition Structures. Shadow built-ins by id.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub structures: Vec<crate::project::library::composition::Structure>,

    /// Project-tier Styles.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub styles: Vec<crate::project::library::composition::Style>,

    /// Project-tier saved Prompts.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub prompts: Vec<crate::project::library::composition::PromptTemplate>,
}

impl Default for ProjectAi {
    fn default() -> Self {
        Self {
            style_notes: String::new(),
            asset_library: AssetLibrary::default(),
            per_operation_model_prefs: BTreeMap::new(),
            default_chroma: default_reference_chroma(),
            default_quality: Quality::Medium,
            default_candidate_count: default_candidate_count(),
            training_jobs: Vec::new(),
            style_corpus: Vec::new(),
            project_lora_path: None,
            prompt_history: Vec::new(),
            structures: Vec::new(),
            styles: Vec::new(),
            prompts: Vec::new(),
        }
    }
}

impl ProjectAi {
    /// Returns `true` when no project-level AI memory is set.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.style_notes.is_empty()
            && self.asset_library.is_empty()
            && self.per_operation_model_prefs.is_empty()
            && is_default_reference_chroma(&self.default_chroma)
            && self.default_quality.is_medium()
            && is_default_candidate_count(&self.default_candidate_count)
            && self.training_jobs.is_empty()
            && self.style_corpus.is_empty()
            && self.project_lora_path.is_none()
            && self.prompt_history.is_empty()
            && self.structures.is_empty()
            && self.styles.is_empty()
            && self.prompts.is_empty()
    }
}

/// Default chroma-key background for AI-generated reference sheets.
#[must_use]
pub const fn default_reference_chroma() -> Rgba {
    Rgba::opaque(255, 0, 255)
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_default_reference_chroma(color: &Rgba) -> bool {
    *color == default_reference_chroma()
}

fn default_project_quality() -> Quality {
    Quality::Medium
}

fn default_candidate_count() -> u8 {
    2
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_default_candidate_count(n: &u8) -> bool {
    *n == default_candidate_count()
}

/// Per-entity AI metadata.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AiMetadata {
    /// Auto-tags suggested by VLM analysis but not yet user-confirmed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub suggested_tags: Vec<TagId>,

    /// VLM-generated description used for semantic search. Re-derived
    /// when the entity changes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vlm_summary: Option<String>,

    /// Embedding for similarity search. Optional and computed lazily —
    /// the entity is fully usable without one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub embedding: Option<Vec<f32>>,

    /// Per-entity `LoRA` reference. Populated by the B10.5
    /// train-entity-lora verb after a successful training run against
    /// this entity's canonical reference sheet. **Currently the Replicate
    /// weights URL written verbatim by the IPC layer; a future host-side
    /// download will replace it with a project-relative path.** When
    /// present, anchor payloads built for this entity carry it through to
    /// backends, overriding any project-wide `LoRA` for the duration of
    /// generations against this entity. `None` means "fall back to the
    /// project-wide `LoRA`, if any."
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lora_path: Option<String>,

    /// Animation-studio working state for this entity — stage, controls,
    /// first-frame candidates, approved frame, normalized candidates, the raw
    /// video clip, and the video job — as a UI-owned JSON blob. Opaque here on
    /// purpose: the studio's shape lives in the frontend, and this lets the
    /// user resume the staged pipeline after save/reload. `None` means the
    /// studio has never been used for this entity (start fresh).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub animation_studio_state: Option<String>,
}

impl AiMetadata {
    /// Returns `true` when no AI metadata is attached.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.suggested_tags.is_empty()
            && self.vlm_summary.is_none()
            && self.embedding.is_none()
            && self.lora_path.is_none()
            && self.animation_studio_state.is_none()
    }
}

/// One entry in the per-project prompt history.
///
/// Defined minimally for B9; the verb runtime (S21 / B10) extends the
/// shape as the streaming-prompt workflow lands.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PromptHistoryEntry {
    /// Verb that consumed the prompt.
    pub verb_name: String,
    /// The user-typed prompt text.
    pub prompt: String,
    /// UTC seconds-since-epoch at which the prompt was issued.
    #[ts(type = "number")]
    pub timestamp: i64,
}

/// High-level quality tier exposed by the reference-sheet editor.
#[derive(
    Copy, Clone, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, TS,
)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum Quality {
    /// Let the router/provider choose the quality tier.
    Auto,
    /// Cheapest / fastest tier.
    Low,
    /// Balanced default tier.
    #[default]
    Medium,
    /// Final/promoted output tier.
    High,
}

impl Quality {
    #[allow(clippy::trivially_copy_pass_by_ref)]
    pub(super) fn is_medium(&self) -> bool {
        matches!(self, Self::Medium)
    }
}

/// Stable Pixhaus model labels. Provider-specific endpoint IDs live in
/// `pixhaus-ai`; the project file stores these product-facing identifiers.
#[derive(
    Copy, Clone, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, TS,
)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ModelId {
    /// Let the router pick by operation type and provider availability.
    #[default]
    Auto,
    /// `OpenAI` image model label for `gpt-image-2`.
    OpenAiGptImage2,
    /// Google AI Studio Nano Banana Pro image model label.
    GoogleNanoBananaPro,
    /// Google AI Studio Flash image model label.
    GoogleGeminiFlashImage,
    /// fal Flux Kontext image/edit model.
    FalFluxKontext,
    /// fal Flux.1 dev with extensions such as `LoRA`/IP-Adapter.
    FalFluxDev,
    /// fal Recraft vectorize endpoint.
    FalRecraftVectorize,
    /// fal Real-ESRGAN upscaler.
    FalRealEsrgan,
}

/// Reference-sheet operation used by model routing and project prefs.
#[allow(missing_docs)]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum OperationKind {
    FreshGeneration,
    MaskedRefinement,
    PromptOnlyRefinement,
    RegionalRefinement,
    ChatTurn,
    Promotion,
    CrossModelGrid,
    VectorExport,
    Upscale,
    LoraTraining,
}

/// State of a fal `LoRA` training job.
#[allow(missing_docs)]
#[derive(
    Copy, Clone, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, TS,
)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum TrainingStatus {
    #[default]
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
}

/// Async `LoRA` training job record.
#[allow(missing_docs)]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct TrainingJob {
    pub id: TrainingJobId,
    pub asset_name: String,
    #[serde(default)]
    pub kind: LoraKind,
    pub target_model: ModelId,
    pub trigger_word: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub training_data: Vec<AssetId>,
    pub fal_job_id: String,
    #[serde(default)]
    pub status: TrainingStatus,
    #[ts(type = "number")]
    pub created_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(type = "number | null")]
    pub completed_at: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result_lora_id: Option<AssetId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[cfg(test)]
mod project_ai_library_tests {
    use super::*;
    use crate::project::library::composition::{Structure, StructureId, StructureOutput};

    #[test]
    fn ai_metadata_is_empty_tracks_animation_studio_state() {
        assert!(AiMetadata::default().is_empty());

        let with_state = AiMetadata {
            animation_studio_state: Some(r#"{"stage":"reference"}"#.into()),
            ..AiMetadata::default()
        };
        assert!(!with_state.is_empty());
    }

    #[test]
    fn new_project_ai_has_empty_library() {
        let ai = ProjectAi::default();
        assert!(ai.structures.is_empty());
        assert!(ai.styles.is_empty());
        assert!(ai.prompts.is_empty());
        assert!(ai.is_empty());
    }

    #[test]
    fn project_ai_with_structure_is_not_empty() {
        let mut ai = ProjectAi::default();
        ai.structures.push(Structure {
            id: StructureId("p.s".into()),
            name: "P".into(),
            output: StructureOutput::Single,
            layout_negatives: String::new(),
        });
        assert!(!ai.is_empty());
    }

    #[test]
    fn old_blob_without_library_deserializes() {
        // Simulate an old file: a ProjectAi JSON with none of the new fields.
        let old = r"{}";
        let ai: ProjectAi = serde_json::from_str(old).unwrap();
        assert!(ai.structures.is_empty());
    }
}
