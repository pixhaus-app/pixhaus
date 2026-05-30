//! Weight loading — turns a [`ModelStore`]'s on-disk files plus a device into a
//! [`LoadedModel`] ready to run text-to-image and image-to-image.
//!
//! The multi-GB load is expensive and runs once; the backend holds the result
//! behind a `OnceCell<Mutex<..>>`. The VAE and Qwen3 text encoder are wired; the
//! transformer lands in a later gate, so its field stays a placeholder.
//!
//! Working dtype is bf16 on a GPU, f32 on CPU (candle's `bf16_default_to_f32`).
//! `VarBuilder::from_mmaped_safetensors` returns every tensor already cast to the
//! working dtype, which upcasts an `F8E4M3` checkpoint on load without naming the
//! fp8 dtype at the call site — we load at the dtype we compute in.

use std::path::Path;

use candle_core::{DType, Device};
use candle_nn::VarBuilder;

use crate::FluxError;
use crate::device::DeviceChoice;
use crate::model::FluxTransformer;
use crate::pipeline::{self, SampleParams};
use crate::store::ModelStore;
use crate::text_encoder::TextEncoder;
use crate::vae::Vae;

/// An advanced override of the distilled sampling defaults. Present only when the
/// settings advanced-override toggle is on; otherwise the backend runs the pinned
/// distilled 4 steps / guidance 1.0 and these fields are ignored.
#[derive(Clone, Copy, Debug)]
pub struct AdvancedSampling {
    /// Flow-matching step count override.
    pub steps: usize,
    /// Guidance-scale override (a no-op for klein, carried for completeness).
    pub guidance: f64,
}

/// A request to generate or edit an image.
#[derive(Clone, Debug, Default)]
pub struct FluxRequest {
    /// The text prompt.
    pub prompt: String,
    /// Output width in pixels. Rounded up to a multiple of the latent downsample.
    pub width: u32,
    /// Output height in pixels. Rounded up to a multiple of the latent downsample.
    pub height: u32,
    /// RNG seed. A fixed seed yields a fixed image for a fixed prompt.
    pub seed: u64,
    /// How many images to sample in one batch. Zero is treated as one.
    pub num_images: u32,
    /// Optional init image (PNG bytes) for image-to-image. `None` is text-to-image.
    pub init_image: Option<Vec<u8>>,
    /// Optional inpaint mask (single-channel PNG bytes), white = repaint, black =
    /// preserve. `None` with an init image is a whole-image edit; `Some` repaints
    /// only the white region. Ignored without an init image.
    pub mask: Option<Vec<u8>>,
    /// Image-to-image strength in `0.0..=1.0`. Ignored without an init image.
    pub strength: f32,
    /// Advanced override of the distilled step/guidance defaults. `None` pins the
    /// distilled posture (4 steps, guidance 1.0).
    pub advanced: Option<AdvancedSampling>,
}

impl FluxRequest {
    /// Resolve the sampling parameters: the advanced override when present, else
    /// the pinned distilled defaults.
    fn sample_params(&self) -> SampleParams {
        match self.advanced {
            Some(adv) => SampleParams {
                steps: adv.steps.max(1),
                guidance: adv.guidance,
            },
            None => SampleParams::default(),
        }
    }

    /// At least one image — a zero `num_images` is treated as one.
    fn image_count(&self) -> usize {
        (self.num_images as usize).max(1)
    }
}

/// A loaded, ready-to-run FLUX.2 model: the transformer, the VAE, the text
/// encoder, and the device they live on.
///
/// Construction (`load`) is the multi-GB step; the run methods are
/// synchronous and GPU-bound, so the backend invokes them inside
/// `spawn_blocking` under a `parking_lot::Mutex`.
pub struct LoadedModel {
    /// The FLUX.2 `DiT`. Driven one denoise step per schedule window by the t2i
    /// pipeline.
    transformer: FluxTransformer,
    /// The VAE — latent encode/decode plus the patchify/`BatchNorm` boundary the
    /// t2i pipeline inverts before decoding.
    vae: Vae,
    /// The Qwen3 text encoder — prompt -> `(1, seq, 7680)` conditioning.
    text_encoder: TextEncoder,
    /// The device every component lives on. The t2i pipeline seeds it and draws
    /// the initial noise on it.
    device: Device,
}

/// mmap a safetensors shard set into a [`VarBuilder`] at `dtype`.
///
/// Candle's loader is `unsafe` because it maps the files — the bytes must not
/// change underneath us, which holds for our read-only cache. Every tensor is
/// returned already cast to `dtype`, so an `F8E4M3` checkpoint upcasts to bf16
/// here without a separate pass.
///
/// # Errors
///
/// Returns [`FluxError::Candle`] if a shard is missing or malformed.
pub(crate) fn var_builder<'a, P: AsRef<Path>>(paths: &[P], dtype: DType, device: &Device) -> Result<VarBuilder<'a>, FluxError> {
    // The crate denies `unsafe`; this one call is the documented exception. The
    // loader is `unsafe` only because it mmaps the files — the contract is that
    // the mapped bytes must not change underneath us. Our weight files are
    // read-only on disk for the process lifetime, so the invariant holds. mmap is
    // the memory-critical path for the multi-GB transformer (no full read into
    // RAM); a safe buffered loader would defeat that.
    #[allow(unsafe_code)]
    let vb = unsafe { VarBuilder::from_mmaped_safetensors(paths, dtype, device)? };
    Ok(vb)
}

impl LoadedModel {
    /// Load every component from the store's cache onto `device`.
    ///
    /// # Errors
    ///
    /// Returns [`FluxError`] if a required file is missing, a safetensors load
    /// fails, or the device cannot be initialized.
    pub fn load(store: &ModelStore, device: DeviceChoice) -> Result<Self, FluxError> {
        if !store.is_downloaded() {
            return Err(FluxError::NotDownloaded);
        }
        let device = device.to_device()?;
        // bf16 on GPU, f32 on CPU. The VAE is small (~168 MB) but force_upcast is
        // set in its config; f32 on CPU keeps the round-trip numerically honest.
        let dtype = device.bf16_default_to_f32();

        let vae = Vae::load(store, &device, dtype)?;
        let text_encoder = TextEncoder::load(store, &device, dtype)?;
        let transformer = FluxTransformer::load(store, &device, dtype)?;

        Ok(Self {
            transformer,
            vae,
            text_encoder,
            device,
        })
    }

    /// Run text-to-image. `should_continue(step, total)` is called before each
    /// sampling step; returning `false` cancels the run (the loop breaks and the
    /// partial latent decodes — the backend bridge discards a cancelled result).
    /// Returns one PNG buffer per requested image.
    ///
    /// The path, end to end: encode the prompt to Qwen3 conditioning, draw seeded
    /// noise in the normalized patchified latent space, integrate the 4-step
    /// flow-matching schedule through the transformer, then invert the latent prep
    /// and VAE-decode to PNG. Width/height are rounded up to a multiple of the
    /// latent downsample; seed and `num_images` come from the request; steps and
    /// guidance are pinned to the distilled defaults unless the advanced override
    /// is set.
    ///
    /// # Errors
    ///
    /// Returns [`FluxError`] on a tensor, encode, or decode failure.
    pub fn text_to_image<F>(&mut self, req: &FluxRequest, should_continue: F) -> Result<Vec<Vec<u8>>, FluxError>
    where
        F: FnMut(usize, usize) -> bool,
    {
        let params = req.sample_params();
        let num_images = req.image_count();
        let device = &self.device;

        // Seed the device so a fixed seed yields a fixed image. CPU cannot be
        // seeded in candle 0.10 (`set_seed` errors on `Device::Cpu`); ignore that
        // failure so the CPU path still runs — only the GPU path is reproducible.
        let _ = device.set_seed(req.seed);

        let dtype = device.bf16_default_to_f32();

        // Output geometry: round each side up to a multiple of the 16x downsample
        // and derive the latent token grid.
        let (out_w, lw) = pipeline::latent_grid(req.width);
        let (out_h, lh) = pipeline::latent_grid(req.height);
        let _ = (out_w, out_h);

        // Prompt -> Qwen3 conditioning (1, seq, 7680). Repeat across the batch so
        // every image shares the prompt.
        let txt_single = self.text_encoder.encode(&req.prompt)?.to_dtype(dtype)?;
        let seq = txt_single.dim(1)?;
        let txt = txt_single.broadcast_as((num_images, seq, txt_single.dim(2)?))?.contiguous()?;
        let txt_ids = pipeline::text_position_ids(seq, device)?;

        // Image position ids over the latent grid.
        let img_ids = pipeline::image_position_ids(lh, lw, device)?;

        // Seeded noise in the normalized patchified packed space, then integrate.
        let channels = self.vae.config().patched_latent_channels();
        let noise = pipeline::initial_noise(num_images, lh, lw, channels, device, dtype)?;
        let timesteps = pipeline::schedule(params.steps);

        let latent = pipeline::denoise(&self.transformer, &noise, &img_ids, &txt, &txt_ids, &timesteps, should_continue)?;

        pipeline::decode_to_pngs(&self.vae, &latent, lh, lw)
    }

    /// Run image-to-image (single-reference edit and masked inpaint). Same
    /// callback contract as [`LoadedModel::text_to_image`]: `should_continue` runs
    /// before each step, `false` cancels.
    ///
    /// The path: VAE-encode the init image into the normalized, patchified, packed
    /// clean latent; draw seeded unit noise of that shape; pick the schedule entry
    /// index from `strength`; noise the clean latent to that schedule time; then
    /// run the partial denoise from the entry to the end and invert the prep. With
    /// a mask, the preserved (black) region is re-pinned to the reference after
    /// every step so only the white region is repainted.
    ///
    /// Strength maps to the schedule: `1.0` re-noises fully (from `t = 1`, the
    /// whole schedule, like text-to-image), `0.0` returns the init image untouched,
    /// in between it enters the schedule at `round((1 - strength) * steps)` so a
    /// lower strength keeps more of the reference.
    ///
    /// # Errors
    ///
    /// Returns [`FluxError`] if the init image is missing, or on a tensor / encode
    /// / decode failure.
    pub fn image_to_image<F>(&mut self, req: &FluxRequest, should_continue: F) -> Result<Vec<Vec<u8>>, FluxError>
    where
        F: FnMut(usize, usize) -> bool,
    {
        let Some(init_png) = req.init_image.as_ref() else {
            return Err(FluxError::Device("image_to_image called without an init image".to_owned()));
        };

        let params = req.sample_params();
        let num_images = req.image_count();
        let device = &self.device;

        // Seed the device so a fixed seed yields a fixed edit. CPU cannot be seeded
        // in candle 0.10 (`set_seed` errors on `Device::Cpu`); ignore that so the
        // CPU path still runs — only the GPU path is reproducible.
        let _ = device.set_seed(req.seed);
        let dtype = device.bf16_default_to_f32();

        // Output geometry: round each side up to a multiple of the 16x downsample.
        // The init image is resized to this so its latent grid matches the noise.
        let (out_w, _) = pipeline::latent_grid(req.width);
        let (out_h, _) = pipeline::latent_grid(req.height);
        // cast bound: rounded request dims fit u32 by construction.
        #[allow(clippy::cast_possible_truncation)]
        let (out_w, out_h) = (out_w as u32, out_h as u32);

        // Reference -> normalized patchified packed clean latent.
        let rgb = pipeline::png_to_rgb_tensor(init_png, out_w, out_h, device, dtype)?;
        let (clean_single, lh, lw) = pipeline::encode_init_latent(&self.vae, &rgb, dtype)?;
        let channels = self.vae.config().patched_latent_channels();
        // Repeat the clean latent across the requested batch.
        let clean = clean_single.broadcast_as((num_images, lh * lw, channels))?.contiguous()?;

        // Prompt -> Qwen3 conditioning, repeated across the batch.
        let txt_single = self.text_encoder.encode(&req.prompt)?.to_dtype(dtype)?;
        let seq = txt_single.dim(1)?;
        let txt = txt_single.broadcast_as((num_images, seq, txt_single.dim(2)?))?.contiguous()?;
        let txt_ids = pipeline::text_position_ids(seq, device)?;
        let img_ids = pipeline::image_position_ids(lh, lw, device)?;

        // Schedule + entry index from strength.
        let timesteps = pipeline::schedule(params.steps);
        let start_index = pipeline::i2i_schedule_index(req.strength, params.steps);

        // Seeded noise of the clean latent's shape, then noise the reference to the
        // entry schedule time.
        let noise = pipeline::initial_noise(num_images, lh, lw, channels, device, dtype)?;
        // t at the entry point. start_index indexes into the schedule points; the
        // entry time is timesteps[start_index] (1.0 at index 0, 0.0 at the end).
        let t_start = timesteps.get(start_index).copied().unwrap_or(0.0);
        let entry = pipeline::add_noise(&clean, &noise, t_start)?;

        // Optional inpaint mask -> packed per-token mask. When present, the partial
        // denoise re-pins the preserved region after each step.
        let mask = match req.mask.as_ref() {
            Some(mask_png) => Some(pipeline::mask_to_packed(mask_png, lh, lw, device, dtype)?),
            None => None,
        };

        let latent = pipeline::denoise_partial(
            &self.transformer,
            &entry,
            &img_ids,
            &txt,
            &txt_ids,
            &timesteps,
            start_index,
            mask.as_ref(),
            mask.as_ref().map(|_| &clean),
            mask.as_ref().map(|_| &noise),
            should_continue,
        )?;

        pipeline::decode_to_pngs(&self.vae, &latent, lh, lw)
    }
}
