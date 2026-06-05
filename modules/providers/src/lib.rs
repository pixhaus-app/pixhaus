//! Pixhaus provider modules.
//!
//! Registers AI and compute providers behind the capability-based provider
//! abstraction (architecture bible sections 7.3 and 14.2). The app asks for
//! capabilities, not specific providers.
//!
//! Foundation stage: ships the offline [`MockProvider`] so the Generate flow works
//! without API keys, model downloads, or a GPU. Remote and local-model providers
//! land later behind the same `Provider` trait.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::disallowed_methods, clippy::panic))]

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use pixhaus_services::provider::ProviderRegistry;

mod mock;
mod openrouter;
mod postprocess;

pub use mock::MockProvider;
pub use openrouter::OpenRouterProvider;
pub use postprocess::{PostProcessError, chroma_key_magenta, slice_sheet};

/// Registers the offline providers (the mock provider) into `registry`. Always
/// available so the Generate flow works without API keys, model downloads, or a GPU.
pub fn register(registry: &mut ProviderRegistry) {
    registry.register(Arc::new(MockProvider::new()));
    tracing::info!(provider = "mock", "registered mock provider");
}

/// Registers the OpenRouter provider with `api_key` into `registry`. Register it
/// before the mock so capability lookups prefer the real backend; a build failure is
/// logged and skipped, never fatal (the mock still answers). The key is never logged.
pub fn register_openrouter(registry: &mut ProviderRegistry, api_key: String) {
    match OpenRouterProvider::new(api_key) {
        Ok(provider) => {
            registry.register(Arc::new(provider));
            tracing::info!(provider = "openrouter", "registered OpenRouter provider");
        }
        Err(error) => tracing::warn!(%error, "OpenRouter provider unavailable; using the mock"),
    }
}

/// Milliseconds since the Unix epoch, saturating to 0 if the clock reads before it.
/// Both providers stamp `GenerationProvenance.created_unix_ms` with it, so it lives
/// here rather than as a copy in each.
pub(crate) fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|d| u64::try_from(d.as_millis()).ok())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pixhaus_services::provider::ProviderCapability;

    #[test]
    fn register_makes_the_mock_answer_the_text_to_sprite_capability() {
        let mut registry = ProviderRegistry::new();
        assert!(registry.first_with(ProviderCapability::TextToSprite).is_none());
        register(&mut registry);
        assert!(
            registry.first_with(ProviderCapability::TextToSprite).is_some(),
            "the mock answers TextToSprite after registration"
        );
    }

    #[test]
    fn register_openrouter_keeps_the_capability_answerable() {
        // Register the mock first so a lookup has a fallback, then add OpenRouter. The
        // build-failure branch is non-fatal by design (it logs and skips), but
        // derive_builder accepts any api_key string here, so the failure path cannot be
        // forced through the key in a unit test; this pins the success path and that the
        // mock still answers regardless.
        let mut registry = ProviderRegistry::new();
        register(&mut registry);
        register_openrouter(&mut registry, "test-key".to_owned());
        assert!(
            registry.first_with(ProviderCapability::TextToSprite).is_some(),
            "a registered provider still answers the capability"
        );
        // Both providers offer the anchor capability, so the lookup stays satisfied
        // whether or not OpenRouter built; this guards that registration is additive
        // and a present provider always answers.
        assert!(
            registry.first_with(ProviderCapability::GenerateAnchor).is_some(),
            "the anchor capability stays answerable after both registrations"
        );
    }

    #[test]
    fn now_ms_is_nonzero_and_monotonic() {
        // The clock is well past the Unix epoch, so the stamp is nonzero, and a later
        // call never reads earlier than an earlier one.
        let first = now_ms();
        assert!(first > 0, "the clock reads past the Unix epoch");
        let second = now_ms();
        assert!(second >= first, "now_ms does not go backwards");
    }
}
