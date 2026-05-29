//! The Qwen3 text encoder.
//!
//! Imports `candle_transformers::models::qwen3::Model` (the trunk that returns
//! post-final-norm hidden states, not `ModelForCausalLM` with its LM head) plus
//! the `tokenizers` crate for the prompt. FLUX.2 conditions on Qwen3 hidden
//! states only — no CLIP pooled vector — so this produces the `(1, seq, hidden)`
//! token tensor the `DiT` consumes.
//!
//! This file is a typed placeholder; the conditioning wiring lands in gate 2.

/// The Qwen3 encoder plus its tokenizer; turns a prompt into conditioning tokens.
pub struct TextEncoder {
    // Fields land with the Qwen3 conditioning port (gate 2).
}

impl TextEncoder {
    /// A zero-field placeholder so [`crate::loader::LoadedModel`] can hold a
    /// text-encoder slot before the Qwen3 conditioning lands. Removed once `new`
    /// exists.
    #[must_use]
    pub(crate) fn placeholder() -> Self {
        Self {}
    }
}
