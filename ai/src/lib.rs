//! Pixhaus AI runtime: verb dispatch, backend adapters, built-in verbs.
//!
//! The verb plugin protocol is bedrock B5 (the highest-leverage spec in the
//! project). Backends (Anthropic, `OpenAI`, Replicate, Ollama, `ComfyUI`) implement
//! a single trait so verbs and backends compose freely.

#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::missing_panics_doc
    )
)]

/// Returns the crate name. Placeholder until the verb runtime lands (B5/S21).
#[must_use]
pub fn crate_name() -> &'static str {
    "pixhaus-ai"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_name_is_stable() {
        assert_eq!(crate_name(), "pixhaus-ai");
    }
}
