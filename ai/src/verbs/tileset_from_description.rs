//! Verb: generate a full autotile-compatible tileset from a description.
//!
//! The verb sends a structured prompt to an image-generation backend,
//! requests a tile sheet sized to the chosen autotile layout, decodes
//! the returned PNG, and wraps it as a [`VerbEffect::AddTileset`]. The
//! host assigns real IDs at commit time; this module uses placeholder
//! zeros throughout.
//!
//! # Autotile layouts
//!
//! Three layouts are supported (matching `core::tilemap::autotile`):
//!
//! | Variant  | Sheet    | Tiles |
//! |----------|----------|-------|
//! | Blob47   | 8 × 6    | 47 + empty(0) = 48 |
//! | Corner16 | 4 × 4    | 16    |
//! | Minimal4 | 4 × 1    | 4     |
//!
//! The prompt instructs the backend to produce a single sprite sheet
//! image whose cells map 1-to-1 to the tile slots in the chosen layout.

use std::time::Duration;

use async_trait::async_trait;
use pixhaus_core::project::{
    PixelBufferId, Size, TileShape, Tileset, TilesetId, TilesetSource, UserData,
};
use serde::{Deserialize, Serialize};
use tokio::select;
use tokio_util::sync::CancellationToken;

use crate::backends::{ImageGenRequest, InferenceRequest, InferenceResponse};
use crate::plugin::context::{PixelData, VerbContext};
use crate::plugin::descriptor::{
    BackendCapabilities, CostEstimate, EffectKind, VerbDescriptor, VerbId,
};
use crate::plugin::error::{Result, VerbError};
use crate::plugin::inputs::VerbInputs;
use crate::plugin::output::{ActualCost, NewPixelBuffer, VerbEffect, VerbOutput};
use crate::plugin::progress::{VerbProgress, VerbProgressEvent};
use crate::plugin::verb::Verb;

/// Stable ID for the tileset-from-description verb.
pub const TILESET_FROM_DESCRIPTION_VERB_ID: &str = "pixhaus.builtin.tileset_from_description";

const PLACEHOLDER_TILESET_ID: u32 = 0;
const PLACEHOLDER_BUFFER_ID: u32 = 0;

/// Which autotile layout to generate.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutotileKindInput {
    /// 47-tile blob layout (8×6 sheet). Handles all orthogonal
    /// neighbour combinations.
    #[default]
    Blob47,
    /// 16-tile corner layout (4×4 sheet). Lighter than Blob47;
    /// represents corner blending.
    Corner16,
    /// 4-tile minimal layout (4×1 sheet). One tile per edge neighbour
    /// count (0-3), for the simplest possible autotile.
    Minimal4,
}

impl AutotileKindInput {
    const fn cols(&self) -> u32 {
        match self {
            Self::Blob47 => 8,
            Self::Corner16 | Self::Minimal4 => 4,
        }
    }

    const fn rows(&self) -> u32 {
        match self {
            Self::Blob47 => 6,
            Self::Corner16 => 4,
            Self::Minimal4 => 1,
        }
    }

    /// Number of tile slots in the sheet, including the empty tile at
    /// index 0 for Blob47. `Corner16` and `Minimal4` do not reserve an
    /// empty slot — every cell in their grid is paintable — so callers
    /// pair this with [`base_index`](Self::base_index) when wiring up
    /// the [`pixhaus_core::project::Tileset`] record.
    pub const fn tile_count(&self) -> u32 {
        match self {
            Self::Blob47 => 48,
            Self::Corner16 => 16,
            Self::Minimal4 => 4,
        }
    }

    /// First paintable tile index in the sheet. Blob47 reserves
    /// `0` for the empty tile so paintable indices start at `1`;
    /// Corner16 and Minimal4 use every cell so their base is `0`.
    pub const fn base_index(&self) -> i16 {
        match self {
            Self::Blob47 => 1,
            Self::Corner16 | Self::Minimal4 => 0,
        }
    }

    fn layout_description(&self) -> &'static str {
        match self {
            Self::Blob47 => {
                "8 columns by 6 rows (48 tiles total, first tile is transparent empty tile, remaining 47 tiles cover all orthogonal neighbour combinations for blob autotiling)"
            }
            Self::Corner16 => {
                "4 columns by 4 rows (16 tiles covering all corner blend combinations)"
            }
            Self::Minimal4 => {
                "4 columns by 1 row (4 tiles: isolated, one-sided, two-sided, fully-connected)"
            }
        }
    }
}

/// Inputs for the tileset-from-description verb.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TilesetFromDescriptionInputs {
    /// Text description of the tileset's visual style and subject.
    pub description: String,
    /// Width and height of each individual tile in pixels.
    #[serde(default = "default_tile_size")]
    pub tile_size: u32,
    /// Autotile layout to generate.
    #[serde(default)]
    pub autotile_kind: AutotileKindInput,
    /// Display name for the resulting tileset. Defaults to a slug
    /// derived from the description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Optional style-reference image (raw PNG bytes) passed to the
    /// backend as a conditioning image.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style_reference: Option<Vec<u8>>,
}

fn default_tile_size() -> u32 {
    16
}

/// Generates a full autotile-compatible tileset from a text description.
///
/// Requires a backend with [`BackendCapabilities::IMAGE_GENERATION`].
/// The runtime selects the backend per invocation; the verb resolves it
/// through `VerbContext::backend`. On accept the host registers the tile
/// sheet as an inline pixel buffer and adds the tileset to the active
/// sprite.
pub struct TilesetFromDescriptionVerb {
    descriptor: VerbDescriptor,
}

impl std::fmt::Debug for TilesetFromDescriptionVerb {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TilesetFromDescriptionVerb")
            .field("id", &self.descriptor.id)
            .finish_non_exhaustive()
    }
}

impl TilesetFromDescriptionVerb {
    /// Constructs the verb. Backend resolution happens at invoke time
    /// from `VerbContext::backend`; the runtime injects the matching
    /// backend before dispatch.
    #[must_use]
    // serde_json::json! expands to internal helpers that call unwrap on
    // infallible value builders. Exempt the constructor to avoid fighting
    // the workspace-level unwrap_used lint.
    #[allow(clippy::disallowed_methods)]
    pub fn new() -> Self {
        let input_schema = serde_json::json!({
            "type": "object",
            "properties": {
                "description": {
                    "type": "string",
                    "minLength": 1,
                    "description": "Text description of the tileset's visual style and subject"
                },
                "tile_size": {
                    "type": "integer",
                    "minimum": 4,
                    "maximum": 128,
                    "default": 16,
                    "description": "Width and height of each tile in pixels"
                },
                "autotile_kind": {
                    "type": "string",
                    "enum": ["blob47", "corner16", "minimal4"],
                    "default": "blob47",
                    "description": "Autotile layout to generate"
                },
                "name": {
                    "type": ["string", "null"],
                    "description": "Display name for the resulting tileset"
                },
                "style_reference": {
                    "type": ["array", "null"],
                    "items": {"type": "integer"},
                    "description": "Optional PNG bytes for style conditioning"
                }
            },
            "required": ["description"]
        });

        Self {
            descriptor: VerbDescriptor {
                id: VerbId::new(TILESET_FROM_DESCRIPTION_VERB_ID),
                display_name: "Tileset from description".into(),
                description: "Generate a full autotile-compatible tileset from a text description using an image-generation backend".into(),
                version: env!("CARGO_PKG_VERSION").into(),
                required_capabilities: BackendCapabilities::IMAGE_GENERATION,
                input_schema,
                output_schema: None,
                output_kinds: vec![EffectKind::AddTileset],
                cost_estimate: CostEstimate {
                    typical_latency: Duration::from_secs(15),
                    max_latency: Duration::from_secs(120),
                    typical_usd_cents: 2.0,
                    max_usd_cents: 10.0,
                },
                streaming: true,
                cancellable: true,
                documentation_url: None,
            },
        }
    }
}

impl Default for TilesetFromDescriptionVerb {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Verb for TilesetFromDescriptionVerb {
    fn descriptor(&self) -> &VerbDescriptor {
        &self.descriptor
    }

    fn validate(&self, inputs: &VerbInputs) -> Result<()> {
        let parsed: TilesetFromDescriptionInputs = inputs.deserialize()?;
        if parsed.description.trim().is_empty() {
            return Err(VerbError::Schema(
                "tileset_from_description: description must not be empty".into(),
            ));
        }
        if parsed.tile_size < 4 || parsed.tile_size > 128 {
            return Err(VerbError::Schema(format!(
                "tileset_from_description: tile_size {} is out of range 4..=128",
                parsed.tile_size
            )));
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
        let started = std::time::Instant::now();
        let inputs: TilesetFromDescriptionInputs = inputs.deserialize_owned()?;
        let sprite_id = ctx.require_sprite_id()?;

        let backend = crate::verbs::ctx_fat_backend(&ctx)?;

        progress
            .send(VerbProgressEvent::Started {
                backend: Some(backend.backend_id().into()),
            })
            .await;

        let kind = &inputs.autotile_kind;
        let sheet_w = inputs.tile_size * kind.cols();
        let sheet_h = inputs.tile_size * kind.rows();
        let prompt = build_prompt(&inputs.description, kind, inputs.tile_size);

        progress
            .step(Some(0.1), "requesting tile sheet from backend")
            .await;

        if cancel.is_cancelled() {
            return Err(VerbError::Cancelled);
        }

        // Style image precedence: explicit user input first, then the
        // entity's anchor sheet (B10.3). When neither is present the
        // backend generates from the prompt alone.
        let style_image = inputs
            .style_reference
            .clone()
            .or_else(|| crate::verbs::anchor_style_image_bytes(&ctx));

        let req = InferenceRequest::ImageGeneration(ImageGenRequest {
            model: None,
            prompt,
            negative_prompt: Some(
                "text, labels, borders, shadows, 3d, photorealistic, blurry".into(),
            ),
            width: sheet_w,
            height: sheet_h,
            steps: None,
            seed: None,
            num_images: 1,
            quality: None,
            style_image,
            reference_images: Vec::new(),
        });

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

        progress.step(Some(0.7), "decoding tile sheet").await;

        if cancel.is_cancelled() {
            return Err(VerbError::Cancelled);
        }

        let sheet_pixels = decode_png_to_rgba8(&png_bytes, sheet_w, sheet_h)?;

        let tileset_name = inputs
            .name
            .unwrap_or_else(|| truncate_description(&inputs.description, 32));

        let tileset = Tileset {
            id: TilesetId::new(PLACEHOLDER_TILESET_ID),
            name: tileset_name.clone(),
            tile_size: Size::new(inputs.tile_size, inputs.tile_size),
            shape: TileShape::Square,
            hex_offset: None,
            tile_count: kind.tile_count(),
            base_index: kind.base_index(),
            source: TilesetSource::Inline {
                buffer: PixelBufferId::new(PLACEHOLDER_BUFFER_ID),
            },
            properties: Vec::new(),
            autotile: None,
            user_data: UserData::default(),
        };

        let pixel_buffer = NewPixelBuffer {
            placeholder: PixelBufferId::new(PLACEHOLDER_BUFFER_ID),
            pixels: sheet_pixels.clone(),
        };

        progress.step(Some(1.0), "tileset ready").await;

        let elapsed = started.elapsed();
        let summary = format!(
            "Add tileset \"{tileset_name}\" ({} tiles, {}×{}px each)",
            kind.tile_count(),
            inputs.tile_size,
            inputs.tile_size,
        );

        Ok(VerbOutput {
            summary,
            effects: vec![VerbEffect::AddTileset {
                sprite: sprite_id,
                tileset,
                pixel_buffers: vec![pixel_buffer],
            }],
            thumbnail: Some(sheet_pixels),
            actual_cost: ActualCost::free(elapsed.max(Duration::from_micros(1))),
            notes: vec![],
        })
    }
}

fn build_prompt(description: &str, kind: &AutotileKindInput, tile_size: u32) -> String {
    format!(
        "Pixel art tileset sprite sheet for: {description}. \
        Layout: {layout}. \
        Each tile is {tile_size}x{tile_size} pixels. \
        Style: clean pixel art, consistent lighting, seamless edges where tiles connect, \
        transparent background for empty tile. \
        No grid lines, no borders between tiles.",
        description = description,
        layout = kind.layout_description(),
        tile_size = tile_size,
    )
}

fn truncate_description(desc: &str, max_len: usize) -> String {
    let trimmed = desc.trim();
    if trimmed.len() <= max_len {
        return trimmed.to_owned();
    }
    // Find the last UTF-8 character boundary at or before max_len.
    let safe_cut = trimmed
        .char_indices()
        .take_while(|(i, _)| *i < max_len)
        .last()
        .map_or(max_len, |(i, c)| i + c.len_utf8());
    let prefix = &trimmed[..safe_cut];
    // Prefer the last whitespace inside the prefix so we cut on a word
    // boundary rather than mid-word. Fall back to the safe-char cut
    // when the prefix contains no whitespace (e.g. one very long token).
    let cut = prefix.rfind(char::is_whitespace).unwrap_or(prefix.len());
    let mut result = trimmed[..cut].trim_end().to_owned();
    result.push_str("...");
    result
}

fn decode_png_to_rgba8(png_bytes: &[u8], expected_w: u32, expected_h: u32) -> Result<PixelData> {
    let img = image::load_from_memory(png_bytes)
        .map_err(|e| VerbError::Backend(format!("PNG decode failed: {e}")))?;
    let rgba = img.into_rgba8();
    let (w, h) = rgba.dimensions();
    if w != expected_w || h != expected_h {
        return Err(VerbError::Backend(format!(
            "backend returned {w}×{h} image; expected {expected_w}×{expected_h}"
        )));
    }
    Ok(PixelData::rgba8(w, h, rgba.into_raw()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backends::bridge::BackendProxy;
    use crate::backends::{BackendError, InferenceRequest};
    use crate::plugin::context::VerbContext;
    use crate::plugin::descriptor::{BackendCapabilities, CostEstimate};
    use crate::plugin::error::VerbError;
    use crate::plugin::output::VerbEffect;
    use crate::plugin::progress::VerbProgress;
    use crate::plugin::runtime::VerbRuntime;
    use crate::plugin::{VerbId, VerbInputs};
    use pixhaus_core::project::{ProjectMetadata, SpriteId};
    use std::sync::Arc;
    use tokio_util::sync::CancellationToken;

    // ── Test helpers ──────────────────────────────────────────────────────

    fn metadata() -> ProjectMetadata {
        ProjectMetadata {
            name: "s35-test".into(),
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
        ctx.backend = Some(Arc::new(BackendProxy::new(MockImageBackend)));
        ctx
    }

    fn ctx_with_sprite_no_backend() -> VerbContext {
        let mut ctx = VerbContext::empty(metadata());
        ctx.active_sprite = Some(SpriteId::new(1));
        ctx
    }

    /// Returns a minimal valid RGBA PNG of the requested dimensions.
    fn make_test_png(width: u32, height: u32) -> Vec<u8> {
        let img = image::RgbaImage::new(width, height);
        let mut buf = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut buf, image::ImageFormat::Png)
            .unwrap();
        buf.into_inner()
    }

    /// Fake image-generation backend that returns a solid PNG of the
    /// requested dimensions.
    struct MockImageBackend;

    #[async_trait::async_trait]
    impl crate::backends::InferenceBackend for MockImageBackend {
        fn backend_id(&self) -> &'static str {
            "mock-image"
        }
        fn capabilities(&self) -> BackendCapabilities {
            BackendCapabilities::IMAGE_GENERATION
        }
        fn supports_streaming(&self) -> bool {
            false
        }
        fn estimate_cost(&self, _req: &InferenceRequest) -> CostEstimate {
            CostEstimate::free()
        }
        async fn invoke(
            &self,
            req: InferenceRequest,
            _progress: VerbProgress,
            _cancel: CancellationToken,
        ) -> crate::backends::Result<InferenceResponse> {
            match req {
                InferenceRequest::ImageGeneration(r) => {
                    let png = make_test_png(r.width, r.height);
                    Ok(InferenceResponse::Image(
                        crate::backends::ImageGenResponse {
                            images: vec![png],
                            model: "mock".into(),
                        },
                    ))
                }
                _ => Err(BackendError::UnsupportedCapability),
            }
        }
    }

    fn verb_with_mock() -> TilesetFromDescriptionVerb {
        TilesetFromDescriptionVerb::new()
    }

    fn inputs(kind: AutotileKindInput) -> VerbInputs {
        VerbInputs::from_struct(&TilesetFromDescriptionInputs {
            description: "stone dungeon floor".into(),
            tile_size: 16,
            autotile_kind: kind,
            name: None,
            style_reference: None,
        })
        .unwrap()
    }

    // ── Descriptor ────────────────────────────────────────────────────────

    #[test]
    fn descriptor_advertises_image_generation_capability() {
        let verb = verb_with_mock();
        assert!(
            verb.descriptor()
                .required_capabilities
                .contains(BackendCapabilities::IMAGE_GENERATION)
        );
        assert!(verb.descriptor().cancellable);
        assert!(verb.descriptor().streaming);
    }

    #[test]
    fn descriptor_output_kind_is_add_tileset() {
        let verb = verb_with_mock();
        assert!(
            verb.descriptor()
                .output_kinds
                .contains(&EffectKind::AddTileset)
        );
    }

    // ── Validation ────────────────────────────────────────────────────────

    #[test]
    fn validate_rejects_empty_description() {
        let verb = verb_with_mock();
        let bad = VerbInputs::from_struct(&TilesetFromDescriptionInputs {
            description: "   ".into(),
            tile_size: 16,
            autotile_kind: AutotileKindInput::Blob47,
            name: None,
            style_reference: None,
        })
        .unwrap();
        assert!(verb.validate(&bad).is_err());
    }

    #[test]
    fn validate_rejects_tile_size_too_small() {
        let verb = verb_with_mock();
        let bad = VerbInputs::from_struct(&TilesetFromDescriptionInputs {
            description: "grass".into(),
            tile_size: 2,
            autotile_kind: AutotileKindInput::Blob47,
            name: None,
            style_reference: None,
        })
        .unwrap();
        assert!(verb.validate(&bad).is_err());
    }

    #[test]
    fn validate_rejects_tile_size_too_large() {
        let verb = verb_with_mock();
        let bad = VerbInputs::from_struct(&TilesetFromDescriptionInputs {
            description: "grass".into(),
            tile_size: 256,
            autotile_kind: AutotileKindInput::Blob47,
            name: None,
            style_reference: None,
        })
        .unwrap();
        assert!(verb.validate(&bad).is_err());
    }

    #[test]
    fn validate_accepts_valid_inputs() {
        let verb = verb_with_mock();
        assert!(verb.validate(&inputs(AutotileKindInput::Blob47)).is_ok());
    }

    // ── Invoke ────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn invoke_blob47_produces_tileset_effect() {
        let verb = verb_with_mock();
        let out = verb
            .invoke(
                ctx_with_sprite(),
                inputs(AutotileKindInput::Blob47),
                VerbProgress::discard(),
                CancellationToken::new(),
            )
            .await
            .unwrap();

        assert_eq!(out.effects.len(), 1);
        match &out.effects[0] {
            VerbEffect::AddTileset {
                tileset,
                pixel_buffers,
                ..
            } => {
                assert_eq!(tileset.tile_count, 48);
                // Blob47 reserves cell 0 for the empty tile, so paintable
                // indices start at 1.
                assert_eq!(tileset.base_index, 1);
                assert_eq!(tileset.tile_size, Size::new(16, 16));
                assert_eq!(pixel_buffers.len(), 1);
                let pix = &pixel_buffers[0].pixels;
                // Blob47: 8 cols × 6 rows × 16px = 128 × 96
                assert_eq!(pix.width, 128);
                assert_eq!(pix.height, 96);
            }
            other => panic!("unexpected effect: {other:?}"),
        }
    }

    #[tokio::test]
    async fn invoke_corner16_produces_correct_tile_count() {
        let verb = verb_with_mock();
        let out = verb
            .invoke(
                ctx_with_sprite(),
                inputs(AutotileKindInput::Corner16),
                VerbProgress::discard(),
                CancellationToken::new(),
            )
            .await
            .unwrap();

        match &out.effects[0] {
            VerbEffect::AddTileset { tileset, .. } => {
                assert_eq!(tileset.tile_count, 16);
                // Corner16 has no empty slot — every cell is paintable —
                // so base_index is 0, not 1.
                assert_eq!(tileset.base_index, 0);
                // Corner16: 4 cols × 4 rows × 16px = 64 × 64
                assert_eq!(tileset.tile_size, Size::new(16, 16));
            }
            other => panic!("unexpected effect: {other:?}"),
        }
    }

    #[tokio::test]
    async fn invoke_minimal4_produces_correct_tile_count() {
        let verb = verb_with_mock();
        let out = verb
            .invoke(
                ctx_with_sprite(),
                inputs(AutotileKindInput::Minimal4),
                VerbProgress::discard(),
                CancellationToken::new(),
            )
            .await
            .unwrap();

        match &out.effects[0] {
            VerbEffect::AddTileset {
                tileset,
                pixel_buffers,
                ..
            } => {
                assert_eq!(tileset.tile_count, 4);
                // Minimal4 has no empty slot — paintable indices start at 0.
                assert_eq!(tileset.base_index, 0);
                // Minimal4: 4 cols × 1 row × 16px = 64 × 16
                let pix = &pixel_buffers[0].pixels;
                assert_eq!(pix.width, 64);
                assert_eq!(pix.height, 16);
            }
            other => panic!("unexpected effect: {other:?}"),
        }
    }

    #[tokio::test]
    async fn invoke_uses_custom_name_when_provided() {
        let verb = verb_with_mock();
        let named = VerbInputs::from_struct(&TilesetFromDescriptionInputs {
            description: "lava tiles".into(),
            tile_size: 16,
            autotile_kind: AutotileKindInput::Minimal4,
            name: Some("My Lava Tileset".into()),
            style_reference: None,
        })
        .unwrap();

        let out = verb
            .invoke(
                ctx_with_sprite(),
                named,
                VerbProgress::discard(),
                CancellationToken::new(),
            )
            .await
            .unwrap();

        match &out.effects[0] {
            VerbEffect::AddTileset { tileset, .. } => {
                assert_eq!(tileset.name, "My Lava Tileset");
            }
            other => panic!("unexpected effect: {other:?}"),
        }
    }

    #[tokio::test]
    async fn invoke_requires_active_sprite() {
        let verb = verb_with_mock();
        let res = verb
            .invoke(
                VerbContext::empty(metadata()),
                inputs(AutotileKindInput::Blob47),
                VerbProgress::discard(),
                CancellationToken::new(),
            )
            .await;
        assert!(matches!(
            res,
            Err(VerbError::MissingContext("active sprite"))
        ));
    }

    #[tokio::test]
    async fn invoke_cancels_when_token_fires() {
        let verb = verb_with_mock();
        let token = CancellationToken::new();
        token.cancel();
        let res = verb
            .invoke(
                ctx_with_sprite(),
                inputs(AutotileKindInput::Blob47),
                VerbProgress::discard(),
                token,
            )
            .await;
        assert!(matches!(res, Err(VerbError::Cancelled)));
    }

    #[tokio::test]
    async fn invoke_fails_without_image_backend() {
        // No backend attached to the context — ctx_fat_backend rejects.
        // (Through the runtime, the same scenario surfaces as
        // VerbError::UnsupportedCapability before invoke is ever called;
        // this direct path tests the verb's own guard.)
        let verb = TilesetFromDescriptionVerb::new();
        let res = verb
            .invoke(
                ctx_with_sprite_no_backend(),
                inputs(AutotileKindInput::Blob47),
                VerbProgress::discard(),
                CancellationToken::new(),
            )
            .await;
        assert!(matches!(res, Err(VerbError::Backend(_))));
    }

    // ── Runtime integration ───────────────────────────────────────────────

    #[tokio::test]
    async fn verb_round_trips_through_runtime() {
        let runtime = VerbRuntime::new();
        runtime
            .register_backend(BackendProxy::new(MockImageBackend), 0)
            .unwrap();
        runtime.register(TilesetFromDescriptionVerb::new()).unwrap();

        // Direct-invoke tests preset ctx.backend; the runtime path injects
        // it from the registered backend, so use a context without one.
        let inv = runtime
            .invoke(
                &VerbId::new(TILESET_FROM_DESCRIPTION_VERB_ID),
                ctx_with_sprite_no_backend(),
                inputs(AutotileKindInput::Blob47),
            )
            .unwrap();
        let preview = inv.finish().await.unwrap();
        assert_eq!(preview.output.effects.len(), 1);
        assert!(matches!(
            &preview.output.effects[0],
            VerbEffect::AddTileset { .. }
        ));
    }

    // ── Helpers ───────────────────────────────────────────────────────────

    #[test]
    fn truncate_description_short_passthrough() {
        assert_eq!(truncate_description("grass", 32), "grass");
    }

    #[test]
    fn truncate_description_long_appends_ellipsis() {
        let result = truncate_description("a very long description that exceeds the limit", 20);
        assert!(result.len() <= 23); // 20 chars + "..."
        assert!(result.ends_with("..."));
    }

    #[test]
    fn decode_png_rejects_wrong_dimensions() {
        let png = make_test_png(32, 32);
        let res = decode_png_to_rgba8(&png, 64, 64);
        assert!(res.is_err());
    }

    #[test]
    fn decode_png_accepts_correct_dimensions() {
        let png = make_test_png(64, 96);
        let pix = decode_png_to_rgba8(&png, 64, 96).unwrap();
        assert_eq!(pix.width, 64);
        assert_eq!(pix.height, 96);
        assert_eq!(pix.bytes.len(), 64 * 96 * 4);
    }

    #[test]
    fn autotile_kind_sheet_dimensions() {
        assert_eq!(
            AutotileKindInput::Blob47.cols() * AutotileKindInput::Blob47.rows(),
            48
        );
        assert_eq!(
            AutotileKindInput::Corner16.cols() * AutotileKindInput::Corner16.rows(),
            16
        );
        assert_eq!(
            AutotileKindInput::Minimal4.cols() * AutotileKindInput::Minimal4.rows(),
            4
        );
    }
}
