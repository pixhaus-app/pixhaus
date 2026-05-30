//! The Qwen3 text encoder for FLUX.2 klein conditioning.
//!
//! FLUX.2 klein conditions on Qwen3 hidden states only — no CLIP pooled vector,
//! no projection, no norm. The klein pipeline does NOT consume the last hidden
//! layer: it stacks three intermediate layers (9, 18, 27 of 36) and
//! channel-concatenates them to `3 * 2560 = 7680`, which is exactly the
//! transformer's `joint_attention_dim`. The transformer's own input `Linear`
//! does the dimension match; nothing is applied here between extraction and the
//! `DiT`.
//!
//! ## Why we reimplement the trunk instead of importing `qwen3::Model`
//!
//! `candle_transformers::models::qwen3::Model::forward` returns only the
//! post-final-norm last layer, and its `layers`/`embed_tokens`/`norm` fields plus
//! the `DecoderLayer` type are private — there is no seam to capture an
//! intermediate layer. We need three intermediate residual-stream states, so we
//! rebuild the (short) Qwen3 decoder topology here on candle-nn primitives
//! (`RmsNorm`, `linear_b`, `embedding`, `rotary_emb::rope`,
//! `utils::repeat_kv`, `ops::softmax_last_dim`), matching the candle qwen3
//! weight-key layout (`model.embed_tokens`, `model.layers.{i}.{self_attn,mlp,...}`)
//! key-for-key so the same checkpoint loads. The only behavioral change is that
//! `forward` records every per-layer output rather than discarding all but the
//! last. The final `model.norm` is irrelevant to us — we never read the last
//! layer — so it is not loaded.
//!
//! A text encode is a single full-sequence prefill: one forward pass at offset 0
//! with a causal mask over the whole prompt, no KV cache (we never extend the
//! sequence), so the trunk holds no mutable decode state.

use std::path::Path;
use std::sync::Arc;

use candle_core::{DType, Device, Module, Tensor};
use candle_nn::{Activation, Embedding, Linear, RmsNorm, VarBuilder, embedding, linear_b, ops::softmax_last_dim, rms_norm, rotary_emb};
use candle_transformers::models::qwen3::Config as Qwen3Config;
use candle_transformers::utils::repeat_kv;
use tokenizers::Tokenizer;

use crate::FluxError;
use crate::loader::var_builder;
use crate::store::ModelStore;

/// The three Qwen3 layers the klein pipeline stacks (`text_encoder_out_layers`).
///
/// `output_hidden_states` is 0-indexed including the embedding output at index 0,
/// so `hidden_states[k]` is the residual stream after `k` decoder layers have
/// run. Layers 9, 18, 27 are intermediate (the trunk has 36), so none is the
/// last layer and the final norm never enters the conditioning. These are the
/// klein defaults; the base FLUX.2-dev Mistral path uses (10, 20, 30) instead —
/// do not copy those here.
pub const KLEIN_OUT_LAYERS: [usize; 3] = [9, 18, 27];

/// One Qwen3 decoder layer: pre-norm self-attention then pre-norm `SwiGLU` MLP,
/// each with a residual add. Mirrors candle's `qwen3::DecoderLayer` key-for-key
/// (`self_attn`, `mlp`, `input_layernorm`, `post_attention_layernorm`).
#[derive(Debug)]
struct DecoderLayer {
    input_layernorm: RmsNorm,
    post_attention_layernorm: RmsNorm,
    q_proj: Linear,
    k_proj: Linear,
    v_proj: Linear,
    o_proj: Linear,
    q_norm: RmsNorm,
    k_norm: RmsNorm,
    gate_proj: Linear,
    up_proj: Linear,
    down_proj: Linear,
    act_fn: Activation,
    num_heads: usize,
    num_kv_heads: usize,
    num_kv_groups: usize,
    head_dim: usize,
    rope: Arc<RotaryEmbedding>,
}

impl DecoderLayer {
    // `vb` is the candle by-value `VarBuilder` convention (every candle `::new`
    // takes it this way); `.pp()` borrows, so clippy flags it as not-consumed.
    #[allow(clippy::needless_pass_by_value)]
    fn new(cfg: &Qwen3Config, rope: Arc<RotaryEmbedding>, vb: VarBuilder) -> Result<Self, FluxError> {
        let head_dim = cfg.head_dim;
        let num_heads = cfg.num_attention_heads;
        let num_kv_heads = cfg.num_key_value_heads;
        let bias = cfg.attention_bias;
        let attn = vb.pp("self_attn");
        let q_proj = linear_b(cfg.hidden_size, num_heads * head_dim, bias, attn.pp("q_proj"))?;
        let k_proj = linear_b(cfg.hidden_size, num_kv_heads * head_dim, bias, attn.pp("k_proj"))?;
        let v_proj = linear_b(cfg.hidden_size, num_kv_heads * head_dim, bias, attn.pp("v_proj"))?;
        let o_proj = linear_b(num_heads * head_dim, cfg.hidden_size, bias, attn.pp("o_proj"))?;
        // Qwen3 applies a per-head RMSNorm to q and k over the head_dim.
        let q_norm = rms_norm(head_dim, cfg.rms_norm_eps, attn.pp("q_norm"))?;
        let k_norm = rms_norm(head_dim, cfg.rms_norm_eps, attn.pp("k_norm"))?;

        let mlp = vb.pp("mlp");
        let gate_proj = linear_b(cfg.hidden_size, cfg.intermediate_size, false, mlp.pp("gate_proj"))?;
        let up_proj = linear_b(cfg.hidden_size, cfg.intermediate_size, false, mlp.pp("up_proj"))?;
        let down_proj = linear_b(cfg.intermediate_size, cfg.hidden_size, false, mlp.pp("down_proj"))?;

        let input_layernorm = rms_norm(cfg.hidden_size, cfg.rms_norm_eps, vb.pp("input_layernorm"))?;
        let post_attention_layernorm = rms_norm(cfg.hidden_size, cfg.rms_norm_eps, vb.pp("post_attention_layernorm"))?;

        Ok(Self {
            input_layernorm,
            post_attention_layernorm,
            q_proj,
            k_proj,
            v_proj,
            o_proj,
            q_norm,
            k_norm,
            gate_proj,
            up_proj,
            down_proj,
            act_fn: cfg.hidden_act,
            num_heads,
            num_kv_heads,
            num_kv_groups: num_heads / num_kv_heads,
            head_dim,
            rope,
        })
    }

    /// Self-attention over the full prompt with a causal mask. No KV cache: a
    /// text encode is one prefill, never extended, so there is nothing to cache.
    // head_dim (128) is exact in f64; the scale cast is lossless. The q/k/v/b/l
    // single-char names are the candle tensor-math convention.
    #[allow(clippy::cast_precision_loss, clippy::many_single_char_names)]
    fn attention(&self, x: &Tensor, mask: &Tensor) -> Result<Tensor, FluxError> {
        let (b, l, _) = x.dims3()?;
        let q = self.q_proj.forward(x)?;
        let k = self.k_proj.forward(x)?;
        let v = self.v_proj.forward(x)?;

        // (b, l, h, d) -> (b, h, l, d).
        let q = q.reshape((b, l, self.num_heads, self.head_dim))?.transpose(1, 2)?;
        let k = k.reshape((b, l, self.num_kv_heads, self.head_dim))?.transpose(1, 2)?;
        let v = v.reshape((b, l, self.num_kv_heads, self.head_dim))?.transpose(1, 2)?;

        // Per-head RMSNorm on q and k over the head_dim, flattened to (b*h*l, d).
        let q_flat = q.flatten(0, 2)?;
        let k_flat = k.flatten(0, 2)?;
        let q_flat = self.q_norm.forward(&q_flat)?;
        let k_flat = self.k_norm.forward(&k_flat)?;
        let q = q_flat.reshape((b, self.num_heads, l, self.head_dim))?;
        let k = k_flat.reshape((b, self.num_kv_heads, l, self.head_dim))?;

        // RoPE at offset 0 (single prefill).
        let (q, k) = self.rope.apply(&q, &k)?;

        // GQA: repeat the KV heads up to the query-head count.
        let k = repeat_kv(k, self.num_kv_groups)?.contiguous()?;
        let v = repeat_kv(v, self.num_kv_groups)?.contiguous()?;

        let scale = 1.0 / (self.head_dim as f64).sqrt();
        let scores = (q.contiguous()?.matmul(&k.transpose(2, 3)?)? * scale)?;
        let scores = scores.broadcast_add(mask)?;
        let probs = softmax_last_dim(&scores)?;
        let ctx = probs.matmul(&v)?;
        // (b, h, l, d) -> (b, l, h*d) -> o_proj.
        let ctx = ctx.transpose(1, 2)?.reshape((b, l, self.num_heads * self.head_dim))?;
        Ok(self.o_proj.forward(&ctx)?)
    }

    fn mlp(&self, x: &Tensor) -> Result<Tensor, FluxError> {
        let lhs = x.apply(&self.gate_proj)?.apply(&self.act_fn)?;
        let rhs = x.apply(&self.up_proj)?;
        Ok((lhs * rhs)?.apply(&self.down_proj)?)
    }

    fn forward(&self, x: &Tensor, mask: &Tensor) -> Result<Tensor, FluxError> {
        let h = self.input_layernorm.forward(x)?;
        let h = self.attention(&h, mask)?;
        let x = (x + h)?;
        let h2 = self.post_attention_layernorm.forward(&x)?;
        let h2 = self.mlp(&h2)?;
        Ok((x + h2)?)
    }
}

/// Precomputed `RoPE` sin/cos tables, identical to candle's `Qwen3RotaryEmbedding`.
#[derive(Debug)]
struct RotaryEmbedding {
    sin: Tensor,
    cos: Tensor,
}

impl RotaryEmbedding {
    // The index/theta casts mirror candle's `Qwen3RotaryEmbedding::new` verbatim:
    // head_dim and the sequence length are small (128 / 40960), far inside the
    // exact-integer range of f64/f32, so the casts are lossless in practice.
    #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
    fn new(dtype: DType, cfg: &Qwen3Config, device: &Device) -> Result<Self, FluxError> {
        let dim = cfg.head_dim;
        let max_seq_len = cfg.max_position_embeddings;
        let inv_freq: Vec<f32> = (0..dim).step_by(2).map(|i| 1f32 / cfg.rope_theta.powf(i as f64 / dim as f64) as f32).collect();
        let inv_freq_len = inv_freq.len();
        let inv_freq = Tensor::from_vec(inv_freq, (1, inv_freq_len), device)?.to_dtype(DType::F32)?;
        let t = Tensor::arange(0u32, max_seq_len as u32, device)?
            .to_dtype(DType::F32)?
            .reshape((max_seq_len, 1))?;
        let freqs = t.matmul(&inv_freq)?;
        Ok(Self {
            sin: freqs.sin()?.to_dtype(dtype)?,
            cos: freqs.cos()?.to_dtype(dtype)?,
        })
    }

    /// Apply `RoPE` to q and k (shape `b, h, l, d`) at sequence offset 0.
    fn apply(&self, q: &Tensor, k: &Tensor) -> Result<(Tensor, Tensor), FluxError> {
        let (_, _, seq_len, _) = q.dims4()?;
        let cos = self.cos.narrow(0, 0, seq_len)?;
        let sin = self.sin.narrow(0, 0, seq_len)?;
        let q = rotary_emb::rope(&q.contiguous()?, &cos, &sin)?;
        let k = rotary_emb::rope(&k.contiguous()?, &cos, &sin)?;
        Ok((q, k))
    }
}

/// The Qwen3 decoder trunk, rebuilt to expose every per-layer hidden state.
#[derive(Debug)]
struct Qwen3Trunk {
    embed_tokens: Embedding,
    layers: Vec<DecoderLayer>,
    device: Device,
    dtype: DType,
}

impl Qwen3Trunk {
    // by-value `VarBuilder` per candle convention; `.pp()` borrows it.
    #[allow(clippy::needless_pass_by_value)]
    fn new(cfg: &Qwen3Config, vb: VarBuilder) -> Result<Self, FluxError> {
        let embed_tokens = embedding(cfg.vocab_size, cfg.hidden_size, vb.pp("model.embed_tokens"))?;
        let rope = Arc::new(RotaryEmbedding::new(vb.dtype(), cfg, vb.device())?);
        let vb_layers = vb.pp("model.layers");
        let mut layers = Vec::with_capacity(cfg.num_hidden_layers);
        for i in 0..cfg.num_hidden_layers {
            layers.push(DecoderLayer::new(cfg, rope.clone(), vb_layers.pp(i))?);
        }
        Ok(Self {
            embed_tokens,
            layers,
            device: vb.device().clone(),
            dtype: vb.dtype(),
        })
    }

    /// Run the prompt through the trunk and return every per-layer hidden state.
    ///
    /// The returned vector mirrors `HuggingFace` `output_hidden_states`: index 0 is
    /// the embedding output, index `i + 1` is the output of decoder layer `i`. So
    /// it has `num_hidden_layers + 1` entries, each `(b, seq, hidden_size)`. No
    /// final norm is applied — the intermediate layers we consume are raw
    /// residual-stream states.
    fn hidden_states(&self, input_ids: &Tensor) -> Result<Vec<Tensor>, FluxError> {
        let (b, l) = input_ids.dims2()?;
        let mut h = self.embed_tokens.forward(input_ids)?;
        let mask = self.causal_mask(b, l)?;
        let mut out = Vec::with_capacity(self.layers.len() + 1);
        out.push(h.clone());
        for layer in &self.layers {
            h = layer.forward(&h, &mask)?;
            out.push(h.clone());
        }
        Ok(out)
    }

    /// Additive causal mask of shape `(b, 1, l, l)`: 0 where attention is allowed,
    /// `-inf` above the diagonal. Broadcasts over the head axis.
    fn causal_mask(&self, b: usize, l: usize) -> Result<Tensor, FluxError> {
        let minf = f32::NEG_INFINITY;
        let mask: Vec<f32> = (0..l).flat_map(|i| (0..l).map(move |j| if j <= i { 0.0 } else { minf })).collect();
        let mask = Tensor::from_slice(&mask, (b, 1, l, l), &self.device)?.to_dtype(self.dtype)?;
        Ok(mask)
    }
}

/// The Qwen3 encoder plus its tokenizer; turns a prompt into the `7680`-wide
/// conditioning the FLUX.2 `DiT` consumes.
pub struct TextEncoder {
    trunk: Qwen3Trunk,
    tokenizer: Tokenizer,
    out_layers: [usize; 3],
}

impl TextEncoder {
    /// Load the Qwen3 encoder and the Qwen2 tokenizer from the store.
    ///
    /// Weights are the two `text_encoder/model-0000{1,2}-of-00002.safetensors`
    /// shards (passed together so the name space concatenates), the config is
    /// `text_encoder/config.json`, and the tokenizer is the single-file
    /// `tokenizer/tokenizer.json`. `dtype` is the working dtype (bf16 on GPU, f32
    /// on CPU).
    ///
    /// # Errors
    ///
    /// Returns [`FluxError`] if a file is missing, the config will not parse, a
    /// weight key is absent, or the tokenizer fails to load.
    pub fn load(store: &ModelStore, device: &Device, dtype: DType) -> Result<Self, FluxError> {
        let config_path = store.file_path("text_encoder/config.json");
        let shards = [
            store.file_path("text_encoder/model-00001-of-00002.safetensors"),
            store.file_path("text_encoder/model-00002-of-00002.safetensors"),
        ];
        let tokenizer_path = store.file_path("tokenizer/tokenizer.json");
        Self::from_files(&config_path, &shards, &tokenizer_path, device, dtype)
    }

    /// Build the encoder from explicit file paths. Split out from [`Self::load`]
    /// so a test can point at a fixture tree.
    ///
    /// # Errors
    ///
    /// Returns [`FluxError`] on a missing/malformed file or weight key.
    pub fn from_files<P: AsRef<Path>>(config_path: &Path, shards: &[P], tokenizer_path: &Path, device: &Device, dtype: DType) -> Result<Self, FluxError> {
        let config_text = std::fs::read_to_string(config_path)?;
        let cfg: Qwen3Config = serde_json::from_str(&config_text).map_err(|e| FluxError::Device(format!("parse text_encoder/config.json: {e}")))?;
        let vb = var_builder(shards, dtype, device)?;
        let trunk = Qwen3Trunk::new(&cfg, vb)?;
        let tokenizer = Tokenizer::from_file(tokenizer_path).map_err(|e| FluxError::Device(format!("load tokenizer: {e}")))?;
        Ok(Self {
            trunk,
            tokenizer,
            out_layers: KLEIN_OUT_LAYERS,
        })
    }

    /// Encode a prompt into the `(1, seq, 7680)` conditioning tensor.
    ///
    /// The prompt is wrapped in the Qwen chat template (`user` turn,
    /// `add_generation_prompt`, thinking disabled), tokenized, run through the
    /// trunk, and the three klein layers are stacked and channel-concatenated. No
    /// projection or norm is applied — the transformer's own input `Linear`
    /// handles the dimension.
    ///
    /// # Errors
    ///
    /// Returns [`FluxError`] if tokenization or a tensor op fails, or a configured
    /// output layer index is out of range for the loaded trunk.
    pub fn encode(&self, prompt: &str) -> Result<Tensor, FluxError> {
        let ids = self.tokenize(prompt)?;
        let len = ids.len();
        let device = &self.trunk.device;
        let input_ids = Tensor::from_vec(ids, (1, len), device)?;
        let hidden = self.trunk.hidden_states(&input_ids)?;
        self.select_and_concat(&hidden)
    }

    /// Apply the Qwen chat template and tokenize to `u32` ids.
    ///
    /// We format the template inline rather than pull in a Jinja engine: the
    /// Qwen3 `user` turn with `add_generation_prompt` and thinking off is a fixed
    /// string. `enable_thinking=False` means the assistant turn opens with an
    /// empty `<think></think>` block, matching the klein pipeline.
    fn tokenize(&self, prompt: &str) -> Result<Vec<u32>, FluxError> {
        let templated = format!("<|im_start|>user\n{prompt}<|im_end|>\n<|im_start|>assistant\n<think>\n\n</think>\n\n");
        let encoding = self
            .tokenizer
            .encode(templated, false)
            .map_err(|e| FluxError::Device(format!("tokenize prompt: {e}")))?;
        Ok(encoding.get_ids().to_vec())
    }

    /// Stack the three klein layers and channel-concatenate to `7680`.
    ///
    /// diffusers does `stack([h[k] for k in layers], dim=1)` then permutes and
    /// reshapes to `(b, seq, n * hidden)`. Concatenating the three `(b, seq,
    /// hidden)` tensors on the last axis is the same value layout, so we
    /// `cat(..., D::Minus1)` directly.
    fn select_and_concat(&self, hidden: &[Tensor]) -> Result<Tensor, FluxError> {
        let mut selected = Vec::with_capacity(self.out_layers.len());
        for &k in &self.out_layers {
            let layer = hidden
                .get(k)
                .ok_or_else(|| FluxError::Device(format!("qwen3 output layer {k} out of range ({} states)", hidden.len())))?;
            selected.push(layer.clone());
        }
        Ok(Tensor::cat(&selected, candle_core::D::Minus1)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn klein_out_layers_are_the_intermediate_triple() {
        // 9/18/27 of a 36-layer trunk — all intermediate, none the last layer.
        assert_eq!(KLEIN_OUT_LAYERS, [9, 18, 27]);
        assert!(KLEIN_OUT_LAYERS.iter().all(|&k| k < 36));
    }

    #[test]
    fn concat_width_is_three_times_hidden() {
        // The conditioning width must equal the transformer's joint_attention_dim.
        let hidden = 2560usize;
        assert_eq!(KLEIN_OUT_LAYERS.len() * hidden, 7680);
    }

    #[test]
    fn select_and_concat_widens_by_three() -> Result<(), FluxError> {
        // Fabricate enough fake hidden states to index 9/18/27, then confirm the
        // concat triples the channel width and keeps the sequence length.
        let device = Device::Cpu;
        let hidden_size = 4usize;
        let seq = 5usize;
        // Values are irrelevant — only the concat shape is under test.
        let states: Vec<Tensor> = (0..30)
            .map(|_| Tensor::full(1.0f32, (1, seq, hidden_size), &device).map_err(FluxError::from))
            .collect::<Result<_, _>>()?;
        // Build a TextEncoder-free selector by replicating the logic with the
        // public constant; the method needs &self, so exercise the concat here.
        let mut picked = Vec::new();
        for &k in &KLEIN_OUT_LAYERS {
            picked.push(states[k].clone());
        }
        let out = Tensor::cat(&picked, candle_core::D::Minus1)?;
        assert_eq!(out.dims3()?, (1, seq, hidden_size * 3));
        Ok(())
    }
}
