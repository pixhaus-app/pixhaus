//! Stroke rasterization throughput benchmarks.
//!
//! Three flights, all documenting the freehand-drawing hot path that has to
//! stay responsive up to an 8K canvas:
//!
//! 1. `draw_stroke` across brush shapes (pixel / circle / square), sizes
//!    (1, 8, 32) and point counts (100, 1000) — raw stamp throughput.
//! 2. A long (2000-point) stroke processed in small batches, comparing the
//!    incremental `stamp_segment` path the editor now uses against the old
//!    "re-rasterize the whole accumulated stroke every batch" behavior. The
//!    incremental flight is roughly flat per batch (linear overall); the
//!    cumulative flight is the O(n^2) trap the editor used to hit.
//! 3. Whole-frame `composite_layers` across canvas sizes, showing that
//!    full-frame compositing cost grows with the square of the side — which
//!    is exactly why the editor composites only the dirty tile per brush
//!    move instead of the whole frame.
//!
//! Run with:
//!
//! ```text
//! cargo bench -p pixhaus-core --bench stroke
//! ```

#![allow(
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::disallowed_methods,
    clippy::expect_used,
    clippy::items_after_statements,
    clippy::many_single_char_names,
    clippy::missing_panics_doc,
    missing_docs
)]

use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use pixhaus_core::canvas::tools::{BrushShape, draw_stroke, stamp_segment};
use pixhaus_core::canvas::{LayerInput, PixelBuffer, composite_layers};
use pixhaus_core::project::{BlendMode, Rgba};

const SIDE: u32 = 512;

/// A sine wave spanning the canvas — representative of a freehand drag and
/// guaranteed in-bounds for a `SIDE`-square buffer.
fn wiggly_points(n: usize) -> Vec<[f32; 2]> {
    let w = SIDE as f32;
    (0..n)
        .map(|i| {
            let t = i as f32 / (n.max(1) as f32);
            let x = 4.0 + t * (w - 8.0);
            let y = w * 0.5 + (t * std::f32::consts::TAU * 6.0).sin() * (w * 0.4);
            [x, y]
        })
        .collect()
}

fn bench_draw_stroke(c: &mut Criterion) {
    let color = Rgba::opaque(200, 60, 180);
    let template = PixelBuffer::new(SIDE, SIDE).expect("buffer alloc");

    for (shape, sname) in [
        (BrushShape::Pixel, "pixel"),
        (BrushShape::Circle, "circle"),
        (BrushShape::Square, "square"),
    ] {
        let mut group = c.benchmark_group(format!("draw_stroke/{sname}"));
        for &count in &[100usize, 1000] {
            let points = wiggly_points(count);
            for &size in &[1u32, 8, 32] {
                group.throughput(Throughput::Elements(count as u64));
                group.bench_function(
                    BenchmarkId::from_parameter(format!("size{size}_pts{count}")),
                    |b| {
                        b.iter(|| {
                            let mut buf = template.clone();
                            draw_stroke(&mut buf, black_box(&points), color, shape, size, false);
                            black_box(buf.as_bytes()[0])
                        });
                    },
                );
            }
        }
        group.finish();
    }
}

fn bench_incremental_vs_cumulative(c: &mut Criterion) {
    let color = Rgba::opaque(0, 0, 0);
    const COUNT: usize = 2000;
    const BATCH: usize = 5;
    let points = wiggly_points(COUNT);
    let template = PixelBuffer::new(SIDE, SIDE).expect("buffer alloc");

    let mut group = c.benchmark_group("stroke/long_2000pts_circle8");
    group.throughput(Throughput::Elements(COUNT as u64));

    // What the editor does now: stamp only each new batch, bridging from the
    // previous point. Total work is linear in the number of points.
    group.bench_function("incremental", |b| {
        b.iter(|| {
            let mut buf = template.clone();
            let mut last: Option<[f32; 2]> = None;
            for batch in points.chunks(BATCH) {
                stamp_segment(
                    &mut buf,
                    last,
                    black_box(batch),
                    color,
                    BrushShape::Circle,
                    8,
                );
                if let Some(p) = batch.last() {
                    last = Some(*p);
                }
            }
            black_box(buf.as_bytes()[0])
        });
    });

    // What the editor used to do: re-rasterize every accumulated point from
    // the pre-stroke buffer on every batch — O(n^2) over the stroke.
    group.bench_function("cumulative_rerasterize", |b| {
        b.iter(|| {
            let mut buf = template.clone();
            let mut upto = 0usize;
            for batch in points.chunks(BATCH) {
                upto += batch.len();
                let mut work = template.clone();
                draw_stroke(
                    &mut work,
                    black_box(&points[..upto]),
                    color,
                    BrushShape::Circle,
                    8,
                    false,
                );
                buf = work;
            }
            black_box(buf.as_bytes()[0])
        });
    });
    group.finish();
}

fn bench_composite_canvas_sizes(c: &mut Criterion) {
    // Whole-frame compositing of a 3-layer stack at growing canvas sizes.
    // Cost scales with side^2, so a per-brush-move full-frame composite gets
    // catastrophic at 8K — the reason the editor composites per dirty tile.
    let mut group = c.benchmark_group("composite/whole_frame_3_layers");
    for &side in &[256u32, 1024, 4096] {
        let layers: Vec<PixelBuffer> = (0..3)
            .map(|_| PixelBuffer::filled(side, side, Rgba::opaque(40, 80, 160)).expect("alloc"))
            .collect();
        let inputs: Vec<LayerInput<'_>> = layers
            .iter()
            .map(|buffer| LayerInput {
                buffer,
                mode: BlendMode::Normal,
                opacity: 200,
                visible: true,
            })
            .collect();
        group.throughput(Throughput::Elements((side as u64) * (side as u64) * 3));
        group.bench_function(BenchmarkId::from_parameter(format!("{side}x{side}")), |b| {
            b.iter(|| {
                let out = composite_layers(side, side, black_box(&inputs)).expect("composite");
                black_box(out.as_bytes()[0])
            });
        });
    }
    group.finish();
}

criterion_group!(
    stroke_benches,
    bench_draw_stroke,
    bench_incremental_vs_cumulative,
    bench_composite_canvas_sizes,
);
criterion_main!(stroke_benches);
