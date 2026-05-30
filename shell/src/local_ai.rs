//! Local-model UI plumbing — the download manager and the settings actions.
//!
//! The off-thread spawns model the shapes the rest of the shell already uses:
//! [`spawn_model_download`] mirrors `ai::spawn_clip` (a cancel token owned by the
//! [`ShellApp`] plus a progress closure that sends a `ShellMsg` and repaints),
//! and [`spawn_model_probe`] mirrors `ai::spawn_backend_key_op`'s blocking shape
//! (a presence check off the UI thread). The editor-mode Generate actions land
//! in a later phase; this module owns the settings-side plumbing.
//!
//! Everything that names `pixhaus_flux` / the on-device backend is gated behind
//! `local-flux`. With the feature off, the download spawn reports an immediate
//! failure and the probe reports "not downloaded", so the settings card degrades
//! to a disabled state without a separate code path in the UI.

use std::path::PathBuf;
use std::sync::mpsc::Sender;

use eframe::egui;
use tokio::runtime::Handle;
#[cfg(feature = "local-flux")]
use tokio_util::sync::CancellationToken;

#[cfg(feature = "local-flux")]
use crate::app::LocalModelStatus;
use crate::app::{ShellApp, ShellMsg};

impl ShellApp {
    /// Re-runs local-backend registration after a download completes, the cache
    /// directory changes, or the weights are deleted. Presence-only and instant —
    /// it never loads the multi-GB model — so it is safe on the UI thread.
    pub(crate) fn refresh_local_backend(&mut self) {
        let ready = crate::ai::try_register_local_flux(&self.verb_runtime, self.local_ai.device, self.local_ai.cache_dir.clone());
        // Local registration can flip overall readiness on (offline, no cloud key)
        // or, after a delete, the local backend simply stops being registered;
        // either way fold it into the status-bar chip.
        if ready {
            self.backend_ready = true;
        }
    }

    /// Starts a model download: creates a fresh cancel token (owned here so the
    /// Cancel button can abort), flips the card to `Downloading`, and spawns the
    /// streaming transfer off-thread.
    ///
    /// Driven only by the `local-flux` settings card; gated so a feature-off build
    /// carries no dead download driver.
    #[cfg(feature = "local-flux")]
    pub(crate) fn start_model_download(&mut self) {
        let cancel = CancellationToken::new();
        self.local_ai_cancel = Some(cancel.clone());
        self.local_ai_status = LocalModelStatus::Downloading {
            fraction: 0.0,
            bytes: 0,
            total: 0,
        };
        spawn_model_download(
            self.runtime.handle(),
            self.egui_ctx.clone(),
            self.tx.clone(),
            self.local_ai.default_model.clone(),
            self.local_ai.cache_dir.clone(),
            cancel,
        );
    }

    /// Cancels an in-flight model download. The streaming task sees the token and
    /// reports [`ShellMsg::ModelDownloadFailed`] (cancelled), which clears the
    /// token and flips the card back to a retry state.
    #[cfg(feature = "local-flux")]
    pub(crate) fn cancel_model_download(&mut self) {
        if let Some(cancel) = self.local_ai_cancel.take() {
            cancel.cancel();
        }
    }

    /// Deletes the downloaded weights off the UI thread, then re-probes presence
    /// and re-runs registration so the card and the backend chip update. A
    /// missing cache directory is treated as already deleted.
    #[cfg(feature = "local-flux")]
    pub(crate) fn delete_model(&mut self) {
        let Some(root) = self.local_model_cache_root() else {
            self.set_status("No cache directory to delete.");
            return;
        };
        let tx = self.tx.clone();
        let ctx = self.egui_ctx.clone();
        let model = self.local_ai.default_model.clone();
        let cache_dir = self.local_ai.cache_dir.clone();
        let probe_handle = self.runtime.handle().clone();
        self.runtime.handle().spawn_blocking(move || {
            // Best-effort recursive delete; a not-found dir is success. Any other
            // error surfaces as a failed status so the user knows the weights are
            // still on disk.
            let result = match std::fs::remove_dir_all(&root) {
                Ok(()) => Ok(()),
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(err) => Err(err.to_string()),
            };
            match result {
                Ok(()) => {
                    // Re-probe so the card flips to NotDownloaded.
                    spawn_model_probe(&probe_handle, ctx.clone(), tx.clone(), model.clone(), cache_dir);
                }
                Err(error) => {
                    let _ = tx.send(ShellMsg::ModelDownloadFailed { model, error });
                }
            }
            ctx.request_repaint();
        });
    }

    /// The cache root the local model lives under, honoring a settings override
    /// and falling back to the app-data default. `None` when no app-data
    /// directory can be resolved and no override is set.
    #[cfg(feature = "local-flux")]
    pub(crate) fn local_model_cache_root(&self) -> Option<PathBuf> {
        match &self.local_ai.cache_dir {
            Some(dir) => Some(dir.join(&self.local_ai.default_model)),
            None => default_model_cache_root(&self.local_ai.default_model),
        }
    }
}

/// The app-data model cache root, `<data_dir>/models/<model>`. Mirrors
/// `pixhaus_flux::ModelStore::default_root` without depending on the gated crate,
/// so the settings caption can show a path with the feature off.
#[cfg(feature = "local-flux")]
#[must_use]
pub(crate) fn default_model_cache_root(model: &str) -> Option<PathBuf> {
    let dirs = directories::ProjectDirs::from("app", "Pixhaus", "Pixhaus")?;
    Some(dirs.data_dir().join("models").join(model))
}

/// Spawns the streaming model download on the tokio runtime. Cumulative byte
/// progress, completion, and failure (including cancellation) arrive over `tx`;
/// `ctx` is woken after each message so the idle settings window repaints.
///
/// The real transfer is gated behind `local-flux` (it pulls in the flux crate's
/// `download` toolbox). With the feature off this reports an immediate failure so
/// the card never appears to hang.
#[cfg(feature = "local-flux")]
pub(crate) fn spawn_model_download(
    handle: &Handle,
    ctx: egui::Context,
    tx: Sender<ShellMsg>,
    model: String,
    cache_dir: Option<PathBuf>,
    cancel: CancellationToken,
) {
    use pixhaus_ai::backends::local_flux::ModelStore;

    handle.spawn(async move {
        let Some(store) = ModelStore::from_override(cache_dir) else {
            let _ = tx.send(ShellMsg::ModelDownloadFailed {
                model,
                error: "no cache directory could be resolved for this platform".to_owned(),
            });
            ctx.request_repaint();
            return;
        };

        // Read the optional HF token off-thread. A keychain read blocks, so it
        // runs on a blocking worker, never the async reactor.
        let token = tokio::task::spawn_blocking(crate::ai::hf_token).await.unwrap_or(None);

        // Progress closure: forward each cumulative tick as a ShellMsg and wake
        // the UI. Mirrors spawn_clip's progress shape.
        let progress_tx = tx.clone();
        let progress_ctx = ctx.clone();
        let progress_model = model.clone();
        let progress = move |bytes: u64, total: u64| {
            let fraction = (total > 0).then(|| {
                // total fits well within f32's exact-integer range for a ~16 GB
                // model, so the cast is lossless enough for a progress bar.
                #[allow(clippy::cast_precision_loss)]
                let f = bytes as f32 / total as f32;
                f.clamp(0.0, 1.0)
            });
            let _ = progress_tx.send(ShellMsg::ModelDownloadProgress {
                model: progress_model.clone(),
                fraction,
                bytes,
                total,
                message: "downloading weights".to_owned(),
            });
            progress_ctx.request_repaint();
        };

        let result = store.download(token.as_deref(), progress, &cancel).await;
        let msg = match result {
            Ok(()) => ShellMsg::ModelDownloadDone { model },
            Err(error) => ShellMsg::ModelDownloadFailed {
                model,
                error: error.to_string(),
            },
        };
        let _ = tx.send(msg);
        ctx.request_repaint();
    });
}

/// Spawns a startup presence probe off the UI thread: checks whether the model's
/// weights are present and complete on disk and reports the result over `tx`.
/// Mirrors `spawn_backend_key_op`'s blocking shape — a filesystem walk that must
/// not stall the first paint.
///
/// The real check is gated behind `local-flux` (it consults the flux crate's
/// `ModelStore`). With the feature off it reports "not downloaded", so the card
/// shows the disabled state.
#[cfg(feature = "local-flux")]
pub(crate) fn spawn_model_probe(handle: &Handle, ctx: egui::Context, tx: Sender<ShellMsg>, model: String, cache_dir: Option<PathBuf>) {
    use pixhaus_ai::backends::local_flux::ModelStore;

    handle.spawn_blocking(move || {
        let ready = ModelStore::from_override(cache_dir).is_some_and(|store| store.is_downloaded());
        let _ = tx.send(ShellMsg::LocalModelProbed { model, ready });
        ctx.request_repaint();
    });
}

/// Feature-off stub: the model is never present in a build without on-device
/// generation, so report "not downloaded".
#[cfg(not(feature = "local-flux"))]
pub(crate) fn spawn_model_probe(handle: &Handle, ctx: egui::Context, tx: Sender<ShellMsg>, model: String, _cache_dir: Option<PathBuf>) {
    handle.spawn_blocking(move || {
        let _ = tx.send(ShellMsg::LocalModelProbed { model, ready: false });
        ctx.request_repaint();
    });
}
