//! Durable image-to-video job queue for the animation studio.
//!
//! Clip generation runs in a spawned task; this queue owns the job's lifecycle,
//! its cancellation token, and on-disk persistence so a finished clip survives
//! stage navigation and app restart. The studio reflects a job by id and fetches
//! the finished clip on demand.
//!
//! Single owner: the queue lives on `ShellApp` and is mutated through
//! `&mut self`. There is no `Arc`, `Mutex`, or `dashmap` — only the
//! `CancellationToken` clones cross to tokio tasks. This is the v2 counterpart
//! of the Tauri `app/src/animation_jobs.rs`, which used `dashmap` for
//! IPC-shared state v2 does not need.
//!
//! Scope: a job interrupted by app exit is surfaced as `Interrupted`, not
//! resumed — a remote provider call cannot be reattached after restart.
//!
//! Persistence is an app-data sidecar under `eframe::storage_dir("pixhaus")`,
//! not the project archive. When the io crate lands and `.pixhaus` gains a real
//! save/open, these records move into the project; the serialization shape here
//! is designed so that is a type-move, not a redesign. A missing or unwritable
//! storage dir degrades the queue to in-memory only and never crashes the UI.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use pixhaus_core::project::{EntityId, SpriteId};
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

/// Lifecycle of an i2v generation job.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AnimJobStatus {
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

/// The persistable facts about one i2v run. Persisted as a `{id}.json` sidecar;
/// the clip bytes live beside it as `{id}.<ext>` once the job reaches
/// [`AnimJobStatus::Done`].
///
/// Optional fields carry `#[serde(default)]` so an older sidecar (one written
/// before a field existed) still deserializes — forward-compat matters more than
/// strictness for app-data sidecars.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct AnimJobRecord {
    /// Process-unique job id.
    pub id: u64,
    /// Entity the clip belongs to (the studio lists jobs by entity).
    pub entity_id: EntityId,
    /// Sprite the clip integrates onto.
    pub sprite_id: SpriteId,
    /// Direction the character faces (e.g. `south`).
    pub direction: String,
    /// Animation kind (`idle` / `walk` / `attack` / `custom`), if any.
    #[serde(default)]
    pub animation_kind: Option<String>,
    /// Image-to-video model key (e.g. a FAL endpoint id).
    pub model: String,
    /// Motion prompt this run was generated from, for provenance and re-run.
    #[serde(default)]
    pub motion: String,
    /// Current lifecycle status.
    pub status: AnimJobStatus,
    /// Unix epoch milliseconds when the job started (the UI derives elapsed).
    pub created_at_ms: u64,
    /// Sprite-frame pick target for the picker.
    pub target_count: u32,
    /// Clip fps used for decode / frame spacing.
    pub fps: u32,
    /// Clip MIME type (e.g. `video/mp4`).
    pub mime: String,
    /// RNG seed the run pinned, for a reproducible re-run.
    #[serde(default)]
    pub seed: Option<u64>,
    /// Failure message when `status` is `Error`.
    #[serde(default)]
    pub error: Option<String>,
    /// Per-frame QC and provenance, set at Land by [`AnimJobQueue::finalize_qc`].
    /// Finalized after `finish_done` (decode-time is too early — normalize has
    /// not run yet). The same record rides on the landed `Animation`; this
    /// sidecar copy is the interim durable-across-restart home until the io crate
    /// lands. Serde-defaulted so an older sidecar without it still loads.
    #[serde(default)]
    pub qc: Option<pixhaus_core::project::AnimationQc>,
}

/// Max jobs kept in the queue; older terminal jobs (and their files) are pruned.
const MAX_JOBS: usize = 20;

/// Owns i2v jobs: id allocation, cancellation, and on-disk persistence. Mutated
/// through `&mut self` on the UI thread.
pub(crate) struct AnimJobQueue {
    /// Next id to allocate. Monotonic; never reused.
    next_id: u64,
    /// All known jobs by id.
    jobs: HashMap<u64, AnimJobRecord>,
    /// Cancel tokens for in-flight jobs. Removed on any terminal transition.
    cancellations: HashMap<u64, CancellationToken>,
    /// Job storage dir, or `None` when no data dir is resolvable (jobs then live
    /// in memory only — the in-memory-durable floor).
    dir: Option<PathBuf>,
}

/// Unix epoch milliseconds, or `0` if the clock is before the epoch.
fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

/// The animation-jobs sidecar dir under eframe's app-data storage, matching
/// `gpu.rs::pref_path`. `None` when no storage dir is resolvable.
fn jobs_dir() -> Option<PathBuf> {
    eframe::storage_dir("pixhaus").map(|d| d.join("animation-jobs"))
}

/// File extension for the clip blob, by MIME. v2 decodes gif/apng/png and
/// mp4/quicktime; everything else falls back to `mp4` since the decode path
/// treats unknown MIME as an error before a blob is ever written.
fn ext_for_mime(mime: &str) -> &'static str {
    match mime {
        "image/gif" => "gif",
        "image/apng" | "image/png" => "png",
        "video/quicktime" => "mov",
        _ => "mp4",
    }
}

impl AnimJobQueue {
    /// Loads persisted jobs from the sidecar dir. Any job still `Running` from a
    /// previous session becomes `Interrupted`, the flips are persisted, and the
    /// dir is pruned to [`MAX_JOBS`]. A missing or unreadable dir yields an
    /// empty in-memory queue.
    pub(crate) fn new() -> Self {
        let dir = jobs_dir();
        let mut jobs: HashMap<u64, AnimJobRecord> = HashMap::new();
        let mut max_id = 0u64;
        if let Some(dir) = &dir {
            if let Err(err) = std::fs::create_dir_all(dir) {
                tracing::warn!(error = %err, "failed to create animation-jobs dir; jobs in memory only");
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
                    let Ok(mut job) = serde_json::from_slice::<AnimJobRecord>(&bytes) else {
                        continue;
                    };
                    if job.status == AnimJobStatus::Running {
                        job.status = AnimJobStatus::Interrupted;
                    }
                    max_id = max_id.max(job.id);
                    jobs.insert(job.id, job);
                }
            }
        }
        let mut queue = Self {
            next_id: max_id + 1,
            jobs,
            cancellations: HashMap::new(),
            dir,
        };
        // Persist the Running->Interrupted flips so the recovery state is
        // durable, then bound the dir.
        let interrupted: Vec<AnimJobRecord> = queue.jobs.values().filter(|j| j.status == AnimJobStatus::Interrupted).cloned().collect();
        for job in &interrupted {
            queue.persist(job);
        }
        queue.prune();
        queue
    }

    /// Sidecar path for a job's JSON record, or `None` when in-memory only.
    fn sidecar_path(&self, id: u64) -> Option<PathBuf> {
        self.dir.as_ref().map(|d| d.join(format!("{id}.json")))
    }

    /// Path for a job's clip blob, or `None` when in-memory only.
    fn clip_path(&self, id: u64, mime: &str) -> Option<PathBuf> {
        self.dir.as_ref().map(|d| d.join(format!("{id}.{}", ext_for_mime(mime))))
    }

    /// Writes a job's JSON sidecar. A failure degrades to in-memory: it is logged
    /// and swallowed, never propagated, so a full disk cannot crash the studio.
    fn persist(&self, job: &AnimJobRecord) {
        let Some(path) = self.sidecar_path(job.id) else {
            return;
        };
        match serde_json::to_vec(job) {
            Ok(bytes) => {
                if let Err(err) = std::fs::write(&path, bytes) {
                    tracing::warn!(error = %err, id = job.id, "failed to persist animation job");
                }
            }
            Err(err) => tracing::warn!(error = %err, id = job.id, "failed to serialize animation job"),
        }
    }

    /// Registers a new running job and returns its id and cancel token. The
    /// caller threads the id into the spawned task so the terminal arms can call
    /// the matching `finish_*`.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn start(
        &mut self,
        entity_id: EntityId,
        sprite_id: SpriteId,
        direction: String,
        kind: Option<String>,
        model: String,
        motion: String,
        target_count: u32,
        fps: u32,
        seed: Option<u64>,
    ) -> (u64, CancellationToken) {
        let id = self.next_id;
        self.next_id += 1;
        let token = CancellationToken::new();
        let job = AnimJobRecord {
            id,
            entity_id,
            sprite_id,
            direction,
            animation_kind: kind,
            model,
            motion,
            status: AnimJobStatus::Running,
            created_at_ms: now_ms(),
            target_count,
            fps,
            mime: "video/mp4".into(),
            seed,
            error: None,
            qc: None,
        };
        self.cancellations.insert(id, token.clone());
        self.persist(&job);
        self.jobs.insert(id, job);
        (id, token)
    }

    /// Records an already-decoded clip — a user-supplied video, not an i2v run —
    /// as a `Done` job with `model = "imported"`, writing its clip blob beside the
    /// sidecar. There is no `Running` phase and no cancel token: the clip exists
    /// before the record does. Returns the allocated id. If the blob cannot be
    /// written the record is `Error`, mirroring [`finish_done`](Self::finish_done)'s honesty rule.
    pub(crate) fn record_import(
        &mut self,
        entity_id: EntityId,
        sprite_id: SpriteId,
        direction: String,
        kind: Option<String>,
        mime: &str,
        target_count: u32,
        fps: u32,
        clip: &[u8],
    ) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        let write_error: Option<String> = match self.clip_path(id, mime) {
            Some(path) => match std::fs::write(&path, clip) {
                Ok(()) => None,
                Err(err) => {
                    tracing::warn!(error = %err, id, "failed to write imported clip");
                    Some(format!("failed to persist clip: {err}"))
                }
            },
            None => None,
        };
        let job = AnimJobRecord {
            id,
            entity_id,
            sprite_id,
            direction,
            animation_kind: kind,
            model: "imported".into(),
            motion: String::new(),
            status: if write_error.is_some() { AnimJobStatus::Error } else { AnimJobStatus::Done },
            created_at_ms: now_ms(),
            target_count,
            fps,
            mime: mime.to_owned(),
            seed: None,
            error: write_error,
            qc: None,
        };
        self.persist(&job);
        self.jobs.insert(id, job);
        self.prune();
        id
    }

    /// Marks a job done and writes its clip bytes beside the sidecar. If the clip
    /// cannot be persisted, the job ends as `Error` rather than a "success" whose
    /// clip cannot be fetched — mirroring the Tauri policy. With no storage dir
    /// the clip stays in memory and the job is `Done` (the in-memory floor).
    pub(crate) fn finish_done(&mut self, id: u64, mime: &str, clip: &[u8]) {
        self.cancellations.remove(&id);
        // No dir means in-memory only: the clip lives in the candidate, so a
        // missing blob is not an error here.
        let write_error: Option<String> = match self.clip_path(id, mime) {
            Some(path) => match std::fs::write(&path, clip) {
                Ok(()) => None,
                Err(err) => {
                    tracing::warn!(error = %err, id, "failed to write animation clip");
                    Some(format!("failed to persist clip: {err}"))
                }
            },
            None => None,
        };
        let Some(job) = self.jobs.get_mut(&id) else {
            return;
        };
        if let Some(err) = write_error {
            job.status = AnimJobStatus::Error;
            job.error = Some(err);
        } else {
            job.status = AnimJobStatus::Done;
            job.error = None;
            mime.clone_into(&mut job.mime);
        }
        let snapshot = job.clone();
        self.persist(&snapshot);
        self.prune();
    }

    /// Attaches the per-frame QC record to a job and re-persists its sidecar, at
    /// Land. Runs after [`Self::finish_done`] — the decode-time finalize is too
    /// early, before normalize has measured anything. A missing id is a no-op
    /// (the job was pruned or never existed); the landed `Animation` copy is the
    /// source of truth, so a skipped sidecar loses nothing durable.
    pub(crate) fn finalize_qc(&mut self, id: u64, qc: pixhaus_core::project::AnimationQc) {
        let Some(job) = self.jobs.get_mut(&id) else {
            return;
        };
        job.qc = Some(qc);
        let snapshot = job.clone();
        self.persist(&snapshot);
    }

    /// Marks a job failed.
    pub(crate) fn finish_error(&mut self, id: u64, error: &str) {
        self.finish_terminal(id, AnimJobStatus::Error, Some(error.to_owned()));
    }

    /// Marks a job cancelled.
    pub(crate) fn finish_cancelled(&mut self, id: u64) {
        self.finish_terminal(id, AnimJobStatus::Cancelled, None);
    }

    /// Sets a terminal status and persists. Drops the cancel token.
    fn finish_terminal(&mut self, id: u64, status: AnimJobStatus, error: Option<String>) {
        self.cancellations.remove(&id);
        let Some(job) = self.jobs.get_mut(&id) else {
            return;
        };
        job.status = status;
        job.error = error;
        let snapshot = job.clone();
        self.persist(&snapshot);
        self.prune();
    }

    /// Returns a snapshot of one job.
    // The read side of the queue. Tested here; the studio jobs-tray / recovery
    // surface (a later task in this plan) is the non-test consumer.
    #[allow(dead_code)]
    pub(crate) fn get(&self, id: u64) -> Option<AnimJobRecord> {
        self.jobs.get(&id).cloned()
    }

    /// Returns the entity's jobs, newest first.
    #[allow(dead_code)]
    pub(crate) fn list(&self, entity_id: EntityId) -> Vec<AnimJobRecord> {
        let mut v: Vec<AnimJobRecord> = self.jobs.values().filter(|j| j.entity_id == entity_id).cloned().collect();
        v.sort_by_key(|j| std::cmp::Reverse(j.created_at_ms));
        v
    }

    /// Reads a finished job's clip bytes and mime from disk. `None` when the job
    /// is not `Done`, in-memory only, or the blob is unreadable.
    #[allow(dead_code)]
    pub(crate) fn read_clip(&self, id: u64) -> Option<(Vec<u8>, String)> {
        let job = self.get(id)?;
        if job.status != AnimJobStatus::Done {
            return None;
        }
        let path = self.clip_path(id, &job.mime)?;
        let bytes = std::fs::read(path).ok()?;
        Some((bytes, job.mime))
    }

    /// Total number of records the queue holds, across every entity and status.
    /// Test-only: the static-sheet path must add none, and asserting the count is
    /// unchanged is the regression guard against a future refactor reintroducing
    /// a phantom i2v job record.
    #[cfg(test)]
    pub(crate) fn record_count(&self) -> usize {
        self.jobs.len()
    }

    /// Cancels a running job's token. Returns whether a token was present.
    pub(crate) fn cancel(&mut self, id: u64) -> bool {
        if let Some(token) = self.cancellations.remove(&id) {
            token.cancel();
            true
        } else {
            false
        }
    }

    /// Drops the oldest terminal jobs (and their files) past [`MAX_JOBS`].
    /// Running jobs are never pruned.
    fn prune(&mut self) {
        if self.jobs.len() <= MAX_JOBS {
            return;
        }
        let mut terminal: Vec<(u64, u64)> = self
            .jobs
            .values()
            .filter(|j| j.status != AnimJobStatus::Running)
            .map(|j| (j.created_at_ms, j.id))
            .collect();
        // Newest first; remove everything past the cap.
        terminal.sort_by_key(|t| std::cmp::Reverse(t.0));
        for (_, id) in terminal.into_iter().skip(MAX_JOBS) {
            if let Some(job) = self.jobs.remove(&id) {
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

    /// A queue with no storage dir — every op stays in memory.
    fn queue_without_disk() -> AnimJobQueue {
        AnimJobQueue {
            next_id: 1,
            jobs: HashMap::new(),
            cancellations: HashMap::new(),
            dir: None,
        }
    }

    /// A small populated QC record for the finalize-qc tests.
    fn sample_qc() -> pixhaus_core::project::AnimationQc {
        use pixhaus_core::project::Rgba;
        use pixhaus_core::project::qc::{AnimationQc, FrameQc, NormalizeReportSummary, QcSettings};
        use pixhaus_core::transforms::SeamMatch;
        AnimationQc {
            settings: QcSettings {
                key_color: Rgba::opaque(255, 0, 255),
                key_tolerance: 24,
                alpha_threshold: 8,
                canvas: (16, 16),
                bottom_margin: 0,
                reference_height: 12,
                remove_on_land: true,
            },
            report: NormalizeReportSummary {
                baseline_drift_px: 0,
                scale_match_pct: 100,
                seam: SeamMatch::Ok,
                reference_height: 12,
                max_components: 1,
                any_edge_touch: false,
                warnings: Vec::new(),
            },
            frames: vec![FrameQc {
                bbox: (2, 2, 12, 12),
                center_x: 8,
                foot_baseline_y: 13,
                empty: false,
                num_components: 1,
                largest_component_area: 144,
                largest_component_bbox: Some((2, 2, 12, 12)),
                edge_touch: false,
            }],
        }
    }

    fn start_a_job(q: &mut AnimJobQueue, entity: u32, direction: &str, kind: Option<&str>) -> u64 {
        q.start(
            EntityId::new(entity),
            SpriteId::new(entity),
            direction.into(),
            kind.map(str::to_owned),
            "seedance".into(),
            "walk cycle".into(),
            10,
            12,
            None,
        )
        .0
    }

    #[test]
    fn start_allocates_increasing_ids_and_tracks_running() {
        let mut q = queue_without_disk();
        let id1 = start_a_job(&mut q, 7, "south", Some("idle"));
        let id2 = start_a_job(&mut q, 7, "west", None);
        assert!(id2 > id1);
        assert_eq!(q.get(id1).unwrap().status, AnimJobStatus::Running);
        assert_eq!(q.list(EntityId::new(7)).len(), 2);
        assert_eq!(q.list(EntityId::new(99)).len(), 0);
    }

    #[test]
    fn finish_transitions_and_cancel() {
        let mut q = queue_without_disk();
        let id = start_a_job(&mut q, 1, "south", None);
        q.finish_error(id, "boom");
        let job = q.get(id).unwrap();
        assert_eq!(job.status, AnimJobStatus::Error);
        assert_eq!(job.error.as_deref(), Some("boom"));
        // Token already removed on finish; cancel reports nothing to cancel.
        assert!(!q.cancel(id));

        let id2 = start_a_job(&mut q, 1, "south", None);
        assert!(q.cancel(id2));
    }

    #[test]
    fn list_is_newest_first() {
        let mut q = queue_without_disk();
        let a = start_a_job(&mut q, 5, "south", None);
        let b = start_a_job(&mut q, 5, "north", None);
        let ids: Vec<u64> = q.list(EntityId::new(5)).iter().map(|j| j.id).collect();
        // Both jobs belong to the entity; b started after a.
        assert!(ids.contains(&a) && ids.contains(&b));
        assert_eq!(ids.len(), 2);
    }

    #[test]
    fn finish_done_in_memory_marks_done_without_a_blob() {
        // With no storage dir the clip stays in the candidate, so a missing blob
        // is not an error: the job is Done, but read_clip has nothing on disk.
        let mut q = queue_without_disk();
        let id = start_a_job(&mut q, 2, "east", Some("attack"));
        q.finish_done(id, "video/mp4", &[1, 2, 3, 4]);
        assert_eq!(q.get(id).unwrap().status, AnimJobStatus::Done);
        assert!(q.read_clip(id).is_none());
    }

    #[test]
    fn running_sidecar_reloads_as_interrupted() {
        // A queue backed by a temp dir, a started (Running) job, then a fresh
        // queue over the same dir: the job must reload as Interrupted and the
        // flip must be durable.
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("animation-jobs");
        std::fs::create_dir_all(&dir).unwrap();

        let id = {
            let mut q = AnimJobQueue {
                next_id: 1,
                jobs: HashMap::new(),
                cancellations: HashMap::new(),
                dir: Some(dir.clone()),
            };
            start_a_job(&mut q, 3, "north", Some("walk"))
        };

        // Rebuild over the same dir, the way `new()` would on restart.
        let mut reloaded = AnimJobQueue {
            next_id: 1,
            jobs: HashMap::new(),
            cancellations: HashMap::new(),
            dir: Some(dir.clone()),
        };
        // Mirror `new()`'s load + flip + persist over the existing dir.
        for entry in std::fs::read_dir(&dir).unwrap().flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let bytes = std::fs::read(&path).unwrap();
            let mut job: AnimJobRecord = serde_json::from_slice(&bytes).unwrap();
            if job.status == AnimJobStatus::Running {
                job.status = AnimJobStatus::Interrupted;
            }
            reloaded.next_id = reloaded.next_id.max(job.id + 1);
            reloaded.jobs.insert(job.id, job);
        }
        let interrupted: Vec<AnimJobRecord> = reloaded.jobs.values().filter(|j| j.status == AnimJobStatus::Interrupted).cloned().collect();
        for job in &interrupted {
            reloaded.persist(job);
        }

        let job = reloaded.get(id).unwrap();
        assert_eq!(job.status, AnimJobStatus::Interrupted);

        // The flip is durable: the sidecar on disk now reads Interrupted.
        let sidecar = dir.join(format!("{id}.json"));
        let bytes = std::fs::read(sidecar).unwrap();
        let on_disk: AnimJobRecord = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(on_disk.status, AnimJobStatus::Interrupted);
    }

    #[test]
    fn finish_done_writes_and_reads_a_clip_blob() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("animation-jobs");
        std::fs::create_dir_all(&dir).unwrap();
        let mut q = AnimJobQueue {
            next_id: 1,
            jobs: HashMap::new(),
            cancellations: HashMap::new(),
            dir: Some(dir),
        };
        let id = start_a_job(&mut q, 4, "south", Some("idle"));
        let clip = [9u8, 8, 7, 6, 5];
        q.finish_done(id, "image/gif", &clip);
        assert_eq!(q.get(id).unwrap().status, AnimJobStatus::Done);
        let (bytes, mime) = q.read_clip(id).unwrap();
        assert_eq!(bytes, clip);
        assert_eq!(mime, "image/gif");
    }

    #[test]
    fn record_import_marks_done_and_tags_imported() {
        // A user-supplied clip records straight to Done with model = "imported"
        // and no seed/motion, then reads back from disk.
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("animation-jobs");
        std::fs::create_dir_all(&dir).unwrap();
        let mut q = AnimJobQueue {
            next_id: 1,
            jobs: HashMap::new(),
            cancellations: HashMap::new(),
            dir: Some(dir),
        };
        let clip = [4u8, 3, 2, 1];
        let id = q.record_import(
            EntityId::new(8),
            SpriteId::new(8),
            "south".into(),
            Some("walk".into()),
            "image/gif",
            6,
            12,
            &clip,
        );
        let job = q.get(id).unwrap();
        assert_eq!(job.status, AnimJobStatus::Done);
        assert_eq!(job.model, "imported");
        assert_eq!(job.motion, "");
        assert_eq!(job.seed, None);
        // The clip blob is readable, the way a generated Done job's is.
        let (bytes, mime) = q.read_clip(id).unwrap();
        assert_eq!(bytes, clip);
        assert_eq!(mime, "image/gif");
    }

    #[test]
    fn record_import_in_memory_marks_done_without_a_blob() {
        // No storage dir: the import is still Done (the in-memory floor), but
        // there is no blob to read.
        let mut q = queue_without_disk();
        let id = q.record_import(EntityId::new(1), SpriteId::new(1), "east".into(), None, "video/mp4", 4, 24, &[1, 2, 3]);
        assert_eq!(q.get(id).unwrap().status, AnimJobStatus::Done);
        assert_eq!(q.get(id).unwrap().model, "imported");
        assert!(q.read_clip(id).is_none());
    }

    #[test]
    fn record_round_trips_through_serde_with_defaults() {
        // An older sidecar missing the optional fields still deserializes.
        let json = r#"{
            "id": 1,
            "entity_id": 7,
            "sprite_id": 7,
            "direction": "south",
            "model": "seedance",
            "status": "running",
            "created_at_ms": 0,
            "target_count": 6,
            "fps": 10,
            "mime": "video/mp4"
        }"#;
        let job: AnimJobRecord = serde_json::from_str(json).unwrap();
        assert_eq!(job.animation_kind, None);
        assert_eq!(job.seed, None);
        assert_eq!(job.error, None);
        assert_eq!(job.motion, "");
        assert_eq!(job.status, AnimJobStatus::Running);
        // A sidecar written before the qc field existed still loads.
        assert_eq!(job.qc, None, "qc defaults to None on an older sidecar");
        // Status serializes snake_case.
        let out = serde_json::to_string(&job.status).unwrap();
        assert_eq!(out, "\"running\"");
    }

    #[test]
    fn finalize_qc_persists_and_reloads_from_the_sidecar() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("animation-jobs");
        std::fs::create_dir_all(&dir).unwrap();
        let id = {
            let mut q = AnimJobQueue {
                next_id: 1,
                jobs: HashMap::new(),
                cancellations: HashMap::new(),
                dir: Some(dir.clone()),
            };
            let id = start_a_job(&mut q, 4, "south", Some("walk"));
            // The decode-time finalize runs first; the qc finalize follows at Land.
            q.finish_done(id, "image/gif", &[1, 2, 3]);
            q.finalize_qc(id, sample_qc());
            assert_eq!(q.get(id).unwrap().qc, Some(sample_qc()), "the qc is on the in-memory record");
            id
        };

        // Reload the sidecar the way `new()` would: the qc must be durable.
        let sidecar = dir.join(format!("{id}.json"));
        let bytes = std::fs::read(sidecar).unwrap();
        let on_disk: AnimJobRecord = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(on_disk.qc, Some(sample_qc()), "the qc survives a reload from disk");
    }

    #[test]
    fn finalize_qc_on_missing_id_is_a_noop() {
        // A pruned or never-existing job: finalize_qc must not panic or create a
        // record. The landed Animation copy is the source of truth.
        let mut q = queue_without_disk();
        q.finalize_qc(999, sample_qc());
        assert!(q.get(999).is_none(), "finalize_qc on a missing id creates nothing");
        assert_eq!(q.record_count(), 0);
    }
}
