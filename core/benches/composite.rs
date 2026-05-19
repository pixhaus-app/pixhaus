//! Compositor throughput benchmarks. Baseline measurements for the
//! S60 SIMD audit.
//!
//! Two flights:
//!
//! 1. Per-mode single-layer composite at 256x256 for `Normal`,
//!    `Multiply`, and `Overlay`. These isolate the per-pixel blend
//!    cost so future SIMD work has a clean before/after target.
//! 2. N-layer stack composite that exercises the rayon row fan-out
//!    in `composite::composite_layers`. Kept from the pre-S60 suite.
//!
//! Throughput is reported in pixels (or pixel-layer products for the
//! stack flight) so the numbers are directly comparable.
//!
//! Run with:
//!
//! ```text
//! cargo bench -p pixhaus-core --bench composite
//! ```

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
use pixhaus_core::canvas::{LayerInput, PixelBuffer, composite_layers, composite_onto};
use pixhaus_core::project::BlendMode;

const SIDE: u32 = 256;

/// Hashes the pixel index into pseudo-random RGBA so every blend mode
/// hits all of its branches in one iteration. The hash is deterministic
/// across runs.
fn make_layer(seed: u32) -> PixelBuffer {
    let mut buf = PixelBuffer::new(SIDE, SIDE).expect("buffer alloc");
    let bytes = buf.as_bytes_mut();
    for (i, chunk) in bytes.chunks_exact_mut(4).enumerate() {
        let v = (i as u32).wrapping_mul(seed) ^ (i as u32).wrapping_mul(0x9e37_79b9);
        chunk[0] = (v & 0xff) as u8;
        chunk[1] = ((v >> 8) & 0xff) as u8;
        chunk[2] = ((v >> 16) & 0xff) as u8;
        chunk[3] = ((v >> 24) & 0xff) as u8;
    }
    buf
}

/// Per-mode 256x256 composite. Allocates the backdrop and source
/// once per bench, then composites the source onto a fresh copy of
/// the backdrop per iteration. The copy isolates the bench from
/// any in-place state and keeps every iteration identical.
fn bench_mode(c: &mut Criterion, mode: BlendMode, label: &str) {
    let src = make_layer(0x1357_9bdf);
    let dst_template = make_layer(0xa5a5_a5a5);

    let mut group = c.benchmark_group(label);
    group.throughput(Throughput::Elements((SIDE * SIDE) as u64));
    group.bench_function("256x256", |b| {
        b.iter(|| {
            let mut dst = dst_template.clone();
            composite_onto(
                &mut dst,
                &LayerInput {
                    buffer: black_box(&src),
                    mode: black_box(mode),
                    opacity: black_box(255),
                    visible: true,
                },
            )
            .expect("composite_onto");
            // Touch one output byte so LLVM cannot drop the whole
            // composite as dead code on the iter return.
            black_box(dst.as_bytes()[0])
        });
    });
    group.finish();
}

fn bench_normal_composite(c: &mut Criterion) {
    bench_mode(c, BlendMode::Normal, "composite/normal");
}

fn bench_multiply_composite(c: &mut Criterion) {
    bench_mode(c, BlendMode::Multiply, "composite/multiply");
}

fn bench_overlay_composite(c: &mut Criterion) {
    bench_mode(c, BlendMode::Overlay, "composite/overlay");
}

/// N-layer stack throughput. Exercises the rayon row fan-out at
/// realistic layer counts. Inherited from the pre-S60 bench so a
/// future SIMD stream can also compare on this flight.
fn bench_stack(c: &mut Criterion, layer_count: usize) {
    let buffers: Vec<PixelBuffer> = (0..layer_count)
        .map(|i| make_layer(0x1357_9bdf ^ (i as u32).wrapping_mul(0x9e37_79b9)))
        .collect();
    let inputs: Vec<LayerInput<'_>> = buffers
        .iter()
        .map(|b| LayerInput {
            buffer: b,
            mode: BlendMode::Normal,
            opacity: 200,
            visible: true,
        })
        .collect();

    let label = format!("composite/{layer_count}_layers/256x256");
    let mut group = c.benchmark_group(label);
    let pixel_count = SIDE as u64 * SIDE as u64 * layer_count as u64;
    group.throughput(Throughput::Elements(pixel_count));
    group.bench_function("normal", |b| {
        b.iter(|| {
            let result = composite_layers(SIDE, SIDE, &inputs).expect("composite");
            black_box(result.as_bytes()[0])
        });
    });
    group.finish();
}

fn bench_stack_4(c: &mut Criterion) {
    bench_stack(c, 4);
}
fn bench_stack_16(c: &mut Criterion) {
    bench_stack(c, 16);
}
fn bench_stack_50(c: &mut Criterion) {
    bench_stack(c, 50);
}

criterion_group!(
    composite_benches,
    bench_normal_composite,
    bench_multiply_composite,
    bench_overlay_composite,
    bench_stack_4,
    bench_stack_16,
    bench_stack_50,
);
criterion_main!(composite_benches);
