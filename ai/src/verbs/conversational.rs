//! Verb: Conversational editing — S31.
//!
//! Maps a free-form natural language instruction to a sequence of
//! editor commands. The VLM plans which operations to run (add layer,
//! adjust frame timing, rename a layer, etc.), surfaces the plan as a
//! preview, and on accept the host applies the commands through the
//! undo stack as a single coalesced entry.
//!
//! # Backend
//!
//! Requires `VISION_LANGUAGE | TOOL_USE`. Anthropic Claude and
//! `OpenAI` GPT-5 both satisfy this; Ollama models that support tool
//! calling work too.
//!
//! The verb sends the user instruction plus a compact sprite summary
//! to the VLM along with eight tool definitions. The VLM responds with
//! one or more tool calls; each is converted to a [`VerbEffect`]. The
//! full plan is surfaced in the preview before any edit is committed.
//!
//! # Custom effects
//!
//! Structural edits that don't map to a built-in `VerbEffect` variant
//! (rename layer, set frame durations, set layer opacity/visibility)
//! are emitted as [`VerbEffect::Custom`] with namespaced `name` values
//! (`pixhaus.builtin.conversational.*`). The host's undo system
//! interprets these.

use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use pixhaus_core::project::{
    Frame, FrameIndex, FrameRange, FrameTag, Layer, LayerId, LoopDirection, UserData,
};
use serde::{Deserialize, Serialize};
use tokio::select;
use tokio_util::sync::CancellationToken;
use tracing::debug;

use crate::plugin::context::VerbContext;
use crate::plugin::descriptor::{
    BackendCapabilities, CostEstimate, EffectKind, VerbDescriptor, VerbId,
};
use crate::plugin::error::{Result, VerbError};
use crate::plugin::inputs::VerbInputs;
use crate::plugin::output::{ActualCost, VerbEffect, VerbOutput};
use crate::plugin::progress::{VerbProgress, VerbProgressEvent};
use crate::plugin::verb::Verb;
use crate::backends::{
    InferenceBackend, InferenceRequest, InferenceResponse, TextGenRequest, ToolDef,
};

/// Stable identifier for the conversational editing verb.
pub const CONVERSATIONAL_VERB_ID: &str = "pixhaus.builtin.conversational";

/// Namespace prefix for custom `VerbEffect` names produced by this verb.
const EFFECT_NS: &str = "pixhaus.builtin.conversational";

/// Placeholder layer ID used in `AddLayer` effects (host rewrites on commit).
const LAYER_PLACEHOLDER: u32 = 0;

/// Inputs for [`ConversationalVerb`].
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ConversationalInputs {
    /// Natural language instruction the user typed.
    pub instruction: String,
}

/// Verb: free-form natural language → multi-step editor command plan.
///
/// Constructed with an `Arc<dyn InferenceBackend>` that satisfies
/// `VISION_LANGUAGE | TOOL_USE`. The verb owns its backend reference
/// directly because the `plugin::backend::InferenceBackend` trait (used
/// by the runtime's backend registry) is not yet bridged to the
/// concrete adapters in `backends/`. The app layer passes the concrete
/// adapter at construction time.
pub struct ConversationalVerb {
    descriptor: VerbDescriptor,
    backend: Arc<dyn InferenceBackend>,
}

impl std::fmt::Debug for ConversationalVerb {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConversationalVerb")
            .field("id", &self.descriptor.id)
            .field("backend", &self.descriptor.required_capabilities)
            .finish()
    }
}

impl ConversationalVerb {
    /// Constructs the verb with the given inference backend.
    ///
    /// The backend should advertise at least `VISION_LANGUAGE` and
    /// `TOOL_USE` capabilities. The constructor does not validate this;
    /// mismatches surface as [`VerbError::Backend`] during invocation
    /// when the backend rejects the tool-use request.
    #[must_use]
    #[allow(clippy::disallowed_methods)]
    pub fn new(backend: Arc<dyn InferenceBackend>) -> Self {
        let input_schema = serde_json::json!({
            "type": "object",
            "properties": {
                "instruction": {
                    "type": "string",
                    "description": "Natural language editing instruction."
                }
            },
            "required": ["instruction"]
        });

        Self {
            descriptor: VerbDescriptor {
                id: VerbId::new(CONVERSATIONAL_VERB_ID),
                display_name: "Conversational Edit".into(),
                description:
                    "Turn a natural language instruction into a sequence of editor commands.".into(),
                version: env!("CARGO_PKG_VERSION").into(),
                required_capabilities: BackendCapabilities::VISION_LANGUAGE
                    .union(BackendCapabilities::TOOL_USE),
                input_schema,
                output_schema: None,
                output_kinds: vec![
                    EffectKind::AddLayer,
                    EffectKind::AddFrames,
                    EffectKind::AddTag,
                    EffectKind::Custom(format!("{EFFECT_NS}.rename_layer")),
                    EffectKind::Custom(format!("{EFFECT_NS}.set_frame_durations")),
                    EffectKind::Custom(format!("{EFFECT_NS}.set_layer_opacity")),
                    EffectKind::Custom(format!("{EFFECT_NS}.set_layer_visibility")),
                ],
                cost_estimate: CostEstimate {
                    typical_latency: std::time::Duration::from_secs(4),
                    max_latency: std::time::Duration::from_secs(30),
                    typical_usd_cents: 0.5,
                    max_usd_cents: 5.0,
                },
                streaming: true,
                cancellable: true,
                documentation_url: Some("docs/verbs/conversational.md".into()),
            },
            backend,
        }
    }

    /// Builds a compact, human-readable sprite summary for the system
    /// prompt. The VLM uses this to understand layer names, frame count,
    /// and animation tags without receiving raw pixel data.
    fn sprite_context(ctx: &VerbContext) -> String {
        let Some(sprite) = ctx.sprite.as_ref() else {
            return "No sprite is currently open.".into();
        };

        let mut lines: Vec<String> = Vec::new();
        lines.push(format!(
            "Sprite: \"{}\" ({}x{}, {} frame{})",
            sprite.name,
            sprite.canvas.width,
            sprite.canvas.height,
            sprite.frames.len(),
            if sprite.frames.len() == 1 { "" } else { "s" },
        ));

        if !sprite.layers.is_empty() {
            lines.push(format!(
                "Layers ({} total, index 0 = bottom):",
                sprite.layers.len()
            ));
            for (i, layer) in sprite.layers.iter().enumerate() {
                lines.push(format!(
                    "  [{}] \"{}\" opacity={} blend={:?}{}",
                    i,
                    layer.name,
                    layer.opacity,
                    layer.blend_mode,
                    if layer.visible { "" } else { " (hidden)" },
                ));
            }
        }

        if !sprite.frame_tags.is_empty() {
            lines.push(format!(
                "Animation tags ({} total):",
                sprite.frame_tags.len()
            ));
            for tag in &sprite.frame_tags {
                let total_ms: u32 = (tag.range.start.get()..=tag.range.end.get())
                    .filter_map(|i| sprite.frames.get(i as usize))
                    .map(|f| f.duration_ms)
                    .sum();
                let fps = if total_ms > 0 {
                    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                    let f =
                        (f64::from(tag.range.len()) * 1000.0 / f64::from(total_ms)).round() as u32;
                    f
                } else {
                    0
                };
                lines.push(format!(
                    "  \"{}\" frames {}-{} ~{} fps {:?}",
                    tag.name,
                    tag.range.start.get(),
                    tag.range.end.get(),
                    fps,
                    tag.loop_direction,
                ));
            }
        }

        if let Some(active_layer_id) = ctx.active_layer {
            if let Some(layer) = sprite.layers.iter().find(|l| l.id == active_layer_id) {
                lines.push(format!("Active layer: \"{}\"", layer.name));
            }
        }
        if let Some(active_frame) = ctx.active_frame {
            lines.push(format!("Active frame index: {}", active_frame.get()));
        }

        lines.join("\n")
    }

    /// Returns the tool definitions the VLM may call.
    ///
    /// Eight tools cover structural edits. Pixel-level edits that
    /// require artist work or a generation verb are captured by
    /// `note_pixel_edit` and surfaced as notes rather than effects.
    #[allow(clippy::disallowed_methods, clippy::too_many_lines)]
    fn tool_defs() -> Vec<ToolDef> {
        vec![
            ToolDef {
                name: "add_layer".into(),
                description: "Add an empty raster layer to the active sprite.".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "name": {
                            "type": "string",
                            "description": "Display name for the new layer."
                        },
                        "blend_mode": {
                            "type": "string",
                            "description": "Blend mode (Normal, Multiply, Screen, Overlay, etc.). Defaults to Normal."
                        },
                        "opacity": {
                            "type": "integer",
                            "minimum": 0,
                            "maximum": 255,
                            "description": "Layer opacity 0-255. Defaults to 255."
                        }
                    },
                    "required": ["name"]
                }),
            },
            ToolDef {
                name: "add_frames".into(),
                description: "Add empty frames to the timeline.".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "count": {
                            "type": "integer",
                            "minimum": 1,
                            "description": "Number of frames to add."
                        },
                        "after_frame": {
                            "type": "integer",
                            "minimum": 0,
                            "description": "Insert after this 0-based frame index. Omit to append."
                        },
                        "duration_ms": {
                            "type": "integer",
                            "minimum": 1,
                            "description": "Duration per new frame in milliseconds. Defaults to 100."
                        }
                    },
                    "required": ["count"]
                }),
            },
            ToolDef {
                name: "add_animation_tag".into(),
                description: "Create a named animation tag on a range of frames.".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "name": {
                            "type": "string",
                            "description": "Tag name."
                        },
                        "from_frame": {
                            "type": "integer",
                            "minimum": 0,
                            "description": "First frame in the range, inclusive."
                        },
                        "to_frame": {
                            "type": "integer",
                            "minimum": 0,
                            "description": "Last frame in the range, inclusive."
                        },
                        "direction": {
                            "type": "string",
                            "enum": ["forward", "reverse", "ping_pong", "ping_pong_reverse"],
                            "description": "Playback direction. Defaults to forward."
                        }
                    },
                    "required": ["name", "from_frame", "to_frame"]
                }),
            },
            ToolDef {
                name: "set_frame_durations".into(),
                description: "Set all frame durations to hit a target FPS, optionally scoped to a named animation tag.".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "target_fps": {
                            "type": "number",
                            "exclusiveMinimum": 0,
                            "description": "Target frames-per-second. Each frame duration becomes round(1000 / target_fps) ms."
                        },
                        "tag_name": {
                            "type": "string",
                            "description": "Apply only to frames in this animation tag. Omit to apply to all frames."
                        }
                    },
                    "required": ["target_fps"]
                }),
            },
            ToolDef {
                name: "rename_layer".into(),
                description: "Rename an existing layer.".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "current_name": {
                            "type": "string",
                            "description": "Exact current display name of the layer."
                        },
                        "new_name": {
                            "type": "string",
                            "description": "New display name."
                        }
                    },
                    "required": ["current_name", "new_name"]
                }),
            },
            ToolDef {
                name: "set_layer_opacity".into(),
                description: "Change a layer's opacity.".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "layer_name": {
                            "type": "string",
                            "description": "Name of the layer to modify."
                        },
                        "opacity": {
                            "type": "integer",
                            "minimum": 0,
                            "maximum": 255,
                            "description": "New opacity 0 (invisible) to 255 (fully opaque)."
                        }
                    },
                    "required": ["layer_name", "opacity"]
                }),
            },
            ToolDef {
                name: "set_layer_visibility".into(),
                description: "Show or hide a layer.".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "layer_name": {
                            "type": "string",
                            "description": "Name of the layer."
                        },
                        "visible": {
                            "type": "boolean",
                            "description": "true to show, false to hide."
                        }
                    },
                    "required": ["layer_name", "visible"]
                }),
            },
            ToolDef {
                name: "note_pixel_edit".into(),
                description: "Record a pixel-level edit instruction that requires the artist or a generation verb to execute. Does not modify the sprite; surfaces as a note in the preview dialog.".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "description": {
                            "type": "string",
                            "description": "Clear, actionable description of the pixel edit required."
                        },
                        "layer": {
                            "type": "string",
                            "description": "Target layer name, if known."
                        },
                        "frame": {
                            "type": "integer",
                            "description": "Target frame index, if applicable."
                        }
                    },
                    "required": ["description"]
                }),
            },
        ]
    }

    /// Converts one VLM tool call to a `VerbEffect` (or `None` for
    /// `note_pixel_edit`, which becomes a note string instead).
    ///
    /// Returns `Err(VerbError::Backend(_))` when the tool input is
    /// malformed — the verb treats this as a backend failure since the
    /// VLM produced invalid JSON for its own schema.
    #[allow(clippy::too_many_lines)]
    fn tool_call_to_effect(
        tool_name: &str,
        input: &serde_json::Value,
        sprite_id: pixhaus_core::project::SpriteId,
        notes: &mut Vec<String>,
    ) -> Result<Option<VerbEffect>> {
        match tool_name {
            "add_layer" => {
                let name = string_field(input, "name")?;
                #[allow(clippy::cast_possible_truncation)]
                let opacity = input
                    .get("opacity")
                    .and_then(serde_json::Value::as_u64)
                    .map_or(255, |n| n.min(255) as u8);
                let blend_mode = input
                    .get("blend_mode")
                    .and_then(serde_json::Value::as_str)
                    .and_then(parse_blend_mode)
                    .unwrap_or_default();

                let mut layer = Layer::raster(LayerId::new(LAYER_PLACEHOLDER), name);
                layer.opacity = opacity;
                layer.blend_mode = blend_mode;

                Ok(Some(VerbEffect::AddLayer {
                    sprite: sprite_id,
                    layer,
                    cels: vec![],
                    pixel_buffers: vec![],
                }))
            }

            "add_frames" => {
                #[allow(clippy::cast_possible_truncation)]
                let count = u64_field(input, "count")?.max(1) as u32;
                #[allow(clippy::cast_possible_truncation)]
                let after = input
                    .get("after_frame")
                    .and_then(serde_json::Value::as_u64)
                    .map(|n| FrameIndex::new(n as u32));
                #[allow(clippy::cast_possible_truncation)]
                let duration_ms = input
                    .get("duration_ms")
                    .and_then(serde_json::Value::as_u64)
                    .map_or(100, |n| (n as u32).max(1));

                let frames = (0..count)
                    .map(|_| Frame {
                        duration_ms,
                        user_data: UserData::default(),
                    })
                    .collect();

                Ok(Some(VerbEffect::AddFrames {
                    sprite: sprite_id,
                    after,
                    frames,
                    cels: vec![],
                    pixel_buffers: vec![],
                }))
            }

            "add_animation_tag" => {
                let name = string_field(input, "name")?;
                #[allow(clippy::cast_possible_truncation)]
                let from = u64_field(input, "from_frame")? as u32;
                #[allow(clippy::cast_possible_truncation)]
                let to = u64_field(input, "to_frame")? as u32;
                let loop_direction = input
                    .get("direction")
                    .and_then(|v| v.as_str())
                    .and_then(parse_loop_direction)
                    .unwrap_or_default();

                let tag = FrameTag {
                    name,
                    range: FrameRange::new(FrameIndex::new(from), FrameIndex::new(to)),
                    loop_direction,
                    repeat: 0,
                    user_data: UserData::default(),
                };

                Ok(Some(VerbEffect::AddTag {
                    sprite: sprite_id,
                    tag,
                }))
            }

            "set_frame_durations" => {
                let target_fps = input
                    .get("target_fps")
                    .and_then(serde_json::Value::as_f64)
                    .filter(|&f| f > 0.0)
                    .ok_or_else(|| {
                        VerbError::Backend(
                            "set_frame_durations: target_fps must be a positive number".into(),
                        )
                    })?;
                let tag_name = input
                    .get("tag_name")
                    .and_then(serde_json::Value::as_str)
                    .map(std::borrow::ToOwned::to_owned);

                #[allow(clippy::disallowed_methods)]
                let payload = serde_json::json!({
                    "target_fps": target_fps,
                    "tag_name": tag_name,
                });

                Ok(Some(VerbEffect::Custom {
                    name: format!("{EFFECT_NS}.set_frame_durations"),
                    payload,
                }))
            }

            "rename_layer" => {
                let current = string_field(input, "current_name")?;
                let new = string_field(input, "new_name")?;

                #[allow(clippy::disallowed_methods)]
                let payload = serde_json::json!({
                    "current_name": current,
                    "new_name": new,
                });

                Ok(Some(VerbEffect::Custom {
                    name: format!("{EFFECT_NS}.rename_layer"),
                    payload,
                }))
            }

            "set_layer_opacity" => {
                let layer_name = string_field(input, "layer_name")?;
                #[allow(clippy::cast_possible_truncation)]
                let opacity = input
                    .get("opacity")
                    .and_then(serde_json::Value::as_u64)
                    .map(|n| n.min(255) as u8)
                    .ok_or_else(|| {
                        VerbError::Backend(
                            "set_layer_opacity: opacity field missing or invalid".into(),
                        )
                    })?;

                #[allow(clippy::disallowed_methods)]
                let payload = serde_json::json!({
                    "layer_name": layer_name,
                    "opacity": opacity,
                });

                Ok(Some(VerbEffect::Custom {
                    name: format!("{EFFECT_NS}.set_layer_opacity"),
                    payload,
                }))
            }

            "set_layer_visibility" => {
                let layer_name = string_field(input, "layer_name")?;
                let visible = input
                    .get("visible")
                    .and_then(serde_json::Value::as_bool)
                    .ok_or_else(|| {
                        VerbError::Backend(
                            "set_layer_visibility: visible field missing or not a boolean".into(),
                        )
                    })?;

                #[allow(clippy::disallowed_methods)]
                let payload = serde_json::json!({
                    "layer_name": layer_name,
                    "visible": visible,
                });

                Ok(Some(VerbEffect::Custom {
                    name: format!("{EFFECT_NS}.set_layer_visibility"),
                    payload,
                }))
            }

            "note_pixel_edit" => {
                let description = string_field(input, "description")?;
                let layer_clause = input
                    .get("layer")
                    .and_then(serde_json::Value::as_str)
                    .map(|l| format!(" on layer \"{l}\""))
                    .unwrap_or_default();
                let frame_clause = input
                    .get("frame")
                    .and_then(serde_json::Value::as_u64)
                    .map(|f| format!(" at frame {f}"))
                    .unwrap_or_default();

                notes.push(format!(
                    "Pixel edit required{layer_clause}{frame_clause}: {description}"
                ));
                Ok(None)
            }

            unknown => {
                debug!(tool = unknown, "VLM called an unrecognised tool; ignoring");
                Ok(None)
            }
        }
    }
}

#[async_trait]
impl Verb for ConversationalVerb {
    fn descriptor(&self) -> &VerbDescriptor {
        &self.descriptor
    }

    fn validate(&self, inputs: &VerbInputs) -> Result<()> {
        let parsed: ConversationalInputs = inputs.deserialize()?;
        if parsed.instruction.trim().is_empty() {
            return Err(VerbError::Schema(
                "conversational: instruction must not be empty".into(),
            ));
        }
        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    async fn invoke(
        &self,
        ctx: VerbContext,
        inputs: VerbInputs,
        progress: VerbProgress,
        cancel: CancellationToken,
    ) -> Result<VerbOutput> {
        let started = Instant::now();
        let inputs: ConversationalInputs = inputs.deserialize_owned()?;
        let sprite_id = ctx.require_sprite_id()?;

        progress
            .send(VerbProgressEvent::Started { backend: None })
            .await;
        progress.step(Some(0.1), "sending instruction to VLM").await;

        if cancel.is_cancelled() {
            return Err(VerbError::Cancelled);
        }

        let sprite_summary = Self::sprite_context(&ctx);
        debug!(sprite = %sprite_summary, "built sprite context for VLM");

        let system = format!(
            "You are an assistant inside Pixhaus, a pixel art editor. \
             The user will give you an editing instruction. \
             Use the provided tools to plan the changes — one tool call per \
             operation, in the order they should be applied. \
             Do not describe what you are doing; just call the tools. \
             For pixel-level edits (painting, erasing, drawing shapes) that \
             require human or AI artistry, use note_pixel_edit instead of \
             trying to paint directly. \
             Current project state:\n\n{sprite_summary}"
        );

        let mut req = TextGenRequest::user(inputs.instruction.clone());
        req.system = Some(system);
        req.tools = Self::tool_defs();
        req.max_tokens = Some(2048);
        req.temperature = Some(0.2);

        let (progress_clone, cancel_clone) = (progress.clone(), cancel.clone());
        let response = select! {
            biased;
            () = cancel_clone.cancelled() => return Err(VerbError::Cancelled),
            res = self.backend.invoke(
                InferenceRequest::Text(req),
                progress_clone,
                cancel.clone(),
            ) => res.map_err(|e| VerbError::Backend(e.to_string()))?,
        };

        if cancel.is_cancelled() {
            return Err(VerbError::Cancelled);
        }

        progress.step(Some(0.8), "parsing plan").await;

        let InferenceResponse::Text(text_resp) = response else {
            return Err(VerbError::Backend(
                "conversational: backend returned a non-text response".into(),
            ));
        };

        if text_resp.tool_calls.is_empty() {
            // The VLM responded with text only — it may have asked a
            // clarifying question or explained why it can't proceed.
            // Surface the text as a note with no effects.
            let note = if text_resp.content.is_empty() {
                "The VLM did not produce any tool calls or text.".into()
            } else {
                format!(
                    "VLM response (no operations planned): {}",
                    text_resp.content.trim()
                )
            };
            return Ok(VerbOutput {
                summary: "No operations planned".into(),
                effects: vec![],
                thumbnail: None,
                actual_cost: ActualCost {
                    elapsed: started.elapsed(),
                    usd_cents: 0.0,
                    backend: Some(text_resp.model.clone()),
                    tokens_input: Some(text_resp.input_tokens),
                    tokens_output: Some(text_resp.output_tokens),
                },
                notes: vec![note],
            });
        }

        let mut effects: Vec<VerbEffect> = Vec::new();
        let mut notes: Vec<String> = Vec::new();

        for call in &text_resp.tool_calls {
            debug!(tool = %call.name, "processing VLM tool call");
            match Self::tool_call_to_effect(&call.name, &call.input, sprite_id, &mut notes) {
                Ok(Some(effect)) => effects.push(effect),
                Ok(None) => {}
                Err(e) => {
                    notes.push(format!("Skipped tool call \"{}\": {e}", call.name));
                }
            }
        }

        if effects.is_empty() && notes.is_empty() {
            return Err(VerbError::Backend(
                "conversational: VLM produced tool calls that all failed to parse".into(),
            ));
        }

        let summary = build_summary(&effects, &notes, &inputs.instruction);

        progress.step(Some(1.0), "plan ready").await;

        Ok(VerbOutput {
            summary,
            effects,
            thumbnail: None,
            actual_cost: ActualCost {
                elapsed: started.elapsed(),
                usd_cents: 0.0,
                backend: Some(text_resp.model),
                tokens_input: Some(text_resp.input_tokens),
                tokens_output: Some(text_resp.output_tokens),
            },
            notes,
        })
    }
}

// ── Helpers ────────────────────────────────────────────────────────────────

fn string_field(input: &serde_json::Value, field: &str) -> Result<String> {
    input
        .get(field)
        .and_then(serde_json::Value::as_str)
        .map(std::borrow::ToOwned::to_owned)
        .ok_or_else(|| {
            VerbError::Backend(format!(
                "tool input missing required string field \"{field}\""
            ))
        })
}

fn u64_field(input: &serde_json::Value, field: &str) -> Result<u64> {
    input
        .get(field)
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| {
            VerbError::Backend(format!(
                "tool input missing required integer field \"{field}\""
            ))
        })
}

fn parse_blend_mode(s: &str) -> Option<pixhaus_core::project::BlendMode> {
    use pixhaus_core::project::BlendMode;
    match s.to_ascii_lowercase().as_str() {
        "normal" => Some(BlendMode::Normal),
        "multiply" => Some(BlendMode::Multiply),
        "screen" => Some(BlendMode::Screen),
        "overlay" => Some(BlendMode::Overlay),
        "darken" => Some(BlendMode::Darken),
        "lighten" => Some(BlendMode::Lighten),
        "color_dodge" | "colordodge" => Some(BlendMode::ColorDodge),
        "color_burn" | "colorburn" => Some(BlendMode::ColorBurn),
        "hard_light" | "hardlight" => Some(BlendMode::HardLight),
        "soft_light" | "softlight" => Some(BlendMode::SoftLight),
        "difference" => Some(BlendMode::Difference),
        "exclusion" => Some(BlendMode::Exclusion),
        "addition" | "add" => Some(BlendMode::Addition),
        "subtract" => Some(BlendMode::Subtract),
        "divide" => Some(BlendMode::Divide),
        "hue" => Some(BlendMode::Hue),
        "saturation" => Some(BlendMode::Saturation),
        "color" => Some(BlendMode::Color),
        "luminosity" => Some(BlendMode::Luminosity),
        _ => None,
    }
}

fn parse_loop_direction(s: &str) -> Option<LoopDirection> {
    match s {
        "forward" => Some(LoopDirection::Forward),
        "reverse" => Some(LoopDirection::Reverse),
        "ping_pong" => Some(LoopDirection::PingPong),
        "ping_pong_reverse" => Some(LoopDirection::PingPongReverse),
        _ => None,
    }
}

fn build_summary(effects: &[VerbEffect], notes: &[String], instruction: &str) -> String {
    let op_count = effects.len();
    let note_count = notes.len();
    let truncated = instruction.chars().take(60).collect::<String>();
    let ellipsis = if instruction.len() > 60 { "…" } else { "" };
    match (op_count, note_count) {
        (0, n) => format!(
            "\"{truncated}{ellipsis}\" — {n} pixel edit note{}",
            if n == 1 { "" } else { "s" }
        ),
        (e, 0) => format!(
            "\"{truncated}{ellipsis}\" — {e} operation{}",
            if e == 1 { "" } else { "s" }
        ),
        (e, n) => format!(
            "\"{truncated}{ellipsis}\" — {e} operation{} + {n} pixel edit note{}",
            if e == 1 { "" } else { "s" },
            if n == 1 { "" } else { "s" },
        ),
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backends::{BackendError, InferenceRequest};
    use crate::plugin::descriptor::{BackendCapabilities, CostEstimate as BackendCostEstimate};
    use pixhaus_core::project::{FrameRange, ProjectMetadata, Size, Sprite, SpriteId};

    // ── Stub backend ──────────────────────────────────────────────────────

    /// Stub backend that returns a fixed set of tool calls.
    #[derive(Debug)]
    struct StubVlm {
        /// Tool calls to include in the response.
        tool_calls: Vec<crate::backends::ToolCall>,
    }

    impl StubVlm {
        fn with_calls(calls: Vec<crate::backends::ToolCall>) -> Arc<Self> {
            Arc::new(Self { tool_calls: calls })
        }

        fn empty_response() -> Arc<Self> {
            Arc::new(Self { tool_calls: vec![] })
        }
    }

    #[async_trait::async_trait]
    impl InferenceBackend for StubVlm {
        fn backend_id(&self) -> &'static str {
            "stub-vlm"
        }

        fn capabilities(&self) -> BackendCapabilities {
            BackendCapabilities::VISION_LANGUAGE.union(BackendCapabilities::TOOL_USE)
        }

        fn supports_streaming(&self) -> bool {
            false
        }

        fn estimate_cost(&self, _: &InferenceRequest) -> BackendCostEstimate {
            BackendCostEstimate::free()
        }

        async fn invoke(
            &self,
            _req: InferenceRequest,
            _progress: crate::plugin::progress::VerbProgress,
            _cancel: CancellationToken,
        ) -> crate::backends::Result<InferenceResponse> {
            Ok(InferenceResponse::Text(crate::backends::TextGenResponse {
                content: String::new(),
                model: "stub".into(),
                input_tokens: 10,
                output_tokens: 20,
                stop_reason: Some("tool_use".into()),
                tool_calls: self.tool_calls.clone(),
            }))
        }
    }

    // ── Fixtures ──────────────────────────────────────────────────────────

    fn metadata() -> ProjectMetadata {
        ProjectMetadata {
            name: "conv-test".into(),
            description: None,
            author: None,
            created_at: 0,
            updated_at: 0,
            editor_version: "0".into(),
        }
    }

    fn sprite_with_layers() -> Sprite {
        let mut s = Sprite::empty(SpriteId::new(1), "Knight", Size::new(32, 32));
        s.layers.push(Layer::raster(LayerId::new(1), "Body"));
        s.layers.push(Layer::raster(LayerId::new(2), "Outline"));
        s.frames.push(Frame::default());
        s.frames.push(Frame::default());
        s
    }

    fn ctx_with_sprite() -> VerbContext {
        use crate::plugin::context::VerbContextBuilder;
        VerbContextBuilder::new(metadata())
            .with_sprite(sprite_with_layers())
            .with_active_frame(FrameIndex::new(0))
            .build()
    }

    // ── Descriptor tests ──────────────────────────────────────────────────

    #[test]
    fn descriptor_advertises_vlm_and_tool_use() {
        let verb = ConversationalVerb::new(StubVlm::empty_response());
        let caps = verb.descriptor().required_capabilities;
        assert!(caps.contains(BackendCapabilities::VISION_LANGUAGE));
        assert!(caps.contains(BackendCapabilities::TOOL_USE));
        assert!(verb.descriptor().cancellable);
        assert!(verb.descriptor().streaming);
    }

    #[test]
    fn validate_rejects_empty_instruction() {
        let verb = ConversationalVerb::new(StubVlm::empty_response());
        let inputs = VerbInputs::from_struct(&ConversationalInputs {
            instruction: "   ".into(),
        })
        .unwrap();
        assert!(verb.validate(&inputs).is_err());
    }

    #[test]
    fn validate_accepts_non_empty_instruction() {
        let verb = ConversationalVerb::new(StubVlm::empty_response());
        let inputs = VerbInputs::from_struct(&ConversationalInputs {
            instruction: "add a scar layer".into(),
        })
        .unwrap();
        assert!(verb.validate(&inputs).is_ok());
    }

    // ── Sprite context tests ───────────────────────────────────────────────

    #[test]
    fn sprite_context_includes_layer_names() {
        let ctx = ctx_with_sprite();
        let summary = ConversationalVerb::sprite_context(&ctx);
        assert!(summary.contains("Body"), "expected 'Body' in: {summary}");
        assert!(
            summary.contains("Outline"),
            "expected 'Outline' in: {summary}"
        );
        assert!(
            summary.contains("Knight"),
            "expected sprite name in: {summary}"
        );
    }

    #[test]
    fn sprite_context_no_sprite_returns_placeholder() {
        let ctx = VerbContext::empty(metadata());
        let summary = ConversationalVerb::sprite_context(&ctx);
        assert!(
            summary.contains("No sprite"),
            "expected placeholder in: {summary}"
        );
    }

    // ── Tool definition tests ─────────────────────────────────────────────

    #[test]
    fn tool_defs_cover_all_expected_operations() {
        let tools = ConversationalVerb::tool_defs();
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        for expected in &[
            "add_layer",
            "add_frames",
            "add_animation_tag",
            "set_frame_durations",
            "rename_layer",
            "set_layer_opacity",
            "set_layer_visibility",
            "note_pixel_edit",
        ] {
            assert!(names.contains(expected), "missing tool: {expected}");
        }
    }

    // ── Effect conversion tests ────────────────────────────────────────────

    #[test]
    fn add_layer_tool_produces_add_layer_effect() {
        let sprite_id = SpriteId::new(1);
        let mut notes = Vec::new();
        #[allow(clippy::disallowed_methods)]
        let input = serde_json::json!({"name": "Scar", "opacity": 200, "blend_mode": "multiply"});
        let effect =
            ConversationalVerb::tool_call_to_effect("add_layer", &input, sprite_id, &mut notes)
                .unwrap()
                .unwrap();
        match effect {
            VerbEffect::AddLayer { layer, .. } => {
                assert_eq!(layer.name, "Scar");
                assert_eq!(layer.opacity, 200);
                assert_eq!(layer.blend_mode, pixhaus_core::project::BlendMode::Multiply);
            }
            other => panic!("unexpected effect: {other:?}"),
        }
        assert!(notes.is_empty());
    }

    #[test]
    fn add_layer_missing_name_returns_error() {
        let mut notes = Vec::new();
        #[allow(clippy::disallowed_methods)]
        let input = serde_json::json!({"opacity": 255});
        let result = ConversationalVerb::tool_call_to_effect(
            "add_layer",
            &input,
            SpriteId::new(1),
            &mut notes,
        );
        assert!(result.is_err());
    }

    #[test]
    fn add_frames_defaults_to_100ms() {
        let mut notes = Vec::new();
        #[allow(clippy::disallowed_methods)]
        let input = serde_json::json!({"count": 4});
        let effect = ConversationalVerb::tool_call_to_effect(
            "add_frames",
            &input,
            SpriteId::new(1),
            &mut notes,
        )
        .unwrap()
        .unwrap();
        match effect {
            VerbEffect::AddFrames { frames, after, .. } => {
                assert_eq!(frames.len(), 4);
                assert!(frames.iter().all(|f| f.duration_ms == 100));
                assert!(after.is_none());
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn add_animation_tag_parses_direction() {
        let mut notes = Vec::new();
        #[allow(clippy::disallowed_methods)]
        let input = serde_json::json!({
            "name": "walk",
            "from_frame": 0,
            "to_frame": 7,
            "direction": "ping_pong"
        });
        let effect = ConversationalVerb::tool_call_to_effect(
            "add_animation_tag",
            &input,
            SpriteId::new(1),
            &mut notes,
        )
        .unwrap()
        .unwrap();
        match effect {
            VerbEffect::AddTag { tag, .. } => {
                assert_eq!(tag.name, "walk");
                assert_eq!(tag.range.start.get(), 0);
                assert_eq!(tag.range.end.get(), 7);
                assert_eq!(tag.loop_direction, LoopDirection::PingPong);
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn set_frame_durations_emits_custom_effect() {
        let mut notes = Vec::new();
        #[allow(clippy::disallowed_methods)]
        let input = serde_json::json!({"target_fps": 8.0, "tag_name": "walk"});
        let effect = ConversationalVerb::tool_call_to_effect(
            "set_frame_durations",
            &input,
            SpriteId::new(1),
            &mut notes,
        )
        .unwrap()
        .unwrap();
        match effect {
            VerbEffect::Custom { name, payload } => {
                assert!(name.contains("set_frame_durations"));
                assert_eq!(payload["target_fps"], 8.0);
                assert_eq!(payload["tag_name"], "walk");
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn note_pixel_edit_goes_to_notes_not_effects() {
        let mut notes = Vec::new();
        #[allow(clippy::disallowed_methods)]
        let input = serde_json::json!({
            "description": "Add a scar above the left eye",
            "layer": "Face",
            "frame": 0
        });
        let effect = ConversationalVerb::tool_call_to_effect(
            "note_pixel_edit",
            &input,
            SpriteId::new(1),
            &mut notes,
        )
        .unwrap();
        assert!(effect.is_none());
        assert_eq!(notes.len(), 1);
        assert!(notes[0].contains("scar"), "note: {}", notes[0]);
        assert!(notes[0].contains("Face"), "note: {}", notes[0]);
    }

    #[test]
    fn unknown_tool_returns_none_without_error() {
        let mut notes = Vec::new();
        #[allow(clippy::disallowed_methods)]
        let input = serde_json::json!({});
        let result = ConversationalVerb::tool_call_to_effect(
            "completely_unknown_tool",
            &input,
            SpriteId::new(1),
            &mut notes,
        );
        assert!(result.unwrap().is_none());
        assert!(notes.is_empty());
    }

    // ── End-to-end invocation tests ───────────────────────────────────────

    #[tokio::test]
    async fn verb_converts_add_layer_tool_call_to_effect() {
        let calls = vec![crate::backends::ToolCall {
            id: "tc1".into(),
            name: "add_layer".into(),
            #[allow(clippy::disallowed_methods)]
            input: serde_json::json!({"name": "Scar", "opacity": 180}),
        }];
        let verb = ConversationalVerb::new(StubVlm::with_calls(calls));
        let inputs = VerbInputs::from_struct(&ConversationalInputs {
            instruction: "add a scar layer".into(),
        })
        .unwrap();
        let (progress, _rx) = VerbProgress::channel();
        let output = verb
            .invoke(
                ctx_with_sprite(),
                inputs,
                progress,
                CancellationToken::new(),
            )
            .await
            .unwrap();

        assert_eq!(output.effects.len(), 1);
        assert!(matches!(output.effects[0], VerbEffect::AddLayer { .. }));
        assert!(output.notes.is_empty());
    }

    #[tokio::test]
    async fn verb_mixed_effects_and_notes() {
        let calls = vec![
            crate::backends::ToolCall {
                id: "tc1".into(),
                name: "add_layer".into(),
                #[allow(clippy::disallowed_methods)]
                input: serde_json::json!({"name": "Scar"}),
            },
            crate::backends::ToolCall {
                id: "tc2".into(),
                name: "note_pixel_edit".into(),
                #[allow(clippy::disallowed_methods)]
                input: serde_json::json!({"description": "Draw scar on frame 0"}),
            },
            crate::backends::ToolCall {
                id: "tc3".into(),
                name: "set_frame_durations".into(),
                #[allow(clippy::disallowed_methods)]
                input: serde_json::json!({"target_fps": 8.0}),
            },
        ];

        let verb = ConversationalVerb::new(StubVlm::with_calls(calls));
        let inputs = VerbInputs::from_struct(&ConversationalInputs {
            instruction: "make enemy angrier, add scar, slow walk to 8fps".into(),
        })
        .unwrap();
        let (progress, _rx) = VerbProgress::channel();
        let output = verb
            .invoke(
                ctx_with_sprite(),
                inputs,
                progress,
                CancellationToken::new(),
            )
            .await
            .unwrap();

        assert_eq!(output.effects.len(), 2, "expected AddLayer + Custom effect");
        assert_eq!(output.notes.len(), 1, "expected 1 pixel edit note");
        assert!(output.summary.contains("2 operations"));
        assert!(output.summary.contains("1 pixel edit note"));
    }

    #[tokio::test]
    async fn verb_requires_active_sprite() {
        let verb = ConversationalVerb::new(StubVlm::empty_response());
        let inputs = VerbInputs::from_struct(&ConversationalInputs {
            instruction: "do something".into(),
        })
        .unwrap();
        let (progress, _rx) = VerbProgress::channel();
        let result = verb
            .invoke(
                VerbContext::empty(metadata()),
                inputs,
                progress,
                CancellationToken::new(),
            )
            .await;
        assert!(matches!(result, Err(VerbError::MissingContext(_))));
    }

    #[tokio::test]
    async fn verb_surfaces_text_only_response_as_note() {
        #[derive(Debug)]
        struct TextOnlyVlm;

        #[async_trait::async_trait]
        impl InferenceBackend for TextOnlyVlm {
            fn backend_id(&self) -> &'static str {
                "text-only"
            }
            fn capabilities(&self) -> BackendCapabilities {
                BackendCapabilities::VISION_LANGUAGE.union(BackendCapabilities::TOOL_USE)
            }
            fn supports_streaming(&self) -> bool {
                false
            }
            fn estimate_cost(&self, _: &InferenceRequest) -> BackendCostEstimate {
                BackendCostEstimate::free()
            }
            async fn invoke(
                &self,
                _req: InferenceRequest,
                _progress: VerbProgress,
                _cancel: CancellationToken,
            ) -> crate::backends::Result<InferenceResponse> {
                Ok(InferenceResponse::Text(crate::backends::TextGenResponse {
                    content: "I need more information about which layer to modify.".into(),
                    model: "text-only-stub".into(),
                    input_tokens: 5,
                    output_tokens: 15,
                    stop_reason: Some("end_turn".into()),
                    tool_calls: vec![],
                }))
            }
        }

        let verb = ConversationalVerb::new(Arc::new(TextOnlyVlm));
        let inputs = VerbInputs::from_struct(&ConversationalInputs {
            instruction: "make it look better".into(),
        })
        .unwrap();
        let (progress, _rx) = VerbProgress::channel();
        let output = verb
            .invoke(
                ctx_with_sprite(),
                inputs,
                progress,
                CancellationToken::new(),
            )
            .await
            .unwrap();

        assert!(output.effects.is_empty());
        assert_eq!(output.notes.len(), 1);
        assert!(
            output.notes[0].contains("need more information"),
            "note: {}",
            output.notes[0]
        );
        assert_eq!(output.summary, "No operations planned");
    }

    #[tokio::test]
    async fn verb_cancels_before_backend_call() {
        #[derive(Debug)]
        struct NeverReturnsVlm;
        #[async_trait::async_trait]
        impl InferenceBackend for NeverReturnsVlm {
            fn backend_id(&self) -> &'static str {
                "never"
            }
            fn capabilities(&self) -> BackendCapabilities {
                BackendCapabilities::VISION_LANGUAGE.union(BackendCapabilities::TOOL_USE)
            }
            fn supports_streaming(&self) -> bool {
                false
            }
            fn estimate_cost(&self, _: &InferenceRequest) -> BackendCostEstimate {
                BackendCostEstimate::free()
            }
            async fn invoke(
                &self,
                _req: InferenceRequest,
                _progress: VerbProgress,
                cancel: CancellationToken,
            ) -> crate::backends::Result<InferenceResponse> {
                cancel.cancelled().await;
                Err(BackendError::Cancelled)
            }
        }

        let verb = ConversationalVerb::new(Arc::new(NeverReturnsVlm));
        let inputs = VerbInputs::from_struct(&ConversationalInputs {
            instruction: "do something".into(),
        })
        .unwrap();
        let (progress, _rx) = VerbProgress::channel();
        let cancel = CancellationToken::new();
        cancel.cancel();

        let result = verb
            .invoke(ctx_with_sprite(), inputs, progress, cancel)
            .await;
        assert!(matches!(result, Err(VerbError::Cancelled)));
    }

    #[test]
    fn build_summary_single_operation() {
        let effects = vec![VerbEffect::AddTag {
            sprite: SpriteId::new(1),
            tag: FrameTag {
                name: "walk".into(),
                range: FrameRange::new(FrameIndex::new(0), FrameIndex::new(3)),
                loop_direction: LoopDirection::Forward,
                repeat: 0,
                user_data: UserData::default(),
            },
        }];
        let summary = build_summary(&effects, &[], "add walk tag");
        assert!(summary.contains("1 operation"), "summary: {summary}");
        assert!(!summary.contains("operations"), "summary: {summary}");
    }

    #[test]
    fn build_summary_truncates_long_instruction() {
        let long = "a".repeat(80);
        let summary = build_summary(&[], &["note".into()], &long);
        assert!(summary.contains('…'), "expected ellipsis in: {summary}");
    }
}
