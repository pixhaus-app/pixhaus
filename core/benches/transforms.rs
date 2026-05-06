//! Transform throughput benchmarks.
//!
//! Covers the transforms most likely to be performance-sensitive in a
//! live editor session:
//!
//! - `rotate_rotsprite_256` — RotSprite at 45° on a 256×256 buffer.
//!   The brief requires single-digit milliseconds; this benchmark
//!   surfaces any regression.
//! - `rotate_bilinear_256` — direct bilinear rotation as a baseline.
//! - `scale_nearest_256` — nearest-neighbor resize 256→512.
//! - `scale_integer_2x_256` — integer 2× upscale on 256×256.
//! - `flip_horizontal_256` — H-flip; should be close to a memcpy.
//! - `translate_256` — integer translate with no mask.
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

fn bench_rotsprite_256(c: &mut Criterion) {
    let buf = make_gradient(SIDE);
    let pixels = (SIDE * SIDE) as u64;
    let mut group = c.benchmark_group("transforms");
    group.throughput(Throughput::Elements(pixels));
    group.bench_function("rotate_rotsprite_256", |b| {
        b.iter(|| rotate_rotsprite(black_box(&buf), black_box(45.0)).unwrap())
    });
    group.finish();
}

fn bench_rotate_bilinear_256(c: &mut Criterion) {
    let buf = make_gradient(SIDE);
    let pixels = (SIDE * SIDE) as u64;
    let mut group = c.benchmark_group("transforms");
    group.throughput(Throughput::Elements(pixels));
    group.bench_function("rotate_bilinear_256", |b| {
        b.iter(|| rotate_bilinear(black_box(&buf), black_box(45_f32.to_radians())).unwrap())
    });
    group.finish();
}

fn bench_scale_nearest_256(c: &mut Criterion) {
    let buf = make_gradient(SIDE);
    let pixels = (SIDE * SIDE * 4) as u64; // output is 512×512
    let mut group = c.benchmark_group("transforms");
    group.throughput(Throughput::Elements(pixels));
    group.bench_function("scale_nearest_256_to_512", |b| {
        b.iter(|| scale_nearest(black_box(&buf), black_box(SIDE * 2), black_box(SIDE * 2)).unwrap())
    });
    group.finish();
}

fn bench_scale_integer_2x_256(c: &mut Criterion) {
    let buf = make_gradient(SIDE);
    let pixels = (SIDE * SIDE * 4) as u64;
    let mut group = c.benchmark_group("transforms");
    group.throughput(Throughput::Elements(pixels));
    group.bench_function("scale_integer_2x_256", |b| {
        b.iter(|| scale_integer(black_box(&buf), black_box(2)).unwrap())
    });
    group.finish();
}

fn bench_flip_horizontal_256(c: &mut Criterion) {
    let buf = make_gradient(SIDE);
    let pixels = (SIDE * SIDE) as u64;
    let mut group = c.benchmark_group("transforms");
    group.throughput(Throughput::Elements(pixels));
    group.bench_function("flip_horizontal_256", |b| {
        b.iter(|| flip_horizontal(black_box(&buf), black_box(None)).unwrap())
    });
    group.finish();
}

fn bench_translate_256(c: &mut Criterion) {
    let buf = make_gradient(SIDE);
    let pixels = (SIDE * SIDE) as u64;
    let mut group = c.benchmark_group("transforms");
    group.throughput(Throughput::Elements(pixels));
    group.bench_function("translate_256", |b| {
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
    benches,
    bench_rotsprite_256,
    bench_rotate_bilinear_256,
    bench_scale_nearest_256,
    bench_scale_integer_2x_256,
    bench_flip_horizontal_256,
    bench_translate_256,
);
criterion_main!(benches);
