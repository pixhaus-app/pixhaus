//! Verb: Tile — generate a 47-tile blob autotile set from example tiles.
//!
//! The verb takes 1–3 example `PixelData` tiles that show representative
//! transitions in the desired visual style. It runs two backend passes:
//!
//! 1. **Style analysis** (`VISION_LANGUAGE`) — a brief VLM call that produces a
//!    concise style description used to condition the next pass.
//! 2. **Atlas generation** (`IMAGE_GENERATION`) — produces an 8×6 grid of
//!    tile-sized cells covering all 47 blob autotile configurations plus a
//!    blank tile 0.
//!
//! Output is a single [`VerbEffect::AddTileset`] containing the new
//! [`Tileset`] record and the atlas pixel buffer. The tileset is configured
//! for the `AutotileKind::Blob47` resolver so the tilemap engine can
//! immediately paint autotile-aware strokes.
//!
//! See `docs/verbs/tile.md` for the user-facing reference.

use std::io::Cursor;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use image::{ImageFormat, RgbaImage};
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use pixhaus_core::project::{PixelBufferId, Size, Tileset, TilesetId, TilesetSource, UserData};

use crate::backends::{
    ChatMessage, ChatRole, ContentPart, ImageGenRequest, ImageGenResponse, InferenceBackend,
    InferenceRequest, InferenceResponse, TextGenRequest, TextGenResponse,
};
use crate::plugin::progress::LogLevel;
use crate::plugin::verb::Verb;
use crate::plugin::{
    ActualCost, BackendCapabilities, CostEstimate, EffectKind, NewPixelBuffer, PixelData, Result,
    VerbContext, VerbDescriptor, VerbEffect, VerbError, VerbId, VerbInputs, VerbOutput,
    VerbProgress, VerbProgressEvent,
};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Stable identifier for the built-in tile verb.
pub const TILE_VERB_ID: &str = "pixhaus.builtin.tile";

/// Tiles laid out per row in the atlas.
const ATLAS_COLS: u32 = 8;

/// Rows in the atlas: ceil(48 / 8) = 6.
/// Cell 0 is the blank tile; cells 1–47 are the 47 blob configurations.
const ATLAS_ROWS: u32 = 6;

/// Total cells in the atlas (47 blob tiles + 1 blank).
const ATLAS_TILE_COUNT: u32 = ATLAS_COLS * ATLAS_ROWS;

/// Capabilities required by the tile verb: image generation for the atlas plus
/// vision-language for style conditioning from the example tiles.
const REQUIRED_CAPS: BackendCapabilities =
    BackendCapabilities::IMAGE_GENERATION.union(BackendCapabilities::VISION_LANGUAGE);

// ---------------------------------------------------------------------------
// Input struct
// ---------------------------------------------------------------------------

/// Inputs accepted by [`TileVerb`].
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TileInputs {
    /// 1–3 RGBA8 example tiles that define the visual style. Each should
    /// show one representative transition variant (e.g. an isolated tile, a
    /// straight edge, or a corner). More examples improve style fidelity.
    pub examples: Vec<PixelData>,
    /// Tile width in pixels. Must be 1–256. Defaults to 16.
    #[serde(default = "default_tile_dim")]
    pub tile_width: u32,
    /// Tile height in pixels. Must be 1–256. Defaults to 16.
    #[serde(default = "default_tile_dim")]
    pub tile_height: u32,
    /// Display name for the resulting tileset. Defaults to `"Autotile"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tileset_name: Option<String>,
}

fn default_tile_dim() -> u32 {
    16
}

// ---------------------------------------------------------------------------
// Verb struct
// ---------------------------------------------------------------------------

/// Generates a 47-tile blob autotile set from style-conditioning examples.
///
/// Construct via [`TileVerb::new`] (no args). Register it with
/// [`crate::plugin::runtime::VerbRuntime`] once at editor startup; the
/// runtime selects a capability-matching backend and injects it into
/// `VerbContext::backend` per invocation.
///
/// ```no_run
/// use pixhaus_ai::verbs::tile::TileVerb;
/// use pixhaus_ai::plugin::VerbRuntime;
///
/// let runtime = VerbRuntime::new();
/// runtime.register(TileVerb::new()).unwrap();
/// ```
pub struct TileVerb {
    descriptor: VerbDescriptor,
}

impl TileVerb {
    /// Constructs a tile verb. Backend resolution happens at invoke time
    /// from `VerbContext::backend`, which the runtime injects after
    /// matching the verb's `required_capabilities` against the registered
    /// backend pool.
    #[must_use]
    // `serde_json::json!` internally calls `unwrap` on infallible builders;
    // the workspace lint bans `unwrap` but this constructor is exempt so we
    // can express the schema as a JSON literal rather than a `Value::Object`.
    #[allow(clippy::disallowed_methods)]
    pub fn new() -> Self {
        let input_schema = serde_json::json!({
            "type": "object",
            "properties": {
                "examples": {
                    "type": "array",
                    "minItems": 1,
                    "maxItems": 3,
                    "items": {
                        "type": "object",
                        "properties": {
                            "width":  { "type": "integer", "minimum": 1 },
                            "height": { "type": "integer", "minimum": 1 },
                            "bytes_per_pixel": { "type": "integer" },
                            "stride": { "type": "integer" },
                            "bytes":  { "type": "array", "items": { "type": "integer" } }
                        },
                        "required": ["width", "height", "bytes_per_pixel", "stride", "bytes"]
                    }
                },
                "tile_width":  { "type": "integer", "minimum": 1, "maximum": 256, "default": 16 },
                "tile_height": { "type": "integer", "minimum": 1, "maximum": 256, "default": 16 },
                "tileset_name": { "type": ["string", "null"] }
            },
            "required": ["examples"]
        });

        Self {
            descriptor: VerbDescriptor {
                id: VerbId::new(TILE_VERB_ID),
                display_name: "Tile".into(),
                description: "Generate a 47-tile blob autotile set from 1–3 example transitions"
                    .into(),
                version: env!("CARGO_PKG_VERSION").into(),
                required_capabilities: REQUIRED_CAPS,
                input_schema,
                output_schema: None,
                output_kinds: vec![EffectKind::AddTileset],
                cost_estimate: CostEstimate {
                    typical_latency: Duration::from_secs(30),
                    max_latency: Duration::from_secs(120),
                    typical_usd_cents: 5.0,
                    max_usd_cents: 20.0,
                },
                streaming: true,
                cancellable: true,
                documentation_url: Some("docs/verbs/tile.md".into()),
            },
        }
    }
}

impl Default for TileVerb {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Verb implementation
// ---------------------------------------------------------------------------

#[async_trait]
impl Verb for TileVerb {
    fn descriptor(&self) -> &VerbDescriptor {
        &self.descriptor
    }

    fn validate(&self, inputs: &VerbInputs) -> Result<()> {
        let parsed: TileInputs = inputs.deserialize()?;

        if parsed.examples.is_empty() {
            return Err(VerbError::Schema(
                "tile: at least one example tile is required".into(),
            ));
        }
        if parsed.examples.len() > 3 {
            return Err(VerbError::Schema(
                "tile: at most 3 example tiles are accepted".into(),
            ));
        }
        for (i, ex) in parsed.examples.iter().enumerate() {
            if !ex.is_well_formed() {
                return Err(VerbError::Schema(format!(
                    "tile: example[{i}] pixel buffer is malformed \
                     (dimensions and byte count are inconsistent)"
                )));
            }
            if ex.bytes_per_pixel != 4 {
                return Err(VerbError::Schema(format!(
                    "tile: example[{i}] must be RGBA8 (4 bytes per pixel), \
                     got {}",
                    ex.bytes_per_pixel
                )));
            }
        }
        if parsed.tile_width == 0 || parsed.tile_width > 256 {
            return Err(VerbError::Schema(format!(
                "tile: tile_width must be 1–256, got {}",
                parsed.tile_width
            )));
        }
        if parsed.tile_height == 0 || parsed.tile_height > 256 {
            return Err(VerbError::Schema(format!(
                "tile: tile_height must be 1–256, got {}",
                parsed.tile_height
            )));
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
        let inputs: TileInputs = inputs.deserialize_owned()?;
        let sprite_id = ctx.require_sprite_id()?;

        let backend = crate::verbs::ctx_fat_backend(&ctx)?;

        progress
            .send(VerbProgressEvent::Started {
                backend: Some(backend.backend_id().to_owned()),
            })
            .await;
        progress
            .step(Some(0.05), "analyzing example tile style")
            .await;

        // Pass 1: style analysis (non-fatal; falls back to a generic prompt).
        let style_desc =
            analyze_style(&inputs.examples, backend, progress.clone(), &cancel).await?;

        if cancel.is_cancelled() {
            return Err(VerbError::Cancelled);
        }

        progress.step(Some(0.30), "generating autotile atlas").await;

        // Pass 2: full autotile atlas generation. The anchor sheet, if
        // any, is the style fallback when no `examples` are supplied —
        // an anchored tileset entity inherits its parent reference's
        // look automatically.
        let anchor_style = crate::verbs::anchor_style_image_bytes(&ctx);
        let atlas = generate_atlas(
            &inputs,
            &style_desc,
            anchor_style,
            backend,
            progress.clone(),
            &cancel,
        )
        .await?;

        if cancel.is_cancelled() {
            return Err(VerbError::Cancelled);
        }

        progress.step(Some(0.90), "building tileset").await;

        let tileset_name = inputs
            .tileset_name
            .clone()
            .unwrap_or_else(|| "Autotile".into());

        let buffer_id = PixelBufferId::new(0);
        let tileset = Tileset {
            id: TilesetId::new(0), // placeholder; host rewrites on commit
            name: tileset_name.clone(),
            tile_size: Size::new(inputs.tile_width, inputs.tile_height),
            tile_count: ATLAS_TILE_COUNT,
            base_index: 1,
            source: TilesetSource::Inline { buffer: buffer_id },
            properties: Vec::new(),
            autotile: None,
            user_data: UserData::default(),
        };

        let pixel_buffer = NewPixelBuffer {
            placeholder: buffer_id,
            pixels: atlas.clone(),
        };

        // Use tile 1 (the isolated configuration) as the preview thumbnail.
        let thumbnail = extract_tile(&atlas, 1, inputs.tile_width, inputs.tile_height, ATLAS_COLS);

        progress.step(Some(1.0), "autotile set ready").await;

        let elapsed = started.elapsed();
        let summary = format!(
            "Add autotile set \"{tileset_name}\" ({} tiles, {}×{} px each)",
            ATLAS_TILE_COUNT - 1,
            inputs.tile_width,
            inputs.tile_height,
        );

        Ok(VerbOutput {
            summary,
            effects: vec![VerbEffect::AddTileset {
                sprite: sprite_id,
                tileset,
                pixel_buffers: vec![pixel_buffer],
            }],
            thumbnail,
            actual_cost: ActualCost::free(elapsed.max(Duration::from_micros(1))),
            notes: vec![],
        })
    }
}

// ---------------------------------------------------------------------------
// Step helpers
// ---------------------------------------------------------------------------

/// Calls the VLM to describe the visual style of the example tiles.
///
/// Returns a concise style description string suitable for appending to an
/// image-generation prompt. On any backend error, logs a warning and returns
/// a safe fallback string rather than failing the whole verb — style analysis
/// is advisory.
async fn analyze_style(
    examples: &[PixelData],
    backend: &dyn InferenceBackend,
    progress: VerbProgress,
    cancel: &CancellationToken,
) -> Result<String> {
    if examples.is_empty() {
        return Ok("seamless pixel art terrain with clean edge treatment".into());
    }

    let mut parts: Vec<ContentPart> = vec![ContentPart::text(
        "Analyze these pixel art tile examples. Describe the visual style in 2–3 sentences: \
         include key palette colors (hex), edge treatment, texture character, and theme. \
         The description will condition a 47-tile blob autotile set generation.",
    )];

    for (i, example) in examples.iter().enumerate() {
        match encode_pixel_data_as_png(example) {
            Ok(png) => parts.push(ContentPart::png(png)),
            Err(e) => {
                progress
                    .send(VerbProgressEvent::Log {
                        level: LogLevel::Warning,
                        message: format!("tile: could not encode example[{i}] as PNG: {e}"),
                    })
                    .await;
            }
        }
    }

    let request = InferenceRequest::Text(TextGenRequest {
        model: None,
        system: Some(
            "You are a pixel art style analyst. \
             Describe tile styles concisely for image-generation prompts."
                .into(),
        ),
        messages: vec![ChatMessage {
            role: ChatRole::User,
            content: parts,
        }],
        max_tokens: Some(160),
        temperature: Some(0.3),
        tools: Vec::new(),
        stop: Vec::new(),
    });

    let result = backend
        .invoke(request, progress.clone(), cancel.clone())
        .await;

    // Check cancellation before acting on the result — the backend may have
    // returned an error because it observed the token firing.
    if cancel.is_cancelled() {
        return Err(VerbError::Cancelled);
    }

    match result {
        Ok(InferenceResponse::Text(TextGenResponse { content, .. })) => Ok(content),
        Ok(_) => {
            // Unexpected response shape — non-fatal, fall through to generic.
            progress
                .send(VerbProgressEvent::Log {
                    level: LogLevel::Warning,
                    message: "tile: unexpected response type from style analysis; \
                              using generic style description"
                        .into(),
                })
                .await;
            Ok("seamless pixel art terrain with consistent edge treatment".into())
        }
        Err(e) if e.is_cancelled() => Err(VerbError::Cancelled),
        Err(e) => {
            progress
                .send(VerbProgressEvent::Log {
                    level: LogLevel::Warning,
                    message: format!(
                        "tile: style analysis failed ({e}); using generic style description"
                    ),
                })
                .await;
            Ok("seamless pixel art terrain with consistent edge treatment".into())
        }
    }
}

/// Calls the image-generation backend to produce the 47-tile blob atlas.
///
/// The atlas is an 8×6 grid of `tile_width × tile_height` cells:
/// - Cell 0 (top-left): reserved empty/blank tile
/// - Cells 1–47: the 47 blob configurations, in `BLOB47_MASKS` index order
///
/// If the backend returns an image at different dimensions, it is resized
/// with nearest-neighbour to fit exactly.
async fn generate_atlas(
    inputs: &TileInputs,
    style_desc: &str,
    anchor_style: Option<Vec<u8>>,
    backend: &dyn InferenceBackend,
    progress: VerbProgress,
    cancel: &CancellationToken,
) -> Result<PixelData> {
    let atlas_w = inputs.tile_width.saturating_mul(ATLAS_COLS);
    let atlas_h = inputs.tile_height.saturating_mul(ATLAS_ROWS);

    let style_clause = style_desc.trim();
    let prompt = format!(
        "pixel art sprite sheet, {ATLAS_COLS} columns by {ATLAS_ROWS} rows grid, \
         each cell {tw}x{th} pixels, blob wang autotile set showing all 47 neighbor \
         transition variants, top-left cell empty, row-major order, all tile edges \
         seamlessly tileable, no anti-aliasing, limited palette, {style_clause}",
        tw = inputs.tile_width,
        th = inputs.tile_height,
    );

    // Style image precedence: first user-supplied example, then the
    // entity's anchor sheet (B10.3). Anchored tilesets inherit their
    // parent reference's look automatically when no examples are passed.
    let style_image = inputs
        .examples
        .first()
        .and_then(|ex| encode_pixel_data_as_png(ex).ok())
        .or(anchor_style);

    let request = InferenceRequest::ImageGeneration(ImageGenRequest {
        model: None,
        prompt,
        negative_prompt: Some(
            "anti-aliasing, blur, gradients, smooth edges, 3d, realistic, photo".into(),
        ),
        width: atlas_w,
        height: atlas_h,
        steps: None,
        seed: None,
        num_images: 1,
        style_image,
    });

    let response = backend
        .invoke(request, progress.clone(), cancel.clone())
        .await;

    if cancel.is_cancelled() {
        return Err(VerbError::Cancelled);
    }

    let image_bytes = match response {
        Ok(InferenceResponse::Image(ImageGenResponse { mut images, .. })) => {
            if images.is_empty() {
                return Err(VerbError::Backend(
                    "tile: image generation returned no images".into(),
                ));
            }
            images.remove(0)
        }
        Ok(_) => {
            return Err(VerbError::Backend(
                "tile: unexpected response type from image generation".into(),
            ));
        }
        Err(e) if e.is_cancelled() => return Err(VerbError::Cancelled),
        Err(e) => {
            return Err(VerbError::Backend(format!(
                "tile: image generation failed: {e}"
            )));
        }
    };

    decode_and_fit_atlas(&image_bytes, atlas_w, atlas_h)
}

// ---------------------------------------------------------------------------
// Image helpers
// ---------------------------------------------------------------------------

/// Encodes `pixels` (RGBA8) as a PNG byte buffer.
fn encode_pixel_data_as_png(pixels: &PixelData) -> Result<Vec<u8>> {
    if pixels.bytes_per_pixel != 4 {
        return Err(VerbError::Backend(format!(
            "encode_pixel_data_as_png: expected RGBA8, got {} bytes per pixel",
            pixels.bytes_per_pixel
        )));
    }

    let packed = unpack_to_tight_rgba(pixels);
    let img = RgbaImage::from_raw(pixels.width, pixels.height, packed).ok_or_else(|| {
        VerbError::Backend(
            "encode_pixel_data_as_png: failed to construct RGBA image from pixel data".into(),
        )
    })?;

    let mut buf = Cursor::new(Vec::new());
    img.write_to(&mut buf, ImageFormat::Png).map_err(|e| {
        VerbError::Backend(format!(
            "encode_pixel_data_as_png: PNG encoding failed: {e}"
        ))
    })?;
    Ok(buf.into_inner())
}

/// Decodes a PNG and resizes it to `(expected_w, expected_h)` if needed,
/// then wraps it in a tightly-packed RGBA8 [`PixelData`].
fn decode_and_fit_atlas(png_bytes: &[u8], expected_w: u32, expected_h: u32) -> Result<PixelData> {
    let img = image::load_from_memory(png_bytes)
        .map_err(|e| VerbError::Backend(format!("tile: PNG decode failed: {e}")))?
        .into_rgba8();

    let img = if img.width() != expected_w || img.height() != expected_h {
        image::imageops::resize(
            &img,
            expected_w,
            expected_h,
            image::imageops::FilterType::Nearest,
        )
    } else {
        img
    };

    Ok(PixelData::rgba8(img.width(), img.height(), img.into_raw()))
}

/// Copies the pixel buffer's rows, dropping any stride padding, to produce a
/// tightly-packed `width × 4` RGBA sequence.
fn unpack_to_tight_rgba(pixels: &PixelData) -> Vec<u8> {
    let row_bytes = pixels.width as usize * 4;
    let stride = pixels.stride as usize;
    let mut packed = Vec::with_capacity(row_bytes * pixels.height as usize);
    for row in 0..pixels.height as usize {
        let start = row * stride;
        packed.extend_from_slice(&pixels.bytes[start..start + row_bytes]);
    }
    packed
}

/// Extracts the pixel data for tile at index `tile_idx` from the atlas.
///
/// Returns `None` if the tile region falls outside the atlas grid (either
/// horizontally — `tile_x + tile_w > atlas.width` — or vertically) or
/// outside the atlas byte buffer. The pixel-grid check guards against a
/// tile that would otherwise read pixels from the next row of the atlas
/// when its right edge crosses `atlas.width`; the byte-buffer check
/// stays as defense in depth in case `stride` is misreported.
fn extract_tile(
    atlas: &PixelData,
    tile_idx: u32,
    tile_w: u32,
    tile_h: u32,
    cols: u32,
) -> Option<PixelData> {
    let col = tile_idx % cols;
    let row = tile_idx / cols;
    let tile_x = col * tile_w;
    let tile_y = row * tile_h;

    // Pixel-grid bounds: refuse a tile whose right or bottom edge would
    // walk past the atlas extents. Without this an atlas whose width is
    // not a clean multiple of tile_w would silently produce tiles whose
    // right column was sourced from the leftmost pixels of the next row.
    if tile_x.checked_add(tile_w)? > atlas.width || tile_y.checked_add(tile_h)? > atlas.height {
        return None;
    }

    let bpp = atlas.bytes_per_pixel as usize;
    let stride = atlas.stride as usize;
    let row_bytes = tile_w as usize * bpp;

    let mut tile_bytes = Vec::with_capacity(tile_h as usize * row_bytes);
    for ty in 0..tile_h as usize {
        let src_y = tile_y as usize + ty;
        let src_x = tile_x as usize;
        let start = src_y * stride + src_x * bpp;
        let end = start + row_bytes;
        if end > atlas.bytes.len() {
            return None;
        }
        tile_bytes.extend_from_slice(&atlas.bytes[start..end]);
    }

    Some(PixelData {
        width: tile_w,
        height: tile_h,
        bytes_per_pixel: atlas.bytes_per_pixel,
        stride: tile_w * u32::from(atlas.bytes_per_pixel),
        bytes: tile_bytes,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backends::bridge::BackendProxy;
    use crate::backends::{BackendError, ImageGenResponse, TextGenResponse};
    use crate::plugin::runtime::VerbRuntime;
    use crate::plugin::{PixelData, VerbEffect};
    use async_trait::async_trait;
    use pixhaus_core::project::{ProjectMetadata, SpriteId};
    use std::time::Duration;
    use tokio_util::sync::CancellationToken;

    // ── Mock backend ──────────────────────────────────────────────────────────

    /// A test double for [`crate::backends::InferenceBackend`] that
    /// returns a solid-color atlas PNG for image-generation requests and a
    /// canned style description for VLM requests.
    struct MockImageBackend;

    #[async_trait]
    impl InferenceBackend for MockImageBackend {
        fn backend_id(&self) -> &'static str {
            "mock.image"
        }
        fn capabilities(&self) -> BackendCapabilities {
            REQUIRED_CAPS
        }
        fn supports_streaming(&self) -> bool {
            false
        }
        fn estimate_cost(&self, _req: &InferenceRequest) -> CostEstimate {
            CostEstimate::free()
        }
        async fn invoke(
            &self,
            request: InferenceRequest,
            _progress: VerbProgress,
            _cancel: CancellationToken,
        ) -> crate::backends::Result<InferenceResponse> {
            match request {
                InferenceRequest::Text(_) => Ok(InferenceResponse::Text(TextGenResponse {
                    content: "flat green terrain, #4a7a3c palette, crisp 1-pixel edges".into(),
                    model: "mock".into(),
                    input_tokens: 10,
                    output_tokens: 20,
                    stop_reason: Some("end_turn".into()),
                    tool_calls: Vec::new(),
                })),
                InferenceRequest::ImageGeneration(req) => {
                    let png = solid_rgba_png(req.width, req.height, [80, 120, 60, 255]);
                    Ok(InferenceResponse::Image(ImageGenResponse {
                        images: vec![png],
                        model: "mock".into(),
                    }))
                }
                _ => Err(BackendError::UnsupportedCapability),
            }
        }
    }

    /// Returns a solid-colour PNG with every pixel set to `color`.
    fn solid_rgba_png(width: u32, height: u32, color: [u8; 4]) -> Vec<u8> {
        let pixels: Vec<u8> = std::iter::repeat_n(color, (width * height) as usize)
            .flatten()
            .collect();
        let img = RgbaImage::from_raw(width, height, pixels).expect("valid solid image");
        let mut buf = Cursor::new(Vec::new());
        img.write_to(&mut buf, ImageFormat::Png)
            .expect("PNG encoding succeeds");
        buf.into_inner()
    }

    fn metadata() -> ProjectMetadata {
        ProjectMetadata {
            name: "tile-test".into(),
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

    fn single_example() -> PixelData {
        // 16×16 solid green example tile.
        PixelData::rgba8(16, 16, [80u8, 120, 60, 255].repeat(256))
    }

    fn valid_inputs() -> TileInputs {
        TileInputs {
            examples: vec![single_example()],
            tile_width: 16,
            tile_height: 16,
            tileset_name: Some("Test Tiles".into()),
        }
    }

    // ── Descriptor ────────────────────────────────────────────────────────────

    #[test]
    fn descriptor_declares_required_capabilities() {
        let verb = TileVerb::new();
        let caps = verb.descriptor().required_capabilities;
        assert!(caps.contains(BackendCapabilities::IMAGE_GENERATION));
        assert!(caps.contains(BackendCapabilities::VISION_LANGUAGE));
        assert!(verb.descriptor().cancellable);
        assert!(verb.descriptor().streaming);
        assert_eq!(verb.descriptor().output_kinds, vec![EffectKind::AddTileset]);
    }

    #[test]
    fn descriptor_cost_estimate_is_non_zero() {
        let verb = TileVerb::new();
        let cost = verb.descriptor().cost_estimate;
        assert!(cost.typical_latency > Duration::ZERO);
        assert!(cost.typical_usd_cents > 0.0);
    }

    // ── Validation ────────────────────────────────────────────────────────────

    #[test]
    fn validate_rejects_empty_examples() {
        let verb = TileVerb::new();
        let inputs = VerbInputs::from_struct(&TileInputs {
            examples: vec![],
            ..valid_inputs()
        })
        .unwrap();
        assert!(matches!(verb.validate(&inputs), Err(VerbError::Schema(_))));
    }

    #[test]
    fn validate_rejects_more_than_three_examples() {
        let verb = TileVerb::new();
        let inputs = VerbInputs::from_struct(&TileInputs {
            examples: vec![
                single_example(),
                single_example(),
                single_example(),
                single_example(),
            ],
            ..valid_inputs()
        })
        .unwrap();
        assert!(matches!(verb.validate(&inputs), Err(VerbError::Schema(_))));
    }

    #[test]
    fn validate_rejects_malformed_pixel_data() {
        let verb = TileVerb::new();
        let inputs = VerbInputs::from_struct(&TileInputs {
            examples: vec![PixelData {
                width: 4,
                height: 4,
                bytes_per_pixel: 4,
                stride: 16,
                bytes: vec![0; 8], // wrong: should be stride * height = 64
            }],
            ..valid_inputs()
        })
        .unwrap();
        assert!(matches!(verb.validate(&inputs), Err(VerbError::Schema(_))));
    }

    #[test]
    fn validate_rejects_non_rgba_examples() {
        let verb = TileVerb::new();
        let inputs = VerbInputs::from_struct(&TileInputs {
            examples: vec![PixelData {
                width: 4,
                height: 4,
                bytes_per_pixel: 3, // RGB, not RGBA
                stride: 12,
                bytes: vec![0; 48],
            }],
            ..valid_inputs()
        })
        .unwrap();
        assert!(matches!(verb.validate(&inputs), Err(VerbError::Schema(_))));
    }

    #[test]
    fn validate_rejects_zero_tile_size() {
        let verb = TileVerb::new();
        let inputs = VerbInputs::from_struct(&TileInputs {
            tile_width: 0,
            ..valid_inputs()
        })
        .unwrap();
        assert!(matches!(verb.validate(&inputs), Err(VerbError::Schema(_))));
    }

    #[test]
    fn validate_rejects_oversized_tile() {
        let verb = TileVerb::new();
        let inputs = VerbInputs::from_struct(&TileInputs {
            tile_width: 512,
            ..valid_inputs()
        })
        .unwrap();
        assert!(matches!(verb.validate(&inputs), Err(VerbError::Schema(_))));
    }

    #[test]
    fn validate_accepts_valid_inputs() {
        let verb = TileVerb::new();
        let inputs = VerbInputs::from_struct(&valid_inputs()).unwrap();
        assert!(verb.validate(&inputs).is_ok());
    }

    #[test]
    fn validate_accepts_three_examples() {
        let verb = TileVerb::new();
        let inputs = VerbInputs::from_struct(&TileInputs {
            examples: vec![single_example(), single_example(), single_example()],
            ..valid_inputs()
        })
        .unwrap();
        assert!(verb.validate(&inputs).is_ok());
    }

    // ── Round-trip through runtime ────────────────────────────────────────────

    #[tokio::test]
    async fn tile_verb_produces_add_tileset_effect() {
        let verb = TileVerb::new();

        let runtime = VerbRuntime::new();
        runtime
            .register_backend(BackendProxy::new(MockImageBackend), 0)
            .unwrap();
        runtime.register(verb).unwrap();

        let inputs = VerbInputs::from_struct(&valid_inputs()).unwrap();
        let inv = runtime
            .invoke(&VerbId::new(TILE_VERB_ID), ctx_with_sprite(), inputs)
            .unwrap();
        let preview = inv.finish().await.unwrap();

        assert_eq!(preview.verb.as_str(), TILE_VERB_ID);
        assert_eq!(preview.output.effects.len(), 1);

        match &preview.output.effects[0] {
            VerbEffect::AddTileset {
                tileset,
                pixel_buffers,
                ..
            } => {
                assert_eq!(tileset.name, "Test Tiles");
                assert_eq!(tileset.tile_count, ATLAS_TILE_COUNT);
                assert_eq!(tileset.tile_size, Size::new(16, 16));
                assert_eq!(tileset.base_index, 1);
                assert_eq!(pixel_buffers.len(), 1);
                let buf = &pixel_buffers[0].pixels;
                assert!(buf.is_well_formed());
                // Atlas must be exactly 8×6 tiles.
                assert_eq!(buf.width, 16 * ATLAS_COLS);
                assert_eq!(buf.height, 16 * ATLAS_ROWS);
            }
            other => panic!("expected AddTileset, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn tile_verb_requires_active_sprite() {
        let verb = TileVerb::new();

        let runtime = VerbRuntime::new();
        runtime
            .register_backend(BackendProxy::new(MockImageBackend), 0)
            .unwrap();
        runtime.register(verb).unwrap();

        let inputs = VerbInputs::from_struct(&valid_inputs()).unwrap();
        // No active sprite in the context.
        let inv = runtime
            .invoke(
                &VerbId::new(TILE_VERB_ID),
                VerbContext::empty(metadata()),
                inputs,
            )
            .unwrap();
        let err = inv.finish().await.unwrap_err();
        assert!(matches!(err, VerbError::MissingContext("active sprite")));
    }

    #[tokio::test]
    async fn tile_verb_fails_when_no_backend_available() {
        // No backend registered — the runtime's pre-flight rejects the
        // invocation before it ever reaches the verb body.
        let verb = TileVerb::new();
        let runtime = VerbRuntime::new();
        runtime.register(verb).unwrap();

        let inputs = VerbInputs::from_struct(&valid_inputs()).unwrap();
        let err = runtime
            .invoke(&VerbId::new(TILE_VERB_ID), ctx_with_sprite(), inputs)
            .unwrap_err();
        assert!(matches!(
            err,
            VerbError::UnsupportedCapability { .. } | VerbError::BackendUnavailable { .. }
        ));
    }

    #[tokio::test]
    async fn tile_verb_uses_default_tileset_name_when_none_provided() {
        let verb = TileVerb::new();

        let runtime = VerbRuntime::new();
        runtime
            .register_backend(BackendProxy::new(MockImageBackend), 0)
            .unwrap();
        runtime.register(verb).unwrap();

        let inputs = VerbInputs::from_struct(&TileInputs {
            tileset_name: None,
            ..valid_inputs()
        })
        .unwrap();
        let inv = runtime
            .invoke(&VerbId::new(TILE_VERB_ID), ctx_with_sprite(), inputs)
            .unwrap();
        let preview = inv.finish().await.unwrap();

        match &preview.output.effects[0] {
            VerbEffect::AddTileset { tileset, .. } => {
                assert_eq!(tileset.name, "Autotile");
            }
            other => panic!("expected AddTileset, got {other:?}"),
        }
    }

    // ── Image helpers ─────────────────────────────────────────────────────────

    #[test]
    fn extract_tile_returns_correct_region() {
        // Build a 32×32 atlas (2 cols × 2 rows, each tile 16×16).
        // Fill each quadrant with a distinct grey level.
        let mut pixels = vec![0u8; 32 * 32 * 4];
        for row in 0..32usize {
            for col in 0..32usize {
                let tile_col = col / 16;
                let tile_row = row / 16;
                #[allow(clippy::cast_possible_truncation)]
                let grey = ((tile_row * 2 + tile_col) as u8) * 60 + 40;
                let offset = (row * 32 + col) * 4;
                pixels[offset] = grey;
                pixels[offset + 1] = grey;
                pixels[offset + 2] = grey;
                pixels[offset + 3] = 255;
            }
        }
        let atlas = PixelData::rgba8(32, 32, pixels);

        // Tile index 0 (col=0, row=0) should be grey level 40.
        let t0 = extract_tile(&atlas, 0, 16, 16, 2).unwrap();
        assert_eq!(t0.width, 16);
        assert_eq!(t0.height, 16);
        assert_eq!(t0.bytes[0], 40); // R channel of first pixel

        // Tile index 1 (col=1, row=0) should be grey level 100.
        let t1 = extract_tile(&atlas, 1, 16, 16, 2).unwrap();
        assert_eq!(t1.bytes[0], 100);

        // Tile index 2 (col=0, row=1) should be grey level 160.
        let t2 = extract_tile(&atlas, 2, 16, 16, 2).unwrap();
        assert_eq!(t2.bytes[0], 160);
    }

    #[test]
    fn extract_tile_returns_none_for_out_of_bounds() {
        let atlas = PixelData::rgba8(16, 16, vec![0; 16 * 16 * 4]);
        // Tile index 1 in a 1-column atlas would start at y=16, which is
        // outside the 16-row atlas.
        let result = extract_tile(&atlas, 1, 16, 16, 1);
        assert!(result.is_none());
    }

    #[test]
    fn extract_tile_returns_none_when_tile_crosses_right_edge() {
        // Atlas width 24px, tile width 16px, 2 cols laid out logically.
        // Tile 0 fits (x=0..16). Tile 1 would start at x=16 and extend
        // to x=32 — past the 24px width. Without the pixel-grid bounds
        // check this would silently sample bytes from the next row;
        // assert that the explicit check rejects it.
        let atlas = PixelData::rgba8(24, 16, vec![0; 24 * 16 * 4]);
        assert!(extract_tile(&atlas, 1, 16, 16, 2).is_none());
    }

    #[test]
    fn decode_and_fit_atlas_resizes_to_expected_dims() {
        // Encode a 32×32 PNG, then ask for 64×48 — should resize.
        let png = solid_rgba_png(32, 32, [0, 0, 0, 255]);
        let atlas = decode_and_fit_atlas(&png, 64, 48).unwrap();
        assert_eq!(atlas.width, 64);
        assert_eq!(atlas.height, 48);
        assert!(atlas.is_well_formed());
    }

    #[test]
    fn encode_and_decode_pixel_data_round_trip() {
        let original = PixelData::rgba8(4, 4, (0u8..64).collect());
        let png = encode_pixel_data_as_png(&original).unwrap();
        let decoded = decode_and_fit_atlas(&png, 4, 4).unwrap();
        assert_eq!(decoded.bytes, original.bytes);
    }

    #[test]
    fn atlas_tile_count_constant_is_48() {
        assert_eq!(ATLAS_TILE_COUNT, 48);
        assert_eq!(ATLAS_COLS * ATLAS_ROWS, 48);
    }
}
