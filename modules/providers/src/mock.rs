//! The offline mock provider.
//!
//! Runs as a real tokio task with a small simulated delay so the queued -> running
//! -> complete lifecycle and the result channel are exercised exactly as a real
//! provider would be (bible 13.5). Deterministic: it hashes the prompt and seed to
//! pick a two-colour palette and draws a centred diamond, so the same input always
//! yields the same bytes — visible and reproducible without a GPU or API key. For an
//! idle animation it bobs that diamond vertically across the frames so the result is
//! visibly animated while staying deterministic.

use std::time::Duration;

use tokio_util::sync::CancellationToken;

use pixhaus_core::LoopMode;
use pixhaus_services::generated::{GeneratedAnimation, GeneratedAsset, GeneratedFrame, GeneratedResult, GenerationProvenance};
use pixhaus_services::job::{GenerationJobInput, GenerationKind, Grid};
use pixhaus_services::provider::{GenerateFuture, Provider, ProviderCapability, ProviderError, ProviderId};

const CAPABILITIES: &[ProviderCapability] = &[
    ProviderCapability::TextToSprite,
    ProviderCapability::GenerateAnchor,
    ProviderCapability::GenerateIdleAnimation,
];

/// An offline provider that draws a deterministic sprite or idle animation.
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
            Ok(match &input.kind {
                GenerationKind::Anchor => GeneratedResult::Sprite(draw_anchor(input)),
                GenerationKind::IdleAnimation { animation_id, grid, fps, .. } => {
                    GeneratedResult::Animation(draw_idle_animation(input, animation_id, *grid, *fps))
                }
            })
        })
    }
}

/// A two-colour palette derived from the prompt.
struct Palette {
    background: [u8; 4],
    foreground: [u8; 4],
}

/// Draws a centred diamond in the foreground colour over the background colour.
fn draw_anchor(input: &GenerationJobInput) -> GeneratedAsset {
    let (width, height) = input.size;
    let palette = palette_from_prompt(&input.prompt, input.seed);
    let rgba = draw_diamond(width, height, &palette, 0);
    GeneratedAsset {
        width,
        height,
        stride: width * 4,
        rgba,
        provenance: provenance(input, "mock-shapes-1"),
    }
}

/// Draws an idle animation: the same diamond bobbing vertically across the frames,
/// returning to its start so the loop is seamless. Identity (palette) is constant.
fn draw_idle_animation(input: &GenerationJobInput, animation_id: &str, grid: Grid, fps: u16) -> GeneratedAnimation {
    let (width, height) = input.size;
    let palette = palette_from_prompt(&input.prompt, input.seed);
    let frame_count = grid.cell_count().max(1);
    let amplitude = i32::try_from(height / 12).unwrap_or(1).max(1);

    let frames = (0..frame_count)
        .map(|frame| {
            let offset = bob_offset(frame, frame_count, amplitude);
            GeneratedFrame {
                width,
                height,
                stride: width * 4,
                rgba: draw_diamond(width, height, &palette, offset),
            }
        })
        .collect();

    GeneratedAnimation {
        frames,
        clip_name: animation_id.to_owned(),
        fps,
        loop_mode: LoopMode::Loop,
        provenance: provenance(input, "mock-idle-1"),
    }
}

/// Renders one diamond, its centre shifted vertically by `center_y_offset` pixels.
fn draw_diamond(width: u32, height: u32, palette: &Palette, center_y_offset: i32) -> Vec<u8> {
    let center_x = width / 2;
    let base_center_y = i32::try_from(height / 2).unwrap_or(0);
    let center_y = base_center_y + center_y_offset;
    let radius = width.min(height) / 2;

    let mut rgba = Vec::with_capacity(width as usize * height as usize * 4);
    for y in 0..height {
        let dy = i32::try_from(y).unwrap_or(0).abs_diff(center_y);
        for x in 0..width {
            let dx = x.abs_diff(center_x);
            let inside = dx + dy <= radius;
            let color = if inside { palette.foreground } else { palette.background };
            rgba.extend_from_slice(&color);
        }
    }
    rgba
}

/// A smooth vertical bob that returns to its starting offset over `frame_count`
/// frames, so frame `n` meets frame 0 and the loop is seamless.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn bob_offset(frame: u32, frame_count: u32, amplitude: i32) -> i32 {
    if frame_count == 0 {
        return 0;
    }
    // One full sine cycle across the frames; clamped/rounded to whole pixels. The
    // cast back to i32 is bounded by `amplitude`, so it neither truncates nor wraps.
    let phase = f64::from(frame) / f64::from(frame_count);
    let wave = (phase * std::f64::consts::TAU).sin();
    (wave * f64::from(amplitude)).round() as i32
}

fn provenance(input: &GenerationJobInput, model: &str) -> GenerationProvenance {
    GenerationProvenance {
        prompt: input.prompt.clone(),
        seed: input.seed,
        provider_id: "mock".to_owned(),
        model: model.to_owned(),
        created_unix_ms: crate::now_ms(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use pixhaus_core::PixelBuffer;
    use pixhaus_services::job::{GenerationContext, ReferenceImage};

    fn anchor_input(prompt: &str) -> GenerationJobInput {
        GenerationJobInput {
            prompt: prompt.to_owned(),
            seed: 7,
            size: (16, 16),
            context: GenerationContext::NewAsset,
            kind: GenerationKind::Anchor,
        }
    }

    fn idle_input(prompt: &str) -> GenerationJobInput {
        GenerationJobInput {
            prompt: prompt.to_owned(),
            seed: 7,
            size: (16, 16),
            context: GenerationContext::NewAsset,
            kind: GenerationKind::IdleAnimation {
                reference: ReferenceImage {
                    width: 16,
                    height: 16,
                    stride: 64,
                    rgba: vec![0; 16 * 16 * 4],
                },
                animation_id: "idle".to_owned(),
                grid: Grid { cols: 4, rows: 2 },
                fps: 12,
            },
        }
    }

    #[test]
    fn anchor_is_deterministic_for_same_input() {
        let a = draw_anchor(&anchor_input("knight"));
        let b = draw_anchor(&anchor_input("knight"));
        assert_eq!(a.rgba, b.rgba);
    }

    #[test]
    fn different_prompts_produce_different_bytes() {
        let a = draw_anchor(&anchor_input("knight"));
        let b = draw_anchor(&anchor_input("goblin"));
        assert_ne!(a.rgba, b.rgba);
    }

    #[test]
    fn anchor_size_and_stride_satisfy_pixel_buffer() {
        let asset = draw_anchor(&anchor_input("x"));
        assert_eq!((asset.width, asset.height, asset.stride), (16, 16, 64));
        assert!(PixelBuffer::from_rgba8(asset.width, asset.height, asset.stride, asset.rgba).is_ok());
    }

    #[test]
    fn idle_has_one_frame_per_grid_cell() {
        let GenerationKind::IdleAnimation { grid, fps, animation_id, .. } = idle_input("bit").kind else {
            unreachable!("idle_input builds an IdleAnimation");
        };
        let animation = draw_idle_animation(&idle_input("bit"), &animation_id, grid, fps);
        assert_eq!(animation.frames.len(), 8, "4x2 grid yields eight frames");
        assert_eq!(animation.clip_name, "idle");
        assert_eq!(animation.fps, 12);
        assert_eq!(animation.loop_mode, LoopMode::Loop);
    }

    #[test]
    fn idle_is_deterministic_but_animated() {
        let a = draw_idle_animation(&idle_input("bit"), "idle", Grid { cols: 4, rows: 2 }, 12);
        let b = draw_idle_animation(&idle_input("bit"), "idle", Grid { cols: 4, rows: 2 }, 12);
        assert_eq!(a.frames[0].rgba, b.frames[0].rgba, "same input is reproducible");
        // The diamond bobs, so a mid-cycle frame differs from the first.
        assert_ne!(a.frames[0].rgba, a.frames[2].rgba, "the frame visibly moves");
    }

    #[test]
    fn idle_frames_satisfy_pixel_buffer() {
        let animation = draw_idle_animation(&idle_input("bit"), "idle", Grid { cols: 4, rows: 2 }, 12);
        for frame in &animation.frames {
            assert!(PixelBuffer::from_rgba8(frame.width, frame.height, frame.stride, frame.rgba.clone()).is_ok());
        }
    }
}
