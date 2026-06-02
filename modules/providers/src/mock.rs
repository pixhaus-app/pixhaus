//! The offline mock provider.
//!
//! Runs as a real tokio task with a small simulated delay so the queued -> running
//! -> complete lifecycle and the result channel are exercised exactly as a real
//! provider would be (bible 13.5). Deterministic: it hashes the prompt and seed to
//! pick a two-colour palette and draws a centred diamond, so the same input always
//! yields the same bytes — visible and reproducible without a GPU or API key.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokio_util::sync::CancellationToken;

use pixhaus_services::generated::{GeneratedAsset, GenerationProvenance};
use pixhaus_services::job::GenerationJobInput;
use pixhaus_services::provider::{GenerateFuture, Provider, ProviderCapability, ProviderError, ProviderId};

const CAPABILITIES: &[ProviderCapability] = &[ProviderCapability::TextToSprite];

/// An offline provider that draws a deterministic sprite from the prompt.
pub struct MockProvider {
    delay: Duration,
}

impl MockProvider {
    /// A mock provider with a small default latency.
    pub fn new() -> Self {
        Self::with_delay(Duration::from_millis(40))
    }

    /// A mock provider with an explicit simulated latency (used in tests).
    pub fn with_delay(delay: Duration) -> Self {
        Self { delay }
    }
}

impl Default for MockProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl Provider for MockProvider {
    fn id(&self) -> ProviderId {
        ProviderId("mock".to_owned())
    }

    fn label_key(&self) -> &'static str {
        "provider.mock.label"
    }

    fn capabilities(&self) -> &[ProviderCapability] {
        CAPABILITIES
    }

    fn generate<'a>(&'a self, input: &'a GenerationJobInput, cancel: CancellationToken) -> GenerateFuture<'a> {
        let delay = self.delay;
        Box::pin(async move {
            // PERF/UX: simulate provider latency so the async job lifecycle is real.
            tokio::time::sleep(delay).await;
            if cancel.is_cancelled() {
                return Err(ProviderError::Cancelled);
            }
            Ok(draw_deterministic(input))
        })
    }
}

/// A two-colour palette derived from the prompt.
struct Palette {
    background: [u8; 4],
    foreground: [u8; 4],
}

/// Draws a centred diamond in the foreground colour over the background colour.
fn draw_deterministic(input: &GenerationJobInput) -> GeneratedAsset {
    let (width, height) = input.size;
    let palette = palette_from_prompt(&input.prompt, input.seed);
    let center_x = width / 2;
    let center_y = height / 2;
    let radius = width.min(height) / 2;

    let mut rgba = Vec::with_capacity(width as usize * height as usize * 4);
    for y in 0..height {
        for x in 0..width {
            let inside = x.abs_diff(center_x) + y.abs_diff(center_y) <= radius;
            let color = if inside { palette.foreground } else { palette.background };
            rgba.extend_from_slice(&color);
        }
    }

    GeneratedAsset {
        width,
        height,
        stride: width * 4,
        rgba,
        provenance: GenerationProvenance {
            prompt: input.prompt.clone(),
            seed: input.seed,
            provider_id: "mock".to_owned(),
            model: "mock-shapes-1".to_owned(),
            created_unix_ms: now_ms(),
        },
    }
}

fn palette_from_prompt(prompt: &str, seed: u64) -> Palette {
    let bytes = hash_prompt(prompt, seed).to_le_bytes();
    Palette {
        background: [bytes[0], bytes[1], bytes[2], 255],
        foreground: [bytes[3], bytes[4], bytes[5], 255],
    }
}

/// FNV-1a over the prompt, mixed with the seed. Deterministic, no `rand` dependency.
fn hash_prompt(prompt: &str, seed: u64) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64 ^ seed;
    for byte in prompt.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|d| u64::try_from(d.as_millis()).ok())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pixhaus_core::PixelBuffer;
    use pixhaus_services::job::GenerationContext;

    fn input(prompt: &str) -> GenerationJobInput {
        GenerationJobInput {
            prompt: prompt.to_owned(),
            seed: 7,
            size: (8, 8),
            context: GenerationContext::NewAsset,
        }
    }

    #[test]
    fn draw_is_deterministic_for_same_input() {
        let a = draw_deterministic(&input("knight"));
        let b = draw_deterministic(&input("knight"));
        assert_eq!(a.rgba, b.rgba);
    }

    #[test]
    fn different_prompts_produce_different_bytes() {
        let a = draw_deterministic(&input("knight"));
        let b = draw_deterministic(&input("goblin"));
        assert_ne!(a.rgba, b.rgba);
    }

    #[test]
    fn output_size_and_stride_satisfy_pixel_buffer() {
        let asset = draw_deterministic(&input("x"));
        assert_eq!(asset.width, 8);
        assert_eq!(asset.height, 8);
        assert_eq!(asset.stride, 32);
        assert!(PixelBuffer::from_rgba8(asset.width, asset.height, asset.stride, asset.rgba).is_ok());
    }
}
