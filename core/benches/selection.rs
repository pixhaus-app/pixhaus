//! Selection throughput benchmarks. Baseline measurements for the
//! S60 SIMD audit.
//!
//! Two magic_wand inputs:
//!
//! - `magic_wand_dense` — 256x256 buffer with one large connected
//!   blob filling more than half the canvas. The BFS frontier stays
//!   wide and the matched-pixel count is high.
//! - `magic_wand_sparse` — 256x256 buffer with many small
//!   disconnected blobs sharing a color. The seed lands in one blob;
//!   the BFS never reaches the others. Exercises the frontier
//!   short-circuit path.
//!
//! Run with:
//!
//! ```text
//! cargo bench -p pixhaus-core --bench selection
//! ```

#![allow(
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
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
use pixhaus_core::canvas::PixelBuffer;
use pixhaus_core::project::Rgba;
use pixhaus_core::selection::{Connectivity, magic_wand};

const SIDE: u32 = 256;

/// Builds a 256x256 buffer with a single connected blob: every pixel
/// inside a centred disc of radius `SIDE / 2 - 4` is opaque red,
/// everything else is opaque white. The disc covers roughly 78% of
/// the canvas (pi * r^2 / SIDE^2), which gives the BFS a wide
/// frontier for most of its lifetime.
fn make_dense() -> PixelBuffer {
    let mut buf = PixelBuffer::new(SIDE, SIDE).expect("buffer alloc");
    let cx = SIDE as i32 / 2;
    let cy = SIDE as i32 / 2;
    let r = SIDE as i32 / 2 - 4;
    let r_sq = r * r;
    let red = Rgba::opaque(255, 0, 0);
    let white = Rgba::opaque(255, 255, 255);
    for y in 0..SIDE {
        for x in 0..SIDE {
            let dx = x as i32 - cx;
            let dy = y as i32 - cy;
            let inside = dx * dx + dy * dy <= r_sq;
            buf.set_pixel(x, y, if inside { red } else { white });
        }
    }
    buf
}

/// Builds a 256x256 buffer with many small disconnected blobs sharing
/// the same color. Each blob is a 4x4 square on an 8-pixel grid, so
/// the canvas is half blob and half background but no blob touches
/// its neighbours. The seed picks the top-left blob; the BFS visits
/// 16 pixels and stops.
fn make_sparse() -> PixelBuffer {
    let mut buf = PixelBuffer::new(SIDE, SIDE).expect("buffer alloc");
    let red = Rgba::opaque(255, 0, 0);
    let white = Rgba::opaque(255, 255, 255);
    for y in 0..SIDE {
        for x in 0..SIDE {
            // Cell coords on an 8-pixel grid; the first 4 pixels of
            // each cell are blob, the next 4 are background.
            let cell_x = x % 8;
            let cell_y = y % 8;
            let blob = cell_x < 4 && cell_y < 4;
            buf.set_pixel(x, y, if blob { red } else { white });
        }
    }
    buf
}

fn bench_magic_wand_dense(c: &mut Criterion) {
    let buf = make_dense();
    let pixels = (SIDE * SIDE) as u64;
    let mut group = c.benchmark_group("selection");
    group.throughput(Throughput::Elements(pixels));
    group.bench_function("magic_wand/dense/256x256", |b| {
        b.iter(|| {
            // Seed at the centre, inside the blob.
            magic_wand(
                black_box(&buf),
                black_box(SIDE / 2),
                black_box(SIDE / 2),
                black_box(0),
                black_box(Connectivity::Four),
            )
            .unwrap()
        })
    });
    group.finish();
}

fn bench_magic_wand_sparse(c: &mut Criterion) {
    let buf = make_sparse();
    let pixels = (SIDE * SIDE) as u64;
    let mut group = c.benchmark_group("selection");
    group.throughput(Throughput::Elements(pixels));
    group.bench_function("magic_wand/sparse/256x256", |b| {
        b.iter(|| {
            // Seed inside the top-left blob; the BFS visits only that
            // 4x4 region because no other blob is connected.
            magic_wand(
                black_box(&buf),
                black_box(1),
                black_box(1),
                black_box(0),
                black_box(Connectivity::Four),
            )
            .unwrap()
        })
    });
    group.finish();
}

criterion_group!(
    selection_benches,
    bench_magic_wand_dense,
    bench_magic_wand_sparse,
);
criterion_main!(selection_benches);
