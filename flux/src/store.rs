//! Model presence and the pinned FLUX.2-klein-4B download surface.
//!
//! [`ModelStore`] owns the on-disk cache location and answers the registration
//! gate question: are the weights present and complete? The pinned file list,
//! sizes, and revision come from the live HF tree (see
//! `.tmp/lf-research/04-download.md`). With the `download` feature enabled it
//! also streams each file from the HF resolve URL, reporting cumulative byte
//! progress and honoring cooperative cancellation.

use std::path::{Path, PathBuf};

use directories::ProjectDirs;

/// HF repo id. The 4B weights are Apache-2.0 — safe to download and redistribute.
/// The download path hard-codes this id; the non-commercial 9B is never offered.
pub const FLUX2_KLEIN_REPO: &str = "black-forest-labs/FLUX.2-klein-4B";

/// Pinned commit sha so downloads are reproducible. HEAD of `main` at 2026-02-24.
pub const FLUX2_KLEIN_REVISION: &str = "e7b7dc27f91deacad38e78976d1f2b499d76a294";

/// Keychain id for the optional Hugging Face token. The shell stores the token
/// under `pixhaus.huggingface` via the existing `ApiKeyStore` — this crate owns
/// only the id so the keychain code is never duplicated. The token is optional:
/// klein-4B is a public repo, so a token only matters for rate limits or future
/// gated repos.
pub const HF_TOKEN_SERVICE_ID: &str = "huggingface";

/// One required file: repo-relative path plus its byte size for the progress
/// denominator and the truncated-download integrity check.
#[derive(Clone, Copy, Debug)]
pub struct RequiredFile {
    /// Repo-relative path, e.g. `transformer/diffusion_pytorch_model.safetensors`.
    pub path: &'static str,
    /// Exact byte size from the HF API. Zero is never used here.
    pub size: u64,
}

/// Every file the loader needs (diffusers component layout). The top-level
/// single-file checkpoint and the example images are intentionally excluded.
/// Sum of `size` is [`FLUX2_KLEIN_TOTAL_BYTES`] (~15.96 GB).
pub const FLUX2_KLEIN_FILES: &[RequiredFile] = &[
    RequiredFile {
        path: "model_index.json",
        size: 446,
    },
    RequiredFile {
        path: "transformer/config.json",
        size: 541,
    },
    RequiredFile {
        path: "transformer/diffusion_pytorch_model.safetensors",
        size: 7_751_109_744,
    },
    RequiredFile {
        path: "vae/config.json",
        size: 821,
    },
    RequiredFile {
        path: "vae/diffusion_pytorch_model.safetensors",
        size: 168_120_878,
    },
    RequiredFile {
        path: "text_encoder/config.json",
        size: 1_536,
    },
    RequiredFile {
        path: "text_encoder/generation_config.json",
        size: 214,
    },
    RequiredFile {
        path: "text_encoder/model.safetensors.index.json",
        size: 32_855,
    },
    RequiredFile {
        path: "text_encoder/model-00001-of-00002.safetensors",
        size: 4_967_215_360,
    },
    RequiredFile {
        path: "text_encoder/model-00002-of-00002.safetensors",
        size: 3_077_766_632,
    },
    RequiredFile {
        path: "tokenizer/tokenizer.json",
        size: 11_422_654,
    },
    RequiredFile {
        path: "tokenizer/tokenizer_config.json",
        size: 5_404,
    },
    RequiredFile {
        path: "tokenizer/vocab.json",
        size: 2_776_833,
    },
    RequiredFile {
        path: "tokenizer/merges.txt",
        size: 1_671_853,
    },
    RequiredFile {
        path: "tokenizer/added_tokens.json",
        size: 707,
    },
    RequiredFile {
        path: "tokenizer/special_tokens_map.json",
        size: 613,
    },
    RequiredFile {
        path: "tokenizer/chat_template.jinja",
        size: 4_168,
    },
    RequiredFile {
        path: "scheduler/scheduler_config.json",
        size: 486,
    },
];

/// Total bytes across every required file — the overall progress denominator.
///
/// This is the exact sum of the [`FLUX2_KLEIN_FILES`] sizes (~15.98 GB), held as
/// a constant so the download bar's denominator is a compile-time value.
/// `required_file_total_matches_constant` asserts it stays in sync. (The
/// research note's prose carried a smaller figure that did not equal the sum of
/// the per-file sizes it listed; the sum below is authoritative.)
pub const FLUX2_KLEIN_TOTAL_BYTES: u64 = 15_980_131_745;

/// The local model id this store manages, used as the cache subdirectory and the
/// settings model key.
pub const FLUX2_KLEIN_MODEL_ID: &str = "flux2-klein-4b";

/// Build the HF resolve URL for one repo-relative path at the pinned revision.
///
/// `resolve` 302-redirects LFS files to a CDN; reqwest follows redirects by
/// default, so a plain GET on this URL reaches the bytes. The path is passed
/// literally (slash separators, not percent-double-encoded).
#[must_use]
pub fn resolve_url(path: &str) -> String {
    format!("https://huggingface.co/{FLUX2_KLEIN_REPO}/resolve/{FLUX2_KLEIN_REVISION}/{path}")
}

/// Owns the cache directory and answers presence questions for the FLUX.2 weights.
///
/// The store does not load the model — it only locates files and reports whether
/// the download is complete. [`crate::loader::LoadedModel::load`] consumes the
/// resolved paths.
#[derive(Clone, Debug)]
pub struct ModelStore {
    /// Root directory holding the model's files, e.g.
    /// `<app-data>/models/flux2-klein-4b`.
    root: PathBuf,
}

impl ModelStore {
    /// Create a store rooted at an explicit directory (a persisted settings
    /// override). The directory need not exist yet.
    #[must_use]
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    /// Create a store rooted at the app-data default,
    /// `<data_dir>/models/flux2-klein-4b`.
    ///
    /// Returns `None` if no home/app-data directory can be determined for the
    /// platform — the caller then falls back to a settings override.
    #[must_use]
    pub fn app_data_default() -> Option<Self> {
        Some(Self::new(Self::default_root()?))
    }

    /// Resolve a store from an optional settings override.
    ///
    /// `Some(dir)` forces that cache directory; `None` uses the app-data
    /// default. Returns `None` only when the override is absent *and* no
    /// app-data directory can be determined for the platform.
    #[must_use]
    pub fn from_override(override_dir: Option<PathBuf>) -> Option<Self> {
        match override_dir {
            Some(dir) => Some(Self::new(dir)),
            None => Self::app_data_default(),
        }
    }

    /// The platform app-data cache root for this model,
    /// `<data_dir>/models/flux2-klein-4b`. `None` when no app-data dir exists.
    #[must_use]
    pub fn default_root() -> Option<PathBuf> {
        let dirs = ProjectDirs::from("app", "Pixhaus", "Pixhaus")?;
        Some(dirs.data_dir().join("models").join(FLUX2_KLEIN_MODEL_ID))
    }

    /// The cache root for this model.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Absolute path to one repo-relative file under the cache root.
    #[must_use]
    pub fn file_path(&self, repo_relative: &str) -> PathBuf {
        self.root.join(repo_relative)
    }

    /// The registration gate: true only when every required file exists and is
    /// non-empty on disk.
    ///
    /// Non-emptiness is the cheap presence check the gate needs; the stronger
    /// per-file size equality used during download (see [`ModelStore::download`])
    /// catches truncation.
    #[must_use]
    pub fn is_downloaded(&self) -> bool {
        FLUX2_KLEIN_FILES.iter().all(|required| {
            let path = self.file_path(required.path);
            std::fs::metadata(&path).is_ok_and(|meta| meta.is_file() && meta.len() > 0)
        })
    }
}

#[cfg(feature = "download")]
mod download;

#[cfg(feature = "download")]
pub use download::DownloadError;

#[cfg(test)]
mod tests {
    use super::*;

    use tempfile::TempDir;

    #[test]
    fn file_path_joins_under_root() {
        let store = ModelStore::new(PathBuf::from("/cache/flux"));
        assert_eq!(
            store.file_path("transformer/config.json"),
            PathBuf::from("/cache/flux").join("transformer/config.json")
        );
    }

    #[test]
    fn from_override_some_forces_dir() {
        let dir = PathBuf::from("/custom/models/flux2-klein-4b");
        let store = ModelStore::from_override(Some(dir.clone())).expect("explicit override never needs a platform app-data dir");
        assert_eq!(store.root(), dir.as_path());
    }

    #[test]
    fn default_root_ends_with_model_id() {
        // Skip on platforms with no resolvable app-data dir (CI containers can
        // lack one); the path shape is what we assert when it exists.
        if let Some(root) = ModelStore::default_root() {
            assert!(root.ends_with(FLUX2_KLEIN_MODEL_ID), "root was {root:?}");
            assert!(root.components().any(|c| c.as_os_str() == "models"), "root was {root:?}");
        }
    }

    #[test]
    fn resolve_url_pins_repo_and_revision() {
        let url = resolve_url("vae/config.json");
        assert_eq!(
            url,
            format!("https://huggingface.co/{FLUX2_KLEIN_REPO}/resolve/{FLUX2_KLEIN_REVISION}/vae/config.json")
        );
    }

    #[test]
    fn required_file_total_matches_constant() {
        let sum: u64 = FLUX2_KLEIN_FILES.iter().map(|f| f.size).sum();
        assert_eq!(sum, FLUX2_KLEIN_TOTAL_BYTES);
        assert!(FLUX2_KLEIN_FILES.iter().all(|f| f.size > 0));
    }

    #[test]
    fn is_downloaded_false_when_empty() {
        let dir = TempDir::new().expect("tempdir");
        let store = ModelStore::new(dir.path().to_path_buf());
        assert!(!store.is_downloaded());
    }

    #[test]
    fn is_downloaded_false_when_a_file_is_zero_length() {
        let dir = TempDir::new().expect("tempdir");
        let store = ModelStore::new(dir.path().to_path_buf());
        write_all_required(&store, /* zero_len_last = */ true);
        assert!(!store.is_downloaded(), "a zero-length required file must fail the gate");
    }

    #[test]
    fn is_downloaded_true_when_all_present_and_nonempty() {
        let dir = TempDir::new().expect("tempdir");
        let store = ModelStore::new(dir.path().to_path_buf());
        write_all_required(&store, /* zero_len_last = */ false);
        assert!(store.is_downloaded());
    }

    /// Write a one-byte stub for every required file under the store root. When
    /// `zero_len_last` is set the final file is left empty to exercise the
    /// non-empty gate.
    fn write_all_required(store: &ModelStore, zero_len_last: bool) {
        let last = FLUX2_KLEIN_FILES.len() - 1;
        for (idx, required) in FLUX2_KLEIN_FILES.iter().enumerate() {
            let path = store.file_path(required.path);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).expect("create parent dir");
            }
            let body: &[u8] = if zero_len_last && idx == last { b"" } else { b"x" };
            std::fs::write(&path, body).expect("write stub file");
        }
    }
}
