//! Live end-to-end smoke test for `OpenAI` `gpt-image` generation.
//!
//! This is the one part of the reference-sheet pipeline a mock can't prove:
//! that the live `OpenAI` API accepts our request body, that the configured
//! model id is valid, and that the streamed response decodes to a real image.
//! It is `#[ignore]`d because it hits the network, needs a key, and costs a
//! few cents per run.
//!
//! Run it after setting your `OpenAI` key in the app (Settings -> AI), or pass
//! the key via the environment:
//!
//! ```text
//! cargo test -p pixhaus-ai --test openai_live_image -- --ignored --nocapture
//! # or, without touching the keychain:
//! OPENAI_API_KEY=sk-... cargo test -p pixhaus-ai --test openai_live_image -- --ignored --nocapture
//! ```
//!
//! On macOS the OS keychain prompts for access the first time a process reads
//! the stored key — click "Always Allow". The generated anchor PNG is written
//! under the crate's target tmp dir; the path is printed on success.

// Integration tests are a separate crate, so lib.rs's `#![cfg_attr(test,
// allow(...))]` doesn't reach them. Lift the same exemptions, plus
// `print_stderr` for the skip/success diagnostics.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::disallowed_methods,
    clippy::print_stderr,
    clippy::missing_panics_doc
)]

use std::time::Duration;

use pixhaus_ai::backends::openai::OpenAiBackend;
use pixhaus_ai::backends::{
    ImageGenRequest, ImageQuality, InferenceBackend, InferenceRequest, InferenceResponse,
};
use pixhaus_ai::plugin::progress::VerbProgress;
use tokio_util::sync::CancellationToken;

/// Builds a backend from `OPENAI_API_KEY` if set, otherwise from the OS
/// keychain (the same source the app uses). Returns `None` when no key is
/// available so the test can skip rather than fail.
fn backend_from_available_key() -> Option<OpenAiBackend> {
    if let Ok(key) = std::env::var("OPENAI_API_KEY") {
        let trimmed = key.trim();
        if !trimmed.is_empty() {
            return Some(OpenAiBackend::new(trimmed));
        }
    }
    OpenAiBackend::from_keychain().ok()
}

#[ignore = "hits the real OpenAI API; needs a key and costs a few cents"]
#[tokio::test]
async fn generates_a_decodable_anchor_image() {
    let Some(backend) = backend_from_available_key() else {
        eprintln!(
            "skipping: no OpenAI key found. Configure it in the app (Settings -> AI) \
             or set OPENAI_API_KEY before running with --ignored."
        );
        return;
    };

    let request = ImageGenRequest {
        model: None, // backend default (gpt-image-2)
        prompt: "a single pixel-art knight character sprite, front view, \
                 plain flat background, crisp pixels, 32x32 sprite scaled up"
            .into(),
        negative_prompt: None,
        width: 1024,
        height: 1024,
        steps: None,
        seed: None,
        num_images: 1,
        quality: Some(ImageQuality::Low),
        style_image: None,
        reference_images: Vec::new(),
    };

    let response = tokio::time::timeout(
        Duration::from_secs(180),
        backend.invoke(
            InferenceRequest::ImageGeneration(request),
            VerbProgress::discard(),
            CancellationToken::new(),
        ),
    )
    .await
    .expect("OpenAI image request timed out after 180s")
    .expect("OpenAI image generation failed");

    let InferenceResponse::Image(image) = response else {
        panic!("expected an image response from the image-generation request");
    };

    assert!(!image.images.is_empty(), "OpenAI returned zero images");
    let bytes = &image.images[0];
    assert!(!bytes.is_empty(), "first returned image is empty");

    let decoded = image::load_from_memory(bytes)
        .expect("returned bytes are not a decodable image — the pipeline produced garbage");
    assert!(
        decoded.width() > 0 && decoded.height() > 0,
        "decoded image has zero dimensions"
    );

    let out = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("pixhaus-openai-anchor.png");
    std::fs::write(&out, bytes).expect("failed to write the generated anchor PNG");

    eprintln!(
        "OK: model={} dims={}x{} bytes={} -> wrote anchor to {}",
        image.model,
        decoded.width(),
        decoded.height(),
        bytes.len(),
        out.display(),
    );
}
