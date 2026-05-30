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
use crate::store::ModelStore;
use crate::text_encoder::TextEncoder;
use crate::vae::Vae;

/// A request to generate or edit an image. The concrete fields land with the
/// backend bridge; for now it is the seam the loader's run methods accept.
#[derive(Clone, Debug, Default)]
pub struct FluxRequest {
    /// The text prompt.
    pub prompt: String,
    /// Output width in pixels.
    pub width: u32,
    /// Output height in pixels.
    pub height: u32,
    /// Optional init image (PNG bytes) for image-to-image. `None` is text-to-image.
    pub init_image: Option<Vec<u8>>,
    /// Image-to-image strength in `0.0..=1.0`. Ignored without an init image.
    pub strength: f32,
}

/// A loaded, ready-to-run FLUX.2 model: the transformer, the VAE, the text
/// encoder, and the device they live on.
///
/// Construction (`load`) is the multi-GB step; the run methods are
/// synchronous and GPU-bound, so the backend invokes them inside
/// `spawn_blocking` under a `parking_lot::Mutex`.
pub struct LoadedModel {
    /// The FLUX.2 `DiT`. Driven by the t2i/img2img pipelines in a later gate; the
    /// single-forward test drives it through its own public `FluxTransformer::load`,
    /// so this field is not yet read inside the crate.
    #[allow(dead_code)]
    transformer: FluxTransformer,
    /// The VAE — latent encode/decode. Read by the t2i/img2img pipelines in a
    /// later gate; the round-trip test drives the VAE through its own public
    /// `Vae::load`, so this field is not yet read inside the crate.
    #[allow(dead_code)]
    vae: Vae,
    /// The Qwen3 text encoder — prompt -> `(1, seq, 7680)` conditioning. Read by
    /// the t2i/img2img pipelines in a later gate; the parity test drives it
    /// through its own public `TextEncoder::load`, so this field is not yet read
    /// inside the crate.
    #[allow(dead_code)]
    text_encoder: TextEncoder,
    /// The device every component lives on. Read by the run methods in a later gate.
    #[allow(dead_code)]
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

    /// Run text-to-image. `step_cb(step, total)` is called before each sampling
    /// step; returning `false` cancels the run. Returns one or more PNG buffers.
    ///
    /// # Errors
    ///
    /// Returns [`FluxError`] on a tensor or decode failure.
    pub fn text_to_image<F>(&mut self, req: &FluxRequest, step_cb: F) -> Result<Vec<Vec<u8>>, FluxError>
    where
        F: FnMut(usize, usize) -> bool,
    {
        let _ = (req, step_cb);
        todo!("port the 4-step flow-matching t2i pipeline")
    }

    /// Run image-to-image (edit / inpaint). Same callback contract as
    /// [`LoadedModel::text_to_image`].
    ///
    /// # Errors
    ///
    /// Returns [`FluxError`] on a tensor or decode failure.
    pub fn image_to_image<F>(&mut self, req: &FluxRequest, step_cb: F) -> Result<Vec<Vec<u8>>, FluxError>
    where
        F: FnMut(usize, usize) -> bool,
    {
        let _ = (req, step_cb);
        todo!("port the img2img pipeline: VAE-encode -> add noise -> partial denoise")
    }
}
