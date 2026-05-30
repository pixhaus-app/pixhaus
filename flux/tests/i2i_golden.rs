//! Gate 5: single-reference image-to-image against a golden image.
//!
//! Fixed init image + fixed prompt + fixed strength + fixed seed -> one PNG,
//! compared to a committed golden with `image-compare`'s MSSIM at the `>= 0.999`
//! bar. The test drives the whole `LoadedModel::image_to_image` path: VAE-encode
//! the reference, noise it to the strength-derived schedule entry, partial
//! denoise, decode.
//!
//! It also pins the strength -> schedule-index mapping with a few plain-Rust
//! assertions that need no weights, so the contract is checked even in CI where
//! the weighted stage self-skips.
//!
//! Triple-gated so GPU-less CI stays green, exactly like the t2i / VAE gates:
//! 1. The whole file compiles only under a backend feature
//!    (`cpu` / `cuda` / `metal`); a `--no-default-features` build skips it.
//! 2. `#[ignore]` — `cargo test` runs it only with `-- --ignored`.
//! 3. Early-return when `PIXHAUS_FLUX_WEIGHTS` is unset, so even `--ignored` is a
//!    no-op without a weights dir.
//!
//! The golden fixture is produced once from the `PyTorch` reference at this exact
//! init image / prompt / strength / seed / size and dropped into
//! `flux/tests/fixtures/`. Until it is present the weighted stage self-skips with
//! a message rather than asserting against a fabricated file.
//!
//! `PIXHAUS_FLUX_WEIGHTS` points at the model store root — the directory that
//! holds `transformer/`, `vae/`, `text_encoder/`, and `tokenizer/`.

#![cfg(any(feature = "cpu", feature = "cuda", feature = "metal"))]

use std::path::{Path, PathBuf};

use candle_core::Device;
use image::RgbImage;
use image_compare::Algorithm;
use pixhaus_flux::loader::{FluxRequest, LoadedModel};
use pixhaus_flux::pipeline;
use pixhaus_flux::store::ModelStore;

/// The fixed edit inputs. Keep these in lockstep with the script that dumps the
/// golden fixture — change one, regenerate the golden.
const PROMPT: &str = "a small blue robot mascot waving, pixel art";
const SEED: u64 = 0;
const STRENGTH: f32 = 0.6;
const WIDTH: u32 = 64;
const HEIGHT: u32 = 64;

/// The init/reference image fed to the edit. Reuses the VAE round-trip fixture —
/// a real 64x64 PNG already in the tree.
const INIT_PNG: &[u8] = include_bytes!("fixtures/vae_round_trip_64.png");

/// The golden PNG, produced from the reference i2i pipeline at the inputs above.
const GOLDEN_PNG: &str = "fixtures/i2i_golden_64.png";

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

/// The strength -> schedule-entry-index contract, checked without weights so CI
/// always exercises it. With the distilled 4 steps:
/// - strength 1.0 enters at index 0 (full schedule; equivalent to t2i),
/// - strength 0.0 enters at index 4 (no steps; init image returned),
/// - strength 0.6 enters at round((1 - 0.6) * 4) = round(1.6) = index 2.
#[test]
fn strength_maps_to_schedule_index() {
    let steps = pipeline::DISTILLED_STEPS; // 4
    assert_eq!(pipeline::i2i_schedule_index(1.0, steps), 0, "full strength runs the whole schedule");
    assert_eq!(pipeline::i2i_schedule_index(0.0, steps), steps, "zero strength runs no steps");
    assert_eq!(pipeline::i2i_schedule_index(STRENGTH, steps), 2, "strength 0.6 enters at index 2");
    // Clamps out-of-range inputs.
    assert_eq!(pipeline::i2i_schedule_index(2.0, steps), 0, "over-range strength clamps to full");
    assert_eq!(pipeline::i2i_schedule_index(-1.0, steps), steps, "under-range strength clamps to none");
    // Monotone: a higher strength never enters later than a lower one.
    let idx_low = pipeline::i2i_schedule_index(0.25, steps);
    let idx_high = pipeline::i2i_schedule_index(0.75, steps);
    assert!(idx_high <= idx_low, "higher strength enters the schedule no later");
}

#[test]
#[ignore = "requires downloaded FLUX.2 weights and a GPU"]
fn i2i_matches_golden() {
    let Some(root) = std::env::var_os("PIXHAUS_FLUX_WEIGHTS") else {
        eprintln!("PIXHAUS_FLUX_WEIGHTS unset; skipping i2i golden gate");
        return;
    };
    let store = ModelStore::new(PathBuf::from(root));

    // CPU keeps the run in f32 and deterministic across machines; a GPU build
    // computes in bf16, where the 0.999 bar is the bf16-compute tolerance. The
    // device seed fixes the noise draw and the VAE-encode sample (CPU cannot be
    // seeded in candle 0.10, so the CPU path relies on the committed golden, not
    // reproducibility).
    let device = Device::Cpu;
    let _ = device.set_seed(SEED);

    let mut model = LoadedModel::load(&store, pixhaus_flux::DeviceChoice::Cpu).expect("load full model");
    let req = FluxRequest {
        prompt: PROMPT.to_owned(),
        width: WIDTH,
        height: HEIGHT,
        seed: SEED,
        num_images: 1,
        init_image: Some(INIT_PNG.to_vec()),
        mask: None,
        strength: STRENGTH,
        ..Default::default()
    };
    let images = model.image_to_image(&req, |_, _| true).expect("image_to_image");
    assert_eq!(images.len(), 1, "one image requested -> one PNG");

    let produced = image::load_from_memory(&images[0]).expect("produced PNG decodes").to_rgb8();
    let (pw, ph) = produced.dimensions();
    assert_eq!((pw, ph), (WIDTH, HEIGHT), "produced image matches the rounded request");

    let golden_png_path = fixture(GOLDEN_PNG);
    if !golden_png_path.exists() {
        eprintln!("golden PNG {golden_png_path:?} absent; skipping image compare.");
        eprintln!("generate it from the reference at prompt={PROMPT:?} seed={SEED} strength={STRENGTH} {WIDTH}x{HEIGHT}.");
        return;
    }
    let (golden_rgb, gw, gh) = load_png(&golden_png_path);
    assert_eq!((gw, gh), (WIDTH, HEIGHT), "golden image size matches the request");

    let golden = RgbImage::from_raw(gw, gh, golden_rgb).expect("golden RgbImage");
    let result = image_compare::rgb_similarity_structure(&Algorithm::MSSIMSimple, &golden, &produced).expect("compare images");
    assert!(result.score >= 0.999, "i2i MSSIM {} below 0.999 vs golden", result.score);
}

#[test]
#[ignore = "requires downloaded FLUX.2 weights and a GPU"]
fn inpaint_preserves_unmasked_region() {
    let Some(root) = std::env::var_os("PIXHAUS_FLUX_WEIGHTS") else {
        eprintln!("PIXHAUS_FLUX_WEIGHTS unset; skipping inpaint preservation check");
        return;
    };
    let store = ModelStore::new(PathBuf::from(root));
    let device = Device::Cpu;
    let _ = device.set_seed(SEED);

    // An all-black mask preserves everything: the edit must return the init image
    // essentially unchanged (only VAE round-trip loss separates them), which is a
    // weight-free property of the masked path that needs no golden.
    let black_mask = solid_mask(WIDTH, HEIGHT, 0);

    let mut model = LoadedModel::load(&store, pixhaus_flux::DeviceChoice::Cpu).expect("load full model");
    let req = FluxRequest {
        prompt: PROMPT.to_owned(),
        width: WIDTH,
        height: HEIGHT,
        seed: SEED,
        num_images: 1,
        init_image: Some(INIT_PNG.to_vec()),
        mask: Some(black_mask),
        strength: 1.0, // even at full strength a black mask repaints nothing.
        ..Default::default()
    };
    let images = model.image_to_image(&req, |_, _| true).expect("inpaint");
    let produced = image::load_from_memory(&images[0]).expect("produced PNG decodes").to_rgb8();

    let init = image::load_from_memory(INIT_PNG).expect("init decodes").to_rgb8();
    let result = image_compare::rgb_similarity_structure(&Algorithm::MSSIMSimple, &init, &produced).expect("compare images");
    // VAE round-trip is lossy, so the bar matches the VAE gate, not the i2i one:
    // a fully-preserved inpaint can only be as faithful as one encode/decode.
    assert!(result.score >= 0.99, "black-mask inpaint MSSIM {} below 0.99 vs init", result.score);
}

/// Build a solid single-channel PNG of `value` at `w x h`, for the mask tests.
fn solid_mask(w: u32, h: u32, value: u8) -> Vec<u8> {
    let img = image::GrayImage::from_pixel(w, h, image::Luma([value]));
    let mut buf = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png).expect("encode mask PNG");
    buf
}
