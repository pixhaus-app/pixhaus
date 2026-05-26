//! AI verb plugin protocol — bedrock B5.
//!
//! Defines the contract every AI verb implements and the runtime that
//! coordinates them. The protocol covers:
//!
//! - **Declaration.** [`descriptor::VerbDescriptor`] publishes a
//!   verb's identity, schemas, backend requirements, cost estimate,
//!   and effect kinds.
//! - **Context.** [`context::VerbContext`] hands the verb a read-only
//!   snapshot of the project — palette, layer stack, active frame,
//!   reference images, project style references.
//! - **Inputs.** [`inputs::VerbInputs`] carries the verb-specific
//!   payload as JSON. The descriptor's `input_schema` is the contract
//!   the UI consults when generating forms; the runtime does not
//!   enforce it. Verbs implement [`verb::Verb::validate`] for the
//!   checks that matter.
//! - **Invocation.** [`verb::Verb::invoke`] runs the verb, returning a
//!   [`output::VerbOutput`] that describes effects to apply on commit.
//! - **Streaming.** Verbs emit [`progress::VerbProgressEvent`]s
//!   through the [`progress::VerbProgress`] sender; the runtime pairs
//!   it with a `tokio::sync::mpsc::Receiver` exposed on
//!   [`runtime::VerbInvocation`].
//! - **Cancellation.** Each invocation gets a `tokio_util::sync::CancellationToken`;
//!   cancellable verbs observe it between expensive operations.
//! - **Preview / commit / discard.** [`runtime::VerbInvocation::finish`]
//!   wraps the output in a [`preview::VerbPreview`] tagged with a
//!   fresh ID. The host shows the preview; on accept,
//!   [`runtime::VerbRuntime::commit`] returns a
//!   [`preview::VerbCommit`] for the undo system to consume.

pub mod anchor;
pub mod backend;
pub mod context;
pub mod descriptor;
pub mod error;
pub mod inputs;
pub mod output;
pub mod preview;
pub mod progress;
pub mod runtime;
pub mod verb;

pub use anchor::{AnchorPayload, DEFAULT_ANCHOR_STRENGTH};
pub use backend::{BackendInfo, InferenceBackend};
pub use context::{CompositionLibraryView, PixelData, ProjectCompositionLibrary, ReferenceImage, StyleReference, VerbContext, VerbContextBuilder};
pub use descriptor::{BackendCapabilities, CostEstimate, EffectKind, VerbDescriptor, VerbId};
pub use error::{Result, VerbError};
pub use inputs::VerbInputs;
pub use output::{ActualCost, CritiqueCategory, CritiqueFinding, CritiqueSeverity, NewPixelBuffer, VerbEffect, VerbOutput};
pub use preview::{PreviewId, PreviewIdMinter, VerbCommit, VerbDiscard, VerbPreview};
pub use progress::{CostUpdate, LogLevel, PROGRESS_CHANNEL_CAPACITY, VerbProgress, VerbProgressEvent};
pub use runtime::{VerbInvocation, VerbRuntime};
pub use verb::Verb;

// Re-export the one built-in verb this slice ports so downstream crates can
// register it via a single `use pixhaus_ai::plugin::*`.
pub use crate::verbs::reference_sheet::{GENERATE_REFERENCE_SHEET_VERB_ID, GenerateReferenceSheetInputs, GenerateReferenceSheetVerb};
