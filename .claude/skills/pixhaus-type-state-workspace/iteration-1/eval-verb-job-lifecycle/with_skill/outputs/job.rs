//! Type-state model for an AI inference job's lifecycle.
//!
//! Three states, each a distinct type:
//!   - `Job<Draft>`   — configured but not sent. Can `submit()`. Cannot read output.
//!   - `Job<Running>` — spawned, holds the task handle. Can `await_output()`. Cannot re-submit.
//!   - `Job<Done>`    — finished, holds the output image. Can `output()`.
//!
//! The guarantees the compiler enforces (not a runtime status field):
//!   - `output()` exists ONLY on `Job<Done>` — you cannot read the image before the job finishes.
//!   - `submit()` exists ONLY on `Job<Draft>` — you cannot re-submit a running or finished job;
//!     `submit` consumes the draft, so the draft handle is gone after the first call.
//!
//! State-specific data lives inside the state type (the skill's Pixhaus rule):
//! `Running` owns the `JoinHandle`, `Done` owns the `Image`. There is no
//! `Option` + `unreachable!`/`unwrap` anywhere — a field exists exactly when
//! the state that owns it does.
//!
//! Stubs for standalone compilation
//! --------------------------------
//! In the real crate these are the genuine types and this file carries none of
//! the stubs below:
//!   - `JoinHandle<T>` -> `tokio::task::JoinHandle<T>`
//!   - `Image`         -> the `image` crate's buffer (e.g. `image::RgbaImage`)
//!   - `submit` would take a `&tokio::runtime::Handle` and call `rt.spawn(..)`,
//!     and `await_output` would `.await` the real `JoinHandle` (returning
//!     `Result<Job<Done>, JobError>` for a join/cancel error). The shape of the
//!     state transitions is identical; only the spawn/await plumbing differs.

use std::marker::PhantomData;

// ---------------------------------------------------------------------------
// Stubs (real types noted above). Kept minimal so the file compiles with rustc
// alone, no external crates.
// ---------------------------------------------------------------------------

/// Stand-in for `tokio::task::JoinHandle<T>`.
pub struct JoinHandle<T> {
    _result: T,
}

impl<T> JoinHandle<T> {
    fn new(result: T) -> Self {
        JoinHandle { _result: result }
    }

    /// Stand-in for `.await` on a real `JoinHandle`. The real version is async
    /// and returns `Result<T, JoinError>`; here we just hand back the value.
    fn join(self) -> T {
        self._result
    }
}

/// Stand-in for the `image` crate's RGBA buffer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

// ---------------------------------------------------------------------------
// State types. `Draft` carries no data, so it's a bare marker used via
// PhantomData. `Running` and `Done` own their data, so they are real fields.
// The marker/state types are pub(crate) so downstream code can't forge a
// `Job<Done>` it never ran (skill pitfall: don't let callers construct states).
// ---------------------------------------------------------------------------

/// Configured but not yet sent. No state data; pure marker.
pub struct Draft;

/// Spawned and in flight. Owns the task handle.
pub struct Running {
    handle: JoinHandle<Image>,
}

/// Finished. Owns the produced image.
pub struct Done {
    output: Image,
}

/// An inference job, generic over its lifecycle state.
///
/// `prompt` and `config` are shared across every state, so they live on the
/// outer struct. State-specific data (the handle, the output) lives on `state`.
pub struct Job<S> {
    prompt: String,
    config: JobConfig,
    state: S,
    // Draft is a zero-data marker, so for the Draft arm `state` is the unit-like
    // `Draft`. PhantomData isn't strictly needed because `state: S` already ties
    // the type parameter, but we keep `S` as a real field so Running/Done can
    // store data without an Option.
    _phantom: PhantomData<fn() -> S>,
}

/// Whatever the caller dialed in before sending — model, steps, seed, etc.
#[derive(Debug, Clone, Default)]
pub struct JobConfig {
    pub model: String,
    pub steps: u32,
    pub seed: u64,
}

// ---------------------------------------------------------------------------
// Draft: the only state that can be constructed from scratch and the only one
// that can submit().
// ---------------------------------------------------------------------------

impl Job<Draft> {
    /// Start configuring a new job. This is the one public entry point; you
    /// cannot build a `Job<Running>` or `Job<Done>` directly.
    pub fn new(prompt: impl Into<String>, config: JobConfig) -> Self {
        Job {
            prompt: prompt.into(),
            config,
            state: Draft,
            _phantom: PhantomData,
        }
    }

    /// Send the job for inference. Consumes the draft and returns a running job.
    ///
    /// Because this takes `self` by value, the draft is gone afterward — there
    /// is no handle left to call `submit` on twice. And `submit` exists on no
    /// other state, so a `Job<Running>` or `Job<Done>` can't be re-submitted.
    ///
    /// Real version: `pub fn submit(self, rt: &tokio::runtime::Handle) -> Job<Running>`
    /// which calls `rt.spawn(async move { run_inference(prompt, config).await })`.
    pub fn submit(self) -> Job<Running> {
        let handle = spawn_inference(&self.prompt, &self.config);
        Job {
            prompt: self.prompt,
            config: self.config,
            state: Running { handle },
            _phantom: PhantomData,
        }
    }
}

// ---------------------------------------------------------------------------
// Running: holds the handle. Can be awaited into Done. Cannot read output yet
// (no `output` method exists here) and cannot be re-submitted (no `submit`).
// ---------------------------------------------------------------------------

impl Job<Running> {
    /// Wait for the spawned task and move to the finished state.
    ///
    /// `self.state.handle` is always a real handle here — `Job<Running>` can
    /// only be built by `submit`, which always supplies one. No `Option`, no
    /// `unwrap`, no `unreachable!`.
    ///
    /// Real version is `async` and returns `Result<Job<Done>, JobError>` so a
    /// join/cancel failure surfaces as a typed error rather than a panic.
    pub fn await_output(self) -> Job<Done> {
        let output = self.state.handle.join();
        Job {
            prompt: self.prompt,
            config: self.config,
            state: Done { output },
            _phantom: PhantomData,
        }
    }
}

// ---------------------------------------------------------------------------
// Done: holds the output. The ONLY state with an output accessor.
// ---------------------------------------------------------------------------

impl Job<Done> {
    /// Read the produced image. This method exists only on `Job<Done>`, so the
    /// output is unreadable until the job has actually finished. Always valid:
    /// `Job<Done>` can only be built by `await_output`, which always supplies it.
    pub fn output(&self) -> &Image {
        &self.state.output
    }

    /// Take ownership of the produced image.
    pub fn into_output(self) -> Image {
        self.state.output
    }
}

// ---------------------------------------------------------------------------
// Shared accessors — valid in every state, so they hang off `impl<S>`.
// ---------------------------------------------------------------------------

impl<S> Job<S> {
    pub fn prompt(&self) -> &str {
        &self.prompt
    }

    pub fn config(&self) -> &JobConfig {
        &self.config
    }
}

// ---------------------------------------------------------------------------
// Stub spawn. Real version: `rt.spawn(async move { run_inference(..).await })`.
// ---------------------------------------------------------------------------

fn spawn_inference(_prompt: &str, config: &JobConfig) -> JoinHandle<Image> {
    // The real backend produces this asynchronously; the stub fabricates a
    // tiny image so the lifecycle compiles and runs end to end.
    let image = Image {
        width: 1,
        height: 1,
        pixels: vec![0, 0, 0, config.seed as u8],
    };
    JoinHandle::new(image)
}

// ---------------------------------------------------------------------------
// Standalone demonstration of the happy path. (Present so the file compiles as
// a standalone binary with `rustc job.rs`; the real crate is a library and
// would drop this `main`.)
// ---------------------------------------------------------------------------

fn main() {
    let done = Job::new("a fox in snow", JobConfig::default())
        .submit()
        .await_output();
    let img = done.output();
    println!(
        "job done: {}x{}, {} bytes",
        img.width,
        img.height,
        img.pixels.len()
    );
}

// ---------------------------------------------------------------------------
// Tests. The commented lines inside `happy_path` are the calls the type system
// rejects — uncommenting any of them is a compile error, which is the whole
// point of the model.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_config() -> JobConfig {
        JobConfig {
            model: "sd-xl".to_string(),
            steps: 30,
            seed: 7,
        }
    }

    #[test]
    fn happy_path_draft_to_running_to_done() {
        let draft = Job::new("a cat", sample_config());
        assert_eq!(draft.prompt(), "a cat");

        let running = draft.submit();
        // draft.submit();          // <- won't compile: `draft` was moved by submit()
        // running.output();        // <- won't compile: no `output` on Job<Running>
        // running.submit();        // <- won't compile: no `submit` on Job<Running>

        let done = running.await_output();
        // running.await_output();  // <- won't compile: `running` was moved

        let img = done.output();
        assert_eq!(img.width, 1);
        assert_eq!(img.pixels.last().copied(), Some(7));
    }

    #[test]
    fn into_output_takes_ownership() {
        let done = Job::new("a dog", sample_config()).submit().await_output();
        let img = done.into_output();
        assert_eq!(img.height, 1);
    }

    #[test]
    fn config_readable_in_every_state() {
        let draft = Job::new("p", sample_config());
        assert_eq!(draft.config().steps, 30);
        let running = draft.submit();
        assert_eq!(running.config().model, "sd-xl");
        let done = running.await_output();
        assert_eq!(done.config().seed, 7);
    }
}
