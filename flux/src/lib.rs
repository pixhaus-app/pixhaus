//! On-device FLUX.2 `klein` 4B image generation for Pixhaus.
//!
//! This crate ports the FLUX.2 inference pipeline to the [Candle](https://github.com/huggingface/candle)
//! Rust ML framework: load the bf16 weights, run 4-step distilled
//! flow-matching, and decode to RGB. It exposes presence + download
//! ([`store`]), weight loading ([`loader`]), the transformer/VAE/text-encoder
//! topology ([`model`], [`vae`], [`text_encoder`]), and device selection
//! ([`device`]). The `ai` crate wraps a [`loader::LoadedModel`] in an
//! `InferenceBackend` behind the `local-flux` feature. The multi-GB load runs
//! once, held behind a `OnceCell<Mutex<..>>` on the backend.
//!
//! There is no `flux2` module in candle-transformers — only FLUX.1's `flux` and
//! `qwen3`. The FLUX.2 topology is assembled on candle's published base APIs
//! (no fork): import the public sampler, VAE, Qwen3 encoder, and `DiT`
//! building-block structs; reimplement the handful of module-private helpers
//! locally; build the new block topology where it diverges from FLUX.1.

// Test-only relaxations for the in-crate `#[cfg(test)]` modules. The workspace
// floor denies unwrap/expect/panic and clippy.toml bans unwrap/expect, but the
// unit tests build their own fixtures and tensors, where unwrapping a just-built
// value and index/shape casts are idiomatic. `cfg_attr(test, ...)` keeps
// production code held to the full strictness.
#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::missing_panics_doc,
        clippy::disallowed_methods,
        clippy::cast_possible_truncation,
        clippy::cast_precision_loss,
        clippy::identity_op
    )
)]

pub mod device;
pub mod loader;
pub mod model;
pub mod pipeline;
pub mod store;
pub mod text_encoder;
pub mod vae;

pub use device::{DeviceChoice, DevicePref};
pub use loader::{AdvancedSampling, FluxRequest, LoadedModel};
pub use store::{FLUX2_KLEIN_MODEL_ID, FLUX2_KLEIN_REPO, FLUX2_KLEIN_REVISION, HF_TOKEN_SERVICE_ID, ModelStore};

#[cfg(feature = "download")]
pub use store::DownloadError;

use thiserror::Error;

/// Errors raised by the FLUX.2 runtime.
#[derive(Debug, Error)]
pub enum FluxError {
    /// A candle tensor / device / load operation failed.
    #[error("candle error: {0}")]
    Candle(#[from] candle_core::Error),

    /// A required model file was missing or unreadable.
    #[error("model file error: {0}")]
    Io(#[from] std::io::Error),

    /// The selected device could not be initialized (no CUDA/Metal runtime, etc.).
    #[error("device unavailable: {0}")]
    Device(String),

    /// The weights are not fully downloaded for the configured cache directory.
    #[error("model not downloaded")]
    NotDownloaded,
}
