//! Pixhaus application shell entry point.
//!
//! IPC commands land here per the catalog in B4. The shell wires the core,
//! io, ai, and scripting crates into a Tauri 2 process.

#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::missing_panics_doc
    )
)]

use tracing_subscriber::EnvFilter;

/// Application errors at the shell layer.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    /// The Tauri runtime returned an error during startup or operation.
    #[error("tauri runtime error: {0}")]
    Tauri(#[from] tauri::Error),
}

/// Returns the application name. Used by tracing setup and the window title.
#[must_use]
pub fn app_name() -> &'static str {
    "Pixhaus"
}

/// Builds and runs the Tauri application.
///
/// # Errors
/// Returns [`AppError::Tauri`] when the Tauri runtime fails to start.
pub fn run() -> Result<(), AppError> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    tracing::info!("starting {}", app_name());

    // tauri::generate_context! still expands to code that calls .unwrap() on
    // baked-in results in Tauri 2.11; the disallowed_methods lint can't see
    // through the macro. Re-test on each Tauri minor bump and remove if fixed.
    #[allow(clippy::disallowed_methods)]
    let context = tauri::generate_context!();

    tauri::Builder::default().run(context)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_name_is_stable() {
        assert_eq!(app_name(), "Pixhaus");
    }
}
