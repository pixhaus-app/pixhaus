//! Built-in AI verbs.
//!
//! This vertical-slice port ships the reference-sheet generator plus an
//! auto-tag stub. Each lives in its own submodule and is public so the
//! app/shell crate can instantiate and register it with the
//! [`crate::plugin::runtime::VerbRuntime`]. The auto-tag verb is backend-gated
//! on a vision-language backend that v2 does not ship yet; its `invoke` reports
//! the backend as unavailable rather than fabricating a result.
//!
//! # Shared helper
//!
//! `ctx_fat_backend` recovers the fat operational backend
//! ([`crate::backends::InferenceBackend`]) from the thin
//! [`crate::plugin::backend::InferenceBackend`] the runtime stores on
//! [`crate::plugin::context::VerbContext::backend`]. The full crate downcasts
//! a chain of concrete adapters here; this slice registers FAL through a
//! [`crate::backends::bridge::BackendProxy`], so the helper only needs the
//! `BackendProxy` branch.

pub mod auto_tag;
pub mod reference_sheet;

pub use auto_tag::AutoTagVerb;
pub use reference_sheet::GenerateReferenceSheetVerb;

use crate::backends::{InferenceBackend as OpsBackend, bridge::BackendProxy};
use crate::plugin::context::VerbContext;
use crate::plugin::error::{Result, VerbError};

/// Returns the fat operational backend attached to `ctx`, downcasting
/// from the thin trait the runtime stores.
///
/// The runtime selects a backend matching the verb's `required_capabilities`
/// and stores it in `ctx.backend` as a
/// `Arc<dyn plugin::backend::InferenceBackend>`. Verbs that need to make
/// actual inference calls reach the operational
/// [`crate::backends::InferenceBackend`] trait through this helper.
///
/// In this slice the only registration path is
/// [`BackendProxy::new`], so the helper recognises only `BackendProxy`. The
/// full crate also recognised the Anthropic/OpenAI/Replicate/Stability
/// concrete adapters directly; those branches are dropped here along with
/// their adapters.
///
/// # Errors
///
/// - [`VerbError::Backend`] if `ctx.backend` is `None` (the runtime skips
///   selection for verbs whose `required_capabilities` are empty) or holds a
///   backend that was not wrapped in [`BackendProxy`].
pub(crate) fn ctx_fat_backend(ctx: &VerbContext) -> Result<&dyn OpsBackend> {
    let plug = ctx
        .backend
        .as_ref()
        .ok_or_else(|| VerbError::Backend("no backend attached to verb context".into()))?;
    if let Some(p) = plug.as_any().downcast_ref::<BackendProxy>() {
        Ok(p.fat())
    } else {
        Err(VerbError::Backend(
            "ctx.backend type not recognised; register a known adapter or wrap in BackendProxy".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::verb::Verb;

    #[test]
    fn generate_reference_sheet_verb_is_exported() {
        let verb = GenerateReferenceSheetVerb::new();
        assert_eq!(verb.descriptor().id.as_str(), reference_sheet::GENERATE_REFERENCE_SHEET_VERB_ID);
    }

    #[test]
    fn ctx_fat_backend_errors_without_backend() {
        use pixhaus_core::project::ProjectMetadata;

        let ctx = VerbContext::empty(ProjectMetadata {
            name: "no-backend".into(),
            description: None,
            author: None,
            created_at: 0,
            updated_at: 0,
            editor_version: "0".into(),
        });
        assert!(matches!(ctx_fat_backend(&ctx), Err(VerbError::Backend(_))));
    }
}
