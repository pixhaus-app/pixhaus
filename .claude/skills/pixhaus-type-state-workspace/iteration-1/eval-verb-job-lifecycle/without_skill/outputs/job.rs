//! Type-state model for an AI inference job's lifecycle.
//!
//! Three states, each a distinct type:
//!   - `Draft`    — configured but not sent. Can `submit()`. Has no handle, no output.
//!   - `Running`  — submitted, owns the spawned task handle. Can `await_output()`.
//!   - `Finished` — holds the output image. Can `output()`.
//!
//! The state lives in the type parameter `S`, so the compiler — not a runtime
//! `status` field — decides which methods exist. Reading the output before the
//! job finishes, or submitting a job that's already running, is a type error,
//! not a runtime check.
//!
//! Each state stores exactly the data it owns, inline, with no `Option` and no
//! `unreachable!()` to "prove" a field is present. A `Draft` has no handle field
//! at all; a `Finished` has no handle field at all. The data that's irrelevant
//! to a state simply isn't in that state's struct.

// --- Stubs so this file compiles standalone (no tokio / image crate). ---------
// In the real crate these are `tokio::task::JoinHandle<RgbaImage>` and
// `image::RgbaImage`; nothing about the type-state design depends on them.

/// Stand-in for `tokio::task::JoinHandle<OutputImage>`.
pub struct JoinHandle<T> {
    _marker: core::marker::PhantomData<T>,
}

impl<T> JoinHandle<T> {
    /// Stand-in for awaiting the task. Real code would be `self.handle.await`.
    pub fn join_stub(self, value: T) -> T {
        // The stub just hands back a value; the real one awaits the future.
        let _ = self;
        value
    }
}

/// Stand-in for `image::RgbaImage`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutputImage {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}
// -----------------------------------------------------------------------------

/// What the user configures before submitting. Shared by every state that needs
/// to remember the request (here: `Draft` and `Running`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JobConfig {
    pub prompt: String,
    pub seed: u64,
}

// --- The three state types. ---------------------------------------------------
// These are the marker types that go into `Job<S>`. Each carries the data that
// only that state owns. No state carries data it doesn't have.

/// Draft state: a request configured but not yet sent. Owns only the config.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Draft {
    config: JobConfig,
}

/// Running state: owns the spawned task handle (and keeps the config around for
/// diagnostics / retry). The handle is stored directly — not behind `Option`.
pub struct Running {
    config: JobConfig,
    handle: JoinHandle<OutputImage>,
}

/// Finished state: owns the produced image directly. No `Option`, no handle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Finished {
    output: OutputImage,
}

/// The job, parameterized by its state. One struct, three reachable shapes.
///
/// The state value is the single source of truth; there is no separate `status`
/// enum to keep in sync, and no field that's only "sometimes" valid.
pub struct Job<S> {
    state: S,
}

// --- Draft: the only state that can submit. -----------------------------------

impl Job<Draft> {
    /// Start a new draft from a config. This is the only public constructor that
    /// hands you a job you can edit and submit.
    #[must_use]
    pub fn new(config: JobConfig) -> Self {
        Self {
            state: Draft { config },
        }
    }

    /// Read or tweak the config while still a draft.
    #[must_use]
    pub fn config(&self) -> &JobConfig {
        &self.state.config
    }

    /// Submit the draft. Consumes the `Draft` job and returns a `Running` one.
    ///
    /// `submit` exists ONLY on `Job<Draft>`. You cannot call it on a
    /// `Job<Running>` or a `Job<Finished>` — those types have no such method, so
    /// re-submitting an in-flight or completed job won't compile.
    ///
    /// `self` is taken by value, so the `Draft` is moved out of the caller's
    /// hands: the old draft no longer exists to be submitted twice.
    #[must_use]
    pub fn submit(self, handle: JoinHandle<OutputImage>) -> Job<Running> {
        Job {
            state: Running {
                config: self.state.config,
                handle,
            },
        }
    }
}

// --- Running: can be awaited, cannot be re-submitted, output not readable. -----

impl Job<Running> {
    /// Config is still available for display while the job runs.
    #[must_use]
    pub fn config(&self) -> &JobConfig {
        &self.state.config
    }

    /// Await the spawned task and transition to `Finished`.
    ///
    /// Consumes the `Running` job and the handle it owns. In the real crate this
    /// is `async` and does `self.state.handle.await`; here the stub takes the
    /// produced image to keep the file dependency-free.
    ///
    /// Note there is no `output()` here — you can't read the image off a
    /// `Running` job, because the field doesn't exist on `Running`.
    #[must_use]
    pub fn await_output(self, produced: OutputImage) -> Job<Finished> {
        let output = self.state.handle.join_stub(produced);
        Job {
            state: Finished { output },
        }
    }
}

// --- Finished: the only state that exposes the output. -------------------------

impl Job<Finished> {
    /// The output image. This accessor exists ONLY on `Job<Finished>`.
    ///
    /// There's nothing to unwrap and nothing to fail: a `Finished` job owns its
    /// `OutputImage` by value. Reaching for the output on a `Draft` or `Running`
    /// job is a compile error, because neither of those types has this method.
    #[must_use]
    pub fn output(&self) -> &OutputImage {
        &self.state.output
    }

    /// Take the output by value, consuming the job.
    #[must_use]
    pub fn into_output(self) -> OutputImage {
        self.state.output
    }
}

// --- Compile-time proof of the invariants. ------------------------------------
// These functions exist only to type-check. The commented-out lines are the
// errors the compiler would raise; uncommenting any of them fails the build.

/// Happy path: draft -> running -> finished, reading output only at the end.
fn _lifecycle_typechecks(handle: JoinHandle<OutputImage>, produced: OutputImage) {
    let draft = Job::<Draft>::new(JobConfig {
        prompt: "a small dragon".to_owned(),
        seed: 42,
    });
    let _ = draft.config();

    let running = draft.submit(handle);
    // draft.submit(handle);          // ERROR: `draft` was moved — can't re-submit.
    // let _ = draft.config();        // ERROR: use of moved value.
    // let _ = running.output();      // ERROR: no method `output` on Job<Running>.
    // running.submit(other_handle);  // ERROR: no method `submit` on Job<Running>.

    let finished = running.await_output(produced);
    // let _ = running.config();      // ERROR: `running` was moved.

    let _img = finished.output(); // OK: output() only here.
    let _ = finished.into_output();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_handle() -> JoinHandle<OutputImage> {
        JoinHandle {
            _marker: core::marker::PhantomData,
        }
    }

    fn fake_image() -> OutputImage {
        OutputImage {
            width: 2,
            height: 1,
            pixels: vec![255, 0, 0, 255, 0, 255, 0, 255],
        }
    }

    #[test]
    fn draft_carries_config() {
        let job = Job::<Draft>::new(JobConfig {
            prompt: "p".to_owned(),
            seed: 7,
        });
        assert_eq!(job.config().seed, 7);
    }

    #[test]
    fn full_lifecycle_yields_output() {
        let job = Job::<Draft>::new(JobConfig {
            prompt: "p".to_owned(),
            seed: 7,
        });
        let running = job.submit(fake_handle());
        assert_eq!(running.config().prompt, "p");
        let finished = running.await_output(fake_image());
        assert_eq!(finished.output().width, 2);
        let owned = finished.into_output();
        assert_eq!(owned.height, 1);
    }
}
