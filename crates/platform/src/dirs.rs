//! Platform-standard app directories.
//!
//! `platform` owns where Pixhaus puts files on each OS. [`app_dirs`] resolves the
//! standard config/data/cache locations via the `directories` crate (XDG on Linux,
//! the Known Folder API on Windows, the Apple guidelines on macOS) and derives the
//! log and autosave leaves under them. [`log_dir`] additionally creates the log
//! directory, because `directories` only computes paths — it never touches disk.
//!
//! The subscriber that writes into the log dir lives in `app` (`src/diagnostics.rs`);
//! this crate only resolves the path so `app` and `services` agree on it.

use std::path::{Path, PathBuf};

use directories::ProjectDirs;
use thiserror::Error;

/// The `directories` identity triple `(qualifier, organization, application)`.
///
/// Locked: this is what derives every Pixhaus app directory, so changing it later
/// relocates config, data, cache, logs, and autosaves. The empty qualifier is right
/// for an open-source app with no reverse-domain bundle id; it only affects the macOS
/// bundle name. See the `pixhaus-directories` skill.
const PROJECT_DIRS: (&str, &str, &str) = ("", "Pixhaus", "Pixhaus");

/// An error resolving or creating a Pixhaus app directory.
#[derive(Debug, Error)]
pub enum DirsError {
    /// No home directory could be determined for the current user.
    ///
    /// `directories` returns `None` only in a degenerate environment (no `$HOME`,
    /// no Known Folder). There is no path to fall back to, so resolution fails.
    #[error("no home directory could be determined for this user")]
    NoHome,

    /// Creating an app directory on disk failed.
    ///
    /// `what` names the directory (e.g. `"log"`), `path` is where the create was
    /// attempted, and the source is the underlying I/O error (unwritable disk,
    /// permission denied).
    #[error("could not create the {what} directory at {path}")]
    Create {
        /// Which app directory was being created.
        what: &'static str,
        /// The path the create was attempted at.
        path: PathBuf,
        /// The underlying filesystem error.
        #[source]
        io: std::io::Error,
    },
}

/// The set of platform-standard directories Pixhaus uses.
///
/// Computed by [`app_dirs`]; the paths may not yet exist on disk (`directories`
/// never creates them). `config`/`data`/`cache` are the OS-standard locations;
/// `log` and `autosave` are leaf subdirectories under the local data dir so they
/// stay off Windows roaming and survive the macOS config==data merge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppDirs {
    /// User preferences, keybindings, recent files, workspace layouts.
    pub config: PathBuf,
    /// User-created persistent assets: palettes, brushes, templates, module data.
    pub data: PathBuf,
    /// Regenerable data: thumbnails, decoded asset and AI result caches.
    pub cache: PathBuf,
    /// Rolling log files and diagnostic traces (a leaf under the local data dir).
    pub log: PathBuf,
    /// Project recovery snapshots and crash-recovery metadata (a leaf under data).
    pub autosave: PathBuf,
}

/// Resolve the Pixhaus app directories for the current OS.
///
/// Computes paths only — none of the returned directories is guaranteed to exist
/// on disk; create the one you write to first (see [`log_dir`]). `log` and
/// `autosave` are derived as leaf subdirectories of the local data directory so
/// they never roam on Windows and don't collide on macOS, where config and data
/// resolve to the same folder.
///
/// # Errors
///
/// Returns [`DirsError::NoHome`] when no home directory can be determined.
pub fn app_dirs() -> Result<AppDirs, DirsError> {
    let (qualifier, organization, application) = PROJECT_DIRS;
    let dirs = ProjectDirs::from(qualifier, organization, application).ok_or(DirsError::NoHome)?;

    let data_local = dirs.data_local_dir();
    Ok(AppDirs {
        config: dirs.config_dir().to_path_buf(),
        data: data_local.to_path_buf(),
        cache: dirs.cache_dir().to_path_buf(),
        // Leaf subdirs under local data: off Windows roaming, distinct names so the
        // macOS config==data merge can't clobber them.
        log: data_local.join("logs"),
        autosave: data_local.join("autosave"),
    })
}

/// Resolve the log directory and create it on disk.
///
/// The rolling file appender in `app` needs the directory to exist before it opens
/// a file, and `directories` never creates anything, so this calls `create_dir_all`
/// (idempotent). Returns the path once it is present.
///
/// # Errors
///
/// Returns [`DirsError::NoHome`] when no home directory can be determined, or
/// [`DirsError::Create`] when the directory cannot be created.
#[tracing::instrument(level = "debug")]
pub fn log_dir() -> Result<PathBuf, DirsError> {
    let log = app_dirs()?.log;
    create(&log, "log")?;
    Ok(log)
}

/// Create a directory and all its parents, mapping the I/O error to [`DirsError`].
#[tracing::instrument(level = "debug")]
fn create(path: &Path, what: &'static str) -> Result<(), DirsError> {
    std::fs::create_dir_all(path).map_err(|io| DirsError::Create {
        what,
        path: path.to_path_buf(),
        io,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_dirs_resolves_and_log_is_a_logs_leaf() {
        let dirs = match app_dirs() {
            Ok(dirs) => dirs,
            Err(err) => panic!("app_dirs should resolve on the test host: {err}"),
        };
        // Don't assert absolute paths — they vary per machine. Assert the leaf join
        // and that the log leaf sits under the local data dir.
        assert_eq!(dirs.log.file_name(), Some(std::ffi::OsStr::new("logs")));
        assert_eq!(dirs.autosave.file_name(), Some(std::ffi::OsStr::new("autosave")));
        assert_eq!(dirs.log.parent(), Some(dirs.data.as_path()));
        assert_eq!(dirs.autosave.parent(), Some(dirs.data.as_path()));
    }

    #[test]
    fn log_dir_returns_an_existing_directory() {
        let path = match log_dir() {
            Ok(path) => path,
            Err(err) => panic!("log_dir should resolve and create on the test host: {err}"),
        };
        assert!(path.is_dir(), "log_dir must exist on disk after the call: {}", path.display());
        assert_eq!(path.file_name(), Some(std::ffi::OsStr::new("logs")));
    }

    #[test]
    fn create_reports_what_and_path_on_failure() {
        // Creating a directory whose parent is an existing file must fail; assert the
        // error carries the directory name and path rather than panicking.
        let tmp = match tempfile::tempdir() {
            Ok(tmp) => tmp,
            Err(err) => panic!("tempdir: {err}"),
        };
        let file = tmp.path().join("not-a-dir");
        if let Err(err) = std::fs::write(&file, b"x") {
            panic!("seed file: {err}");
        }
        let target = file.join("child");
        match create(&target, "log") {
            Err(DirsError::Create { what, path, .. }) => {
                assert_eq!(what, "log");
                assert_eq!(path, target);
            }
            Err(other) => panic!("expected a Create error, got {other:?}"),
            Ok(()) => panic!("creating a dir under a file should fail"),
        }
    }
}
