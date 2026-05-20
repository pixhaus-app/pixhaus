//! Transform throughput benchmarks. Baseline measurements for the
//! S60 SIMD audit.
//!
//! S60-mandated benches:
//!
//! - `bench_rotate_bilinear_45` — `rotate_bilinear` at 45 degrees on
//!   a 256x256 gradient. The bilinear core is the rank-9 transform
//!   loop in the audit.
//! - `bench_scale_nearest_2x` — `scale_nearest` 256 → 512.
//! - `bench_scale_nearest_1_5x` — `scale_nearest` 256 → 384.
//!   Substitutes for the spec's `bench_scale_bilinear_1_5x`: the
//!   current public scale API exposes nearest only. The audit doc
//!   notes the substitution.
//!
//! Inherited benches:
//!
//! - `bench_rotsprite_256` — full RotSprite path at 45 degrees.
//! - `bench_scale_integer_2x_256` — integer 2x upscale.
//! - `bench_flip_horizontal_256` — H-flip; close to a memcpy.
//! - `bench_translate_256` — integer translate with no mask.
//!
//! Run with:
//!
//! ```text
//! cargo bench -p pixhaus-core --bench transforms
//! ```

#![allow(
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::disallowed_methods,
    clippy::doc_markdown,
    clippy::expect_used,
    clippy::missing_panics_doc,
    clippy::panic,
    clippy::semicolon_if_nothing_returned,
    clippy::unwrap_used,
    missing_docs
)]

use std::hint::black_box;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use pixhaus_core::canvas::buffer::PixelBuffer;
use pixhaus_core::project::Rgba;
use pixhaus_core::transforms::{
    flip_horizontal, rotate_bilinear, rotate_rotsprite, scale_integer, scale_nearest, translate,
};

const SIDE: u32 = 256;

fn make_gradient(side: u32) -> PixelBuffer {
    let mut buf = PixelBuffer::new(side, side).unwrap();
    for y in 0..side {
        for x in 0..side {
            buf.set_pixel(
                x,
                y,
                Rgba::new(
                    (x * 255 / (side - 1)) as u8,
                    (y * 255 / (side - 1)) as u8,
                    128,
                    255,
                ),
            );
        }
    }
    buf
}

// --- S60 mandated benches --------------------------------------------------

fn bench_rotate_bilinear_45(c: &mut Criterion) {
    let buf = make_gradient(SIDE);
    let pixels = (SIDE * SIDE) as u64;
    let mut group = c.benchmark_group("transforms");
    group.throughput(Throughput::Elements(pixels));
    group.bench_function("rotate_bilinear/256x256/45deg", |b| {
        b.iter(|| rotate_bilinear(black_box(&buf), black_box(45_f32.to_radians())).unwrap())
    });
    group.finish();
}

fn bench_scale_nearest_2x(c: &mut Criterion) {
    let buf = make_gradient(SIDE);
    let pixels = (SIDE * 2 * SIDE * 2) as u64;
    let mut group = c.benchmark_group("transforms");
    group.throughput(Throughput::Elements(pixels));
    group.bench_function("scale_nearest/256_to_512", |b| {
        b.iter(|| scale_nearest(black_box(&buf), black_box(SIDE * 2), black_box(SIDE * 2)).unwrap())
    });
    group.finish();
}

/// Substitutes for `bench_scale_bilinear_1_5x` in the S60 spec. The
/// public scale API exposes nearest only today; the audit doc records
/// the substitution and the conditions under which a bilinear scale
/// bench arrives.
fn bench_scale_nearest_1_5x(c: &mut Criterion) {
    let buf = make_gradient(SIDE);
    let new_dim = SIDE + SIDE / 2; // 384
    let pixels = (new_dim * new_dim) as u64;
    let mut group = c.benchmark_group("transforms");
    group.throughput(Throughput::Elements(pixels));
    group.bench_function("scale_nearest/256_to_384", |b| {
        b.iter(|| scale_nearest(black_box(&buf), black_box(new_dim), black_box(new_dim)).unwrap())
    });
    group.finish();
}

// --- Inherited benches ------------------------------------------------------

fn bench_rotsprite_256(c: &mut Criterion) {
    let buf = make_gradient(SIDE);
    let pixels = (SIDE * SIDE) as u64;
    let mut group = c.benchmark_group("transforms");
    group.throughput(Throughput::Elements(pixels));
    group.bench_function("rotate_rotsprite/256x256/45deg", |b| {
        b.iter(|| rotate_rotsprite(black_box(&buf), black_box(45.0)).unwrap())
    });
    group.finish();
}

fn bench_scale_integer_2x_256(c: &mut Criterion) {
    let buf = make_gradient(SIDE);
    let pixels = (SIDE * SIDE * 4) as u64;
    let mut group = c.benchmark_group("transforms");
    group.throughput(Throughput::Elements(pixels));
    group.bench_function("scale_integer/2x/256", |b| {
        b.iter(|| scale_integer(black_box(&buf), black_box(2)).unwrap())
    });
    group.finish();
}

fn bench_flip_horizontal_256(c: &mut Criterion) {
    let buf = make_gradient(SIDE);
    let pixels = (SIDE * SIDE) as u64;
    let mut group = c.benchmark_group("transforms");
    group.throughput(Throughput::Elements(pixels));
    group.bench_function("flip_horizontal/256", |b| {
        b.iter(|| flip_horizontal(black_box(&buf), black_box(None)).unwrap())
    });
    group.finish();
}

fn bench_translate_256(c: &mut Criterion) {
    let buf = make_gradient(SIDE);
    let pixels = (SIDE * SIDE) as u64;
    let mut group = c.benchmark_group("transforms");
    group.throughput(Throughput::Elements(pixels));
    group.bench_function("translate/256", |b| {
        b.iter(|| {
            translate(
                black_box(&buf),
                black_box(10),
                black_box(10),
                black_box(None),
            )
            .unwrap()
        })
    });
    group.finish();
}

criterion_group!(
    transforms_benches,
    bench_rotate_bilinear_45,
    bench_scale_nearest_2x,
    bench_scale_nearest_1_5x,
    bench_rotsprite_256,
    bench_scale_integer_2x_256,
    bench_flip_horizontal_256,
    bench_translate_256,
);
criterion_main!(transforms_benches);
