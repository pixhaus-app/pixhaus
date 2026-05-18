//! AI backend settings commands.

use pixhaus_ai::backends::fal::FalBackend;
use pixhaus_ai::backends::google::GoogleAiBackend;
use pixhaus_ai::backends::openai::{DEFAULT_IMAGE_MODEL, OpenAiBackend};
use pixhaus_ai::backends::{ApiKeyStore, BackendError, BackendProxy};
use serde::Serialize;
use tauri::State;

use crate::error::{AppCommandError, CommandResult};
use crate::state::AppState;

/// Redacted `OpenAI` backend configuration status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OpenAiStatus {
    /// Whether an `OpenAI` API key is stored in the OS keychain.
    pub configured: bool,
    /// Whether the `OpenAI` backend is currently registered in the verb runtime.
    pub registered: bool,
    /// Image model used by Pixhaus reference-sheet generation.
    pub model: &'static str,
}

/// Redacted provider state used by the v1 AI preferences panel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProviderStatus {
    /// Stable provider id (`openai`, `google_ai`, `fal`).
    pub provider: &'static str,
    /// User-facing provider label.
    pub label: &'static str,
    /// Whether an API key is stored in the OS keychain.
    pub configured: bool,
    /// Whether a runtime backend is currently registered.
    pub registered: bool,
    /// `not_configured`, `configured`, or `invalid`.
    pub state: &'static str,
    /// Product-facing model labels exposed by Pixhaus.
    pub models: &'static [&'static str],
}

struct ProviderSpec {
    id: &'static str,
    label: &'static str,
    models: &'static [&'static str],
}

const OPENAI: ProviderSpec = ProviderSpec {
    id: "openai",
    label: "OpenAI",
    models: &[DEFAULT_IMAGE_MODEL],
};

const GOOGLE_AI: ProviderSpec = ProviderSpec {
    id: "google_ai",
    label: "Google AI Studio",
    models: &[
        "gemini-3-pro-image-preview",
        "gemini-3.1-flash-image-preview",
    ],
};

const FAL: ProviderSpec = ProviderSpec {
    id: "fal",
    label: "fal.ai",
    models: &[
        "fal-ai/flux-pro/kontext",
        "fal-ai/flux-lora",
        "fal-ai/recraft/vectorize",
        "fal-ai/real-esrgan",
    ],
};

fn status_for(spec: &ProviderSpec, state: &AppState) -> CommandResult<ProviderStatus> {
    let configured = provider_key_configured(spec.id)?;
    let (registered, available) = provider_presence(state, spec.id);
    Ok(provider_status(
        spec.id,
        spec.label,
        configured,
        registered,
        available,
        spec.models,
    ))
}

fn status_optimistic(spec: &ProviderSpec, configured: bool) -> ProviderStatus {
    provider_status(
        spec.id,
        spec.label,
        configured,
        configured,
        configured,
        spec.models,
    )
}

/// Returns the `OpenAI` backend status without exposing the stored key.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn ai_get_openai_status(state: State<'_, AppState>) -> CommandResult<OpenAiStatus> {
    let configured = openai_key_configured()?;
    Ok(openai_status(&state, configured))
}

/// Stores an `OpenAI` API key and registers/re-registers the backend.
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

    ApiKeyStore::set(OPENAI.id, trimmed).map_err(|err| key_error(OPENAI.id, &err))?;
    let _ = state.verb_runtime.unregister_backend(OPENAI.id);
    state
        .verb_runtime
        .register_backend(BackendProxy::new(OpenAiBackend::new(trimmed)), 0)
        .map_err(|e| AppCommandError::VerbError {
            message: e.to_string(),
        })?;

    Ok(openai_status(&state, true))
}

/// Deletes the stored `OpenAI` API key and unregisters the backend.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn ai_clear_openai_api_key(state: State<'_, AppState>) -> CommandResult<OpenAiStatus> {
    match ApiKeyStore::delete(OPENAI.id) {
        Ok(()) | Err(BackendError::ApiKeyNotFound(_)) => {}
        Err(err) => return Err(key_error(OPENAI.id, &err)),
    }
    let _ = state.verb_runtime.unregister_backend(OPENAI.id);
    Ok(openai_status(&state, false))
}

/// Returns all provider statuses without exposing raw keys.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn ai_get_provider_overview(
    state: State<'_, AppState>,
) -> CommandResult<Vec<ProviderStatus>> {
    Ok(vec![
        status_for(&OPENAI, &state)?,
        status_for(&GOOGLE_AI, &state)?,
        status_for(&FAL, &state)?,
    ])
}

/// Returns the Google AI Studio key status.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn ai_get_google_ai_status(state: State<'_, AppState>) -> CommandResult<ProviderStatus> {
    status_for(&GOOGLE_AI, &state)
}

/// Stores a Google AI Studio API key. The provider adapter consumes this
/// when image routing is enabled.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn ai_set_google_ai_api_key(
    api_key: String,
    state: State<'_, AppState>,
) -> CommandResult<ProviderStatus> {
    let trimmed = api_key.trim();
    set_provider_key(GOOGLE_AI.id, trimmed)?;
    let _ = state.verb_runtime.unregister_backend(GOOGLE_AI.id);
    state
        .verb_runtime
        .register_backend(BackendProxy::new(GoogleAiBackend::new(trimmed)), 10)
        .map_err(|e| AppCommandError::VerbError {
            message: e.to_string(),
        })?;
    Ok(status_optimistic(&GOOGLE_AI, true))
}

/// Clears the Google AI Studio API key.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn ai_clear_google_ai_api_key(
    state: State<'_, AppState>,
) -> CommandResult<ProviderStatus> {
    clear_provider_key(GOOGLE_AI.id)?;
    let _ = state.verb_runtime.unregister_backend(GOOGLE_AI.id);
    Ok(status_optimistic(&GOOGLE_AI, false))
}

/// Returns the fal.ai key status.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn ai_get_fal_status(state: State<'_, AppState>) -> CommandResult<ProviderStatus> {
    status_for(&FAL, &state)
}

/// Stores a fal.ai API key. Flux, `LoRA` training, Recraft, and Real-ESRGAN
/// operations consume this provider.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn ai_set_fal_api_key(
    api_key: String,
    state: State<'_, AppState>,
) -> CommandResult<ProviderStatus> {
    let trimmed = api_key.trim();
    set_provider_key(FAL.id, trimmed)?;
    let _ = state.verb_runtime.unregister_backend(FAL.id);
    state
        .verb_runtime
        .register_backend(BackendProxy::new(FalBackend::new(trimmed)), 20)
        .map_err(|e| AppCommandError::VerbError {
            message: e.to_string(),
        })?;
    Ok(status_optimistic(&FAL, true))
}

/// Clears the fal.ai API key.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn ai_clear_fal_api_key(state: State<'_, AppState>) -> CommandResult<ProviderStatus> {
    clear_provider_key(FAL.id)?;
    let _ = state.verb_runtime.unregister_backend(FAL.id);
    Ok(status_optimistic(&FAL, false))
}

fn openai_key_configured() -> CommandResult<bool> {
    provider_key_configured(OPENAI.id)
}

fn openai_status(state: &AppState, configured: bool) -> OpenAiStatus {
    OpenAiStatus {
        configured,
        registered: provider_presence(state, OPENAI.id).0,
        model: DEFAULT_IMAGE_MODEL,
    }
}

fn provider_presence(state: &AppState, provider: &str) -> (bool, bool) {
    state
        .verb_runtime
        .list_backends()
        .iter()
        .find(|backend| backend.id == provider)
        .map_or((false, false), |backend| (true, backend.available))
}

fn provider_status(
    provider: &'static str,
    label: &'static str,
    configured: bool,
    registered: bool,
    available: bool,
    models: &'static [&'static str],
) -> ProviderStatus {
    ProviderStatus {
        provider,
        label,
        configured,
        registered,
        state: if !configured {
            "not_configured"
        } else if registered && available {
            "configured"
        } else {
            "invalid"
        },
        models,
    }
}

fn provider_key_configured(provider: &str) -> CommandResult<bool> {
    match ApiKeyStore::get(provider) {
        Ok(key) => Ok(!key.trim().is_empty()),
        Err(BackendError::ApiKeyNotFound(_)) => Ok(false),
        Err(err) => Err(key_error(provider, &err)),
    }
}

fn set_provider_key(provider: &str, api_key: &str) -> CommandResult<()> {
    let trimmed = api_key.trim();
    if trimmed.is_empty() {
        return Err(AppCommandError::Validation {
            detail: "API key must not be empty".into(),
        });
    }
    ApiKeyStore::set(provider, trimmed).map_err(|err| key_error(provider, &err))
}

fn clear_provider_key(provider: &str) -> CommandResult<()> {
    match ApiKeyStore::delete(provider) {
        Ok(()) | Err(BackendError::ApiKeyNotFound(_)) => Ok(()),
        Err(err) => Err(key_error(provider, &err)),
    }
}

fn key_error(provider: &str, err: &BackendError) -> AppCommandError {
    AppCommandError::Validation {
        detail: format!("{provider} keychain error: {err}"),
    }
}
