//! Drawing hot-path microbenchmarks.
//!
//! These guard the per-move cost of the editor's drawing loop so a future
//! change that reintroduces O(canvas) or O(stroke) work shows up here. The two
//! brush benches are bounded by the brush footprint and must stay flat
//! regardless of canvas size; the composite benches show the dirty-rect win and
//! the rayon speedup at large rects.
//!
//! Run with `cargo bench -p pixhaus-core`.

// `criterion_group!`/`criterion_main!` expand to undocumented public items.
#![allow(missing_docs)]

use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;

use pixhaus_core::canvas::{BrushShape, LayerInput, PixelBuffer, composite_region, draw_line, paint_brush};
use pixhaus_core::project::{BlendMode, Rgba};

const CANVAS: u32 = 1024;

fn red() -> Rgba {
    Rgba::opaque(220, 40, 40)
}

/// A fixture buffer. The dimensions are valid, so the fallback never fires —
/// it only keeps the bench free of `unwrap`/`expect`.
fn buffer(width: u32, height: u32, color: Rgba) -> PixelBuffer {
    PixelBuffer::filled(width, height, color).unwrap_or_else(|_| PixelBuffer::empty())
}

/// One circle dab, size 40, on a 1024x1024 canvas. Cost is the brush footprint,
/// not the canvas — this must stay flat if the canvas grows to 8K.
fn bench_dab(c: &mut Criterion) {
    let mut buf = buffer(CANVAS, CANVAS, Rgba::transparent());
    c.bench_function("paint_brush circle size 40", |b| {
        b.iter(|| {
            paint_brush(&mut buf, black_box(512), black_box(512), red(), BrushShape::Circle, black_box(40));
        });
    });
}

/// One stroke segment (the per-move primitive), circle size 40. Bounded by the
/// segment length plus brush footprint, never the canvas.
fn bench_segment(c: &mut Criterion) {
    let mut buf = buffer(CANVAS, CANVAS, Rgba::transparent());
    c.bench_function("draw_line segment circle size 40", |b| {
        b.iter(|| {
            draw_line(
                &mut buf,
                black_box(500),
                black_box(500),
                black_box(540),
                black_box(520),
                red(),
                BrushShape::Circle,
                40,
            );
        });
    });
}

/// Recompositing a dirty rect across two layers, at the per-move dab size and at
/// large rects. The 48x48 case is the brush hot path (serial); the 256/1024
/// cases cross the rayon threshold and show the parallel composite.
fn bench_composite_region(c: &mut Criterion) {
    let bottom = buffer(CANVAS, CANVAS, Rgba::opaque(30, 30, 40));
    let top = buffer(CANVAS, CANVAS, Rgba::new(200, 80, 60, 160));
    let layers = [
        LayerInput {
            buffer: &bottom,
            mode: BlendMode::Normal,
            opacity: 255,
            visible: true,
        },
        LayerInput {
            buffer: &top,
            mode: BlendMode::Normal,
            opacity: 255,
            visible: true,
        },
    ];
    let mut dst = buffer(CANVAS, CANVAS, Rgba::transparent());

    let mut group = c.benchmark_group("composite_region");
    for side in [48u32, 256, 1024] {
        group.bench_function(format!("{side}x{side}"), |b| {
            b.iter(|| {
                composite_region(&mut dst, &layers, 0, 0, black_box(side), black_box(side));
            });
        });
    }
    group.finish();
}

criterion_group!(benches, bench_dab, bench_segment, bench_composite_region);
criterion_main!(benches);
