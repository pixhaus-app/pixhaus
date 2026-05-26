# Backend registry: what to wrap it in

## Recommendation

Wrap it in `Arc<parking_lot::RwLock<BackendRegistry>>`.

```rust
use std::sync::Arc;
use parking_lot::RwLock;

let registry: Arc<RwLock<BackendRegistry>> = Arc::new(RwLock::new(BackendRegistry::new()));

// UI thread, every frame: many concurrent readers, no allocation, no contention with each other
let names: Vec<String> = registry.read().backend_ids();

// Settings flow, occasionally: one writer, exclusive
registry.write().insert(id, backend);

// Background task: clone the Arc into the spawned closure, then look up by id
let registry = Arc::clone(&registry);
rt.spawn(async move {
    // look up + clone out what you need, THEN drop the guard before any .await
    let backend = registry.read().get(&id).map(Arc::clone);
    if let Some(backend) = backend {
        backend.run(request).await // guard already dropped — future stays Send
    } else {
        Err(BackendError::Unknown(id))
    }
});
```

The registry itself holds the adapters as `Vec<Box<dyn Backend>>` (or a
`HashMap<BackendId, Arc<dyn Backend>>` if you want to hand a single adapter out to a
task — see the await note below). That heterogeneous-trait-object store is the one place
`Box<dyn Trait>` in a collection is the right call.

## Why this and not the alternatives

This is genuinely shared mutable state across threads. Three independent holders need it
alive at once: the egui UI thread, the settings flow, and any number of tokio background
tasks. That rules out plain ownership with `&`/`&mut` — no single owner can lend borrows
to a thread it doesn't control. So you need shared ownership (`Arc`) plus a synchronizer.

The access pattern decides the synchronizer. Reads dominate massively: the UI lists
backends every single frame, while writes happen only when a human changes settings.
That is the textbook `RwLock` case — many concurrent readers *or* one exclusive writer.
The UI's per-frame reads never block each other; the rare write takes the lock exclusively
for the moment it mutates. The skill calls out this exact scenario by name: "a backend
registry the UI reads constantly and a config flow writes occasionally," and labels
`Arc<parking_lot::RwLock<...>>` the *legitimate* `Arc<lock<>>` — real cross-thread shared
mutation, not the reflexive wrap around single-owner data.

## What I'd reject, and why

- **`Arc<Mutex<T>>` (the reflex).** A `Mutex` serializes readers against each other, so
  every per-frame UI listing would contend with every other reader for no reason — reads
  don't conflict. `Mutex` is the wrong fit when reads vastly outnumber writes. Reserve it
  for write-heavy or balanced access; this is read-heavy.

- **`std::sync::RwLock`.** Use `parking_lot`: faster, no lock poisoning to handle, cleaner
  guard API. This is a workspace convention, not a preference.

- **Plain `Arc<T>` with no lock.** That gives shared *reads* only. The settings flow needs
  to add and remove backends, so there's real shared mutation — you need the lock. (If the
  set of backends were fixed at startup and never changed, `Arc<BackendRegistry>` with no
  lock would be correct. It isn't — adds/removes are in the requirements.)

- **`RefCell` / `Rc`.** Single-threaded only. `Rc` is `!Send` and won't cross into a tokio
  task; `RefCell` is `!Sync` and trades a compile error for a runtime panic. Both are out
  the moment background tasks are involved.

- **`Mutex<Option<T>>` / lazy-static hacks** for the registry handle — not relevant here;
  the registry is built at startup and owned, not lazily initialized on first access.

- **Raw pointers.** Forbidden workspace-wide (`#![forbid(unsafe_code)]`). Not a
  consideration.

## The one trap to avoid: the lock and the await

A `parking_lot` read/write guard is `!Send`. If you hold it across `.await` — e.g. call
`registry.read().get(&id).unwrap().run(req).await` with the guard still live — the future
becomes `!Send` and `tokio::spawn` rejects it, and you'd also serialize every backend run
through the registry lock and risk deadlock. Never hold the lock across an await.

The fix is in the example above: under the lock, look up the backend and clone out a cheap
handle to it (an `Arc<dyn Backend>`, or whatever owned thing the request needs), then let
the guard drop at the end of that statement. Do the `.await` afterward with no lock held.
If the registry stores `HashMap<BackendId, Arc<dyn Backend>>`, the lookup is a one-line
`Arc::clone` out — cheap, and it keeps the await clean. (Full footgun detail is in the
tokio skill.)
