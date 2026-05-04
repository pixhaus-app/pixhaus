//! Throughput benchmarks for the `.pixhaus` encode/decode pipeline.
//!
//! Scenario: 100 frames of 256×256 RGBA pixel data — the target from the
//! S07 brief. The encode + decode round trip should complete in under 200 ms
//! on a recent laptop.
//!
//! Run with:
//!
//! ```text
//! cargo bench -p pixhaus-io
//! ```

#![allow(
    clippy::cast_possible_truncation,
    clippy::disallowed_methods,
    clippy::missing_panics_doc,
    clippy::unwrap_used,
    missing_docs
)]

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use pixhaus_core::project::Project;
use pixhaus_io::pixhaus::{PixelBufferEntry, PixhausArchive, decode, encode};

const FRAME_COUNT: usize = 100;
const FRAME_SIDE: u32 = 256;

/// Builds an archive with `FRAME_COUNT` frames of `FRAME_SIDE×FRAME_SIDE` RGBA pixels.
///
/// Uses a simple counter pattern so each buffer is distinct but still
/// compressible — similar to real sprite data.
fn make_archive() -> PixhausArchive {
    let project = Project::new("bench");
    let buffers: Vec<PixelBufferEntry> = (0..FRAME_COUNT)
        .map(|i| {
            let pixels: Vec<u8> = (0..(FRAME_SIDE * FRAME_SIDE * 4) as usize)
                .map(|j| ((i.wrapping_mul(7)).wrapping_add(j)) as u8)
                .collect();
            PixelBufferEntry {
                id: i as u32 + 1,
                width: FRAME_SIDE,
                height: FRAME_SIDE,
                stride: FRAME_SIDE * 4,
                pixels,
            }
        })
        .collect();
    PixhausArchive { project, buffers }
}

fn bench_encode(c: &mut Criterion) {
    let archive = make_archive();
    let raw_bytes: u64 = archive.buffers.iter().map(|b| b.pixels.len() as u64).sum();

    c.benchmark_group("pixhaus/encode")
        .throughput(Throughput::Bytes(raw_bytes))
        .bench_function("100x256x256_rgba", |b| {
            b.iter(|| encode(&archive).unwrap());
        });
}

fn bench_decode(c: &mut Criterion) {
    let archive = make_archive();
    let encoded = encode(&archive).unwrap();
    let raw_bytes: u64 = archive.buffers.iter().map(|b| b.pixels.len() as u64).sum();

    c.benchmark_group("pixhaus/decode")
        .throughput(Throughput::Bytes(raw_bytes))
        .bench_function("100x256x256_rgba", |b| {
            b.iter(|| decode(&encoded).unwrap());
        });
}

criterion_group!(benches, bench_encode, bench_decode);
criterion_main!(benches);
