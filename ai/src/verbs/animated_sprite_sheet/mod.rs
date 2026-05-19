//! Verb: animated sprite sheet from prompt (S53).
//!
//! Generates a `g×g` grid of animation frames from a single text prompt
//! via a two-stage LLM choreography:
//!
//! 1. **Rewrite stage** (optional, requires `TEXT_GENERATION`): the
//!    user's character concept is passed through a `CHARACTER ×
//!    CHOREOGRAPHY` system prompt that returns a vivid character
//!    description and a `g`-beat animation loop.
//! 2. **Sprite stage** (requires `IMAGE_GENERATION`): the rewritten
//!    text is wrapped in a strict technical-requirements scaffold and
//!    sent to an image-generation backend. The returned PNG is sliced
//!    into `g²` cells in row-major order; each cell becomes a new
//!    timeline frame with one cel on the active layer. Cells snap to
//!    the active palette when present.
//!
//! # Three modes
//!
//! The verb implements the three-mode architecture from
//! `docs/planning/synthesis/ai-opportunity.md`:
//!
//! - [`GenerationMode::Procedural`] — emits `g²` empty frames the artist
//!   draws by hand. No backend call.
//! - [`GenerationMode::Ai`] — runs the full two-stage pipeline.
//! - [`GenerationMode::Hybrid`] — runs the rewrite stage and returns the
//!   rewritten text in [`VerbOutput::notes`] for review; emits empty
//!   frames so the artist can either draw in or re-run in AI mode with
//!   the reviewed rewrite supplied via
//!   [`AnimatedSpriteSheetInputs::rewritten_prompt`].
//!
//! # Attribution
//!
//! The CHARACTER × CHOREOGRAPHY system prompt and the technical-
//! requirements scaffold are adapted from `FalSprite`
//! (<https://github.com/lovisdotio/falsprite>), MIT-licensed,
//! Copyright (c) 2026 lovisdotio. See [`prompts`] for the asset files
//! and `NOTICES.md` at the repo root for the project-wide attribution
//! surface.

use std::time::{Duration, Instant};

use async_trait::async_trait;
use pixhaus_core::project::{Cel, Frame, FrameIndex, Layer, LayerId, Palette, PixelBufferId, Size};
use serde::{Deserialize, Serialize};
use tokio::select;
use tokio_util::sync::CancellationToken;

use crate::backends::{
    ChatMessage, ImageGenRequest, InferenceRequest, InferenceResponse, TextGenRequest,
};
use crate::plugin::backend::InferenceBackend as PluginBackend;
use crate::plugin::context::{PixelData, VerbContext};
use crate::plugin::descriptor::{
    BackendCapabilities, CostEstimate, EffectKind, VerbDescriptor, VerbId,
};
use crate::plugin::error::{Result, VerbError};
use crate::plugin::inputs::VerbInputs;
use crate::plugin::output::{ActualCost, NewPixelBuffer, VerbEffect, VerbOutput};
use crate::plugin::progress::{VerbProgress, VerbProgressEvent};
use crate::plugin::prompt_scaffold::PromptScaffold;
use crate::plugin::verb::Verb;

pub mod grid;
pub mod prompts;

pub use grid::{GridCoord, cell_size, frame_coord, slice_grid};
pub use prompts::{
    DEFAULT_GRID_SIZE, MAX_GRID_SIZE, MIN_GRID_SIZE, REWRITE_SYSTEM_PROMPT_TEMPLATE,
    SPRITE_CONSTRAINT_TEMPLATE, fill_grid_word, grid_word, rewrite_system_prompt,
    sprite_constraint_scaffold,
};

/// Stable identifier for the animated-sprite-sheet verb.
pub const ANIMATED_SPRITE_SHEET_VERB_ID: &str = "pixhaus.builtin.animated_sprite_sheet";

/// Default cell size in pixels. Matches the typical 64×64 sprite-art
/// authoring resolution; configurable via
/// [`AnimatedSpriteSheetInputs::cell_size_px`].
pub const DEFAULT_CELL_SIZE_PX: u32 = 64;

/// Minimum / maximum cell size accepted by the verb. The floor keeps
/// the image-model output recognisable as pixel art; the ceiling guards
/// against accidental 4K generations that would burn through credits.
pub const MIN_CELL_SIZE_PX: u32 = 16;
/// Upper bound for [`DEFAULT_CELL_SIZE_PX`]; together with `grid_size=6`
/// this caps the sheet at 1536×1536 — well below most backends' limits.
pub const MAX_CELL_SIZE_PX: u32 = 256;

/// Default per-frame duration in milliseconds. Falls back to
/// [`Frame::default`]'s 100 ms when the input doesn't set one.
pub const DEFAULT_FRAME_DURATION_MS: u32 = 100;

const PLACEHOLDER_LAYER_ID: u32 = 0;

/// Generation mode for the verb.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GenerationMode {
    /// Emit `grid_size²` empty frames the artist will draw by hand. No
    /// backend call; runs offline.
    Procedural,
    /// Full two-stage AI pipeline: rewrite (if available) then image
    /// generation. Default.
    #[default]
    Ai,
    /// Rewrite-only stage: runs the LLM rewrite (if available), returns
    /// the rewritten text in [`VerbOutput::notes`], and emits empty
    /// frames for the artist to review or re-run in AI mode.
    Hybrid,
}

/// Inputs to [`AnimatedSpriteSheetVerb`].
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AnimatedSpriteSheetInputs {
    /// User's character concept (e.g. `"baby dragon, pastel pixel art"`).
    pub prompt: String,
    /// Grid side length. Must satisfy
    /// [`MIN_GRID_SIZE`] ≤ value ≤ [`MAX_GRID_SIZE`].
    pub grid_size: u8,
    /// Ordered list of action names that constrain the choreography
    /// (e.g. `["idle", "walk", "attack"]`). The UI caps this at
    /// `grid_size`; the verb validates the same invariant.
    #[serde(default)]
    pub actions: Vec<String>,
    /// Per-cell width and height in pixels. Same value for both axes so
    /// the grid stays square. Must satisfy
    /// [`MIN_CELL_SIZE_PX`] ≤ value ≤ [`MAX_CELL_SIZE_PX`].
    #[serde(default = "default_cell_size_px")]
    pub cell_size_px: u32,
    /// Frame duration in milliseconds applied to every generated frame.
    /// Defaults to [`DEFAULT_FRAME_DURATION_MS`].
    #[serde(default = "default_frame_duration_ms")]
    pub frame_duration_ms: u32,
    /// Generation mode. Defaults to [`GenerationMode::Ai`].
    #[serde(default)]
    pub mode: GenerationMode,
    /// Pre-edited rewrite output (CHARACTER + CHOREOGRAPHY text). When
    /// `Some`, the verb skips the LLM rewrite stage and feeds this text
    /// directly to the image backend. Lets the UI surface the rewrite
    /// for review and re-submit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rewritten_prompt: Option<String>,
    /// Override for the rewrite system prompt template. When `None`,
    /// the bundled [`REWRITE_SYSTEM_PROMPT_TEMPLATE`] is used.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system_prompt_override: Option<String>,
    /// Override for the sprite-stage constraint scaffold template. When
    /// `None`, the bundled [`SPRITE_CONSTRAINT_TEMPLATE`] is used.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub constraint_prompt_override: Option<String>,
    /// Optional style-reference image (raw PNG bytes). Precedence:
    /// explicit input → entity anchor sheet → none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style_reference: Option<Vec<u8>>,
    /// Display name for the new layer. Defaults to `"Animated sheet"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layer_name: Option<String>,
    /// Optional RNG seed forwarded to the image backend for
    /// reproducible generation. Backends that ignore seeds destructure-
    /// and-discard.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<u64>,
}

fn default_cell_size_px() -> u32 {
    DEFAULT_CELL_SIZE_PX
}
fn default_frame_duration_ms() -> u32 {
    DEFAULT_FRAME_DURATION_MS
}

impl AnimatedSpriteSheetInputs {
    /// Returns a minimal valid input set for `prompt`. Used in tests and
    /// as a convenience constructor for callers that take all defaults.
    #[must_use]
    pub fn new(prompt: impl Into<String>) -> Self {
        Self {
            prompt: prompt.into(),
            grid_size: DEFAULT_GRID_SIZE,
            actions: Vec::new(),
            cell_size_px: DEFAULT_CELL_SIZE_PX,
            frame_duration_ms: DEFAULT_FRAME_DURATION_MS,
            mode: GenerationMode::Ai,
            rewritten_prompt: None,
            system_prompt_override: None,
            constraint_prompt_override: None,
            style_reference: None,
            layer_name: None,
            seed: None,
        }
    }
}

/// Verb: generates an animated sprite sheet from a single prompt.
///
/// See the module-level docs for the pipeline and attribution.
#[derive(Debug)]
pub struct AnimatedSpriteSheetVerb {
    descriptor: VerbDescriptor,
}

impl AnimatedSpriteSheetVerb {
    /// Constructs the verb with its default descriptor.
    #[must_use]
    // `serde_json::json!` calls infallible builders that internally
    // use `unwrap`; we exempt the constructor rather than open-code
    // the schema as `Value::Object`.
    #[allow(clippy::disallowed_methods)]
    pub fn new() -> Self {
        let input_schema = serde_json::json!({
            "type": "object",
            "properties": {
                "prompt":             {"type": "string", "minLength": 1},
                "grid_size":          {
                    "type": "integer",
                    "minimum": MIN_GRID_SIZE,
                    "maximum": MAX_GRID_SIZE,
                },
                "actions":            {
                    "type": "array",
                    "items": {"type": "string"},
                },
                "cell_size_px":       {
                    "type": "integer",
                    "minimum": MIN_CELL_SIZE_PX,
                    "maximum": MAX_CELL_SIZE_PX,
                },
                "frame_duration_ms":  {"type": "integer", "minimum": 1},
                "mode":               {
                    "type": "string",
                    "enum": ["procedural", "ai", "hybrid"],
                },
                "rewritten_prompt":           {"type": ["string", "null"]},
                "system_prompt_override":     {"type": ["string", "null"]},
                "constraint_prompt_override": {"type": ["string", "null"]},
                "style_reference":            {"type": ["array", "null"], "items": {"type": "integer"}},
                "layer_name":                 {"type": ["string", "null"]},
                "seed":                       {"type": ["integer", "null"]},
            },
            "required": ["prompt", "grid_size"],
        });

        Self {
            descriptor: VerbDescriptor {
                id: VerbId::new(ANIMATED_SPRITE_SHEET_VERB_ID),
                display_name: "Animated sprite sheet".into(),
                description: "Generates a g×g animation grid from a single prompt via \
                              a two-stage LLM choreography (CHARACTER × CHOREOGRAPHY)."
                    .into(),
                version: env!("CARGO_PKG_VERSION").into(),
                required_capabilities: BackendCapabilities::IMAGE_GENERATION,
                input_schema,
                output_schema: None,
                output_kinds: vec![EffectKind::AddLayer, EffectKind::AddFrames],
                cost_estimate: CostEstimate {
                    typical_latency: Duration::from_secs(20),
                    max_latency: Duration::from_secs(60),
                    typical_usd_cents: 5.0,
                    max_usd_cents: 12.0,
                },
                streaming: true,
                cancellable: true,
                documentation_url: Some("docs/verbs/animated_sprite_sheet.md".into()),
            },
        }
    }
}

impl Default for AnimatedSpriteSheetVerb {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Verb for AnimatedSpriteSheetVerb {
    fn descriptor(&self) -> &VerbDescriptor {
        &self.descriptor
    }

    fn validate(&self, inputs: &VerbInputs) -> Result<()> {
        let parsed: AnimatedSpriteSheetInputs = inputs.deserialize()?;
        validate_parsed(&parsed)
    }

    async fn invoke(
        &self,
        ctx: VerbContext,
        inputs: VerbInputs,
        progress: VerbProgress,
        cancel: CancellationToken,
    ) -> Result<VerbOutput> {
        let started = Instant::now();
        let inputs: AnimatedSpriteSheetInputs = inputs.deserialize_owned()?;
        validate_parsed(&inputs)?;
        let sprite_id = ctx.require_sprite_id()?;

        progress
            .send(VerbProgressEvent::Started {
                backend: ctx.backend.as_ref().map(|b| b.id().to_owned()),
            })
            .await;

        match inputs.mode {
            GenerationMode::Procedural => {
                invoke_procedural(&inputs, sprite_id, started, progress).await
            }
            GenerationMode::Hybrid => {
                invoke_hybrid(&inputs, sprite_id, &ctx, started, progress, cancel).await
            }
            GenerationMode::Ai => {
                invoke_ai(&inputs, sprite_id, &ctx, started, progress, cancel).await
            }
        }
    }
}

// ── Validation ──────────────────────────────────────────────────────────────

fn validate_parsed(parsed: &AnimatedSpriteSheetInputs) -> Result<()> {
    if parsed.prompt.trim().is_empty() {
        return Err(VerbError::Schema(
            "animated_sprite_sheet: prompt must not be empty".into(),
        ));
    }
    if parsed.grid_size < MIN_GRID_SIZE || parsed.grid_size > MAX_GRID_SIZE {
        return Err(VerbError::Schema(format!(
            "animated_sprite_sheet: grid_size {} is out of range {}..={}",
            parsed.grid_size, MIN_GRID_SIZE, MAX_GRID_SIZE,
        )));
    }
    if parsed.actions.len() > parsed.grid_size as usize {
        return Err(VerbError::Schema(format!(
            "animated_sprite_sheet: too many actions ({}) for grid_size {} — \
             one action per row, max {} actions",
            parsed.actions.len(),
            parsed.grid_size,
            parsed.grid_size,
        )));
    }
    if parsed.cell_size_px < MIN_CELL_SIZE_PX || parsed.cell_size_px > MAX_CELL_SIZE_PX {
        return Err(VerbError::Schema(format!(
            "animated_sprite_sheet: cell_size_px {} is out of range {}..={}",
            parsed.cell_size_px, MIN_CELL_SIZE_PX, MAX_CELL_SIZE_PX,
        )));
    }
    if parsed.frame_duration_ms == 0 {
        return Err(VerbError::Schema(
            "animated_sprite_sheet: frame_duration_ms must be at least 1".into(),
        ));
    }
    for (i, action) in parsed.actions.iter().enumerate() {
        if action.trim().is_empty() {
            return Err(VerbError::Schema(format!(
                "animated_sprite_sheet: action[{i}] must not be empty"
            )));
        }
    }
    Ok(())
}

// ── Procedural mode ────────────────────────────────────────────────────────

async fn invoke_procedural(
    inputs: &AnimatedSpriteSheetInputs,
    sprite_id: pixhaus_core::project::SpriteId,
    started: Instant,
    progress: VerbProgress,
) -> Result<VerbOutput> {
    progress.step(Some(0.5), "building placeholder grid").await;

    let frame_count = u32::from(inputs.grid_size).pow(2);
    let (frame_pixels, frame_size) = build_empty_cells(inputs);

    let (layer, cels, pixel_buffers) = pack_layer_with_cells(inputs, frame_pixels, frame_size);

    progress.step(Some(1.0), "placeholder grid ready").await;

    let elapsed = started.elapsed();
    let thumbnail = pixel_buffers.first().map(|b| b.pixels.clone());

    Ok(VerbOutput {
        summary: format!(
            "Add layer \"{}\" with {} placeholder frames ({}×{} grid)",
            layer.name, frame_count, inputs.grid_size, inputs.grid_size,
        ),
        effects: vec![VerbEffect::AddLayer {
            sprite: sprite_id,
            layer,
            cels,
            pixel_buffers,
        }],
        thumbnail,
        actual_cost: ActualCost::free(elapsed.max(Duration::from_micros(1))),
        notes: action_notes(inputs),
    })
}

// ── Hybrid mode ─────────────────────────────────────────────────────────────

async fn invoke_hybrid(
    inputs: &AnimatedSpriteSheetInputs,
    sprite_id: pixhaus_core::project::SpriteId,
    ctx: &VerbContext,
    started: Instant,
    progress: VerbProgress,
    cancel: CancellationToken,
) -> Result<VerbOutput> {
    let rewrite = match maybe_rewrite(inputs, ctx, &progress, &cancel).await? {
        RewriteOutcome::Generated(text) | RewriteOutcome::UserSupplied(text) => Some(text),
        RewriteOutcome::NotAvailable(_) => None,
    };

    progress
        .step(Some(0.8), "building hybrid placeholder grid")
        .await;

    let frame_count = u32::from(inputs.grid_size).pow(2);
    let (frame_pixels, frame_size) = build_empty_cells(inputs);
    let (layer, cels, pixel_buffers) = pack_layer_with_cells(inputs, frame_pixels, frame_size);

    progress.step(Some(1.0), "hybrid preview ready").await;

    let mut notes = action_notes(inputs);
    if let Some(text) = rewrite {
        notes.push("--- Rewritten prompt (CHARACTER × CHOREOGRAPHY) ---".into());
        notes.push(text);
        notes.push(
            "Re-run in AI mode with `rewritten_prompt` set to commit this rewrite, \
             or edit and resubmit."
                .into(),
        );
    } else {
        notes.push(
            "Rewrite stage skipped — no TEXT_GENERATION backend attached. \
             Switching to AI mode will send the prompt directly to the image backend."
                .into(),
        );
    }

    let elapsed = started.elapsed();
    let thumbnail = pixel_buffers.first().map(|b| b.pixels.clone());

    Ok(VerbOutput {
        summary: format!(
            "Hybrid preview: add layer \"{}\" with {} placeholder frames + rewrite for review",
            layer.name, frame_count,
        ),
        effects: vec![VerbEffect::AddLayer {
            sprite: sprite_id,
            layer,
            cels,
            pixel_buffers,
        }],
        thumbnail,
        actual_cost: ActualCost::free(elapsed.max(Duration::from_micros(1))),
        notes,
    })
}

// ── AI mode ─────────────────────────────────────────────────────────────────

async fn invoke_ai(
    inputs: &AnimatedSpriteSheetInputs,
    sprite_id: pixhaus_core::project::SpriteId,
    ctx: &VerbContext,
    started: Instant,
    progress: VerbProgress,
    cancel: CancellationToken,
) -> Result<VerbOutput> {
    let mut notes = action_notes(inputs);

    // Stage 1 — rewrite (if possible / requested).
    let rewrite_outcome = maybe_rewrite(inputs, ctx, &progress, &cancel).await?;
    let rewritten_text = match &rewrite_outcome {
        RewriteOutcome::Generated(t) | RewriteOutcome::UserSupplied(t) => t.clone(),
        RewriteOutcome::NotAvailable(reason) => {
            notes.push(format!("Rewrite skipped: {reason}"));
            inputs.prompt.clone()
        }
    };
    if matches!(rewrite_outcome, RewriteOutcome::Generated(_)) {
        notes.push("--- Rewritten prompt (CHARACTER × CHOREOGRAPHY) ---".into());
        notes.push(rewritten_text.clone());
    }

    // Stage 2 — sprite generation.
    let sheet = call_image_stage(inputs, &rewritten_text, ctx, &progress, &cancel).await?;

    progress.step(Some(0.9), "slicing grid").await;
    let mut frame_pixels = slice_grid(&sheet, inputs.grid_size)?;

    // Snap to palette (or note its absence).
    let palette = ctx.active_palette.as_ref();
    if let Some(pal) = palette {
        for cell in &mut frame_pixels {
            *cell = snap_to_palette(cell, pal);
        }
    } else {
        notes.push(
            "No active palette — generated frames retain the backend's full RGBA gamut.".into(),
        );
    }

    let frame_size = Size::new(inputs.cell_size_px, inputs.cell_size_px);
    let (layer, cels, pixel_buffers) = pack_layer_with_cells(inputs, frame_pixels, frame_size);

    progress.step(Some(1.0), "animated sheet ready").await;

    let elapsed = started.elapsed();
    let thumbnail = pixel_buffers.first().map(|b| b.pixels.clone());

    Ok(VerbOutput {
        summary: format!(
            "Add layer \"{}\" with {}×{} = {} frames ({}×{} px each)",
            layer.name,
            inputs.grid_size,
            inputs.grid_size,
            cels.len(),
            inputs.cell_size_px,
            inputs.cell_size_px,
        ),
        effects: vec![VerbEffect::AddLayer {
            sprite: sprite_id,
            layer,
            cels,
            pixel_buffers,
        }],
        thumbnail,
        actual_cost: ActualCost::free(elapsed.max(Duration::from_micros(1))),
        notes,
    })
}

// ── Image stage ─────────────────────────────────────────────────────────────

async fn call_image_stage(
    inputs: &AnimatedSpriteSheetInputs,
    rewritten_text: &str,
    ctx: &VerbContext,
    progress: &VerbProgress,
    cancel: &CancellationToken,
) -> Result<PixelData> {
    progress
        .step(Some(0.4), "building sprite-stage prompt")
        .await;

    let sprite_prompt = build_sprite_prompt(
        inputs,
        rewritten_text,
        inputs.constraint_prompt_override.as_deref(),
    );

    let sheet_w = inputs.cell_size_px * u32::from(inputs.grid_size);
    let sheet_h = sheet_w;

    let style_image = inputs
        .style_reference
        .clone()
        .or_else(|| crate::verbs::anchor_style_image_bytes(ctx));

    let req = InferenceRequest::ImageGeneration(ImageGenRequest {
        model: None,
        prompt: sprite_prompt,
        negative_prompt: Some(
            "text, labels, watermarks, signatures, UI, gradients, blurry, photorealistic, \
             3d render, jpeg artifacts, low-resolution"
                .into(),
        ),
        width: sheet_w,
        height: sheet_h,
        steps: None,
        seed: inputs.seed,
        num_images: 1,
        quality: None,
        style_image,
        reference_images: Vec::new(),
    });

    progress
        .step(Some(0.5), "calling image-generation backend")
        .await;

    let backend = crate::verbs::ctx_fat_backend(ctx)?;
    let response = select! {
        biased;
        () = cancel.cancelled() => return Err(VerbError::Cancelled),
        res = backend.invoke(req, progress.clone(), cancel.clone()) => {
            res.map_err(|e| VerbError::Backend(e.to_string()))?
        }
    };

    let png_bytes = match response {
        InferenceResponse::Image(r) => r
            .images
            .into_iter()
            .next()
            .ok_or_else(|| VerbError::Backend("backend returned no images".into()))?,
        _ => {
            return Err(VerbError::Backend(
                "expected image response from image-generation backend".into(),
            ));
        }
    };

    progress.step(Some(0.8), "decoding sheet").await;
    decode_png_to_rgba8(&png_bytes)
}

// ── Rewrite plumbing ────────────────────────────────────────────────────────

enum RewriteOutcome {
    /// We ran the LLM rewrite and got fresh text.
    Generated(String),
    /// The caller supplied a pre-edited rewrite via
    /// [`AnimatedSpriteSheetInputs::rewritten_prompt`].
    UserSupplied(String),
    /// No rewrite happened. The string explains why so the verb can
    /// surface it as a note.
    NotAvailable(String),
}

async fn maybe_rewrite(
    inputs: &AnimatedSpriteSheetInputs,
    ctx: &VerbContext,
    progress: &VerbProgress,
    cancel: &CancellationToken,
) -> Result<RewriteOutcome> {
    if let Some(text) = inputs.rewritten_prompt.as_ref() {
        return Ok(RewriteOutcome::UserSupplied(text.clone()));
    }

    let Some(backend) = ctx.backend.as_ref() else {
        return Ok(RewriteOutcome::NotAvailable(
            "no backend attached to the verb context".into(),
        ));
    };

    if !backend
        .capabilities()
        .contains(BackendCapabilities::TEXT_GENERATION)
    {
        return Ok(RewriteOutcome::NotAvailable(format!(
            "backend {} does not advertise TEXT_GENERATION; \
             only the prompt scaffold will run",
            backend.id(),
        )));
    }

    progress
        .step(Some(0.15), "rewriting prompt (CHARACTER × CHOREOGRAPHY)")
        .await;

    let system_text = inputs
        .system_prompt_override
        .clone()
        .unwrap_or_else(|| rewrite_system_prompt(inputs.grid_size));
    let user_text = build_rewrite_user_message(inputs);

    let req = TextGenRequest {
        model: None,
        system: Some(system_text),
        messages: vec![ChatMessage::user(user_text)],
        tools: Vec::new(),
        max_tokens: Some(800),
        temperature: Some(0.7),
        stop: Vec::new(),
    };

    let resp =
        call_text_via_backend(backend.as_ref(), req, progress.clone(), cancel.clone()).await?;

    Ok(RewriteOutcome::Generated(resp.content.trim().to_owned()))
}

async fn call_text_via_backend(
    backend: &dyn PluginBackend,
    request: TextGenRequest,
    progress: VerbProgress,
    cancel: CancellationToken,
) -> Result<crate::backends::TextGenResponse> {
    crate::verbs::call_text_vlm(backend, request, progress, cancel).await
}

fn build_rewrite_user_message(inputs: &AnimatedSpriteSheetInputs) -> String {
    let mut out = inputs.prompt.trim().to_owned();
    if !inputs.actions.is_empty() {
        out.push_str("\n\nRequested actions, in order (one per beat row):\n");
        for action in &inputs.actions {
            out.push_str("- ");
            out.push_str(action.trim());
            out.push('\n');
        }
    }
    out
}

// ── Sprite-stage prompt ─────────────────────────────────────────────────────

/// Builds the final image-stage prompt by rendering the constraint
/// scaffold and appending the LLM-rewritten character + choreography
/// text.
///
/// `constraint_override` lets the UI substitute a custom scaffold; when
/// `None` the bundled FalSprite-derived template is used.
#[must_use]
pub fn build_sprite_prompt(
    inputs: &AnimatedSpriteSheetInputs,
    rewritten_text: &str,
    constraint_override: Option<&str>,
) -> String {
    let scaffold_text = constraint_override.map_or_else(
        || sprite_constraint_scaffold(inputs.grid_size),
        |s| fill_grid_word(s, inputs.grid_size),
    );
    // The bundled scaffold ends with "CHARACTER AND ANIMATION DIRECTION:\n";
    // we trim trailing whitespace then append the rewritten text so the
    // two halves read as one prompt.
    let mut out = scaffold_text.trim_end().to_owned();
    out.push('\n');
    out.push_str(rewritten_text.trim());
    out
}

/// Builds a labelled-section [`PromptScaffold`] from `inputs`. Surfaced
/// for callers (UI, tests, plugin authors) that want to render the
/// scaffold without the verb's image-generation pipeline.
#[must_use]
pub fn sprite_scaffold(inputs: &AnimatedSpriteSheetInputs) -> PromptScaffold {
    // Split the FalSprite-derived scaffold into discrete sections so
    // structured-prompt backends (a future MCP-style API) can consume
    // it without re-parsing the rendered text.
    let g = inputs.grid_size;
    let w = grid_word(g);
    PromptScaffold::new()
        .section(
            "STRICT TECHNICAL REQUIREMENTS FOR THIS IMAGE",
            String::new(),
        )
        .section(
            "FORMAT",
            format!(
                "A single image containing a {w}-by-{w} grid of equally sized cells. \
                 Every cell must be the exact same dimensions, perfectly aligned, with no \
                 gaps or overlap."
            ),
        )
        .section(
            "FORBIDDEN",
            String::from(
                "Absolutely no text, no numbers, no letters, no digits, no labels, no \
                 watermarks, no signatures, no UI elements anywhere in the image. The image \
                 must contain ONLY the character illustrations in the grid cells and nothing \
                 else.",
            ),
        )
        .section(
            "CONSISTENCY",
            String::from(
                "The exact same single character must appear in every cell. Same proportions, \
                 same art style, same level of detail, same camera angle throughout. Isometric \
                 three-quarter view. Full body visible head to toe in every cell. Strong clean \
                 silhouette against a plain solid flat-color background.",
            ),
        )
        .section(
            "ANIMATION FLOW",
            format!(
                "The cells read left-to-right, top-to-bottom, like reading a page. This is \
                 one continuous motion sequence. Each cell shows the next moment in the \
                 movement. Each row contains {w} phases of the motion. The very last cell \
                 loops back seamlessly to the very first cell."
            ),
        )
        .section(
            "MOTION QUALITY",
            String::from(
                "Show real weight and physics. Bodies shift weight between feet. Arms \
                 counterbalance legs. Torsos rotate into actions. Follow-through on every \
                 movement. No stiff poses — every cell must feel like a freeze-frame of fluid \
                 motion.",
            ),
        )
}

// ── Empty / placeholder cells ──────────────────────────────────────────────

fn build_empty_cells(inputs: &AnimatedSpriteSheetInputs) -> (Vec<PixelData>, Size) {
    let frame_count = u32::from(inputs.grid_size).pow(2) as usize;
    let cell_w = inputs.cell_size_px;
    let cell_h = inputs.cell_size_px;
    let empty_bytes = (cell_w as usize) * (cell_h as usize) * 4;
    let cells = (0..frame_count)
        .map(|_| PixelData::rgba8(cell_w, cell_h, vec![0u8; empty_bytes]))
        .collect();
    (cells, Size::new(cell_w, cell_h))
}

// ── Layer / cels / buffers packing ─────────────────────────────────────────

fn pack_layer_with_cells(
    inputs: &AnimatedSpriteSheetInputs,
    frame_pixels: Vec<PixelData>,
    frame_size: Size,
) -> (Layer, Vec<Cel>, Vec<NewPixelBuffer>) {
    let layer_id = LayerId::new(PLACEHOLDER_LAYER_ID);
    let layer_name = inputs.layer_name.clone().unwrap_or_else(|| {
        if inputs.actions.is_empty() {
            "Animated sheet".into()
        } else {
            format!("Animated sheet — {}", inputs.actions.join(" / "))
        }
    });
    let layer = Layer::raster(layer_id, &layer_name);

    let mut cel_records = Vec::with_capacity(frame_pixels.len());
    let mut buffers = Vec::with_capacity(frame_pixels.len());
    for (i, pixels) in frame_pixels.into_iter().enumerate() {
        #[allow(clippy::cast_possible_truncation)]
        let buf_id = PixelBufferId::new(i as u32);
        #[allow(clippy::cast_possible_truncation)]
        let frame_index = FrameIndex::new(i as u32);
        cel_records.push(Cel::raster(layer_id, frame_index, buf_id, frame_size));
        buffers.push(NewPixelBuffer {
            placeholder: buf_id,
            pixels,
        });
    }

    (layer, cel_records, buffers)
}

/// Builds the per-frame [`Frame`] records for a hypothetical `AddFrames`
/// effect. Surfaced for callers (UI, scripting plugins, future re-routing
/// of the verb's output through `AddFrames` instead of `AddLayer`) that
/// need the same timing the verb would use.
#[must_use]
pub fn build_timeline_frames(inputs: &AnimatedSpriteSheetInputs) -> Vec<Frame> {
    let frame_count = u32::from(inputs.grid_size).pow(2) as usize;
    (0..frame_count)
        .map(|_| Frame {
            duration_ms: inputs.frame_duration_ms,
            ..Frame::default()
        })
        .collect()
}

// ── Misc helpers ────────────────────────────────────────────────────────────

fn action_notes(inputs: &AnimatedSpriteSheetInputs) -> Vec<String> {
    if inputs.actions.is_empty() {
        return Vec::new();
    }
    vec![format!(
        "Actions ({} of {} rows): {}",
        inputs.actions.len(),
        inputs.grid_size,
        inputs.actions.join(", "),
    )]
}

fn decode_png_to_rgba8(png_bytes: &[u8]) -> Result<PixelData> {
    let img = image::load_from_memory(png_bytes)
        .map_err(|e| VerbError::Backend(format!("PNG decode failed: {e}")))?
        .to_rgba8();
    let (w, h) = img.dimensions();
    Ok(PixelData::rgba8(w, h, img.into_raw()))
}

/// Snaps every opaque pixel in `src` to the nearest palette colour by
/// squared Euclidean RGB distance. Transparent pixels (alpha == 0) pass
/// through unchanged so palette snapping doesn't leak colour into empty
/// cells.
#[must_use]
pub fn snap_to_palette(src: &PixelData, palette: &Palette) -> PixelData {
    if palette.colors.is_empty() {
        return src.clone();
    }
    let stride = src.stride as usize;
    let row_bytes = (src.width as usize) * 4;
    let height = src.height as usize;
    let mut out = vec![0u8; row_bytes * height];

    for y in 0..height {
        for x in 0..src.width as usize {
            let in_off = y * stride + x * 4;
            let out_off = y * row_bytes + x * 4;
            let a = src.bytes[in_off + 3];
            if a == 0 {
                continue;
            }
            let r = src.bytes[in_off];
            let g = src.bytes[in_off + 1];
            let b = src.bytes[in_off + 2];
            let target = pixhaus_core::project::color::Rgba::new(r, g, b, a);
            let snap = palette
                .nearest_index(target)
                .and_then(|idx| palette.color_at(idx))
                .unwrap_or(target);
            out[out_off] = snap.r;
            out[out_off + 1] = snap.g;
            out[out_off + 2] = snap.b;
            out[out_off + 3] = a;
        }
    }

    PixelData::rgba8(src.width, src.height, out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::context::PixelData;
    use pixhaus_core::project::{Palette, PaletteId, ProjectMetadata, Rgba, SpriteId};

    fn metadata() -> ProjectMetadata {
        ProjectMetadata {
            name: "s53-test".into(),
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

    #[test]
    fn descriptor_declares_image_generation_capability() {
        let v = AnimatedSpriteSheetVerb::new();
        let d = v.descriptor();
        assert!(
            d.required_capabilities
                .contains(BackendCapabilities::IMAGE_GENERATION)
        );
        assert!(d.streaming);
        assert!(d.cancellable);
        assert_eq!(d.id.as_str(), ANIMATED_SPRITE_SHEET_VERB_ID);
    }

    #[test]
    fn validate_rejects_empty_prompt() {
        let v = AnimatedSpriteSheetVerb::new();
        let inputs = VerbInputs::from_struct(&AnimatedSpriteSheetInputs::new("   ")).unwrap();
        assert!(matches!(v.validate(&inputs), Err(VerbError::Schema(_))));
    }

    #[test]
    fn validate_rejects_grid_size_out_of_range() {
        let v = AnimatedSpriteSheetVerb::new();
        for bad in [0u8, 1, 7, 100] {
            let mut p = AnimatedSpriteSheetInputs::new("baby dragon");
            p.grid_size = bad;
            let inputs = VerbInputs::from_struct(&p).unwrap();
            assert!(matches!(v.validate(&inputs), Err(VerbError::Schema(_))));
        }
    }

    #[test]
    fn validate_rejects_too_many_actions() {
        let v = AnimatedSpriteSheetVerb::new();
        let mut p = AnimatedSpriteSheetInputs::new("baby dragon");
        p.grid_size = 3;
        p.actions = vec!["idle".into(), "walk".into(), "run".into(), "attack".into()];
        let inputs = VerbInputs::from_struct(&p).unwrap();
        assert!(matches!(v.validate(&inputs), Err(VerbError::Schema(_))));
    }

    #[test]
    fn validate_rejects_empty_action() {
        let v = AnimatedSpriteSheetVerb::new();
        let mut p = AnimatedSpriteSheetInputs::new("baby dragon");
        p.actions = vec!["idle".into(), "  ".into()];
        let inputs = VerbInputs::from_struct(&p).unwrap();
        assert!(matches!(v.validate(&inputs), Err(VerbError::Schema(_))));
    }

    #[test]
    fn validate_rejects_cell_size_out_of_range() {
        let v = AnimatedSpriteSheetVerb::new();
        let mut p = AnimatedSpriteSheetInputs::new("baby dragon");
        p.cell_size_px = 8;
        let inputs = VerbInputs::from_struct(&p).unwrap();
        assert!(matches!(v.validate(&inputs), Err(VerbError::Schema(_))));
        let mut p = AnimatedSpriteSheetInputs::new("baby dragon");
        p.cell_size_px = 4096;
        let inputs = VerbInputs::from_struct(&p).unwrap();
        assert!(matches!(v.validate(&inputs), Err(VerbError::Schema(_))));
    }

    #[test]
    fn validate_rejects_zero_frame_duration() {
        let v = AnimatedSpriteSheetVerb::new();
        let mut p = AnimatedSpriteSheetInputs::new("baby dragon");
        p.frame_duration_ms = 0;
        let inputs = VerbInputs::from_struct(&p).unwrap();
        assert!(matches!(v.validate(&inputs), Err(VerbError::Schema(_))));
    }

    #[test]
    fn validate_accepts_minimal_inputs() {
        let v = AnimatedSpriteSheetVerb::new();
        let inputs =
            VerbInputs::from_struct(&AnimatedSpriteSheetInputs::new("baby dragon")).unwrap();
        assert!(v.validate(&inputs).is_ok());
    }

    #[test]
    fn validate_accepts_actions_equal_to_grid() {
        let v = AnimatedSpriteSheetVerb::new();
        let mut p = AnimatedSpriteSheetInputs::new("baby dragon");
        p.grid_size = 3;
        p.actions = vec!["idle".into(), "walk".into(), "attack".into()];
        let inputs = VerbInputs::from_struct(&p).unwrap();
        assert!(v.validate(&inputs).is_ok());
    }

    #[tokio::test]
    async fn procedural_mode_emits_grid_squared_empty_frames() {
        let v = AnimatedSpriteSheetVerb::new();
        let mut p = AnimatedSpriteSheetInputs::new("baby dragon");
        p.grid_size = 3;
        p.mode = GenerationMode::Procedural;
        let inputs = VerbInputs::from_struct(&p).unwrap();

        let (progress, _rx) = VerbProgress::channel();
        let cancel = CancellationToken::new();
        let out = v
            .invoke(ctx_with_sprite(), inputs, progress, cancel)
            .await
            .unwrap();

        assert_eq!(out.effects.len(), 1);
        match &out.effects[0] {
            VerbEffect::AddLayer {
                cels,
                pixel_buffers,
                ..
            } => {
                assert_eq!(cels.len(), 9);
                assert_eq!(pixel_buffers.len(), 9);
                for buf in pixel_buffers {
                    assert!(buf.pixels.bytes.iter().all(|&b| b == 0));
                }
            }
            other => panic!("expected AddLayer, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn procedural_mode_requires_active_sprite() {
        let v = AnimatedSpriteSheetVerb::new();
        let mut p = AnimatedSpriteSheetInputs::new("baby dragon");
        p.mode = GenerationMode::Procedural;
        let inputs = VerbInputs::from_struct(&p).unwrap();

        let (progress, _rx) = VerbProgress::channel();
        let cancel = CancellationToken::new();
        let out = v
            .invoke(VerbContext::empty(metadata()), inputs, progress, cancel)
            .await;
        assert!(matches!(out, Err(VerbError::MissingContext(_))));
    }

    #[test]
    fn build_sprite_prompt_uses_scaffold_then_rewrite() {
        let p = AnimatedSpriteSheetInputs::new("baby dragon");
        let out = build_sprite_prompt(&p, "CHARACTER: a tiny dragon\nCHOREOGRAPHY: idle...", None);
        assert!(out.contains("STRICT TECHNICAL REQUIREMENTS"));
        assert!(out.contains("FORMAT"));
        assert!(out.contains("CHARACTER AND ANIMATION DIRECTION"));
        assert!(out.contains("CHARACTER: a tiny dragon"));
        // four-by-four because default grid is 4.
        assert!(out.contains("four-by-four"));
    }

    #[test]
    fn build_sprite_prompt_respects_override_template() {
        let p = AnimatedSpriteSheetInputs::new("baby dragon");
        let custom = "CUSTOM SCAFFOLD ({grid_word}x{grid_word}):\n";
        let out = build_sprite_prompt(&p, "rewrite text", Some(custom));
        assert!(out.contains("CUSTOM SCAFFOLD (four"));
        assert!(out.contains("rewrite text"));
    }

    #[test]
    fn sprite_scaffold_renders_all_required_sections() {
        let p = AnimatedSpriteSheetInputs::new("baby dragon");
        let scaffold = sprite_scaffold(&p);
        let text = scaffold.render("PAYLOAD");
        for needle in [
            "FORMAT:",
            "FORBIDDEN:",
            "CONSISTENCY:",
            "ANIMATION FLOW:",
            "MOTION QUALITY:",
            "PAYLOAD",
        ] {
            assert!(text.contains(needle), "missing {needle}");
        }
    }

    #[test]
    fn build_rewrite_user_message_appends_actions() {
        let mut p = AnimatedSpriteSheetInputs::new("baby dragon");
        p.actions = vec!["idle".into(), "walk".into()];
        let msg = build_rewrite_user_message(&p);
        assert!(msg.contains("baby dragon"));
        assert!(msg.contains("Requested actions"));
        assert!(msg.contains("- idle"));
        assert!(msg.contains("- walk"));
    }

    #[test]
    fn snap_to_palette_passes_through_transparent() {
        let palette = Palette::from_colors(
            PaletteId::new(1),
            "test",
            vec![Rgba::new(255, 0, 0, 255), Rgba::new(0, 255, 0, 255)],
        );
        let bytes = vec![
            0, 0, 0, 0, // transparent black
            10, 200, 10, 255, // greenish opaque
        ];
        let src = PixelData::rgba8(2, 1, bytes);
        let snapped = snap_to_palette(&src, &palette);
        // Transparent pixel stays zeroed; opaque pixel snaps to green.
        assert_eq!(snapped.bytes[0..4], [0, 0, 0, 0]);
        assert_eq!(snapped.bytes[4..8], [0, 255, 0, 255]);
    }

    #[test]
    fn snap_to_palette_no_op_when_palette_empty() {
        let palette = Palette::from_colors(PaletteId::new(1), "empty", Vec::new());
        let bytes = vec![10, 20, 30, 255];
        let src = PixelData::rgba8(1, 1, bytes.clone());
        let snapped = snap_to_palette(&src, &palette);
        assert_eq!(snapped.bytes, bytes);
    }

    #[test]
    fn inputs_round_trip_json() {
        let mut original = AnimatedSpriteSheetInputs::new("baby dragon");
        original.grid_size = 3;
        original.actions = vec!["idle".into(), "walk".into()];
        original.cell_size_px = 64;
        original.frame_duration_ms = 80;
        original.mode = GenerationMode::Hybrid;
        original.rewritten_prompt = Some("CHARACTER: x\nCHOREOGRAPHY: y".into());
        original.layer_name = Some("Hero".into());
        original.seed = Some(42);

        let json = serde_json::to_string(&original).unwrap();
        let back: AnimatedSpriteSheetInputs = serde_json::from_str(&json).unwrap();
        assert_eq!(back, original);
    }

    #[test]
    fn timeline_frames_carry_custom_duration() {
        let mut p = AnimatedSpriteSheetInputs::new("baby dragon");
        p.frame_duration_ms = 50;
        p.grid_size = 2;
        let frames = build_timeline_frames(&p);
        assert_eq!(frames.len(), 4);
        for f in frames {
            assert_eq!(f.duration_ms, 50);
        }
    }

    #[test]
    fn pack_layer_uses_actions_in_default_name() {
        let mut p = AnimatedSpriteSheetInputs::new("baby dragon");
        p.actions = vec!["idle".into(), "walk".into()];
        let cells = vec![PixelData::rgba8(8, 8, vec![0; 256])];
        let (layer, _, _) = pack_layer_with_cells(&p, cells, Size::new(8, 8));
        assert!(layer.name.contains("idle"));
        assert!(layer.name.contains("walk"));
    }

    #[test]
    fn pack_layer_uses_explicit_name_when_set() {
        let mut p = AnimatedSpriteSheetInputs::new("baby dragon");
        p.actions = vec!["idle".into()];
        p.layer_name = Some("MyHero".into());
        let cells = vec![PixelData::rgba8(8, 8, vec![0; 256])];
        let (layer, _, _) = pack_layer_with_cells(&p, cells, Size::new(8, 8));
        assert_eq!(layer.name, "MyHero");
    }
}
