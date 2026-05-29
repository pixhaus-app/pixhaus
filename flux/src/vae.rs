//! The FLUX.2 VAE — latent encode/decode.
//!
//! Imports the encoder/decoder structure from
//! `candle_transformers::models::flux::autoencoder` and re-derives the config
//! (channels, scale, shift, downsample) from the diffusers `Flux2` VAE config —
//! FLUX.1's `z_channels=16`, `scale_factor=0.3611`, `shift_factor=0.1159` do not
//! transfer unchanged. `encode` folds in scale/shift and samples the diagonal
//! Gaussian; `decode` inverts it.
//!
//! This file is a typed placeholder; the VAE wiring lands in gate 1.

/// The variational autoencoder: encodes RGB to latents and decodes back.
pub struct Vae {
    // Fields land with the VAE port (gate 1).
}
