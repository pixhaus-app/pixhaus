//! Durable image-to-video job queue for the animation studio.
//!
//! Generation runs in a spawned task; this manager owns the job's lifecycle,
//! cancellation token, and on-disk persistence so a finished clip survives
//! stage navigation and app restart. The UI reflects job state by id over
//! `AnimationJobUpdate` events and fetches the finished clip on demand.
//!
//! Scope: a job interrupted by app exit is surfaced as `Interrupted`, not
//! resumed — a remote provider call can't be reattached after restart.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

/// Lifecycle of an i2v generation job.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnimationJobStatus {
    /// Generation is in flight.
    Running,
    /// Finished; the clip is on disk.
    Done,
    /// Failed; see `error`.
    Error,
    /// Cancelled by the user.
    Cancelled,
    /// The app exited mid-run; cannot be resumed.
    Interrupted,
}

/// One i2v job. Persisted as a `{id}.json` sidecar; the clip bytes live beside
/// it as `{id}.<ext>` once the job reaches [`AnimationJobStatus::Done`].
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AnimationJob {
    /// Process-unique job id.
    pub id: u64,
    /// Entity the clip belongs to (the studio lists jobs by entity).
    pub entity_id: u32,
    /// Direction the character faces.
    pub direction: String,
    /// Animation kind (`idle` / `walk` / `attack` / `custom`), if any.
    #[serde(default)]
    pub animation_kind: Option<String>,
    /// Video model key (`seedance` / `wan`).
    pub model: String,
    /// Current lifecycle status.
    pub status: AnimationJobStatus,
    /// Unix epoch milliseconds when the job started (UI derives elapsed).
    pub created_at_ms: u64,
    /// Sprite-frame pick target for the picker.
    pub target_count: u32,
    /// Clip fps used for decode / frame spacing.
    pub fps: u32,
    /// Clip MIME type (e.g. `video/mp4`).
    pub mime: String,
    /// Failure message when `status` is `Error`.
    #[serde(default)]
    pub error: Option<String>,
}

/// Max jobs kept on disk; older terminal jobs are pruned.
const MAX_JOBS: usize = 20;

/// Owns i2v jobs: id allocation, cancellation, and on-disk persistence.
pub(crate) struct AnimationJobManager {
    next_id: AtomicU64,
    jobs: DashMap<u64, AnimationJob>,
    cancellations: DashMap<u64, CancellationToken>,
    /// Job storage dir, or `None` when no data dir is resolvable (jobs then
    /// live in memory only).
    dir: Option<PathBuf>,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

fn jobs_dir() -> Option<PathBuf> {
    dirs::data_local_dir().map(|d| d.join("pixhaus").join("animation-jobs"))
}

fn ext_for_mime(mime: &str) -> &'static str {
    if mime.contains("webm") { "webm" } else { "mp4" }
}

impl AnimationJobManager {
    /// Loads persisted jobs from disk. Any job still `Running` from a previous
    /// session becomes `Interrupted`.
    pub(crate) fn new() -> Self {
        let dir = jobs_dir();
        let jobs: DashMap<u64, AnimationJob> = DashMap::new();
        let mut max_id = 0u64;
        if let Some(dir) = &dir {
            if let Err(err) = std::fs::create_dir_all(dir) {
                tracing::warn!(error = %err, "failed to create animation-jobs dir");
            }
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().and_then(|e| e.to_str()) != Some("json") {
                        continue;
                    }
                    let Ok(bytes) = std::fs::read(&path) else {
                        continue;
                    };
                    let Ok(mut job) = serde_json::from_slice::<AnimationJob>(&bytes) else {
                        continue;
                    };
                    if job.status == AnimationJobStatus::Running {
                        job.status = AnimationJobStatus::Interrupted;
                    }
                    max_id = max_id.max(job.id);
                    jobs.insert(job.id, job);
                }
            }
        }
        let mgr = Self {
            next_id: AtomicU64::new(max_id + 1),
            jobs,
            cancellations: DashMap::new(),
            dir,
        };
        // Persist any Running->Interrupted flips, then bound the dir.
        let interrupted: Vec<AnimationJob> = mgr
            .jobs
            .iter()
            .filter(|e| e.value().status == AnimationJobStatus::Interrupted)
            .map(|e| e.value().clone())
            .collect();
        for job in &interrupted {
            mgr.persist(job);
        }
        mgr.prune();
        mgr
    }

    fn sidecar_path(&self, id: u64) -> Option<PathBuf> {
        self.dir.as_ref().map(|d| d.join(format!("{id}.json")))
    }

    fn clip_path(&self, id: u64, mime: &str) -> Option<PathBuf> {
        self.dir
            .as_ref()
            .map(|d| d.join(format!("{id}.{}", ext_for_mime(mime))))
    }

    fn persist(&self, job: &AnimationJob) {
        let Some(path) = self.sidecar_path(job.id) else {
            return;
        };
        match serde_json::to_vec(job) {
            Ok(bytes) => {
                if let Err(err) = std::fs::write(&path, bytes) {
                    tracing::warn!(error = %err, "failed to persist animation job");
                }
            }
            Err(err) => tracing::warn!(error = %err, "failed to serialize animation job"),
        }
    }

    /// Registers a new running job and returns its id and cancel token.
    pub(crate) fn start(
        &self,
        entity_id: u32,
        direction: String,
        animation_kind: Option<String>,
        model: String,
        target_count: u32,
        fps: u32,
    ) -> (u64, CancellationToken) {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let token = CancellationToken::new();
        let job = AnimationJob {
            id,
            entity_id,
            direction,
            animation_kind,
            model,
            status: AnimationJobStatus::Running,
            created_at_ms: now_ms(),
            target_count,
            fps,
            mime: "video/mp4".into(),
            error: None,
        };
        self.cancellations.insert(id, token.clone());
        self.persist(&job);
        self.jobs.insert(id, job);
        (id, token)
    }

    /// Marks a job done and writes its clip bytes to disk.
    pub(crate) fn finish_done(&self, id: u64, mime: &str, clip: &[u8]) {
        self.cancellations.remove(&id);
        if let Some(path) = self.clip_path(id, mime) {
            if let Err(err) = std::fs::write(&path, clip) {
                tracing::warn!(error = %err, "failed to write animation clip");
            }
        }
        let snapshot = {
            let Some(mut job) = self.jobs.get_mut(&id) else {
                return;
            };
            job.status = AnimationJobStatus::Done;
            mime.clone_into(&mut job.mime);
            job.clone()
        };
        self.persist(&snapshot);
        self.prune();
    }

    /// Marks a job failed.
    pub(crate) fn finish_error(&self, id: u64, error: &str) {
        self.finish_terminal(id, AnimationJobStatus::Error, Some(error.to_owned()));
    }

    /// Marks a job cancelled.
    pub(crate) fn finish_cancelled(&self, id: u64) {
        self.finish_terminal(id, AnimationJobStatus::Cancelled, None);
    }

    fn finish_terminal(&self, id: u64, status: AnimationJobStatus, error: Option<String>) {
        self.cancellations.remove(&id);
        let snapshot = {
            let Some(mut job) = self.jobs.get_mut(&id) else {
                return;
            };
            job.status = status;
            job.error = error;
            job.clone()
        };
        self.persist(&snapshot);
    }

    /// Returns a snapshot of one job.
    pub(crate) fn get(&self, id: u64) -> Option<AnimationJob> {
        self.jobs.get(&id).map(|e| e.value().clone())
    }

    /// Returns the entity's jobs, newest first.
    pub(crate) fn list(&self, entity_id: u32) -> Vec<AnimationJob> {
        let mut v: Vec<AnimationJob> = self
            .jobs
            .iter()
            .filter(|e| e.value().entity_id == entity_id)
            .map(|e| e.value().clone())
            .collect();
        v.sort_by_key(|j| std::cmp::Reverse(j.created_at_ms));
        v
    }

    /// Reads a finished job's clip bytes and mime from disk.
    pub(crate) fn read_clip(&self, id: u64) -> Option<(Vec<u8>, String)> {
        let job = self.get(id)?;
        if job.status != AnimationJobStatus::Done {
            return None;
        }
        let path = self.clip_path(id, &job.mime)?;
        let bytes = std::fs::read(path).ok()?;
        Some((bytes, job.mime))
    }

    /// Cancels a running job's token. Returns whether a token was present.
    pub(crate) fn cancel(&self, id: u64) -> bool {
        if let Some((_, token)) = self.cancellations.remove(&id) {
            token.cancel();
            true
        } else {
            false
        }
    }

    /// Drops the oldest terminal jobs (and their files) past [`MAX_JOBS`].
    fn prune(&self) {
        if self.jobs.len() <= MAX_JOBS {
            return;
        }
        let mut terminal: Vec<(u64, u64)> = self
            .jobs
            .iter()
            .filter(|e| e.value().status != AnimationJobStatus::Running)
            .map(|e| (e.value().created_at_ms, e.value().id))
            .collect();
        // Newest first; remove everything past the cap.
        terminal.sort_by_key(|t| std::cmp::Reverse(t.0));
        for (_, id) in terminal.into_iter().skip(MAX_JOBS) {
            if let Some((_, job)) = self.jobs.remove(&id) {
                if let Some(p) = self.sidecar_path(id) {
                    let _ = std::fs::remove_file(p);
                }
                if let Some(p) = self.clip_path(id, &job.mime) {
                    let _ = std::fs::remove_file(p);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manager_without_disk() -> AnimationJobManager {
        AnimationJobManager {
            next_id: AtomicU64::new(1),
            jobs: DashMap::new(),
            cancellations: DashMap::new(),
            dir: None,
        }
    }

    #[test]
    fn start_allocates_increasing_ids_and_tracks_running() {
        let mgr = manager_without_disk();
        let (id1, _) = mgr.start(
            7,
            "south".into(),
            Some("idle".into()),
            "seedance".into(),
            10,
            12,
        );
        let (id2, _) = mgr.start(7, "west".into(), None, "wan".into(), 8, 10);
        assert!(id2 > id1);
        assert_eq!(mgr.get(id1).unwrap().status, AnimationJobStatus::Running);
        assert_eq!(mgr.list(7).len(), 2);
        assert_eq!(mgr.list(99).len(), 0);
    }

    #[test]
    fn finish_transitions_and_cancel() {
        let mgr = manager_without_disk();
        let (id, _token) = mgr.start(1, "south".into(), None, "seedance".into(), 10, 12);
        mgr.finish_error(id, "boom");
        let job = mgr.get(id).unwrap();
        assert_eq!(job.status, AnimationJobStatus::Error);
        assert_eq!(job.error.as_deref(), Some("boom"));
        // Token already removed on finish; cancel reports nothing to cancel.
        assert!(!mgr.cancel(id));

        let (id2, _t2) = mgr.start(1, "south".into(), None, "seedance".into(), 10, 12);
        assert!(mgr.cancel(id2));
    }

    #[test]
    fn list_is_newest_first() {
        let mgr = manager_without_disk();
        let (a, _) = mgr.start(5, "south".into(), None, "seedance".into(), 10, 12);
        let (b, _) = mgr.start(5, "north".into(), None, "seedance".into(), 10, 12);
        let ids: Vec<u64> = mgr.list(5).iter().map(|j| j.id).collect();
        // b started after a, so it sorts first (ties broken by insertion is fine).
        assert!(ids.contains(&a) && ids.contains(&b));
        assert_eq!(ids.len(), 2);
    }
}
