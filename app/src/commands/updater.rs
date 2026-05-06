//! Update-check and install commands.
//!
//! On startup the runtime calls [`check_on_startup`], which emits
//! `updater:available` when a new version is found. The UI reacts to that
//! event and surfaces a toast; the user then triggers [`updater_install`]
//! from the preferences or AI menu to download and apply the update.
//!
//! Both the check and the install paths talk directly to the
//! `tauri-plugin-updater` API, so no update state is stored in `AppState`.
//! A user who dismisses the toast and later wants to install can trigger
//! another check from the UI, which re-fetches the manifest.

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use tauri_plugin_updater::UpdaterExt;
use ts_rs::TS;

use crate::error::{AppCommandError, CommandResult};

/// Metadata returned to the frontend when an update is available.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct UpdateInfo {
    /// `SemVer` version string of the available release, e.g. `"0.2.0"`.
    pub version: String,
    /// Release notes extracted from the update manifest (Markdown).
    pub body: Option<String>,
    /// RFC 3339 publication timestamp of the release.
    pub date: Option<String>,
}

/// Payload emitted on the `updater:available` event.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct UpdateAvailablePayload {
    /// Available update details.
    pub update: UpdateInfo,
}

/// Payload emitted on the `updater:progress` event during download.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct UpdateProgressPayload {
    /// Bytes downloaded so far.
    pub downloaded: u64,
    /// Total bytes to download, if the server reported `Content-Length`.
    pub total: Option<u64>,
}

fn updater_err(e: impl std::fmt::Display) -> AppCommandError {
    AppCommandError::Validation {
        detail: format!("updater: {e}"),
    }
}

/// Checks for an available update.
///
/// Returns `Some(UpdateInfo)` when a newer version is available, or `None`
/// when the running build is already the latest.
#[tauri::command(async)]
pub async fn updater_check(app: AppHandle) -> CommandResult<Option<UpdateInfo>> {
    let updater = app.updater().map_err(updater_err)?;
    match updater.check().await {
        Ok(Some(u)) => Ok(Some(UpdateInfo {
            version: u.version.clone(),
            body: u.body.clone(),
            date: u.date.map(|d| d.to_string()),
        })),
        Ok(None) => Ok(None),
        Err(e) => Err(updater_err(e)),
    }
}

/// Downloads and installs the latest available update.
///
/// Emits `updater:progress` events during the download and
/// `updater:ready` when the installer is staged. On most platforms the
/// app restarts automatically; on Windows with NSIS the user may see an
/// installer prompt.
#[tauri::command(async)]
pub async fn updater_install(app: AppHandle) -> CommandResult<()> {
    let updater = app.updater().map_err(updater_err)?;
    let update =
        updater
            .check()
            .await
            .map_err(updater_err)?
            .ok_or_else(|| AppCommandError::Validation {
                detail: "no update available".into(),
            })?;

    let app_clone = app.clone();
    let mut downloaded: u64 = 0;

    update
        .download_and_install(
            move |chunk, total| {
                downloaded += chunk as u64;
                let _ = app_clone.emit(
                    "updater:progress",
                    UpdateProgressPayload { downloaded, total },
                );
            },
            || {},
        )
        .await
        .map_err(updater_err)?;

    let _ = app.emit("updater:ready", ());
    Ok(())
}

/// Called once from the app `setup` closure to check for updates in the
/// background without blocking startup.
///
/// Emits `updater:available` if a new version is found. Errors are
/// logged at `warn` level and silently swallowed — an update check
/// failing is never user-visible unless the user manually triggers one.
pub async fn check_on_startup(app: AppHandle) {
    let Ok(updater) = app.updater() else {
        return;
    };
    match updater.check().await {
        Ok(Some(u)) => {
            tracing::info!("update available: {}", u.version);
            let payload = UpdateAvailablePayload {
                update: UpdateInfo {
                    version: u.version.clone(),
                    body: u.body.clone(),
                    date: u.date.map(|d| d.to_string()),
                },
            };
            if let Err(e) = app.emit("updater:available", payload) {
                tracing::warn!("failed to emit updater:available: {e}");
            }
        }
        Ok(None) => tracing::debug!("app is up to date"),
        Err(e) => tracing::warn!("update check failed: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_info_serializes() {
        let info = UpdateInfo {
            version: "0.2.0".into(),
            body: Some("Bug fixes".into()),
            date: Some("2026-01-01T00:00:00Z".into()),
        };
        let json = serde_json::to_string(&info).expect("serialize");
        assert!(json.contains("0.2.0"));
    }

    #[test]
    fn updater_err_wraps_message() {
        let err = updater_err("connection refused");
        match err {
            AppCommandError::Validation { detail } => {
                assert!(detail.contains("connection refused"));
            }
            _ => panic!("expected Validation variant"),
        }
    }
}
