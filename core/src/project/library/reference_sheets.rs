//! Reference sheets: the canonical AI consistency anchor for a sprite
//! entity, its generated/imported/refined variants, templates, panel
//! composition, refinement metadata, and prompt provenance.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use ts_rs::{Config, TS, TypeVisitor};

use crate::project::color::Rgba;
use crate::project::geometry::{IVec2, Rect};
use crate::project::id::{AssetId, SheetVariantId};
use crate::project::palette::PaletteEntry;

use super::ai::{ModelId, Quality, default_reference_chroma};

/// Opaque holder for an image referenced inside the data model.
///
/// Pixhaus does not interpret the bytes here; it just round-trips them.
/// The `mime` hint helps the UI decide how to render — typically
/// `image/png` for B9, with B10 widening to other formats as the sheet
/// generator lands.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ReferenceImage {
    /// Source bytes. Stored inline in B9; B10 may externalise large
    /// images to a project sub-folder.
    pub bytes: Vec<u8>,
    /// MIME type hint. `image/png` is the common case.
    pub mime: String,
}

/// Built-in reference-sheet templates exposed by core/UI.
#[derive(
    Copy, Clone, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, TS,
)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ReferenceSheetTemplateId {
    /// Four-view turnaround for character sheets.
    #[default]
    Turnaround4View,
    /// Eight-direction top-down turnaround.
    Turnaround8Direction,
    /// Facial expression sheet.
    ExpressionSheet,
    /// Action pose sheet.
    ActionPoses,
    /// Labeled turnaround for text-heavy outputs.
    TypographicTurnaround,
    /// Legacy character template identifier.
    Character,
    /// Legacy item template identifier.
    Item,
    /// Legacy tileset template identifier.
    Tileset,
    /// User-authored/custom template.
    Custom,
}

/// Width/height pair for template-controlled sheet output sizes.
#[allow(missing_docs)]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SheetDimensions {
    pub width: u32,
    pub height: u32,
}

/// Core/UI-facing template definition. Backend-only prompt fragments stay
/// in `pixhaus-ai`.
#[allow(missing_docs)]
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ReferenceSheetTemplateDefinition {
    pub id: ReferenceSheetTemplateId,
    pub label: String,
    pub allowed_dimensions: Vec<SheetDimensions>,
    pub default_dimensions: SheetDimensions,
    pub default_chroma: Rgba,
    pub benefits_from_text_labels: bool,
}

/// Returns the built-in reference-sheet template definitions.
#[must_use]
pub fn built_in_reference_sheet_templates() -> Vec<ReferenceSheetTemplateDefinition> {
    use ReferenceSheetTemplateId::{
        ActionPoses, ExpressionSheet, Turnaround4View, Turnaround8Direction, TypographicTurnaround,
    };

    vec![
        template_definition(
            Turnaround4View,
            "Turnaround (4-view)",
            &[(2048, 1024), (1024, 512), (2560, 1280), (1536, 1024)],
            0,
            false,
        ),
        template_definition(
            Turnaround8Direction,
            "Turnaround (8-direction)",
            &[(2048, 1024), (1024, 512), (3072, 1024)],
            0,
            false,
        ),
        template_definition(
            ExpressionSheet,
            "Expression sheet",
            &[(1024, 1024), (1536, 1024), (2048, 2048)],
            0,
            false,
        ),
        template_definition(
            ActionPoses,
            "Action poses",
            &[(2048, 1024), (1024, 512), (3072, 1024)],
            0,
            false,
        ),
        template_definition(
            TypographicTurnaround,
            "Typographic turnaround",
            &[(2048, 1024), (1536, 1024), (2560, 1280)],
            0,
            true,
        ),
    ]
}

fn template_definition(
    id: ReferenceSheetTemplateId,
    label: &str,
    dims: &[(u32, u32)],
    default_index: usize,
    benefits_from_text_labels: bool,
) -> ReferenceSheetTemplateDefinition {
    let allowed_dimensions = dims
        .iter()
        .map(|&(width, height)| SheetDimensions { width, height })
        .collect::<Vec<_>>();
    let default_dimensions =
        allowed_dimensions
            .get(default_index)
            .copied()
            .unwrap_or(SheetDimensions {
                width: 2048,
                height: 1024,
            });
    ReferenceSheetTemplateDefinition {
        id,
        label: label.into(),
        allowed_dimensions,
        default_dimensions,
        default_chroma: default_reference_chroma(),
        benefits_from_text_labels,
    }
}

fn default_reference_template() -> ReferenceSheetTemplateId {
    ReferenceSheetTemplateId::Turnaround4View
}

fn default_sheet_width() -> u32 {
    2048
}

fn default_sheet_height() -> u32 {
    1024
}

fn default_lora_weight() -> f32 {
    1.0
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_default_lora_weight(weight: &f32) -> bool {
    (*weight - 1.0).abs() < f32::EPSILON
}

/// Reads an encoded image's pixel dimensions from its header without a full
/// decode. Returns `None` when the bytes aren't a recognizable image.
fn image_dimensions(image: &ReferenceImage) -> Option<(u32, u32)> {
    use std::io::Cursor;

    image::ImageReader::new(Cursor::new(&image.bytes))
        .with_guessed_format()
        .ok()?
        .into_dimensions()
        .ok()
}

/// Role hint for one reference image slot.
#[allow(missing_docs)]
#[derive(
    Copy, Clone, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, TS,
)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ReferenceRole {
    Subject,
    Style,
    Pose,
    Outfit,
    Context,
    #[default]
    Generic,
}

/// One ordered reference image supplied to a generation/refinement request.
#[allow(missing_docs)]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ReferenceSlot {
    pub image: ReferenceImage,
    #[serde(default)]
    pub role: ReferenceRole,
    #[serde(default = "default_lora_weight")]
    pub weight: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_asset_id: Option<AssetId>,
}

/// Source operation that produced a sheet variant.
#[allow(missing_docs)]
#[derive(
    Copy, Clone, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, TS,
)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum VariantOrigin {
    #[default]
    FreshGeneration,
    Refinement,
    ChatTurn,
    Promotion,
    CrossModelGrid,
    ManualImport,
}

/// Region used by regional reference refinement.
#[allow(missing_docs)]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct RegionDefinition {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub polygon: Vec<IVec2>,
    pub prompt: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub region_references: Vec<ReferenceSlot>,
}

/// Refinement metadata stored on variants produced by refinement.
#[allow(missing_docs)]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
#[ts(export)]
pub enum RefinementKind {
    Masked { mask_png: ReferenceImage },
    PromptOnly,
    Regional { regions: Vec<RegionDefinition> },
}

/// Full chat transcript stored for conversational variant provenance.
#[allow(missing_docs)]
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ChatTranscript {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub turns: Vec<ChatTurn>,
}

/// One conversational editing turn and the variant it produced.
#[allow(missing_docs)]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ChatTurn {
    #[ts(type = "number")]
    pub timestamp: i64,
    pub user_message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mask: Option<ReferenceImage>,
    pub resulting_variant_id: SheetVariantId,
}

/// A structured asset reference sheet: the canonical AI anchor and all
/// drafts/history for one sprite entity.
///
/// Draft-only sheets are valid: `canonical` is `None` until the user
/// approves a generated or imported variant. AI verbs only use approved
/// canonical sheets as anchors; variants in `variants` are retained as
/// candidates/history and never anchor generation on their own.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReferenceSheet {
    /// The current canonical variant — the user-approved sheet. `None`
    /// means candidates exist, but none has been approved as the AI
    /// consistency anchor.
    #[serde(default)]
    pub canonical: Option<SheetVariant>,

    /// Drafts and previous canonicals, newest first. The canonical
    /// variant is stored only in `canonical`, not duplicated here.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub variants: Vec<SheetVariant>,

    /// Legacy prompt log retained for pre-v1 command compatibility. New
    /// provenance lives directly on [`SheetVariant`].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub prompts: Vec<PromptEntry>,

    /// Legacy structured metadata retained for existing project commands.
    /// New reusable metadata belongs in
    /// [`AssetLibrary`](super::AssetLibrary).
    #[serde(default, skip_serializing_if = "AssetInfo::is_empty")]
    pub info: AssetInfo,
}

impl TS for ReferenceSheet {
    type WithoutGenerics = Self;
    type OptionInnerType = Self;

    fn name(_: &Config) -> String {
        "ReferenceSheet".into()
    }

    fn inline(_: &Config) -> String {
        // Keep this in sync with the struct fields below. `prompts` and
        // `info` are serialized (with `skip_serializing_if`, hence optional)
        // and read by the frontend, so they must appear in the TS shape.
        r"{
canonical: SheetVariant | null,
variants?: Array<SheetVariant>,
prompts?: Array<PromptEntry>,
info?: AssetInfo,
}"
        .into()
    }

    fn decl(cfg: &Config) -> String {
        format!("type {} = {};", Self::name(cfg), Self::inline(cfg))
    }

    fn decl_concrete(cfg: &Config) -> String {
        Self::decl(cfg)
    }

    fn visit_dependencies(v: &mut impl TypeVisitor)
    where
        Self: 'static,
    {
        v.visit::<SheetVariant>();
        v.visit::<PromptEntry>();
        v.visit::<AssetInfo>();
    }

    fn output_path() -> Option<PathBuf> {
        Some(PathBuf::from("ReferenceSheet.ts"))
    }
}

/// One generated/imported/refined version of a reference sheet.
///
/// The raster image is always embedded. Optional vector output is stored
/// as another [`ReferenceImage`] with `mime = "image/svg+xml"`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SheetVariant {
    /// Stable identifier within the parent [`ReferenceSheet`].
    pub id: SheetVariantId,
    /// UTC seconds-since-epoch at which this variant was generated or
    /// imported.
    #[ts(type = "number")]
    pub created_at: i64,
    /// Layout template used to generate this sheet.
    #[serde(default = "default_reference_template")]
    pub template: ReferenceSheetTemplateId,
    /// Output raster width.
    #[serde(default = "default_sheet_width")]
    pub width: u32,
    /// Output raster height.
    #[serde(default = "default_sheet_height")]
    pub height: u32,
    /// Flat chroma-key background color requested for the model.
    #[serde(default = "default_reference_chroma")]
    pub chroma_color: Rgba,
    /// User-typed prompt for this operation.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub user_prompt: String,
    /// Full composed prompt after style notes/template/reference hints.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub composed_prompt: String,
    /// Ordered generation references with role hints.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub references: Vec<ReferenceSlot>,
    /// Whether Google Search grounding was requested.
    #[serde(default)]
    pub real_world_grounding: bool,
    /// Applied `LoRA` asset, Flux-only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub applied_lora: Option<AssetId>,
    /// Applied `LoRA` strength for Flux requests.
    #[serde(
        default = "default_lora_weight",
        skip_serializing_if = "is_default_lora_weight"
    )]
    pub lora_weight: f32,
    /// The composite sheet image.
    pub image: ReferenceImage,
    /// Optional vectorized SVG output.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vector_image: Option<ReferenceImage>,
    /// Model that produced this variant.
    #[serde(default)]
    pub model: ModelId,
    /// Quality tier used for the run.
    #[serde(default)]
    pub quality: Quality,
    /// Parent/source variant for refinements, chat turns, promotions, and
    /// cross-model comparisons.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_variant_id: Option<SheetVariantId>,
    /// Operation origin.
    #[serde(default)]
    pub origin: VariantOrigin,
    /// Refinement-specific metadata.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refinement: Option<RefinementKind>,
    /// Full chat transcript for chat-generated variants.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chat_transcript: Option<ChatTranscript>,
    /// True when produced by "Promote to final".
    #[serde(default)]
    pub promotion: bool,
    /// Actual cost recorded after completion, if known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_usd: Option<f64>,
    /// What's in the sheet, panel by panel. Empty in B9; B10 fills it.
    #[serde(default, skip_serializing_if = "SheetComposition::is_empty")]
    pub composition: SheetComposition,
    /// Generation provenance. `None` in B9 for user-uploaded references;
    /// B10's generator populates these.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generation: Option<GenerationProvenance>,
    /// Palette extracted from the sheet image. Empty in B9; B10's
    /// generator runs eyedropper extraction at sheet-creation time.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extracted_palette: Vec<PaletteEntry>,
}

impl SheetVariant {
    /// Builds a variant with PRD v1 defaults from an embedded image.
    #[must_use]
    pub fn from_image(id: SheetVariantId, created_at: i64, image: ReferenceImage) -> Self {
        // Prefer the real encoded dimensions over the template defaults so
        // manual imports of non-default-sized images carry correct metadata.
        // Falls back to the defaults when the bytes aren't a decodable image.
        let (width, height) =
            image_dimensions(&image).unwrap_or((default_sheet_width(), default_sheet_height()));
        Self {
            id,
            created_at,
            template: ReferenceSheetTemplateId::Custom,
            width,
            height,
            chroma_color: default_reference_chroma(),
            user_prompt: String::new(),
            composed_prompt: String::new(),
            references: Vec::new(),
            real_world_grounding: false,
            applied_lora: None,
            lora_weight: default_lora_weight(),
            image,
            vector_image: None,
            model: ModelId::Auto,
            quality: Quality::Medium,
            parent_variant_id: None,
            origin: VariantOrigin::ManualImport,
            refinement: None,
            chat_transcript: None,
            promotion: false,
            cost_usd: None,
            composition: SheetComposition::default(),
            generation: None,
            extracted_palette: Vec::new(),
        }
    }
}

/// Panel rectangles within a sheet image, labelled by what they show.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SheetComposition {
    /// Full-body view rectangles: front, side, three-quarter, back.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub views: Vec<SheetPanel>,
    /// Facial expression panels: happy, angry, surprised, ...
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub expressions: Vec<SheetPanel>,
    /// Detail close-ups: scars, accessories, tattoos, runes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub callouts: Vec<SheetPanel>,
    /// Outfit / equipment variations.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub outfits: Vec<SheetPanel>,
    /// Palette swatch rectangle, if the sheet includes one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub palette_swatch: Option<Rect>,
}

impl SheetComposition {
    /// Returns `true` when no panel rectangles or palette swatch are
    /// recorded.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.views.is_empty()
            && self.expressions.is_empty()
            && self.callouts.is_empty()
            && self.outfits.is_empty()
            && self.palette_swatch.is_none()
    }
}

/// One labelled panel within a sheet.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SheetPanel {
    /// Rectangle within the sheet image.
    pub region: Rect,
    /// Semantic label: `front`, `side-left`, `happy`, `scar-over-eye`.
    pub label: String,
}

/// Provenance for a generated sheet variant.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct GenerationProvenance {
    /// Backend that produced the sheet (e.g. `anthropic`, `stability`,
    /// `replicate`).
    pub backend: String,
    /// Model identifier used for the run.
    pub model: String,
    /// The prompt that produced this variant.
    pub prompt: String,
    /// Seed for reproducible regeneration. `None` if the backend does
    /// not expose seeds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<u64>,
    /// Negative prompt, if used.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub negative_prompt: Option<String>,
}

/// Free-form structured metadata for an asset.
///
/// Common keys: `name`, `age`, `species`, `era`, `faction`. Open-ended;
/// artists capture whatever matters.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AssetInfo {
    /// Free-form keyed metadata.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub fields: BTreeMap<String, String>,
    /// Personality / behaviour notes — bullets shown in the sheet panel.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
}

impl AssetInfo {
    /// Returns `true` when no fields and no notes are recorded.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.fields.is_empty() && self.notes.is_empty()
    }
}

/// A prompt issued against a reference sheet.
///
/// Defined minimally for B9 — `prompt` plus optional `negative_prompt`
/// and `seed`, with the result the prompt produced. B10's sheet
/// generator extends this with backend-specific knobs (steps, CFG
/// scale, sampler) as the verb wiring lands.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PromptEntry {
    /// The prompt text.
    pub prompt: String,
    /// Optional negative prompt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub negative_prompt: Option<String>,
    /// Optional seed for reproducible regeneration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<u64>,
    /// What the prompt produced. `None` if the prompt is queued but not
    /// yet run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<PromptResult>,
    /// UTC seconds-since-epoch at which the prompt was issued.
    #[ts(type = "number")]
    pub issued_at: i64,
}

/// The outcome of a [`PromptEntry`].
///
/// Defined minimally for B9; B10 extends this as the streaming verb
/// runtime grows new shape (intermediate previews, multiple samples,
/// failure detail).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum PromptResult {
    /// The prompt produced a sheet variant; the id refers to a variant
    /// stored in the parent [`ReferenceSheet::canonical`] or
    /// [`ReferenceSheet::variants`].
    Variant(SheetVariantId),
    /// The prompt failed; the string is the user-facing error message.
    Error(String),
}
