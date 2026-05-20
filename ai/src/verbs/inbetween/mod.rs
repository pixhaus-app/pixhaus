//! Verb: Inbetween — generate intermediate frames between two key frames.
//!
//! # What it does
//!
//! Takes two pixel-art frames (A and B) and generates N intermediate
//! frames using a frame-interpolation backend (RIFE-class or video
//! diffusion). If the project has an active palette, the output pixels are
//! snapped to the nearest palette color by squared Euclidean distance in
//! RGB space.
//!
//! # Output
//!
//! A [`VerbEffect::AddFrames`] that inserts N new frames immediately after
//! `frame_a_index` in the timeline. Each new frame gets one cel on the
//! active layer.
//!
//! # Backend
//!
//! Requires `FRAME_INTERPOLATION` capability. The runtime selects a
//! backend before calling `invoke`; the verb downcasts `ctx.backend` to
//! [`BackendProxy`] and calls the underlying
//! fat backend's `invoke(InferenceRequest::FrameInterpolation(...))`.

mod procedural;

use std::io::Cursor;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use image::ImageFormat;
use pixhaus_core::project::{
    Cel, Frame, FrameIndex, LayerId, Palette, PixelBufferId, Size, SpriteId,
};
use serde::{Deserialize, Serialize};
use tokio::select;
use tokio_util::sync::CancellationToken;
use tracing::{debug, instrument};

use crate::backends::{
    BackendProxy, FrameInterpolationRequest, InferenceRequest, InferenceResponse,
};
use crate::plugin::context::{PixelData, VerbContext};
use crate::plugin::descriptor::{
    BackendCapabilities, CostEstimate, EffectKind, VerbDescriptor, VerbId,
};
use crate::plugin::error::{Result, VerbError};
use crate::plugin::inputs::VerbInputs;
use crate::plugin::output::{ActualCost, NewPixelBuffer, VerbEffect, VerbOutput};
use crate::plugin::progress::{VerbProgress, VerbProgressEvent};
use crate::plugin::verb::Verb;

/// Stable identifier for the built-in Inbetween verb.
pub const INBETWEEN_VERB_ID: &str = "pixhaus.builtin.inbetween";

/// Maximum intermediate frames per call. Guards against accidental
/// runaway invocations that would exhaust backend quota.
const MAX_OUTPUTS: u32 = 16;

/// Placeholder IDs for new layers/buffers within the effect.
/// The host rewrites them to real IDs at commit time.
const PLACEHOLDER_LAYER: u32 = 0;

fn default_num_outputs() -> u32 {
    1
}

/// Inbetween generation strategy.
///
/// `Ai` is the default and dispatches to the configured
/// frame-interpolation backend. `Procedural` runs entirely locally
/// using the variance-rejected weighted averaging in the private
/// `procedural` submodule; no backend is required.
/// `AiWithProceduralPreview` emits the procedural midpoint as a
/// `PartialPixels` progress event before invoking the backend, so
/// the host can render a fast preview while the AI call runs.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InbetweenMode {
    /// Procedural variance-rejected weighted averaging. No backend
    /// dispatch — succeeds with `ctx.backend` unset.
    Procedural {
        /// Outlier rejection multiplier. Samples whose squared error
        /// from the local mean exceed `variance_range * variance` are
        /// rejected. `OpenToonz`'s canonical value is `2.5`.
        variance_range: f32,
    },
    /// Backend-driven frame interpolation (current behaviour).
    #[default]
    Ai,
    /// Run the procedural midpoint first, surface it as a preview,
    /// then invoke the AI backend.
    AiWithProceduralPreview {
        /// Outlier rejection multiplier for the procedural preview.
        variance_range: f32,
    },
}

/// Inputs for [`InbetweenVerb`].
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InbetweenInputs {
    /// Pixel data of the first key frame.
    pub frame_a: PixelData,
    /// Pixel data of the second key frame.
    pub frame_b: PixelData,
    /// Timeline index of frame A. The generated cels are inserted
    /// immediately after this position.
    pub frame_a_index: u32,
    /// Timeline index of frame B. Used for validation only (must be
    /// strictly greater than `frame_a_index`).
    pub frame_b_index: u32,
    /// Number of intermediate frames to generate. Clamped to
    /// `[1, MAX_OUTPUTS]` in `validate`.
    #[serde(default = "default_num_outputs")]
    pub num_outputs: u32,
    /// Generation strategy. Defaults to [`InbetweenMode::Ai`] so
    /// pre-S58 callers keep their behaviour.
    #[serde(default)]
    pub mode: InbetweenMode,
}

/// Generates intermediate frames between two key frames.
///
/// Dispatched to a `FRAME_INTERPOLATION`-capable backend via
/// [`BackendProxy`]. Output cels are placed on the active layer at new
/// timeline positions inserted after `frame_a_index`.
#[derive(Debug)]
pub struct InbetweenVerb {
    descriptor: VerbDescriptor,
}

impl InbetweenVerb {
    /// Constructs the verb.
    #[must_use]
    #[allow(clippy::disallowed_methods)]
    pub fn new() -> Self {
        let input_schema = serde_json::json!({
            "type": "object",
            "properties": {
                "frame_a": {
                    "type": "object",
                    "description": "Pixel data of the first key frame (RGBA8)"
                },
                "frame_b": {
                    "type": "object",
                    "description": "Pixel data of the second key frame (RGBA8)"
                },
                "frame_a_index": {
                    "type": "integer",
                    "minimum": 0,
                    "description": "Timeline index of frame A"
                },
                "frame_b_index": {
                    "type": "integer",
                    "minimum": 1,
                    "description": "Timeline index of frame B (must be > frame_a_index)"
                },
                "num_outputs": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 16,
                    "default": 1,
                    "description": "Number of intermediate frames to generate"
                },
                "mode": {
                    "description": "Generation strategy. Default is Ai. Use Procedural for backend-less local interpolation.",
                    "oneOf": [
                        {
                            "type": "object",
                            "properties": {
                                "kind": {"const": "procedural"},
                                "variance_range": {"type": "number", "minimum": 0.0, "default": 2.5}
                            },
                            "required": ["kind", "variance_range"]
                        },
                        {
                            "type": "object",
                            "properties": {"kind": {"const": "ai"}},
                            "required": ["kind"]
                        },
                        {
                            "type": "object",
                            "properties": {
                                "kind": {"const": "ai_with_procedural_preview"},
                                "variance_range": {"type": "number", "minimum": 0.0, "default": 2.5}
                            },
                            "required": ["kind", "variance_range"]
                        }
                    ],
                    "default": {"kind": "ai"}
                }
            },
            "required": ["frame_a", "frame_b", "frame_a_index", "frame_b_index"]
        });

        Self {
            descriptor: VerbDescriptor {
                id: VerbId::new(INBETWEEN_VERB_ID),
                display_name: "Inbetween".into(),
                description: "Generate intermediate frames between two key frames".into(),
                version: env!("CARGO_PKG_VERSION").into(),
                required_capabilities: BackendCapabilities::FRAME_INTERPOLATION,
                input_schema,
                output_schema: None,
                output_kinds: vec![EffectKind::AddFrames, EffectKind::AddCels],
                cost_estimate: CostEstimate {
                    typical_latency: Duration::from_secs(30),
                    max_latency: Duration::from_secs(300),
                    typical_usd_cents: 5.0,
                    max_usd_cents: 20.0,
                },
                streaming: true,
                cancellable: true,
                documentation_url: Some("docs/verbs/inbetween.md".into()),
            },
        }
    }
}

impl InbetweenVerb {
    /// Runs the procedural fallback. Generates `num_outputs` frames
    /// locally and packages them into the same `VerbEffect::AddFrames`
    /// shape the AI path produces. No backend dispatch.
    #[allow(clippy::too_many_arguments)]
    async fn invoke_procedural(
        &self,
        inputs: &InbetweenInputs,
        sprite_id: SpriteId,
        active_layer: LayerId,
        palette: Option<&Palette>,
        variance_range: f32,
        progress: &VerbProgress,
        cancel: &CancellationToken,
        started: Instant,
    ) -> Result<VerbOutput> {
        progress
            .step(Some(0.1), "computing procedural inbetween frames")
            .await;

        let n = inputs.num_outputs;
        // `num_outputs` is bounded to `[1, MAX_OUTPUTS]` by validate
        // (MAX_OUTPUTS == 16), so the u16 conversion never saturates.
        let n_u16 = u16::try_from(n).unwrap_or(u16::MAX);
        let denom = f32::from(n_u16) + 1.0;
        let width = inputs.frame_a.width;
        let height = inputs.frame_a.height;
        let frame_size = Size::new(width, height);

        if cancel.is_cancelled() {
            return Err(VerbError::Cancelled);
        }

        // The interpolation and palette snapping are CPU-bound pixel
        // loops; run them off the async runtime per the verb-protocol
        // pattern. Owned copies move into the blocking closure.
        let start_bytes = inputs.frame_a.bytes.clone();
        let end_bytes = inputs.frame_b.bytes.clone();
        let palette_owned = palette.cloned();
        let pixel_datas = tokio::task::spawn_blocking(move || {
            (0..n)
                .map(|i| {
                    let i_u16 = u16::try_from(i).unwrap_or(u16::MAX);
                    let t = (f32::from(i_u16) + 1.0) / denom;
                    let bytes = procedural::interpolate_frames(
                        &start_bytes,
                        &end_bytes,
                        width,
                        height,
                        t,
                        variance_range,
                    );
                    let pixel_data = PixelData::rgba8(width, height, bytes);
                    match &palette_owned {
                        Some(pal) => snap_to_palette(pixel_data, pal),
                        None => pixel_data,
                    }
                })
                .collect::<Vec<_>>()
        })
        .await
        .map_err(|e| VerbError::Aborted(e.to_string()))?;

        if cancel.is_cancelled() {
            return Err(VerbError::Cancelled);
        }

        let mut frames = Vec::with_capacity(n as usize);
        let mut cels = Vec::with_capacity(n as usize);
        let mut pixel_buffers = Vec::with_capacity(n as usize);

        for (i, pixel_data) in (0..n).zip(pixel_datas) {
            let buf_id = PixelBufferId::new(i);
            let cel = Cel::raster(
                LayerId::new(PLACEHOLDER_LAYER),
                FrameIndex::new(i),
                buf_id,
                frame_size,
            );
            let cel = Cel {
                layer_id: active_layer,
                ..cel
            };

            frames.push(Frame::default());
            cels.push(cel);
            pixel_buffers.push(NewPixelBuffer {
                placeholder: buf_id,
                pixels: pixel_data,
            });
        }

        progress.step(Some(1.0), "done").await;

        let elapsed = started.elapsed();
        let summary = format!(
            "Add {} procedural inbetween frame{} after frame {} ({}x{})",
            n,
            if n == 1 { "" } else { "s" },
            inputs.frame_a_index,
            width,
            height,
        );

        let thumbnail = pixel_buffers.first().map(|b| b.pixels.clone());

        let mut notes = vec!["Procedural inbetween — no backend invoked.".to_string()];
        if palette.is_none() {
            notes.push("No active palette — output was not snapped to a palette.".into());
        }

        Ok(VerbOutput {
            summary,
            effects: vec![VerbEffect::AddFrames {
                sprite: sprite_id,
                after: Some(FrameIndex::new(inputs.frame_a_index)),
                frames,
                cels,
                pixel_buffers,
            }],
            thumbnail,
            actual_cost: ActualCost::free(elapsed.max(Duration::from_micros(1))),
            notes,
        })
    }
}

impl Default for InbetweenVerb {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Verb for InbetweenVerb {
    fn descriptor(&self) -> &VerbDescriptor {
        &self.descriptor
    }

    fn required_capabilities_for(&self, inputs: &VerbInputs) -> BackendCapabilities {
        // Procedural mode runs entirely locally — no backend needed, so
        // the runtime must not reject the invocation when none is
        // configured. Any parse failure falls back to the descriptor's
        // requirement; `validate` rejects malformed inputs separately.
        match inputs.deserialize::<InbetweenInputs>() {
            Ok(parsed) if matches!(parsed.mode, InbetweenMode::Procedural { .. }) => {
                BackendCapabilities::empty()
            }
            _ => self.descriptor.required_capabilities,
        }
    }

    fn validate(&self, inputs: &VerbInputs) -> Result<()> {
        let parsed: InbetweenInputs = inputs.deserialize()?;

        if !parsed.frame_a.is_well_formed() {
            return Err(VerbError::Schema(
                "inbetween: frame_a pixel data is malformed".into(),
            ));
        }
        if !parsed.frame_b.is_well_formed() {
            return Err(VerbError::Schema(
                "inbetween: frame_b pixel data is malformed".into(),
            ));
        }
        if parsed.frame_a.bytes_per_pixel != 4 || parsed.frame_b.bytes_per_pixel != 4 {
            return Err(VerbError::Schema(
                "inbetween: both frames must be RGBA8 (bytes_per_pixel == 4)".into(),
            ));
        }
        if parsed.frame_a.width != parsed.frame_b.width
            || parsed.frame_a.height != parsed.frame_b.height
        {
            return Err(VerbError::Schema(
                "inbetween: frame_a and frame_b must have identical dimensions".into(),
            ));
        }
        if parsed.frame_b_index <= parsed.frame_a_index {
            return Err(VerbError::Schema(
                "inbetween: frame_b_index must be strictly greater than frame_a_index".into(),
            ));
        }
        if parsed.num_outputs == 0 || parsed.num_outputs > MAX_OUTPUTS {
            return Err(VerbError::Schema(format!(
                "inbetween: num_outputs must be in [1, {MAX_OUTPUTS}], got {}",
                parsed.num_outputs
            )));
        }
        Ok(())
    }

    #[instrument(skip(self, ctx, inputs, progress, cancel), fields(verb = INBETWEEN_VERB_ID))]
    async fn invoke(
        &self,
        ctx: VerbContext,
        inputs: VerbInputs,
        progress: VerbProgress,
        cancel: CancellationToken,
    ) -> Result<VerbOutput> {
        // B10.3 anchor: this verb intentionally ignores `ctx.anchor`.
        // `FrameInterpolationRequest` has no style slot — the
        // interpolation backend uses the two keyframes as both content
        // and style sources; an external style condition would conflict
        // with their per-pixel constraint.
        let started = Instant::now();
        let inputs: InbetweenInputs = inputs.deserialize_owned()?;

        let sprite_id = ctx.require_sprite_id()?;
        let active_layer = ctx.require_active_layer()?;

        progress
            .send(VerbProgressEvent::Started {
                backend: ctx.backend.as_ref().map(|b| b.id().to_owned()),
            })
            .await;

        if cancel.is_cancelled() {
            return Err(VerbError::Cancelled);
        }

        // Procedural fast path: variance-rejected weighted averaging,
        // no backend dispatch. AiWithProceduralPreview emits the
        // midpoint as a PartialPixels progress event and falls through
        // to the AI path.
        match inputs.mode {
            InbetweenMode::Procedural { variance_range } => {
                return self
                    .invoke_procedural(
                        &inputs,
                        sprite_id,
                        active_layer,
                        ctx.active_palette.as_ref(),
                        variance_range,
                        &progress,
                        &cancel,
                        started,
                    )
                    .await;
            }
            InbetweenMode::AiWithProceduralPreview { variance_range } => {
                let preview_bytes = procedural::interpolate_frames(
                    &inputs.frame_a.bytes,
                    &inputs.frame_b.bytes,
                    inputs.frame_a.width,
                    inputs.frame_a.height,
                    0.5,
                    variance_range,
                );
                let preview =
                    PixelData::rgba8(inputs.frame_a.width, inputs.frame_a.height, preview_bytes);
                progress
                    .send(VerbProgressEvent::PartialPixels {
                        effect_index: 0,
                        pixels: preview,
                    })
                    .await;
            }
            InbetweenMode::Ai => {}
        }

        // Encode both frames as PNG for the backend.
        progress.step(Some(0.05), "encoding frames").await;
        let frame_a_png = pixel_data_to_png(&inputs.frame_a)
            .map_err(|e| VerbError::Backend(format!("encode frame_a: {e}")))?;
        let frame_b_png = pixel_data_to_png(&inputs.frame_b)
            .map_err(|e| VerbError::Backend(format!("encode frame_b: {e}")))?;

        if cancel.is_cancelled() {
            return Err(VerbError::Cancelled);
        }

        // Resolve the backend.
        let proxy = ctx
            .backend
            .as_ref()
            .and_then(|b| b.as_any().downcast_ref::<BackendProxy>())
            .ok_or_else(|| {
                VerbError::Backend(
                    "inbetween: no BackendProxy attached; register a backend with \
                     FRAME_INTERPOLATION capability via BackendProxy::new()"
                        .into(),
                )
            })?;

        debug!(
            num_outputs = inputs.num_outputs,
            "invoking frame interpolation backend"
        );

        let interp_request = FrameInterpolationRequest {
            model: None,
            frames: vec![frame_a_png, frame_b_png],
            num_outputs: inputs.num_outputs,
        };

        progress
            .step(Some(0.1), "calling interpolation backend")
            .await;

        let backend_result = select! {
            biased;
            () = cancel.cancelled() => return Err(VerbError::Cancelled),
            result = proxy.fat().invoke(
                InferenceRequest::FrameInterpolation(interp_request),
                progress.clone(),
                cancel.child_token(),
            ) => result,
        };

        let response = backend_result.map_err(|e| VerbError::Backend(e.to_string()))?;

        if cancel.is_cancelled() {
            return Err(VerbError::Cancelled);
        }

        let frame_pngs = match response {
            InferenceResponse::Frames(r) => r.frames,
            other => {
                return Err(VerbError::Backend(format!(
                    "inbetween: expected Frames response, got {:?}",
                    std::mem::discriminant(&other)
                )));
            }
        };

        if frame_pngs.len() < inputs.num_outputs as usize {
            return Err(VerbError::Backend(format!(
                "inbetween: backend returned {} frames, expected {}",
                frame_pngs.len(),
                inputs.num_outputs
            )));
        }

        progress
            .step(Some(0.8), "decoding and palette-snapping output frames")
            .await;

        // Decode each output PNG, snap to palette if available, pack
        // into pixel buffers.
        let palette = ctx.active_palette.as_ref();
        let frame_size = Size::new(inputs.frame_a.width, inputs.frame_a.height);

        let mut frames = Vec::with_capacity(inputs.num_outputs as usize);
        let mut cels = Vec::with_capacity(inputs.num_outputs as usize);
        let mut pixel_buffers = Vec::with_capacity(inputs.num_outputs as usize);

        for (i, png_bytes) in (0u32..).zip(frame_pngs.iter().take(inputs.num_outputs as usize)) {
            let mut pixel_data = png_to_pixel_data(png_bytes)
                .map_err(|e| VerbError::Backend(format!("decode output frame {i}: {e}")))?;

            if let Some(pal) = palette {
                pixel_data = snap_to_palette(pixel_data, pal);
            }

            let buf_id = PixelBufferId::new(i);
            let cel = Cel::raster(
                LayerId::new(PLACEHOLDER_LAYER), // overridden below
                FrameIndex::new(i),
                buf_id,
                frame_size,
            );

            // Replace the placeholder layer ID with the real active layer.
            let cel = Cel {
                layer_id: active_layer,
                ..cel
            };

            frames.push(Frame::default());
            cels.push(cel);
            pixel_buffers.push(NewPixelBuffer {
                placeholder: buf_id,
                pixels: pixel_data,
            });
        }

        progress.step(Some(1.0), "done").await;

        let elapsed = started.elapsed();
        let summary = format!(
            "Add {} inbetween frame{} after frame {} ({}x{})",
            inputs.num_outputs,
            if inputs.num_outputs == 1 { "" } else { "s" },
            inputs.frame_a_index,
            inputs.frame_a.width,
            inputs.frame_a.height,
        );

        // Use the first generated frame as a preview thumbnail.
        let thumbnail = pixel_buffers.first().map(|b| b.pixels.clone());

        Ok(VerbOutput {
            summary,
            effects: vec![VerbEffect::AddFrames {
                sprite: sprite_id,
                after: Some(FrameIndex::new(inputs.frame_a_index)),
                frames,
                cels,
                pixel_buffers,
            }],
            thumbnail,
            actual_cost: ActualCost::free(elapsed.max(Duration::from_micros(1))),
            notes: if palette.is_none() {
                vec!["No active palette — output was not snapped to a palette.".into()]
            } else {
                vec![]
            },
        })
    }
}

// ── PNG encode / decode helpers ────────────────────────────────────────────

/// Encodes a [`PixelData`] buffer (must be RGBA8) as PNG bytes.
fn pixel_data_to_png(data: &PixelData) -> std::result::Result<Vec<u8>, String> {
    let bpp = data.bytes_per_pixel as usize;
    let stride = data.stride as usize;
    let row_bytes = data.width as usize * bpp;

    // De-stride into a tightly-packed buffer.
    let mut packed = Vec::with_capacity(data.width as usize * data.height as usize * bpp);
    for row in 0..data.height as usize {
        let start = row * stride;
        packed.extend_from_slice(
            data.bytes
                .get(start..start + row_bytes)
                .ok_or("pixel buffer shorter than declared dimensions")?,
        );
    }

    let img = image::RgbaImage::from_raw(data.width, data.height, packed)
        .ok_or("failed to construct RgbaImage from pixel data")?;

    let mut buf = Vec::new();
    img.write_to(&mut Cursor::new(&mut buf), ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    Ok(buf)
}

/// Decodes a PNG byte slice into an RGBA8 [`PixelData`].
fn png_to_pixel_data(bytes: &[u8]) -> std::result::Result<PixelData, String> {
    let img = image::load_from_memory_with_format(bytes, ImageFormat::Png)
        .map_err(|e| e.to_string())?
        .into_rgba8();
    let (width, height) = img.dimensions();
    Ok(PixelData::rgba8(width, height, img.into_raw()))
}

// ── Palette snapping ───────────────────────────────────────────────────────

/// Snaps every non-transparent pixel in `data` to the nearest color in
/// `palette` by squared Euclidean distance in RGB space. Transparent
/// pixels (alpha == 0) are left unchanged.
///
/// If the palette is empty the original buffer is returned as-is.
fn snap_to_palette(mut data: PixelData, palette: &Palette) -> PixelData {
    if palette.colors.is_empty() || data.bytes_per_pixel != 4 {
        return data;
    }

    // Pre-extract palette colors as [r, g, b] to avoid repeated field
    // access inside the hot loop.
    let pal_rgb: Vec<[u8; 3]> = palette
        .colors
        .iter()
        .map(|e| [e.color.r, e.color.g, e.color.b])
        .collect();

    let bpp = data.bytes_per_pixel as usize;
    let stride = data.stride as usize;
    let width = data.width as usize;
    let height = data.height as usize;

    for y in 0..height {
        for x in 0..width {
            let offset = y * stride + x * bpp;
            let a = data.bytes[offset + 3];
            if a == 0 {
                continue;
            }

            let r = i32::from(data.bytes[offset]);
            let g = i32::from(data.bytes[offset + 1]);
            let b = i32::from(data.bytes[offset + 2]);

            let nearest = pal_rgb
                .iter()
                .min_by_key(|&&[pr, pg, pb]| {
                    let dr = r - i32::from(pr);
                    let dg = g - i32::from(pg);
                    let db = b - i32::from(pb);
                    dr * dr + dg * dg + db * db
                })
                .copied();

            if let Some([nr, ng, nb]) = nearest {
                data.bytes[offset] = nr;
                data.bytes[offset + 1] = ng;
                data.bytes[offset + 2] = nb;
                // Alpha unchanged.
            }
        }
    }
    data
}

// ── Unit tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::inputs::VerbInputs;
    use pixhaus_core::project::{PaletteEntry, PaletteId, Rgba, UserData};

    fn two_pixel_frame(r: u8, g: u8, b: u8) -> PixelData {
        PixelData::rgba8(
            2,
            2,
            vec![r, g, b, 255, r, g, b, 255, r, g, b, 255, r, g, b, 255],
        )
    }

    fn make_inputs(num_outputs: u32) -> VerbInputs {
        VerbInputs::from_struct(&InbetweenInputs {
            frame_a: two_pixel_frame(0, 0, 0),
            frame_b: two_pixel_frame(255, 255, 255),
            frame_a_index: 0,
            frame_b_index: 5,
            num_outputs,
            mode: InbetweenMode::Ai,
        })
        .unwrap()
    }

    #[test]
    fn descriptor_requires_frame_interpolation() {
        let verb = InbetweenVerb::new();
        assert!(
            verb.descriptor()
                .required_capabilities
                .contains(BackendCapabilities::FRAME_INTERPOLATION)
        );
        assert!(verb.descriptor().cancellable);
        assert!(verb.descriptor().streaming);
    }

    #[test]
    fn procedural_mode_requires_no_backend_capability() {
        let verb = InbetweenVerb::new();
        let inputs = VerbInputs::from_struct(&InbetweenInputs {
            frame_a: two_pixel_frame(0, 0, 0),
            frame_b: two_pixel_frame(255, 255, 255),
            frame_a_index: 0,
            frame_b_index: 5,
            num_outputs: 1,
            mode: InbetweenMode::Procedural {
                variance_range: 2.5,
            },
        })
        .unwrap();
        assert!(verb.required_capabilities_for(&inputs).is_empty());
    }

    #[test]
    fn ai_mode_still_requires_frame_interpolation() {
        let verb = InbetweenVerb::new();
        let inputs = make_inputs(1);
        assert!(
            verb.required_capabilities_for(&inputs)
                .contains(BackendCapabilities::FRAME_INTERPOLATION)
        );
    }

    #[test]
    fn validate_rejects_mismatched_dimensions() {
        let verb = InbetweenVerb::new();
        let inputs = VerbInputs::from_struct(&InbetweenInputs {
            frame_a: PixelData::rgba8(2, 2, vec![0; 16]),
            frame_b: PixelData::rgba8(4, 4, vec![0; 64]),
            frame_a_index: 0,
            frame_b_index: 1,
            num_outputs: 1,
            mode: InbetweenMode::Ai,
        })
        .unwrap();
        assert!(verb.validate(&inputs).is_err());
    }

    #[test]
    fn validate_rejects_b_not_after_a() {
        let verb = InbetweenVerb::new();
        let inputs = VerbInputs::from_struct(&InbetweenInputs {
            frame_a: PixelData::rgba8(2, 2, vec![0; 16]),
            frame_b: PixelData::rgba8(2, 2, vec![0; 16]),
            frame_a_index: 5,
            frame_b_index: 3,
            num_outputs: 1,
            mode: InbetweenMode::Ai,
        })
        .unwrap();
        assert!(verb.validate(&inputs).is_err());
    }

    #[test]
    fn validate_rejects_zero_outputs() {
        let verb = InbetweenVerb::new();
        let inputs = VerbInputs::from_struct(&InbetweenInputs {
            frame_a: PixelData::rgba8(2, 2, vec![0; 16]),
            frame_b: PixelData::rgba8(2, 2, vec![0; 16]),
            frame_a_index: 0,
            frame_b_index: 2,
            num_outputs: 0,
            mode: InbetweenMode::Ai,
        })
        .unwrap();
        assert!(verb.validate(&inputs).is_err());
    }

    #[test]
    fn validate_rejects_too_many_outputs() {
        let verb = InbetweenVerb::new();
        let inputs = VerbInputs::from_struct(&InbetweenInputs {
            frame_a: PixelData::rgba8(2, 2, vec![0; 16]),
            frame_b: PixelData::rgba8(2, 2, vec![0; 16]),
            frame_a_index: 0,
            frame_b_index: 20,
            num_outputs: MAX_OUTPUTS + 1,
            mode: InbetweenMode::Ai,
        })
        .unwrap();
        assert!(verb.validate(&inputs).is_err());
    }

    #[test]
    fn validate_accepts_valid_inputs() {
        let verb = InbetweenVerb::new();
        assert!(verb.validate(&make_inputs(1)).is_ok());
        assert!(verb.validate(&make_inputs(MAX_OUTPUTS)).is_ok());
    }

    #[test]
    fn png_round_trip() {
        let original = PixelData::rgba8(
            2,
            2,
            vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
        );
        let png = pixel_data_to_png(&original).unwrap();
        let decoded = png_to_pixel_data(&png).unwrap();
        assert_eq!(decoded.width, 2);
        assert_eq!(decoded.height, 2);
        assert_eq!(decoded.bytes, original.bytes);
    }

    #[test]
    fn snap_to_palette_replaces_nearest_color() {
        let palette = Palette {
            id: PaletteId::new(1),
            name: "test".into(),
            colors: vec![
                PaletteEntry::new(Rgba {
                    r: 0,
                    g: 0,
                    b: 0,
                    a: 255,
                }), // black
                PaletteEntry::new(Rgba {
                    r: 255,
                    g: 0,
                    b: 0,
                    a: 255,
                }), // red
            ],
            user_data: UserData::default(),
            pages: Vec::new(),
            animation: None,
        };

        // A pixel that is almost-red should snap to red (255, 0, 0).
        let data = PixelData::rgba8(1, 1, vec![200, 10, 10, 255]);
        let snapped = snap_to_palette(data, &palette);
        assert_eq!(&snapped.bytes[0..4], &[255, 0, 0, 255]);
    }

    #[test]
    fn snap_to_palette_leaves_transparent_pixels() {
        let palette = Palette {
            id: PaletteId::new(1),
            name: "test".into(),
            colors: vec![PaletteEntry::new(Rgba {
                r: 255,
                g: 0,
                b: 0,
                a: 255,
            })],
            user_data: UserData::default(),
            pages: Vec::new(),
            animation: None,
        };

        // Alpha == 0 means fully transparent — should not be changed.
        let data = PixelData::rgba8(1, 1, vec![100, 100, 100, 0]);
        let snapped = snap_to_palette(data.clone(), &palette);
        assert_eq!(snapped.bytes, data.bytes);
    }

    // ── InbetweenMode serde ────────────────────────────────────────────────

    #[test]
    fn inbetween_mode_defaults_to_ai() {
        let m = InbetweenMode::default();
        assert!(matches!(m, InbetweenMode::Ai));
    }

    #[test]
    fn inbetween_mode_round_trips_json_for_all_variants() {
        let modes = [
            InbetweenMode::Procedural {
                variance_range: 2.5,
            },
            InbetweenMode::Ai,
            InbetweenMode::AiWithProceduralPreview {
                variance_range: 2.5,
            },
        ];
        for m in modes {
            let j = serde_json::to_string(&m).unwrap();
            let back: InbetweenMode = serde_json::from_str(&j).unwrap();
            assert_eq!(m, back);
        }
    }

    #[test]
    fn inbetween_mode_serializes_with_kind_tag() {
        let j = serde_json::to_string(&InbetweenMode::Ai).unwrap();
        assert!(j.contains("\"kind\""), "missing kind tag in {j}");
        assert!(j.contains("\"ai\""), "expected snake_case ai in {j}");
    }

    #[test]
    fn input_schema_exposes_mode_with_three_variants() {
        let verb = InbetweenVerb::new();
        let schema = &verb.descriptor().input_schema;
        let mode = schema
            .get("properties")
            .and_then(|p| p.get("mode"))
            .expect("mode property missing from input_schema");
        let variants = mode
            .get("oneOf")
            .and_then(|o| o.as_array())
            .expect("mode must be a oneOf union");
        assert_eq!(
            variants.len(),
            3,
            "expected 3 mode variants, got {variants:?}"
        );

        let mut kinds: Vec<&str> = variants
            .iter()
            .filter_map(|v| {
                v.get("properties")
                    .and_then(|p| p.get("kind"))
                    .and_then(|k| k.get("const"))
                    .and_then(|c| c.as_str())
            })
            .collect();
        kinds.sort_unstable();
        assert_eq!(
            kinds,
            vec!["ai", "ai_with_procedural_preview", "procedural"]
        );

        let default = mode.get("default").expect("mode default missing");
        assert_eq!(default.get("kind").and_then(|k| k.as_str()), Some("ai"));
    }

    #[test]
    fn inbetween_inputs_default_mode_is_ai() {
        // JSON missing the `mode` field must deserialize to AI.
        let json = serde_json::json!({
            "frame_a": {
                "width": 1,
                "height": 1,
                "bytes_per_pixel": 4,
                "stride": 4,
                "bytes": [0, 0, 0, 255],
            },
            "frame_b": {
                "width": 1,
                "height": 1,
                "bytes_per_pixel": 4,
                "stride": 4,
                "bytes": [255, 255, 255, 255],
            },
            "frame_a_index": 0,
            "frame_b_index": 2,
        });
        let parsed: InbetweenInputs = serde_json::from_value(json).unwrap();
        assert!(matches!(parsed.mode, InbetweenMode::Ai));
    }
}
