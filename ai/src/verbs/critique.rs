//! Verb: Critique — vision-language quality analysis of a sprite.
//!
//! Sends the active sprite's metadata, palette, and any reference images to
//! a VLM (Claude or GPT-4o) and surfaces structured findings in five
//! categories: pose continuity, palette violations, missing frames, pivot
//! drift, and style inconsistency.
//!
//! # Protocol contract
//!
//! - **Required capability:** `VISION_LANGUAGE`
//! - **Output kind:** `Critique` (quality analysis) or `SuggestEntityTags` (library auto-tag)
//! - **Inputs:** [`CritiqueInputs`] — mode, which checks to run, optional notes
//! - **Output:** [`VerbOutput`] with one `VerbEffect::Critique { findings }` or
//!   one `VerbEffect::SuggestEntityTags { entity_id, tag_names }`
//!
//! # Operating modes
//!
//! `CritiqueInputs::mode` selects between two behaviors:
//!
//! - **`QualityAnalysis`** (default): returns structured sprite quality findings.
//!   Same behavior as before B9.4.
//! - **`LibraryAutoTag`**: returns descriptive tag suggestions for a library
//!   entity. The VLM reads the reference images attached by the host (composited
//!   from the entity's primary state) and produces a flat list of descriptive tag
//!   names (style, subject, color, mood). The host creates `TagDefinition` entries
//!   and populates `Entity.ai.suggested_tags`.
//!
//! # Image supply
//!
//! The verb reads pixel data from `VerbContext::references`. The caller
//! (app IPC command) is responsible for compositing the active sprite's
//! visible layers into PNG bytes and attaching them as `ReferenceImage`
//! entries before invoking the verb. If no reference images are present,
//! the verb produces a metadata-only analysis.

use std::time::{Duration, Instant};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;
use tracing::instrument;

use pixhaus_core::project::{EntityId, FrameIndex, LayerId};

use crate::plugin::context::VerbContext;
use crate::plugin::descriptor::{
    BackendCapabilities, CostEstimate, EffectKind, VerbDescriptor, VerbId,
};
use crate::plugin::error::{Result, VerbError};
use crate::plugin::inputs::VerbInputs;
use crate::plugin::output::{
    ActualCost, CritiqueCategory, CritiqueFinding, CritiqueSeverity, VerbEffect, VerbOutput,
};
use crate::plugin::progress::{VerbProgress, VerbProgressEvent};
use crate::plugin::verb::Verb;

use super::call_text_vlm;

/// Stable identifier for the built-in critique verb.
pub const CRITIQUE_VERB_ID: &str = "pixhaus.builtin.critique";

/// Operating mode for the Critique verb.
///
/// Defaults to [`QualityAnalysis`] (the pre-B9.4 behavior) when absent from
/// the input payload so existing callers do not need to be updated.
///
/// [`QualityAnalysis`]: CritiqueMode::QualityAnalysis
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CritiqueMode {
    /// Standard sprite quality analysis. Returns `VerbEffect::Critique`.
    /// This is the default mode and the only one that uses `checks`.
    #[default]
    QualityAnalysis,
    /// Library auto-tagging. Returns `VerbEffect::SuggestEntityTags`.
    ///
    /// The host composites the entity's primary state into PNG bytes,
    /// attaches them as `VerbContext::references`, and sets
    /// `VerbContext::library_entity_id` before invoking. The verb asks
    /// the VLM for a flat list of descriptive tags and returns them so
    /// the host can populate `Entity.ai.suggested_tags`.
    LibraryAutoTag {
        /// Entity to tag. Must match `VerbContext::library_entity_id`.
        entity_id: EntityId,
        /// Free-form summary the host attaches when no sprite reference
        /// is available — entity name, kind, existing tags. Anything
        /// useful for grounding the VLM. Optional; when present and
        /// `ctx.references` is empty, the verb feeds this into the
        /// user prompt instead of asking the model to tag blind.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        entity_metadata: Option<String>,
    },
}

/// Which quality checks the verb should perform.
///
/// An empty `checks` list in [`CritiqueInputs`] means "all categories".
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CritiqueCheck {
    /// Frame-to-frame pose continuity breaks.
    PoseContinuity,
    /// Pixels that use colours outside the active palette.
    PaletteViolations,
    /// Missing or implausibly-timed frames in an animation.
    MissingFrames,
    /// Pivot point drift across animation frames.
    PivotDrift,
    /// Style inconsistency relative to project references.
    StyleInconsistency,
}

/// Inputs accepted by [`CritiqueVerb`].
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CritiqueInputs {
    /// Operating mode. Defaults to `QualityAnalysis` when absent.
    #[serde(default)]
    pub mode: CritiqueMode,
    /// Categories to check. Applies only in `QualityAnalysis` mode.
    /// Empty means all five categories.
    #[serde(default)]
    pub checks: Vec<CritiqueCheck>,
    /// Optional context or guidance for the VLM (max ~500 chars).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

/// VLM response shape for `QualityAnalysis` mode.
#[derive(Deserialize)]
struct RawFinding {
    category: String,
    severity: String,
    summary: String,
    #[serde(default)]
    frame: Option<u32>,
    #[serde(default)]
    layer: Option<u32>,
}

/// VLM response shape for `LibraryAutoTag` mode.
#[derive(Deserialize)]
struct RawTagResponse {
    tags: Vec<String>,
}

/// Built-in critique verb.
///
/// Runs a vision-language analysis of the active sprite and surfaces
/// structured findings. Uses any VLM backend registered with the runtime
/// that declares [`BackendCapabilities::VISION_LANGUAGE`].
#[derive(Debug)]
pub struct CritiqueVerb {
    descriptor: VerbDescriptor,
}

impl CritiqueVerb {
    /// Constructs the critique verb.
    #[must_use]
    // `serde_json::json!` internally calls `.unwrap()` on infallible builders.
    #[allow(clippy::disallowed_methods)]
    pub fn new() -> Self {
        let input_schema = serde_json::json!({
            "type": "object",
            "properties": {
                "mode": {
                    "oneOf": [
                        { "type": "object", "properties": { "kind": { "const": "quality_analysis" } }, "required": ["kind"] },
                        {
                            "type": "object",
                            "properties": {
                                "kind": { "const": "library_auto_tag" },
                                "entity_id": { "type": "integer", "minimum": 0 }
                            },
                            "required": ["kind", "entity_id"]
                        }
                    ]
                },
                "checks": {
                    "type": "array",
                    "items": {
                        "type": "string",
                        "enum": [
                            "pose_continuity",
                            "palette_violations",
                            "missing_frames",
                            "pivot_drift",
                            "style_inconsistency"
                        ]
                    },
                    "default": []
                },
                "notes": {
                    "type": ["string", "null"]
                }
            }
        });

        Self {
            descriptor: VerbDescriptor {
                id: VerbId::new(CRITIQUE_VERB_ID),
                display_name: "Critique".into(),
                description: "Visual quality analysis and library auto-tagging via VLM".into(),
                version: env!("CARGO_PKG_VERSION").into(),
                required_capabilities: BackendCapabilities::VISION_LANGUAGE,
                input_schema,
                output_schema: None,
                output_kinds: vec![
                    EffectKind::Critique,
                    EffectKind::Custom("pixhaus.builtin.critique.suggest_tags".into()),
                ],
                cost_estimate: CostEstimate {
                    typical_latency: Duration::from_secs(10),
                    max_latency: Duration::from_secs(60),
                    typical_usd_cents: 0.5,
                    max_usd_cents: 5.0,
                },
                streaming: true,
                cancellable: true,
                documentation_url: Some("docs/verbs/critique.md".into()),
            },
        }
    }
}

impl Default for CritiqueVerb {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Verb for CritiqueVerb {
    fn descriptor(&self) -> &VerbDescriptor {
        &self.descriptor
    }

    fn validate(&self, inputs: &VerbInputs) -> Result<()> {
        inputs.deserialize::<CritiqueInputs>().map(|_| ())
    }

    #[instrument(skip_all, fields(verb = CRITIQUE_VERB_ID))]
    async fn invoke(
        &self,
        ctx: VerbContext,
        inputs: VerbInputs,
        progress: VerbProgress,
        cancel: CancellationToken,
    ) -> Result<VerbOutput> {
        let inputs: CritiqueInputs = inputs.deserialize_owned()?;
        match inputs.mode.clone() {
            CritiqueMode::QualityAnalysis => {
                invoke_quality_analysis(ctx, inputs, progress, cancel).await
            }
            CritiqueMode::LibraryAutoTag {
                entity_id,
                entity_metadata,
            } => {
                invoke_library_auto_tag(entity_id, entity_metadata, ctx, inputs, progress, cancel)
                    .await
            }
        }
    }
}

// ── Mode dispatch ────────────────────────────────────────────────────────────

async fn invoke_quality_analysis(
    ctx: VerbContext,
    inputs: CritiqueInputs,
    progress: VerbProgress,
    cancel: CancellationToken,
) -> Result<VerbOutput> {
    let started = Instant::now();

    let backend = ctx
        .backend
        .as_ref()
        .ok_or(VerbError::MissingContext("VLM backend"))?;

    progress
        .send(VerbProgressEvent::Started {
            backend: Some(backend.id().into()),
        })
        .await;
    progress.step(Some(0.1), "building analysis prompt").await;

    if cancel.is_cancelled() {
        return Err(VerbError::Cancelled);
    }

    let request = build_request(&ctx, &inputs);

    progress.step(Some(0.2), "sending to VLM").await;

    if cancel.is_cancelled() {
        return Err(VerbError::Cancelled);
    }

    let response =
        call_text_vlm(backend.as_ref(), request, progress.clone(), cancel.clone()).await?;

    progress.step(Some(0.9), "parsing findings").await;

    if cancel.is_cancelled() {
        return Err(VerbError::Cancelled);
    }

    let findings = parse_findings(&response.content)?;
    let count = findings.len();

    progress.step(Some(1.0), "done").await;

    let elapsed = started.elapsed();
    let summary = if count == 0 {
        "No issues found".into()
    } else {
        format!("{count} finding{}", if count == 1 { "" } else { "s" })
    };

    Ok(VerbOutput {
        summary,
        effects: vec![VerbEffect::Critique { findings }],
        thumbnail: None,
        actual_cost: ActualCost {
            elapsed,
            #[allow(clippy::cast_possible_truncation)]
            usd_cents: (f64::from(response.input_tokens) * (3.0 / 1_000_000.0 * 100.0)
                + f64::from(response.output_tokens) * (15.0 / 1_000_000.0 * 100.0))
                as f32,
            backend: Some(response.model),
            tokens_input: Some(response.input_tokens),
            tokens_output: Some(response.output_tokens),
        },
        notes: vec![],
    })
}

async fn invoke_library_auto_tag(
    entity_id: EntityId,
    entity_metadata: Option<String>,
    ctx: VerbContext,
    inputs: CritiqueInputs,
    progress: VerbProgress,
    cancel: CancellationToken,
) -> Result<VerbOutput> {
    if ctx.library_entity_id != Some(entity_id) {
        return Err(VerbError::Schema(format!(
            "library_auto_tag: ctx.library_entity_id ({:?}) does not match \
             inputs.entity_id ({})",
            ctx.library_entity_id,
            entity_id.get(),
        )));
    }
    // The VLM needs *something* to ground its suggestions: either a
    // sprite reference image or a metadata summary. Both empty means
    // the host called us with no context, which would produce
    // hallucinated tags — fail-fast so the caller can fix the call.
    if ctx.references.is_empty() && entity_metadata.is_none() {
        return Err(VerbError::Schema(
            "library_auto_tag: needs either a sprite reference image \
             (ctx.references) or `entity_metadata`; both are empty"
                .into(),
        ));
    }
    let started = Instant::now();

    let backend = ctx
        .backend
        .as_ref()
        .ok_or(VerbError::MissingContext("VLM backend"))?;

    progress
        .send(VerbProgressEvent::Started {
            backend: Some(backend.id().into()),
        })
        .await;
    progress
        .step(Some(0.1), "building tag-suggestion prompt")
        .await;

    if cancel.is_cancelled() {
        return Err(VerbError::Cancelled);
    }

    let request = build_auto_tag_request(&ctx, &inputs, entity_metadata.as_deref());

    progress.step(Some(0.2), "sending to VLM").await;

    if cancel.is_cancelled() {
        return Err(VerbError::Cancelled);
    }

    let response =
        call_text_vlm(backend.as_ref(), request, progress.clone(), cancel.clone()).await?;

    progress.step(Some(0.9), "parsing tags").await;

    if cancel.is_cancelled() {
        return Err(VerbError::Cancelled);
    }

    let tag_names = parse_tag_response(&response.content)?;
    let count = tag_names.len();

    progress.step(Some(1.0), "done").await;

    let elapsed = started.elapsed();
    Ok(VerbOutput {
        summary: format!(
            "{count} tag suggestion{} for entity",
            if count == 1 { "" } else { "s" }
        ),
        effects: vec![VerbEffect::SuggestEntityTags {
            entity_id,
            tag_names,
        }],
        thumbnail: None,
        actual_cost: ActualCost {
            elapsed,
            #[allow(clippy::cast_possible_truncation)]
            usd_cents: (f64::from(response.input_tokens) * (3.0 / 1_000_000.0 * 100.0)
                + f64::from(response.output_tokens) * (15.0 / 1_000_000.0 * 100.0))
                as f32,
            backend: Some(response.model),
            tokens_input: Some(response.input_tokens),
            tokens_output: Some(response.output_tokens),
        },
        notes: vec![
            "Suggested tags are stored in Entity.ai.suggested_tags pending user review.".into(),
        ],
    })
}

// ── Prompt construction ─────────────────────────────────────────────────────

fn build_request(ctx: &VerbContext, inputs: &CritiqueInputs) -> crate::backends::TextGenRequest {
    use crate::backends::{ChatMessage, ContentPart, TextGenRequest};

    let system = build_system_prompt(inputs);
    let user_text = build_user_text(ctx, inputs);

    let mut user_content = vec![ContentPart::text(user_text)];

    // Attach reference images so the VLM can perform visual analysis.
    // Each reference image becomes a PNG content part.
    for reference in &ctx.references {
        // Reconstruct a minimal PNG-like representation from the PixelData.
        // In a full implementation the caller composites actual PNG bytes; here
        // we include raw bytes with a PNG media type. The VLM sees the pixels.
        let bytes = encode_pixel_data_as_png(&reference.pixels);
        user_content.push(ContentPart::png(bytes));
    }

    let mut req = TextGenRequest {
        model: None,
        system: Some(system),
        messages: vec![ChatMessage {
            role: crate::backends::ChatRole::User,
            content: user_content,
        }],
        max_tokens: Some(2048),
        temperature: Some(0.2), // low temp for consistent structured output
        tools: Vec::new(),
        stop: Vec::new(),
    };

    // Ask Claude to stop after the JSON array closes so we don't get trailing prose.
    req.stop = vec!["]\n\n".into()];
    req
}

fn build_system_prompt(inputs: &CritiqueInputs) -> String {
    let checks = if inputs.checks.is_empty() {
        "all five categories".into()
    } else {
        inputs
            .checks
            .iter()
            .map(|c| match c {
                CritiqueCheck::PoseContinuity => "pose continuity",
                CritiqueCheck::PaletteViolations => "palette violations",
                CritiqueCheck::MissingFrames => "missing frames",
                CritiqueCheck::PivotDrift => "pivot drift",
                CritiqueCheck::StyleInconsistency => "style inconsistency",
            })
            .collect::<Vec<_>>()
            .join(", ")
    };

    format!(
        "You are a pixel art quality analyst. Analyze the provided sprite metadata and \
         images for technical issues in: {checks}.\n\
         \n\
         Return ONLY a JSON array (no prose, no markdown fences). Each element has:\n\
         - category: one of \"pose_continuity\", \"palette_violation\", \
           \"missing_frame\", \"pivot_drift\", \"style_inconsistency\", \"other\"\n\
         - severity: \"info\", \"warning\", or \"error\"\n\
         - summary: one sentence describing the specific issue\n\
         - frame: 0-based frame index (integer or null)\n\
         - layer: 0-based layer index (integer or null)\n\
         \n\
         Focus only on objectively observable technical issues. Skip aesthetic \
         preferences. If no issues are found, return []."
    )
}

fn build_user_text(ctx: &VerbContext, inputs: &CritiqueInputs) -> String {
    use std::fmt::Write as _;

    let mut out = String::new();

    // Sprite metadata
    if let Some(sprite) = &ctx.sprite {
        let _ = write!(
            out,
            "Sprite: \"{}\"\nDimensions: {}x{}\nFrames: {}\nLayers: {}\n",
            sprite.name,
            sprite.canvas.width,
            sprite.canvas.height,
            sprite.frames.len(),
            sprite.layers.len(),
        );

        // Frame durations
        if !sprite.frames.is_empty() {
            let durations: Vec<String> = sprite
                .frames
                .iter()
                .map(|f| format!("{}ms", f.duration_ms))
                .collect();
            let _ = writeln!(out, "Frame durations: {}", durations.join(", "));
        }
    } else {
        out.push_str("No sprite context provided.\n");
    }

    // Active palette
    if let Some(palette) = &ctx.active_palette {
        let colors: Vec<String> = palette
            .colors
            .iter()
            .take(64) // cap at 64 to keep prompt size bounded
            .map(|e| {
                format!(
                    "#{:02X}{:02X}{:02X}{:02X}",
                    e.color.r, e.color.g, e.color.b, e.color.a
                )
            })
            .collect();
        let _ = writeln!(
            out,
            "Active palette ({} colors): {}",
            palette.colors.len(),
            colors.join(" ")
        );
    }

    // Reference images
    if ctx.references.is_empty() {
        out.push_str("No reference images provided; base analysis on metadata only.\n");
    } else {
        let _ = writeln!(
            out,
            "{} reference image(s) attached above.",
            ctx.references.len()
        );
    }

    // User notes
    if let Some(notes) = &inputs.notes {
        let _ = writeln!(out, "\nAdditional context from artist: {notes}");
    }

    out.push_str("\nReturn findings as a JSON array.");
    out
}

fn build_auto_tag_request(
    ctx: &VerbContext,
    inputs: &CritiqueInputs,
    entity_metadata: Option<&str>,
) -> crate::backends::TextGenRequest {
    use crate::backends::{ChatMessage, ContentPart, TextGenRequest};

    let system = "You are a pixel art cataloguer. Analyze the provided sprite image and \
                  return a JSON object with a single key \"tags\" whose value is an array \
                  of descriptive strings. Focus on: visual style (e.g. \"fantasy\", \
                  \"sci-fi\"), subject (e.g. \"character\", \"animal\", \"vehicle\"), \
                  color palette mood (e.g. \"warm\", \"dark\", \"pastel\"), animation \
                  state (e.g. \"idle\", \"walk\", \"attack\"), and other artist-useful \
                  descriptors. Produce 5–15 tags. Use lowercase, single words or short \
                  hyphenated phrases. Return ONLY the JSON object, no prose."
        .into();

    let mut user_content = Vec::new();

    if ctx.references.is_empty() {
        // The invoke-side guard ensures `entity_metadata` is Some when
        // references are empty; treat the unwrap as a contract check.
        let metadata = entity_metadata.unwrap_or("(no metadata supplied)");
        user_content.push(ContentPart::text(format!(
            "No image is available. Return tags based on this entity metadata:\n\n{metadata}"
        )));
    } else {
        user_content.push(ContentPart::text(
            "Analyze the sprite image below and return descriptive tags as JSON.".to_owned(),
        ));
        for reference in &ctx.references {
            let bytes = encode_pixel_data_as_png(&reference.pixels);
            user_content.push(ContentPart::png(bytes));
        }
        if let Some(metadata) = entity_metadata {
            user_content.push(ContentPart::text(format!(
                "Entity metadata for additional context:\n\n{metadata}"
            )));
        }
    }

    if let Some(notes) = &inputs.notes {
        user_content.push(ContentPart::text(format!(
            "Additional context from artist: {notes}"
        )));
    }

    TextGenRequest {
        model: None,
        system: Some(system),
        messages: vec![ChatMessage {
            role: crate::backends::ChatRole::User,
            content: user_content,
        }],
        max_tokens: Some(256),
        temperature: Some(0.3),
        tools: Vec::new(),
        stop: Vec::new(),
    }
}

/// Parses the VLM's `{"tags": [...]}` response into a `Vec<String>`.
///
/// Tolerates markdown fences and leading/trailing prose. Returns an empty
/// vec (not an error) when the VLM returns a valid but empty tag list.
fn parse_tag_response(raw: &str) -> Result<Vec<String>> {
    let raw = raw.trim();
    // Strip optional ```json / ``` fences.
    let raw = if let Some(inner) = raw
        .strip_prefix("```json")
        .or_else(|| raw.strip_prefix("```"))
    {
        inner.trim_start()
    } else {
        raw
    };
    let raw = raw.trim_end_matches("```").trim_end();

    // Find the outermost `{...}` object.
    let raw = match (raw.find('{'), raw.rfind('}')) {
        (Some(start), Some(end)) if end >= start => &raw[start..=end],
        _ => raw,
    };

    let parsed: RawTagResponse = serde_json::from_str(raw).map_err(|e| {
        VerbError::Schema(format!(
            "VLM auto-tag response is not valid JSON: {e}. Raw: {raw}"
        ))
    })?;

    // Normalise: lowercase, trim whitespace, drop empties.
    Ok(parsed
        .tags
        .into_iter()
        .map(|t| t.trim().to_lowercase())
        .filter(|t| !t.is_empty())
        .collect())
}

/// Converts raw RGBA8 `PixelData` to a PNG byte stream for VLM consumption.
///
/// Falls back to a 1×1 transparent PNG sentinel when the pixel data is not
/// well-formed RGBA8, so the VLM call still proceeds without aborting.
fn encode_pixel_data_as_png(pixels: &crate::plugin::context::PixelData) -> Vec<u8> {
    use image::{ImageBuffer, RgbaImage};
    use std::io::Cursor;

    if pixels.bytes_per_pixel != 4 || !pixels.is_well_formed() {
        return TRANSPARENT_1X1_PNG.to_vec();
    }

    let w = pixels.width;
    let h = pixels.height;
    let stride = pixels.stride as usize;
    let row_bytes = (w * 4) as usize;

    // Collect the tightly-packed pixel bytes, stripping any row padding.
    let mut flat = Vec::with_capacity(row_bytes * h as usize);
    for y in 0..h as usize {
        flat.extend_from_slice(&pixels.bytes[y * stride..y * stride + row_bytes]);
    }

    let img: Option<RgbaImage> = ImageBuffer::from_raw(w, h, flat);
    let Some(img) = img else {
        return TRANSPARENT_1X1_PNG.to_vec();
    };

    let mut buf = Cursor::new(Vec::new());
    if img.write_to(&mut buf, image::ImageFormat::Png).is_ok() {
        buf.into_inner()
    } else {
        TRANSPARENT_1X1_PNG.to_vec()
    }
}

/// Minimal 1×1 transparent PNG (67 bytes).
static TRANSPARENT_1X1_PNG: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, // PNG signature
    0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52, // IHDR length + type
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, // 1x1
    0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4, 0x89, // 8-bit RGBA
    0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, // IDAT length + type
    0x78, 0x9C, 0x62, 0x00, 0x00, 0x00, 0x02, 0x00, 0x01, // compressed data
    0xE2, 0x21, 0xBC, 0x33, 0x00, 0x00, 0x00, 0x00, // CRC
    0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82, // IEND
];

// ── Response parsing ────────────────────────────────────────────────────────

/// Parses the VLM's JSON-array response into structured findings.
///
/// The model may wrap the array in markdown code fences; this function
/// strips them before parsing. Unrecognised category or severity strings
/// are tolerated rather than rejected — unknown categories become
/// `CritiqueCategory::Other` and unknown severities default to `Info`.
fn parse_findings(raw: &str) -> Result<Vec<CritiqueFinding>> {
    let json_str = extract_json_array(raw);

    let raw_findings: Vec<RawFinding> = serde_json::from_str(json_str)
        .map_err(|e| VerbError::Schema(format!("VLM response is not a valid JSON array: {e}")))?;

    Ok(raw_findings
        .into_iter()
        .map(|rf| CritiqueFinding {
            category: parse_category(&rf.category),
            severity: parse_severity(&rf.severity),
            summary: rf.summary,
            frame: rf.frame.map(FrameIndex::new),
            layer: rf.layer.map(LayerId::new),
            region: None,
        })
        .collect())
}

/// Extracts the outermost JSON array from the model's response, tolerating
/// markdown code fences.
fn extract_json_array(s: &str) -> &str {
    let s = s.trim();
    // Strip optional ```json / ``` fences.
    let s = if let Some(inner) = s.strip_prefix("```json").or_else(|| s.strip_prefix("```")) {
        inner.trim_start()
    } else {
        s
    };
    let s = s.trim_end_matches("```").trim_end();
    // Find the first '[' and last ']'.
    match (s.find('['), s.rfind(']')) {
        (Some(start), Some(end)) if end >= start => &s[start..=end],
        _ => s,
    }
}

fn parse_category(s: &str) -> CritiqueCategory {
    match s {
        "pose_continuity" => CritiqueCategory::PoseContinuity,
        "palette_violation" | "palette_violations" => CritiqueCategory::PaletteViolation,
        "missing_frame" | "missing_frames" => CritiqueCategory::MissingFrame,
        "pivot_drift" => CritiqueCategory::PivotDrift,
        "style_inconsistency" => CritiqueCategory::StyleInconsistency,
        other => CritiqueCategory::Other(other.into()),
    }
}

fn parse_severity(s: &str) -> CritiqueSeverity {
    match s {
        "error" => CritiqueSeverity::Error,
        "warning" => CritiqueSeverity::Warning,
        _ => CritiqueSeverity::Info,
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::context::PixelData;
    use crate::plugin::runtime::VerbRuntime;
    use pixhaus_core::project::{EntityId, ProjectMetadata, SpriteId};

    fn metadata() -> ProjectMetadata {
        ProjectMetadata {
            name: "critique-test".into(),
            description: None,
            author: None,
            created_at: 0,
            updated_at: 0,
            editor_version: "0".into(),
        }
    }

    #[test]
    fn descriptor_declares_vision_language() {
        let verb = CritiqueVerb::new();
        let caps = verb.descriptor().required_capabilities;
        assert!(caps.contains(BackendCapabilities::VISION_LANGUAGE));
        assert!(verb.descriptor().cancellable);
        assert!(verb.descriptor().streaming);
        assert!(
            verb.descriptor()
                .output_kinds
                .contains(&EffectKind::Critique)
        );
    }

    #[test]
    fn validate_accepts_empty_inputs() {
        let verb = CritiqueVerb::new();
        let inputs = VerbInputs::from_struct(&CritiqueInputs {
            mode: CritiqueMode::QualityAnalysis,
            checks: vec![],
            notes: None,
        })
        .unwrap();
        assert!(verb.validate(&inputs).is_ok());
    }

    #[test]
    fn validate_accepts_library_auto_tag_inputs() {
        let verb = CritiqueVerb::new();
        let inputs = VerbInputs::from_struct(&CritiqueInputs {
            mode: CritiqueMode::LibraryAutoTag {
                entity_id: EntityId::new(42),
                entity_metadata: None,
            },
            checks: vec![],
            notes: None,
        })
        .unwrap();
        assert!(verb.validate(&inputs).is_ok());
    }

    #[test]
    fn validate_rejects_malformed_json() {
        let verb = CritiqueVerb::new();
        // checks must be an array; a string is invalid.
        let inputs = VerbInputs::new(serde_json::json!({"checks": "not-an-array"}));
        assert!(verb.validate(&inputs).is_err());
    }

    #[test]
    fn registered_in_runtime() {
        let runtime = VerbRuntime::new();
        runtime.register(CritiqueVerb::new()).unwrap();
        let desc = runtime.descriptor(&VerbId::new(CRITIQUE_VERB_ID));
        assert!(desc.is_some());
        assert_eq!(desc.unwrap().id.as_str(), CRITIQUE_VERB_ID);
    }

    #[test]
    fn parse_findings_empty_array() {
        let findings = parse_findings("[]").unwrap();
        assert!(findings.is_empty());
    }

    #[test]
    fn parse_findings_single_finding() {
        let json = r#"[{"category":"palette_violation","severity":"warning","summary":"Off-palette colour at frame 2","frame":1,"layer":null}]"#;
        let findings = parse_findings(json).unwrap();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].category, CritiqueCategory::PaletteViolation);
        assert_eq!(findings[0].severity, CritiqueSeverity::Warning);
        assert_eq!(findings[0].frame, Some(FrameIndex::new(1)));
        assert!(findings[0].layer.is_none());
    }

    #[test]
    fn parse_findings_strips_markdown_fences() {
        let wrapped = "```json\n[{\"category\":\"pivot_drift\",\"severity\":\"error\",\"summary\":\"drift\",\"frame\":null,\"layer\":null}]\n```";
        let findings = parse_findings(wrapped).unwrap();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].category, CritiqueCategory::PivotDrift);
    }

    #[test]
    fn parse_findings_unknown_category_becomes_other() {
        let json = r#"[{"category":"texture_bleeding","severity":"info","summary":"Bleeding","frame":null,"layer":null}]"#;
        let findings = parse_findings(json).unwrap();
        assert!(matches!(
            &findings[0].category,
            CritiqueCategory::Other(s) if s == "texture_bleeding"
        ));
    }

    #[test]
    fn parse_findings_unknown_severity_becomes_info() {
        let json = r#"[{"category":"other","severity":"critical","summary":"Severe","frame":null,"layer":null}]"#;
        let findings = parse_findings(json).unwrap();
        assert_eq!(findings[0].severity, CritiqueSeverity::Info);
    }

    #[test]
    fn parse_findings_invalid_json_is_schema_error() {
        let err = parse_findings("not json at all").unwrap_err();
        assert!(matches!(err, VerbError::Schema(_)));
    }

    #[test]
    fn parse_tag_response_returns_tags() {
        let json = r#"{"tags":["fantasy","character","blue","armored","pixel-art"]}"#;
        let tags = parse_tag_response(json).unwrap();
        assert_eq!(
            tags,
            vec!["fantasy", "character", "blue", "armored", "pixel-art"]
        );
    }

    #[test]
    fn parse_tag_response_strips_fences() {
        let wrapped = "```json\n{\"tags\":[\"hero\",\"idle\"]}\n```";
        let tags = parse_tag_response(wrapped).unwrap();
        assert_eq!(tags, vec!["hero", "idle"]);
    }

    #[test]
    fn parse_tag_response_normalises_to_lowercase() {
        let json = r#"{"tags":["Fantasy","HERO","Pixel Art"]}"#;
        let tags = parse_tag_response(json).unwrap();
        assert_eq!(tags, vec!["fantasy", "hero", "pixel art"]);
    }

    #[test]
    fn parse_tag_response_drops_empty_tags() {
        let json = r#"{"tags":["hero","","  ","idle"]}"#;
        let tags = parse_tag_response(json).unwrap();
        assert_eq!(tags, vec!["hero", "idle"]);
    }

    #[test]
    fn parse_tag_response_invalid_json_is_schema_error() {
        let err = parse_tag_response("not json").unwrap_err();
        assert!(matches!(err, VerbError::Schema(_)));
    }

    #[test]
    fn critique_mode_defaults_to_quality_analysis() {
        // When mode is absent from the JSON, it should deserialise as QualityAnalysis.
        let json = r#"{"checks":[]}"#;
        let inputs: CritiqueInputs = serde_json::from_str(json).unwrap();
        assert_eq!(inputs.mode, CritiqueMode::QualityAnalysis);
    }

    #[test]
    fn critique_mode_library_auto_tag_round_trips() {
        let inputs = CritiqueInputs {
            mode: CritiqueMode::LibraryAutoTag {
                entity_id: EntityId::new(7),
                entity_metadata: Some("Entity name: Hero\nKind: Custom(Hero)".into()),
            },
            checks: vec![],
            notes: None,
        };
        let json = serde_json::to_string(&inputs).unwrap();
        let back: CritiqueInputs = serde_json::from_str(&json).unwrap();
        assert_eq!(back.mode, inputs.mode);
    }

    #[test]
    fn system_prompt_names_all_checks_when_empty() {
        let inputs = CritiqueInputs {
            mode: CritiqueMode::QualityAnalysis,
            checks: vec![],
            notes: None,
        };
        let prompt = build_system_prompt(&inputs);
        assert!(prompt.contains("all five categories"));
    }

    #[test]
    fn system_prompt_lists_selected_checks() {
        let inputs = CritiqueInputs {
            mode: CritiqueMode::QualityAnalysis,
            checks: vec![CritiqueCheck::PaletteViolations, CritiqueCheck::PivotDrift],
            notes: None,
        };
        let prompt = build_system_prompt(&inputs);
        assert!(prompt.contains("palette violations"));
        assert!(prompt.contains("pivot drift"));
        assert!(!prompt.contains("pose continuity"));
    }

    #[test]
    fn encode_pixel_data_produces_nonempty_bytes() {
        let pixels = PixelData::rgba8(2, 2, vec![0u8; 16]);
        let png = encode_pixel_data_as_png(&pixels);
        assert!(!png.is_empty());
        // PNG signature check.
        assert_eq!(&png[..4], &[0x89, 0x50, 0x4E, 0x47]);
    }

    #[test]
    fn invoke_without_backend_returns_missing_context() {
        // Context has no backend attached (runtime skipped because verb
        // requires a backend but we call invoke directly for isolation).
        let rt = tokio::runtime::Runtime::new().unwrap();
        let verb = CritiqueVerb::new();
        let ctx = VerbContext::empty(metadata());
        let inputs = VerbInputs::from_struct(&CritiqueInputs {
            mode: CritiqueMode::QualityAnalysis,
            checks: vec![],
            notes: None,
        })
        .unwrap();

        let result = rt.block_on(verb.invoke(
            ctx,
            inputs,
            crate::plugin::progress::VerbProgress::discard(),
            tokio_util::sync::CancellationToken::new(),
        ));

        // QualityAnalysis mode hits the backend check first.
        assert!(matches!(result, Err(VerbError::MissingContext(_))));
    }

    #[tokio::test]
    async fn runtime_rejects_invocation_without_vlm_backend() {
        // Register the verb but add no VLM-capable backend — the runtime
        // should surface UnsupportedCapability before the verb even runs.
        let runtime = VerbRuntime::new();
        runtime.register(CritiqueVerb::new()).unwrap();

        let mut ctx = VerbContext::empty(metadata());
        ctx.active_sprite = Some(SpriteId::new(1));

        let inputs = VerbInputs::from_struct(&CritiqueInputs {
            mode: CritiqueMode::QualityAnalysis,
            checks: vec![],
            notes: None,
        })
        .unwrap();

        let inv = runtime.invoke(&VerbId::new(CRITIQUE_VERB_ID), ctx, inputs);
        assert!(
            matches!(inv, Err(VerbError::UnsupportedCapability { .. })),
            "expected UnsupportedCapability, got: {inv:?}"
        );
    }
}
