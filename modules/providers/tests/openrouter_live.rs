//! Live OpenRouter integration test. Ignored by default - it needs a real
//! `OPENROUTER_API_KEY` and network access, so it never runs in the session gate.
//!
//! Run it manually after setting the key (and optionally `PIXHAUS_OPENROUTER_MODEL`):
//!
//! ```text
//! OPENROUTER_API_KEY=... cargo nextest run -p pixhaus-mod-providers -- --ignored
//! ```

#![allow(clippy::disallowed_methods, clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use tokio_util::sync::CancellationToken;

use pixhaus_mod_providers::OpenRouterProvider;
use pixhaus_services::generated::GeneratedResult;
use pixhaus_services::job::{GenerationContext, GenerationJobInput, GenerationKind};
use pixhaus_services::provider::Provider;

#[tokio::test]
#[ignore = "requires OPENROUTER_API_KEY and network access"]
async fn live_anchor_generation_returns_a_sprite() {
    // Skip cleanly if the key is absent (the test is `--ignored`, so this guard only
    // matters when someone runs it without a key set).
    let Ok(key) = std::env::var("OPENROUTER_API_KEY") else {
        return;
    };
    let provider = OpenRouterProvider::new(key).expect("the client builds from a key");
    let input = GenerationJobInput {
        prompt: "A small friendly robot mascot, 2D side view, on a flat solid pure magenta #FF00FF background.".to_owned(),
        seed: 1,
        size: (512, 512),
        context: GenerationContext::NewAsset,
        kind: GenerationKind::Anchor,
    };

    let result = provider.generate(&input, CancellationToken::new()).await.expect("generation succeeds");
    let GeneratedResult::Sprite(asset) = result else {
        panic!("an anchor must come back as a still sprite");
    };
    assert!(!asset.rgba.is_empty(), "the anchor has pixels");
    assert_eq!(asset.rgba.len(), (asset.width * asset.height * 4) as usize, "the anchor RGBA is tightly packed");
}
