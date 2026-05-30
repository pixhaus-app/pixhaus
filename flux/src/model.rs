//! The FLUX.2 `DiT` transformer topology and Qwen3-conditioned forward.
//!
//! There is no `flux2` module in candle-transformers — only FLUX.1's `flux`. So
//! the FLUX.2 block topology is assembled here on candle's base APIs (candle-core
//! `Tensor`, candle-nn `VarBuilder`/`Linear`/`LayerNorm`/`RmsNorm`/`ops`),
//! reimplementing the module-private helpers candle keeps internal
//! (`apply_rope`, scaled-dot-product `attention`, `timestep_embedding`, the
//! modulation split) and rewriting the blocks for the FLUX.2 conditioning.
//!
//! Why this is a rewrite, not a reuse of `candle_transformers::models::flux::model`:
//! candle's `Config`/`Flux`/`DoubleStreamBlock`/`SingleStreamBlock`/`SelfAttention`
//! constructors are module-private and hardcode the FLUX.1 BFL weight keys
//! (`img_attn.qkv`, `linear1`, `adaLN_modulation`) and a 3-axis `RoPE` plus a CLIP
//! pooled-vector modulation. FLUX.2 klein ships diffusers-format keys
//! (`to_q`/`to_k`/`to_v`, `add_q_proj`, `to_qkv_mlp_proj`, `norm_out`/`proj_out`),
//! a 4-axis (t,h,w,l) real-valued `RoPE`, QK-RMSNorm before rotary, and Qwen3 text
//! conditioning with NO pooled vector. The shapes match candle's blocks closely
//! enough that this is a guided rewrite (per `.tmp/lf-research/01-flux2-transformer.md`
//! section 5 port/reuse table), not a from-zero design.
//!
//! Distilled posture: klein is guidance-distilled. There is no guidance embedder
//! (`guidance_embeds=false`) and CFG is off — the sampler passes `guidance = 1.0`
//! as a no-op. The forward here takes no guidance argument at all.

use std::path::Path;

use candle_core::{D, DType, Device, IndexOp, Module, Tensor};
use candle_nn::{LayerNorm, Linear, RmsNorm, VarBuilder, linear, linear_b, rms_norm};

use crate::FluxError;
use crate::loader::var_builder;
use crate::store::ModelStore;

/// The FLUX.2 klein-4B transformer config. Numbers from
/// `FLUX.2-klein-4B/transformer/config.json` (`Flux2Transformer2DModel`), recorded
/// in `.tmp/lf-research/01-flux2-transformer.md` section 1. These are the klein
/// values, NOT the diffusers 32B-dev defaults.
#[derive(Clone, Debug)]
pub struct Flux2Config {
    /// Packed latent channels entering the `DiT` (VAE patchified 32 -> 128).
    pub in_channels: usize,
    /// Output channels = `in_channels` (reference sets `out_channels = in_channels`).
    pub out_channels: usize,
    /// Dim of the Qwen3 text-embed fed to `context_embedder` (3 layers * 2560).
    pub context_in_dim: usize,
    /// `inner_dim` = `num_heads * head_dim` = 24 * 128 = 3072.
    pub hidden_size: usize,
    /// FFN hidden = `hidden_size * mlp_ratio`.
    pub mlp_ratio: f64,
    /// Attention heads.
    pub num_heads: usize,
    /// Double-stream (`Flux2TransformerBlock`) count.
    pub depth: usize,
    /// Single-stream (`Flux2SingleTransformerBlock`) count.
    pub depth_single_blocks: usize,
    /// The 4 `RoPE` axes (t, h, w, l), each 32, summing to `head_dim` = 128.
    pub axes_dim: Vec<usize>,
    /// `RoPE` base.
    pub theta: usize,
    /// Sinusoidal timestep projection width before the time MLP.
    pub timestep_channels: usize,
    /// Norm epsilon.
    pub eps: f64,
}

impl Flux2Config {
    /// The klein-4B config. Verbatim from `transformer/config.json`.
    #[must_use]
    pub fn klein() -> Self {
        Self {
            in_channels: 128,
            out_channels: 128,
            context_in_dim: 7680,
            hidden_size: 3072,
            mlp_ratio: 3.0,
            num_heads: 24,
            depth: 5,
            depth_single_blocks: 20,
            axes_dim: vec![32, 32, 32, 32],
            theta: 2000,
            timestep_channels: 256,
            eps: 1e-6,
        }
    }

    /// Per-head dim = `hidden_size / num_heads` = 128.
    #[must_use]
    pub fn head_dim(&self) -> usize {
        self.hidden_size / self.num_heads
    }

    /// FFN hidden width = `hidden_size * mlp_ratio` = 9216.
    #[must_use]
    pub fn mlp_hidden(&self) -> usize {
        // mlp_ratio is exactly 3.0 and hidden_size is small; the product
        // 3072*3 = 9216 is exact in f64, so the casts are lossless here.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::cast_precision_loss)]
        let h = (self.hidden_size as f64 * self.mlp_ratio) as usize;
        h
    }
}

// LayerNorm with no affine params (elementwise_affine=false), matching FLUX.2's
// `norm1`/`norm2`/`norm` — modulation supplies the scale/shift, so the norm itself
// learns nothing. Reimplemented locally because candle's `layer_norm` helper is
// module-private in the flux model.
fn layer_norm_no_affine(dim: usize, eps: f64, device: &candle_core::Device, dtype: DType) -> Result<LayerNorm, FluxError> {
    let ws = Tensor::ones(dim, dtype, device)?;
    Ok(LayerNorm::new_no_bias(ws, eps))
}

/// Sinusoidal timestep embedding. Reimplemented locally — candle's
/// `timestep_embedding` is module-private. Matches the FLUX reference: scale the
/// timestep by 1000, build `half` log-spaced frequencies, concat cos||sin.
//
// half (128) and the period are exact in f64; the index/period casts are lossless.
#[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
fn timestep_embedding(t: &Tensor, dim: usize, dtype: DType) -> Result<Tensor, FluxError> {
    const TIME_FACTOR: f64 = 1000.0;
    const MAX_PERIOD: f64 = 10000.0;
    if dim % 2 == 1 {
        return Err(FluxError::Device(format!("timestep embedding dim {dim} is odd")));
    }
    let dev = t.device();
    let half = dim / 2;
    let t = (t * TIME_FACTOR)?;
    let arange = Tensor::arange(0, half as u32, dev)?.to_dtype(DType::F32)?;
    let freqs = (arange * (-MAX_PERIOD.ln() / half as f64))?.exp()?;
    let args = t.unsqueeze(1)?.to_dtype(DType::F32)?.broadcast_mul(&freqs.unsqueeze(0)?)?;
    let emb = Tensor::cat(&[args.cos()?, args.sin()?], D::Minus1)?.to_dtype(dtype)?;
    Ok(emb)
}

/// Real-valued 4-axis rotary tables for FLUX.2. Net-new: candle's `EmbedNd`/`rope`
/// build the FLUX.1 stacked complex form `(b, n, d, 2, 2)`; FLUX.2 uses diffusers'
/// `get_1d_rotary_pos_embed(..., repeat_interleave_real=True, use_real=True)` per
/// axis, concatenated to `head_dim`, applied as real `(cos, sin)` with
/// `repeat_interleave`. Implements `.tmp/lf-research/01-flux2-transformer.md`
/// section 3.
#[derive(Debug, Clone)]
pub struct Flux2PosEmbed {
    theta: usize,
    axes_dim: Vec<usize>,
}

impl Flux2PosEmbed {
    /// `axes_dim` are the per-axis `RoPE` widths (klein: `[32, 32, 32, 32]`).
    #[must_use]
    pub fn new(theta: usize, axes_dim: Vec<usize>) -> Self {
        Self { theta, axes_dim }
    }

    /// Build `(cos, sin)` tables for position ids `(seq, n_axes)`. Each is
    /// `(seq, head_dim)` after concatenating the per-axis tables and
    /// `repeat_interleave`-ing the half-width frequencies to full `head_dim`.
    //
    // theta and the index casts are small integers, lossless in f64/f32.
    #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
    fn tables(&self, ids: &Tensor) -> Result<(Tensor, Tensor), FluxError> {
        let n_axes = ids.dim(D::Minus1)?;
        if n_axes != self.axes_dim.len() {
            return Err(FluxError::Device(format!("ids have {n_axes} axes, config declares {}", self.axes_dim.len())));
        }
        let dev = ids.device();
        let theta = self.theta as f64;
        let mut cos_axes = Vec::with_capacity(n_axes);
        let mut sin_axes = Vec::with_capacity(n_axes);
        for (axis, &axis_dim) in self.axes_dim.iter().enumerate() {
            if axis_dim % 2 == 1 {
                return Err(FluxError::Device(format!("axis {axis} dim {axis_dim} is odd")));
            }
            // pos for this axis: (seq,) f32.
            let pos = ids.get_on_dim(D::Minus1, axis)?.to_dtype(DType::F32)?;
            let seq = pos.dim(0)?;
            // half-width inverse frequencies, then repeat_interleave_real doubles
            // each freq so cos/sin reach the full axis_dim.
            let inv_freq: Vec<f32> = (0..axis_dim).step_by(2).map(|i| 1f32 / theta.powf(i as f64 / axis_dim as f64) as f32).collect();
            let half = inv_freq.len();
            let inv_freq = Tensor::from_vec(inv_freq, (1, half), dev)?;
            // (seq, 1) x (1, half) -> (seq, half).
            let freqs = pos.reshape((seq, 1))?.matmul(&inv_freq)?;
            let cos = freqs.cos()?;
            let sin = freqs.sin()?;
            // repeat_interleave_real=True: each of the `half` freqs is duplicated
            // adjacently to width `axis_dim`. Stack on a new last axis then flatten.
            let cos = Tensor::stack(&[&cos, &cos], 2)?.reshape((seq, axis_dim))?;
            let sin = Tensor::stack(&[&sin, &sin], 2)?.reshape((seq, axis_dim))?;
            cos_axes.push(cos);
            sin_axes.push(sin);
        }
        let cos = Tensor::cat(&cos_axes, D::Minus1)?;
        let sin = Tensor::cat(&sin_axes, D::Minus1)?;
        Ok((cos, sin))
    }
}

/// The rotary tables produced once per forward and shared by every block.
#[derive(Debug, Clone)]
struct Rope {
    // (1, 1, seq, head_dim) so they broadcast over (b, heads, seq, head_dim).
    cos: Tensor,
    sin: Tensor,
}

impl Rope {
    fn build(embed: &Flux2PosEmbed, ids: &Tensor, dtype: DType) -> Result<Self, FluxError> {
        let (cos, sin) = embed.tables(ids)?;
        let cos = cos.unsqueeze(0)?.unsqueeze(0)?.to_dtype(dtype)?;
        let sin = sin.unsqueeze(0)?.unsqueeze(0)?.to_dtype(dtype)?;
        Ok(Self { cos, sin })
    }
}

/// Apply real-valued rotary to `x` of shape `(b, heads, seq, head_dim)`. Net-new:
/// the FLUX.2 form is the diffusers `apply_rotary_emb` with
/// `repeat_interleave_real=True` — `x_rotated = x*cos + rotate_half(x)*sin`, where
/// `rotate_half` negates-and-swaps adjacent pairs `(x0, x1) -> (-x1, x0)`. candle's
/// private `apply_rope` consumes the stacked-complex FLUX.1 tables, which is a
/// different layout, so this is reimplemented.
// b/h/l/d are the candle tensor-math axis convention (batch, heads, seq, dim).
#[allow(clippy::many_single_char_names)]
fn apply_rope(x: &Tensor, rope: &Rope) -> Result<Tensor, FluxError> {
    let (b, h, l, d) = x.dims4()?;
    // rotate_half on adjacent pairs: view (..., d/2, 2), swap and negate.
    let x_pairs = x.reshape((b, h, l, d / 2, 2))?;
    let x0 = x_pairs.narrow(D::Minus1, 0, 1)?;
    let x1 = x_pairs.narrow(D::Minus1, 1, 1)?;
    // (-x1, x0) interleaved back to (..., d).
    let rotated = Tensor::cat(&[&x1.neg()?, &x0], D::Minus1)?.reshape((b, h, l, d))?;
    let out = x.broadcast_mul(&rope.cos)?.add(&rotated.broadcast_mul(&rope.sin)?)?;
    Ok(out)
}

/// Scaled-dot-product attention over `(b, heads, seq, head_dim)`, no mask (the
/// FLUX sequence is fully visible). Reimplemented locally — candle's
/// `scaled_dot_product_attention` is module-private.
//
// head_dim is small and exact in f64; the scale cast is lossless.
#[allow(clippy::cast_precision_loss)]
fn attention(q: &Tensor, k: &Tensor, v: &Tensor) -> Result<Tensor, FluxError> {
    let dim = q.dim(D::Minus1)?;
    let scale = 1.0 / (dim as f64).sqrt();
    let q = q.contiguous()?;
    let k = k.contiguous()?;
    let v = v.contiguous()?;
    let scores = (q.matmul(&k.transpose(D::Minus2, D::Minus1)?)? * scale)?;
    let probs = candle_nn::ops::softmax_last_dim(&scores)?;
    let ctx = probs.matmul(&v)?;
    Ok(ctx)
}

/// One modulation parameter set: shift, scale, gate. Mirrors FLUX.1's
/// `ModulationOut` (the helper struct is module-private, so reimplemented).
struct ModulationOut {
    shift: Tensor,
    scale: Tensor,
    gate: Tensor,
}

impl ModulationOut {
    fn scale_shift(&self, xs: &Tensor) -> Result<Tensor, FluxError> {
        Ok(xs.broadcast_mul(&(&self.scale + 1.0)?)?.broadcast_add(&self.shift)?)
    }

    fn gate(&self, xs: &Tensor) -> Result<Tensor, FluxError> {
        Ok(self.gate.broadcast_mul(xs)?)
    }
}

/// `Flux2Modulation`: `linear(silu(temb))` split into `n_sets` shift/scale/gate
/// triplets. Net-new — replaces FLUX.1's `Modulation1`/`Modulation2` (which are
/// module-private) with one struct parameterized by set count, matching diffusers'
/// `Flux2Modulation(inner_dim, mod_param_sets)`.
#[derive(Debug, Clone)]
struct Flux2Modulation {
    lin: Linear,
    n_sets: usize,
}

impl Flux2Modulation {
    #[allow(clippy::needless_pass_by_value)]
    fn new(dim: usize, n_sets: usize, vb: VarBuilder) -> Result<Self, FluxError> {
        // 3 params (shift, scale, gate) per set.
        let lin = linear(dim, 3 * n_sets * dim, vb.pp("lin"))?;
        Ok(Self { lin, n_sets })
    }

    fn forward(&self, temb: &Tensor) -> Result<Vec<ModulationOut>, FluxError> {
        let ys = temb.silu()?.apply(&self.lin)?.unsqueeze(1)?.chunk(3 * self.n_sets, D::Minus1)?;
        if ys.len() != 3 * self.n_sets {
            return Err(FluxError::Device(format!(
                "modulation chunk produced {} parts, expected {}",
                ys.len(),
                3 * self.n_sets
            )));
        }
        let mut out = Vec::with_capacity(self.n_sets);
        for set in 0..self.n_sets {
            out.push(ModulationOut {
                shift: ys[set * 3].clone(),
                scale: ys[set * 3 + 1].clone(),
                gate: ys[set * 3 + 2].clone(),
            });
        }
        Ok(out)
    }
}

/// QK-RMSNorm over `head_dim`, applied to q and k before rotary. Same shape as
/// candle's `QkNorm`; reimplemented because that struct's constructor is private
/// and FLUX.2's diffusers keys are `norm_q`/`norm_k` (vs FLUX.1's
/// `query_norm`/`key_norm`).
#[derive(Debug, Clone)]
struct QkNorm {
    norm_q: RmsNorm,
    norm_k: RmsNorm,
}

impl QkNorm {
    #[allow(clippy::needless_pass_by_value)]
    fn new(head_dim: usize, eps: f64, vb: VarBuilder) -> Result<Self, FluxError> {
        let norm_q = rms_norm(head_dim, eps, vb.pp("norm_q"))?;
        let norm_k = rms_norm(head_dim, eps, vb.pp("norm_k"))?;
        Ok(Self { norm_q, norm_k })
    }
}

/// `Flux2FeedForward`: a GELU MLP at `mlp_ratio`. Net-new (FLUX.1's `Mlp` is
/// private and uses numeric `0`/`2` keys); diffusers keys the two linears
/// `net.0.proj` and `net.2`.
#[derive(Debug, Clone)]
struct FeedForward {
    proj_in: Linear,
    proj_out: Linear,
}

impl FeedForward {
    #[allow(clippy::needless_pass_by_value)]
    fn new(dim: usize, hidden: usize, vb: VarBuilder) -> Result<Self, FluxError> {
        let proj_in = linear(dim, hidden, vb.pp("net.0.proj"))?;
        let proj_out = linear(hidden, dim, vb.pp("net.2"))?;
        Ok(Self { proj_in, proj_out })
    }
}

impl Module for FeedForward {
    fn forward(&self, xs: &Tensor) -> candle_core::Result<Tensor> {
        xs.apply(&self.proj_in)?.gelu()?.apply(&self.proj_out)
    }
}

// Split a packed qkv tensor (b, l, 3*inner) into per-head q, k, v of shape
// (b, heads, l, head_dim). Shared by both block types.
// b/l are the candle tensor-math axis convention.
#[allow(clippy::many_single_char_names)]
fn split_heads_qkv(qkv: &Tensor, num_heads: usize) -> Result<(Tensor, Tensor, Tensor), FluxError> {
    let (b, l, _) = qkv.dims3()?;
    let qkv = qkv.reshape((b, l, 3, num_heads, ()))?;
    let q = qkv.i((.., .., 0))?.transpose(1, 2)?.contiguous()?;
    let k = qkv.i((.., .., 1))?.transpose(1, 2)?.contiguous()?;
    let v = qkv.i((.., .., 2))?.transpose(1, 2)?.contiguous()?;
    Ok((q, k, v))
}

// Project one stream's hidden states to per-head q, k, v through three separate
// linears, then apply QK-RMSNorm. Returns (b, heads, l, head_dim).
// q/k/v/b/l are the candle tensor-math convention.
#[allow(clippy::many_single_char_names)]
fn project_qkv(x: &Tensor, q_proj: &Linear, k_proj: &Linear, v_proj: &Linear, norm: &QkNorm, num_heads: usize) -> Result<(Tensor, Tensor, Tensor), FluxError> {
    let (b, l, _) = x.dims3()?;
    let to_heads = |t: &Tensor| -> Result<Tensor, FluxError> { Ok(t.reshape((b, l, num_heads, ()))?.transpose(1, 2)?.contiguous()?) };
    let q = to_heads(&x.apply(q_proj)?)?;
    let k = to_heads(&x.apply(k_proj)?)?;
    let v = to_heads(&x.apply(v_proj)?)?;
    // QK-RMSNorm over head_dim, before rotary.
    let q = q.apply(&norm.norm_q)?;
    let k = k.apply(&norm.norm_k)?;
    Ok((q, k, v))
}

// Merge per-head attention output (b, heads, l, head_dim) back to (b, l, inner).
// b/h/l/d are the candle tensor-math axis convention.
#[allow(clippy::many_single_char_names)]
fn merge_heads(ctx: &Tensor) -> Result<Tensor, FluxError> {
    let (b, h, l, d) = ctx.dims4()?;
    Ok(ctx.transpose(1, 2)?.reshape((b, l, h * d))?)
}

/// `Flux2TransformerBlock` (double-stream): separate img and txt streams that
/// cross-attend through one joint attention, each with its own modulation, norm,
/// QK-norm, and FFN. Net-new (reuse shape, rewrite mod/norm) per the port table:
/// FLUX.2 uses `LayerNorm(affine=false)` + a 2-set `Flux2Modulation`, separate
/// `to_q/to_k/to_v` + `add_q_proj/add_k_proj/add_v_proj` (diffusers keys), and the
/// 4-axis real rotary.
#[derive(Debug, Clone)]
struct DoubleStreamBlock {
    img_mod: Flux2Modulation,
    txt_mod: Flux2Modulation,
    img_norm1: LayerNorm,
    img_norm2: LayerNorm,
    txt_norm1: LayerNorm,
    txt_norm2: LayerNorm,
    // img-stream projections.
    to_q: Linear,
    to_k: Linear,
    to_v: Linear,
    to_out: Linear,
    img_qk_norm: QkNorm,
    // txt-stream (added) projections.
    add_q_proj: Linear,
    add_k_proj: Linear,
    add_v_proj: Linear,
    to_add_out: Linear,
    txt_qk_norm: QkNorm,
    img_mlp: FeedForward,
    txt_mlp: FeedForward,
    num_heads: usize,
}

impl DoubleStreamBlock {
    #[allow(clippy::needless_pass_by_value)]
    fn new(cfg: &Flux2Config, vb: VarBuilder) -> Result<Self, FluxError> {
        let h = cfg.hidden_size;
        let dev = vb.device();
        let dt = vb.dtype();
        let head_dim = cfg.head_dim();
        let attn = vb.pp("attn");
        Ok(Self {
            img_mod: Flux2Modulation::new(h, 2, vb.pp("img_mod"))?,
            txt_mod: Flux2Modulation::new(h, 2, vb.pp("txt_mod"))?,
            img_norm1: layer_norm_no_affine(h, cfg.eps, dev, dt)?,
            img_norm2: layer_norm_no_affine(h, cfg.eps, dev, dt)?,
            txt_norm1: layer_norm_no_affine(h, cfg.eps, dev, dt)?,
            txt_norm2: layer_norm_no_affine(h, cfg.eps, dev, dt)?,
            to_q: linear(h, h, attn.pp("to_q"))?,
            to_k: linear(h, h, attn.pp("to_k"))?,
            to_v: linear(h, h, attn.pp("to_v"))?,
            to_out: linear(h, h, attn.pp("to_out.0"))?,
            img_qk_norm: QkNorm::new(head_dim, cfg.eps, attn.clone())?,
            add_q_proj: linear(h, h, attn.pp("add_q_proj"))?,
            add_k_proj: linear(h, h, attn.pp("add_k_proj"))?,
            add_v_proj: linear(h, h, attn.pp("add_v_proj"))?,
            to_add_out: linear(h, h, attn.pp("to_add_out"))?,
            txt_qk_norm: QkNorm::new(head_dim, cfg.eps, attn.pp("added"))?,
            img_mlp: FeedForward::new(h, cfg.mlp_hidden(), vb.pp("img_mlp"))?,
            txt_mlp: FeedForward::new(h, cfg.mlp_hidden(), vb.pp("txt_mlp"))?,
            num_heads: cfg.num_heads,
        })
    }

    fn forward(&self, img: &Tensor, txt: &Tensor, temb: &Tensor, rope: &Rope) -> Result<(Tensor, Tensor), FluxError> {
        let img_mods = self.img_mod.forward(temb)?;
        let txt_mods = self.txt_mod.forward(temb)?;
        // 2 sets each: [attn, mlp].
        let (img_m1, img_m2) = (&img_mods[0], &img_mods[1]);
        let (txt_m1, txt_m2) = (&txt_mods[0], &txt_mods[1]);

        let img_modulated = img_m1.scale_shift(&img.apply(&self.img_norm1)?)?;
        let txt_modulated = txt_m1.scale_shift(&txt.apply(&self.txt_norm1)?)?;

        let (img_q, img_k, img_v) = project_qkv(&img_modulated, &self.to_q, &self.to_k, &self.to_v, &self.img_qk_norm, self.num_heads)?;
        let (txt_q, txt_k, txt_v) = project_qkv(
            &txt_modulated,
            &self.add_q_proj,
            &self.add_k_proj,
            &self.add_v_proj,
            &self.txt_qk_norm,
            self.num_heads,
        )?;

        // Apply rotary per stream, then join. The shared rope table is indexed
        // text-first then image (matching the id concat order in the forward).
        let txt_len = txt.dim(1)?;
        let img_len = img.dim(1)?;
        let (rope_txt, rope_img) = split_rope(rope, txt_len, img_len)?;
        let img_q = apply_rope(&img_q, &rope_img)?;
        let img_k = apply_rope(&img_k, &rope_img)?;
        let txt_q = apply_rope(&txt_q, &rope_txt)?;
        let txt_k = apply_rope(&txt_k, &rope_txt)?;

        // Joint attention: text first, then image (sequence dim is dim 2 here).
        let q = Tensor::cat(&[&txt_q, &img_q], 2)?;
        let k = Tensor::cat(&[&txt_k, &img_k], 2)?;
        let v = Tensor::cat(&[&txt_v, &img_v], 2)?;
        let attn = attention(&q, &k, &v)?;
        let attn = merge_heads(&attn)?;
        let txt_attn = attn.narrow(1, 0, txt_len)?;
        let img_attn = attn.narrow(1, txt_len, img_len)?;

        // img stream: gated attn add, then gated FFN.
        let img = (img + img_m1.gate(&img_attn.apply(&self.to_out)?)?)?;
        let img = (&img + img_m2.gate(&img_m2.scale_shift(&img.apply(&self.img_norm2)?)?.apply(&self.img_mlp)?)?)?;
        // txt stream: same shape.
        let txt = (txt + txt_m1.gate(&txt_attn.apply(&self.to_add_out)?)?)?;
        let txt = (&txt + txt_m2.gate(&txt_m2.scale_shift(&txt.apply(&self.txt_norm2)?)?.apply(&self.txt_mlp)?)?)?;
        Ok((img, txt))
    }
}

/// `Flux2SingleTransformerBlock` (single-stream): a fused parallel block over the
/// concatenated sequence. `to_qkv_mlp_proj` produces QKV and the MLP-in together;
/// attention and the gated MLP run in parallel; `to_out` recombines. Net-new
/// (reuse fused-parallel idea): FLUX.1's `SingleStreamBlock` is the same pattern
/// with `linear1`/`linear2` keys and 1 modulation set; FLUX.2 uses
/// `to_qkv_mlp_proj`/`to_out` (diffusers keys, confirmed in the FLUX2 dreambooth
/// docs) and the 4-axis rotary.
#[derive(Debug, Clone)]
struct SingleStreamBlock {
    modulation: Flux2Modulation,
    norm: LayerNorm,
    qk_norm: QkNorm,
    to_qkv_mlp_proj: Linear,
    to_out: Linear,
    hidden_size: usize,
    mlp_hidden: usize,
    num_heads: usize,
}

impl SingleStreamBlock {
    #[allow(clippy::needless_pass_by_value)]
    fn new(cfg: &Flux2Config, vb: VarBuilder) -> Result<Self, FluxError> {
        let h = cfg.hidden_size;
        let mlp = cfg.mlp_hidden();
        let head_dim = cfg.head_dim();
        let attn = vb.pp("attn");
        Ok(Self {
            modulation: Flux2Modulation::new(h, 1, vb.pp("norm"))?,
            norm: layer_norm_no_affine(h, cfg.eps, vb.device(), vb.dtype())?,
            qk_norm: QkNorm::new(head_dim, cfg.eps, attn.clone())?,
            // qkv (3*h) plus mlp-in (mlp), fused.
            to_qkv_mlp_proj: linear(h, 3 * h + mlp, attn.pp("to_qkv_mlp_proj"))?,
            // attn out (h) plus gated mlp (mlp) -> h.
            to_out: linear(h + mlp, h, attn.pp("to_out"))?,
            hidden_size: h,
            mlp_hidden: mlp,
            num_heads: cfg.num_heads,
        })
    }

    fn forward(&self, xs: &Tensor, temb: &Tensor, rope: &Rope) -> Result<Tensor, FluxError> {
        let mods = self.modulation.forward(temb)?;
        let m = &mods[0];
        let x_mod = m.scale_shift(&xs.apply(&self.norm)?)?;
        let fused = x_mod.apply(&self.to_qkv_mlp_proj)?;
        let qkv = fused.narrow(D::Minus1, 0, 3 * self.hidden_size)?;
        let mlp = fused.narrow(D::Minus1, 3 * self.hidden_size, self.mlp_hidden)?;

        let (q, k, v) = split_heads_qkv(&qkv, self.num_heads)?;
        let q = q.apply(&self.qk_norm.norm_q)?;
        let k = k.apply(&self.qk_norm.norm_k)?;
        let q = apply_rope(&q, rope)?;
        let k = apply_rope(&k, rope)?;
        let attn = attention(&q, &k, &v)?;
        let attn = merge_heads(&attn)?;

        // attn || gelu(mlp) -> to_out, gated, residual.
        let combined = Tensor::cat(&[&attn, &mlp.gelu()?], D::Minus1)?;
        let output = combined.apply(&self.to_out)?;
        Ok((xs + m.gate(&output)?)?)
    }
}

/// `norm_out` + `proj_out`: `AdaLayerNormContinuous` (modulated affine-free
/// `LayerNorm` driven by `temb`) then a `Linear(inner -> out_channels)`. Same shape
/// as candle's `LastLayer`; reimplemented for the diffusers `norm_out`/`proj_out`
/// keys and FLUX.2 dims.
#[derive(Debug, Clone)]
struct LastLayer {
    norm: LayerNorm,
    ada_ln: Linear,
    proj_out: Linear,
}

impl LastLayer {
    #[allow(clippy::needless_pass_by_value)]
    fn new(cfg: &Flux2Config, vb: VarBuilder) -> Result<Self, FluxError> {
        let h = cfg.hidden_size;
        let norm = layer_norm_no_affine(h, cfg.eps, vb.device(), vb.dtype())?;
        // AdaLayerNormContinuous projects temb -> (scale, shift).
        let ada_ln = linear(h, 2 * h, vb.pp("norm_out.linear"))?;
        let proj_out = linear(h, cfg.out_channels, vb.pp("proj_out"))?;
        Ok(Self { norm, ada_ln, proj_out })
    }

    fn forward(&self, xs: &Tensor, temb: &Tensor) -> Result<Tensor, FluxError> {
        let chunks = temb.silu()?.apply(&self.ada_ln)?.chunk(2, D::Minus1)?;
        let (shift, scale) = (&chunks[0], &chunks[1]);
        let xs = xs
            .apply(&self.norm)?
            .broadcast_mul(&(scale.unsqueeze(1)? + 1.0)?)?
            .broadcast_add(&shift.unsqueeze(1)?)?;
        Ok(xs.apply(&self.proj_out)?)
    }
}

// Slice the shared rope tables into the text span and the image span. The forward
// concatenates ids text-first, so the table rows are [txt(0..txt_len),
// img(txt_len..)]; the double block needs them split to rotate each stream before
// the joint cat.
fn split_rope(rope: &Rope, txt_len: usize, img_len: usize) -> Result<(Rope, Rope), FluxError> {
    let cos_txt = rope.cos.narrow(2, 0, txt_len)?;
    let sin_txt = rope.sin.narrow(2, 0, txt_len)?;
    let cos_img = rope.cos.narrow(2, txt_len, img_len)?;
    let sin_img = rope.sin.narrow(2, txt_len, img_len)?;
    Ok((Rope { cos: cos_txt, sin: sin_txt }, Rope { cos: cos_img, sin: sin_img }))
}

/// The FLUX.2 transformer: embedders, double/single blocks, and the output head.
/// Runs one denoise step per [`FluxTransformer::forward`] call.
pub struct FluxTransformer {
    cfg: Flux2Config,
    /// `x_embedder` — packed latent (128) -> inner (3072).
    img_in: Linear,
    /// `context_embedder` — Qwen3 conditioning (7680) -> inner, no bias.
    txt_in: Linear,
    /// Timestep MLP — sinusoidal (256) -> inner -> inner.
    time_in_1: Linear,
    time_in_2: Linear,
    pe_embedder: Flux2PosEmbed,
    double_blocks: Vec<DoubleStreamBlock>,
    single_blocks: Vec<SingleStreamBlock>,
    last_layer: LastLayer,
}

impl FluxTransformer {
    /// Build the transformer from a [`VarBuilder`] over the `transformer/` weights.
    ///
    /// # Errors
    ///
    /// Returns [`FluxError`] if a weight key is missing or a layer fails to build.
    #[allow(clippy::needless_pass_by_value)]
    pub fn new(cfg: &Flux2Config, vb: VarBuilder) -> Result<Self, FluxError> {
        let h = cfg.hidden_size;
        // x_embedder: packed latent -> inner. diffusers names it `x_embedder`.
        let img_in = linear(cfg.in_channels, h, vb.pp("x_embedder"))?;
        // context_embedder: Qwen3 7680 -> inner, NO bias (per the reference).
        let txt_in = linear_b(cfg.context_in_dim, h, false, vb.pp("context_embedder"))?;
        // time_guidance_embed: Timesteps(256) -> TimestepEmbedding(256 -> inner).
        // klein has no guidance embedder; only the timestep path exists.
        let time_vb = vb.pp("time_guidance_embed").pp("timestep_embedder");
        let time_in_1 = linear(cfg.timestep_channels, h, time_vb.pp("linear_1"))?;
        let time_in_2 = linear(h, h, time_vb.pp("linear_2"))?;

        let mut double_blocks = Vec::with_capacity(cfg.depth);
        let vb_d = vb.pp("transformer_blocks");
        for i in 0..cfg.depth {
            double_blocks.push(DoubleStreamBlock::new(cfg, vb_d.pp(i))?);
        }
        let mut single_blocks = Vec::with_capacity(cfg.depth_single_blocks);
        let vb_s = vb.pp("single_transformer_blocks");
        for i in 0..cfg.depth_single_blocks {
            single_blocks.push(SingleStreamBlock::new(cfg, vb_s.pp(i))?);
        }
        let last_layer = LastLayer::new(cfg, vb.clone())?;
        let pe_embedder = Flux2PosEmbed::new(cfg.theta, cfg.axes_dim.clone());

        Ok(Self {
            cfg: cfg.clone(),
            img_in,
            txt_in,
            time_in_1,
            time_in_2,
            pe_embedder,
            double_blocks,
            single_blocks,
            last_layer,
        })
    }

    /// Load the transformer from the store's `transformer/` component.
    ///
    /// The klein checkpoint is a single safetensors file
    /// (`transformer/diffusion_pytorch_model.safetensors`); `dtype` is the working
    /// dtype (bf16 on GPU, f32 on CPU), so an `F8E4M3` checkpoint upcasts on load.
    /// The config is the fixed klein topology — the on-disk `transformer/config.json`
    /// records the same numbers, but the klein values are pinned here so a config
    /// drift cannot silently change the architecture.
    ///
    /// # Errors
    ///
    /// Returns [`FluxError`] if the weight file is missing or a key is absent.
    pub fn load(store: &ModelStore, device: &Device, dtype: DType) -> Result<Self, FluxError> {
        let weights = store.file_path("transformer/diffusion_pytorch_model.safetensors");
        Self::from_file(&Flux2Config::klein(), &weights, device, dtype)
    }

    /// Build the transformer from an explicit weight file. Split out so a test can
    /// point at a fixture checkpoint.
    ///
    /// # Errors
    ///
    /// Returns [`FluxError`] on a missing/malformed file or weight key.
    pub fn from_file(cfg: &Flux2Config, weights: &Path, device: &Device, dtype: DType) -> Result<Self, FluxError> {
        let vb = var_builder(&[weights], dtype, device)?;
        Self::new(cfg, vb)
    }

    /// Run one denoise step. The FLUX.2 conditioning rewrite, no CLIP pooled
    /// vector: `txt` is the Qwen3 `(b, txt_len, 7680)` hidden-state conditioning.
    ///
    /// - `img`: packed latent `(b, img_len, 128)`.
    /// - `img_ids`: position ids `(img_len, 4)` over axes (t, h, w, l).
    /// - `txt`: Qwen3 conditioning `(b, txt_len, context_in_dim)`.
    /// - `txt_ids`: position ids `(txt_len, 4)`; text spans the `l` axis.
    /// - `timesteps`: `(b,)` the current flow-matching time.
    ///
    /// Distilled: there is no guidance argument — klein is guidance-distilled and
    /// the sampler runs CFG-off. Position ids are concatenated text-first, the
    /// same order the double-stream joint attention assumes.
    ///
    /// # Errors
    ///
    /// Returns [`FluxError`] on a shape mismatch or a tensor op failure.
    pub fn forward(&self, img: &Tensor, img_ids: &Tensor, txt: &Tensor, txt_ids: &Tensor, timesteps: &Tensor) -> Result<Tensor, FluxError> {
        if img.rank() != 3 {
            return Err(FluxError::Device(format!("img must be rank 3, got {:?}", img.shape())));
        }
        if txt.rank() != 3 {
            return Err(FluxError::Device(format!("txt must be rank 3, got {:?}", txt.shape())));
        }
        let dtype = img.dtype();
        // Shared rotary tables over the text-first concatenated id space.
        let ids = Tensor::cat(&[txt_ids, img_ids], 0)?;
        let rope = Rope::build(&self.pe_embedder, &ids, dtype)?;

        let mut img = img.apply(&self.img_in)?;
        let mut txt = txt.apply(&self.txt_in)?;
        // Timestep conditioning (no guidance, no pooled vector).
        let temb = timestep_embedding(timesteps, self.cfg.timestep_channels, dtype)?;
        let temb = temb.apply(&self.time_in_1)?.silu()?.apply(&self.time_in_2)?;

        for block in &self.double_blocks {
            (img, txt) = block.forward(&img, &txt, &temb, &rope)?;
        }

        // Single-stream blocks operate on the concatenated sequence, text-first.
        let txt_len = txt.dim(1)?;
        let mut joined = Tensor::cat(&[&txt, &img], 1)?;
        for block in &self.single_blocks {
            joined = block.forward(&joined, &temb, &rope)?;
        }
        // Drop the text tokens; keep the image span for the output head.
        let img_out = joined.i((.., txt_len..))?;
        self.last_layer.forward(&img_out, &temb)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use candle_core::Device;

    #[test]
    fn klein_config_numbers() {
        let cfg = Flux2Config::klein();
        assert_eq!(cfg.in_channels, 128);
        assert_eq!(cfg.out_channels, 128);
        assert_eq!(cfg.context_in_dim, 7680);
        assert_eq!(cfg.hidden_size, 3072);
        assert_eq!(cfg.num_heads, 24);
        assert_eq!(cfg.head_dim(), 128);
        assert_eq!(cfg.depth, 5);
        assert_eq!(cfg.depth_single_blocks, 20);
        assert_eq!(cfg.axes_dim, vec![32, 32, 32, 32]);
        assert_eq!(cfg.theta, 2000);
        assert_eq!(cfg.mlp_hidden(), 9216);
        // The 4 axes sum to head_dim.
        assert_eq!(cfg.axes_dim.iter().sum::<usize>(), cfg.head_dim());
    }

    #[test]
    fn rope_tables_match_head_dim() -> Result<(), FluxError> {
        // 4-axis ids -> (cos, sin) each (seq, head_dim).
        let device = Device::Cpu;
        let seq = 6usize;
        let ids: Vec<u32> = (0..seq).flat_map(|i| [0u32, i as u32, 0, 0]).collect();
        let ids = Tensor::from_vec(ids, (seq, 4), &device)?.to_dtype(DType::F32)?;
        let pe = Flux2PosEmbed::new(2000, vec![32, 32, 32, 32]);
        let (cos, sin) = pe.tables(&ids)?;
        assert_eq!(cos.dims2()?, (seq, 128));
        assert_eq!(sin.dims2()?, (seq, 128));
        Ok(())
    }

    #[test]
    fn apply_rope_preserves_shape() -> Result<(), FluxError> {
        // Rotary is shape-preserving on (b, heads, seq, head_dim).
        let device = Device::Cpu;
        let (b, heads, seq, head_dim) = (1usize, 2usize, 4usize, 8usize);
        let x = Tensor::randn(0f32, 1f32, (b, heads, seq, head_dim), &device)?;
        let cos = Tensor::ones((1, 1, seq, head_dim), DType::F32, &device)?;
        let sin = Tensor::zeros((1, 1, seq, head_dim), DType::F32, &device)?;
        let rope = Rope { cos, sin };
        // cos=1, sin=0 is the identity rotation: output equals input.
        let y = apply_rope(&x, &rope)?;
        assert_eq!(y.dims4()?, (b, heads, seq, head_dim));
        let diff = (&x - &y)?.abs()?.max_all()?.to_scalar::<f32>()?;
        assert!(diff < 1e-6, "identity rotary drifted: {diff}");
        Ok(())
    }
}
