//! Qwen3 text-encoder parity: encode a fixed prompt and snapshot the shape and
//! redacted statistics of the conditioning tensor the FLUX.2 `DiT` consumes.
//!
//! The snapshot pins the load-bearing contract — the `(1, seq, 7680)` shape and a
//! healthy, finite mean/std — without pinning exact float values, which drift
//! across machines and between the CPU (f32) and GPU (bf16) working dtypes. The
//! mean and std are redacted to a coarse bucket so the snapshot stays stable
//! while still catching a dead-zero or NaN encoder.
//!
//! Triple-gated so CI never needs the 8 GB text-encoder weights or a GPU, exactly
//! like the VAE round-trip:
//! 1. The whole file compiles only under a backend feature
//!    (`cpu` / `cuda` / `metal`); a `--no-default-features` build skips it.
//! 2. `#[ignore]` — `cargo test` runs it only with `-- --ignored`.
//! 3. Early-return when `PIXHAUS_FLUX_WEIGHTS` is unset, so even `--ignored` is a
//!    no-op without a weights dir.
//!
//! `PIXHAUS_FLUX_WEIGHTS` points at the model store root — the directory that
//! holds `text_encoder/` and `tokenizer/`.

#![cfg(any(feature = "cpu", feature = "cuda", feature = "metal"))]
// Test-only relaxations. The workspace floor denies unwrap/expect/panic and the
// clippy.toml disallowed-methods list bans unwrap/expect; this self-contained
// gate test builds its own fixtures and tensors, where unwrapping a just-built
// value and index/shape casts are idiomatic. Mirrors ai/tests/local_flux_backend.rs.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::disallowed_methods,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::doc_markdown
)]

use std::path::PathBuf;

use candle_core::Device;
use pixhaus_flux::store::ModelStore;
use pixhaus_flux::text_encoder::TextEncoder;

const PROMPT: &str = "a small orange robot mascot waving, pixel art";

/// Coarse, machine-stable bucket for a float: sign plus order of magnitude. This
/// is the redaction — it survives the bf16/f32 split and host-to-host drift while
/// still failing loudly on a degenerate (zero / NaN / exploded) statistic.
fn bucket(value: f32) -> String {
    if !value.is_finite() {
        return "non-finite".to_owned();
    }
    let mag = value.abs();
    let sign = if value < 0.0 { "neg" } else { "pos" };
    let order = if mag == 0.0 {
        "zero".to_owned()
    } else {
        // floor(log10(mag)) as a decade label, e.g. 1e0, 1e-1, 1e1.
        format!("1e{}", mag.log10().floor() as i32)
    };
    format!("{sign} ~{order}")
}

#[test]
#[ignore = "requires downloaded FLUX.2 weights"]
fn text_encoder_emits_klein_conditioning() {
    let Some(root) = std::env::var_os("PIXHAUS_FLUX_WEIGHTS") else {
        eprintln!("PIXHAUS_FLUX_WEIGHTS unset; skipping Qwen3 text-encoder parity");
        return;
    };
    let store = ModelStore::new(PathBuf::from(root));

    // CPU keeps the encode in f32 and deterministic across machines; the trunk
    // holds no random state, so no seed is needed.
    let device = Device::Cpu;
    let dtype = device.bf16_default_to_f32();

    let encoder = TextEncoder::load(&store, &device, dtype).expect("load Qwen3 text encoder");
    let cond = encoder.encode(PROMPT).expect("encode prompt");

    let (b, seq, width) = cond.dims3().expect("conditioning is (b, seq, width)");
    assert_eq!(b, 1, "single prompt -> batch 1");
    assert_eq!(width, 7680, "klein conditioning is 3 layers x 2560 = 7680");
    assert!(seq > 0, "tokenized prompt is non-empty");

    // Statistics over the f32 view of the whole tensor.
    let f = cond.to_dtype(candle_core::DType::F32).expect("to f32");
    let mean = f.mean_all().expect("mean").to_scalar::<f32>().expect("mean scalar");
    let sq = f
        .sqr()
        .expect("square")
        .mean_all()
        .expect("mean sq")
        .to_scalar::<f32>()
        .expect("mean sq scalar");
    let var = (f64::from(sq) - f64::from(mean) * f64::from(mean)).max(0.0);
    let std = var.sqrt() as f32;

    assert!(mean.is_finite() && std.is_finite(), "stats must be finite (mean {mean}, std {std})");
    assert!(std > 0.0, "a live encoder has spread; std was {std}");

    // Snapshot the shape exactly and the stats redacted to a bucket. `seq` varies
    // with the chat-template token count, so record it as part of the contract
    // rather than redacting it — it is deterministic for a fixed prompt + tokenizer.
    let report = format!("shape = (1, {seq}, 7680)\nmean = {}\nstd = {}\n", bucket(mean), bucket(std));
    insta::assert_snapshot!("klein_conditioning_stats", report);
}
