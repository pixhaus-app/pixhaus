# Modeling the inference-job lifecycle with type-state

Use the type-state pattern: make the lifecycle stage a type parameter on the
job, not a `status` field on it. Then the set of methods you can call is decided
by the compiler, and the invariants you asked for fall out for free:

- `output()` exists only on `Job<Finished>`. There is no method to read the
  image on a draft or a running job, so "read the output before it's finished"
  is a compile error, not a runtime check.
- `submit()` exists only on `Job<Draft>`, and it takes `self` by value. Once you
  submit, the draft is moved and gone; the `Running` job has no `submit` method.
  So "re-submit a job that's already running" is a compile error too.

## Why type-state, and the rule it follows

The rule: when an object's legal operations change as it moves through stages,
encode the stage in the type, not in a field you check at runtime. A `status:
Stage` enum forces every method to start with "if we're not in the right stage,
return an error" — and that error path is dead weight that can still be reached.
Type-state deletes the error path. The wrong call doesn't fail at runtime; it
doesn't exist.

This fits the repo conventions directly. The no-`unwrap`/no-`panic` rule exists
because runtime "this can't happen" assertions are a smell. Type-state removes
the assertion entirely: there's no `Option<OutputImage>` to unwrap on a job
that hasn't finished, because the finished job owns an `OutputImage` by value and
the unfinished ones don't have the field at all.

## The shape

```rust
pub struct Job<S> { state: S }

pub struct Draft    { config: JobConfig }                              // can submit()
pub struct Running  { config: JobConfig, handle: JoinHandle<Image> }   // can await_output()
pub struct Finished { output: OutputImage }                            // can output()

impl Job<Draft>    { pub fn submit(self, h: JoinHandle<Image>) -> Job<Running> { … } }
impl Job<Running>  { pub fn await_output(self, …) -> Job<Finished> { … } }
impl Job<Finished> { pub fn output(&self) -> &OutputImage { … } }
```

Each transition takes `self` by value and returns the next state, so the old
state is consumed — you can't hold a `Draft` and a `Running` for the same job at
once, and you can't reuse the spawned handle after awaiting it.

## Data lives inline, per state — no `Option`, no `unreachable!()`

Each state struct holds exactly what that stage owns:

- `Draft` holds the `JobConfig`. No handle, no output.
- `Running` holds the `JobConfig` and the `JoinHandle` directly. The handle is a
  real field, not `Option<JoinHandle>`, because a running job always has one.
- `Finished` holds the `OutputImage` by value. Not `Option<OutputImage>`,
  because a finished job always has its output.

This is the payoff over a one-struct-with-`Option`-fields design. In that design
`output: Option<OutputImage>` is `None` for most of the job's life, and every
read is `self.output.as_ref().unwrap()` or a `match` with an `unreachable!()`
arm — exactly the panic-bait the repo forbids. Here, the field is present
precisely when it's valid, so there's nothing to unwrap and no impossible arm to
mark unreachable.

## Notes for the real crate

- Replace the stub `JoinHandle<T>` with `tokio::task::JoinHandle<RgbaImage>` and
  `OutputImage` with `image::RgbaImage`.
- `await_output` becomes `async fn` and does `self.state.handle.await`. Per the
  repo's async rules the egui loop owns the document, so in practice you poll the
  handle from the frame loop and swap `Job<Running>` for `Job<Finished>` when it
  resolves, rather than blocking on `.await` inside the loop. The type-state
  shape is the same either way — the transition just happens when the channel /
  handle yields.
- The join can fail (task panicked or was cancelled). Model that with a fallible
  transition returning `Result<Job<Finished>, JobError>` (a `thiserror` enum),
  or add a `Failed` state. Either keeps the "output only on success" invariant.
- If you store jobs in a single collection, the three `Job<S>` types are
  distinct, so wrap them in an `enum JobSlot { Draft(Job<Draft>),
  Running(Job<Running>), Finished(Job<Finished>) }` for storage. You still get
  type-state safety inside each match arm; the enum is only the storage seam.

## Compile status

Built with `rustc --edition 2024` both as a `--crate-type lib` and as a test
binary: clean, no warnings. The two unit tests pass. The `_lifecycle_typechecks`
function documents the rejected calls as comments (uncommenting any one fails the
build), proving the negative cases.
