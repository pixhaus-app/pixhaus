//! The job system: submit generation work off the UI thread, deliver results back.
//!
//! [`JobManager::submit`] spawns a provider's work onto the binary's tokio runtime
//! (it never creates one) and returns immediately. The task emits [`JobMsg`]
//! notifications over a `std::sync::mpsc` channel the shell drains each frame; the
//! [`GeneratedAsset`](crate::generated::GeneratedAsset) itself lands in the shared
//! [`ResultStore`], keyed by job, so large buffers stay off the channel (bible 31.5).
//! The worker receives only immutable input and a cancel token — never the live
//! document (bible 13.6).

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::Sender;

use tokio_util::sync::CancellationToken;

use crate::provider::{Provider, ProviderError};
use crate::result_store::ResultStore;

/// A job's stable id within one [`JobManager`].
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct JobId(pub u64);

/// Where a job is in its lifecycle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JobStatus {
    /// Submitted, not yet picked up.
    Queued,
    /// Running on a worker.
    Running,
    /// Finished successfully; the result is in the [`ResultStore`].
    Complete,
    /// Failed with an error.
    Failed,
    /// Cancelled before it finished.
    Cancelled,
}

/// Immutable input handed to a provider worker (bible 13.6). Owns everything the
/// worker needs; carries no document handle, no undo stack, no egui context.
#[derive(Clone, Debug)]
pub struct GenerationJobInput {
    /// The user's prompt (content).
    pub prompt: String,
    /// The seed for reproducible output.
    pub seed: u64,
    /// The target image size, `(width, height)`.
    pub size: (u32, u32),
    /// What the generation is relative to.
    pub context: GenerationContext,
}

/// Generation context as immutable data (bible 14.6). Foundation: a new asset only;
/// variants that reference live sprites will carry a snapshot, never `&Document`.
#[derive(Clone, Debug)]
pub enum GenerationContext {
    /// Generate a brand-new asset with no prior context.
    NewAsset,
}

/// A notification a running job emits toward the UI lane. The shell maps these onto
/// its background channel; `services` stays egui-free.
#[derive(Debug)]
pub enum JobMsg {
    /// The job changed status.
    Status {
        /// The job.
        job: JobId,
        /// Its new status.
        status: JobStatus,
    },
    /// The job finished; its asset is in the [`ResultStore`], keyed by `job`.
    Completed {
        /// The job.
        job: JobId,
    },
    /// The job failed; `error` is a pre-resolved, user-presentable message.
    Failed {
        /// The job.
        job: JobId,
        /// The failure message.
        error: String,
    },
}

/// Queues, dispatches, and cancels generation jobs. Long-lived, held by the host.
/// Spawns provider work onto the ambient tokio runtime the binary owns.
pub struct JobManager {
    counter: AtomicU64,
    statuses: HashMap<JobId, JobStatus>,
    cancels: HashMap<JobId, CancellationToken>,
    out: Sender<JobMsg>,
}

impl JobManager {
    /// A manager that sends notifications to `out` (the shell's background sender).
    pub fn new(out: Sender<JobMsg>) -> Self {
        Self {
            counter: AtomicU64::new(0),
            statuses: HashMap::new(),
            cancels: HashMap::new(),
            out,
        }
    }

    /// Submits a generation job to `provider`, returning immediately with its id.
    ///
    /// Spawns the provider future as a tokio task that emits `Status(Running)`, then
    /// deposits the asset into `results` and emits `Completed` on success, or
    /// `Failed`/`Cancelled` otherwise.
    #[tracing::instrument(skip_all, fields(provider = %provider.id().0))]
    pub fn submit(&mut self, provider: Arc<dyn Provider>, input: GenerationJobInput, results: Arc<ResultStore>) -> JobId {
        let id = JobId(self.counter.fetch_add(1, Ordering::Relaxed));
        let cancel = CancellationToken::new();
        self.statuses.insert(id, JobStatus::Queued);
        self.cancels.insert(id, cancel.clone());
        let out = self.out.clone();

        tokio::spawn(async move {
            let _ = out.send(JobMsg::Status {
                job: id,
                status: JobStatus::Running,
            });
            let outcome = tokio::select! {
                biased;
                () = cancel.cancelled() => Err(ProviderError::Cancelled),
                result = provider.generate(&input, cancel.clone()) => result,
            };
            match outcome {
                Ok(asset) => {
                    results.put(id, asset);
                    let _ = out.send(JobMsg::Completed { job: id });
                }
                Err(ProviderError::Cancelled) => {
                    let _ = out.send(JobMsg::Status {
                        job: id,
                        status: JobStatus::Cancelled,
                    });
                }
                Err(error) => {
                    let _ = out.send(JobMsg::Failed {
                        job: id,
                        error: error.to_string(),
                    });
                }
            }
        });

        id
    }

    /// The last-known status for `job`, if tracked.
    pub fn status(&self, job: JobId) -> Option<JobStatus> {
        self.statuses.get(&job).cloned()
    }

    /// Records a status (the shell calls this when it drains a [`JobMsg::Status`]).
    pub fn set_status(&mut self, job: JobId, status: JobStatus) {
        self.statuses.insert(job, status);
    }

    /// Requests cancellation of `job` (fires its token; no-op if unknown).
    pub fn cancel(&mut self, job: JobId) {
        if let Some(token) = self.cancels.get(&job) {
            token.cancel();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::{Receiver, TryRecvError};
    use std::time::Duration;

    use crate::generated::{GeneratedAsset, GenerationProvenance};
    use crate::provider::{GenerateFuture, ProviderCapability, ProviderId};

    fn input() -> GenerationJobInput {
        GenerationJobInput {
            prompt: "p".to_owned(),
            seed: 1,
            size: (2, 2),
            context: GenerationContext::NewAsset,
        }
    }

    /// A test provider with selectable behaviour: succeed fast, fail, or stall until
    /// cancelled.
    enum Behaviour {
        Ok,
        Fail,
        Stall,
    }

    struct Fake {
        behaviour: Behaviour,
    }

    impl Provider for Fake {
        fn id(&self) -> ProviderId {
            ProviderId("fake".to_owned())
        }
        fn label_key(&self) -> &'static str {
            "provider.fake.label"
        }
        fn capabilities(&self) -> &[ProviderCapability] {
            &[ProviderCapability::TextToSprite]
        }
        fn generate<'a>(&'a self, _input: &'a GenerationJobInput, cancel: CancellationToken) -> GenerateFuture<'a> {
            match self.behaviour {
                Behaviour::Ok => Box::pin(async {
                    Ok(GeneratedAsset {
                        width: 1,
                        height: 1,
                        stride: 4,
                        rgba: vec![1, 2, 3, 255],
                        provenance: GenerationProvenance {
                            prompt: String::new(),
                            seed: 0,
                            provider_id: "fake".to_owned(),
                            model: "fake".to_owned(),
                            created_unix_ms: 0,
                        },
                    })
                }),
                Behaviour::Fail => Box::pin(async { Err(ProviderError::Unavailable("down".to_owned())) }),
                Behaviour::Stall => Box::pin(async move {
                    cancel.cancelled().await;
                    Err(ProviderError::Cancelled)
                }),
            }
        }
    }

    async fn drain_until_terminal(rx: &Receiver<JobMsg>) -> Vec<JobMsg> {
        let mut msgs = Vec::new();
        for _ in 0..5000 {
            match rx.try_recv() {
                Ok(msg) => {
                    let terminal = matches!(
                        &msg,
                        JobMsg::Completed { .. }
                            | JobMsg::Failed { .. }
                            | JobMsg::Status {
                                status: JobStatus::Cancelled,
                                ..
                            }
                    );
                    msgs.push(msg);
                    if terminal {
                        break;
                    }
                }
                Err(TryRecvError::Empty) => tokio::time::sleep(Duration::from_millis(1)).await,
                Err(TryRecvError::Disconnected) => break,
            }
        }
        msgs
    }

    #[tokio::test]
    async fn submit_runs_then_completes_and_stores_the_asset() {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut jobs = JobManager::new(tx);
        let results = Arc::new(ResultStore::new());
        let provider: Arc<dyn Provider> = Arc::new(Fake { behaviour: Behaviour::Ok });

        jobs.submit(provider, input(), results.clone());
        let msgs = drain_until_terminal(&rx).await;

        assert!(matches!(
            msgs.first(),
            Some(JobMsg::Status {
                status: JobStatus::Running,
                ..
            })
        ));
        assert!(matches!(msgs.last(), Some(JobMsg::Completed { .. })));
        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    async fn submit_reports_failure() {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut jobs = JobManager::new(tx);
        let results = Arc::new(ResultStore::new());
        let provider: Arc<dyn Provider> = Arc::new(Fake { behaviour: Behaviour::Fail });

        jobs.submit(provider, input(), results.clone());
        let msgs = drain_until_terminal(&rx).await;

        assert!(matches!(msgs.last(), Some(JobMsg::Failed { .. })));
        assert_eq!(results.len(), 0);
    }

    #[tokio::test]
    async fn cancel_stops_a_stalled_job() {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut jobs = JobManager::new(tx);
        let results = Arc::new(ResultStore::new());
        let provider: Arc<dyn Provider> = Arc::new(Fake { behaviour: Behaviour::Stall });

        let id = jobs.submit(provider, input(), results.clone());
        jobs.cancel(id);
        let msgs = drain_until_terminal(&rx).await;

        assert!(matches!(
            msgs.last(),
            Some(JobMsg::Status {
                status: JobStatus::Cancelled,
                ..
            })
        ));
        assert_eq!(results.len(), 0);
    }
}
