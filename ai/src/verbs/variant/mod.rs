//! Verb: Variant — palette swaps, equipment overlays, expression sets.
//!
//! The Variant verb generates one or more derived sprites from a base layer.
//! It operates in two modes:
//!
//! - **Palette swap** (`VariantMode::PaletteSwap`): classical pixel
//!   remapping using an explicit color substitution list. No inference
//!   backend required. Each pixel whose RGBA value exactly matches a
//!   `from` color is replaced with the corresponding `to` color; pixels
//!   that match no substitution are left unchanged.
//!
//! - **Text edit** (`VariantMode::TextEdit`): free-form natural-language
//!   description of the desired variant (e.g. *"same character with blue
//!   armor"*). Requires an [`IMAGE_EDIT`][crate::plugin::descriptor::BackendCapabilities::IMAGE_EDIT]
//!   backend in the registry. The source pixels are PNG-encoded and sent
//!   to the backend alongside the description.
//!
//! Both modes produce one or more new layers via
//! [`VerbEffect::AddLayer`]. The source layer is unchanged.
//!
//! # Brief reference
//!
//! Implements: `docs/planning/work/streams.md#s26`

use std::io::Cursor;
use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use image::{ImageBuffer, ImageFormat, RgbaImage};
use serde::{Deserialize, Serialize};
use tokio::select;
use tokio_util::sync::CancellationToken;

use pixhaus_core::project::{Cel, FrameIndex, Layer, LayerId, PixelBufferId, Rgba, Size, SpriteId};

use crate::backends::{BackendRegistry, ImageEditRequest, InferenceRequest, InferenceResponse};
use crate::plugin::context::{PixelData, VerbContext};
use crate::plugin::descriptor::{
    BackendCapabilities, CostEstimate, EffectKind, VerbDescriptor, VerbId,
};
use crate::plugin::error::{Result, VerbError};
use crate::plugin::inputs::VerbInputs;
use crate::plugin::output::{ActualCost, NewPixelBuffer, VerbEffect, VerbOutput};
use crate::plugin::progress::{VerbProgress, VerbProgressEvent};
use crate::plugin::verb::Verb;

/// Stable ID for the built-in Variant verb.
pub const VARIANT_VERB_ID: &str = "pixhaus.builtin.variant";

/// Effect-local placeholder base for layer and buffer IDs.
/// The host rewrites these to real IDs on commit.
const PLACEHOLDER_BASE: u32 = 0;

// ── Input types ──────────────────────────────────────────────────────────────

/// A single color substitution for palette-swap mode.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ColorSubstitution {
    /// Source color to replace.
    pub from: Rgba,
    /// Target color to substitute in.
    pub to: Rgba,
}

/// Operating mode for the Variant verb.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum VariantMode {
    /// Classical pixel-exact color remapping. No backend required.
    PaletteSwap {
        /// Color substitution list. Pixels whose RGBA value exactly matches a
        /// `from` entry are replaced with the corresponding `to` value;
        /// unmatched pixels are left unchanged.
        substitutions: Vec<ColorSubstitution>,
    },
    /// AI-powered variant from a natural-language description.
    ///
    /// Requires an `IMAGE_EDIT`-capable backend in the
    /// [`BackendRegistry`]. Returns [`VerbError::Backend`] if no
    /// matching backend is registered.
    TextEdit {
        /// Description of the desired variant, e.g. *"same pose, blue armor
        /// instead of red, identical palette count"*.
        description: String,
        /// Number of variant images to generate. `0` is treated as `1`.
        #[serde(default = "default_count")]
        count: u32,
    },
}

fn default_count() -> u32 {
    1
}

/// Inputs for the [`VariantVerb`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VariantInputs {
    /// Source pixel data to generate variants from.
    pub pixels: PixelData,
    /// Variant mode.
    pub mode: VariantMode,
    /// Display name prefix for the output layer(s).
    /// Defaults to `"Variant"`. When multiple variants are produced
    /// they are suffixed with a 1-based index: *"Variant 1"*, *"Variant 2"*.
    #[serde(default)]
    pub layer_name: Option<String>,
}

// ── Verb implementation ───────────────────────────────────────────────────────

/// AI verb that generates sprite variants.
///
/// Construct with [`VariantVerb::new`], passing a shared
/// [`BackendRegistry`] so the text-edit path can reach the configured
/// image-edit backend without coupling the verb to a concrete adapter.
pub struct VariantVerb {
    descriptor: VerbDescriptor,
    registry: Arc<BackendRegistry>,
}

impl VariantVerb {
    /// Constructs the verb with the supplied backend registry.
    #[must_use]
    #[allow(clippy::disallowed_methods)]
    pub fn new(registry: Arc<BackendRegistry>) -> Self {
        let input_schema = serde_json::json!({
            "type": "object",
            "properties": {
                "pixels": { "$ref": "#/$defs/PixelData" },
                "mode": {
                    "oneOf": [
                        {
                            "type": "object",
                            "properties": {
                                "kind": { "const": "palette_swap" },
                                "substitutions": {
                                    "type": "array",
                                    "items": {
                                        "type": "object",
                                        "properties": {
                                            "from": { "$ref": "#/$defs/Rgba" },
                                            "to":   { "$ref": "#/$defs/Rgba" }
                                        },
                                        "required": ["from", "to"]
                                    }
                                }
                            },
                            "required": ["kind", "substitutions"]
                        },
                        {
                            "type": "object",
                            "properties": {
                                "kind": { "const": "text_edit" },
                                "description": { "type": "string", "minLength": 1 },
                                "count": { "type": "integer", "minimum": 1, "default": 1 }
                            },
                            "required": ["kind", "description"]
                        }
                    ]
                },
                "layer_name": { "type": ["string", "null"] }
            },
            "required": ["pixels", "mode"],
            "$defs": {
                "Rgba": {
                    "type": "object",
                    "properties": {
                        "r": { "type": "integer", "minimum": 0, "maximum": 255 },
                        "g": { "type": "integer", "minimum": 0, "maximum": 255 },
                        "b": { "type": "integer", "minimum": 0, "maximum": 255 },
                        "a": { "type": "integer", "minimum": 0, "maximum": 255 }
                    },
                    "required": ["r", "g", "b", "a"]
                },
                "PixelData": {
                    "type": "object",
                    "properties": {
                        "width":           { "type": "integer", "minimum": 1 },
                        "height":          { "type": "integer", "minimum": 1 },
                        "bytes_per_pixel": { "type": "integer" },
                        "stride":          { "type": "integer" },
                        "bytes":           { "type": "array", "items": { "type": "integer" } }
                    },
                    "required": ["width", "height", "bytes_per_pixel", "stride", "bytes"]
                }
            }
        });

        Self {
            descriptor: VerbDescriptor {
                id: VerbId::new(VARIANT_VERB_ID),
                display_name: "Variant".into(),
                description:
                    "Generate sprite variants: palette swaps, equipment overlays, expression sets"
                        .into(),
                version: env!("CARGO_PKG_VERSION").into(),
                // No required capabilities: palette-swap path works without
                // any backend. The text-edit path checks the registry at
                // invoke time and returns a clear error when no IMAGE_EDIT
                // backend is available.
                required_capabilities: BackendCapabilities::empty(),
                input_schema,
                output_schema: None,
                output_kinds: vec![EffectKind::AddLayer],
                cost_estimate: CostEstimate {
                    typical_latency: std::time::Duration::from_secs(5),
                    max_latency: std::time::Duration::from_secs(60),
                    typical_usd_cents: 0.5,
                    max_usd_cents: 5.0,
                },
                streaming: true,
                cancellable: true,
                documentation_url: Some("docs/verbs/variant.md".into()),
            },
            registry,
        }
    }
}

impl std::fmt::Debug for VariantVerb {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VariantVerb")
            .field("id", &self.descriptor.id)
            .finish_non_exhaustive()
    }
}

#[async_trait]
impl Verb for VariantVerb {
    fn descriptor(&self) -> &VerbDescriptor {
        &self.descriptor
    }

    fn validate(&self, inputs: &VerbInputs) -> Result<()> {
        let parsed: VariantInputs = inputs.deserialize()?;

        if !parsed.pixels.is_well_formed() {
            return Err(VerbError::Schema(
                "variant: pixel buffer dimensions and byte count are inconsistent".into(),
            ));
        }

        if parsed.pixels.bytes_per_pixel != 4 {
            return Err(VerbError::Schema(
                "variant: only RGBA8 (4 bytes per pixel) is supported".into(),
            ));
        }

        match &parsed.mode {
            VariantMode::PaletteSwap { substitutions } => {
                if substitutions.is_empty() {
                    return Err(VerbError::Schema(
                        "variant: palette_swap mode requires at least one substitution".into(),
                    ));
                }
            }
            VariantMode::TextEdit { description, .. } => {
                if description.trim().is_empty() {
                    return Err(VerbError::Schema(
                        "variant: text_edit mode requires a non-empty description".into(),
                    ));
                }
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
        let inputs: VariantInputs = inputs.deserialize_owned()?;
        let ictx = InvokeCtx {
            sprite_id: ctx.require_sprite_id()?,
            active_frame: ctx.active_frame.unwrap_or(FrameIndex::new(0)),
            started: Instant::now(),
            pixels: inputs.pixels,
            layer_name: inputs.layer_name,
        };
        match inputs.mode {
            VariantMode::PaletteSwap { substitutions } => {
                invoke_palette_swap(substitutions, ictx, progress, cancel).await
            }
            VariantMode::TextEdit { description, count } => {
                self.invoke_text_edit(description, count, ictx, progress, cancel)
                    .await
            }
        }
    }
}

// ── Mode helpers ──────────────────────────────────────────────────────────────

/// Shared invocation context extracted from [`VerbContext`] before dispatch.
struct InvokeCtx {
    sprite_id: SpriteId,
    active_frame: FrameIndex,
    started: Instant,
    pixels: PixelData,
    layer_name: Option<String>,
}

async fn invoke_palette_swap(
    substitutions: Vec<ColorSubstitution>,
    ictx: InvokeCtx,
    progress: VerbProgress,
    cancel: CancellationToken,
) -> Result<VerbOutput> {
    progress
        .send(VerbProgressEvent::Started { backend: None })
        .await;
    if cancel.is_cancelled() {
        return Err(VerbError::Cancelled);
    }
    progress.step(Some(0.3), "applying palette swap").await;
    let swapped = apply_palette_swap(&ictx.pixels, &substitutions);
    if cancel.is_cancelled() {
        return Err(VerbError::Cancelled);
    }
    progress.step(Some(1.0), "done").await;
    let layer_name = ictx.layer_name.unwrap_or_else(|| "Variant".into());
    let sub_count = substitutions.len();
    let (layer, cel, pixel_buffer) =
        make_layer_effect(PLACEHOLDER_BASE, &layer_name, ictx.active_frame, swapped);
    let elapsed = ictx.started.elapsed();
    Ok(VerbOutput {
        summary: format!(
            "Add variant layer \"{layer_name}\" ({}x{}, {sub_count} substitutions)",
            ictx.pixels.width, ictx.pixels.height,
        ),
        effects: vec![VerbEffect::AddLayer {
            sprite: ictx.sprite_id,
            layer,
            cels: vec![cel],
            pixel_buffers: vec![pixel_buffer],
        }],
        thumbnail: None,
        actual_cost: ActualCost::free(elapsed),
        notes: vec![],
    })
}

/// Decodes each image in `images` and builds `AddLayer` effects.
fn build_text_effects(
    images: &[Vec<u8>],
    layer_prefix: &str,
    sprite_id: SpriteId,
    active_frame: FrameIndex,
) -> Result<Vec<VerbEffect>> {
    let mut effects = Vec::with_capacity(images.len());
    for (i, img_bytes) in images.iter().enumerate() {
        let variant_pixels = png_to_pixel_data(img_bytes)?;
        // Each variant occupies its own placeholder ID pair so layer and
        // buffer IDs don't alias across effects within the same output.
        #[allow(clippy::cast_possible_truncation)]
        let placeholder_offset = (i as u32).saturating_mul(2);
        let name = if images.len() == 1 {
            layer_prefix.to_owned()
        } else {
            format!("{layer_prefix} {}", i + 1)
        };
        let (layer, cel, pixel_buffer) = make_layer_effect(
            PLACEHOLDER_BASE + placeholder_offset,
            &name,
            active_frame,
            variant_pixels,
        );
        effects.push(VerbEffect::AddLayer {
            sprite: sprite_id,
            layer,
            cels: vec![cel],
            pixel_buffers: vec![pixel_buffer],
        });
    }
    Ok(effects)
}

impl VariantVerb {
    async fn invoke_text_edit(
        &self,
        description: String,
        count: u32,
        ictx: InvokeCtx,
        progress: VerbProgress,
        cancel: CancellationToken,
    ) -> Result<VerbOutput> {
        let backend = self
            .registry
            .find_for(BackendCapabilities::IMAGE_EDIT)
            .ok_or_else(|| {
                VerbError::Backend(
                    "no IMAGE_EDIT backend is configured; \
                     add a backend in preferences"
                        .into(),
                )
            })?;
        let backend_id = backend.backend_id().to_owned();
        progress
            .send(VerbProgressEvent::Started {
                backend: Some(backend_id.clone()),
            })
            .await;
        if cancel.is_cancelled() {
            return Err(VerbError::Cancelled);
        }
        progress.step(Some(0.1), "encoding source pixels").await;
        let png_bytes = pixel_data_to_png(&ictx.pixels)?;
        if cancel.is_cancelled() {
            return Err(VerbError::Cancelled);
        }
        let req = ImageEditRequest {
            model: None,
            image: png_bytes,
            mask: None,
            prompt: description.clone(),
            negative_prompt: None,
            num_images: count.max(1),
        };
        progress
            .step(Some(0.2), "sending to inference backend")
            .await;
        let response = select! {
            biased;
            () = cancel.cancelled() => return Err(VerbError::Cancelled),
            res = backend.invoke(
                InferenceRequest::ImageEdit(req),
                VerbProgress::discard(),
                cancel.clone(),
            ) => res.map_err(|e| VerbError::Backend(e.to_string()))?,
        };
        if cancel.is_cancelled() {
            return Err(VerbError::Cancelled);
        }
        let images = match response {
            InferenceResponse::Image(r) => r.images,
            InferenceResponse::Text(_) => {
                return Err(VerbError::Backend(
                    "backend returned a text response to an image-edit request".into(),
                ));
            }
            InferenceResponse::Frames(_) => {
                return Err(VerbError::Backend(
                    "backend returned frames to an image-edit request".into(),
                ));
            }
            InferenceResponse::Raw(_) => {
                return Err(VerbError::Backend(
                    "backend returned raw JSON to an image-edit request; \
                     use a typed image-edit adapter"
                        .into(),
                ));
            }
        };
        progress.step(Some(0.9), "decoding variant images").await;
        let layer_prefix = ictx.layer_name.unwrap_or_else(|| "Variant".into());
        let effects =
            build_text_effects(&images, &layer_prefix, ictx.sprite_id, ictx.active_frame)?;
        let elapsed = ictx.started.elapsed();
        let variant_count = effects.len();
        let summary = format!(
            "Add {variant_count} variant layer{} from \"{description}\"",
            if variant_count == 1 { "" } else { "s" }
        );
        Ok(VerbOutput {
            summary,
            effects,
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

/// Applies a palette swap to `pixels`, returning a new `PixelData`.
///
/// Each RGBA8 pixel is compared against each `ColorSubstitution.from`
/// value using exact channel equality. The first matching substitution
/// wins; if none match, the pixel is copied unchanged. Alpha is included
/// in the match so transparent pixels and semi-transparent pixels with
/// the same RGB are treated independently.
fn apply_palette_swap(pixels: &PixelData, substitutions: &[ColorSubstitution]) -> PixelData {
    let mut out = pixels.bytes.clone();
    let stride = pixels.stride as usize;
    let bpp = pixels.bytes_per_pixel as usize;

    for y in 0..pixels.height as usize {
        let row_start = y * stride;
        for x in 0..pixels.width as usize {
            let off = row_start + x * bpp;
            let pixel = Rgba::new(out[off], out[off + 1], out[off + 2], out[off + 3]);
            if let Some(sub) = substitutions.iter().find(|s| s.from == pixel) {
                out[off] = sub.to.r;
                out[off + 1] = sub.to.g;
                out[off + 2] = sub.to.b;
                out[off + 3] = sub.to.a;
            }
        }
    }

    PixelData {
        width: pixels.width,
        height: pixels.height,
        bytes_per_pixel: pixels.bytes_per_pixel,
        stride: pixels.stride,
        bytes: out,
    }
}

/// Encodes a `PixelData` as a PNG byte vector.
///
/// Requires RGBA8 layout (`bytes_per_pixel == 4`). Returns
/// [`VerbError::Backend`] on encode failure.
fn pixel_data_to_png(pixels: &PixelData) -> Result<Vec<u8>> {
    let img: RgbaImage = ImageBuffer::from_fn(pixels.width, pixels.height, |x, y| {
        let off =
            y as usize * pixels.stride as usize + x as usize * pixels.bytes_per_pixel as usize;
        image::Rgba([
            pixels.bytes[off],
            pixels.bytes[off + 1],
            pixels.bytes[off + 2],
            pixels.bytes[off + 3],
        ])
    });

    let mut buf: Vec<u8> = Vec::new();
    img.write_to(&mut Cursor::new(&mut buf), ImageFormat::Png)
        .map_err(|e| VerbError::Backend(format!("PNG encode failed: {e}")))?;
    Ok(buf)
}

/// Decodes a PNG byte slice into a tightly-packed RGBA8 `PixelData`.
fn png_to_pixel_data(png_bytes: &[u8]) -> Result<PixelData> {
    let img = image::load_from_memory(png_bytes)
        .map_err(|e| VerbError::Backend(format!("PNG decode failed: {e}")))?
        .into_rgba8();
    let (width, height) = img.dimensions();
    Ok(PixelData::rgba8(width, height, img.into_raw()))
}

/// Builds the `(Layer, Cel, NewPixelBuffer)` triple for an `AddLayer` effect.
///
/// `placeholder` must be unique within the enclosing output's effect list.
/// The host replaces placeholder IDs with real ones at commit time.
fn make_layer_effect(
    placeholder: u32,
    name: &str,
    frame: FrameIndex,
    pixels: PixelData,
) -> (Layer, Cel, NewPixelBuffer) {
    let layer_id = LayerId::new(placeholder);
    let buffer_id = PixelBufferId::new(placeholder + 1);
    let size = Size::new(pixels.width, pixels.height);

    let layer = Layer::raster(layer_id, name);
    let cel = Cel::raster(layer_id, frame, buffer_id, size);
    let pixel_buffer = NewPixelBuffer {
        placeholder: buffer_id,
        pixels,
    };

    (layer, cel, pixel_buffer)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use async_trait::async_trait;
    use tokio_util::sync::CancellationToken;

    use pixhaus_core::project::{FrameIndex, ProjectMetadata, SpriteId};

    use crate::backends::{
        BackendError, BackendRegistry, ImageEditRequest, ImageGenResponse, InferenceBackend,
        InferenceRequest, InferenceResponse,
    };
    use crate::plugin::context::VerbContext;
    use crate::plugin::descriptor::{BackendCapabilities, CostEstimate, VerbId};
    use crate::plugin::error::VerbError;
    use crate::plugin::inputs::VerbInputs;
    use crate::plugin::output::VerbEffect;
    use crate::plugin::progress::VerbProgress;
    use crate::plugin::runtime::VerbRuntime;

    use super::*;

    fn meta() -> ProjectMetadata {
        ProjectMetadata {
            name: "variant-test".into(),
            description: None,
            author: None,
            created_at: 0,
            updated_at: 0,
            editor_version: "0".into(),
        }
    }

    fn ctx_with_sprite() -> VerbContext {
        let mut ctx = VerbContext::empty(meta());
        ctx.active_sprite = Some(SpriteId::new(1));
        ctx.active_frame = Some(FrameIndex::new(0));
        ctx
    }

    fn empty_registry() -> Arc<BackendRegistry> {
        Arc::new(BackendRegistry::new())
    }

    /// 2×2 all-red RGBA8 image.
    fn red2x2() -> PixelData {
        PixelData::rgba8(
            2,
            2,
            vec![
                255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255,
            ],
        )
    }

    // ── Descriptor ────────────────────────────────────────────────────────────

    #[test]
    fn descriptor_declares_no_required_capabilities() {
        let verb = VariantVerb::new(empty_registry());
        assert!(
            verb.descriptor().required_capabilities.is_empty(),
            "palette-swap path must work without a backend"
        );
        assert!(verb.descriptor().cancellable);
        assert!(verb.descriptor().streaming);
    }

    #[test]
    fn verb_id_matches_constant() {
        let verb = VariantVerb::new(empty_registry());
        assert_eq!(verb.descriptor().id, VerbId::new(VARIANT_VERB_ID));
    }

    // ── Validation ────────────────────────────────────────────────────────────

    #[test]
    fn validate_rejects_malformed_pixels() {
        let verb = VariantVerb::new(empty_registry());
        let inputs = VerbInputs::from_struct(&VariantInputs {
            pixels: PixelData {
                width: 4,
                height: 4,
                bytes_per_pixel: 4,
                stride: 16,
                bytes: vec![0; 8], // should be 64
            },
            mode: VariantMode::PaletteSwap {
                substitutions: vec![ColorSubstitution {
                    from: Rgba::new(255, 0, 0, 255),
                    to: Rgba::new(0, 0, 255, 255),
                }],
            },
            layer_name: None,
        })
        .unwrap();
        assert!(verb.validate(&inputs).is_err());
    }

    #[test]
    fn validate_rejects_non_rgba_pixels() {
        let verb = VariantVerb::new(empty_registry());
        let inputs = VerbInputs::from_struct(&VariantInputs {
            pixels: PixelData {
                width: 2,
                height: 2,
                bytes_per_pixel: 1, // indexed — not supported
                stride: 2,
                bytes: vec![0; 4],
            },
            mode: VariantMode::PaletteSwap {
                substitutions: vec![ColorSubstitution {
                    from: Rgba::new(0, 0, 0, 255),
                    to: Rgba::new(255, 0, 0, 255),
                }],
            },
            layer_name: None,
        })
        .unwrap();
        assert!(verb.validate(&inputs).is_err());
    }

    #[test]
    fn validate_rejects_empty_substitutions() {
        let verb = VariantVerb::new(empty_registry());
        let inputs = VerbInputs::from_struct(&VariantInputs {
            pixels: red2x2(),
            mode: VariantMode::PaletteSwap {
                substitutions: vec![],
            },
            layer_name: None,
        })
        .unwrap();
        assert!(verb.validate(&inputs).is_err());
    }

    #[test]
    fn validate_rejects_empty_description() {
        let verb = VariantVerb::new(empty_registry());
        let inputs = VerbInputs::from_struct(&VariantInputs {
            pixels: red2x2(),
            mode: VariantMode::TextEdit {
                description: "   ".into(),
                count: 1,
            },
            layer_name: None,
        })
        .unwrap();
        assert!(verb.validate(&inputs).is_err());
    }

    #[test]
    fn validate_accepts_well_formed_palette_swap() {
        let verb = VariantVerb::new(empty_registry());
        let inputs = VerbInputs::from_struct(&VariantInputs {
            pixels: red2x2(),
            mode: VariantMode::PaletteSwap {
                substitutions: vec![ColorSubstitution {
                    from: Rgba::new(255, 0, 0, 255),
                    to: Rgba::new(0, 0, 255, 255),
                }],
            },
            layer_name: None,
        })
        .unwrap();
        assert!(verb.validate(&inputs).is_ok());
    }

    // ── Palette swap logic ────────────────────────────────────────────────────

    #[test]
    fn palette_swap_replaces_exact_matches() {
        let pixels = red2x2();
        let subs = vec![ColorSubstitution {
            from: Rgba::new(255, 0, 0, 255),
            to: Rgba::new(0, 0, 255, 255),
        }];
        let out = apply_palette_swap(&pixels, &subs);
        for chunk in out.bytes.chunks(4) {
            assert_eq!(chunk, [0, 0, 255, 255]);
        }
    }

    #[test]
    fn palette_swap_preserves_unmatched_pixels() {
        let pixels = red2x2();
        let subs = vec![ColorSubstitution {
            from: Rgba::new(0, 255, 0, 255), // green — not in source
            to: Rgba::new(0, 0, 255, 255),
        }];
        let out = apply_palette_swap(&pixels, &subs);
        for chunk in out.bytes.chunks(4) {
            assert_eq!(chunk, [255, 0, 0, 255]);
        }
    }

    #[test]
    fn palette_swap_first_match_wins() {
        let pixels = red2x2();
        let subs = vec![
            ColorSubstitution {
                from: Rgba::new(255, 0, 0, 255),
                to: Rgba::new(0, 255, 0, 255), // first match: green
            },
            ColorSubstitution {
                from: Rgba::new(255, 0, 0, 255),
                to: Rgba::new(0, 0, 255, 255), // second match: blue (should not apply)
            },
        ];
        let out = apply_palette_swap(&pixels, &subs);
        for chunk in out.bytes.chunks(4) {
            assert_eq!(chunk, [0, 255, 0, 255]);
        }
    }

    #[test]
    fn palette_swap_respects_stride() {
        // 2×1 image with 16-byte stride (8 bytes content + 8 bytes padding).
        let pixels = PixelData {
            width: 2,
            height: 1,
            bytes_per_pixel: 4,
            stride: 16,
            bytes: vec![
                255, 0, 0, 255, // pixel 0 — red
                0, 255, 0, 255, // pixel 1 — green
                0, 0, 0, 0, // padding
                0, 0, 0, 0, // padding
            ],
        };
        let subs = vec![ColorSubstitution {
            from: Rgba::new(255, 0, 0, 255),
            to: Rgba::new(0, 0, 255, 255),
        }];
        let out = apply_palette_swap(&pixels, &subs);
        assert_eq!(&out.bytes[0..4], [0, 0, 255, 255]); // red → blue
        assert_eq!(&out.bytes[4..8], [0, 255, 0, 255]); // green unchanged
    }

    // ── PNG round-trip ────────────────────────────────────────────────────────

    #[test]
    fn png_encode_decode_round_trips() {
        let pixels = red2x2();
        let png = pixel_data_to_png(&pixels).unwrap();
        let back = png_to_pixel_data(&png).unwrap();
        assert_eq!(back.width, pixels.width);
        assert_eq!(back.height, pixels.height);
        assert_eq!(back.bytes_per_pixel, 4);
        assert_eq!(back.stride, pixels.width * 4);
        assert_eq!(back.bytes, pixels.bytes);
    }

    // ── Full verb invocation ──────────────────────────────────────────────────

    #[tokio::test]
    async fn palette_swap_round_trips_through_runtime() {
        let runtime = VerbRuntime::new();
        runtime
            .register(VariantVerb::new(empty_registry()))
            .unwrap();

        let inputs = VerbInputs::from_struct(&VariantInputs {
            pixels: red2x2(),
            mode: VariantMode::PaletteSwap {
                substitutions: vec![ColorSubstitution {
                    from: Rgba::new(255, 0, 0, 255),
                    to: Rgba::new(0, 0, 255, 255),
                }],
            },
            layer_name: Some("Blue Variant".into()),
        })
        .unwrap();

        let inv = runtime
            .invoke(&VerbId::new(VARIANT_VERB_ID), ctx_with_sprite(), inputs)
            .unwrap();
        let preview = inv.finish().await.unwrap();

        assert_eq!(preview.verb.as_str(), VARIANT_VERB_ID);
        assert_eq!(preview.output.effects.len(), 1);
        match &preview.output.effects[0] {
            VerbEffect::AddLayer {
                layer,
                cels,
                pixel_buffers,
                ..
            } => {
                assert_eq!(layer.name, "Blue Variant");
                assert_eq!(cels.len(), 1);
                assert_eq!(pixel_buffers.len(), 1);
                for chunk in pixel_buffers[0].pixels.bytes.chunks(4) {
                    assert_eq!(chunk, [0, 0, 255, 255]);
                }
            }
            other => panic!("unexpected effect: {other:?}"),
        }
    }

    #[tokio::test]
    async fn text_edit_fails_without_backend() {
        let runtime = VerbRuntime::new();
        runtime
            .register(VariantVerb::new(empty_registry()))
            .unwrap();

        let inputs = VerbInputs::from_struct(&VariantInputs {
            pixels: red2x2(),
            mode: VariantMode::TextEdit {
                description: "make it blue".into(),
                count: 1,
            },
            layer_name: None,
        })
        .unwrap();

        let inv = runtime
            .invoke(&VerbId::new(VARIANT_VERB_ID), ctx_with_sprite(), inputs)
            .unwrap();
        let result = inv.finish().await;
        assert!(
            matches!(result, Err(VerbError::Backend(_))),
            "expected Backend error when no IMAGE_EDIT backend is registered"
        );
    }

    // ── AI path with a stub backend ───────────────────────────────────────────

    /// Stub backend: returns a fixed 2×2 green image for any `ImageEdit` request.
    #[derive(Debug)]
    struct GreenStub;

    #[async_trait]
    impl InferenceBackend for GreenStub {
        fn backend_id(&self) -> &'static str {
            "stub.green"
        }

        fn capabilities(&self) -> BackendCapabilities {
            BackendCapabilities::IMAGE_EDIT
        }

        fn supports_streaming(&self) -> bool {
            false
        }

        fn estimate_cost(&self, _request: &InferenceRequest) -> CostEstimate {
            CostEstimate {
                typical_latency: Duration::ZERO,
                max_latency: Duration::ZERO,
                typical_usd_cents: 0.0,
                max_usd_cents: 0.0,
            }
        }

        async fn invoke(
            &self,
            request: InferenceRequest,
            _progress: VerbProgress,
            _cancel: CancellationToken,
        ) -> std::result::Result<InferenceResponse, BackendError> {
            match request {
                InferenceRequest::ImageEdit(ImageEditRequest { num_images, .. }) => {
                    let green = PixelData::rgba8(
                        2,
                        2,
                        vec![
                            0, 255, 0, 255, 0, 255, 0, 255, 0, 255, 0, 255, 0, 255, 0, 255,
                        ],
                    );
                    let png = pixel_data_to_png(&green).map_err(|e| BackendError::Http {
                        status: 500,
                        body: e.to_string(),
                    })?;
                    let images = vec![png; num_images.max(1) as usize];
                    Ok(InferenceResponse::Image(ImageGenResponse {
                        images,
                        model: "stub.green".into(),
                    }))
                }
                _ => Err(BackendError::UnsupportedCapability),
            }
        }
    }

    #[tokio::test]
    async fn text_edit_produces_layer_with_ai_backend() {
        let mut reg = BackendRegistry::new();
        reg.add(GreenStub);
        let reg = Arc::new(reg);

        let runtime = VerbRuntime::new();
        runtime.register(VariantVerb::new(reg)).unwrap();

        let inputs = VerbInputs::from_struct(&VariantInputs {
            pixels: red2x2(),
            mode: VariantMode::TextEdit {
                description: "make it green".into(),
                count: 1,
            },
            layer_name: Some("Green Variant".into()),
        })
        .unwrap();

        let inv = runtime
            .invoke(&VerbId::new(VARIANT_VERB_ID), ctx_with_sprite(), inputs)
            .unwrap();
        let preview = inv.finish().await.unwrap();

        assert_eq!(preview.output.effects.len(), 1);
        match &preview.output.effects[0] {
            VerbEffect::AddLayer {
                layer,
                cels,
                pixel_buffers,
                ..
            } => {
                assert_eq!(layer.name, "Green Variant");
                assert_eq!(cels.len(), 1);
                assert_eq!(pixel_buffers.len(), 1);
                for chunk in pixel_buffers[0].pixels.bytes.chunks(4) {
                    assert_eq!(chunk, [0, 255, 0, 255]);
                }
            }
            other => panic!("unexpected effect: {other:?}"),
        }
    }

    #[tokio::test]
    async fn text_edit_multi_count_produces_multiple_layers() {
        let mut reg = BackendRegistry::new();
        reg.add(GreenStub);
        let reg = Arc::new(reg);

        let runtime = VerbRuntime::new();
        runtime.register(VariantVerb::new(reg)).unwrap();

        let inputs = VerbInputs::from_struct(&VariantInputs {
            pixels: red2x2(),
            mode: VariantMode::TextEdit {
                description: "generate three variants".into(),
                count: 3,
            },
            layer_name: Some("Alt".into()),
        })
        .unwrap();

        let inv = runtime
            .invoke(&VerbId::new(VARIANT_VERB_ID), ctx_with_sprite(), inputs)
            .unwrap();
        let preview = inv.finish().await.unwrap();

        assert_eq!(preview.output.effects.len(), 3);
        for (i, effect) in preview.output.effects.iter().enumerate() {
            if let VerbEffect::AddLayer { layer, .. } = effect {
                assert_eq!(layer.name, format!("Alt {}", i + 1));
            }
        }
    }

    #[tokio::test]
    async fn cancellation_stops_palette_swap() {
        let verb = VariantVerb::new(empty_registry());
        let cancel = CancellationToken::new();
        cancel.cancel();

        let ctx = ctx_with_sprite();
        let inputs = VerbInputs::from_struct(&VariantInputs {
            pixels: red2x2(),
            mode: VariantMode::PaletteSwap {
                substitutions: vec![ColorSubstitution {
                    from: Rgba::new(255, 0, 0, 255),
                    to: Rgba::new(0, 0, 255, 255),
                }],
            },
            layer_name: None,
        })
        .unwrap();

        let result = verb
            .invoke(ctx, inputs, VerbProgress::discard(), cancel)
            .await;
        assert!(matches!(result, Err(VerbError::Cancelled)));
    }

    #[tokio::test]
    async fn palette_swap_requires_active_sprite() {
        let runtime = VerbRuntime::new();
        runtime
            .register(VariantVerb::new(empty_registry()))
            .unwrap();

        let inputs = VerbInputs::from_struct(&VariantInputs {
            pixels: red2x2(),
            mode: VariantMode::PaletteSwap {
                substitutions: vec![ColorSubstitution {
                    from: Rgba::new(255, 0, 0, 255),
                    to: Rgba::new(0, 0, 255, 255),
                }],
            },
            layer_name: None,
        })
        .unwrap();

        let inv = runtime
            .invoke(
                &VerbId::new(VARIANT_VERB_ID),
                VerbContext::empty(meta()),
                inputs,
            )
            .unwrap();
        let result = inv.finish().await;
        assert!(matches!(
            result,
            Err(VerbError::MissingContext("active sprite"))
        ));
    }
}
