//! Pixhaus AI runtime: verb dispatch, backend adapters, built-in verbs.
//!
//! The verb plugin protocol is bedrock B5 — the highest-leverage spec
//! in the project. Backends (Anthropic, `OpenAI`, Replicate, Ollama,
//! `ComfyUI`, Stability) implement a single trait so verbs and
//! backends compose freely. Verb runtime, backend adapters, and the
//! 14 built-in verbs land in their own streams (S21–S36); this crate
//! ships the protocol they all build against.
//!
//! # Backend adapters (S22)
//!
//! See [`backends`] for the [`backends::InferenceBackend`] trait,
//! [`backends::BackendRegistry`], and the six provider adapters.
//!
//! # Quick start
//!
//! ```no_run
//! use pixhaus_ai::plugin::{EchoVerb, VerbContext, VerbInputs, VerbRuntime, ECHO_VERB_ID, VerbId};
//! use pixhaus_core::project::{ProjectMetadata, SpriteId, FrameIndex};
//!
//! # async fn doc() -> anyhow::Result<()> {
//! let runtime = VerbRuntime::new();
//! runtime.register(EchoVerb::new())?;
//!
//! let mut ctx = VerbContext::empty(ProjectMetadata {
//!     name: "demo".into(),
//!     description: None,
//!     author: None,
//!     created_at: 0,
//!     updated_at: 0,
//!     editor_version: env!("CARGO_PKG_VERSION").into(),
//! });
//! ctx.active_sprite = Some(SpriteId::new(1));
//! ctx.active_frame = Some(FrameIndex::new(0));
//!
//! let inputs = VerbInputs::from_struct(&pixhaus_ai::plugin::EchoInputs {
//!     pixels: pixhaus_ai::plugin::PixelData::rgba8(1, 1, vec![255, 0, 0, 255]),
//!     layer_name: None,
//! })?;
//!
//! let invocation = runtime.invoke(&VerbId::new(ECHO_VERB_ID), ctx, inputs)?;
//! let preview = invocation.finish().await?;
//! let commit = runtime.commit(preview);
//! assert_eq!(commit.effects.len(), 1);
//! # Ok(())
//! # }
//! ```

#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::missing_panics_doc,
        clippy::disallowed_methods,
        clippy::float_cmp,
        clippy::print_stderr,
        clippy::print_stdout,
    )
)]

pub mod backends;
pub mod plugin;
pub mod verbs;

/// Returns the crate name. Stable identifier downstream consumers can
/// use as a sanity check after a workspace-wide upgrade.
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
