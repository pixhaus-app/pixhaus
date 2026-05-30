//! Gate 6 — the always-on, mocked `LocalFluxBackend` ergonomics test.
//!
//! Runs WITHOUT weights or a GPU: a `mockall` double stands in for the
//! [`FluxRunner`] so the `spawn_blocking` / progress / cancel bridge is exercised
//! end to end. Asserts the capability bits, the backend id, that unsupported
//! request arms return `UnsupportedCapability`, that a fired cancel token yields
//! `Cancelled`, and that the std-channel -> `VerbProgress` bridge emits the
//! expected `Step` events.
//!
//! Gated on `feature = "local-flux"` — the only build where `LocalFluxBackend`
//! exists. Per the spec this is the one milestone-6 gate that runs in normal CI.

#![cfg(feature = "local-flux")]
// Test-only relaxations: the workspace floor denies unwrap/expect/panic and the
// disallowed-methods list bans unwrap/expect, but a self-contained test that
// builds its own fixtures may unwrap on values it just constructed.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::disallowed_methods)]
// The mock! body re-emits the trait's `&(dyn Fn() -> bool + Send + Sync)` param
// verbatim; the grouping parens are flagged as unnecessary there but keeping them
// matches the trait signature exactly.
#![allow(unused_parens)]

use std::sync::Arc;
use std::sync::mpsc;
use std::time::Duration;

use mockall::mock;
use mockall::predicate::eq;
use pixhaus_ai::backends::local_flux::{FluxMode, FluxRequest, FluxRunner, FluxTick, LocalFluxBackend};
use pixhaus_ai::backends::{BackendError, ImageEditRequest, ImageGenRequest, InferenceBackend, InferenceRequest, InferenceResponse, Result as BackendResult};
use pixhaus_ai::plugin::descriptor::BackendCapabilities;
use pixhaus_ai::plugin::progress::{VerbProgress, VerbProgressEvent};
use tokio_util::sync::CancellationToken;

// mockall double for the boundary trait. `run` takes a `&dyn Fn` and a channel
// sender, neither of which mockall can match on by value, so each test supplies
// a closure via `.returning(..)` that drives the channel and the callback
// itself. The trait method must be matched with the exact signature.
mock! {
    Runner {}
    impl FluxRunner for Runner {
        fn run(
            &self,
            mode: FluxMode,
            request: FluxRequest,
            tick: &mpsc::Sender<FluxTick>,
            should_continue: &(dyn Fn() -> bool + Send + Sync),
        ) -> BackendResult<Vec<Vec<u8>>>;
    }
}

/// A 1x1 green PNG, the stand-in "generated" image the mock returns.
fn one_pixel_png() -> Vec<u8> {
    let img = image::RgbaImage::from_pixel(1, 1, image::Rgba([0, 255, 0, 255]));
    let mut bytes = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png).unwrap();
    bytes
}

fn t2i_request() -> InferenceRequest {
    InferenceRequest::ImageGeneration(ImageGenRequest {
        model: None,
        prompt: "a small robot".to_owned(),
        negative_prompt: None,
        width: 64,
        height: 64,
        steps: None,
        seed: Some(7),
        num_images: 1,
        quality: None,
        style_image: None,
        reference_images: Vec::new(),
    })
}

fn backend_with<F>(setup: F) -> LocalFluxBackend
where
    F: FnOnce(&mut MockRunner),
{
    let mut runner = MockRunner::new();
    setup(&mut runner);
    LocalFluxBackend::with_runner(Arc::new(runner))
}

#[test]
fn backend_id_is_flux_local() {
    let backend = backend_with(|_| {});
    assert_eq!(backend.backend_id(), "flux-local");
}

#[test]
fn capabilities_are_generation_edit_inpaint() {
    let backend = backend_with(|_| {});
    let caps = backend.capabilities();
    assert!(caps.contains(BackendCapabilities::IMAGE_GENERATION));
    assert!(caps.contains(BackendCapabilities::IMAGE_EDIT));
    assert!(caps.contains(BackendCapabilities::IMAGE_INPAINT));
    // Nothing it does not advertise.
    assert!(!caps.contains(BackendCapabilities::IMAGE_TO_VIDEO));
    assert!(!caps.contains(BackendCapabilities::TEXT_GENERATION));
}

#[test]
fn does_not_stream_and_is_free() {
    let backend = backend_with(|_| {});
    assert!(!backend.supports_streaming());
    let estimate = backend.estimate_cost(&t2i_request());
    assert_eq!(estimate.typical_usd_cents, 0.0);
    assert_eq!(estimate.max_usd_cents, 0.0);
    // The latency hint is non-zero so the UI can warn on a slow device.
    assert!(estimate.max_latency > Duration::ZERO);
}

#[tokio::test]
async fn unsupported_arms_return_unsupported_capability() {
    // Text generation, frame interpolation, etc. are never routed here by the
    // registry, but a direct caller must get the right error rather than a panic.
    let backend = backend_with(|runner| {
        // The mock must never be invoked for an unsupported arm.
        runner.expect_run().never();
    });
    let progress = VerbProgress::discard();
    let cancel = CancellationToken::new();
    let request = InferenceRequest::Text(pixhaus_ai::backends::TextGenRequest::user("hi"));
    let result = backend.invoke(request, progress, cancel).await;
    assert!(matches!(result, Err(BackendError::UnsupportedCapability)));
}

#[tokio::test]
async fn text_to_image_succeeds_and_bridges_step_events() {
    // The mock drives the std channel exactly as the real sampler would: a
    // Loading tick, then one Step tick per distilled step, polling the cancel
    // callback before each.
    let backend = backend_with(|runner| {
        runner
            .expect_run()
            .withf(|mode, _req, _tick, _cont| *mode == FluxMode::TextToImage)
            .times(1)
            .returning(|_mode, _req, tick, should_continue| {
                tick.send(FluxTick::Loading).unwrap();
                let total = 4;
                // Mirror the production callback's 1-based step ticks.
                for step in 1..=total {
                    assert!(should_continue(), "callback should report continue when not cancelled");
                    tick.send(FluxTick::Step { step, total }).unwrap();
                }
                Ok(vec![one_pixel_png()])
            });
    });

    let (progress, mut rx) = VerbProgress::channel();
    let cancel = CancellationToken::new();

    let response = backend.invoke(t2i_request(), progress, cancel).await.expect("t2i should succeed");
    match response {
        InferenceResponse::Image(image) => {
            assert_eq!(image.images.len(), 1);
            assert_eq!(image.model, "flux2-klein-4b");
        }
        _ => panic!("expected an image response"),
    }

    // Drain the progress channel: a Started, then a "loading model" Step, then a
    // Step per sampling step with a 0..=1 fraction and a "step n/4" message.
    let mut step_messages = Vec::new();
    let mut saw_started = false;
    while let Some(event) = rx.recv().await {
        match event {
            VerbProgressEvent::Started { .. } => saw_started = true,
            VerbProgressEvent::Step { fraction, message } => {
                if let Some(frac) = fraction {
                    assert!((0.0..=1.0).contains(&frac), "fraction {frac} out of range");
                }
                step_messages.push(message);
            }
            _ => {}
        }
    }
    assert!(saw_started, "the backend should emit a Started event");
    // One "loading model" plus four "step n/4" messages.
    assert!(step_messages.iter().any(|m| m == "loading model"), "messages: {step_messages:?}");
    for n in 1..=4 {
        let expected = format!("step {n}/4");
        assert!(step_messages.contains(&expected), "missing {expected}; got {step_messages:?}");
    }
}

#[tokio::test]
async fn cancel_during_a_step_yields_cancelled() {
    // Mid-run cancel: the mock cancels the shared token before polling the
    // callback, so `should_continue` reports stop (false) — exactly what the real
    // sampler sees when the token fires between steps. The mock then returns Ok
    // with a partial result (the real sampler decodes a partial latent on an
    // early break); the backend must surface Cancelled over that Ok via its
    // post-join token re-check.
    let cancel = CancellationToken::new();
    let cancel_in_closure = cancel.clone();
    let backend = backend_with(move |runner| {
        runner.expect_run().times(1).returning(move |_mode, _req, tick, should_continue| {
            tick.send(FluxTick::Loading).unwrap();
            // Fire the token now, as if the user hit Cancel during the run.
            cancel_in_closure.cancel();
            assert!(!should_continue(), "callback must report stop once the token is cancelled");
            Ok(vec![one_pixel_png()])
        });
    });

    let progress = VerbProgress::discard();
    match backend.invoke(t2i_request(), progress, cancel).await {
        Err(BackendError::Cancelled) => {}
        Err(other) => panic!("expected Cancelled, got error {other:?}"),
        Ok(_) => panic!("expected Cancelled, got a successful response"),
    }
}

#[tokio::test]
async fn cancel_before_invoke_short_circuits_to_cancelled() {
    // A token already fired before invoke never reaches the runner — the backend
    // short-circuits at the top. The mock must not be called.
    let backend = backend_with(|runner| {
        runner.expect_run().never();
    });
    let progress = VerbProgress::discard();
    let cancel = CancellationToken::new();
    cancel.cancel();

    match backend.invoke(t2i_request(), progress, cancel).await {
        Err(BackendError::Cancelled) => {}
        Err(other) => panic!("expected Cancelled, got error {other:?}"),
        Ok(_) => panic!("expected Cancelled, got a successful response"),
    }
}

#[tokio::test]
async fn image_edit_routes_to_image_to_image() {
    let source = one_pixel_png();
    let backend = backend_with(|runner| {
        runner
            .expect_run()
            .with(
                eq(FluxMode::ImageToImage),
                mockall::predicate::always(),
                mockall::predicate::always(),
                mockall::predicate::always(),
            )
            .times(1)
            .returning(|_mode, req, tick, _cont| {
                // The init image is threaded through from the edit request.
                assert!(req.init_image.is_some(), "i2i must carry an init image");
                tick.send(FluxTick::Loading).unwrap();
                Ok(vec![one_pixel_png()])
            });
    });

    let request = InferenceRequest::ImageEdit(ImageEditRequest {
        model: None,
        image: source,
        mask: None,
        prompt: "make it blue".to_owned(),
        negative_prompt: None,
        num_images: 1,
        style_image: None,
        reference_images: Vec::new(),
        strength: None,
    });
    let progress = VerbProgress::discard();
    let cancel = CancellationToken::new();
    let response = backend.invoke(request, progress, cancel).await.expect("edit should succeed");
    assert!(matches!(response, InferenceResponse::Image(_)));
}
