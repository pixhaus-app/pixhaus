//! VAE round-trip: encode a fixture, decode it, and confirm the decoded image
//! matches the input. The FLUX.2 VAE is lossy by design, so the bar is high but
//! not exact — `image-compare`'s MSSIM `>= 0.99`.
//!
//! Triple-gated so CI never needs the 168 MB weights or a GPU:
//! 1. The whole file compiles only under a backend feature
//!    (`cpu` / `cuda` / `metal`); a `--no-default-features` build skips it.
//! 2. `#[ignore]` — `cargo test` runs it only with `-- --ignored`.
//! 3. Early-return when `PIXHAUS_FLUX_WEIGHTS` is unset, so even
//!    `--ignored` is a no-op without a weights dir.
//!
//! `PIXHAUS_FLUX_WEIGHTS` points at the model store root — the directory that
//! holds `vae/diffusion_pytorch_model.safetensors`.

#![cfg(any(feature = "cpu", feature = "cuda", feature = "metal"))]

use std::path::PathBuf;

use candle_core::{DType, Device, IndexOp, Tensor};
use image::RgbImage;
use image_compare::Algorithm;
use pixhaus_flux::store::ModelStore;
use pixhaus_flux::vae::Vae;

const FIXTURE: &[u8] = include_bytes!("fixtures/vae_round_trip_64.png");

/// Decode the fixture PNG into `(rgb_bytes, width, height)`.
fn load_fixture() -> (Vec<u8>, u32, u32) {
    let img = image::load_from_memory(FIXTURE).expect("fixture PNG decodes").to_rgb8();
    let (w, h) = img.dimensions();
    (img.into_raw(), w, h)
}

/// HWC u8 RGB (`0..=255`) -> NCHW f32 in `-1..=1`, the VAE's input convention.
fn rgb_to_tensor(rgb: &[u8], w: u32, h: u32, device: &Device) -> Tensor {
    let (w, h) = (w as usize, h as usize);
    let hwc = Tensor::from_vec(rgb.to_vec(), (h, w, 3), device)
        .expect("build HWC tensor")
        .to_dtype(DType::F32)
        .expect("to f32");
    // 0..255 -> -1..1, then HWC -> CHW -> NCHW.
    let normalized = hwc.affine(2.0 / 255.0, -1.0).expect("normalize to -1..1");
    let chw = normalized.permute((2, 0, 1)).expect("HWC -> CHW");
    chw.unsqueeze(0).expect("CHW -> NCHW")
}

/// NCHW f32 in `-1..=1` -> HWC u8 RGB (`0..=255`).
fn tensor_to_rgb(t: &Tensor) -> (Vec<u8>, u32, u32) {
    let t = t.i(0).expect("drop batch dim"); // (3, h, w)
    let (_, h, w) = t.dims3().expect("CHW dims");
    let hwc = t.permute((1, 2, 0)).expect("CHW -> HWC");
    // -1..1 -> 0..255, clamp, round to u8.
    let scaled = hwc
        .clamp(-1.0_f64, 1.0_f64)
        .expect("clamp")
        .affine(127.5, 127.5)
        .expect("denormalize")
        .clamp(0.0_f64, 255.0_f64)
        .expect("clamp 0..255")
        .to_dtype(DType::U8)
        .expect("to u8");
    let bytes = scaled.flatten_all().expect("flatten").to_vec1::<u8>().expect("to bytes");
    (bytes, w as u32, h as u32)
}

#[test]
#[ignore = "requires downloaded FLUX.2 weights and a GPU"]
fn vae_round_trip_preserves_structure() {
    let Some(root) = std::env::var_os("PIXHAUS_FLUX_WEIGHTS") else {
        eprintln!("PIXHAUS_FLUX_WEIGHTS unset; skipping VAE round-trip");
        return;
    };
    let store = ModelStore::new(PathBuf::from(root));

    // CPU keeps the round-trip in f32 and deterministic across machines. A GPU
    // build would compute in bf16; the 0.99 bar holds either way, but the seed
    // only fixes the DiagonalGaussian sample on the device we run on.
    let device = Device::Cpu;
    device.set_seed(0).expect("seed device");
    let dtype = device.bf16_default_to_f32();

    let vae = Vae::load(&store, &device, dtype).expect("load FLUX.2 VAE");

    let (rgb, w, h) = load_fixture();
    let input = rgb_to_tensor(&rgb, w, h, &device).to_dtype(dtype).expect("input to working dtype");

    let latent = vae.encode(&input).expect("encode");
    // Latent shape sanity: 32 channels, 8x spatial downsample.
    let (_, lc, lh, lw) = latent.dims4().expect("latent dims");
    assert_eq!(lc, 32, "FLUX.2 latent has 32 channels");
    assert_eq!((lh, lw), (h as usize / 8, w as usize / 8), "8x downsample");

    let decoded = vae.decode(&latent).expect("decode");
    let (out_rgb, ow, oh) = tensor_to_rgb(&decoded);
    assert_eq!((ow, oh), (w, h), "decoded image matches input size");

    let first = RgbImage::from_raw(w, h, rgb).expect("input RgbImage");
    let second = RgbImage::from_raw(ow, oh, out_rgb).expect("decoded RgbImage");
    let result = image_compare::rgb_similarity_structure(&Algorithm::MSSIMSimple, &first, &second).expect("compare images");

    assert!(result.score >= 0.99, "VAE round-trip MSSIM {} below 0.99", result.score);
}
