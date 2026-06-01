# Spawning a decode job and returning its error to the egui loop

## What the compile error is actually telling you

`tokio::spawn` requires the future to be `Send + 'static`, because the
multi-threaded runtime can move the task across worker threads. A future is
`Send` only if every value held *across an `.await`* is `Send`. Your decode
returns `Result<DecodedImage, pixhaus_io::DecodeError>`, so that error type is
part of the task's output and lives inside the future. If `DecodeError` is not
`Send`, the whole future is not `Send`, and `spawn` rejects it.

The fix is not to wrap things or sprinkle `?Send`. It's to make the error type
`Send` at the source. That's the right call regardless — an error that has to
travel back to the UI thread over a channel must be `Send` anyway, channel or
runtime aside.

## Why the error isn't Send (the usual culprits)

A `thiserror` enum is `Send` automatically *unless one of its fields isn't*. The
things that silently poison it:

- `#[from] std::io::Error` is fine — `io::Error` is `Send`.
- A `source` or field of type `Box<dyn std::error::Error>` — **not** `Send`.
  `Box<dyn Error + Send + Sync>` is the Send-safe form.
- An `Rc<_>`, a `Cell`/`RefCell`, or a raw pointer captured in a variant — not
  `Send`.
- A third-party error from a dependency that itself isn't `Send` (rare, but it
  happens with FFI-backed crates).

Check the variant fields first. The error message from the compiler will name
the exact non-`Send` type in the "within `...`, the trait `Send` is not
implemented for `...`" note — read that note, it points straight at the field.

## Set the error type up so it's Send + Sync + 'static

In `pixhaus_io`, define the decode error with `thiserror` and keep every field
`Send + Sync`. If you need to box an opaque inner error, require the bounds:

```rust
// pixhaus_io/src/error.rs
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DecodeError {
    #[error("unsupported format: {0}")]
    UnsupportedFormat(String),

    #[error("truncated file: expected {expected} bytes, found {found}")]
    Truncated { expected: usize, found: usize },

    #[error("io error reading project")]
    Io(#[from] std::io::Error),

    // If you must carry an opaque inner error, bound it Send + Sync.
    // Box<dyn Error> alone is NOT Send and will re-break the spawn.
    #[error("decode backend failed")]
    Backend(#[source] Box<dyn std::error::Error + Send + Sync + 'static>),
}
```

`thiserror` doesn't add `Send`/`Sync` — it just derives `std::error::Error`. The
auto traits come from the fields, so the rule is: keep the fields `Send + Sync`
and the enum is `Send + Sync` for free. You don't write an explicit `impl Send`,
and you must not (that would need `unsafe`, which the workspace forbids).

If you want a hard guarantee that future edits can't regress this, add a
compile-time assertion in `pixhaus_io`'s tests:

```rust
#[test]
fn decode_error_is_send_sync() {
    fn assert_send_sync<T: Send + Sync + 'static>() {}
    assert_send_sync::<DecodeError>();
}
```

That test fails to compile the day someone adds a non-`Send` field, which is
exactly when you want to find out.

## The channel: oneshot carries the Result whole

Send the *whole* `Result` over the channel, not just the `Ok`. One message, one
outcome, success or failure. The receiver then matches on it. `tokio::sync::oneshot`
is the right primitive for a single decode → single reply.

The channel's payload must be `Send` too. Since `DecodedImage` and `DecodeError`
are both `Send + 'static`, `Result<DecodedImage, DecodeError>` is `Send`, and the
`oneshot::Sender` of that type is `Send`. Good.

## Spawn side

```rust
// shell/src/app.rs (or wherever the app owns the runtime handle)
use tokio::sync::oneshot;
use pixhaus_io::{decode_project, DecodeError, DecodedImage};

struct PendingDecode {
    // The loop polls this each frame; None until the task replies.
    rx: oneshot::Receiver<Result<DecodedImage, DecodeError>>,
}

impl App {
    fn start_decode(&mut self, path: std::path::PathBuf) {
        let (tx, rx) = oneshot::channel::<Result<DecodedImage, DecodeError>>();

        // self.rt is the tokio runtime Handle the binary owns (one owner).
        self.rt.spawn(async move {
            // spawn_blocking for the CPU-bound decode; await its JoinHandle.
            // The blocking closure returns the io Result; the JoinError from a
            // panic is mapped into our own error so the loop sees one type.
            let result = tokio::task::spawn_blocking(move || decode_project(&path))
                .await
                .unwrap_or_else(|join_err| {
                    Err(DecodeError::Backend(Box::new(join_err)))
                });

            // Receiver may be gone if the user closed the doc mid-decode.
            // That's not an error worth logging loudly — drop quietly.
            let _ = tx.send(result);
        });

        self.pending_decode = Some(PendingDecode { rx });
    }
}
```

Notes on the spawn:

- `decode_project` is CPU-bound (parsing, zstd, pixel work), so it runs inside
  `spawn_blocking`, not directly on an async worker. The outer `spawn` exists
  only to `.await` the blocking handle and forward the result.
- `tokio::task::JoinError` is `Send + Sync`, so boxing it into
  `DecodeError::Backend` keeps the error type `Send`. A panic in the decode thus
  becomes a normal `Err` the UI can show, instead of taking down a worker
  silently.
- `tx.send(result)` returns `Err(value)` if the receiver was dropped. We discard
  it with `let _ =` — a closed doc isn't a failure.
- No lock is held across the `.await`. The task owns `path` by move and touches
  no shared state.

## Receiver-drain side (egui update loop)

The update loop owns the document directly and drains the channel each frame.
`oneshot::Receiver::try_recv()` is non-blocking: it returns `Empty` while the
task is still running, the value once it lands, and `Closed` if the sender was
dropped without sending (a task panic that escaped, which shouldn't happen given
the mapping above, but handle it).

```rust
use tokio::sync::oneshot::error::TryRecvError;

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain_pending_decode(ctx);
        // ... rest of the frame: panels, canvas, etc.
    }
}

impl App {
    fn drain_pending_decode(&mut self, ctx: &egui::Context) {
        let Some(pending) = self.pending_decode.as_mut() else {
            return;
        };

        match pending.rx.try_recv() {
            Ok(Ok(image)) => {
                self.document.replace_image(image);
                self.pending_decode = None;
                ctx.request_repaint(); // new pixels to draw
            }
            Ok(Err(err)) => {
                // LOG HERE: at the UI boundary, where we have context
                // (the user, the action) and can surface it. Library code in
                // pixhaus_io returns the error; it does not log.
                tracing::error!(error = %err, "project decode failed");
                self.toast_error(format!("Couldn't open project: {err}"));
                self.pending_decode = None;
            }
            Err(TryRecvError::Empty) => {
                // Still decoding. Keep the slot, ask for another frame so we
                // poll again soon even if nothing else invalidates the UI.
                ctx.request_repaint();
            }
            Err(TryRecvError::Closed) => {
                // Sender dropped without sending — treat as a failure.
                tracing::error!("decode task ended without a result");
                self.toast_error("Decode task ended unexpectedly".to_owned());
                self.pending_decode = None;
            }
        }
    }
}
```

## Where to log

- **`pixhaus_io` (library): do not log.** It returns `DecodeError`. Logging
  inside a library duplicates messages and steals the caller's context.
- **`shell` (binary): log at the drain site**, the `Ok(Err(err))` and `Closed`
  arms above, with `tracing::error!`. That's the one place that knows it's a
  user-initiated open, can attach a toast, and owns the error's final
  disposition. Log once, where you decide what to do about it.

Use `%err` (the `Display` form) so the `thiserror` `#[error("...")]` strings come
through. If you want the source chain, `tracing::error!(error = ?err, ...)` gives
the `Debug` view, or walk `std::error::Error::source()` and log the chain.

## Summary of the moving parts

1. `DecodeError` is a `thiserror` enum whose fields are all `Send + Sync` — box
   any opaque inner as `Box<dyn Error + Send + Sync + 'static>`. A compile-time
   `assert_send_sync::<DecodeError>()` test locks that in.
2. The channel is `oneshot::channel::<Result<DecodedImage, DecodeError>>()` —
   the whole `Result` travels as one message.
3. Spawn wraps the CPU-bound decode in `spawn_blocking`, maps `JoinError` into
   `DecodeError`, and `let _ = tx.send(result)`.
4. The egui loop calls `try_recv()` each frame, handles `Ok(Ok)`, `Ok(Err)`,
   `Empty` (repaint and wait), `Closed` (treat as failure), and logs only at the
   `Err` arms.
