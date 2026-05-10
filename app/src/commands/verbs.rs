//! AI verb invocation commands.
//!
//! `verb_list` returns the descriptors of all registered verbs, including
//! plugin-registered verbs (the plugin registry shares the same runtime).
//! `verb_invoke` runs a verb and returns its output plus an invocation
//! handle that `verb_cancel` can use to interrupt long-running calls.
//! In-flight invocations live in `AppState::invocations`, keyed by the
//! runtime's `PreviewId` (rendered as a string at the IPC boundary so
//! the JS side never has to think about u64 precision).

use serde::{Deserialize, Serialize};
use tauri::State;

use pixhaus_ai::plugin::context::VerbContext;
use pixhaus_ai::plugin::descriptor::VerbId;
use pixhaus_ai::plugin::inputs::VerbInputs;
use pixhaus_ai::plugin::output::VerbOutput;
use pixhaus_core::project::ProjectMetadata;

use crate::error::{AppCommandError, CommandResult};
use crate::state::AppState;

/// Arguments for invoking a verb.
#[derive(Debug, Deserialize)]
pub struct VerbInvokeArgs {
    /// Stable verb ID (e.g. `"pixhaus.builtin.critique"`).
    pub verb_id: String,
    /// JSON payload whose schema is defined by the verb's descriptor.
    pub inputs: serde_json::Value,
}

/// Result of a verb invocation: the output the host commits, plus the
/// opaque handle the UI hands back to `verb_cancel` if the user
/// interrupts a still-running invocation.
#[derive(Debug, Serialize)]
pub struct VerbInvocationResult {
    /// Per-invocation handle. Stringified `PreviewId` so the JS side
    /// doesn't lose precision on values above 2^53.
    pub invocation_id: String,
    /// The verb's output, ready for preview / commit.
    pub output: VerbOutput,
}

/// Metadata about an available verb.
#[derive(Debug, Serialize)]
pub struct VerbInfo {
    /// Stable verb ID, used in `verb_invoke`.
    pub id: String,
    /// Display name for menus and the command palette.
    pub display_name: String,
    /// One-line description shown in the command palette.
    pub description: String,
    /// Whether the verb can be cancelled mid-run.
    pub cancellable: bool,
    /// Backend capabilities required to run this verb.
    pub required_capabilities: u32,
    /// JSON Schema for the verb's input payload — surfaced so the UI
    /// can render an input form without baking per-verb knowledge.
    pub input_schema: serde_json::Value,
}

/// Lists all verbs registered with the runtime, sorted by ID.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn verb_list(state: State<'_, AppState>) -> CommandResult<Vec<VerbInfo>> {
    let descriptors = state.verb_runtime.list();
    Ok(descriptors
        .into_iter()
        .map(|d| VerbInfo {
            id: d.id.as_str().to_owned(),
            display_name: d.display_name,
            description: d.description,
            cancellable: d.cancellable,
            required_capabilities: d.required_capabilities.0,
            input_schema: d.input_schema,
        })
        .collect())
}

/// Invokes a registered verb synchronously and returns its output.
///
/// Builds a [`VerbContext`] from the current document state, dispatches
/// through the [`pixhaus_ai::plugin::runtime::VerbRuntime`], awaits
/// completion, and returns the [`VerbOutput`].
///
/// Returns an error when:
/// - the verb ID is not registered,
/// - the inputs fail schema validation,
/// - no backend satisfies the verb's required capabilities,
/// - the verb itself returns an error.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn verb_invoke(
    args: VerbInvokeArgs,
    state: State<'_, AppState>,
) -> CommandResult<VerbInvocationResult> {
    let verb_id = VerbId::new(&args.verb_id);
    let inputs = VerbInputs::new(args.inputs);

    // Build a minimal VerbContext from the current document state. The
    // full context (sprite, palette, references) requires a read guard on
    // the document; we release it before awaiting the verb so we never
    // hold a lock across an I/O suspension.
    let ctx = {
        let doc = state.doc.read().await;
        let project_meta = doc.project.as_ref().map_or_else(
            || ProjectMetadata {
                name: "untitled".into(),
                description: None,
                author: None,
                created_at: 0,
                updated_at: 0,
                editor_version: env!("CARGO_PKG_VERSION").into(),
            },
            |p| p.metadata.clone(),
        );
        VerbContext::empty(project_meta)
        // doc guard drops here
    };

    let invocation = state
        .verb_runtime
        .invoke(&verb_id, ctx, inputs)
        .map_err(|e| AppCommandError::VerbError {
            message: e.to_string(),
        })?;

    // Register the cancel token before awaiting the verb body so a
    // concurrent `verb_cancel` IPC call can find and fire it.
    let preview_id = invocation.preview_id().get();
    state
        .invocations
        .insert(preview_id, invocation.cancellation());

    let result = invocation.finish().await;

    // Always remove the entry — successful, cancelled, or errored.
    state.invocations.remove(&preview_id);

    let preview = result.map_err(|e| AppCommandError::VerbError {
        message: e.to_string(),
    })?;

    Ok(VerbInvocationResult {
        invocation_id: preview_id.to_string(),
        output: preview.output,
    })
}

/// Cancels an in-progress verb invocation by its opaque ID.
///
/// `invocation_id` is the value returned by `verb_invoke` (a stringified
/// `PreviewId`). Cancellation is cooperative: the verb observes the
/// token between expensive operations and returns
/// [`pixhaus_ai::plugin::error::VerbError::Cancelled`] when it sees the
/// fire. Idempotent — a missing id (already finished or never seen) is
/// not an error.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn verb_cancel(invocation_id: String, state: State<'_, AppState>) -> CommandResult<()> {
    let id: u64 = invocation_id
        .parse()
        .map_err(|_| AppCommandError::Validation {
            detail: format!("invocation_id is not a valid u64: {invocation_id:?}"),
        })?;
    if let Some((_, token)) = state.invocations.remove(&id) {
        token.cancel();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verb_info_fields_are_present() {
        let info = VerbInfo {
            id: "pixhaus.builtin.critique".into(),
            display_name: "Critique".into(),
            description: "VLM quality analysis".into(),
            cancellable: true,
            required_capabilities: 0b10, // VISION_LANGUAGE bit
            input_schema: serde_json::json!({"type": "object"}),
        };
        assert!(!info.id.is_empty());
        assert!(info.cancellable);
    }
}
