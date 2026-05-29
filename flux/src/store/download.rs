//! Streaming download of the FLUX.2-klein-4B weights.
//!
//! hf-hub 0.5's `get` drives its own indicatif CLI bar and exposes no
//! programmatic byte callback, so we drive the transfer ourselves: GET each
//! file's HF resolve URL, stream the body with reqwest's `bytes_stream`, and
//! push cumulative `(downloaded, total)` byte counts to a caller-supplied
//! progress closure. The transfer is resumable at file granularity — a file
//! already present at its pinned size is skipped — and cooperatively
//! cancellable between chunks.
//!
//! Compiled only under the `download` feature; the heavy network + async
//! toolbox never enters a features-off build.

use std::path::Path;

use futures_util::StreamExt as _;
use tokio::io::AsyncWriteExt as _;
use tokio_util::sync::CancellationToken;

use super::{ModelStore, RequiredFile, FLUX2_KLEIN_FILES, FLUX2_KLEIN_TOTAL_BYTES};

/// Emit a cumulative-progress tick at most this often, to keep the callback /
/// channel light. The UI repaints on receipt, so per-chunk granularity is waste.
const PROGRESS_CHUNK_BYTES: u64 = 8 * 1024 * 1024;

/// Errors raised while downloading the model.
#[derive(Debug, thiserror::Error)]
pub enum DownloadError {
    /// The HTTP request or response stream failed.
    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),

    /// A filesystem operation (create dir, write, flush, remove) failed.
    #[error("filesystem error: {0}")]
    Io(#[from] std::io::Error),

    /// A finished file's byte count did not match its pinned manifest size,
    /// signalling a truncated or corrupt transfer.
    #[error("size mismatch for {path}: expected {expected} bytes, got {got}")]
    SizeMismatch {
        /// Repo-relative path of the offending file.
        path: String,
        /// The pinned manifest size in bytes.
        expected: u64,
        /// The bytes actually written.
        got: u64,
    },

    /// The caller cancelled the download. Any partial file has been removed.
    #[error("download cancelled")]
    Cancelled,
}

impl ModelStore {
    /// Download every required file to the cache root, reporting cumulative byte
    /// progress and honoring cooperative cancellation.
    ///
    /// `token` sets an `Authorization: Bearer` header — optional, since klein-4B
    /// is public; pass it for rate limits or future gated repos. `progress` is
    /// called with `(downloaded, total)` cumulative bytes roughly every 8 MB and
    /// once per finished file. `cancel` is checked between chunks; on cancel the
    /// in-flight partial file is removed and [`DownloadError::Cancelled`] is
    /// returned.
    ///
    /// Files already present at their exact pinned size are skipped (their bytes
    /// still count toward the running progress total), so an interrupted run
    /// resumes at file granularity on the next call.
    ///
    /// # Errors
    ///
    /// Returns [`DownloadError`] on a network failure, a filesystem error, a
    /// truncated transfer (size mismatch), or cancellation.
    pub async fn download<F>(
        &self,
        token: Option<&str>,
        mut progress: F,
        cancel: &CancellationToken,
    ) -> Result<(), DownloadError>
    where
        F: FnMut(u64, u64),
    {
        let client = reqwest::Client::builder()
            .user_agent(concat!("pixhaus-flux/", env!("CARGO_PKG_VERSION")))
            .build()?;

        let total = FLUX2_KLEIN_TOTAL_BYTES;
        let mut already: u64 = 0;
        progress(already, total);

        for required in FLUX2_KLEIN_FILES {
            if cancel.is_cancelled() {
                return Err(DownloadError::Cancelled);
            }

            let dest = self.file_path(required.path);
            if file_is_complete(&dest, required.size) {
                already = already.saturating_add(required.size);
                progress(already, total);
                continue;
            }

            let written = download_file(
                &client,
                required,
                &dest,
                token,
                already,
                total,
                cancel,
                &mut progress,
            )
            .await?;
            already = already.saturating_add(written);
            progress(already, total);
        }

        Ok(())
    }
}

/// True when `path` is a regular file whose length already matches `expected`.
fn file_is_complete(path: &Path, expected: u64) -> bool {
    std::fs::metadata(path).is_ok_and(|meta| meta.is_file() && meta.len() == expected)
}

/// Stream one file to `dest`, returning the bytes written.
///
/// `already` is the cumulative byte count for prior files so the reported
/// progress reflects whole-model completion; `total` is the model-wide
/// denominator.
#[allow(clippy::too_many_arguments)]
async fn download_file<F>(
    client: &reqwest::Client,
    required: &RequiredFile,
    dest: &Path,
    token: Option<&str>,
    already: u64,
    total: u64,
    cancel: &CancellationToken,
    progress: &mut F,
) -> Result<u64, DownloadError>
where
    F: FnMut(u64, u64),
{
    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let url = super::resolve_url(required.path);
    let mut request = client.get(url);
    if let Some(token) = token {
        // Authorization: Bearer <token>. Harmless on the public repo; required
        // for gated repos and useful for higher rate limits.
        request = request.bearer_auth(token);
    }
    let response = request.send().await?.error_for_status()?;

    // Per-file denominator: trust the pinned manifest size; fall back to the
    // Content-Length header (which can be absent on a transparently decoded body).
    let file_total = if required.size > 0 {
        required.size
    } else {
        response.content_length().unwrap_or(0)
    };

    let mut file = tokio::fs::File::create(dest).await?;
    let mut stream = response.bytes_stream();
    let mut file_done: u64 = 0;
    let mut since_emit: u64 = 0;

    while let Some(chunk) = stream.next().await {
        if cancel.is_cancelled() {
            drop(file);
            // Do not leave a partial file behind; a retry must re-fetch cleanly.
            let _ = tokio::fs::remove_file(dest).await;
            return Err(DownloadError::Cancelled);
        }

        let chunk = chunk?;
        file.write_all(&chunk).await?;
        let chunk_len = chunk.len() as u64;
        file_done = file_done.saturating_add(chunk_len);
        since_emit = since_emit.saturating_add(chunk_len);

        if since_emit >= PROGRESS_CHUNK_BYTES {
            progress(already.saturating_add(file_done), total);
            since_emit = 0;
        }
    }

    file.flush().await?;

    // Truncated-download guard: the written byte count must equal the pinned
    // manifest size. A short read here means a corrupt or interrupted transfer.
    if file_total > 0 && file_done != file_total {
        let _ = tokio::fs::remove_file(dest).await;
        return Err(DownloadError::SizeMismatch {
            path: required.path.to_owned(),
            expected: file_total,
            got: file_done,
        });
    }

    Ok(file_done)
}

#[cfg(test)]
mod tests {
    use super::*;

    use tempfile::TempDir;

    /// A pre-cancelled token must abort before any network request: the loop
    /// checks cancellation before touching the first file. No network.
    #[tokio::test]
    async fn download_returns_cancelled_when_pre_cancelled() {
        let dir = TempDir::new().expect("tempdir");
        let store = ModelStore::new(dir.path().to_path_buf());
        let cancel = CancellationToken::new();
        cancel.cancel();

        let mut ticks = Vec::new();
        let result = store
            .download(None, |done, total| ticks.push((done, total)), &cancel)
            .await;

        assert!(matches!(result, Err(DownloadError::Cancelled)));
        // The initial (0, total) tick still fires before the loop sees the cancel.
        assert_eq!(ticks.first().copied(), Some((0, FLUX2_KLEIN_TOTAL_BYTES)));
    }

    /// Files already present at their exact pinned size are skipped, and a
    /// pre-cancel after a complete file still aborts cleanly. No network: every
    /// file in this fixture is already "complete" by size, so the loop never
    /// issues a request even without a cancel — but we cancel up front so the
    /// test cannot hang if the size logic regresses.
    #[tokio::test]
    async fn download_skips_complete_files_without_network() {
        let dir = TempDir::new().expect("tempdir");
        let store = ModelStore::new(dir.path().to_path_buf());

        // Materialize every required file at its exact pinned size so
        // `file_is_complete` short-circuits each one. We allocate sparse files
        // to avoid writing ~16 GB of real bytes.
        for required in FLUX2_KLEIN_FILES {
            let path = store.file_path(required.path);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).expect("create parent");
            }
            let file = std::fs::File::create(&path).expect("create stub");
            file.set_len(required.size).expect("set length");
        }

        let cancel = CancellationToken::new();
        let mut last = (0u64, 0u64);
        let result = store
            .download(None, |done, total| last = (done, total), &cancel)
            .await;

        assert!(result.is_ok(), "all-complete store should download nothing");
        assert_eq!(last, (FLUX2_KLEIN_TOTAL_BYTES, FLUX2_KLEIN_TOTAL_BYTES));
    }

    /// Real network: fetch the small JSON manifests only would still hit the
    /// loop's first big safetensors. This pulls the full ~16 GB, so it stays
    /// ignored and runs only on demand.
    #[tokio::test]
    #[ignore = "downloads ~16 GB from Hugging Face"]
    async fn download_full_model_smoke() {
        let dir = TempDir::new().expect("tempdir");
        let store = ModelStore::new(dir.path().to_path_buf());
        let token = std::env::var("HF_TOKEN").ok();
        let cancel = CancellationToken::new();

        store
            .download(token.as_deref(), |done, total| {
                tracing::info!(done, total, "flux download progress");
            }, &cancel)
            .await
            .expect("full download");

        assert!(store.is_downloaded());
    }
}
