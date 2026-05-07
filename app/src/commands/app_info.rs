//! Application metadata commands.
//!
//! Exposes a small read-only surface so the UI can render the about
//! dialog without baking version strings into the JS bundle (which
//! would drift the moment Cargo bumps the workspace version).

use serde::Serialize;

use crate::error::CommandResult;

/// Static application metadata returned by [`app_about`].
#[derive(Debug, Serialize)]
pub struct AppAbout {
    /// Display name of the application.
    pub name: &'static str,
    /// Semantic version baked in at compile time from `CARGO_PKG_VERSION`.
    pub version: &'static str,
}

/// Returns the application name and version for the about dialog.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn app_about() -> CommandResult<AppAbout> {
    Ok(AppAbout {
        name: "Pixhaus",
        version: env!("CARGO_PKG_VERSION"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn returns_name_and_version() {
        let info = app_about().await.expect("app_about must succeed");
        assert_eq!(info.name, "Pixhaus");
        assert!(!info.version.is_empty(), "version must be non-empty");
    }
}
