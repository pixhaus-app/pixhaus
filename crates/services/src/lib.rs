//! Pixhaus service layer: command execution, jobs, dispatch, and localization.
//!
//! `services` owns the shared behavior that sits above the domain model and
//! below the UI — command execution and undo/redo, transactions, the background
//! job system, asset indexing, provider dispatch, and the localization service.
//! It depends on `core` and never on egui.
//!
//! Foundation stage: command execution and undo/redo ([`History`], [`Transaction`])
//! and the background job system ([`JobManager`], [`ProviderRegistry`],
//! [`ResultStore`]) are live. The localization service ([`i18n`]) is the one place
//! rust-i18n is wired up, the string-side parallel to the binary owning the one
//! tracing subscriber.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::disallowed_methods, clippy::panic))]

// The one `i18n!` for the whole app: embeds every `locales/*.yaml` bundle at
// compile time and generates the crate-local `t!` machinery the `i18n` module
// wraps. `en` is the fallback when a key is missing in the active language.
rust_i18n::i18n!("locales", fallback = "en");

pub mod error;
pub mod generated;
pub mod history;
pub mod i18n;
pub mod job;
pub mod provider;
pub mod result_store;
pub mod transaction;

pub use error::ServiceError;
pub use generated::{GeneratedAnimation, GeneratedAsset, GeneratedFrame, GeneratedResult, GenerationProvenance, ResultKind};
pub use history::History;
pub use job::{GenerationContext, GenerationJobInput, GenerationKind, Grid, JobId, JobManager, JobMsg, JobStatus, ReferenceImage};
pub use provider::{GenerateFuture, Provider, ProviderCapability, ProviderError, ProviderId, ProviderRegistry};
pub use result_store::ResultStore;
pub use transaction::Transaction;
