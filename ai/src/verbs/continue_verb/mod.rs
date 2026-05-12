//! Verb: Continue — S24.
//!
//! Predicts the next 1–3 animation frames given the last 3–5 frames of
//! the active layer as context. The host packages the context frames as
//! [`crate::plugin::context::ReferenceImage`]s in
//! [`crate::plugin::context::VerbContext::references`] before invoking;
//! the verb encodes them to PNG, calls the backend, decodes the response,
//! optionally snaps pixels to the active palette, and returns
//! [`crate::plugin::output::VerbEffect::AddFrames`].
//!
//! # Context preparation
//!
//! Before invoking Continue, the host must add the last 3–5 frames of
//! the active layer as `ReferenceImage`s in timeline display order
//! (earliest index first). The verb treats `ctx.references` as an ordered
//! context window and uses up to `MAX_CONTEXT_WINDOW` entries.
//!
//! # Backend selection
//!
//! Continue declares `IMAGE_GENERATION | FRAME_INTERPOLATION` required
//! capabilities. The runtime selects a compatible backend before `invoke`
//! runs. The verb dispatches to the concrete backend through a known-type
//! downcast chain, forwarding a
//! [`crate::backends::FrameInterpolationRequest`] whose `frames` are the
//! context PNGs and `num_outputs` is the requested generation count.
//!
//! If the backend responds with an `Image` response instead of `Frames`
//! (e.g. a `ComfyUI` workflow that emits a sprite sheet), each image in the
//! response is treated as one output frame in order.

use std::io::Cursor;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use image::{ImageEncoder, ImageFormat};
use pixhaus_core::project::{Cel, Frame, FrameIndex, Palette, PixelBufferId, Size};
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;
use tracing::warn;

use crate::backends::{
    FrameInterpolationRequest, InferenceBackend as FullBackend, InferenceRequest, InferenceResponse,
};
use crate::plugin::{
    context::{PixelData, VerbContext},
    descriptor::{BackendCapabilities, CostEstimate, EffectKind, VerbDescriptor, VerbId},
    error::{Result, VerbError},
    inputs::VerbInputs,
    output::{ActualCost, NewPixelBuffer, VerbEffect, VerbOutput},
    progress::{VerbProgress, VerbProgressEvent},
    verb::Verb,
};

/// Stable identifier for the Continue verb.
pub const CONTINUE_VERB_ID: &str = "pixhaus.builtin.continue";

/// Maximum frames the verb will generate in a single invocation.
const MAX_FRAMES: u32 = 3;
/// Maximum context frames read from `ctx.references`.
const MAX_CONTEXT_WINDOW: usize = 5;

// ── Inputs ────────────────────────────────────────────────────────────────────

/// Inputs for [`ContinueVerb`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ContinueInputs {
    /// Number of frames to predict (1–3).
    #[serde(default = "default_num_frames")]
    pub num_frames: u32,
    /// Duration of each generated frame in milliseconds. `None` copies
    /// the display rate of the active frame (falls back to 100 ms when
    /// the sprite has no frames yet).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame_duration_ms: Option<u32>,
}

fn default_num_frames() -> u32 {
    1
}

impl Default for ContinueInputs {
    fn default() -> Self {
        Self {
            num_frames: 1,
            frame_duration_ms: None,
        }
    }
}

// ── Verb ──────────────────────────────────────────────────────────────────────

/// Predicts the next 1–3 animation frames from prior context frames.
///
/// See the module documentation for a full description of the expected
/// `VerbContext` layout and the backend dispatch contract.
#[derive(Debug)]
pub struct ContinueVerb {
    descriptor: VerbDescriptor,
}

impl ContinueVerb {
    /// Constructs a fresh Continue verb with its compiled-in descriptor.
    ///
    /// The `serde_json::json!` macro uses `unwrap` internally to build
    /// the JSON value; the workspace lint is suppressed here as it is in
    /// all other verb constructors that build inline schemas.
    #[must_use]
    #[allow(clippy::disallowed_methods)]
    pub fn new() -> Self {
        let input_schema = serde_json::json!({
            "type": "object",
            "properties": {
                "num_frames": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 3,
                    "default": 1,
                    "description": "Number of frames to predict."
                },
                "frame_duration_ms": {
                    "type": ["integer", "null"],
                    "minimum": 1,
                    "description":
                        "Duration of each generated frame in ms. \
                         Defaults to the active frame's duration (or 100 ms)."
                }
            }
        });

        Self {
            descriptor: VerbDescriptor {
                id: VerbId::new(CONTINUE_VERB_ID),
                display_name: "Continue".into(),
                description: "Predict the next 1–3 animation frames from the last 3–5 frames \
                     of the active layer."
                    .into(),
                version: env!("CARGO_PKG_VERSION").into(),
                required_capabilities: BackendCapabilities::IMAGE_GENERATION
                    .union(BackendCapabilities::FRAME_INTERPOLATION),
                input_schema,
                output_schema: None,
                output_kinds: vec![EffectKind::AddFrames],
                cost_estimate: CostEstimate {
                    typical_latency: Duration::from_secs(20),
                    max_latency: Duration::from_secs(120),
                    typical_usd_cents: 5.0,
                    max_usd_cents: 20.0,
                },
                streaming: true,
                cancellable: true,
                documentation_url: Some("docs/ai-verbs/continue.md".into()),
            },
        }
    }
}

impl Default for ContinueVerb {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Verb for ContinueVerb {
    fn descriptor(&self) -> &VerbDescriptor {
        &self.descriptor
    }

    fn validate(&self, inputs: &VerbInputs) -> Result<()> {
        let parsed: ContinueInputs = inputs.deserialize()?;
        if parsed.num_frames == 0 || parsed.num_frames > MAX_FRAMES {
            return Err(VerbError::Schema(format!(
                "continue: num_frames must be 1–{MAX_FRAMES}, got {}",
                parsed.num_frames
            )));
        }
        if let Some(ms) = parsed.frame_duration_ms {
            if ms == 0 {
                return Err(VerbError::Schema(
                    "continue: frame_duration_ms must be at least 1 ms".into(),
                ));
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
        // B10.3 anchor: this verb intentionally ignores `ctx.anchor`.
        // `FrameInterpolationRequest` has no style slot — the
        // interpolation backend uses the context frames as both content
        // and style sources; an external style condition would conflict
        // with their per-pixel constraint.
        let started = Instant::now();
        let inputs: ContinueInputs = inputs.deserialize_owned()?;

        // Require active sprite and layer; active_frame defaults to 0.
        let sprite_id = ctx.require_sprite_id()?;
        let layer_id = ctx.require_active_layer()?;
        let active_frame = ctx.active_frame.unwrap_or(FrameIndex::new(0));

        // The host must populate ctx.references with prior frames.
        if ctx.references.is_empty() {
            return Err(VerbError::MissingContext(
                "continue verb: ctx.references is empty; \
                 the host must add the last 3–5 frames of the active layer as references",
            ));
        }

        // Use up to MAX_CONTEXT_WINDOW references in display order.
        let context_refs: Vec<&PixelData> = ctx
            .references
            .iter()
            .take(MAX_CONTEXT_WINDOW)
            .map(|r| &r.pixels)
            .collect();

        let canvas_width = context_refs[0].width;
        let canvas_height = context_refs[0].height;
        // When the caller doesn't pin a duration, copy the active
        // frame's duration so generated frames hold for the same
        // beat as their context. Falls back to 100ms (10fps) only
        // when neither the input nor the active frame is available.
        let frame_duration_ms = inputs.frame_duration_ms.unwrap_or_else(|| {
            ctx.sprite
                .as_ref()
                .and_then(|s| s.frames.get(active_frame.get() as usize))
                .map_or(100, |f| f.duration_ms)
        });

        let backend_id = ctx.backend.as_ref().map(|b| b.id().to_owned());

        progress
            .send(VerbProgressEvent::Started {
                backend: backend_id.clone(),
            })
            .await;
        progress
            .step(Some(0.05), "encoding context frames as PNG")
            .await;

        if cancel.is_cancelled() {
            return Err(VerbError::Cancelled);
        }

        // Encode context frames to PNG.
        let mut context_pngs: Vec<Vec<u8>> = Vec::with_capacity(context_refs.len());
        for frame_data in &context_refs {
            context_pngs.push(encode_to_png(frame_data)?);
        }

        progress.step(Some(0.10), "calling inference backend").await;

        if cancel.is_cancelled() {
            return Err(VerbError::Cancelled);
        }

        // Dispatch to the concrete backend.
        let backend = ctx
            .backend
            .as_ref()
            .ok_or(VerbError::MissingContext("inference backend"))?;

        let generated_pngs = call_frame_backend(
            backend.as_ref(),
            context_pngs,
            inputs.num_frames,
            canvas_width,
            canvas_height,
            progress.clone(),
            cancel.clone(),
        )
        .await
        .map_err(VerbError::Backend)?;

        if cancel.is_cancelled() {
            return Err(VerbError::Cancelled);
        }

        progress
            .step(Some(0.85), "decoding and snapping generated frames")
            .await;

        let (frames, cels, pixel_buffers) = decode_and_build_effects(
            generated_pngs,
            ctx.active_palette.as_ref(),
            layer_id,
            frame_duration_ms,
        )?;

        progress.step(Some(1.0), "done").await;

        let count = frames.len();
        let summary = format!(
            "Continue: add {count} frame{} after frame {}",
            if count == 1 { "" } else { "s" },
            active_frame.get(),
        );
        let thumbnail = pixel_buffers.first().map(|b| b.pixels.clone());

        Ok(VerbOutput {
            summary,
            effects: vec![VerbEffect::AddFrames {
                sprite: sprite_id,
                after: Some(active_frame),
                frames,
                cels,
                pixel_buffers,
            }],
            thumbnail,
            // Backends report incremental cost via VerbProgressEvent::Cost;
            // zero here means "not double-counted in the commit summary".
            actual_cost: ActualCost {
                elapsed: started.elapsed(),
                usd_cents: 0.0,
                backend: backend_id,
                tokens_input: None,
                tokens_output: None,
            },
            notes: vec![],
        })
    }
}

// ── Frame decode + effect builder ────────────────────────────────────────────

/// Decodes each PNG, optionally palette-snaps it, and returns the three
/// parallel `VerbEffect::AddFrames` vecs.
fn decode_and_build_effects(
    generated_pngs: Vec<Vec<u8>>,
    active_palette: Option<&Palette>,
    layer_id: pixhaus_core::project::LayerId,
    frame_duration_ms: u32,
) -> Result<(Vec<Frame>, Vec<Cel>, Vec<NewPixelBuffer>)> {
    let mut frames: Vec<Frame> = Vec::with_capacity(generated_pngs.len());
    let mut cels: Vec<Cel> = Vec::with_capacity(generated_pngs.len());
    let mut pixel_buffers: Vec<NewPixelBuffer> = Vec::with_capacity(generated_pngs.len());

    for (idx, png_bytes) in (0u32..).zip(generated_pngs) {
        let mut pixel_data = decode_from_png(&png_bytes)?;

        if let Some(palette) = active_palette {
            snap_to_palette(&mut pixel_data, palette);
        }

        let placeholder = PixelBufferId::new(idx);
        let size = Size::new(pixel_data.width, pixel_data.height);

        frames.push(Frame {
            duration_ms: frame_duration_ms,
            ..Frame::default()
        });

        // frame_index is relative to the first new frame; the host renumbers on commit.
        cels.push(Cel::raster(
            layer_id,
            FrameIndex::new(idx),
            placeholder,
            size,
        ));

        pixel_buffers.push(NewPixelBuffer {
            placeholder,
            pixels: pixel_data,
        });
    }

    Ok((frames, cels, pixel_buffers))
}

// ── Backend dispatch ──────────────────────────────────────────────────────────

/// Dispatches to the concrete backend and returns generated frame PNG bytes.
///
/// The verb protocol stores the backend as `Arc<dyn plugin::InferenceBackend>`
/// (the thin capability-advertisement trait). Verbs that need inference
/// calls downcast to concrete adapter types via `as_any()`. This function
/// tries each of the six built-in adapters in capability order, building
/// the `FrameInterpolationRequest` once and forwarding it to the first
/// match.
///
/// If none of the downcasts match — which happens when the registered
/// backend is a test stub that only implements `plugin::InferenceBackend` —
/// the function returns an error describing which concrete types it tried.
async fn call_frame_backend(
    backend: &dyn crate::plugin::InferenceBackend,
    context_frames: Vec<Vec<u8>>,
    num_outputs: u32,
    canvas_width: u32,
    canvas_height: u32,
    progress: VerbProgress,
    cancel: CancellationToken,
) -> std::result::Result<Vec<Vec<u8>>, String> {
    use crate::backends::{
        anthropic::AnthropicBackend, comfyui::ComfyUiBackend, ollama::OllamaBackend,
        openai::OpenAiBackend, replicate::ReplicateBackend, stability::StabilityBackend,
    };

    let any = backend.as_any();

    // Build the request once before the downcast chain so the context
    // frames are not cloned per attempt. Each arm in the if-else ladder
    // is disjoint at runtime; the compiler accepts the single move.
    let request = InferenceRequest::FrameInterpolation(FrameInterpolationRequest {
        model: None,
        frames: context_frames,
        num_outputs,
    });

    if let Some(b) = any.downcast_ref::<ReplicateBackend>() {
        return invoke_backend(b, request, canvas_width, canvas_height, progress, cancel).await;
    }
    if let Some(b) = any.downcast_ref::<StabilityBackend>() {
        return invoke_backend(b, request, canvas_width, canvas_height, progress, cancel).await;
    }
    if let Some(b) = any.downcast_ref::<OpenAiBackend>() {
        return invoke_backend(b, request, canvas_width, canvas_height, progress, cancel).await;
    }
    if let Some(b) = any.downcast_ref::<AnthropicBackend>() {
        return invoke_backend(b, request, canvas_width, canvas_height, progress, cancel).await;
    }
    if let Some(b) = any.downcast_ref::<OllamaBackend>() {
        return invoke_backend(b, request, canvas_width, canvas_height, progress, cancel).await;
    }
    if let Some(b) = any.downcast_ref::<ComfyUiBackend>() {
        return invoke_backend(b, request, canvas_width, canvas_height, progress, cancel).await;
    }

    #[cfg(test)]
    if let Some(b) = any.downcast_ref::<TestFrameBackend>() {
        return Ok(b.frames.clone());
    }

    Err(format!(
        "backend `{}` is not a recognized concrete adapter; \
         the Continue verb dispatches through a downcast chain covering the \
         six built-in adapters (Replicate, Stability, OpenAI, Anthropic, \
         Ollama, ComfyUI) — register one of those or implement the downcast \
         contract described in docs/verb-protocol.md",
        backend.id()
    ))
}

/// Calls `FullBackend::invoke` and extracts PNG frames from the response.
///
/// A `FrameInterpolation` response yields frames directly. An `Image`
/// response (e.g. a `ComfyUI` workflow that emits multiple PNGs) treats
/// each image as one output frame, in order. Other response types surface
/// as errors.
async fn invoke_backend<B: FullBackend>(
    backend: &B,
    request: InferenceRequest,
    _canvas_width: u32,
    _canvas_height: u32,
    progress: VerbProgress,
    cancel: CancellationToken,
) -> std::result::Result<Vec<Vec<u8>>, String> {
    backend
        .invoke(request, progress, cancel)
        .await
        .map_err(|e| e.to_string())
        .and_then(|resp| match resp {
            InferenceResponse::Frames(r) => Ok(r.frames),
            InferenceResponse::Image(r) => Ok(r.images),
            _ => Err(
                "backend returned an unexpected response type for a frame-generation request; \
                      expected Frames or Image"
                    .into(),
            ),
        })
}

// ── PNG encode / decode ───────────────────────────────────────────────────────

/// Encodes RGBA8 `PixelData` to a PNG byte vector.
///
/// Strips stride padding when present so the `image` encoder receives a
/// densely-packed row buffer.
///
/// Returns a typed `Schema` error if the input fails `is_well_formed`
/// (stride too small, byte count off) — without that guard the
/// `pixels.bytes[start..start + tight_stride]` slice below could panic
/// when given a malformed `PixelData` from an untrusted IPC client.
fn encode_to_png(pixels: &PixelData) -> Result<Vec<u8>> {
    if !pixels.is_well_formed() {
        return Err(VerbError::Schema(
            "continue: pixel buffer dimensions and byte count are inconsistent; \
             refusing to encode"
                .into(),
        ));
    }
    let tight_stride = pixels.width as usize * pixels.bytes_per_pixel as usize;
    let packed: Vec<u8> = if pixels.stride as usize == tight_stride {
        pixels.bytes.clone()
    } else {
        (0..pixels.height as usize)
            .flat_map(|row| {
                let start = row * pixels.stride as usize;
                pixels.bytes[start..start + tight_stride].iter().copied()
            })
            .collect()
    };

    let mut out = Vec::new();
    let encoder = image::codecs::png::PngEncoder::new(Cursor::new(&mut out));
    encoder
        .write_image(
            &packed,
            pixels.width,
            pixels.height,
            image::ExtendedColorType::Rgba8,
        )
        .map_err(|e| VerbError::Backend(format!("PNG encode failed: {e}")))?;

    Ok(out)
}

/// Decodes a PNG byte slice into tightly-packed RGBA8 `PixelData`.
fn decode_from_png(png: &[u8]) -> Result<PixelData> {
    let img = image::load_from_memory_with_format(png, ImageFormat::Png)
        .map_err(|e| VerbError::Backend(format!("PNG decode failed: {e}")))?;

    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();
    Ok(PixelData::rgba8(width, height, rgba.into_raw()))
}

// ── Palette snapping ──────────────────────────────────────────────────────────

/// Replaces every opaque pixel's RGB with the nearest palette color.
///
/// Alpha is preserved unchanged. Pixels with `a == 0` are skipped to
/// avoid introducing palette noise into transparent regions. An empty
/// palette is logged at `warn` level (not silently ignored) so a
/// misconfigured runtime is visible in the editor's normal log surface.
fn snap_to_palette(data: &mut PixelData, palette: &Palette) {
    use pixhaus_core::project::color::Rgba;

    if palette.colors.is_empty() {
        warn!(
            verb = CONTINUE_VERB_ID,
            "palette snap requested but active palette is empty; skipping"
        );
        return;
    }
    if data.bytes_per_pixel != 4 {
        return; // only RGBA8 is supported
    }

    let bpp = data.bytes_per_pixel as usize;
    let stride = data.stride as usize;
    let width = data.width as usize;

    for y in 0..data.height as usize {
        for x in 0..width {
            let offset = y * stride + x * bpp;
            let end = offset + 4;
            if end > data.bytes.len() {
                break;
            }
            let a = data.bytes[offset + 3];
            if a == 0 {
                continue; // transparent — leave untouched
            }
            let target = Rgba::new(
                data.bytes[offset],
                data.bytes[offset + 1],
                data.bytes[offset + 2],
                a,
            );
            if let Some(idx) = palette.nearest_index(target) {
                if let Some(nearest) = palette.color_at(idx) {
                    data.bytes[offset] = nearest.r;
                    data.bytes[offset + 1] = nearest.g;
                    data.bytes[offset + 2] = nearest.b;
                    // alpha preserved
                }
            }
        }
    }
}

// ── Test backend (module-level so call_frame_backend's #[cfg(test)] arm sees it) ──

/// Minimal backend stub that produces deterministic PNG frames.
///
/// Defined at module level (not inside `mod tests`) so the `#[cfg(test)]`
/// downcast arm in `call_frame_backend` can reference it by name.
#[cfg(test)]
#[derive(Debug)]
pub(crate) struct TestFrameBackend {
    /// Pre-encoded PNG bytes to return per generated frame.
    pub frames: Vec<Vec<u8>>,
}

#[cfg(test)]
impl crate::plugin::backend::InferenceBackend for TestFrameBackend {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn id(&self) -> &'static str {
        "test.frame_backend"
    }
    fn capabilities(&self) -> crate::plugin::descriptor::BackendCapabilities {
        crate::plugin::descriptor::BackendCapabilities::IMAGE_GENERATION
            .union(crate::plugin::descriptor::BackendCapabilities::FRAME_INTERPOLATION)
    }
    fn cost_estimate(
        &self,
        _required: crate::plugin::descriptor::BackendCapabilities,
    ) -> crate::plugin::descriptor::CostEstimate {
        crate::plugin::descriptor::CostEstimate::free()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use pixhaus_core::project::{FrameIndex, IVec2, LayerId, PaletteId, ProjectMetadata, SpriteId};

    use super::*;
    use crate::plugin::{
        context::ReferenceImage, descriptor::BackendCapabilities, output::VerbEffect,
        runtime::VerbRuntime,
    };

    // ── Helpers ───────────────────────────────────────────────────────────────

    fn make_test_backend(count: usize) -> TestFrameBackend {
        let frame = tiny_rgba_png(2, 2, [0, 0, 0, 255]);
        TestFrameBackend {
            frames: vec![frame; count],
        }
    }

    fn metadata() -> ProjectMetadata {
        ProjectMetadata {
            name: "continue-test".into(),
            description: None,
            author: None,
            created_at: 0,
            updated_at: 0,
            editor_version: "0".into(),
        }
    }

    /// Encodes a flat-color RGBA PNG in memory for use as a reference frame.
    fn tiny_rgba_png(width: u32, height: u32, color: [u8; 4]) -> Vec<u8> {
        let pixels = PixelData::rgba8(
            width,
            height,
            color
                .iter()
                .cycle()
                .take((width * height * 4) as usize)
                .copied()
                .collect(),
        );
        encode_to_png(&pixels).unwrap()
    }

    /// A context with sprite, layer, frame, and one reference frame.
    fn ctx_with_reference() -> VerbContext {
        let mut ctx = VerbContext::empty(metadata());
        ctx.active_sprite = Some(SpriteId::new(1));
        ctx.active_layer = Some(LayerId::new(1));
        ctx.active_frame = Some(FrameIndex::new(2));
        ctx.references = vec![ReferenceImage {
            origin: IVec2::new(0, 0),
            pixels: PixelData::rgba8(
                2,
                2,
                [255u8, 0, 0, 255]
                    .iter()
                    .cycle()
                    .take(16)
                    .copied()
                    .collect(),
            ),
            label: "frame:2".into(),
        }];
        ctx
    }

    // ── Descriptor ────────────────────────────────────────────────────────────

    #[test]
    fn descriptor_has_correct_id_and_capabilities() {
        let verb = ContinueVerb::new();
        let d = verb.descriptor();
        assert_eq!(d.id, VerbId::new(CONTINUE_VERB_ID));
        assert!(
            d.required_capabilities
                .contains(BackendCapabilities::IMAGE_GENERATION)
        );
        assert!(
            d.required_capabilities
                .contains(BackendCapabilities::FRAME_INTERPOLATION)
        );
        assert!(d.cancellable);
        assert!(d.streaming);
        assert!(!d.output_kinds.is_empty());
    }

    // ── Validation ────────────────────────────────────────────────────────────

    #[test]
    fn validate_rejects_zero_frames() {
        let verb = ContinueVerb::new();
        let inputs = VerbInputs::from_struct(&ContinueInputs {
            num_frames: 0,
            frame_duration_ms: None,
        })
        .unwrap();
        assert!(verb.validate(&inputs).is_err());
    }

    #[test]
    fn validate_rejects_too_many_frames() {
        let verb = ContinueVerb::new();
        let inputs = VerbInputs::from_struct(&ContinueInputs {
            num_frames: MAX_FRAMES + 1,
            frame_duration_ms: None,
        })
        .unwrap();
        assert!(verb.validate(&inputs).is_err());
    }

    #[test]
    fn validate_rejects_zero_duration() {
        let verb = ContinueVerb::new();
        let inputs = VerbInputs::from_struct(&ContinueInputs {
            num_frames: 1,
            frame_duration_ms: Some(0),
        })
        .unwrap();
        assert!(verb.validate(&inputs).is_err());
    }

    #[test]
    fn validate_accepts_defaults() {
        let verb = ContinueVerb::new();
        let inputs = VerbInputs::from_struct(&ContinueInputs::default()).unwrap();
        assert!(verb.validate(&inputs).is_ok());
    }

    #[test]
    fn validate_accepts_max_frames() {
        let verb = ContinueVerb::new();
        let inputs = VerbInputs::from_struct(&ContinueInputs {
            num_frames: MAX_FRAMES,
            frame_duration_ms: Some(200),
        })
        .unwrap();
        assert!(verb.validate(&inputs).is_ok());
    }

    // ── Missing context errors ────────────────────────────────────────────────

    #[tokio::test]
    async fn invoke_requires_active_sprite() {
        let runtime = VerbRuntime::new();
        runtime.register(ContinueVerb::new()).unwrap();
        runtime.register_backend(make_test_backend(1), 0).unwrap();

        let inputs = VerbInputs::from_struct(&ContinueInputs::default()).unwrap();

        let inv = runtime
            .invoke(
                &VerbId::new(CONTINUE_VERB_ID),
                VerbContext::empty(metadata()),
                inputs,
            )
            .unwrap();
        let res = inv.finish().await;
        assert!(matches!(
            res,
            Err(VerbError::MissingContext("active sprite"))
        ));
    }

    #[tokio::test]
    async fn invoke_requires_active_layer() {
        let runtime = VerbRuntime::new();
        runtime.register(ContinueVerb::new()).unwrap();
        runtime.register_backend(make_test_backend(1), 0).unwrap();

        let inputs = VerbInputs::from_struct(&ContinueInputs::default()).unwrap();

        let mut ctx = VerbContext::empty(metadata());
        ctx.active_sprite = Some(SpriteId::new(1));
        // active_layer deliberately absent

        let inv = runtime
            .invoke(&VerbId::new(CONTINUE_VERB_ID), ctx, inputs)
            .unwrap();
        let res = inv.finish().await;
        assert!(matches!(
            res,
            Err(VerbError::MissingContext("active layer"))
        ));
    }

    #[tokio::test]
    async fn invoke_requires_references() {
        let runtime = VerbRuntime::new();
        runtime.register(ContinueVerb::new()).unwrap();
        runtime.register_backend(make_test_backend(1), 0).unwrap();

        let inputs = VerbInputs::from_struct(&ContinueInputs::default()).unwrap();

        let mut ctx = VerbContext::empty(metadata());
        ctx.active_sprite = Some(SpriteId::new(1));
        ctx.active_layer = Some(LayerId::new(1));
        // ctx.references is empty

        let inv = runtime
            .invoke(&VerbId::new(CONTINUE_VERB_ID), ctx, inputs)
            .unwrap();
        let res = inv.finish().await;
        assert!(matches!(res, Err(VerbError::MissingContext(_))));
    }

    // ── Full round-trip ───────────────────────────────────────────────────────

    #[tokio::test]
    async fn invoke_produces_add_frames_effect() {
        let runtime = VerbRuntime::new();
        runtime.register(ContinueVerb::new()).unwrap();
        runtime.register_backend(make_test_backend(2), 0).unwrap();

        let inputs = VerbInputs::from_struct(&ContinueInputs {
            num_frames: 2,
            frame_duration_ms: Some(150),
        })
        .unwrap();

        let inv = runtime
            .invoke(&VerbId::new(CONTINUE_VERB_ID), ctx_with_reference(), inputs)
            .unwrap();
        let preview = inv.finish().await.unwrap();

        assert_eq!(preview.verb.as_str(), CONTINUE_VERB_ID);
        assert_eq!(preview.output.effects.len(), 1);
        match &preview.output.effects[0] {
            VerbEffect::AddFrames {
                after,
                frames,
                cels,
                pixel_buffers,
                ..
            } => {
                assert_eq!(*after, Some(FrameIndex::new(2)));
                assert_eq!(frames.len(), 2);
                assert_eq!(cels.len(), 2);
                assert_eq!(pixel_buffers.len(), 2);
                assert!(frames.iter().all(|f| f.duration_ms == 150));
            }
            other => panic!("expected AddFrames, got {other:?}"),
        }
    }

    // ── PNG encode/decode round-trip ─────────────────────────────────────────

    #[test]
    fn png_round_trip_preserves_pixels() {
        let original = PixelData::rgba8(4, 4, (0..64).collect());
        let encoded = encode_to_png(&original).unwrap();
        let decoded = decode_from_png(&encoded).unwrap();
        assert_eq!(decoded.width, 4);
        assert_eq!(decoded.height, 4);
        assert_eq!(decoded.bytes, original.bytes);
    }

    #[test]
    fn encode_strips_stride_padding() {
        // Build a PixelData with 2px stride padding per row.
        let w = 2u32;
        let h = 2u32;
        let stride = (w * 4 + 2) as usize; // 2 extra bytes per row
        let mut padded = vec![0u8; stride * h as usize];
        // Row 0: [255, 0, 0, 255, 0, 0, 255, 0, 0, 255, xx, xx]
        // Row 1: [0, 255, 0, 255, 0, 255, 0, 255, xx, xx]
        padded[0..4].copy_from_slice(&[255, 0, 0, 255]);
        padded[4..8].copy_from_slice(&[255, 0, 0, 255]);
        padded[stride..stride + 4].copy_from_slice(&[0, 255, 0, 255]);
        padded[stride + 4..stride + 8].copy_from_slice(&[0, 255, 0, 255]);

        let pixels = PixelData {
            width: w,
            height: h,
            bytes_per_pixel: 4,
            stride: u32::try_from(stride).unwrap(),
            bytes: padded,
        };

        let encoded = encode_to_png(&pixels).unwrap();
        let decoded = decode_from_png(&encoded).unwrap();
        assert_eq!(decoded.width, w);
        assert_eq!(decoded.height, h);
        // Tight stride: no padding
        assert_eq!(decoded.bytes.len(), (w * h * 4) as usize);
    }

    #[test]
    fn encode_rejects_malformed_pixeldata() {
        // bytes.len() < stride * height — without the well-formed
        // guard this would panic on the row-slice indexing below.
        let pixels = PixelData {
            width: 4,
            height: 4,
            bytes_per_pixel: 4,
            stride: 16,
            bytes: vec![0u8; 8], // way too few
        };
        let result = encode_to_png(&pixels);
        assert!(matches!(result, Err(VerbError::Schema(_))));
    }

    // ── Palette snapping ─────────────────────────────────────────────────────

    #[test]
    fn snap_replaces_near_color_with_palette_color() {
        use pixhaus_core::project::color::Rgba;

        let mut pixels = PixelData::rgba8(1, 1, vec![200, 10, 10, 255]);
        let palette = Palette::from_colors(
            PaletteId::new(1),
            "test",
            vec![Rgba::opaque(255, 0, 0), Rgba::opaque(0, 255, 0)],
        );
        snap_to_palette(&mut pixels, &palette);
        assert_eq!(&pixels.bytes[0..3], &[255, 0, 0]);
        assert_eq!(pixels.bytes[3], 255); // alpha unchanged
    }

    #[test]
    fn snap_skips_transparent_pixels() {
        use pixhaus_core::project::color::Rgba;

        let mut pixels = PixelData::rgba8(1, 1, vec![200, 10, 10, 0]);
        let palette =
            Palette::from_colors(PaletteId::new(1), "test", vec![Rgba::opaque(255, 0, 0)]);
        snap_to_palette(&mut pixels, &palette);
        // Pixel was transparent — must not be touched.
        assert_eq!(&pixels.bytes[0..4], &[200, 10, 10, 0]);
    }

    #[test]
    fn snap_empty_palette_is_no_op() {
        use pixhaus_core::project::Palette;

        let orig = vec![200u8, 10, 10, 255];
        let mut pixels = PixelData::rgba8(1, 1, orig.clone());
        let empty_palette = Palette::from_colors(PaletteId::new(1), "empty", vec![]);
        snap_to_palette(&mut pixels, &empty_palette);
        assert_eq!(pixels.bytes, orig);
    }
}
