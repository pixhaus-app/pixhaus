//! Per-pixel blend mode throughput benchmarks.
//!
//! For each blend mode, runs the per-pixel `blend` over a fixed-size
//! 256x256 backdrop+source pair. The benchmark reports throughput in
//! megapixels per second. Run with:
//!
//! ```text
//! cargo bench -p pixhaus-core --bench blend
//! ```
//!
//! The benchmark uses two simple gradients so the blend math hits
//! every code path (`b == 0`, `s == 255`, equal-channel, etc.) within
//! a single iteration.

#![allow(
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::disallowed_methods,
    clippy::expect_used,
    clippy::missing_panics_doc,
    clippy::panic,
    clippy::unwrap_used,
    missing_docs
)]

use std::hint::black_box;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use pixhaus_core::canvas::blend::blend;
use pixhaus_core::project::{BlendMode, Rgba};

const SIDE: usize = 256;
const PIXELS: usize = SIDE * SIDE;

fn make_gradient(seed: u32) -> Vec<Rgba> {
    (0..PIXELS as u32)
        .map(|i| {
            let v = i.wrapping_mul(seed) ^ (i.wrapping_mul(0x9e37_79b9));
            Rgba::new(
                (v & 0xff) as u8,
                ((v >> 8) & 0xff) as u8,
                ((v >> 16) & 0xff) as u8,
                ((v >> 24) & 0xff) as u8,
            )
        })
        .collect()
}

fn bench_mode(c: &mut Criterion, mode: BlendMode, label: &str) {
    let backdrop = make_gradient(0x1234_5678);
    let source = make_gradient(0x8765_4321);

    let mut group = c.benchmark_group(label);
    group.throughput(Throughput::Elements(PIXELS as u64));
    group.bench_function("256x256", |b| {
        b.iter(|| {
            let mut accum: u32 = 0;
            for (s, d) in source.iter().zip(backdrop.iter()) {
                let r = blend(mode, *s, *d, 255);
                accum = accum.wrapping_add(r.r as u32);
            }
            black_box(accum)
        });
    });
    group.finish();
}

fn benches(c: &mut Criterion) {
    bench_mode(c, BlendMode::Normal, "blend/normal");
    bench_mode(c, BlendMode::Multiply, "blend/multiply");
    bench_mode(c, BlendMode::Screen, "blend/screen");
    bench_mode(c, BlendMode::Overlay, "blend/overlay");
    bench_mode(c, BlendMode::SoftLight, "blend/soft_light");
    bench_mode(c, BlendMode::HardLight, "blend/hard_light");
    bench_mode(c, BlendMode::Difference, "blend/difference");
    bench_mode(c, BlendMode::ColorDodge, "blend/color_dodge");
    bench_mode(c, BlendMode::ColorBurn, "blend/color_burn");
    bench_mode(c, BlendMode::Hue, "blend/hue");
    bench_mode(c, BlendMode::Color, "blend/color");
    bench_mode(c, BlendMode::Luminosity, "blend/luminosity");
}

criterion_group!(blend_benches, benches);
criterion_main!(blend_benches);
