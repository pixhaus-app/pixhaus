# Wrapping the AI backend registry

## Recommendation

Wrap it in `Arc<RwLock<HashMap<BackendId, Arc<dyn Backend>>>>`.

Concretely:

```rust
use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::RwLock; // or std::sync::RwLock

/// A stable id for a backend (newtype, not a bare String).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct BackendId(String);

/// The adapter trait. Object-safe so the registry can hold any backend
/// behind a pointer.
pub trait Backend: Send + Sync {
    fn id(&self) -> &BackendId;
    fn display_name(&self) -> &str;
    // run(...) etc.
}

#[derive(Clone, Default)]
pub struct BackendRegistry {
    inner: Arc<RwLock<HashMap<BackendId, Arc<dyn Backend>>>>,
}

impl BackendRegistry {
    /// Cheap: clones the metadata the UI needs, drops the lock immediately.
    pub fn list(&self) -> Vec<(BackendId, String)> {
        self.inner
            .read()
            .iter()
            .map(|(id, b)| (id.clone(), b.display_name().to_owned()))
            .collect()
    }

    /// Hand a background task its own owned handle to one backend.
    pub fn get(&self, id: &BackendId) -> Option<Arc<dyn Backend>> {
        self.inner.read().get(id).cloned()
    }

    pub fn insert(&self, backend: Arc<dyn Backend>) {
        self.inner.write().insert(backend.id().clone(), backend);
    }

    pub fn remove(&self, id: &BackendId) -> Option<Arc<dyn Backend>> {
        self.inner.write().remove(id)
    }
}
```

The two pointer layers each do a distinct job, so pick them deliberately:

- **`Arc<...>` on the outside** — the registry handle itself is shared across
  threads. The egui update loop holds a clone; every spawned tokio task gets a
  clone. `Clone` is a refcount bump, so `BackendRegistry` is `Clone` and you pass
  copies around freely instead of threading borrows through `'static` task
  boundaries.
- **`RwLock` in the middle** — access is overwhelmingly read (the UI lists
  backends every frame; tasks look one up per request) and rarely write (a
  settings flow adds/removes one occasionally). That read-heavy, write-rare shape
  is exactly what a reader-writer lock is for: any number of concurrent readers,
  exclusive access only for the rare mutation.
- **`Arc<dyn Backend>` on the values** — `Backend` is a trait with multiple
  concrete implementors (Anthropic, OpenAI, Replicate, Ollama, ComfyUI,
  Stability), so the map stores trait objects. `Arc` (not `Box`) so `get()` hands
  a task an owned, refcounted handle to one backend without cloning the adapter
  or holding the registry lock for the request's lifetime.

## Why this and not the alternatives

**Why not `Mutex` instead of `RwLock`?** A `Mutex` serializes readers against each
other. The UI thread listing backends would block a background task's lookup and
vice versa, for no reason — neither mutates. With reads dominating, `RwLock` lets
them run concurrently and only the rare add/remove takes the exclusive path. If
the registry were write-heavy or held for long critical sections the calculus
flips, but this is the canonical read-mostly case.

**Why not `Arc<Mutex<HashMap<...>>>`?** Same point: workable, but it throws away
free read concurrency. The repo's own guidance is to avoid reaching for
`Arc<Mutex<>>` reflexively; here a `RwLock` is the better-fitting primitive.

**Why `parking_lot::RwLock` over `std::sync::RwLock`?** `parking_lot` is smaller,
faster, has no poisoning (so `read()`/`write()` return guards directly instead of
`Result` you'd have to handle or unwrap — and unwrap is banned outside tests), and
is fair about writers, which matters so the occasional add/remove doesn't starve
behind a constant stream of UI reads. `std::sync::RwLock` is fine if you want zero
extra deps; you'll just unwrap-or-handle the poison `Result`, and on some
platforms it can starve writers. Either works — the structure is what matters.

**Critical: don't hold the lock across `.await`.** A `parking_lot` guard is not
`Send` and holding any lock guard across a suspension point is a deadlock and
correctness hazard. The `get()` method clones out an `Arc<dyn Backend>` and drops
the read guard before returning — the task then `.await`s on the backend it owns,
with no lock held. Likewise `list()` collects owned data and releases the guard.
Never do `registry.inner.read().get(id).run(...).await` with the guard live.

**Why `RwLock`, not a lock-free / copy-on-write scheme like `arc-swap` or
`Arc<RwLock<Arc<Map>>>`?** You could, and for a truly hammered read path it's
faster. But the read rate here is per-frame and per-request, not millions/sec, and
the registry is tiny. `RwLock` is simpler, obviously correct, and removes a
dependency and a layer of indirection. Reach for `arc-swap` only if profiling
shows the lock is actually contended — it won't be at this scale.

**Why `Arc<dyn Backend>` values, not `Box<dyn Backend>`?** `Box` is single-owner.
To run a request you'd either have to keep the registry locked for the whole
async call (forbidden — see above) or move the backend out. `Arc` lets the
registry keep its copy while a task holds its own, and multiple concurrent tasks
can share one backend. The adapters are effectively immutable config + an HTTP
client, so sharing is exactly right.

**Bound on the trait:** `Backend: Send + Sync` is required. The values cross the
thread boundary into spawned tokio tasks, so `Arc<dyn Backend>` must be `Send +
Sync`, which means the trait carries those bounds. If a backend holds non-`Sync`
interior state, wrap that state, not the whole registry.

## When you'd deviate

- If background tasks needed to **mutate** a backend's internal state (not just
  call it), that state would need its own interior synchronization
  (`Arc<Mutex<...>>` *inside* the adapter), kept off the registry lock.
- If you wanted the UI to react to add/remove without polling each frame, layer a
  change notification (a `tokio::sync::watch` of a version counter, or a channel
  the loop drains) on top — the storage choice doesn't change.
