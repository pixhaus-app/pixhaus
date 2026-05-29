//! On-device FLUX.2 backend.
//!
//! `LocalFluxBackend` implements [`super::InferenceBackend`] over a loaded
//! `pixhaus_flux::LoadedModel`: text-to-image, image-to-image edit, and inpaint,
//! all running locally with no API key and no per-image cost. Inference is
//! synchronous and GPU-bound, so `invoke` offloads to `spawn_blocking` and
//! bridges sampling-step progress back to the async `VerbProgress` channel.
//!
//! The implementation lands with gate 6; this module currently re-exports the
//! crate's load-side types so the rest of the backend wiring can name them. No
//! `panic!`/`todo!` here — the `ai` crate keeps the workspace lint floor.

#![cfg(feature = "local-flux")]

pub use pixhaus_flux::{DeviceChoice, DevicePref, ModelStore};
