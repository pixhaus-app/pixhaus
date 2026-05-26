# Model the inference job as type-state

You have three states known at compile time, transitions you drive in code (not
from runtime input), and operations that are only legal in one state — reading
the output, and submitting. That's exactly the case type-state is for. Encode
each state as its own type and hang the methods off the matching `impl`. Calling
`output()` on a draft or `submit()` on a running job then isn't a runtime check
you can forget — it's a method that doesn't exist, so the program won't compile.

## The types

```rust
use std::marker::PhantomData;

// In the real crate: JoinHandle -> tokio::task::JoinHandle, Image -> image::RgbaImage.

pub struct Draft;                              // configured, no extra data — bare marker
pub struct Running { handle: JoinHandle<Image> } // owns the spawned task handle
pub struct Done    { output: Image }            // owns the finished image

pub struct Job<S> {
    prompt: String,
    config: JobConfig,
    state: S,
    _phantom: PhantomData<fn() -> S>,
}

impl Job<Draft> {
    pub fn new(prompt: impl Into<String>, config: JobConfig) -> Self { /* ... */ }
    pub fn submit(self) -> Job<Running> { /* spawn, move handle into Running */ }
}

impl Job<Running> {
    pub fn await_output(self) -> Job<Done> { /* join handle, move image into Done */ }
}

impl Job<Done> {
    pub fn output(&self) -> &Image { &self.state.output }   // exists ONLY here
    pub fn into_output(self) -> Image { self.state.output }
}

impl<S> Job<S> {                  // shared, valid in every state
    pub fn prompt(&self) -> &str { &self.prompt }
    pub fn config(&self) -> &JobConfig { &self.config }
}
```

Full compiling source is in `job.rs`.

## Why this shape

**`submit()` lives only on `Job<Draft>`, and it takes `self` by value.** Two
guarantees fall out of that. First, no other state has a `submit` method, so you
can't re-submit a `Job<Running>` or a `Job<Done>` — the method isn't there.
Second, because `submit` consumes the draft, the draft value is *gone* after the
first call; even a second `submit` on the same draft fails to compile because the
binding was moved. There's no live handle to re-submit through.

**`output()` lives only on `Job<Done>`.** A `Job<Draft>` and a `Job<Running>`
have no way to expose the image — the accessor doesn't exist on those types. You
cannot read a result that hasn't been produced. The transition that *makes* a
`Job<Done>` (`await_output`) is the only thing that puts an `Image` in your hands,
and it only runs after the job finishes.

**Transitions consume `self`.** Each step (`submit`, `await_output`) takes the
job by value and returns the next type, so a stale handle in the old state can't
linger — you can't keep poking a `Job<Running>` after you've awaited it.

## The Pixhaus-specific part: state carries its own data

The textbook write-up of this pattern keeps one struct with `Option` fields —
`handle: Option<JoinHandle>`, `output: Option<Image>` — and `unwrap`s or
`unreachable!`s them once the state "guarantees" they're set. That's banned here
twice over: `unwrap`/`expect`/`unreachable!`/`panic!` are clippy-denied
workspace-wide, and the `Option` defeats the point — you proved the invariant in
the type, then re-checked it at runtime.

Instead the data lives **inside the state type**. `Running` owns the
`JoinHandle`; `Done` owns the `Image`. The field exists exactly when the state
that owns it exists, so `self.state.handle` and `self.state.output` are always
valid where they're used — no `Option`, nothing to unwrap. `Draft` carries no
data, so it stays a bare marker.

State types are `pub` here for the eval, but in the crate you'd keep `Running`
and `Done` constructible only through the transitions (don't let downstream code
forge a `Job<Done>` it never ran) — export `Job` and `Job<Draft>::new`, and seal
or crate-private the state markers.

## On the stubs and the real types

The file stubs `JoinHandle` and `Image` so it compiles with `rustc` alone. In the
crate:

- `submit` takes `&tokio::runtime::Handle` and calls `rt.spawn(async move { run_inference(prompt, config).await })`.
- `await_output` is `async`, `.await`s the real `JoinHandle`, and returns
  `Result<Job<Done>, JobError>` — a join/cancel failure is a real runtime error,
  so it surfaces as a typed `thiserror` error, not a panic. (The type system
  already removed the *illegal* transitions; the only failures left are genuine
  runtime ones, and those get a `Result`.) The state-machine shape is identical;
  only the spawn/await plumbing changes.

## Is type-state the right call here?

Yes — it clears every bar in the skill's decision list: the states are a small
fixed set known at compile time, the transitions are code-driven, and reading an
unfinished result or double-submitting are real bugs with costly failure modes
(a torn AI request, a half-read buffer). The bug class it removes is nameable,
which is the skill's final check. If all three operations were legal in all three
states, a plain `enum` + `match` would be simpler and you'd skip this. They
aren't, so the verbosity earns its keep.

## Verification

- `rustc --edition 2024 --test job.rs` — compiles clean; 3 tests pass.
- `clippy-driver --edition 2024 --test -Dwarnings -Dclippy::unwrap_used -Dclippy::expect_used -Dclippy::panic -Dclippy::unreachable` — exit 0, no findings.
- Negative checks: `r.output()` on a `Job<Running>` and a second `r.submit()` on
  a `Job<Running>` each fail with `error[E0599]: no method named ... found for
  struct Job<Running>` — the compiler, not a runtime status field, rejects them.
