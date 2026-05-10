//! AI verb invocation commands.
//!
//! `verb_list` returns the descriptors of all registered verbs, including
//! plugin-registered verbs (the plugin registry shares the same runtime).
//! `verb_invoke` runs a verb synchronously and returns its output.
//! `verb_cancel` is not yet wired (in-flight cancellation requires a
//! per-session invocation map; tracked for a follow-up stream).

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

/// Metadata about an available verb.
#[derive(Debug, Serialize)]
pub struct VerbInfo {
    /// Stable verb ID, used in `verb_invoke`.
    pub id: String,
    /// One-line description shown in the command palette.
    pub description: String,
    /// Whether the verb can be cancelled mid-run.
    pub cancellable: bool,
    /// Backend capabilities required to run this verb.
    pub required_capabilities: u32,
}

/// Lists all verbs registered with the runtime, sorted by ID.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn verb_list(state: State<'_, AppState>) -> CommandResult<Vec<VerbInfo>> {
    let descriptors = state.verb_runtime.list();
    Ok(descriptors
        .into_iter()
        .map(|d| VerbInfo {
            id: d.id.as_str().to_owned(),
            description: d.description,
            cancellable: d.cancellable,
            required_capabilities: d.required_capabilities.0,
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
) -> CommandResult<VerbOutput> {
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

    let preview = invocation
        .finish()
        .await
        .map_err(|e| AppCommandError::VerbError {
            message: e.to_string(),
        })?;

    Ok(preview.output)
}

/// Cancels an in-progress verb invocation by its opaque ID.
///
/// `invocation_id` identifies a specific in-flight call, not a verb
/// type — multiple concurrent invocations of the same verb each get
/// their own id. Per-session in-flight tracking is not yet wired,
/// so this command returns `Unimplemented` until the invocation map
/// lands.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn verb_cancel(_invocation_id: String, _state: State<'_, AppState>) -> CommandResult<()> {
    Err(AppCommandError::Unimplemented {
        stream: "verb cancellation (in-flight invocation map)".into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verb_info_fields_are_present() {
        let info = VerbInfo {
            id: "pixhaus.builtin.critique".into(),
            description: "VLM quality analysis".into(),
            cancellable: true,
            required_capabilities: 0b10, // VISION_LANGUAGE bit
        };
        assert!(!info.id.is_empty());
        assert!(info.cancellable);
    }
}
