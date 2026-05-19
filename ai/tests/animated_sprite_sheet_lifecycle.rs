//! End-to-end lifecycle tests for [`AnimatedSpriteSheetVerb`] (S53).
//!
//! Spins up a [`VerbRuntime`], registers a mock backend that advertises
//! `IMAGE_GENERATION` (and optionally `TEXT_GENERATION`), and verifies:
//!
//! - Procedural mode emits `grid_size²` empty frames offline.
//! - AI mode calls the image backend, slices the returned PNG into
//!   `grid_size²` cells in row-major order, and packs them into a new
//!   layer.
//! - AI mode without `TEXT_GENERATION` capability falls back to the
//!   user's prompt directly and surfaces a note about the skipped
//!   rewrite stage.
//! - AI mode with `TEXT_GENERATION` capability calls the rewrite stage
//!   first and includes the rewritten text in `VerbOutput::notes`.
//! - Pre-supplied `rewritten_prompt` bypasses the rewrite stage.
//! - Cancellation aborts mid-flight.
//! - The runtime refuses to invoke when no `IMAGE_GENERATION` backend is
//!   registered.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::disallowed_methods
)]

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use pixhaus_ai::backends::{
    BackendError, BackendProxy, ImageGenResponse, InferenceBackend, InferenceRequest,
    InferenceResponse, Result as BackendResult, TextGenResponse,
};
use pixhaus_ai::plugin::{
    ANIMATED_SPRITE_SHEET_VERB_ID, AnimatedSpriteSheetInputs, AnimatedSpriteSheetVerb,
    BackendCapabilities, CostEstimate, GenerationMode, VerbContext, VerbEffect, VerbId, VerbInputs,
    VerbProgress, VerbRuntime,
};
use pixhaus_core::project::{LayerId, Palette, PaletteId, ProjectMetadata, Rgba, SpriteId};
use tokio_util::sync::CancellationToken;

// ── Mock backend ───────────────────────────────────────────────────────────

/// Backend that returns a g×g checkerboard PNG for every image
/// generation request, optionally a canned rewrite for text requests.
#[derive(Debug)]
struct MockSheetBackend {
    capabilities: BackendCapabilities,
    image_calls: Arc<AtomicUsize>,
    text_calls: Arc<AtomicUsize>,
    canned_rewrite: String,
}

impl MockSheetBackend {
    fn image_only() -> Self {
        Self {
            capabilities: BackendCapabilities::IMAGE_GENERATION,
            image_calls: Arc::new(AtomicUsize::new(0)),
            text_calls: Arc::new(AtomicUsize::new(0)),
            canned_rewrite: String::new(),
        }
    }

    fn image_and_text() -> Self {
        Self {
            capabilities: BackendCapabilities::IMAGE_GENERATION
                .union(BackendCapabilities::TEXT_GENERATION),
            image_calls: Arc::new(AtomicUsize::new(0)),
            text_calls: Arc::new(AtomicUsize::new(0)),
            canned_rewrite: "CHARACTER: a tiny dragon with iridescent scales.\n\
                             CHOREOGRAPHY: idle breath, eyes blink, head turns."
                .to_string(),
        }
    }
}

#[async_trait]
impl InferenceBackend for MockSheetBackend {
    fn backend_id(&self) -> &'static str {
        "mock.animated_sheet"
    }

    fn capabilities(&self) -> BackendCapabilities {
        self.capabilities
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
    ) -> BackendResult<InferenceResponse> {
        match request {
            InferenceRequest::ImageGeneration(r) => {
                self.image_calls.fetch_add(1, Ordering::SeqCst);
                let png = checkerboard_png(r.width, r.height);
                Ok(InferenceResponse::Image(ImageGenResponse {
                    images: vec![png],
                    model: "mock.sheet".to_owned(),
                }))
            }
            InferenceRequest::Text(_) => {
                self.text_calls.fetch_add(1, Ordering::SeqCst);
                Ok(InferenceResponse::Text(TextGenResponse {
                    content: self.canned_rewrite.clone(),
                    model: "mock.rewrite".to_owned(),
                    input_tokens: 50,
                    output_tokens: 50,
                    stop_reason: Some("end_turn".to_owned()),
                    tool_calls: Vec::new(),
                }))
            }
            _ => Err(BackendError::UnsupportedCapability),
        }
    }
}

fn checkerboard_png(width: u32, height: u32) -> Vec<u8> {
    let mut img = image::RgbaImage::new(width, height);
    for (x, y, pixel) in img.enumerate_pixels_mut() {
        let on = ((x / 8) + (y / 8)) % 2 == 0;
        let v: u8 = if on { 220 } else { 40 };
        *pixel = image::Rgba([v, v, v, 255]);
    }
    let mut buf = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(img)
        .write_to(&mut buf, image::ImageFormat::Png)
        .unwrap();
    buf.into_inner()
}

fn metadata() -> ProjectMetadata {
    ProjectMetadata {
        name: "animated-sprite-sheet-test".into(),
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
    ctx.active_layer = Some(LayerId::new(42));
    ctx
}

fn ai_inputs(grid: u8) -> AnimatedSpriteSheetInputs {
    let mut p = AnimatedSpriteSheetInputs::new("a baby dragon, pastel pixel art");
    p.grid_size = grid;
    p.cell_size_px = 32;
    p.mode = GenerationMode::Ai;
    p
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn procedural_mode_runs_without_backend() {
    let runtime = VerbRuntime::new();
    runtime.register(AnimatedSpriteSheetVerb::new()).unwrap();
    // No backend registered — procedural mode should still run because
    // the runtime only blocks invocation if `required_capabilities` is
    // non-empty AND no backend matches. For a procedural-only path we
    // need a backend nominally registered. Register an image-gen
    // backend and verify it isn't called.
    let mock = MockSheetBackend::image_only();
    let image_calls = mock.image_calls.clone();
    runtime
        .register_backend(BackendProxy::new(mock), 0)
        .unwrap();

    let mut p = ai_inputs(3);
    p.mode = GenerationMode::Procedural;
    let inputs = VerbInputs::from_struct(&p).unwrap();
    let inv = runtime
        .invoke(
            &VerbId::new(ANIMATED_SPRITE_SHEET_VERB_ID),
            ctx_with_sprite(),
            inputs,
        )
        .unwrap();
    let preview = inv.finish().await.unwrap();

    assert_eq!(image_calls.load(Ordering::SeqCst), 0);
    match &preview.output.effects[0] {
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
async fn ai_mode_slices_image_into_grid_squared_cels() {
    let runtime = VerbRuntime::new();
    runtime.register(AnimatedSpriteSheetVerb::new()).unwrap();
    let mock = MockSheetBackend::image_only();
    let image_calls = mock.image_calls.clone();
    runtime
        .register_backend(BackendProxy::new(mock), 0)
        .unwrap();

    let inputs = VerbInputs::from_struct(&ai_inputs(3)).unwrap();
    let inv = runtime
        .invoke(
            &VerbId::new(ANIMATED_SPRITE_SHEET_VERB_ID),
            ctx_with_sprite(),
            inputs,
        )
        .unwrap();
    let preview = inv.finish().await.unwrap();

    assert_eq!(image_calls.load(Ordering::SeqCst), 1);
    match &preview.output.effects[0] {
        VerbEffect::AddLayer {
            cels,
            pixel_buffers,
            layer,
            ..
        } => {
            assert_eq!(cels.len(), 9, "3×3 grid should produce 9 cels");
            assert_eq!(pixel_buffers.len(), 9);
            for buf in pixel_buffers {
                assert_eq!(buf.pixels.width, 32);
                assert_eq!(buf.pixels.height, 32);
            }
            assert!(layer.name.contains("Animated sheet"));
        }
        other => panic!("expected AddLayer, got {other:?}"),
    }
}

#[tokio::test]
async fn ai_mode_with_text_backend_runs_rewrite_first() {
    let runtime = VerbRuntime::new();
    runtime.register(AnimatedSpriteSheetVerb::new()).unwrap();
    let mock = MockSheetBackend::image_and_text();
    let text_calls = mock.text_calls.clone();
    let image_calls = mock.image_calls.clone();
    runtime
        .register_backend(BackendProxy::new(mock), 0)
        .unwrap();

    let inputs = VerbInputs::from_struct(&ai_inputs(2)).unwrap();
    let inv = runtime
        .invoke(
            &VerbId::new(ANIMATED_SPRITE_SHEET_VERB_ID),
            ctx_with_sprite(),
            inputs,
        )
        .unwrap();
    let preview = inv.finish().await.unwrap();

    assert_eq!(text_calls.load(Ordering::SeqCst), 1, "rewrite must run");
    assert_eq!(image_calls.load(Ordering::SeqCst), 1, "sheet must render");

    let notes_joined = preview.output.notes.join(" ");
    assert!(
        notes_joined.contains("CHARACTER"),
        "rewrite text should appear in notes: {notes_joined}",
    );
    assert!(notes_joined.contains("CHOREOGRAPHY"));
}

#[tokio::test]
async fn ai_mode_without_text_capability_skips_rewrite_with_note() {
    let runtime = VerbRuntime::new();
    runtime.register(AnimatedSpriteSheetVerb::new()).unwrap();
    let mock = MockSheetBackend::image_only();
    let text_calls = mock.text_calls.clone();
    runtime
        .register_backend(BackendProxy::new(mock), 0)
        .unwrap();

    let inputs = VerbInputs::from_struct(&ai_inputs(2)).unwrap();
    let inv = runtime
        .invoke(
            &VerbId::new(ANIMATED_SPRITE_SHEET_VERB_ID),
            ctx_with_sprite(),
            inputs,
        )
        .unwrap();
    let preview = inv.finish().await.unwrap();

    assert_eq!(
        text_calls.load(Ordering::SeqCst),
        0,
        "no TEXT_GENERATION capability means no rewrite call",
    );
    let notes_joined = preview.output.notes.join(" ");
    assert!(
        notes_joined.contains("Rewrite skipped"),
        "skip reason should be surfaced: {notes_joined}",
    );
}

#[tokio::test]
async fn ai_mode_with_user_rewrite_bypasses_text_stage() {
    let runtime = VerbRuntime::new();
    runtime.register(AnimatedSpriteSheetVerb::new()).unwrap();
    let mock = MockSheetBackend::image_and_text();
    let text_calls = mock.text_calls.clone();
    runtime
        .register_backend(BackendProxy::new(mock), 0)
        .unwrap();

    let mut p = ai_inputs(2);
    p.rewritten_prompt = Some("CHARACTER: pre-edited.\nCHOREOGRAPHY: pre-edited.".into());
    let inputs = VerbInputs::from_struct(&p).unwrap();
    let inv = runtime
        .invoke(
            &VerbId::new(ANIMATED_SPRITE_SHEET_VERB_ID),
            ctx_with_sprite(),
            inputs,
        )
        .unwrap();
    let _preview = inv.finish().await.unwrap();

    assert_eq!(
        text_calls.load(Ordering::SeqCst),
        0,
        "user-supplied rewrite must bypass the LLM stage",
    );
}

#[tokio::test]
async fn ai_mode_snaps_output_to_active_palette() {
    let runtime = VerbRuntime::new();
    runtime.register(AnimatedSpriteSheetVerb::new()).unwrap();
    runtime
        .register_backend(BackendProxy::new(MockSheetBackend::image_only()), 0)
        .unwrap();

    let mut ctx = ctx_with_sprite();
    ctx.active_palette = Some(Palette::from_colors(
        PaletteId::new(1),
        "test",
        vec![Rgba::new(0, 0, 0, 255), Rgba::new(255, 255, 255, 255)],
    ));

    let inputs = VerbInputs::from_struct(&ai_inputs(2)).unwrap();
    let inv = runtime
        .invoke(&VerbId::new(ANIMATED_SPRITE_SHEET_VERB_ID), ctx, inputs)
        .unwrap();
    let preview = inv.finish().await.unwrap();

    match &preview.output.effects[0] {
        VerbEffect::AddLayer { pixel_buffers, .. } => {
            for buf in pixel_buffers {
                for chunk in buf.pixels.bytes.chunks_exact(4) {
                    // Only two palette colors — black or white — at full alpha.
                    let rgb = (chunk[0], chunk[1], chunk[2]);
                    assert!(
                        rgb == (0, 0, 0) || rgb == (255, 255, 255),
                        "non-palette colour {rgb:?} leaked through snap",
                    );
                    assert_eq!(chunk[3], 255);
                }
            }
        }
        other => panic!("expected AddLayer, got {other:?}"),
    }
}

#[tokio::test]
async fn ai_mode_without_palette_surfaces_note() {
    let runtime = VerbRuntime::new();
    runtime.register(AnimatedSpriteSheetVerb::new()).unwrap();
    runtime
        .register_backend(BackendProxy::new(MockSheetBackend::image_only()), 0)
        .unwrap();

    let inputs = VerbInputs::from_struct(&ai_inputs(2)).unwrap();
    let inv = runtime
        .invoke(
            &VerbId::new(ANIMATED_SPRITE_SHEET_VERB_ID),
            ctx_with_sprite(),
            inputs,
        )
        .unwrap();
    let preview = inv.finish().await.unwrap();
    assert!(
        preview.output.notes.iter().any(|n| n.contains("palette")),
        "missing palette note: {:?}",
        preview.output.notes,
    );
}

#[tokio::test]
async fn hybrid_mode_emits_placeholder_grid_and_rewrite() {
    let runtime = VerbRuntime::new();
    runtime.register(AnimatedSpriteSheetVerb::new()).unwrap();
    let mock = MockSheetBackend::image_and_text();
    let image_calls = mock.image_calls.clone();
    let text_calls = mock.text_calls.clone();
    runtime
        .register_backend(BackendProxy::new(mock), 0)
        .unwrap();

    let mut p = ai_inputs(2);
    p.mode = GenerationMode::Hybrid;
    let inputs = VerbInputs::from_struct(&p).unwrap();
    let inv = runtime
        .invoke(
            &VerbId::new(ANIMATED_SPRITE_SHEET_VERB_ID),
            ctx_with_sprite(),
            inputs,
        )
        .unwrap();
    let preview = inv.finish().await.unwrap();

    assert_eq!(text_calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        image_calls.load(Ordering::SeqCst),
        0,
        "hybrid does not call the image backend",
    );

    match &preview.output.effects[0] {
        VerbEffect::AddLayer { pixel_buffers, .. } => {
            assert_eq!(pixel_buffers.len(), 4);
            for buf in pixel_buffers {
                // Empty placeholder — all bytes zero.
                assert!(buf.pixels.bytes.iter().all(|&b| b == 0));
            }
        }
        other => panic!("expected AddLayer, got {other:?}"),
    }

    let notes = preview.output.notes.join(" ");
    assert!(notes.contains("CHARACTER"), "rewrite missing: {notes}");
}

#[tokio::test]
async fn runtime_refuses_to_invoke_without_image_backend() {
    let runtime = VerbRuntime::new();
    runtime.register(AnimatedSpriteSheetVerb::new()).unwrap();

    let inputs = VerbInputs::from_struct(&ai_inputs(2)).unwrap();
    let res = runtime.invoke(
        &VerbId::new(ANIMATED_SPRITE_SHEET_VERB_ID),
        ctx_with_sprite(),
        inputs,
    );
    assert!(
        res.is_err(),
        "must refuse: no IMAGE_GENERATION backend registered",
    );
}

#[tokio::test]
async fn cancel_before_finish_returns_error() {
    let runtime = VerbRuntime::new();
    runtime.register(AnimatedSpriteSheetVerb::new()).unwrap();
    runtime
        .register_backend(BackendProxy::new(MockSheetBackend::image_only()), 0)
        .unwrap();

    let inputs = VerbInputs::from_struct(&ai_inputs(2)).unwrap();
    let inv = runtime
        .invoke(
            &VerbId::new(ANIMATED_SPRITE_SHEET_VERB_ID),
            ctx_with_sprite(),
            inputs,
        )
        .unwrap();
    inv.cancel();
    let res = inv.finish().await;
    assert!(res.is_err());
}

#[tokio::test]
async fn ai_mode_uses_custom_layer_name() {
    let runtime = VerbRuntime::new();
    runtime.register(AnimatedSpriteSheetVerb::new()).unwrap();
    runtime
        .register_backend(BackendProxy::new(MockSheetBackend::image_only()), 0)
        .unwrap();

    let mut p = ai_inputs(2);
    p.layer_name = Some("hero-idle-walk".into());
    let inputs = VerbInputs::from_struct(&p).unwrap();
    let inv = runtime
        .invoke(
            &VerbId::new(ANIMATED_SPRITE_SHEET_VERB_ID),
            ctx_with_sprite(),
            inputs,
        )
        .unwrap();
    let preview = inv.finish().await.unwrap();
    match &preview.output.effects[0] {
        VerbEffect::AddLayer { layer, .. } => {
            assert_eq!(layer.name, "hero-idle-walk");
        }
        other => panic!("expected AddLayer, got {other:?}"),
    }
}

#[tokio::test]
async fn commit_returns_effects() {
    let runtime = VerbRuntime::new();
    runtime.register(AnimatedSpriteSheetVerb::new()).unwrap();
    let mock = MockSheetBackend::image_only();
    let image_calls_arc = mock.image_calls.clone();
    runtime
        .register_backend(BackendProxy::new(mock), 0)
        .unwrap();

    let inputs = VerbInputs::from_struct(&ai_inputs(2)).unwrap();
    let inv = runtime
        .invoke(
            &VerbId::new(ANIMATED_SPRITE_SHEET_VERB_ID),
            ctx_with_sprite(),
            inputs,
        )
        .unwrap();
    let preview = inv.finish().await.unwrap();
    let commit = runtime.commit(preview);
    assert_eq!(commit.effects.len(), 1);
    assert_eq!(image_calls_arc.load(Ordering::SeqCst), 1);
}
