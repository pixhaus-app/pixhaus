---
name: pixhaus-tokio-util
description: >
  Use when reaching past tokio's core API for the glue around Pixhaus's
  background tasks: cancelling in-flight work (CancellationToken, DropGuard),
  tracking or auto-aborting a fleet of spawned tasks (TaskTracker,
  AbortOnDropHandle, JoinMap), bridging sync codecs and async I/O
  (SyncIoBridge, ReaderStream, StreamReader), framing a byte stream into
  messages (Framed, the Decoder/Encoder traits, LinesCodec /
  LengthDelimitedCodec), scheduling delayed items (DelayQueue), or bridging a
  futures-io crate into tokio (compat). Trigger this for ANY "cancel the AI
  request / the export", "abort the task when this is dropped", "only the
  latest request per key", "feed an async reader into a sync zstd/image
  decoder", "turn the HTTP byte stream into an AsyncRead", "debounce the
  autosave", "wait for all background tasks before exit", or "frame this
  socket / streaming response" work, even when the user never says
  "tokio-util". tokio-util complements the binary-owned tokio runtime; its
  feature flags are all opt-in and a few types deadlock if used on the wrong
  thread, so reach for this skill rather than guessing.
---

# tokio-util for Pixhaus

tokio-util is the utilities crate that sits beside tokio's core. It does not
replace anything in `tokio` — it adds the higher-level building blocks the
runtime leaves out: cancellation tokens, task-fleet management, the
`AsyncRead`/`AsyncWrite` ↔ `Stream`/`Sink` framing layer (`codec`), the
sync ↔ async I/O bridges (`io`), and a delay queue (`time`).

In Pixhaus this is the plumbing around the rule from CLAUDE.md: the binary owns
one tokio runtime, the egui update loop owns the document on one thread, and
background work returns results over channels the loop drains each frame. The
tokio-util types are how you cancel that background work, bound it, and bridge
it to the sync encoders (`zstd`, the `image` crate) that run inside
`spawn_blocking`. See `pixhaus-tokio` for the runtime itself
(spawn, channels, `spawn_blocking`), `pixhaus-rust-conventions` for the async
rules, and `pixhaus-pollster` for the no-runtime sync→async boundary —
tokio-util is the *has-a-runtime* counterpart of pollster, layered on top of
`pixhaus-tokio`.

This skill is the floor: the version pin, the feature map (everything is
opt-in), the handful of facts that prevent the recurring bugs, and how the
pieces land in a pixel-art editor. For the full method surface of a type, open
the matching file in `references/` — don't guess signatures from memory, the
references are derived from docs.rs 0.7.18.

## Version, license, features — pin these

```toml
# Pull only what you use. tokio-util has ZERO default features.
tokio-util = { version = "0.7", features = ["rt", "io-util", "time", "codec"] }
```

| Crate | Version | License |
|---|---|---|
| `tokio-util` | 0.7.18 | `MIT` |

MIT clears the [[project-v2-native-restart]] MIT lock and `cargo deny`. The
crate is versioned independently of `tokio` but follows semver, so pin it on its
own. When you bump it, re-verify the references against docs.rs — see
[[feedback-dep-upgrades]].

**Everything is opt-in — `default = []`.** A bare `tokio-util = "0.7"` gives you
only the always-compiled modules: `sync` (CancellationToken, PollSender,
PollSemaphore, ReusableBoxFuture), `either`, `future`, and the `bytes`
re-export. There is no `sync` feature — those types are free. Everything else is
gated:

| Feature | Unlocks | Implies |
|---|---|---|
| `rt` | `task::{TaskTracker, AbortOnDropHandle, LocalPoolHandle, JoinQueue}`, `context` | `tokio/rt`, `tokio/sync`, `futures-util` |
| `join-map` | `task::JoinMap` | `rt` + `hashbrown` |
| `io` | `io::{ReaderStream, StreamReader, InspectReader/Writer, CopyToBytes, SinkWriter}` | — |
| `io-util` | adds `io::SyncIoBridge`, `read_exact_arc` | `io` + `tokio/io-util` + `tokio/rt` |
| `codec` | `codec::*` (Framed, Decoder/Encoder, built-in codecs) | — |
| `time` | `time::DelayQueue` | `tokio/time` + `slab` |
| `net` | `net::Listener`, `udp::UdpFramed` | `tokio/net` (UdpFramed also needs `codec`) |
| `compat` | `compat::*` (futures-io bridge) | `futures-io` |
| `full` | all of the above | (does not add `tracing`) |

Enabling `rt`/`io-util`/`time`/`net` flips on the matching `tokio` feature, so
your `tokio` dependency must carry it too. Don't reach for `full` — it bloats
the build with codecs and listeners a desktop editor never frames.

## The mental model: four facts that cause most bugs

1. **tokio-util is the *runtime-present* side; pollster is the *runtime-absent*
   side.** Use tokio-util inside the binary's tokio runtime — on a tokio task,
   in `spawn_blocking`, anywhere `tokio::spawn` works. The moment you have no
   runtime (the `render` crate's tests/benches/examples), you want
   `pixhaus-pollster`, not this. Mixing them up is the first wrong turn.

2. **`SyncIoBridge` must run on a `spawn_blocking` thread, never a runtime
   worker.** Every blocking call it makes internally does `Handle::block_on` on
   the captured runtime — calling that from an async worker thread panics or
   deadlocks. This is the single sharpest edge in the crate. Build the bridge,
   move it into `spawn_blocking`, hand it to the sync `zstd`/`image` decoder
   there. See `references/io-bridges.md`.

3. **`CancellationToken`s form a tree, and propagation is one-directional.**
   `parent.cancel()` cancels the parent and every descendant; a child cancelling
   never touches its parent. `.clone()` shares the *same* token —
   `.child_token()` makes an independently-cancellable scope. Hand each subtask a
   child token so you can cancel one job without nuking the rest.

4. **`TaskTracker` and `AbortOnDropHandle` are opposites on drop.** A
   `TaskTracker` only *tracks* — dropping it leaves tasks running, and `wait()`
   resolves only after you `close()` it. An `AbortOnDropHandle` *aborts* its task
   when dropped. Pick by intent: graceful drain at shutdown → `TaskTracker`;
   "this task dies with its owner" → `AbortOnDropHandle`. A plain `JoinHandle`
   does neither (it detaches). See `references/task-helpers.md`.

## Rules that prevent the recurring bugs

- **Never block the egui thread, even with tokio-util.** None of these types
  make blocking safe on the UI thread. `CancellationToken::cancelled()`,
  `DelayQueue`, and `TaskTracker::wait()` are futures — `await` them on a tokio
  task, then signal the loop with a channel + `ctx.request_repaint()`. See
  `pixhaus-egui` for the drain-each-frame pattern.
- **Reserve before you send with `PollSender`.** `poll_reserve` to
  `Poll::Ready(Ok(()))` *then* `send_item` — calling `send_item` without a prior
  successful reserve panics. `PollSender` is for driving an mpsc `Sender` from a
  `poll`/`Sink` context; for ordinary task→loop messaging a plain
  `tokio::mpsc::Sender` is simpler.
- **Map your stream error into `io::Error` before `StreamReader`.** It requires
  the stream's error type to be `Into<std::io::Error>`. A `reqwest::Error` (AI
  backend body stream) won't satisfy that until you `.map_err(...)` it.
- **Bound any codec fed by untrusted bytes.** `LinesCodec::new()` and
  `AnyDelimiterCodec::new()` buffer without limit — a peer that never sends the
  delimiter forces unbounded growth. Use the `*_with_max_length` constructors;
  `LengthDelimitedCodec` defaults to an 8 MB cap.
- **Don't `unwrap` the bridged results.** The workspace no-`unwrap` rule applies:
  `SyncIoBridge` I/O, `DelayQueue::remove` (panics on a stale key — prefer
  `try_remove`), and `JoinMap`/`TaskTracker` results all return values to handle
  with `?` or a `thiserror` variant. `unwrap`/`expect` are test/example only.

## Pixhaus applications

Where the types land in a native pixel-art editor on the binary's tokio runtime:

- **Cancel in-flight AI generation, export, or a slow file load with a
  `CancellationToken`.** Store one per cancellable job (or a `child_token()` off
  a root), hand it to the task, and either `select!` on `token.cancelled()` or
  wrap the work in `token.run_until_cancelled(fut)` — `None` back means it was
  cancelled. A `drop_guard()` cancels automatically if the owning scope unwinds.
  See `references/cancellation-and-sync.md`.
- **Bridge the `.pixhaus` save path (rmp-serde + zstd) and PNG/sprite-sheet I/O
  with `SyncIoBridge`.** The encoders are sync `std::io` types; wrap the async
  file in `SyncIoBridge`, run it inside `spawn_blocking`, stream through. For an
  AI backend's streaming HTTP body, `StreamReader` turns the byte stream into an
  `AsyncRead`. See `references/io-bridges.md` and `pixhaus-zstd`.
- **Manage the background-task fleet with `TaskTracker` + `AbortOnDropHandle`.**
  `TaskTracker` gives graceful shutdown on `eframe`'s `on_exit`
  (`close(); wait().await`); `AbortOnDropHandle` ties a per-panel preview task to
  the panel's lifetime. See `references/task-helpers.md` and `pixhaus-eframe`.
- **Use `JoinMap` for "only the latest request per key".** Re-spawning a key
  aborts the previous task for that key — exactly right for a live thumbnail or
  layer-preview that re-renders as the user drags. `join_next()` hands you back
  the key with each result. (`features = ["join-map"]`.)
- **Debounce autosave and throttle with `DelayQueue`.** Insert the document-dirty
  marker with a delay; resetting the key on each edit collapses a burst of
  strokes into one save. The same queue handles cache-entry expiry. See
  `references/time-compat-misc.md`.
- **Frame a streaming protocol with `codec`.** If an AI backend (Ollama,
  ComfyUI) speaks newline-delimited JSON or length-prefixed frames over a socket,
  wrap it in `Framed` with `LinesCodec` / `LengthDelimitedCodec` and treat it as
  a `Stream` + `Sink`. See `references/codec.md`.

## References

Open the file for the area you're working in; each is a dense API reference for
tokio-util 0.7.18, with load-bearing signatures checked against docs.rs.

| File | Covers |
|---|---|
| `references/cancellation-and-sync.md` | `sync` module — `CancellationToken` (cancel, cancelled, child_token, run_until_cancelled, drop_guard), `DropGuard`, `PollSender` (reserve-then-send), `PollSemaphore`, `ReusableBoxFuture` |
| `references/task-helpers.md` | `task` module (`rt`) — `TaskTracker` (close/wait), `AbortOnDropHandle`, `JoinMap` (`join-map`), `LocalPoolHandle`/`spawn_pinned`, drop-behavior comparison |
| `references/io-bridges.md` | `io` module — `SyncIoBridge` (the spawn_blocking rule), `ReaderStream`, `StreamReader`, `InspectReader/Writer`, `CopyToBytes`, `SinkWriter`, the free `*_buf` fns |
| `references/codec.md` | `codec` module — the `Decoder`/`Encoder` traits you implement, `Framed`/`FramedRead`/`FramedWrite`/`FramedParts`, `LinesCodec`/`BytesCodec`/`AnyDelimiterCodec`/`LengthDelimitedCodec` + the length-delimited `Builder` |
| `references/time-compat-misc.md` | `time::DelayQueue`, `compat` (futures-io bridge), `net::Listener`, `context::{TokioContext, RuntimeExt}`, `either::Either`, the full feature table |

A standing caution: the references record the 0.7.18 API faithfully, but a few
deep signatures were flagged during research as inferred rather than read off the
item page (noted inline as "(verify)"). When one is load-bearing for what you're
building, confirm it against https://docs.rs/tokio-util/0.7.18/tokio_util/ before
depending on it.
