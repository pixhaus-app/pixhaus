# Spawning a `pixhaus_io` decode and sending the result to the egui loop

## The actual cause of the `Send` error

`tokio::spawn` requires the future *and its output* to be `Send + 'static`. The
output here is `Result<DecodedThing, pixhaus_io::Error>`, so the whole error type
has to be `Send` (really `Send + Sync + 'static` once it travels across threads
and gets stored).

A thiserror-derived enum is `Send + Sync` automatically — *as long as every field
is*. So the fix is almost never at the spawn site or the channel; it's in the
error type's fields. The usual culprit is one of:

- a non-`Send` source wrapped in a variant — an `Rc`, a `RefCell`, a raw
  `Box<dyn Error>` with no `Send + Sync` bound, or an FFI/handle type;
- a `Box<dyn std::error::Error>` field (also forbidden in our public APIs — define
  a real enum instead);
- a borrowed field tying the error to a lifetime, so it isn't `'static`.

Keep `pixhaus_io::Error`'s fields to owned, thread-safe data and the `Send` error
disappears on its own. Do **not** paper over it with `#[async_trait(?Send)]`, a
`Box<dyn Error>`, or `spawn_blocking` "to dodge `Send`" — `spawn_blocking`'s
closure output must be `Send` too, so it doesn't help, and the bound is telling you
something real. If a genuinely non-`Send` source is unavoidable, that's a design
question to surface, not to escape-hatch.

One more point that decides `spawn` vs `spawn_blocking` here: a decode is
**CPU-bound, synchronous work** (zstd/PNG/MessagePack). That belongs on
`spawn_blocking`, not `tokio::spawn` — a decode inside `tokio::spawn` hogs a
scheduler worker for the whole decode. The sketch below uses `spawn_blocking`.

## The error type (in `pixhaus_io`)

Nothing special is needed beyond keeping fields owned and thread-safe. A normal
thiserror enum already satisfies `Send + Sync + 'static`:

```rust
// pixhaus_io/src/lib.rs
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("I/O error")]
    Io(#[from] std::io::Error),

    #[error("zstd decompress failed at {stage}")]
    Decompress { stage: &'static str, #[source] source: std::io::Error },

    #[error("MessagePack decode failed")]
    Decode(#[from] rmp_serde::decode::Error),
    // every field above is owned + Send + Sync, so `Error` is too.
}

pub type Result<T> = std::result::Result<T, Error>;
```

If you currently have a field like `source: Box<dyn std::error::Error>` or an
`Rc<_>`, that's the thing breaking `Send` — change it to a concrete owned type (or
`Box<dyn std::error::Error + Send + Sync>` only if you truly can't name the type;
prefer a concrete variant). Defining the enum in detail is `pixhaus-thiserror`'s
job; the load-bearing rule for *this* problem is: owned, thread-safe fields only.

## The channel

It's a single decode answered once, so a `oneshot` is the right choice (you already
picked it). Carry the whole `Result` as the message — let the receiver decide what
to do with `Err`. Don't try to make the channel itself "handle" the error:

```rust
use tokio::sync::oneshot;

// the message is the decode's Result; pixhaus_io::Error is Send + Sync + 'static
type DecodeOutcome = Result<Document, pixhaus_io::Error>;
```

## The spawn side

Where this runs: from `ui` (or a command handler) on the egui thread, kicking the
work onto the binary's one runtime. The egui thread only *starts* the task and
later *drains* it.

```rust
use tokio::sync::oneshot;

impl Pixhaus {
    /// Called from the egui thread when the user opens a project.
    fn start_decode(&mut self, ctx: &egui::Context, path: std::path::PathBuf) {
        let (tx, rx) = oneshot::channel::<DecodeOutcome>();
        self.pending_decode = Some(rx); // UI owns the receiver; drained each frame
        let ctx = ctx.clone();          // cheap; used to wake the idle loop

        // Decode is CPU-bound + synchronous -> spawn_blocking, not tokio::spawn.
        self.rt.spawn_blocking(move || {
            // No `?`/unwrap here: capture the whole Result and ship it as-is.
            let outcome = pixhaus_io::load_project(&path); // -> Result<_, pixhaus_io::Error>

            // send() fails only if the UI dropped the receiver (app closing) — ignore it.
            let _ = tx.send(outcome);
            ctx.request_repaint(); // wake the loop so it drains the oneshot
        });
    }
}
```

Notes that matter:

- The error is **not logged inside the task.** We forward the full `Result` so the
  UI thread — which owns the document and the user-facing surface — decides. Logging
  at the boundary that can actually react keeps the policy in one place.
- `let _ = tx.send(..)` is correct, not a swallowed error: a `oneshot` send only
  fails when the receiver was dropped, which here means the window closed.

## The receiver-drain side

`oneshot::Receiver` has `try_recv()` — call it each frame from `logic`, never
`recv().await` (awaiting on the egui thread freezes the window). The drain is where
the `Err` is handled and logged:

```rust
use tokio::sync::oneshot::error::TryRecvError;

impl Pixhaus {
    /// Called every frame from eframe `App::update` (the logic half).
    fn drain_decode(&mut self) {
        let Some(rx) = self.pending_decode.as_mut() else {
            return; // nothing in flight
        };

        match rx.try_recv() {
            Ok(Ok(doc)) => {
                self.document = doc;          // decode succeeded
                self.pending_decode = None;   // consume the slot
            }
            Ok(Err(e)) => {
                // The decode failed. Log here, at the boundary that can react,
                // and surface it to the user. inspect_err isn't a fit — we're
                // not propagating, we're consuming.
                tracing::warn!("project decode failed: {e}");
                self.show_error_toast(format!("Couldn't open project: {e}"));
                self.pending_decode = None;
            }
            Err(TryRecvError::Empty) => {
                // Not done yet — the normal case, not an error. Leave it pending.
            }
            Err(TryRecvError::Closed) => {
                // Sender dropped without sending (task panicked/aborted). Rare; clear it.
                tracing::error!("decode task ended without a result");
                self.pending_decode = None;
            }
        }
    }
}
```

## Where to log the failure — the short answer

In the **receiver drain on the egui thread**, in the `Ok(Err(e))` arm. That's the
owner that can both record it (`tracing::warn!`) and react (toast, restore state).
Don't log inside the spawned task — the task's job is to compute and forward; if it
logged *and* the UI also reacted, you'd have the failure handled in two places.

`inspect_err` (the skill's log-and-propagate tool) isn't the right call here because
the UI thread is the *terminal* consumer of this `Result` — it's recovering, not
bubbling up. `inspect_err` is for the `... .inspect_err(|e| warn!())?` shape where
you log on the way past and still `?` it onward; the drain has nowhere further to
propagate to.

## Checklist for this specific problem

- [ ] `pixhaus_io::Error` has only owned, `Send + Sync + 'static` fields — no `Rc`,
      no bare `Box<dyn Error>`, no borrowed/`'static`-breaking field. This is the
      real fix for the `Send` error.
- [ ] Use `spawn_blocking` (CPU-bound decode), not `tokio::spawn`.
- [ ] `oneshot` carries the whole `Result<_, pixhaus_io::Error>`; the task does no
      `?`/`unwrap` on it — it forwards it.
- [ ] `let _ = tx.send(..)` and a `ctx.request_repaint()` after.
- [ ] Drain with `try_recv()` in `logic`, never `recv().await`.
- [ ] Log + user-surface the `Err` in the drain's `Ok(Err(e))` arm, not in the task.
- [ ] No `unwrap`/`expect` anywhere outside `#[cfg(test)]` (clippy denies them).
```
