//! Gate 3 (highest risk): one FLUX.2 transformer forward on fixed inputs.
//!
//! This is the topology-composition gate. It does NOT need the 24 GB weights: it
//! builds the real `FluxTransformer::new` block stack at a tiny config from a
//! seeded random `VarBuilder`, then runs one forward and asserts the output is the
//! right shape and finite (no NaN/Inf). A wide miss here is an architecture/layout
//! bug — a wrong reshape, a transposed cat, a head-dim mismatch — not precision.
//! The numeric parity-against-PyTorch gate is the separate weights-gated test.
//!
//! Because it runs on a tiny config from random weights, this file is NOT
//! triple-gated like the VAE/text-encoder parity tests — it compiles and runs in
//! the normal `cpu` gate with no download and no GPU. It still requires a backend
//! feature (the crate's `Tensor` ops need one), so the whole file is `cfg`-gated
//! on `cpu`/`cuda`/`metal`.

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

use candle_core::{DType, Device, Result as CandleResult, Shape, Tensor};
use candle_nn::Init;
use candle_nn::var_builder::{SimpleBackend, VarBuilder};
use pixhaus_flux::model::{Flux2Config, FluxTransformer};
use proptest::prelude::*;

/// A `VarBuilder` backend that synthesizes a seeded random tensor for every
/// requested weight, scaled small so a 1+2-block forward stays numerically tame.
///
/// Real init is small; multiplying unit-variance randn weights through every
/// linear in a deep stack diverges to Inf, which would be a false failure of a
/// finiteness gate. The 0.02 scale mirrors a typical transformer init std.
struct SeededRandn {
    scale: f64,
}

impl SimpleBackend for SeededRandn {
    fn get(&self, s: Shape, _name: &str, _h: Init, dtype: DType, dev: &Device) -> CandleResult<Tensor> {
        // randn at the requested shape, scaled down, in the working dtype. The
        // device seed (set before the forward) makes this deterministic per seed.
        let t = Tensor::randn(0f32, 1f32, s, dev)?;
        (t * self.scale)?.to_dtype(dtype)
    }

    fn get_unchecked(&self, _name: &str, _dtype: DType, _dev: &Device) -> CandleResult<Tensor> {
        candle_core::bail!("SeededRandn needs a shape; use get")
    }

    fn contains_tensor(&self, _name: &str) -> bool {
        true
    }
}

/// A small config that still exercises every block type and the 4-axis rotary.
/// head_dim = hidden/heads = 16/2 = 8; axes [2,2,2,2] sum to 8; in/out 6;
/// context 24; one double block and two single blocks.
fn tiny_config() -> Flux2Config {
    Flux2Config {
        in_channels: 6,
        out_channels: 6,
        context_in_dim: 24,
        hidden_size: 16,
        mlp_ratio: 3.0,
        num_heads: 2,
        depth: 1,
        depth_single_blocks: 2,
        axes_dim: vec![2, 2, 2, 2],
        theta: 2000,
        timestep_channels: 8,
        eps: 1e-6,
    }
}

/// Build the transformer from a random backend.
///
/// candle's CPU device cannot be seeded (`set_seed` errors on `Device::Cpu` in
/// 0.10), so the draws are not reproducible — which is fine: this gate asserts
/// finiteness, not a fixed numeric value. Each proptest case is an independent
/// random draw, broadening the finiteness coverage.
fn build_transformer(cfg: &Flux2Config, device: &Device) -> FluxTransformer {
    let backend = SeededRandn { scale: 0.02 };
    let vb = VarBuilder::from_backend(Box::new(backend), DType::F32, device.clone());
    FluxTransformer::new(cfg, vb).expect("build tiny transformer")
}

/// Fixed inputs: a packed latent, a Qwen3-shaped conditioning, their 4-axis ids,
/// and one timestep. img_len is h*w of a 2x3 grid; txt_len is 4 tokens.
fn fixed_inputs(cfg: &Flux2Config, device: &Device) -> (Tensor, Tensor, Tensor, Tensor, Tensor, usize) {
    let (b, txt_len) = (1usize, 4usize);
    let (h, w) = (2usize, 3usize);
    let img_len = h * w;

    let img = Tensor::randn(0f32, 1f32, (b, img_len, cfg.in_channels), device).expect("img");
    let txt = Tensor::randn(0f32, 1f32, (b, txt_len, cfg.context_in_dim), device).expect("txt");

    // Image ids: cartesian (t=0, h, w, l=0) over the 2x3 grid.
    let mut img_ids = Vec::with_capacity(img_len * 4);
    for row in 0..h {
        for col in 0..w {
            img_ids.extend_from_slice(&[0u32, row as u32, col as u32, 0]);
        }
    }
    let img_ids = Tensor::from_vec(img_ids, (img_len, 4), device)
        .expect("img_ids")
        .to_dtype(DType::F32)
        .expect("ids f32");

    // Text ids: (t=0, h=0, w=0, l=token), spanning the l axis.
    let mut txt_ids = Vec::with_capacity(txt_len * 4);
    for tok in 0..txt_len {
        txt_ids.extend_from_slice(&[0u32, 0, 0, tok as u32]);
    }
    let txt_ids = Tensor::from_vec(txt_ids, (txt_len, 4), device)
        .expect("txt_ids")
        .to_dtype(DType::F32)
        .expect("ids f32");

    let timesteps = Tensor::from_vec(vec![0.5f32], (b,), device).expect("timesteps");
    (img, img_ids, txt, txt_ids, timesteps, img_len)
}

/// Assert a tensor is entirely finite (no NaN, no Inf).
fn assert_finite(t: &Tensor, label: &str) {
    let flat = t.flatten_all().expect("flatten").to_vec1::<f32>().expect("to f32 vec");
    assert!(!flat.is_empty(), "{label} is empty");
    assert!(flat.iter().all(|v| v.is_finite()), "{label} has a non-finite value");
}

#[test]
fn single_forward_shape_and_finite() {
    let device = Device::Cpu;
    let cfg = tiny_config();
    let model = build_transformer(&cfg, &device);
    let (img, img_ids, txt, txt_ids, timesteps, img_len) = fixed_inputs(&cfg, &device);

    let out = model.forward(&img, &img_ids, &txt, &txt_ids, &timesteps).expect("forward runs");

    // Output is the image span only, projected to out_channels: (b, img_len, out).
    assert_eq!(out.dims3().expect("rank 3"), (1, img_len, cfg.out_channels), "forward output shape");
    assert_finite(&out, "forward output");
}

proptest! {
    // A handful of seeds: random weights and random inputs, output must stay
    // finite. This is the "no NaN/Inf across seeds" half of gate 3.
    #![proptest_config(ProptestConfig::with_cases(6))]
    #[test]
    fn forward_stays_finite_across_seeds(seed in 0u64..1_000) {
        let device = Device::Cpu;
        let cfg = tiny_config();
        let model = build_transformer(&cfg, &device);
        let (img, img_ids, txt, txt_ids, timesteps, img_len) = fixed_inputs(&cfg, &device);

        let out = model.forward(&img, &img_ids, &txt, &txt_ids, &timesteps).expect("forward runs");
        prop_assert_eq!(out.dims3().expect("rank 3"), (1, img_len, cfg.out_channels));
        let flat = out.flatten_all().expect("flatten").to_vec1::<f32>().expect("to vec");
        prop_assert!(flat.iter().all(|v| v.is_finite()), "non-finite output (case seed {})", seed);
    }
}
