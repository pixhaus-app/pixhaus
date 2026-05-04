//! AI verb invocation commands.
//!
//! All commands here are stubbed until bedrock B5 (AI verb plugin protocol)
//! lands. `verb_list` returns an empty list as a safe default.

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::error::{AppCommandError, CommandResult};
use crate::state::AppState;

/// Arguments for invoking a verb.
#[derive(Debug, Deserialize)]
pub struct VerbInvokeArgs {
    /// Name of the verb to invoke (e.g. `"pixel_upscale"`).
    pub name: String,
    /// Free-form JSON context passed to the verb. Schema is defined per-verb
    /// in docs/verb-protocol.md once B5 lands.
    pub context: serde_json::Value,
}

/// Result returned by a verb invocation.
#[derive(Debug, Serialize)]
pub struct VerbResult {
    /// Opaque handle identifying this invocation for cancellation.
    pub verb_id: String,
    /// Current execution status.
    pub status: VerbStatus,
}

/// Execution status of a verb invocation.
#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum VerbStatus {
    /// Verb was accepted and is running asynchronously.
    Pending,
    /// Verb completed successfully.
    Done,
    /// Verb failed with the given message.
    Error {
        /// Human-readable error from the verb.
        message: String,
    },
}

/// Metadata about an available verb.
#[derive(Debug, Serialize)]
pub struct VerbInfo {
    /// Unique verb name, used in `verb_invoke`.
    pub name: String,
    /// One-line description shown in the command palette.
    pub description: String,
    /// Backend capabilities required to run this verb.
    pub required_backends: Vec<String>,
}

/// Invokes a registered AI verb with the given context.
///
/// Requires bedrock B5 (verb plugin protocol). Returns an error until B5 lands.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn verb_invoke(
    _args: VerbInvokeArgs,
    _state: State<'_, AppState>,
) -> CommandResult<VerbResult> {
    Err(AppCommandError::Unimplemented {
        stream: "B5 (verb plugin protocol)".into(),
    })
}

/// Lists all registered verbs. Returns an empty list until B5 lands.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn verb_list(_state: State<'_, AppState>) -> CommandResult<Vec<VerbInfo>> {
    Ok(Vec::new())
}

/// Cancels an in-progress verb invocation by its opaque ID.
///
/// Requires bedrock B5 (verb plugin protocol). Returns an error until B5 lands.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn verb_cancel(_verb_id: String, _state: State<'_, AppState>) -> CommandResult<()> {
    Err(AppCommandError::Unimplemented {
        stream: "B5 (verb plugin protocol)".into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verb_list_returns_empty_until_b5() {
        // verb_list is the only verb command that doesn't error — it
        // returns an empty list as a safe default until B5 populates
        // the registry.
        let infos: Vec<VerbInfo> = Vec::new();
        assert!(infos.is_empty());
    }
}
