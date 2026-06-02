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

pub use mock::MockProvider;

/// Registers the offline providers (the mock provider) into `registry`.
pub fn register(registry: &mut ProviderRegistry) {
    registry.register(Arc::new(MockProvider::new()));
    tracing::info!(provider = "mock", "registered mock provider");
}
