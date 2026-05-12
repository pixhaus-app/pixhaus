//! Sample-project discovery for the welcome screen.
//!
//! Lists the `.pixhaus` files Pixhaus ships in `examples/samples/` so the
//! welcome screen can offer "Open Sample" entries without going through the
//! file dialog. The path resolution differs by build:
//!
//! - **Production:** samples ship as bundled resources. Tauri's resource
//!   resolver finds them under the app's resource directory.
//! - **Development (`cargo tauri dev` / `pnpm dev`):** the resource bundle
//!   doesn't exist; resolve relative to `CARGO_MANIFEST_DIR` so the same
//!   list is available without packaging.
//!
//! The command never errors on a missing samples directory — it returns an
//! empty list, so a fresh checkout without the `examples/samples/` tree
//! still launches.

use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::error::CommandResult;

/// One sample project surfaced to the UI.
#[derive(Debug, Serialize)]
pub struct SampleEntry {
    /// Display name (the file stem, e.g. `"character-knight"`).
    pub name: String,
    /// Absolute filesystem path the UI can pass to `project_open`.
    pub path: String,
}

/// Lists every `.pixhaus` file in the bundled samples directory.
///
/// Returns an empty list when the directory does not exist (a fresh
/// checkout without `examples/samples/` should still launch). Hidden
/// files and non-`.pixhaus` entries are skipped silently.
#[tauri::command(async, rename_all = "snake_case")]
pub async fn list_samples(app: AppHandle) -> CommandResult<Vec<SampleEntry>> {
    let dir = resolve_samples_dir(&app);
    Ok(match dir {
        Some(d) => collect_samples(&d),
        None => Vec::new(),
    })
}

/// Resolves the samples directory across dev and production builds.
///
/// Production: ask Tauri for the bundled resource directory, then look
/// for `examples/samples/` inside it (the path layout used by the
/// `bundle.resources` glob in `tauri.conf.json`).
///
/// Development: fall back to `CARGO_MANIFEST_DIR/../examples/samples/`
/// so `pnpm dev` works without a packaged build. The fallback is keyed
/// on the resource lookup failing rather than on a debug-assertions
/// flag — `cargo tauri dev` builds the app crate with `release` flags
/// for end users testing release-mode behaviour, and the resource
/// directory still doesn't exist in that mode.
fn resolve_samples_dir(app: &AppHandle) -> Option<PathBuf> {
    if let Ok(resource_dir) = app.path().resource_dir() {
        let bundled = resource_dir.join("examples").join("samples");
        if bundled.exists() {
            return Some(bundled);
        }
    }
    let dev = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()?
        .join("examples")
        .join("samples");
    dev.exists().then_some(dev)
}

/// Walks the samples directory and returns one entry per `.pixhaus` file.
///
/// Sorted alphabetically by file stem so the welcome screen renders a
/// stable order across launches. IO errors on individual entries are
/// logged and skipped — a malformed entry shouldn't break the listing.
fn collect_samples(dir: &Path) -> Vec<SampleEntry> {
    let read_dir = match std::fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(err) => {
            tracing::warn!(dir = %dir.display(), error = %err, "list_samples: read_dir failed");
            return Vec::new();
        }
    };

    let mut entries: Vec<SampleEntry> = read_dir
        .filter_map(std::result::Result::ok)
        .filter_map(|de| {
            let path = de.path();
            if path.extension().and_then(|s| s.to_str()) != Some("pixhaus") {
                return None;
            }
            let name = path.file_stem().and_then(|s| s.to_str())?.to_owned();
            let path_str = path.to_str()?.to_owned();
            Some(SampleEntry {
                name,
                path: path_str,
            })
        })
        .collect();

    entries.sort_by(|a, b| a.name.cmp(&b.name));
    entries
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shipped samples directory should yield five `.pixhaus` files
    /// (knight, slime, forest tileset, ui sprites, forest level — see
    /// `examples/samples/README.md`). This is the same list the io
    /// crate's round-trip test relies on; a regression here means
    /// `collect_samples` is filtering wrong, not that the fixtures
    /// changed.
    #[test]
    fn collect_samples_finds_committed_pixhaus_files() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("app crate has parent")
            .join("examples")
            .join("samples");
        let entries = collect_samples(&dir);
        assert!(
            entries.len() >= 5,
            "expected at least the five committed sample fixtures, got {} entries",
            entries.len()
        );
        for e in &entries {
            assert!(
                e.path.ends_with(".pixhaus"),
                "every sample entry must point at a .pixhaus file (got {})",
                e.path,
            );
            assert!(
                !e.name.is_empty(),
                "every sample entry must carry a non-empty name"
            );
        }
        // Sorting check: file stems must be in ascending order.
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        let mut sorted = names.clone();
        sorted.sort_unstable();
        assert_eq!(
            names, sorted,
            "samples must be returned in alphabetical order"
        );
    }

    #[test]
    fn collect_samples_skips_non_pixhaus_files() {
        let tmp = tempfile::tempdir().expect("create temp dir");
        let tmp_path = tmp.path();
        std::fs::write(tmp_path.join("a.pixhaus"), b"x").expect("write a");
        std::fs::write(tmp_path.join("README.md"), b"x").expect("write readme");
        std::fs::write(tmp_path.join("b.aseprite"), b"x").expect("write b");

        let entries = collect_samples(tmp_path);

        assert_eq!(entries.len(), 1, "only the .pixhaus file should be listed");
        assert_eq!(entries[0].name, "a");
    }

    #[test]
    fn collect_samples_returns_empty_when_dir_missing() {
        // tempdir() gives a directory that exists; drop it before passing
        // the (now-removed) path to collect_samples to exercise the missing
        // branch without synthesising a path from env vars.
        let missing = {
            let tmp = tempfile::tempdir().expect("create temp dir");
            tmp.path().to_path_buf()
        };
        let entries = collect_samples(&missing);
        assert!(entries.is_empty());
    }
}
