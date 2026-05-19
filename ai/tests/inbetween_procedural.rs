//! Integration tests for the procedural branch of [`InbetweenVerb`].
//!
//! The verb's descriptor still declares `FRAME_INTERPOLATION` as a
//! required capability, so the public runtime entry point
//! (`VerbRuntime::invoke`) refuses to dispatch without a backend. The
//! procedural path is meant to be reachable in that exact scenario:
//! we therefore call [`Verb::invoke`] directly with a backend-less
//! context to confirm the verb succeeds, returns `AddFrames`, and
//! emits one cel per requested output.
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
    clippy::useless_vec
)]

use pixhaus_ai::plugin::{PixelData, Verb, VerbContext, VerbEffect, VerbInputs, VerbProgress};
use pixhaus_ai::verbs::inbetween::{InbetweenInputs, InbetweenMode, InbetweenVerb};
use pixhaus_core::project::{FrameIndex, LayerId, ProjectMetadata, SpriteId};
use tokio_util::sync::CancellationToken;

fn metadata() -> ProjectMetadata {
    ProjectMetadata {
        name: "inbetween-procedural-test".into(),
        description: None,
        author: None,
        created_at: 0,
        updated_at: 0,
        editor_version: "0".into(),
    }
}

fn ctx_no_backend() -> VerbContext {
    let mut ctx = VerbContext::empty(metadata());
    ctx.active_sprite = Some(SpriteId::new(1));
    ctx.active_frame = Some(FrameIndex::new(0));
    ctx.active_layer = Some(LayerId::new(42));
    ctx
}

fn make_frame(r: u8, g: u8, b: u8) -> PixelData {
    PixelData::rgba8(4, 4, vec![r, g, b, 255].repeat(16))
}

fn make_inputs(mode: InbetweenMode, num_outputs: u32) -> VerbInputs {
    VerbInputs::from_struct(&InbetweenInputs {
        frame_a: make_frame(0, 0, 0),
        frame_b: make_frame(255, 255, 255),
        frame_a_index: 0,
        frame_b_index: 5,
        num_outputs,
        mode,
    })
    .unwrap()
}

#[tokio::test]
async fn procedural_mode_succeeds_without_backend() {
    let verb = InbetweenVerb::new();
    let inputs = make_inputs(
        InbetweenMode::Procedural {
            variance_range: 2.5,
        },
        1,
    );

    let out = verb
        .invoke(
            ctx_no_backend(),
            inputs,
            VerbProgress::discard(),
            CancellationToken::new(),
        )
        .await
        .expect("procedural inbetween should succeed without a backend");

    assert_eq!(out.effects.len(), 1, "expected one AddFrames effect");
    match &out.effects[0] {
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
async fn procedural_mode_generates_multiple_frames() {
    let verb = InbetweenVerb::new();
    let inputs = make_inputs(
        InbetweenMode::Procedural {
            variance_range: 2.5,
        },
        4,
    );

    let out = verb
        .invoke(
            ctx_no_backend(),
            inputs,
            VerbProgress::discard(),
            CancellationToken::new(),
        )
        .await
        .unwrap();

    match &out.effects[0] {
        VerbEffect::AddFrames {
            frames,
            cels,
            pixel_buffers,
            ..
        } => {
            assert_eq!(frames.len(), 4);
            assert_eq!(cels.len(), 4);
            assert_eq!(pixel_buffers.len(), 4);
            // Each buffer must carry width*height*4 bytes (4*4*4 = 64).
            for buf in pixel_buffers {
                assert_eq!(buf.pixels.bytes.len(), 64);
                assert_eq!(buf.pixels.width, 4);
                assert_eq!(buf.pixels.height, 4);
            }
        }
        other => panic!("unexpected effect: {other:?}"),
    }
}

#[tokio::test]
async fn procedural_mode_places_cels_on_active_layer() {
    let verb = InbetweenVerb::new();
    let mut ctx = ctx_no_backend();
    let layer = LayerId::new(99);
    ctx.active_layer = Some(layer);

    let inputs = make_inputs(
        InbetweenMode::Procedural {
            variance_range: 2.5,
        },
        2,
    );

    let out = verb
        .invoke(
            ctx,
            inputs,
            VerbProgress::discard(),
            CancellationToken::new(),
        )
        .await
        .unwrap();

    match &out.effects[0] {
        VerbEffect::AddFrames { cels, .. } => {
            for cel in cels {
                assert_eq!(cel.layer_id, layer);
            }
        }
        other => panic!("unexpected effect: {other:?}"),
    }
}

#[tokio::test]
async fn procedural_mode_is_deterministic() {
    let verb = InbetweenVerb::new();
    let ctx = ctx_no_backend();

    let first = verb
        .invoke(
            ctx.clone(),
            make_inputs(
                InbetweenMode::Procedural {
                    variance_range: 2.5,
                },
                3,
            ),
            VerbProgress::discard(),
            CancellationToken::new(),
        )
        .await
        .unwrap();
    let second = verb
        .invoke(
            ctx,
            make_inputs(
                InbetweenMode::Procedural {
                    variance_range: 2.5,
                },
                3,
            ),
            VerbProgress::discard(),
            CancellationToken::new(),
        )
        .await
        .unwrap();

    let extract = |o: &pixhaus_ai::plugin::VerbOutput| -> Vec<Vec<u8>> {
        match &o.effects[0] {
            VerbEffect::AddFrames { pixel_buffers, .. } => pixel_buffers
                .iter()
                .map(|b| b.pixels.bytes.clone())
                .collect(),
            _ => panic!("expected AddFrames"),
        }
    };
    assert_eq!(extract(&first), extract(&second));
}
