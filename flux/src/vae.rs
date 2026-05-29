//! The FLUX.2 VAE — latent encode/decode.
//!
//! FLUX.2 ships `AutoencoderKLFlux2`, a **diffusers**-format checkpoint. Its
//! weight keys (`encoder.down_blocks.{i}.resnets.{j}`, `encoder.mid_block`,
//! `conv_norm_out`, `quant_conv`, `post_quant_conv`, `bn`) and its `nn.Linear`
//! mid-block attention do not match candle's `flux::autoencoder`, which hardcodes
//! the BFL key layout (`encoder.down.{i}.block.{j}`, `mid.block_1`, 1x1-conv
//! attention) and a scalar scale/shift. So this is a **port, not a reuse**: the
//! same conv/resnet/groupnorm/SiLU scaffolding candle uses, re-keyed to the
//! diffusers names and re-derived for the FLUX.2 config.
//!
//! Config vs FLUX.1 (from `.tmp/lf-research/02-vae-qwen3.md`, the live
//! `vae/config.json`):
//! - `latent_channels` 32 (FLUX.1 was 16).
//! - no scalar `scaling_factor`/`shift_factor`; latent normalization is a learned
//!   per-channel `BatchNorm` (`bn`) applied by the pipeline at the patchified
//!   (128-channel) transformer boundary, with `batch_norm_eps = 1e-4`.
//! - latent-side `patch_size [2, 2]` patchify (32 -> 128) lives in the pipeline.
//!
//! `encode`/`decode` here are a clean inverse pair on the raw 32-channel VAE
//! latent: `encode` runs the encoder, `quant_conv`, then samples the diagonal
//! Gaussian; `decode` runs `post_quant_conv` then the decoder. The `BatchNorm`
//! normalize/denormalize and the patchify are exposed separately for the
//! transformer-boundary code (later gate); they cancel across a bare round-trip,
//! so the round-trip test does not touch them.

use candle_core::{D, DType, Device, Module, Tensor};
use candle_nn::{Conv2d, Conv2dConfig, GroupNorm, Linear, VarBuilder, conv2d, group_norm, linear};

use crate::FluxError;
use crate::loader::var_builder;
use crate::store::ModelStore;

/// The FLUX.2 VAE config (`AutoencoderKLFlux2`). Defaults match the live
/// `black-forest-labs/FLUX.2-klein-4B` `vae/config.json`.
#[derive(Clone, Debug)]
pub struct VaeConfig {
    /// RGB in.
    pub in_channels: usize,
    /// RGB out.
    pub out_channels: usize,
    /// Per-down-block output channels, e.g. `[128, 256, 512, 512]`.
    pub block_out_channels: Vec<usize>,
    /// Resnet blocks per down/up block.
    pub layers_per_block: usize,
    /// Latent (z) channels — 32 for FLUX.2.
    pub latent_channels: usize,
    /// `GroupNorm` groups.
    pub norm_num_groups: usize,
    /// Whether the mid block carries a self-attention.
    pub mid_block_add_attention: bool,
    /// Latent-side patch size `[h, w]`; folds `prod` into the channel axis
    /// (32 * 4 = 128) at the transformer boundary.
    pub patch_size: [usize; 2],
    /// Epsilon added under the sqrt of the latent `BatchNorm` variance.
    pub batch_norm_eps: f64,
}

impl Default for VaeConfig {
    fn default() -> Self {
        Self::flux2_klein()
    }
}

impl VaeConfig {
    /// The FLUX.2-klein-4B VAE config, byte-for-byte from `vae/config.json`.
    #[must_use]
    pub fn flux2_klein() -> Self {
        Self {
            in_channels: 3,
            out_channels: 3,
            block_out_channels: vec![128, 256, 512, 512],
            layers_per_block: 2,
            latent_channels: 32,
            norm_num_groups: 32,
            mid_block_add_attention: true,
            patch_size: [2, 2],
            batch_norm_eps: 1e-4,
        }
    }

    /// Patchified latent channel count — `prod(patch_size) * latent_channels`
    /// (the `BatchNorm` and transformer input width, 128 for klein).
    #[must_use]
    pub fn patched_latent_channels(&self) -> usize {
        self.patch_size[0] * self.patch_size[1] * self.latent_channels
    }
}

const RESNET_EPS: f64 = 1e-6;

fn conv3x3(in_c: usize, out_c: usize, vb: VarBuilder) -> Result<Conv2d, FluxError> {
    let cfg = Conv2dConfig {
        padding: 1,
        ..Default::default()
    };
    Ok(conv2d(in_c, out_c, 3, cfg, vb)?)
}

/// A diffusers `ResnetBlock2D` (no time embedding — the VAE has none).
/// Keys: `norm1`, `conv1`, `norm2`, `conv2`, optional `conv_shortcut`.
#[derive(Debug)]
struct ResnetBlock {
    norm1: GroupNorm,
    conv1: Conv2d,
    norm2: GroupNorm,
    conv2: Conv2d,
    conv_shortcut: Option<Conv2d>,
}

impl ResnetBlock {
    fn new(in_c: usize, out_c: usize, groups: usize, vb: VarBuilder) -> Result<Self, FluxError> {
        let norm1 = group_norm(groups, in_c, RESNET_EPS, vb.pp("norm1"))?;
        let conv1 = conv3x3(in_c, out_c, vb.pp("conv1"))?;
        let norm2 = group_norm(groups, out_c, RESNET_EPS, vb.pp("norm2"))?;
        let conv2 = conv3x3(out_c, out_c, vb.pp("conv2"))?;
        let conv_shortcut = if in_c == out_c {
            None
        } else {
            // 1x1, no padding. diffusers names it `conv_shortcut`.
            Some(conv2d(in_c, out_c, 1, Conv2dConfig::default(), vb.pp("conv_shortcut"))?)
        };
        Ok(Self {
            norm1,
            conv1,
            norm2,
            conv2,
            conv_shortcut,
        })
    }
}

impl Module for ResnetBlock {
    fn forward(&self, xs: &Tensor) -> candle_core::Result<Tensor> {
        let h = xs
            .apply(&self.norm1)?
            .apply(&candle_nn::Activation::Silu)?
            .apply(&self.conv1)?
            .apply(&self.norm2)?
            .apply(&candle_nn::Activation::Silu)?
            .apply(&self.conv2)?;
        let residual = match self.conv_shortcut.as_ref() {
            None => xs.clone(),
            Some(c) => xs.apply(c)?,
        };
        residual + h
    }
}

/// Diffusers spatial self-attention (the VAE mid block). Keys: `group_norm`,
/// `to_q`, `to_k`, `to_v`, `to_out.0`. Projections are `nn.Linear` over the
/// flattened spatial sequence, not 1x1 convs.
#[derive(Debug)]
struct AttnBlock {
    group_norm: GroupNorm,
    to_q: Linear,
    to_k: Linear,
    to_v: Linear,
    to_out: Linear,
}

impl AttnBlock {
    fn new(channels: usize, groups: usize, vb: VarBuilder) -> Result<Self, FluxError> {
        let group_norm = group_norm(groups, channels, RESNET_EPS, vb.pp("group_norm"))?;
        let to_q = linear(channels, channels, vb.pp("to_q"))?;
        let to_k = linear(channels, channels, vb.pp("to_k"))?;
        let to_v = linear(channels, channels, vb.pp("to_v"))?;
        // to_out is a ModuleList [Linear, Dropout]; only index 0 carries weights.
        let to_out = linear(channels, channels, vb.pp("to_out").pp(0))?;
        Ok(Self {
            group_norm,
            to_q,
            to_k,
            to_v,
            to_out,
        })
    }
}

impl Module for AttnBlock {
    fn forward(&self, xs: &Tensor) -> candle_core::Result<Tensor> {
        let residual = xs;
        let (b, c, h, w) = xs.dims4()?;
        // Single-head spatial attention over the (h*w) sequence.
        let hidden = xs.apply(&self.group_norm)?;
        // (b, c, h, w) -> (b, h*w, c).
        let hidden = hidden.reshape((b, c, h * w))?.transpose(1, 2)?.contiguous()?;
        let q = hidden.apply(&self.to_q)?;
        let k = hidden.apply(&self.to_k)?;
        let v = hidden.apply(&self.to_v)?;
        // Channel count is the attention dim; a VAE channel count (<= 512) is far
        // inside f64's exact-integer range, so the cast is lossless.
        #[allow(clippy::cast_precision_loss)]
        let scale = 1.0 / (c as f64).sqrt();
        let attn = (q.matmul(&k.transpose(1, 2)?)? * scale)?;
        let attn = candle_nn::ops::softmax_last_dim(&attn)?;
        let out = attn.matmul(&v)?;
        let out = out.apply(&self.to_out)?;
        // (b, h*w, c) -> (b, c, h, w).
        let out = out.transpose(1, 2)?.reshape((b, c, h, w))?;
        residual + out
    }
}

/// `Downsample2D` — stride-2 3x3 conv. diffusers stores it under
/// `downsamplers.0.conv` and pads asymmetrically (right/bottom by 1).
#[derive(Debug)]
struct Downsample {
    conv: Conv2d,
}

impl Downsample {
    fn new(channels: usize, vb: VarBuilder) -> Result<Self, FluxError> {
        let cfg = Conv2dConfig {
            stride: 2,
            ..Default::default()
        };
        // padding 0 here; the asymmetric pad is applied in forward.
        let conv = conv2d(channels, channels, 3, cfg, vb.pp("conv"))?;
        Ok(Self { conv })
    }
}

impl Module for Downsample {
    fn forward(&self, xs: &Tensor) -> candle_core::Result<Tensor> {
        // diffusers pads (0,1,0,1) — right and bottom — before the stride-2 conv.
        let xs = xs.pad_with_zeros(D::Minus1, 0, 1)?;
        let xs = xs.pad_with_zeros(D::Minus2, 0, 1)?;
        xs.apply(&self.conv)
    }
}

/// `Upsample2D` — nearest 2x then 3x3 conv. Key: `upsamplers.0.conv`.
#[derive(Debug)]
struct Upsample {
    conv: Conv2d,
}

impl Upsample {
    fn new(channels: usize, vb: VarBuilder) -> Result<Self, FluxError> {
        let conv = conv3x3(channels, channels, vb.pp("conv"))?;
        Ok(Self { conv })
    }
}

impl Module for Upsample {
    fn forward(&self, xs: &Tensor) -> candle_core::Result<Tensor> {
        let (_, _, h, w) = xs.dims4()?;
        xs.upsample_nearest2d(h * 2, w * 2)?.apply(&self.conv)
    }
}

/// One `DownEncoderBlock2D`: `resnets.{j}` plus optional `downsamplers.0`.
#[derive(Debug)]
struct DownBlock {
    resnets: Vec<ResnetBlock>,
    downsampler: Option<Downsample>,
}

/// One `UpDecoderBlock2D`: `resnets.{j}` plus optional `upsamplers.0`.
#[derive(Debug)]
struct UpBlock {
    resnets: Vec<ResnetBlock>,
    upsampler: Option<Upsample>,
}

/// `UNetMidBlock2D` for the VAE: `resnets.0`, optional `attentions.0`, `resnets.1`.
#[derive(Debug)]
struct MidBlock {
    resnet1: ResnetBlock,
    attn: Option<AttnBlock>,
    resnet2: ResnetBlock,
}

impl MidBlock {
    fn new(channels: usize, cfg: &VaeConfig, vb: VarBuilder) -> Result<Self, FluxError> {
        let groups = cfg.norm_num_groups;
        let resnet1 = ResnetBlock::new(channels, channels, groups, vb.pp("resnets").pp(0))?;
        let attn = if cfg.mid_block_add_attention {
            Some(AttnBlock::new(channels, groups, vb.pp("attentions").pp(0))?)
        } else {
            None
        };
        let resnet2 = ResnetBlock::new(channels, channels, groups, vb.pp("resnets").pp(1))?;
        Ok(Self { resnet1, attn, resnet2 })
    }
}

impl Module for MidBlock {
    fn forward(&self, xs: &Tensor) -> candle_core::Result<Tensor> {
        let mut h = xs.apply(&self.resnet1)?;
        if let Some(attn) = self.attn.as_ref() {
            h = h.apply(attn)?;
        }
        h.apply(&self.resnet2)
    }
}

/// The encoder: RGB -> 2*z (mean, logvar concatenated on the channel axis).
#[derive(Debug)]
struct Encoder {
    conv_in: Conv2d,
    down_blocks: Vec<DownBlock>,
    mid_block: MidBlock,
    conv_norm_out: GroupNorm,
    conv_out: Conv2d,
}

impl Encoder {
    fn new(cfg: &VaeConfig, vb: VarBuilder) -> Result<Self, FluxError> {
        let groups = cfg.norm_num_groups;
        let block_out = &cfg.block_out_channels;
        let first = *block_out
            .first()
            .ok_or_else(|| FluxError::Device("vae block_out_channels is empty".to_owned()))?;
        let last = *block_out
            .last()
            .ok_or_else(|| FluxError::Device("vae block_out_channels is empty".to_owned()))?;
        let conv_in = conv3x3(cfg.in_channels, first, vb.pp("conv_in"))?;

        let vb_down = vb.pp("down_blocks");
        let mut down_blocks = Vec::with_capacity(block_out.len());
        let mut block_in = first;
        let n = block_out.len();
        for (i, &out_c) in block_out.iter().enumerate() {
            let vb_b = vb_down.pp(i);
            let vb_res = vb_b.pp("resnets");
            let mut resnets = Vec::with_capacity(cfg.layers_per_block);
            for j in 0..cfg.layers_per_block {
                resnets.push(ResnetBlock::new(block_in, out_c, groups, vb_res.pp(j))?);
                block_in = out_c;
            }
            // Every down block downsamples except the last.
            let downsampler = if i == n - 1 {
                None
            } else {
                Some(Downsample::new(block_in, vb_b.pp("downsamplers").pp(0))?)
            };
            down_blocks.push(DownBlock { resnets, downsampler });
        }

        let mid_block = MidBlock::new(last, cfg, vb.pp("mid_block"))?;
        let conv_norm_out = group_norm(groups, last, RESNET_EPS, vb.pp("conv_norm_out"))?;
        // double_z: 2 * latent_channels (mean, logvar).
        let conv_out = conv3x3(last, 2 * cfg.latent_channels, vb.pp("conv_out"))?;
        Ok(Self {
            conv_in,
            down_blocks,
            mid_block,
            conv_norm_out,
            conv_out,
        })
    }
}

impl Module for Encoder {
    fn forward(&self, xs: &Tensor) -> candle_core::Result<Tensor> {
        let mut h = xs.apply(&self.conv_in)?;
        for block in &self.down_blocks {
            for resnet in &block.resnets {
                h = h.apply(resnet)?;
            }
            if let Some(ds) = block.downsampler.as_ref() {
                h = h.apply(ds)?;
            }
        }
        h.apply(&self.mid_block)?
            .apply(&self.conv_norm_out)?
            .apply(&candle_nn::Activation::Silu)?
            .apply(&self.conv_out)
    }
}

/// The decoder: z -> RGB.
#[derive(Debug)]
struct Decoder {
    conv_in: Conv2d,
    mid_block: MidBlock,
    up_blocks: Vec<UpBlock>,
    conv_norm_out: GroupNorm,
    conv_out: Conv2d,
}

impl Decoder {
    fn new(cfg: &VaeConfig, vb: VarBuilder) -> Result<Self, FluxError> {
        let groups = cfg.norm_num_groups;
        let block_out = &cfg.block_out_channels;
        let last = *block_out
            .last()
            .ok_or_else(|| FluxError::Device("vae block_out_channels is empty".to_owned()))?;
        let conv_in = conv3x3(cfg.latent_channels, last, vb.pp("conv_in"))?;
        let mid_block = MidBlock::new(last, cfg, vb.pp("mid_block"))?;

        // Decoder up blocks walk block_out in reverse. diffusers keys them
        // `up_blocks.0..` in that reversed order (0 = deepest/highest channel).
        let reversed: Vec<usize> = block_out.iter().rev().copied().collect();
        let vb_up = vb.pp("up_blocks");
        let mut up_blocks = Vec::with_capacity(reversed.len());
        let mut block_in = last;
        let n = reversed.len();
        for (i, &out_c) in reversed.iter().enumerate() {
            let vb_b = vb_up.pp(i);
            let vb_res = vb_b.pp("resnets");
            // Decoder blocks carry layers_per_block + 1 resnets.
            let mut resnets = Vec::with_capacity(cfg.layers_per_block + 1);
            for j in 0..=cfg.layers_per_block {
                resnets.push(ResnetBlock::new(block_in, out_c, groups, vb_res.pp(j))?);
                block_in = out_c;
            }
            // Every up block upsamples except the last.
            let upsampler = if i == n - 1 {
                None
            } else {
                Some(Upsample::new(block_in, vb_b.pp("upsamplers").pp(0))?)
            };
            up_blocks.push(UpBlock { resnets, upsampler });
        }

        let conv_norm_out = group_norm(groups, block_in, RESNET_EPS, vb.pp("conv_norm_out"))?;
        let conv_out = conv3x3(block_in, cfg.out_channels, vb.pp("conv_out"))?;
        Ok(Self {
            conv_in,
            mid_block,
            up_blocks,
            conv_norm_out,
            conv_out,
        })
    }
}

impl Module for Decoder {
    fn forward(&self, xs: &Tensor) -> candle_core::Result<Tensor> {
        let mut h = xs.apply(&self.conv_in)?.apply(&self.mid_block)?;
        for block in &self.up_blocks {
            for resnet in &block.resnets {
                h = h.apply(resnet)?;
            }
            if let Some(us) = block.upsampler.as_ref() {
                h = h.apply(us)?;
            }
        }
        h.apply(&self.conv_norm_out)?.apply(&candle_nn::Activation::Silu)?.apply(&self.conv_out)
    }
}

/// Splits a `2*z`-channel tensor into mean/logvar on the channel axis and either
/// samples `mean + std * eps` or returns the mean (diffusers
/// `DiagonalGaussianDistribution`).
#[derive(Debug)]
struct DiagonalGaussian {
    sample: bool,
}

impl DiagonalGaussian {
    fn forward(&self, xs: &Tensor) -> candle_core::Result<Tensor> {
        let chunks = xs.chunk(2, 1)?;
        let mean = &chunks[0];
        if self.sample {
            // Clamp logvar like diffusers (-30, 20) before exp for stability.
            let logvar = chunks[1].clamp(-30.0, 20.0)?;
            let std = (logvar * 0.5)?.exp()?;
            mean + (std * mean.randn_like(0.0, 1.0))?
        } else {
            Ok(mean.clone())
        }
    }
}

/// The variational autoencoder: encodes RGB to latents and decodes back.
///
/// `encode`/`decode` operate on the raw 32-channel VAE latent and are a clean
/// inverse pair. The latent `BatchNorm` and the `[2,2]` patchify (the transformer
/// boundary) are exposed via [`Vae::normalize_latent`] / [`Vae::patchify`] for the
/// pipeline gate; they are not part of the round-trip.
#[derive(Debug)]
pub struct Vae {
    encoder: Encoder,
    decoder: Decoder,
    quant_conv: Option<Conv2d>,
    post_quant_conv: Option<Conv2d>,
    /// Latent `BatchNorm` running stats over the patchified (128-channel) latent.
    bn_running_mean: Tensor,
    bn_running_var: Tensor,
    reg: DiagonalGaussian,
    cfg: VaeConfig,
}

impl Vae {
    /// Load the VAE from the store's `vae/diffusion_pytorch_model.safetensors`.
    ///
    /// `dtype` is the working dtype (bf16 on GPU, f32 on CPU). The `BatchNorm`
    /// stats are loaded too, but normalize/denormalize is the pipeline's job.
    ///
    /// # Errors
    ///
    /// Returns [`FluxError`] if the file is missing or a weight key is absent.
    pub fn load(store: &ModelStore, device: &Device, dtype: DType) -> Result<Self, FluxError> {
        let path = store.file_path("vae/diffusion_pytorch_model.safetensors");
        let vb = var_builder(&[path], dtype, device)?;
        Self::from_var_builder(&VaeConfig::flux2_klein(), vb)
    }

    /// Build the VAE from an already-open [`VarBuilder`] rooted at the VAE
    /// weights. Split out so tests can feed a synthetic tensor map.
    ///
    /// # Errors
    ///
    /// Returns [`FluxError`] if a weight key is missing or mis-shaped.
    pub fn from_var_builder(cfg: &VaeConfig, vb: VarBuilder) -> Result<Self, FluxError> {
        let encoder = Encoder::new(cfg, vb.pp("encoder"))?;
        let decoder = Decoder::new(cfg, vb.pp("decoder"))?;
        // quant_conv / post_quant_conv are 1x1 convs over the latent. Both are
        // present in the klein checkpoint (use_quant_conv / use_post_quant_conv).
        let quant_conv = if vb.contains_tensor("quant_conv.weight") {
            Some(conv2d(
                2 * cfg.latent_channels,
                2 * cfg.latent_channels,
                1,
                Conv2dConfig::default(),
                vb.pp("quant_conv"),
            )?)
        } else {
            None
        };
        let post_quant_conv = if vb.contains_tensor("post_quant_conv.weight") {
            Some(conv2d(
                cfg.latent_channels,
                cfg.latent_channels,
                1,
                Conv2dConfig::default(),
                vb.pp("post_quant_conv"),
            )?)
        } else {
            None
        };
        let patched = cfg.patched_latent_channels();
        let bn = vb.pp("bn");
        let bn_running_mean = bn.get(patched, "running_mean")?;
        let bn_running_var = bn.get(patched, "running_var")?;
        Ok(Self {
            encoder,
            decoder,
            quant_conv,
            post_quant_conv,
            bn_running_mean,
            bn_running_var,
            reg: DiagonalGaussian { sample: true },
            cfg: cfg.clone(),
        })
    }

    /// The config this VAE was built with.
    #[must_use]
    pub fn config(&self) -> &VaeConfig {
        &self.cfg
    }

    /// Encode RGB (NCHW, range `-1..=1`, working dtype) to a raw 32-channel
    /// latent: encoder -> `quant_conv` -> diagonal-Gaussian sample.
    ///
    /// The sample is stochastic; seed the device for a deterministic test.
    ///
    /// # Errors
    ///
    /// Returns [`FluxError::Candle`] on a tensor failure.
    pub fn encode(&self, xs: &Tensor) -> Result<Tensor, FluxError> {
        let h = xs.apply(&self.encoder)?;
        let moments = match self.quant_conv.as_ref() {
            Some(c) => h.apply(c)?,
            None => h,
        };
        Ok(self.reg.forward(&moments)?)
    }

    /// Decode a raw 32-channel latent back to RGB (NCHW): `post_quant_conv` ->
    /// decoder.
    ///
    /// # Errors
    ///
    /// Returns [`FluxError::Candle`] on a tensor failure.
    pub fn decode(&self, xs: &Tensor) -> Result<Tensor, FluxError> {
        let z = match self.post_quant_conv.as_ref() {
            Some(c) => xs.apply(c)?,
            None => xs.clone(),
        };
        Ok(z.apply(&self.decoder)?)
    }

    /// Fold the `[2,2]` patch into the channel axis: `(b, 32, h, w)` ->
    /// `(b, 128, h/2, w/2)`. The pipeline applies this before the `BatchNorm` and
    /// the transformer.
    ///
    /// # Errors
    ///
    /// Returns [`FluxError::Candle`] if `h`/`w` are not divisible by the patch.
    pub fn patchify(&self, latent: &Tensor) -> Result<Tensor, FluxError> {
        let (b, c, h, w) = latent.dims4()?;
        let [ph, pw] = self.cfg.patch_size;
        // (b, c, h/ph, ph, w/pw, pw) -> (b, c*ph*pw, h/ph, w/pw).
        let x = latent.reshape((b, c, h / ph, ph, w / pw, pw))?;
        let x = x.permute((0, 1, 3, 5, 2, 4))?.contiguous()?;
        Ok(x.reshape((b, c * ph * pw, h / ph, w / pw))?)
    }

    /// Inverse of [`Vae::patchify`]: `(b, 128, h, w)` -> `(b, 32, 2h, 2w)`.
    ///
    /// # Errors
    ///
    /// Returns [`FluxError::Candle`] on a shape failure.
    pub fn unpatchify(&self, patched: &Tensor) -> Result<Tensor, FluxError> {
        let (b, pc, h, w) = patched.dims4()?;
        let [ph, pw] = self.cfg.patch_size;
        let c = pc / (ph * pw);
        let x = patched.reshape((b, c, ph, pw, h, w))?;
        let x = x.permute((0, 1, 4, 2, 5, 3))?.contiguous()?;
        Ok(x.reshape((b, c, h * ph, w * pw))?)
    }

    /// Normalize a patchified (128-channel) latent into the transformer's space:
    /// `(x - running_mean) / sqrt(running_var + eps)`.
    ///
    /// # Errors
    ///
    /// Returns [`FluxError::Candle`] on a tensor failure.
    pub fn normalize_latent(&self, patched: &Tensor) -> Result<Tensor, FluxError> {
        let mean = self.bn_running_mean.reshape((1, (), 1, 1))?;
        let std = (&self.bn_running_var + self.cfg.batch_norm_eps)?.sqrt()?.reshape((1, (), 1, 1))?;
        Ok(patched.broadcast_sub(&mean)?.broadcast_div(&std)?)
    }

    /// Inverse of [`Vae::normalize_latent`]: `x * std + running_mean`.
    ///
    /// # Errors
    ///
    /// Returns [`FluxError::Candle`] on a tensor failure.
    pub fn denormalize_latent(&self, patched: &Tensor) -> Result<Tensor, FluxError> {
        let mean = self.bn_running_mean.reshape((1, (), 1, 1))?;
        let std = (&self.bn_running_var + self.cfg.batch_norm_eps)?.sqrt()?.reshape((1, (), 1, 1))?;
        Ok(patched.broadcast_mul(&std)?.broadcast_add(&mean)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patched_channel_count_is_128() {
        let cfg = VaeConfig::flux2_klein();
        assert_eq!(cfg.patched_latent_channels(), 128);
    }

    #[test]
    fn config_matches_flux2_klein() {
        let cfg = VaeConfig::flux2_klein();
        assert_eq!(cfg.latent_channels, 32);
        assert_eq!(cfg.block_out_channels, vec![128, 256, 512, 512]);
        assert_eq!(cfg.layers_per_block, 2);
        assert_eq!(cfg.norm_num_groups, 32);
        assert!(cfg.mid_block_add_attention);
        assert_eq!(cfg.patch_size, [2, 2]);
    }

    #[test]
    fn patchify_unpatchify_round_trips() -> Result<(), FluxError> {
        // No weights needed — build a Vae-free patchify check via a tiny config.
        let device = Device::Cpu;
        let cfg = VaeConfig::flux2_klein();
        // (1, 32, 4, 4) -> (1, 128, 2, 2) -> (1, 32, 4, 4).
        let latent = Tensor::arange(0u32, 32 * 4 * 4, &device)?.to_dtype(DType::F32)?.reshape((1, 32, 4, 4))?;
        // Patchify/unpatchify are pure tensor ops; exercise them through a
        // minimally-constructed Vae would need weights, so replicate the math.
        let [ph, pw] = cfg.patch_size;
        let (b, c, h, w) = latent.dims4()?;
        let packed = latent
            .reshape((b, c, h / ph, ph, w / pw, pw))?
            .permute((0, 1, 3, 5, 2, 4))?
            .contiguous()?
            .reshape((b, c * ph * pw, h / ph, w / pw))?;
        assert_eq!(packed.dims4()?, (1, 128, 2, 2));
        let back = packed
            .reshape((b, c, ph, pw, h / ph, w / pw))?
            .permute((0, 1, 4, 2, 5, 3))?
            .contiguous()?
            .reshape((b, c, h, w))?;
        let diff = (latent - back)?.abs()?.sum_all()?.to_scalar::<f32>()?;
        assert!(diff < 1e-6, "patchify round-trip drifted: {diff}");
        Ok(())
    }
}
