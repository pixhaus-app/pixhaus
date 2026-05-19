//! Integration tests for [`InbetweenVerb`].
//!
//! Uses a mock backend that implements the fat [`InferenceBackend`] trait
//! and returns synthetic interpolated frames (simple linear blends of
//! frame_a and frame_b). The tests verify the full lifecycle:
//! register → invoke → stream progress → accept → commit.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::missing_panics_doc,
    clippy::disallowed_methods,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_lossless,
    clippy::doc_markdown,
    clippy::useless_vec,
    dead_code
)]

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::time::Duration;

use async_trait::async_trait;
use pixhaus_ai::backends::{
    BackendProxy, FrameInterpolationResponse, InferenceBackend, InferenceRequest,
    InferenceResponse, Result as BackendResult,
};
use pixhaus_ai::plugin::{
    BackendCapabilities, CostEstimate, INBETWEEN_VERB_ID, PixelData, VerbContext, VerbEffect,
    VerbId, VerbInputs, VerbProgress, VerbRuntime,
};
use pixhaus_ai::verbs::inbetween::{InbetweenInputs, InbetweenMode, InbetweenVerb};
use pixhaus_core::project::{FrameIndex, ProjectMetadata, SpriteId};
use tokio_util::sync::CancellationToken;

// ── Mock backend ───────────────────────────────────────────────────────────

/// Returns one blended frame per requested output. The blend is just a
/// 50/50 average of frame_a and frame_b bytes so we produce valid PNG.
#[derive(Debug)]
struct MockInterpolationBackend {
    invoke_count: Arc<AtomicUsize>,
}

impl MockInterpolationBackend {
    fn new() -> Self {
        Self {
            invoke_count: Arc::new(AtomicUsize::new(0)),
        }
    }

    fn invoke_count(&self) -> usize {
        self.invoke_count.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl InferenceBackend for MockInterpolationBackend {
    fn backend_id(&self) -> &'static str {
        "mock.interpolation"
    }

    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities::FRAME_INTERPOLATION
    }

    fn supports_streaming(&self) -> bool {
        false
    }

    fn estimate_cost(&self, _req: &InferenceRequest) -> CostEstimate {
        CostEstimate {
            typical_latency: Duration::from_millis(10),
            max_latency: Duration::from_millis(50),
            typical_usd_cents: 0.0,
            max_usd_cents: 0.0,
        }
    }

    async fn invoke(
        &self,
        request: InferenceRequest,
        _progress: VerbProgress,
        _cancel: CancellationToken,
    ) -> BackendResult<InferenceResponse> {
        self.invoke_count.fetch_add(1, Ordering::SeqCst);

        let interp = match request {
            InferenceRequest::FrameInterpolation(r) => r,
            other => panic!(
                "mock: unexpected request variant {:?}",
                std::mem::discriminant(&other)
            ),
        };

        assert_eq!(
            interp.frames.len(),
            2,
            "Inbetween must send exactly 2 frames"
        );
        let num = interp.num_outputs as usize;

        // Decode frame_a and frame_b, produce `num` blended copies.
        let img_a = image::load_from_memory_with_format(&interp.frames[0], image::ImageFormat::Png)
            .unwrap()
            .into_rgba8();
        let img_b = image::load_from_memory_with_format(&interp.frames[1], image::ImageFormat::Png)
            .unwrap()
            .into_rgba8();

        let (w, h) = img_a.dimensions();
        let mut frames = Vec::with_capacity(num);

        for i in 0..num {
            let t = (i + 1) as f32 / (num + 1) as f32; // 0 < t < 1
            let pixels: Vec<u8> = img_a
                .as_raw()
                .iter()
                .zip(img_b.as_raw().iter())
                .map(|(&a, &b)| ((a as f32) * (1.0 - t) + (b as f32) * t).round() as u8)
                .collect();

            let blended = image::RgbaImage::from_raw(w, h, pixels).unwrap();
            let mut png_buf: Vec<u8> = Vec::new();
            blended
                .write_to(
                    &mut std::io::Cursor::new(&mut png_buf),
                    image::ImageFormat::Png,
                )
                .unwrap();
            frames.push(png_buf);
        }

        Ok(InferenceResponse::Frames(FrameInterpolationResponse {
            frames,
            model: "mock.rife".into(),
        }))
    }
}

// ── Test helpers ───────────────────────────────────────────────────────────

fn metadata() -> ProjectMetadata {
    ProjectMetadata {
        name: "inbetween-test".into(),
        description: None,
        author: None,
        created_at: 0,
        updated_at: 0,
        editor_version: "0".into(),
    }
}

fn ctx_with_sprite() -> VerbContext {
    use pixhaus_core::project::LayerId;
    let mut ctx = VerbContext::empty(metadata());
    ctx.active_sprite = Some(SpriteId::new(1));
    ctx.active_frame = Some(FrameIndex::new(0));
    ctx.active_layer = Some(LayerId::new(42));
    ctx
}

fn make_frame(r: u8, g: u8, b: u8) -> PixelData {
    PixelData::rgba8(4, 4, vec![r, g, b, 255].repeat(16))
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn inbetween_generates_one_frame_by_default() {
    let runtime = VerbRuntime::new();
    runtime.register(InbetweenVerb::new()).unwrap();
    runtime
        .register_backend(BackendProxy::new(MockInterpolationBackend::new()), 0)
        .unwrap();

    let inputs = VerbInputs::from_struct(&InbetweenInputs {
        frame_a: make_frame(0, 0, 0),
        frame_b: make_frame(255, 255, 255),
        frame_a_index: 0,
        frame_b_index: 5,
        num_outputs: 1,
        mode: InbetweenMode::Ai,
    })
    .unwrap();

    let inv = runtime
        .invoke(&VerbId::new(INBETWEEN_VERB_ID), ctx_with_sprite(), inputs)
        .unwrap();
    let preview = inv.finish().await.unwrap();

    assert_eq!(preview.verb.as_str(), INBETWEEN_VERB_ID);
    assert_eq!(preview.output.effects.len(), 1);

    match &preview.output.effects[0] {
        VerbEffect::AddFrames {
            after,
            frames,
            cels,
            pixel_buffers,
            ..
        } => {
            assert_eq!(*after, Some(FrameIndex::new(0)));
            assert_eq!(frames.len(), 1);
            assert_eq!(cels.len(), 1);
            assert_eq!(pixel_buffers.len(), 1);
            assert!(!pixel_buffers[0].pixels.bytes.is_empty());
        }
        other => panic!("unexpected effect: {other:?}"),
    }
}

#[tokio::test]
async fn inbetween_generates_multiple_frames() {
    let runtime = VerbRuntime::new();
    runtime.register(InbetweenVerb::new()).unwrap();
    runtime
        .register_backend(BackendProxy::new(MockInterpolationBackend::new()), 0)
        .unwrap();

    let inputs = VerbInputs::from_struct(&InbetweenInputs {
        frame_a: make_frame(0, 0, 0),
        frame_b: make_frame(200, 100, 50),
        frame_a_index: 2,
        frame_b_index: 10,
        num_outputs: 4,
        mode: InbetweenMode::Ai,
    })
    .unwrap();

    let inv = runtime
        .invoke(&VerbId::new(INBETWEEN_VERB_ID), ctx_with_sprite(), inputs)
        .unwrap();
    let preview = inv.finish().await.unwrap();

    match &preview.output.effects[0] {
        VerbEffect::AddFrames {
            after,
            frames,
            cels,
            pixel_buffers,
            ..
        } => {
            assert_eq!(*after, Some(FrameIndex::new(2)));
            assert_eq!(frames.len(), 4);
            assert_eq!(cels.len(), 4);
            assert_eq!(pixel_buffers.len(), 4);
        }
        other => panic!("unexpected effect: {other:?}"),
    }
}

#[tokio::test]
async fn inbetween_requires_active_sprite() {
    let runtime = VerbRuntime::new();
    runtime.register(InbetweenVerb::new()).unwrap();
    runtime
        .register_backend(BackendProxy::new(MockInterpolationBackend::new()), 0)
        .unwrap();

    let inputs = VerbInputs::from_struct(&InbetweenInputs {
        frame_a: make_frame(0, 0, 0),
        frame_b: make_frame(255, 255, 255),
        frame_a_index: 0,
        frame_b_index: 2,
        num_outputs: 1,
        mode: InbetweenMode::Ai,
    })
    .unwrap();

    // No active sprite or layer in context.
    let inv = runtime
        .invoke(
            &VerbId::new(INBETWEEN_VERB_ID),
            VerbContext::empty(metadata()),
            inputs,
        )
        .unwrap();
    let res = inv.finish().await;
    assert!(res.is_err(), "should fail without active sprite");
}

#[tokio::test]
async fn inbetween_cancelled_before_backend() {
    let runtime = VerbRuntime::new();
    runtime.register(InbetweenVerb::new()).unwrap();
    runtime
        .register_backend(BackendProxy::new(MockInterpolationBackend::new()), 0)
        .unwrap();

    let inputs = VerbInputs::from_struct(&InbetweenInputs {
        frame_a: make_frame(0, 0, 0),
        frame_b: make_frame(255, 255, 255),
        frame_a_index: 0,
        frame_b_index: 2,
        num_outputs: 1,
        mode: InbetweenMode::Ai,
    })
    .unwrap();

    let inv = runtime
        .invoke(&VerbId::new(INBETWEEN_VERB_ID), ctx_with_sprite(), inputs)
        .unwrap();

    // Cancel immediately.
    inv.cancel();
    let res = inv.finish().await;
    assert!(res.is_err(), "cancelled invocation should return an error");
}

#[tokio::test]
async fn inbetween_cels_on_active_layer() {
    use pixhaus_core::project::LayerId;

    let runtime = VerbRuntime::new();
    runtime.register(InbetweenVerb::new()).unwrap();
    runtime
        .register_backend(BackendProxy::new(MockInterpolationBackend::new()), 0)
        .unwrap();

    let active_layer = LayerId::new(99);
    let mut ctx = ctx_with_sprite();
    ctx.active_layer = Some(active_layer);

    let inputs = VerbInputs::from_struct(&InbetweenInputs {
        frame_a: make_frame(0, 0, 0),
        frame_b: make_frame(200, 50, 100),
        frame_a_index: 1,
        frame_b_index: 3,
        num_outputs: 2,
        mode: InbetweenMode::Ai,
    })
    .unwrap();

    let inv = runtime
        .invoke(&VerbId::new(INBETWEEN_VERB_ID), ctx, inputs)
        .unwrap();
    let preview = inv.finish().await.unwrap();

    match &preview.output.effects[0] {
        VerbEffect::AddFrames { cels, .. } => {
            for cel in cels {
                assert_eq!(
                    cel.layer_id, active_layer,
                    "all cels must reference the active layer"
                );
            }
        }
        other => panic!("unexpected effect: {other:?}"),
    }
}

#[tokio::test]
async fn inbetween_commit_returns_effects() {
    let runtime = VerbRuntime::new();
    runtime.register(InbetweenVerb::new()).unwrap();
    runtime
        .register_backend(BackendProxy::new(MockInterpolationBackend::new()), 0)
        .unwrap();

    let inputs = VerbInputs::from_struct(&InbetweenInputs {
        frame_a: make_frame(10, 20, 30),
        frame_b: make_frame(40, 50, 60),
        frame_a_index: 0,
        frame_b_index: 2,
        num_outputs: 1,
        mode: InbetweenMode::Ai,
    })
    .unwrap();

    let inv = runtime
        .invoke(&VerbId::new(INBETWEEN_VERB_ID), ctx_with_sprite(), inputs)
        .unwrap();
    let preview = inv.finish().await.unwrap();
    let commit = runtime.commit(preview);
    assert_eq!(
        commit.effects.len(),
        1,
        "commit should carry the AddFrames effect"
    );
}
