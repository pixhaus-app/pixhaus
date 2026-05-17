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

const GOOGLE_IMAGE_MODELS: &[&str] = &[
    "gemini-3-pro-image-preview",
    "gemini-3.1-flash-image-preview",
];

const FAL_IMAGE_MODELS: &[&str] = &[
    "fal-ai/flux-pro/kontext",
    "fal-ai/flux-lora",
    "fal-ai/recraft/vectorize",
    "fal-ai/real-esrgan",
];

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

    ApiKeyStore::set("openai", trimmed).map_err(|err| key_error("openai", &err))?;
    let _ = state.verb_runtime.unregister_backend("openai");
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
    match ApiKeyStore::delete("openai") {
        Ok(()) | Err(BackendError::ApiKeyNotFound(_)) => {}
        Err(err) => return Err(key_error("openai", &err)),
    }
    let _ = state.verb_runtime.unregister_backend("openai");
    Ok(openai_status(&state, false))
}

/// Returns all provider statuses without exposing raw keys.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn ai_get_provider_overview(
    state: State<'_, AppState>,
) -> CommandResult<Vec<ProviderStatus>> {
    Ok(vec![
        provider_status(
            "openai",
            "OpenAI",
            openai_key_configured()?,
            openai_registered(&state),
            &[DEFAULT_IMAGE_MODEL],
        ),
        provider_status(
            "google_ai",
            "Google AI Studio",
            provider_key_configured("google_ai")?,
            provider_registered(&state, "google_ai"),
            GOOGLE_IMAGE_MODELS,
        ),
        provider_status(
            "fal",
            "fal.ai",
            provider_key_configured("fal")?,
            provider_registered(&state, "fal"),
            FAL_IMAGE_MODELS,
        ),
    ])
}

/// Returns the Google AI Studio key status.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn ai_get_google_ai_status(state: State<'_, AppState>) -> CommandResult<ProviderStatus> {
    Ok(provider_status(
        "google_ai",
        "Google AI Studio",
        provider_key_configured("google_ai")?,
        provider_registered(&state, "google_ai"),
        GOOGLE_IMAGE_MODELS,
    ))
}

/// Stores a Google AI Studio API key. The provider adapter consumes this
/// when image routing is enabled.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn ai_set_google_ai_api_key(
    api_key: String,
    state: State<'_, AppState>,
) -> CommandResult<ProviderStatus> {
    let trimmed = api_key.trim();
    set_provider_key("google_ai", trimmed)?;
    let _ = state.verb_runtime.unregister_backend("google_ai");
    state
        .verb_runtime
        .register_backend(BackendProxy::new(GoogleAiBackend::new(trimmed)), 10)
        .map_err(|e| AppCommandError::VerbError {
            message: e.to_string(),
        })?;
    Ok(provider_status(
        "google_ai",
        "Google AI Studio",
        true,
        true,
        GOOGLE_IMAGE_MODELS,
    ))
}

/// Clears the Google AI Studio API key.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn ai_clear_google_ai_api_key(
    state: State<'_, AppState>,
) -> CommandResult<ProviderStatus> {
    clear_provider_key("google_ai")?;
    let _ = state.verb_runtime.unregister_backend("google_ai");
    Ok(provider_status(
        "google_ai",
        "Google AI Studio",
        false,
        false,
        GOOGLE_IMAGE_MODELS,
    ))
}

/// Returns the fal.ai key status.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn ai_get_fal_status(state: State<'_, AppState>) -> CommandResult<ProviderStatus> {
    Ok(provider_status(
        "fal",
        "fal.ai",
        provider_key_configured("fal")?,
        provider_registered(&state, "fal"),
        FAL_IMAGE_MODELS,
    ))
}

/// Stores a fal.ai API key. Flux, LoRA training, Recraft, and Real-ESRGAN
/// operations consume this provider.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn ai_set_fal_api_key(
    api_key: String,
    state: State<'_, AppState>,
) -> CommandResult<ProviderStatus> {
    let trimmed = api_key.trim();
    set_provider_key("fal", trimmed)?;
    let _ = state.verb_runtime.unregister_backend("fal");
    state
        .verb_runtime
        .register_backend(BackendProxy::new(FalBackend::new(trimmed)), 20)
        .map_err(|e| AppCommandError::VerbError {
            message: e.to_string(),
        })?;
    Ok(provider_status(
        "fal",
        "fal.ai",
        true,
        true,
        FAL_IMAGE_MODELS,
    ))
}

/// Clears the fal.ai API key.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn ai_clear_fal_api_key(state: State<'_, AppState>) -> CommandResult<ProviderStatus> {
    clear_provider_key("fal")?;
    let _ = state.verb_runtime.unregister_backend("fal");
    Ok(provider_status(
        "fal",
        "fal.ai",
        false,
        false,
        FAL_IMAGE_MODELS,
    ))
}

fn openai_key_configured() -> CommandResult<bool> {
    provider_key_configured("openai")
}

fn openai_status(state: &AppState, configured: bool) -> OpenAiStatus {
    OpenAiStatus {
        configured,
        registered: openai_registered(state),
        model: DEFAULT_IMAGE_MODEL,
    }
}

fn openai_registered(state: &AppState) -> bool {
    provider_registered(state, "openai")
}

fn provider_registered(state: &AppState, provider: &str) -> bool {
    state
        .verb_runtime
        .list_backends()
        .iter()
        .any(|backend| backend.id == provider && backend.available)
}

fn provider_status(
    provider: &'static str,
    label: &'static str,
    configured: bool,
    registered: bool,
    models: &'static [&'static str],
) -> ProviderStatus {
    ProviderStatus {
        provider,
        label,
        configured,
        registered,
        state: if configured {
            "configured"
        } else {
            "not_configured"
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
