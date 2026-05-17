//! AI backend settings commands.

use pixhaus_ai::backends::openai::{DEFAULT_IMAGE_MODEL, OpenAiBackend};
use pixhaus_ai::backends::{ApiKeyStore, BackendError, BackendProxy};
use serde::Serialize;
use tauri::State;

use crate::error::{AppCommandError, CommandResult};
use crate::state::AppState;

/// Redacted OpenAI backend configuration status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OpenAiStatus {
    /// Whether an OpenAI API key is stored in the OS keychain.
    pub configured: bool,
    /// Whether the OpenAI backend is currently registered in the verb runtime.
    pub registered: bool,
    /// Image model used by Pixhaus reference-sheet generation.
    pub model: &'static str,
}

/// Returns the OpenAI backend status without exposing the stored key.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn ai_get_openai_status(state: State<'_, AppState>) -> CommandResult<OpenAiStatus> {
    let configured = openai_key_configured()?;
    Ok(openai_status(&state, configured))
}

/// Stores an OpenAI API key and registers/re-registers the backend.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn ai_set_openai_api_key(
    api_key: String,
    state: State<'_, AppState>,
) -> CommandResult<OpenAiStatus> {
    let trimmed = api_key.trim();
    if trimmed.is_empty() {
        return Err(AppCommandError::Validation {
            detail: "OpenAI API key must not be empty".into(),
        });
    }

    ApiKeyStore::set("openai", trimmed).map_err(key_error)?;
    let _ = state.verb_runtime.unregister_backend("openai");
    state
        .verb_runtime
        .register_backend(BackendProxy::new(OpenAiBackend::new(trimmed)), 0)
        .map_err(|e| AppCommandError::VerbError {
            message: e.to_string(),
        })?;

    Ok(openai_status(&state, true))
}

/// Deletes the stored OpenAI API key and unregisters the backend.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn ai_clear_openai_api_key(state: State<'_, AppState>) -> CommandResult<OpenAiStatus> {
    match ApiKeyStore::delete("openai") {
        Ok(()) | Err(BackendError::ApiKeyNotFound(_)) => {}
        Err(err) => return Err(key_error(err)),
    }
    let _ = state.verb_runtime.unregister_backend("openai");
    Ok(openai_status(&state, false))
}

fn openai_key_configured() -> CommandResult<bool> {
    match ApiKeyStore::get("openai") {
        Ok(key) => Ok(!key.trim().is_empty()),
        Err(BackendError::ApiKeyNotFound(_)) => Ok(false),
        Err(err) => Err(key_error(err)),
    }
}

fn openai_status(state: &AppState, configured: bool) -> OpenAiStatus {
    let registered = state
        .verb_runtime
        .list_backends()
        .iter()
        .any(|backend| backend.id == "openai" && backend.available);
    OpenAiStatus {
        configured,
        registered,
        model: DEFAULT_IMAGE_MODEL,
    }
}

fn key_error(err: BackendError) -> AppCommandError {
    AppCommandError::Validation {
        detail: format!("OpenAI keychain error: {err}"),
    }
}
