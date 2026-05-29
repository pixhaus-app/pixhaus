//! Weight loading — turns a [`ModelStore`]'s on-disk files plus a device into a
//! [`LoadedModel`] ready to run text-to-image and image-to-image.
//!
//! The multi-GB load is expensive and runs once; the backend holds the result
//! behind a `OnceCell<Mutex<..>>`. The actual `VarBuilder` wiring lands in a later
//! phase — these are the signatures the backend bridge depends on.

use candle_core::Device;

use crate::device::DeviceChoice;
use crate::model::FluxTransformer;
use crate::store::ModelStore;
use crate::text_encoder::TextEncoder;
use crate::vae::Vae;
use crate::FluxError;

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
    #[allow(dead_code)]
    transformer: FluxTransformer,
    #[allow(dead_code)]
    vae: Vae,
    #[allow(dead_code)]
    text_encoder: TextEncoder,
    #[allow(dead_code)]
    device: Device,
}

impl LoadedModel {
    /// Load every component from the store's cache onto `device`.
    ///
    /// # Errors
    ///
    /// Returns [`FluxError`] if a required file is missing, a safetensors load
    /// fails, or the device cannot be initialized.
    pub fn load(store: &ModelStore, device: DeviceChoice) -> Result<Self, FluxError> {
        let _ = (store, device);
        todo!("port FLUX.2 weight loading on candle VarBuilder")
    }

    /// Run text-to-image. `step_cb(step, total)` is called before each sampling
    /// step; returning `false` cancels the run. Returns one or more PNG buffers.
    ///
    /// # Errors
    ///
    /// Returns [`FluxError`] on a tensor or decode failure.
    pub fn text_to_image<F>(
        &mut self,
        req: &FluxRequest,
        step_cb: F,
    ) -> Result<Vec<Vec<u8>>, FluxError>
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
    pub fn image_to_image<F>(
        &mut self,
        req: &FluxRequest,
        step_cb: F,
    ) -> Result<Vec<Vec<u8>>, FluxError>
    where
        F: FnMut(usize, usize) -> bool,
    {
        let _ = (req, step_cb);
        todo!("port the img2img pipeline: VAE-encode -> add noise -> partial denoise")
    }
}
