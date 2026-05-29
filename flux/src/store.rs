//! Model presence and the pinned FLUX.2-klein-4B download surface.
//!
//! [`ModelStore`] owns the on-disk cache location and answers the registration
//! gate question: are the weights present and complete? The pinned file list,
//! sizes, and revision come from the live HF tree (see
//! `.tmp/lf-research/04-download.md`); the actual streaming download lands in a
//! later phase.

use std::path::{Path, PathBuf};

use directories::ProjectDirs;

/// HF repo id. The 4B weights are Apache-2.0 — safe to download and redistribute.
/// The download path hard-codes this id; the non-commercial 9B is never offered.
pub const FLUX2_KLEIN_REPO: &str = "black-forest-labs/FLUX.2-klein-4B";

/// Pinned commit sha so downloads are reproducible. HEAD of `main` at 2026-02-24.
pub const FLUX2_KLEIN_REVISION: &str = "e7b7dc27f91deacad38e78976d1f2b499d76a294";

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
    RequiredFile { path: "model_index.json", size: 446 },
    RequiredFile { path: "transformer/config.json", size: 541 },
    RequiredFile { path: "transformer/diffusion_pytorch_model.safetensors", size: 7_751_109_744 },
    RequiredFile { path: "vae/config.json", size: 821 },
    RequiredFile { path: "vae/diffusion_pytorch_model.safetensors", size: 168_120_878 },
    RequiredFile { path: "text_encoder/config.json", size: 1_536 },
    RequiredFile { path: "text_encoder/generation_config.json", size: 214 },
    RequiredFile { path: "text_encoder/model.safetensors.index.json", size: 32_855 },
    RequiredFile { path: "text_encoder/model-00001-of-00002.safetensors", size: 4_967_215_360 },
    RequiredFile { path: "text_encoder/model-00002-of-00002.safetensors", size: 3_077_766_632 },
    RequiredFile { path: "tokenizer/tokenizer.json", size: 11_422_654 },
    RequiredFile { path: "tokenizer/tokenizer_config.json", size: 5_404 },
    RequiredFile { path: "tokenizer/vocab.json", size: 2_776_833 },
    RequiredFile { path: "tokenizer/merges.txt", size: 1_671_853 },
    RequiredFile { path: "tokenizer/added_tokens.json", size: 707 },
    RequiredFile { path: "tokenizer/special_tokens_map.json", size: 613 },
    RequiredFile { path: "tokenizer/chat_template.jinja", size: 4_168 },
    RequiredFile { path: "scheduler/scheduler_config.json", size: 486 },
];

/// Total bytes across every required file. The overall progress denominator.
pub const FLUX2_KLEIN_TOTAL_BYTES: u64 = 15_964_309_209;

/// The local model id this store manages, used as the cache subdirectory and the
/// settings model key.
pub const FLUX2_KLEIN_MODEL_ID: &str = "flux2-klein-4b";

/// Build the HF resolve URL for one repo-relative path at the pinned revision.
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
        let dirs = ProjectDirs::from("app", "Pixhaus", "Pixhaus")?;
        let root = dirs.data_dir().join("models").join(FLUX2_KLEIN_MODEL_ID);
        Some(Self::new(root))
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

    /// The registration gate: true only when every required file exists and its
    /// on-disk byte length matches the pinned manifest size (catching truncation).
    #[must_use]
    pub fn is_downloaded(&self) -> bool {
        FLUX2_KLEIN_FILES.iter().all(|required| {
            let path = self.file_path(required.path);
            std::fs::metadata(&path).is_ok_and(|meta| meta.len() == required.size)
        })
    }
}
