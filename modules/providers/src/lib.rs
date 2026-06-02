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
