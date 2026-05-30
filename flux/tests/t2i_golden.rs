//! Gate 4: the full 4-step text-to-image path against a golden image.
//!
//! Fixed prompt + fixed seed -> one PNG, compared to a committed golden with
//! `image-compare`'s MSSIM at the `>= 0.999` bar. The test isolates the sampler
//! from the VAE in two stages, per the gate-4 plan:
//!
//! 1. **Latent stage.** Run the pipeline's public `denoise` on the seeded noise
//!    and the real Qwen3 conditioning, and — when a golden latent `.safetensors`
//!    is present — compare the sampled latent to it directly. A wide miss here is
//!    a sampler/topology bug, not a VAE one.
//! 2. **Image stage.** Drive the whole `LoadedModel::text_to_image` path and
//!    `image-compare` the decoded PNG to the golden PNG.
//!
//! Triple-gated so GPU-less CI stays green, exactly like the VAE/text-encoder
//! gates:
//! 1. The whole file compiles only under a backend feature
//!    (`cpu` / `cuda` / `metal`); a `--no-default-features` build skips it.
//! 2. `#[ignore]` — `cargo test` runs it only with `-- --ignored`.
//! 3. Early-return when `PIXHAUS_FLUX_WEIGHTS` is unset, so even `--ignored` is a
//!    no-op without a weights dir.
//!
//! The golden fixtures are produced once from the PyTorch reference at this exact
//! prompt/seed/size and dropped into `flux/tests/fixtures/`. A maintainer with
//! weights generates them; until a golden is present the corresponding stage
//! self-skips with a message rather than asserting against a fabricated file.
//!
//! `PIXHAUS_FLUX_WEIGHTS` points at the model store root — the directory that
//! holds `transformer/`, `vae/`, `text_encoder/`, and `tokenizer/`.

#![cfg(any(feature = "cpu", feature = "cuda", feature = "metal"))]

use std::path::{Path, PathBuf};

use candle_core::{DType, Device, Tensor};
use image::RgbImage;
use image_compare::Algorithm;
use pixhaus_flux::loader::{FluxRequest, LoadedModel};
use pixhaus_flux::pipeline;
use pixhaus_flux::store::ModelStore;
use pixhaus_flux::text_encoder::TextEncoder;
use pixhaus_flux::vae::Vae;

/// The fixed generation inputs. Keep these in lockstep with the script that dumps
/// the golden fixtures — change one, regenerate the goldens.
const PROMPT: &str = "a small orange robot mascot waving, pixel art";
const SEED: u64 = 0;
const WIDTH: u32 = 64;
const HEIGHT: u32 = 64;

/// The golden PNG, produced from the reference pipeline at the inputs above.
const GOLDEN_PNG: &str = "fixtures/t2i_golden_64.png";
/// The golden sampled latent `(num_images, lh*lw, 128)`, dumped from the
/// reference before its VAE decode, to isolate the sampler.
const GOLDEN_LATENT: &str = "fixtures/t2i_golden_latent.safetensors";

/// Resolve a fixture path next to this test file.
fn fixture(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests").join(rel)
}

/// Decode a PNG file into `(rgb_bytes, width, height)`.
fn load_png(path: &Path) -> (Vec<u8>, u32, u32) {
    let img = image::open(path).expect("golden PNG decodes").to_rgb8();
    let (w, h) = img.dimensions();
    (img.into_raw(), w, h)
}

#[test]
#[ignore = "requires downloaded FLUX.2 weights and a GPU"]
fn t2i_matches_golden() {
    let Some(root) = std::env::var_os("PIXHAUS_FLUX_WEIGHTS") else {
        eprintln!("PIXHAUS_FLUX_WEIGHTS unset; skipping t2i golden gate");
        return;
    };
    let store = ModelStore::new(PathBuf::from(root));

    // CPU keeps the run in f32 and deterministic across machines; a GPU build
    // computes in bf16, where the 0.999 bar is the bf16-compute tolerance. The
    // device seed fixes the noise draw (CPU cannot be seeded in candle 0.10, so
    // the CPU latent stage relies on the committed golden, not reproducibility).
    let device = Device::Cpu;
    let _ = device.set_seed(SEED);
    let dtype = device.bf16_default_to_f32();

    // --- Stage 1: latent isolation -----------------------------------------
    // Rebuild the sampler inputs by hand and run the public `denoise`, then
    // compare the latent to the golden dump when present. This pins the sampler
    // and transformer topology independently of the VAE.
    let (lw_w, lw) = pipeline::latent_grid(WIDTH);
    let (lh_h, lh) = pipeline::latent_grid(HEIGHT);
    let want = (WIDTH as usize, HEIGHT as usize);
    assert_eq!((lw_w, lh_h), want, "64 is already a multiple of the 16x downsample");

    let text_encoder = TextEncoder::load(&store, &device, dtype).expect("load Qwen3 text encoder");
    let vae = Vae::load(&store, &device, dtype).expect("load FLUX.2 VAE");
    let transformer = pixhaus_flux::model::FluxTransformer::load(&store, &device, dtype).expect("load FLUX.2 transformer");

    let txt = text_encoder.encode(PROMPT).expect("encode prompt").to_dtype(dtype).expect("txt dtype");
    let seq = txt.dim(1).expect("txt seq");
    let txt_ids = pipeline::text_position_ids(seq, &device).expect("txt ids");
    let img_ids = pipeline::image_position_ids(lh, lw, &device).expect("img ids");

    let channels = vae.config().patched_latent_channels();
    let noise = pipeline::initial_noise(1, lh, lw, channels, &device, dtype).expect("noise");
    let timesteps = pipeline::schedule(pipeline::DISTILLED_STEPS);
    assert_eq!(timesteps.len(), pipeline::DISTILLED_STEPS + 1, "4 steps -> 5 schedule points");

    let latent = pipeline::denoise(&transformer, &noise, &img_ids, &txt, &txt_ids, &timesteps, |_, _| true).expect("denoise");
    assert_eq!(latent.dims3().expect("latent rank 3"), (1, lh * lw, channels), "latent shape");
    assert_finite(&latent, "sampled latent");

    let golden_latent_path = fixture(GOLDEN_LATENT);
    if golden_latent_path.exists() {
        compare_latent(&latent, &golden_latent_path, &device);
    } else {
        eprintln!("golden latent {golden_latent_path:?} absent; skipping latent-isolation compare (image stage still runs)");
    }

    // --- Stage 2: full path + image compare --------------------------------
    let mut model = LoadedModel::load(&store, pixhaus_flux::DeviceChoice::Cpu).expect("load full model");
    let req = FluxRequest {
        prompt: PROMPT.to_owned(),
        width: WIDTH,
        height: HEIGHT,
        seed: SEED,
        num_images: 1,
        ..Default::default()
    };
    let images = model.text_to_image(&req, |_, _| true).expect("text_to_image");
    assert_eq!(images.len(), 1, "one image requested -> one PNG");

    let produced = image::load_from_memory(&images[0]).expect("produced PNG decodes").to_rgb8();
    let (pw, ph) = produced.dimensions();
    assert_eq!((pw, ph), (WIDTH, HEIGHT), "produced image matches the rounded request");

    let golden_png_path = fixture(GOLDEN_PNG);
    if !golden_png_path.exists() {
        eprintln!("golden PNG {golden_png_path:?} absent; skipping image compare.");
        eprintln!("generate it from the reference at prompt={PROMPT:?} seed={SEED} {WIDTH}x{HEIGHT}.");
        return;
    }
    let (golden_rgb, gw, gh) = load_png(&golden_png_path);
    assert_eq!((gw, gh), (WIDTH, HEIGHT), "golden image size matches the request");

    let golden = RgbImage::from_raw(gw, gh, golden_rgb).expect("golden RgbImage");
    let result = image_compare::rgb_similarity_structure(&Algorithm::MSSIMSimple, &golden, &produced).expect("compare images");
    assert!(result.score >= 0.999, "t2i MSSIM {} below 0.999 vs golden", result.score);
}

/// Load a golden latent and assert the sampled latent matches it within a
/// bf16-compute tolerance. Compared as a normalized correlation plus a max-abs
/// bound so the check is robust to the working-dtype split.
fn compare_latent(latent: &Tensor, golden_path: &Path, device: &Device) {
    let tensors = candle_core::safetensors::load(golden_path, device).expect("load golden latent");
    let golden = tensors
        .get("latent")
        .or_else(|| tensors.values().next())
        .expect("golden latent has a tensor")
        .to_dtype(DType::F32)
        .expect("golden latent f32");
    let got = latent.to_dtype(DType::F32).expect("latent f32");
    assert_eq!(got.shape(), golden.shape(), "latent shape matches golden");

    let a = got.flatten_all().expect("flatten got").to_vec1::<f32>().expect("got vec");
    let b = golden.flatten_all().expect("flatten golden").to_vec1::<f32>().expect("golden vec");
    let max_abs = a.iter().zip(&b).map(|(x, y)| (x - y).abs()).fold(0.0_f32, f32::max);
    let corr = correlation(&a, &b);
    assert!(corr >= 0.999, "latent correlation {corr} below 0.999 (sampler/topology drift)");
    assert!(max_abs < 1e-1, "latent max-abs diff {max_abs} exceeds the bf16 tolerance");
}

/// Pearson correlation of two equal-length slices.
fn correlation(a: &[f32], b: &[f32]) -> f32 {
    let n = a.len() as f32;
    let mean_a = a.iter().sum::<f32>() / n;
    let mean_b = b.iter().sum::<f32>() / n;
    let mut cov = 0.0_f32;
    let mut va = 0.0_f32;
    let mut vb = 0.0_f32;
    for (x, y) in a.iter().zip(b) {
        let dx = x - mean_a;
        let dy = y - mean_b;
        cov += dx * dy;
        va += dx * dx;
        vb += dy * dy;
    }
    if va == 0.0 || vb == 0.0 { 0.0 } else { cov / (va.sqrt() * vb.sqrt()) }
}

/// Assert a tensor is entirely finite (no NaN, no Inf).
fn assert_finite(t: &Tensor, label: &str) {
    let flat = t.flatten_all().expect("flatten").to_vec1::<f32>().expect("to f32 vec");
    assert!(!flat.is_empty(), "{label} is empty");
    assert!(flat.iter().all(|v| v.is_finite()), "{label} has a non-finite value");
}
