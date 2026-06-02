//! The capability-based provider abstraction and its registry.
//!
//! The app asks the [`ProviderRegistry`] for a *capability* ("who can generate a
//! sprite from text?"), never for a named provider (bible 14.2). A [`Provider`]
//! receives immutable input and returns a [`GeneratedResult`] — it never touches the
//! live document, undo stack, or egui context (bible 13.6).
//!
//! `generate` returns a boxed future rather than using `async fn` in the trait, so
//! `dyn Provider` stays object-safe without pulling the `async-trait` crate.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use thiserror::Error;
use tokio_util::sync::CancellationToken;

use crate::generated::GeneratedResult;
use crate::job::GenerationJobInput;

/// What a provider can do. The app queries by capability, not by name.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum ProviderCapability {
    /// Generate a sprite image from a text prompt.
    TextToSprite,
    /// Generate a single neutral character reference (an anchor) on a flat key.
    GenerateAnchor,
    /// Generate an idle-animation sheet, conditioned on an anchor reference image.
    GenerateIdleAnimation,
}

/// A provider's stable id. A `String`, not `&'static str`, because real providers
/// are configuration-driven.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct ProviderId(pub String);

/// A future that resolves to a generated result (still or animation) or an error.
pub type GenerateFuture<'a> = Pin<Box<dyn Future<Output = Result<GeneratedResult, ProviderError>> + Send + 'a>>;

/// A registered AI/compute backend.
pub trait Provider: Send + Sync {
    /// This provider's stable id.
    fn id(&self) -> ProviderId;

    /// i18n key for the provider's display label (resolved by the shell).
    fn label_key(&self) -> &'static str;

    /// The capabilities this provider offers.
    fn capabilities(&self) -> &[ProviderCapability];

    /// Runs one generation request. Receives immutable input and a cancellation
    /// token; returns a generated result or an error. Never receives the document.
    fn generate<'a>(&'a self, input: &'a GenerationJobInput, cancel: CancellationToken) -> GenerateFuture<'a>;
}

/// The provider registry — the bible's provider registry seam (8.7). Long-lived,
/// held by the host; providers register at boot.
#[derive(Default)]
pub struct ProviderRegistry {
    providers: Vec<Arc<dyn Provider>>,
}

impl ProviderRegistry {
    /// An empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a provider.
    pub fn register(&mut self, provider: Arc<dyn Provider>) {
        tracing::debug!(provider = %provider.id().0, "registered provider");
        self.providers.push(provider);
    }

    /// The first provider offering `capability`, or `None`.
    pub fn first_with(&self, capability: ProviderCapability) -> Option<Arc<dyn Provider>> {
        self.providers.iter().find(|p| p.capabilities().contains(&capability)).cloned()
    }

    /// The provider with `id`, or `None`.
    pub fn by_id(&self, id: &ProviderId) -> Option<Arc<dyn Provider>> {
        self.providers.iter().find(|p| &p.id() == id).cloned()
    }

    /// All registered providers.
    pub fn all(&self) -> &[Arc<dyn Provider>] {
        &self.providers
    }
}

/// Why a provider could not produce a result.
#[derive(Debug, Error)]
pub enum ProviderError {
    /// The provider is not available (offline, missing key, etc.).
    #[error("provider unavailable: {0}")]
    Unavailable(String),
    /// The request was cancelled before it finished.
    #[error("generation cancelled")]
    Cancelled,
    /// The provider produced output that is not a valid image.
    #[error("provider produced invalid image: {0}")]
    BadOutput(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generated::{GeneratedAsset, GeneratedResult, GenerationProvenance};

    struct Fake;

    impl Provider for Fake {
        fn id(&self) -> ProviderId {
            ProviderId("fake".to_owned())
        }
        fn label_key(&self) -> &'static str {
            "provider.fake.label"
        }
        fn capabilities(&self) -> &[ProviderCapability] {
            &[ProviderCapability::TextToSprite]
        }
        fn generate<'a>(&'a self, _input: &'a GenerationJobInput, _cancel: CancellationToken) -> GenerateFuture<'a> {
            Box::pin(async {
                Ok(GeneratedResult::Sprite(GeneratedAsset {
                    width: 1,
                    height: 1,
                    stride: 4,
                    rgba: vec![0, 0, 0, 255],
                    provenance: GenerationProvenance {
                        prompt: String::new(),
                        seed: 0,
                        provider_id: "fake".to_owned(),
                        model: "fake".to_owned(),
                        created_unix_ms: 0,
                    },
                }))
            })
        }
    }

    #[test]
    fn first_with_finds_a_registered_capability() {
        let mut registry = ProviderRegistry::new();
        assert!(registry.first_with(ProviderCapability::TextToSprite).is_none());
        registry.register(Arc::new(Fake));
        assert!(registry.first_with(ProviderCapability::TextToSprite).is_some());
    }

    #[test]
    fn by_id_round_trips() {
        let mut registry = ProviderRegistry::new();
        registry.register(Arc::new(Fake));
        assert!(registry.by_id(&ProviderId("fake".to_owned())).is_some());
        assert!(registry.by_id(&ProviderId("nope".to_owned())).is_none());
        assert_eq!(registry.all().len(), 1);
    }
}
