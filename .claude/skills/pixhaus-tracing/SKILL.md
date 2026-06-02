---
name: pixhaus-tracing
description: Use when writing, reviewing, or debugging logging, tracing, or diagnostics anywhere in Pixhaus — adding a `tracing` event (`info!`/`warn!`/`error!`/`debug!`/`trace!`), reaching for `#[instrument]` or a span, timing expensive work, setting log levels or `RUST_LOG`, wiring the subscriber, the rolling `pixhaus.log` file, the OS log directory, or the future diagnostic bundle. Trigger whenever the question is "why isn't my log showing", "how do I time this", "where do the logs go", "should this be info or debug", "instrument this function", or "log this flow" — even when the user doesn't say "tracing". The app owns the ONE subscriber (`app/src/diagnostics.rs`); libraries emit and never configure. For instrumenting async tasks pair with `pixhaus-tokio`; for the log dir, `pixhaus-directories`; for not leaking API keys, `pixhaus-keyring`; for benchmarks/profiling tools, `pixhaus-performance`.
---

# Pixhaus tracing

Pixhaus uses structured `tracing` from the start. Logs are how you debug a native
app you can't attach a console to on a user's machine, how a diagnostic bundle gets
built, and — through span durations — how you find what's slow. This skill is the
floor for every log line and every span in the repo.

The one architectural fact behind every rule: **the binary owns exactly one
subscriber; libraries only emit.** The subscriber lives in `app/src/diagnostics.rs`
([`init_tracing`]). Every crate under `crates/` and `modules/` calls
`tracing::info!` / `#[instrument]` and never installs, configures, or assumes a
subscriber. A library that calls `tracing_subscriber` is a bug.

## 1. The level ladder

Pick the level by who needs to see it and when, not by how you feel about the line.

| Level | Use for | Default visibility |
|---|---|---|
| `error!` | an operation failed and the user is affected — save failed, AI request errored, asset corrupt | always |
| `warn!` | recoverable wrongness — unknown layout id skipped, unknown file extension, falling back | always |
| `info!` | one-line lifecycle landmarks — startup, shutdown, renderer init, module registration, project open/save, job start/finish | always |
| `debug!` | developer detail useful while building — state transitions, command names, resolved ids | debug builds + Pixhaus crates |
| `trace!` | firehose — per-item loop detail, only when chasing a specific bug | off unless `RUST_LOG` raises it |

The default filter (in `diagnostics.rs`) is: debug builds run every `pixhaus_*`
crate at `debug` and the GPU/windowing stack (`wgpu`, `wgpu_core`, `wgpu_hal`,
`naga`, `winit`) at `warn`; release builds run Pixhaus at `info`. `RUST_LOG`
overrides the default entirely — `RUST_LOG=pixhaus_app=trace` to chase one crate,
`RUST_LOG=wgpu=debug` to debug the GPU layer. Targets use underscores
(`pixhaus_app`, not `pixhaus-app`) — that is the crate name tracing sees.

Don't log at `info!` what is really `debug!`: `info` is for landmarks a support
engineer reads in a bundle, not for every frame of internal churn.

## 2. `#[instrument]` on fallible and expensive functions

Reach for `#[instrument]` on a public function that does real work and can fail or
take time — a job body, a command apply, an encode, a load. It opens a span for the
call, so the function's arguments and duration show up in the log automatically and
any event inside the function is nested under it.

```rust
use tracing::instrument;

// A span named "encode_png" with width/height as fields; the pixel buffer is skipped.
#[instrument(skip(pixels), fields(width, height))]
pub fn encode_png(pixels: &[u8], width: u32, height: u32) -> Result<Vec<u8>, Error> {
    // ...
}
```

**Always `skip` big or noisy arguments.** `#[instrument]` records every argument
with `Debug` by default, so a `&[u8]` pixel buffer, an `egui::Context`, or the whole
`Host` would dump megabytes into the log on every call. Use `skip(pixels)` for the
named ones or `skip_all` and then add back just the cheap derived fields with
`fields(width = pixels.len() / 4)`. Skipping is the default posture for anything
that isn't a small id, count, or enum.

Don't instrument trivial getters or hot per-pixel helpers (see rule 3).

## 3. Spans for expensive work — and never on the hot path

Span durations are how Pixhaus measures performance today (see rule 11). Put a span
around the coarse, expensive operations the runtime companion lists as performance
spans:

- frame render (behind a level/feature — never every frame by default)
- canvas composite
- thumbnail batch
- texture upload
- project load index
- lazy asset load
- export encode
- compression
- AI request
- provider response
- model warmup

**Never put a span inside a per-pixel or per-scanline loop.** A span has real setup
cost; at 8K a per-pixel span is millions of allocations and the log is unreadable
besides. Instrument the *operation* ("composite this dirty region"), not the pixels
inside it. The 8K perf constraint means the hot path stays span-free; the dirty-region
boundary is the right granularity. `render` and `core` keep spans off the inner loop
entirely — coarse only.

## 4. Structured fields, not interpolated strings

Attach data as fields, not baked into the message. Fields are queryable and typed;
an interpolated string is a grep target that drifts.

```rust
// DO — structured: ?x is Debug, %x is Display, bare `field = value` for the rest.
tracing::info!(layer = %id, w = width, h = height, "uploaded texture");
tracing::warn!(?path, "unknown file extension; skipping import");

// DON'T — everything melted into the message.
tracing::info!("uploaded texture for layer {id} at {width}x{height}");
```

`%x` uses `Display`, `?x` uses `Debug`, and `field = expr` records the value
directly. Keep the message a short, constant human label; put the variables in
fields.

## 5. Target and module naming

Events are tagged with their module path as the target by default
(`pixhaus_io::png`), which is what the `EnvFilter` matches on — so the default
target is almost always right. Override with `target: "..."` only for a deliberate
cross-cutting channel (e.g. a `"perf"` target you want to filter independently).
Don't set a target just to rename the module.

## 6. The always-trace flow list

The runtime companion (`docs/pixhaus_rust_runtime_state_concurrency_companion.md`,
"Always Trace") names the flows that must produce a log line. Most do not exist yet
— instrument them as they land:

- startup, shutdown — **done** (`app/src/main.rs`)
- renderer initialization + GPU backend choice — **done** (`PixhausApp::new`)
- module registration — **done** (`build_host`)
- workspace activation
- project open / close / save, autosave
- import / export jobs
- AI provider jobs, local model worker state
- command execution failures
- save migration, corrupt asset detection

When you build one of these flows, add its log line in the same change — that is the
point of the list.

## 7. No `println!` / `eprintln!` / `dbg!` in production

Anything that bypasses the subscriber doesn't reach the file, can't be filtered, and
can't be turned off. The workspace clippy config already denies-on-warn
`print_stdout`, `print_stderr`, and `dbg_macro`, so the Stop gate catches these — but
the rule is to reach for `tracing` by reflex, not to get caught. A throwaway
`println!` while debugging becomes a `debug!` or `trace!` before commit, or it comes
out.

## 8. The `WorkerGuard` caveat

`init_tracing` returns a `tracing_appender::non_blocking::WorkerGuard`. The file sink
is non-blocking — a background thread does the actual writing — and the guard flushes
and shuts that thread down on drop. So the guard MUST live for the whole program:

```rust
let _guard = diagnostics::init_tracing(&log_dir);   // held for all of main
// ...
// let _ = diagnostics::init_tracing(&log_dir);      // BUG: drops immediately, log tail lost
```

`let _ = ...` drops the guard at the end of the statement, which flushes and stops
the writer before the app even starts — you lose most of the log. This is the single
highest-risk detail in the logging setup; it is documented in the code, in
`app/CLAUDE.md`, and here.

## 9. The setup

One subscriber, two sinks, one filter, owned by the app:

- **Console (stderr, ANSI on)** + **rolling daily file (`pixhaus.log`, ANSI off)** —
  color codes would corrupt the file, so only the console is colored.
- **One shared `EnvFilter`** gates both sinks, so `RUST_LOG` controls everything at
  once.
- **The `log` bridge** is `tracing_log::LogTracer::init()`, which forwards `log`-crate
  records from wgpu, winit, and naga into tracing so they land in the same place.
- **The log dir** comes from `pixhaus_platform::log_dir()` — the OS-standard log
  location, created on demand (see `pixhaus-directories`). On Windows that is
  `%LOCALAPPDATA%\Pixhaus\Pixhaus\data\logs\`.

Libraries never see any of this. They emit; `app` configures. If you find yourself
wanting a second subscriber or a per-crate logger config, the answer is an
`EnvFilter` directive, not a new subscriber.

## 10. The diagnostic bundle and the secrets rule

Pixhaus will eventually assemble a diagnostic bundle (recent logs, app version,
OS/platform, renderer backend + adapter, enabled modules, a provider-config summary
without secrets, recent job failures). It doesn't exist yet, but design for it: the
log already carries version, OS, arch, backend, adapter, and module count, which is
most of the bundle.

**Never log an API key, token, or secret — ever, at any level.** Provider modules log
that a request happened and how long it took, never the key or the raw prompt if it
carries private content. Secrets live in the OS credential vault, not the log (see
`pixhaus-keyring`). A leaked key in a user-submitted log file is a real incident.

## 11. Profiling: span durations now, dedicated tools later

For now, **profiling means reading span durations.** `#[instrument]` on the expensive
operations gives you per-call timing in the log without adding a dependency. That is
the whole profiling story today, and it is enough to find the obvious bottlenecks.

The next step, when span timing isn't enough, is dedicated tooling — `puffin` /
`puffin_egui` for in-app frame profiling, `tracing-tracy` for a timeline, `criterion`
/ `divan` for benchmarks of image ops and load/save. **None of these are in the tree.**
Adding one is a deliberate decision (a dependency, a `cargo deny` check, a skill load),
not a reflex. See `pixhaus-performance` before reaching for a profiler.

## Cross-references

- `pixhaus-tokio` — instrument spawned async tasks; carry the span across the
  `.await`, and remember the result crosses back over a channel.
- `pixhaus-directories` — `pixhaus_platform::log_dir()` and the path-not-created
  gotcha behind the log directory.
- `pixhaus-keyring` — where secrets actually live, so they never reach the log.
- `pixhaus-performance` — benchmarks and profilers, the next profiling step.
- `pixhaus-rust-conventions` — error policy, the no-`unwrap` rule, the print lints.
- `app/CLAUDE.md` — the subscriber's boundaries; `app/src/diagnostics.rs` — the code.
