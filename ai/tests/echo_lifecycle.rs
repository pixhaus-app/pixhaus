//! End-to-end integration test for the verb plugin protocol.
//!
//! Exercises the full lifecycle through the public API surface — no
//! `pub(crate)` access — to prove the contract holds for downstream
//! consumers (S21 verb runtime, S22 backend adapters, the 14 built-in
//! verb streams).

// Integration tests are a separate crate, so the lib.rs `#![cfg_attr(test,
// allow(...))]` block doesn't reach them. Lift the same exemptions
// here so the test code can use `unwrap` / `expect` / `panic` /
// `assert_eq!` on floats freely.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::disallowed_methods,
    clippy::float_cmp,
    clippy::missing_panics_doc
)]

use std::time::Duration;

use pixhaus_ai::plugin::{
    ECHO_VERB_ID, EchoInputs, EchoVerb, PixelData, VerbContext, VerbEffect, VerbId, VerbInputs,
    VerbProgressEvent, VerbRuntime,
};
use pixhaus_core::project::{FrameIndex, ProjectMetadata, SpriteId};

fn metadata() -> ProjectMetadata {
    ProjectMetadata {
        name: "echo-integration".into(),
        description: None,
        author: None,
        created_at: 0,
        updated_at: 0,
        editor_version: env!("CARGO_PKG_VERSION").into(),
    }
}

fn sample_pixels() -> PixelData {
    PixelData::rgba8(
        2,
        2,
        vec![
            0xff, 0x00, 0x00, 0xff, 0x00, 0xff, 0x00, 0xff, 0x00, 0x00, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff,
        ],
    )
}

fn ctx_with_active_sprite() -> VerbContext {
    let mut ctx = VerbContext::empty(metadata());
    ctx.active_sprite = Some(SpriteId::new(1));
    ctx.active_frame = Some(FrameIndex::new(0));
    ctx
}

fn echo_inputs() -> VerbInputs {
    VerbInputs::from_struct(&EchoInputs {
        pixels: sample_pixels(),
        layer_name: Some("Echo Test".into()),
    })
    .expect("inputs serialise")
}

#[tokio::test]
async fn full_invoke_preview_commit_lifecycle() {
    let runtime = VerbRuntime::new();
    runtime.register(EchoVerb::new()).expect("register");

    // Discovery: descriptor is listed.
    let descs = runtime.list();
    assert_eq!(descs.len(), 1);
    assert_eq!(descs[0].id.as_str(), ECHO_VERB_ID);

    // Invocation: progress events arrive in order, finish yields a
    // preview with the expected effect.
    let mut inv = runtime
        .invoke(
            &VerbId::new(ECHO_VERB_ID),
            ctx_with_active_sprite(),
            echo_inputs(),
        )
        .expect("invoke");

    let mut events = Vec::new();
    while let Some(ev) = inv.next_progress().await {
        events.push(ev);
    }
    assert!(matches!(events[0], VerbProgressEvent::Started { .. }));
    assert!(
        events
            .iter()
            .any(|e| matches!(e, VerbProgressEvent::Step { .. }))
    );

    let preview = inv.finish().await.expect("preview");
    assert_eq!(preview.verb.as_str(), ECHO_VERB_ID);
    assert_eq!(preview.output.effects.len(), 1);
    let VerbEffect::AddLayer {
        sprite,
        layer,
        cels,
        pixel_buffers,
    } = &preview.output.effects[0]
    else {
        panic!("expected AddLayer, got {:?}", preview.output.effects[0]);
    };
    assert_eq!(*sprite, SpriteId::new(1));
    assert_eq!(layer.name, "Echo Test");
    assert_eq!(cels.len(), 1);
    assert_eq!(pixel_buffers.len(), 1);
    assert_eq!(pixel_buffers[0].pixels.bytes.len(), 16);

    // Commit: stateless wrapping with a fresh timestamp.
    let commit = runtime.commit(preview.clone());
    assert_eq!(commit.preview, preview.id);
    assert_eq!(commit.effects.len(), 1);
}

#[tokio::test]
async fn cancel_before_invocation_finishes() {
    // The echo verb completes synchronously, so to demonstrate
    // cancellation we register a verb that blocks on the token.
    use async_trait::async_trait;
    use pixhaus_ai::plugin::{
        BackendCapabilities, CostEstimate, EffectKind, Result as VerbResult, Verb, VerbDescriptor,
        VerbError, VerbInputs, VerbOutput, VerbProgress,
    };
    use tokio_util::sync::CancellationToken;

    struct Blocks {
        descriptor: VerbDescriptor,
    }
    impl Blocks {
        fn new() -> Self {
            Self {
                descriptor: VerbDescriptor {
                    id: VerbId::new("test.blocks"),
                    display_name: "Blocks".into(),
                    description: "Awaits cancellation".into(),
                    version: "0.1.0".into(),
                    required_capabilities: BackendCapabilities::empty(),
                    input_schema: serde_json::json!({}),
                    output_schema: None,
                    output_kinds: vec![EffectKind::AddLayer],
                    cost_estimate: CostEstimate::free(),
                    streaming: false,
                    cancellable: true,
                    documentation_url: None,
                },
            }
        }
    }

    #[async_trait]
    impl Verb for Blocks {
        fn descriptor(&self) -> &VerbDescriptor {
            &self.descriptor
        }
        async fn invoke(
            &self,
            _ctx: VerbContext,
            _inputs: VerbInputs,
            _progress: VerbProgress,
            cancel: CancellationToken,
        ) -> VerbResult<VerbOutput> {
            cancel.cancelled().await;
            Err(VerbError::Cancelled)
        }
    }

    let runtime = VerbRuntime::new();
    runtime.register(Blocks::new()).unwrap();

    let inv = runtime
        .invoke(
            &VerbId::new("test.blocks"),
            VerbContext::empty(metadata()),
            VerbInputs::empty(),
        )
        .unwrap();

    let cancel = inv.cancellation();
    cancel.cancel();

    let res = inv.finish().await;
    assert!(matches!(res, Err(VerbError::Cancelled)));
}

#[tokio::test]
async fn discard_records_reason() {
    let runtime = VerbRuntime::new();
    runtime.register(EchoVerb::new()).unwrap();

    let inv = runtime
        .invoke(
            &VerbId::new(ECHO_VERB_ID),
            ctx_with_active_sprite(),
            echo_inputs(),
        )
        .unwrap();
    let preview = inv.finish().await.unwrap();
    let discard = runtime.discard(preview.clone(), Some("user rejected".into()));
    assert_eq!(discard.preview, preview.id);
    assert_eq!(discard.reason.as_deref(), Some("user rejected"));
}

#[tokio::test]
async fn descriptor_reports_realistic_cost_for_echo() {
    let runtime = VerbRuntime::new();
    runtime.register(EchoVerb::new()).unwrap();

    let desc = runtime
        .descriptor(&VerbId::new(ECHO_VERB_ID))
        .expect("descriptor present");
    assert!(desc.required_capabilities.is_empty());
    assert_eq!(desc.cost_estimate.typical_usd_cents, 0.0);
    assert_eq!(desc.cost_estimate.max_latency, Duration::ZERO);
}

#[tokio::test]
async fn malformed_inputs_fail_validate() {
    let runtime = VerbRuntime::new();
    runtime.register(EchoVerb::new()).unwrap();

    let bad = VerbInputs::from_struct(&EchoInputs {
        pixels: PixelData {
            width: 4,
            height: 4,
            bytes_per_pixel: 4,
            // Stride too small — would overflow on per-row indexing.
            stride: 8,
            bytes: vec![0; 32],
        },
        layer_name: None,
    })
    .unwrap();

    let err = runtime
        .invoke(&VerbId::new(ECHO_VERB_ID), ctx_with_active_sprite(), bad)
        .unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("schema") || msg.contains("dimensions"));
}
