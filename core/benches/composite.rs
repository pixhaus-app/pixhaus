//! Multi-layer compositor throughput benchmark.
//!
//! Builds an N-layer stack of 256x256 RGBA buffers and composites
//! them onto a transparent backdrop. Reports throughput in megapixels
//! per second so changes to the rayon row-fan-out or the per-pixel
//! blend hot path show up immediately.
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
use pixhaus_core::canvas::{LayerInput, PixelBuffer, composite_layers};
use pixhaus_core::project::BlendMode;

const SIDE: u32 = 256;

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

fn bench_stack(c: &mut Criterion, layer_count: usize) {
    let buffers: Vec<PixelBuffer> = (0..layer_count)
        .map(|i| make_layer(0x1357_9bdf ^ (i as u32 * 0x9e37_79b9)))
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

fn benches(c: &mut Criterion) {
    bench_stack(c, 4);
    bench_stack(c, 16);
    bench_stack(c, 50);
}

criterion_group!(composite_benches, benches);
criterion_main!(composite_benches);
