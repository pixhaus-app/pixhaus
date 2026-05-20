//! Deterministic end-to-end test of the live `OpenAI` image HTTP path.
//!
//! Unlike `openai_live_image` (which needs a real key and hits `OpenAI`), this
//! stands up a local TCP server that returns an `OpenAI`-shaped Server-Sent
//! Events stream and points the backend at it via `with_base_url`. It exercises
//! the exact code the live call uses — request construction, the real reqwest
//! round-trip, the SSE stream parser, and PNG decode — with zero network, zero
//! cost, and full determinism, so the "live API half" of our own code is
//! proven in CI. The one thing it can't cover is whether `OpenAI`'s production
//! accepts a given model id; that's an account matter, not code correctness.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::disallowed_methods,
    clippy::missing_panics_doc
)]

use std::time::Duration;

use base64::Engine as _;
use pixhaus_ai::backends::openai::OpenAiBackend;
use pixhaus_ai::backends::{
    ImageGenRequest, ImageQuality, InferenceBackend, InferenceRequest, InferenceResponse,
};
use pixhaus_ai::plugin::progress::VerbProgress;
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

/// Encodes a real (tiny) PNG so the parser's decode path has valid bytes.
fn sample_png_b64() -> String {
    let mut img = image::RgbaImage::new(4, 4);
    for (_, _, p) in img.enumerate_pixels_mut() {
        *p = image::Rgba([10, 200, 90, 255]);
    }
    let mut bytes = Vec::new();
    image::DynamicImage::ImageRgba8(img)
        .write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Png,
        )
        .expect("encode sample png");
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

#[tokio::test]
async fn generate_image_round_trips_through_sse_over_http() {
    let png_b64 = sample_png_b64();

    // An OpenAI gpt-image SSE stream: a partial frame, then the final frame,
    // then the [DONE] sentinel. Frames are separated by a blank line.
    let body = format!(
        "data: {{\"type\":\"image_generation.partial_image\",\"b64_json\":\"{png_b64}\"}}\n\n\
         data: {{\"type\":\"image_generation.completed\",\"b64_json\":\"{png_b64}\"}}\n\n\
         data: [DONE]\n\n"
    );

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    // One-shot mock server: accept the request, drain its headers, and write a
    // fixed text/event-stream response with a matching Content-Length.
    let server_body = body.clone();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buf = [0u8; 4096];
        // Read until we've seen the end of the request headers.
        let mut seen = Vec::new();
        loop {
            let n = socket.read(&mut buf).await.unwrap();
            if n == 0 {
                break;
            }
            seen.extend_from_slice(&buf[..n]);
            if seen.windows(4).any(|w| w == b"\r\n\r\n") {
                break;
            }
        }
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            server_body.len(),
            server_body
        );
        socket.write_all(response.as_bytes()).await.unwrap();
        socket.flush().await.unwrap();
    });

    let backend = OpenAiBackend::new("test-key").with_base_url(format!("http://{addr}/v1"));
    let request = ImageGenRequest {
        model: None, // default gpt-image-2 -> exercises the SSE streaming path
        prompt: "a knight".into(),
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
        Duration::from_secs(10),
        backend.invoke(
            InferenceRequest::ImageGeneration(request),
            VerbProgress::discard(),
            CancellationToken::new(),
        ),
    )
    .await
    .expect("request timed out")
    .expect("backend invoke failed");

    server.await.unwrap();

    let InferenceResponse::Image(image) = response else {
        panic!("expected an image response");
    };
    assert_eq!(image.images.len(), 1, "exactly one final image expected");
    let decoded =
        image::load_from_memory(&image.images[0]).expect("final image bytes must decode as PNG");
    assert_eq!((decoded.width(), decoded.height()), (4, 4));
}
