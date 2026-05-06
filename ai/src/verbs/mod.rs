//! Built-in AI verbs (S23–S36).
//!
//! Each verb lives in its own submodule and is registered with the
//! [`crate::plugin::runtime::VerbRuntime`] at startup. The modules are
//! public so the app crate can instantiate verbs with whatever
//! [`crate::backends::BackendRegistry`] the user has configured.
//!
//! # Verb ID namespace
//!
//! All built-in verbs use the prefix `pixhaus.builtin.`. Third-party
//! plugins use their own reverse-DNS namespace; the runtime does not
//! enforce namespacing but the convention prevents collisions.
//!
//! # Shared helpers
//!
//! `call_text_vlm` is the canonical entry point for any verb that needs a
//! vision-language call. It downcasts the backend attached to the
//! [`crate::plugin::context::VerbContext`] to a known concrete adapter and
//! delegates to that adapter's operational `invoke` method.

pub mod auto_mesh_deformation;
pub mod cleanup;
pub mod critique;
pub mod inbetween;
pub mod motion_from_video;
pub mod project_style_learning;
pub mod sketch_finishing;
pub mod tile;
pub mod tileset_from_description;
pub mod variant;

pub use cleanup::CleanupVerb;
pub use critique::CritiqueVerb;

use tokio_util::sync::CancellationToken;

use crate::backends::{
    InferenceBackend as OpsBackend, InferenceRequest, InferenceResponse, TextGenRequest,
    TextGenResponse, anthropic::AnthropicBackend, openai::OpenAiBackend,
};
use crate::plugin::backend::InferenceBackend as PluginBackend;
use crate::plugin::error::{Result, VerbError};
use crate::plugin::progress::VerbProgress;

/// Sends a text-generation (optionally vision-language) request through
/// whichever concrete backend is attached to the verb context.
///
/// The `plugin::backend::InferenceBackend` trait is intentionally thin —
/// verbs that need to make inference calls downcast to a concrete adapter.
/// This helper consolidates the downcast logic so individual verbs don't
/// repeat it. It tries each known VLM-capable adapter in declaration order
/// and returns [`VerbError::Backend`] if none match.
///
/// # Supported backends
///
/// - [`AnthropicBackend`] — Claude Sonnet 4.6 (default model)
/// - [`OpenAiBackend`] — GPT-4o (default model)
///
/// Additional adapters are added here as they land; verbs do not need to
/// change when a new adapter is added.
pub(crate) async fn call_text_vlm(
    backend: &dyn PluginBackend,
    request: TextGenRequest,
    progress: VerbProgress,
    cancel: CancellationToken,
) -> Result<TextGenResponse> {
    let req = InferenceRequest::Text(request);

    let resp = if let Some(b) = backend.as_any().downcast_ref::<AnthropicBackend>() {
        b.invoke(req, progress, cancel)
            .await
            .map_err(|e| VerbError::Backend(e.to_string()))?
    } else if let Some(b) = backend.as_any().downcast_ref::<OpenAiBackend>() {
        b.invoke(req, progress, cancel)
            .await
            .map_err(|e| VerbError::Backend(e.to_string()))?
    } else {
        return Err(VerbError::Backend(
            "no supported VLM adapter attached to context; \
             register an Anthropic or OpenAI backend with the VerbRuntime"
                .into(),
        ));
    };

    match resp {
        InferenceResponse::Text(t) => Ok(t),
        _ => Err(VerbError::Backend(
            "backend returned a non-text response to a text request".into(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::verb::Verb;

    #[test]
    fn critique_verb_is_exported() {
        let verb = CritiqueVerb::new();
        assert_eq!(verb.descriptor().id.as_str(), critique::CRITIQUE_VERB_ID);
    }
}
