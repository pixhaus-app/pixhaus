//! Verb: Sketch finishing (S36).
//!
//! Refines rough sketches / gesture poses into finished pixel art sprites
//! in the project's learned style. The artist draws silhouettes or stick
//! figures on the active layer; invoking this verb calls the image-edit
//! backend with an optional style prompt and returns a new "AI finish" layer.
//!
//! # Backend
//!
//! Requires [`crate::plugin::descriptor::BackendCapabilities::IMAGE_EDIT`].
//! Backend routing is delegated to [`crate::verbs::call_image_edit`],
//! which handles the concrete-adapter downcast (Stability / `OpenAI` /
//! Replicate, plus the `BackendProxy` path for tests).
//!
//! # Wire format
//!
//! Inputs are serialized via [`VerbInputs::from_struct`] / `deserialize_owned`.
//! The `sketches` vec carries raw RGBA8 frames; each entry maps to one
//! output cel in the new layer.

use std::time::{Duration, Instant};

use async_trait::async_trait;
use pixhaus_core::project::{Cel, FrameIndex, Layer, LayerId, PixelBufferId, Size};
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use crate::backends::ImageEditRequest;
use crate::plugin::context::{PixelData, StyleReference, VerbContext};
use crate::plugin::descriptor::{
    BackendCapabilities, CostEstimate, EffectKind, VerbDescriptor, VerbId,
};
use crate::plugin::error::{Result, VerbError};
use crate::plugin::inputs::VerbInputs;
use crate::plugin::output::{ActualCost, NewPixelBuffer, VerbEffect, VerbOutput};
use crate::plugin::progress::{VerbProgress, VerbProgressEvent};
use crate::plugin::verb::Verb;

/// Stable identifier for the sketch-finishing verb.
pub const SKETCH_FINISHING_VERB_ID: &str = "pixhaus.ai.sketch_finishing";

/// One sketch frame the verb will refine.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SketchFrame {
    /// Frame index in the sprite's timeline.
    pub frame_index: FrameIndex,
    /// Rough sketch pixel data (silhouette / gesture / stick figure).
    /// Must be RGBA8 (`bytes_per_pixel == 4`).
    pub pixels: PixelData,
}

/// Inputs for [`SketchFinishingVerb`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SketchFinishingInputs {
    /// Sketch frames to refine. At least one entry is required.
    pub sketches: Vec<SketchFrame>,
    /// Optional style description appended after the base pixel-art prompt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style_prompt: Option<String>,
    /// Display name for the new refined layer. Defaults to `"AI finish"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layer_name: Option<String>,
}

/// Verb: Sketch finishing.
///
/// Refines rough sketches into finished pixel art sprites using an
/// image-edit backend. Input is one or more sketch frames; output is a
/// new layer with refined pixel art in the project style.
#[derive(Debug)]
pub struct SketchFinishingVerb {
    descriptor: VerbDescriptor,
}

impl SketchFinishingVerb {
    /// Constructs the verb with its default descriptor.
    #[must_use]
    // `serde_json::json!` expands to internal helpers that call `unwrap` on
    // infallible builders. We exempt the constructor rather than open-coding
    // the schema as a `Value::Object`.
    #[allow(clippy::disallowed_methods)]
    pub fn new() -> Self {
        let input_schema = serde_json::json!({
            "type": "object",
            "properties": {
                "sketches": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "frame_index": {"type": "integer", "minimum": 0},
                            "pixels": {"$ref": "#/$defs/PixelData"}
                        },
                        "required": ["frame_index", "pixels"]
                    },
                    "minItems": 1
                },
                "style_prompt": {"type": ["string", "null"]},
                "layer_name":   {"type": ["string", "null"]}
            },
            "required": ["sketches"],
            "$defs": {
                "PixelData": {
                    "type": "object",
                    "properties": {
                        "width":           {"type": "integer", "minimum": 1},
                        "height":          {"type": "integer", "minimum": 1},
                        "bytes_per_pixel": {"type": "integer"},
                        "stride":          {"type": "integer"},
                        "bytes":           {"type": "array", "items": {"type": "integer"}}
                    },
                    "required": ["width", "height", "bytes_per_pixel", "stride", "bytes"]
                }
            }
        });

        Self {
            descriptor: VerbDescriptor {
                id: VerbId::new(SKETCH_FINISHING_VERB_ID),
                display_name: "Sketch finishing".into(),
                description: "Refines rough sketches into finished pixel art in the project style"
                    .into(),
                version: env!("CARGO_PKG_VERSION").into(),
                required_capabilities: BackendCapabilities::IMAGE_EDIT,
                input_schema,
                output_schema: None,
                output_kinds: vec![EffectKind::AddLayer],
                cost_estimate: CostEstimate {
                    typical_latency: Duration::from_secs(10),
                    max_latency: Duration::from_secs(30),
                    typical_usd_cents: 3.5,
                    max_usd_cents: 6.5,
                },
                streaming: true,
                cancellable: true,
                documentation_url: Some("docs/verbs/sketch_finishing.md".into()),
            },
        }
    }
}

impl Default for SketchFinishingVerb {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Verb for SketchFinishingVerb {
    fn descriptor(&self) -> &VerbDescriptor {
        &self.descriptor
    }

    fn validate(&self, inputs: &VerbInputs) -> Result<()> {
        let parsed: SketchFinishingInputs = inputs.deserialize()?;
        if parsed.sketches.is_empty() {
            return Err(VerbError::Schema("sketches must not be empty".into()));
        }
        for (i, sketch) in parsed.sketches.iter().enumerate() {
            if sketch.pixels.bytes_per_pixel != 4 {
                return Err(VerbError::Schema(format!(
                    "sketch[{i}]: only RGBA8 pixels are supported (bytes_per_pixel must be 4)"
                )));
            }
            if !sketch.pixels.is_well_formed() {
                return Err(VerbError::Schema(format!(
                    "sketch[{i}]: pixel buffer dimensions and byte count are inconsistent"
                )));
            }
        }
        Ok(())
    }

    async fn invoke(
        &self,
        ctx: VerbContext,
        inputs: VerbInputs,
        progress: VerbProgress,
        cancel: CancellationToken,
    ) -> Result<VerbOutput> {
        let started = Instant::now();
        let inputs: SketchFinishingInputs = inputs.deserialize_owned()?;
        let sprite_id = ctx.require_sprite_id()?;

        let backend = ctx.backend.as_ref().ok_or(VerbError::MissingContext(
            "image-edit backend (register StabilityBackend, OpenAiBackend, or ReplicateBackend)",
        ))?;

        progress
            .send(VerbProgressEvent::Started {
                backend: Some(backend.id().to_owned()),
            })
            .await;

        let layer_name = inputs
            .layer_name
            .clone()
            .unwrap_or_else(|| "AI finish".into());
        let base_prompt = build_prompt(inputs.style_prompt.as_deref(), &ctx);

        let total = inputs.sketches.len();
        let placeholder_layer_id = LayerId::new(0);
        let mut cels = Vec::with_capacity(total);
        let mut pixel_buffers = Vec::with_capacity(total);

        for (i, sketch) in inputs.sketches.iter().enumerate() {
            if cancel.is_cancelled() {
                return Err(VerbError::Cancelled);
            }

            #[allow(clippy::cast_precision_loss)]
            let fraction = i as f32 / total as f32;
            progress
                .step(
                    Some(fraction),
                    format!("refining frame {} of {total}", i + 1),
                )
                .await;

            let sketch_png = encode_png(&sketch.pixels)?;
            // B10.3 anchor: pass through the active entity's anchor
            // sheet (if any) as a style condition so the finished
            // sketch inherits the Reference's look.
            let req = ImageEditRequest {
                model: None,
                image: sketch_png,
                mask: None,
                prompt: base_prompt.clone(),
                negative_prompt: Some("blurry, noisy, sketch lines, rough edges, artifacts".into()),
                num_images: 1,
                style_image: crate::verbs::anchor_style_image_bytes(&ctx),
                reference_images: Vec::new(),
            };

            let images = crate::verbs::call_image_edit(
                backend.as_ref(),
                req,
                progress.clone(),
                cancel.clone(),
            )
            .await?;

            let raw_png = images
                .into_iter()
                .next()
                .ok_or_else(|| VerbError::Backend("backend returned no images".into()))?;

            let refined = decode_png(&raw_png)?;
            #[allow(clippy::cast_possible_truncation)]
            let buf_placeholder = PixelBufferId::new(i as u32);

            pixel_buffers.push(NewPixelBuffer {
                placeholder: buf_placeholder,
                pixels: refined.clone(),
            });

            cels.push(Cel::raster(
                placeholder_layer_id,
                sketch.frame_index,
                buf_placeholder,
                Size::new(refined.width, refined.height),
            ));
        }

        progress.step(Some(1.0), "sketch finishing complete").await;

        let elapsed = started.elapsed();
        let frame_count = cels.len();
        let summary = format!(
            "Add layer \"{layer_name}\" with {frame_count} refined {}",
            if frame_count == 1 { "frame" } else { "frames" },
        );

        let layer = Layer::raster(placeholder_layer_id, &layer_name);
        let thumbnail = pixel_buffers.first().map(|pb| pb.pixels.clone());

        Ok(VerbOutput {
            summary,
            effects: vec![VerbEffect::AddLayer {
                sprite: sprite_id,
                layer,
                cels,
                pixel_buffers,
            }],
            thumbnail,
            actual_cost: ActualCost::free(elapsed.max(Duration::from_micros(1))),
            notes: vec![],
        })
    }
}

// ── Helpers ────────────────────────────────────────────────────────────────

/// Builds the image-edit prompt from the optional user string and any style
/// references carried in the context.
fn build_prompt(style_prompt: Option<&str>, ctx: &VerbContext) -> String {
    let mut parts: Vec<String> = vec![
        "Convert this rough sketch into finished pixel art. \
         Preserve the pose and proportions exactly. \
         Output clean, crisp pixel art with a limited palette."
            .into(),
    ];

    for style_ref in &ctx.style_refs {
        match style_ref {
            StyleReference::StyleSheet { label, .. } => {
                parts.push(format!("Match the visual style: {label}."));
            }
            StyleReference::TrainedModel { label, .. } => {
                parts.push(format!("Use trained style: {label}."));
            }
            StyleReference::Palette { name, .. } => {
                parts.push(format!("Restrict colours to palette: {name}."));
            }
        }
    }

    if let Some(p) = style_prompt {
        parts.push(p.to_owned());
    }

    parts.join(" ")
}

/// Encodes a [`PixelData`] (RGBA8) as a PNG byte buffer.
///
/// Strips stride padding so `RgbaImage::from_raw` receives tightly-packed rows.
fn encode_png(pixels: &PixelData) -> Result<Vec<u8>> {
    let stride = pixels.stride as usize;
    let row_bytes = pixels.width as usize * 4;

    let packed = if stride == row_bytes {
        pixels.bytes.clone()
    } else {
        let mut out = Vec::with_capacity(pixels.height as usize * row_bytes);
        for row in 0..pixels.height as usize {
            let start = row * stride;
            out.extend_from_slice(
                pixels
                    .bytes
                    .get(start..start + row_bytes)
                    .ok_or_else(|| VerbError::Schema("pixel buffer slice out of range".into()))?,
            );
        }
        out
    };

    let img = image::RgbaImage::from_raw(pixels.width, pixels.height, packed).ok_or_else(|| {
        VerbError::Schema("pixel buffer dimensions don't match byte count".into())
    })?;
    let dyn_img = image::DynamicImage::ImageRgba8(img);
    let mut cursor = std::io::Cursor::new(Vec::new());
    dyn_img
        .write_to(&mut cursor, image::ImageFormat::Png)
        .map_err(|e| VerbError::Backend(format!("PNG encode failed: {e}")))?;
    Ok(cursor.into_inner())
}

/// Decodes PNG bytes into a tightly-packed RGBA8 [`PixelData`].
fn decode_png(bytes: &[u8]) -> Result<PixelData> {
    let img = image::load_from_memory(bytes)
        .map_err(|e| VerbError::Backend(format!("PNG decode failed: {e}")))?
        .to_rgba8();
    let (width, height) = img.dimensions();
    Ok(PixelData::rgba8(width, height, img.into_raw()))
}

#[cfg(test)]
mod tests {
    use std::any::Any;
    use std::time::Duration;

    use super::*;
    use crate::plugin::backend::InferenceBackend as PluginBackend;
    use crate::plugin::runtime::VerbRuntime;
    use pixhaus_core::project::{ProjectMetadata, SpriteId};

    fn metadata() -> ProjectMetadata {
        ProjectMetadata {
            name: "s36-test".into(),
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
        ctx.active_frame = Some(FrameIndex::new(0));
        ctx
    }

    fn red_sketch() -> SketchFrame {
        SketchFrame {
            frame_index: FrameIndex::new(0),
            pixels: PixelData::rgba8(
                2,
                2,
                vec![255, 0, 0, 255, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            ),
        }
    }

    /// Stub backend that claims `IMAGE_EDIT` but is not a known concrete adapter.
    /// Used to satisfy the runtime's capability check in tests that need to
    /// exercise verb-side error paths rather than the runtime's pre-flight.
    #[derive(Debug)]
    struct StubImageEditBackend;

    impl PluginBackend for StubImageEditBackend {
        fn as_any(&self) -> &dyn Any {
            self
        }
        fn id(&self) -> &'static str {
            "stub.image_edit"
        }
        fn capabilities(&self) -> BackendCapabilities {
            BackendCapabilities::IMAGE_EDIT
        }
        fn cost_estimate(&self, _required: BackendCapabilities) -> CostEstimate {
            CostEstimate {
                typical_latency: Duration::from_millis(1),
                max_latency: Duration::from_millis(10),
                typical_usd_cents: 0.0,
                max_usd_cents: 0.0,
            }
        }
    }

    #[test]
    fn descriptor_requires_image_edit() {
        let verb = SketchFinishingVerb::new();
        assert!(
            verb.descriptor()
                .required_capabilities
                .contains(BackendCapabilities::IMAGE_EDIT)
        );
        assert!(verb.descriptor().cancellable);
        assert!(verb.descriptor().streaming);
    }

    #[test]
    fn validate_rejects_empty_sketches() {
        let verb = SketchFinishingVerb::new();
        let inputs = VerbInputs::from_struct(&SketchFinishingInputs {
            sketches: vec![],
            style_prompt: None,
            layer_name: None,
        })
        .unwrap();
        assert!(matches!(verb.validate(&inputs), Err(VerbError::Schema(_))));
    }

    #[test]
    fn validate_rejects_non_rgba8() {
        let verb = SketchFinishingVerb::new();
        let bad_sketch = SketchFrame {
            frame_index: FrameIndex::new(0),
            pixels: PixelData {
                width: 2,
                height: 2,
                bytes_per_pixel: 3,
                stride: 6,
                bytes: vec![0u8; 12],
            },
        };
        let inputs = VerbInputs::from_struct(&SketchFinishingInputs {
            sketches: vec![bad_sketch],
            style_prompt: None,
            layer_name: None,
        })
        .unwrap();
        assert!(matches!(verb.validate(&inputs), Err(VerbError::Schema(_))));
    }

    #[test]
    fn validate_rejects_malformed_pixel_buffer() {
        let verb = SketchFinishingVerb::new();
        let bad_sketch = SketchFrame {
            frame_index: FrameIndex::new(0),
            pixels: PixelData {
                width: 4,
                height: 4,
                bytes_per_pixel: 4,
                stride: 16,
                bytes: vec![0u8; 8], // should be 64 bytes
            },
        };
        let inputs = VerbInputs::from_struct(&SketchFinishingInputs {
            sketches: vec![bad_sketch],
            style_prompt: None,
            layer_name: None,
        })
        .unwrap();
        assert!(matches!(verb.validate(&inputs), Err(VerbError::Schema(_))));
    }

    #[test]
    fn validate_accepts_well_formed_sketch() {
        let verb = SketchFinishingVerb::new();
        let inputs = VerbInputs::from_struct(&SketchFinishingInputs {
            sketches: vec![red_sketch()],
            style_prompt: None,
            layer_name: None,
        })
        .unwrap();
        assert!(verb.validate(&inputs).is_ok());
    }

    /// The runtime refuses to start any verb that requires capabilities but
    /// has no matching backend registered.
    #[test]
    fn no_backend_returns_unsupported_capability() {
        let runtime = VerbRuntime::new();
        runtime.register(SketchFinishingVerb::new()).unwrap();

        let inputs = VerbInputs::from_struct(&SketchFinishingInputs {
            sketches: vec![red_sketch()],
            style_prompt: None,
            layer_name: None,
        })
        .unwrap();

        let res = runtime.invoke(
            &VerbId::new(SKETCH_FINISHING_VERB_ID),
            ctx_with_sprite(),
            inputs,
        );
        assert!(matches!(res, Err(VerbError::UnsupportedCapability { .. })));
    }

    /// Verb fails before reaching any backend call when the context carries
    /// no active sprite.
    #[tokio::test]
    async fn requires_active_sprite() {
        let runtime = VerbRuntime::new();
        runtime.register(SketchFinishingVerb::new()).unwrap();
        runtime.register_backend(StubImageEditBackend, 0).unwrap();

        let inputs = VerbInputs::from_struct(&SketchFinishingInputs {
            sketches: vec![red_sketch()],
            style_prompt: None,
            layer_name: None,
        })
        .unwrap();

        let inv = runtime
            .invoke(
                &VerbId::new(SKETCH_FINISHING_VERB_ID),
                VerbContext::empty(metadata()), // no sprite
                inputs,
            )
            .unwrap();
        let res = inv.finish().await;
        assert!(matches!(res, Err(VerbError::MissingContext(_))));
    }

    #[test]
    fn build_prompt_includes_style_refs() {
        let mut ctx = VerbContext::empty(metadata());
        ctx.style_refs.push(StyleReference::StyleSheet {
            label: "warrior-16x".into(),
            pixels: PixelData::rgba8(1, 1, vec![0, 0, 0, 255]),
        });
        let prompt = build_prompt(Some("vibrant colours"), &ctx);
        assert!(prompt.contains("warrior-16x"));
        assert!(prompt.contains("vibrant colours"));
    }

    #[test]
    fn build_prompt_base_only() {
        let ctx = VerbContext::empty(metadata());
        let prompt = build_prompt(None, &ctx);
        assert!(prompt.contains("pixel art"));
        assert!(!prompt.contains("Match"));
    }

    #[test]
    fn encode_decode_png_round_trips() {
        let original = PixelData::rgba8(
            2,
            2,
            vec![
                255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 255,
            ],
        );
        let png = encode_png(&original).unwrap();
        let decoded = decode_png(&png).unwrap();
        assert_eq!(decoded.width, 2);
        assert_eq!(decoded.height, 2);
        assert_eq!(decoded.bytes, original.bytes);
    }

    #[test]
    fn encode_strips_stride_padding() {
        // stride = 12 (2 pixels × 4 bytes + 4 padding), width = 2
        let mut bytes = vec![0u8; 12 * 2]; // 2 rows, 12 bytes each
        bytes[0] = 255; // red channel of pixel (0,0)
        bytes[3] = 255; // alpha of pixel (0,0)
        let padded = PixelData {
            width: 2,
            height: 2,
            bytes_per_pixel: 4,
            stride: 12,
            bytes,
        };
        let png = encode_png(&padded).unwrap();
        let decoded = decode_png(&png).unwrap();
        assert_eq!(decoded.width, 2);
        assert_eq!(decoded.height, 2);
        assert_eq!(decoded.bytes[0], 255);
        assert_eq!(decoded.bytes[3], 255);
    }

    #[test]
    fn inputs_round_trip_json() {
        let original = SketchFinishingInputs {
            sketches: vec![red_sketch()],
            style_prompt: Some("dark fantasy".into()),
            layer_name: Some("finished".into()),
        };
        let json = serde_json::to_string(&original).unwrap();
        let back: SketchFinishingInputs = serde_json::from_str(&json).unwrap();
        assert_eq!(back, original);
    }
}
