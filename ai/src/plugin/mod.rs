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
//!
//! See `docs/verb-protocol.md` for the user-facing reference and a
//! line-by-line walk-through of the [`echo::EchoVerb`] reference
//! plugin.

pub mod audio_timing;
pub mod backend;
pub mod context;
pub mod conversational;
pub mod descriptor;
pub mod echo;
pub mod error;
pub mod inputs;
pub mod output;
pub mod preview;
pub mod progress;
pub mod runtime;
pub mod verb;
pub mod view_synthesis;

pub use crate::verbs::auto_mesh_deformation::{
    AUTO_MESH_DEFORMATION_VERB_ID, AutoMeshDeformationVerb, AutoMeshInputs,
    MESH_RESOLUTION_DEFAULT, MESH_RESOLUTION_MAX, MESH_RESOLUTION_MIN, RigControlPoint, RigData,
    RigRegion,
};
pub use crate::verbs::inbetween::{INBETWEEN_VERB_ID, InbetweenInputs, InbetweenVerb};
pub use crate::verbs::sketch_finishing::{
    SKETCH_FINISHING_VERB_ID, SketchFinishingInputs, SketchFinishingVerb, SketchFrame,
};
pub use audio_timing::{
    AUDIO_TIMING_VERB_ID, AudioFormat, AudioTimingInputs, AudioTimingMode, AudioTimingVerb,
};
pub use backend::{BackendInfo, InferenceBackend};
pub use context::{PixelData, ReferenceImage, StyleReference, VerbContext, VerbContextBuilder};
pub use conversational::{CONVERSATIONAL_VERB_ID, ConversationalInputs, ConversationalVerb};
pub use descriptor::{BackendCapabilities, CostEstimate, EffectKind, VerbDescriptor, VerbId};
pub use echo::{ECHO_VERB_ID, EchoInputs, EchoVerb};
pub use error::{Result, VerbError};
pub use inputs::VerbInputs;
pub use output::{
    ActualCost, CritiqueCategory, CritiqueFinding, CritiqueSeverity, NewPixelBuffer, VerbEffect,
    VerbOutput,
};
pub use preview::{PreviewId, PreviewIdMinter, VerbCommit, VerbDiscard, VerbPreview};
pub use progress::{
    CostUpdate, LogLevel, PROGRESS_CHANNEL_CAPACITY, VerbProgress, VerbProgressEvent,
};
pub use runtime::{VerbInvocation, VerbRuntime};
pub use verb::Verb;
pub use view_synthesis::{Direction, DirectionalViewRequest, ViewSynthesisBackend};

// Re-export built-in verbs so downstream crates can register them via a single
// `use pixhaus_ai::plugin::*` rather than digging into the verbs sub-tree.
pub use crate::verbs::project_style_learning::{
    PROJECT_STYLE_LEARNING_VERB_ID, ProjectStyleLearningVerb, StyleLearningInputs, TrainedModelRef,
};
