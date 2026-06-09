# Modern Rust: sync, io, fs, time, ptr, mem, and error APIs (1.85-1.96)

Newly stabilized systems APIs by domain — sync/atomic, io, os/path/fs, time, ptr/mem, error, hash, net — flagged for the threaded-pixel, save/load, and async-channel paths. Part of the `pixhaus-rust-modern` skill; start at its `SKILL.md` for the shortlist and the per-version cheat sheet.

The toolchain is pinned at 1.96 on edition 2024, so every std API below is usable today. This is the reference of record for std additions in `sync`, `atomic`, `io`, `os`, `path`, `fs`, `time`, `ptr`, `mem`, `error`, `hash`, and `net`. Grouped by domain. Items flagged "hot-path", "save/load", or "async" are the ones that touch threaded pixel work, file I/O, or the tokio result-channel loop directly.

The rule throughout: reach for these when they delete code or remove a footgun, not because they are new. A working `Vec::with_capacity` + `set_len` buffer does not need rewriting to use `Box::new_zeroed_slice`. The "when not to" notes say where the old way is still correct.

## sync

The `OnceLock`/`Once` wait family and `RwLockWriteGuard::downgrade` are the load-bearing additions for shared state behind `parking_lot`-or-std locks.

`OnceLock::wait` and `Once::wait`/`Once::wait_force` (1.86) block until another thread finishes initialization, instead of you spinning or wiring a `Condvar`. Use this for a lazily-built GPU capability table or a derived cache that several lanes read once and then share.

```rust
// OLD: spin or hand-roll a Condvar to wait for another thread's init
while CAPS.get().is_none() { std::hint::spin_loop(); }
let caps = CAPS.get().expect("set by now");

// NEW (1.86): block until initialized, no spin
let caps: &GpuCaps = CAPS.wait();
```

`RwLockWriteGuard::downgrade` (1.92) atomically turns a write guard into a read guard with no unlocked gap in between — no other writer can slip in. Useful when a job takes the write lock to rebuild a derived cache, then wants to keep reading it without releasing. async: this is std's `RwLock`; if the lock is held across `.await` you still want the `parking_lot` / tokio guidance, not this.

```rust
// NEW (1.92): mutate under the write lock, then keep reading as a reader
let mut w = cache.write().unwrap();
w.rebuild(&doc);
let r = RwLockWriteGuard::downgrade(w); // no window for another writer
use_derived(&r);
```

`LazyCell`/`LazyLock` gained `get`, `get_mut`, and `force_mut` (1.94): peek at the value without forcing initialization (`get`), or force-then-mutate (`force_mut`). And `From<T> for LazyCell`/`LazyLock` (1.96) builds an already-initialized cell — handy in tests that want a `LazyLock` field without paying the init closure.

`task::Waker::noop()` (1.85) is a const no-op waker. async: reach for it when manually polling a future in a test or a `pollster`-style bridge where you do not need wakeups.

`Cell::update` (1.88) applies a closure to the value inside a `Cell` in place — `dirty.update(|n| n + 1)` instead of `dirty.set(dirty.get() + 1)`. `Cell` is single-threaded, so this is for UI-thread interaction state (a dirty counter, a pending-frame flag the egui loop owns), not the cross-thread shared state the rest of this domain covers. Small, but it removes the read-modify-write boilerplate.

When not to: none of this replaces the document-owned-by-the-egui-loop model. Shared locks are for state genuinely touched off the UI thread; a single-owner `Vec<u8>` does not need any of it.

## atomic

`AtomicPtr` got pointer and bit arithmetic (1.91): `fetch_ptr_add`/`fetch_ptr_sub` (element-stride), `fetch_byte_add`/`fetch_byte_sub` (byte-stride), and `fetch_or`/`fetch_and`/`fetch_xor` (mask the address bits, e.g. a tag in the low bits). hot-path: a lock-free bump cursor over a shared tile arena stops needing a CAS loop.

```rust
// OLD: compare_exchange loop to bump a shared pointer
let mut cur = head.load(Ordering::Acquire);
loop {
    let next = unsafe { cur.add(1) };
    match head.compare_exchange_weak(cur, next, Ordering::AcqRel, Ordering::Acquire) {
        Ok(_) => break cur,
        Err(actual) => cur = actual,
    }
};

// NEW (1.91): one atomic op
let slot = head.fetch_ptr_add(1, Ordering::AcqRel);
```

Every atomic type gained `update` and `try_update` (1.95) — `AtomicPtr`, `AtomicBool`, and all the `AtomicI*`/`AtomicU*` widths. These apply a closure in a built-in CAS loop, replacing the hand-written `load` + `compare_exchange_weak` pattern. hot-path: a per-frame dirty-flag word or a packed counter updated from worker threads.

```rust
// NEW (1.95): closure-driven atomic update, loop handled for you
dirty_mask.update(Ordering::AcqRel, Ordering::Acquire, |m| m | tile_bit);
```

When not to: atomics are for genuinely shared, contended scalars. A per-thread accumulator that you merge at the end is faster as a plain local.

## io

`io::ErrorKind` gained `QuotaExceeded` and `CrossesDevices` (1.85). save/load: match these when a project save fails — `CrossesDevices` is the one you hit doing an atomic save as write-temp-then-`rename` when the temp dir and the target are on different mounts. That is exactly when you fall back to copy-then-remove.

```rust
// save/load: handle the rename-across-devices case explicitly
match std::fs::rename(&tmp, &final_path) {
    Ok(()) => {}
    Err(e) if e.kind() == io::ErrorKind::CrossesDevices => {
        std::fs::copy(&tmp, &final_path)?;
        std::fs::remove_file(&tmp)?;
    }
    Err(e) => return Err(e.into()),
}
```

Anonymous pipes are now stable: `io::pipe()` returns `(PipeReader, PipeWriter)` (1.87), with `From` conversions into `Stdio`, `OwnedFd` (Unix), and `OwnedHandle` (Windows). Use this to wire a child process's stdout straight into a reader without a tempfile — e.g. feeding an external exporter or an AI helper process. async: the pipe halves are blocking; read them on a `spawn_blocking` task and return the bytes over a channel to the egui loop, don't poll them on the UI thread.

```rust
// NEW (1.87): capture a child's stdout through an anonymous pipe
let (reader, writer) = io::pipe()?;
let child = Command::new("exporter").stdout(writer).spawn()?;
// read `reader` on a blocking task, hand the result back over a channel
```

## os

`OsStr::display()` / `OsString::display()` (1.87) give a lossy `Display` adapter, so you can put a possibly-non-UTF-8 path component into a log line or error without `to_string_lossy().to_string()` allocating. i18n note: this is user data (a filename), so it goes into the message as an argument, never inside a translation key.

```rust
// OLD: allocate a String just to format a path component
tracing::warn!("skipped {}", name.to_string_lossy());
// NEW (1.87): borrow a Display adapter, no intermediate String
tracing::warn!("skipped {}", name.display());
```

`OsString::leak` (1.89) returns a `&'static mut OsStr` for a value that lives for the program (a parsed-once CLI arg, a process-lifetime env value). `EncodeWide` is now `Debug` on Windows (1.91). When not to: `leak` is a deliberate one-way allocation — fine for genuinely process-lifetime data, wrong inside a per-frame or per-job path.

## path

`Path::file_prefix` (1.91) returns the name before the *first* dot, where `file_stem` stops at the *last*. save/load: for `sprite.tar.gz` you want `sprite` for the default "save as" name, so `file_prefix` is the right call.

```rust
let p = Path::new("hero.idle.png");
p.file_stem();   // Some("hero.idle")
p.file_prefix(); // Some("hero")   <- (1.91)
```

`PathBuf::add_extension` / `with_added_extension` (1.91) append an extra extension instead of replacing the current one — `set_extension("bak")` on `proj.phx` gives `proj.bak`, but `add_extension("bak")` gives `proj.phx.bak`, which is what you want for a sidecar backup before an overwrite-in-place save.

`Path`/`PathBuf` now compare directly against `str` and `String` (1.91) in both directions, so `path == "untitled.phx"` compiles without a `Path::new` wrapper. `PathBuf::leak` (1.89) mirrors `OsString::leak`.

```rust
// NEW (1.91): direct comparison, no Path::new("...") on the RHS
if path.extension().is_some_and(|e| e == "phx") { /* project file */ }
let backup = path.with_added_extension("bak"); // proj.phx -> proj.phx.bak
```

## fs

`File` advisory locking is stable (1.89): `File::lock`, `lock_shared`, `try_lock`, `try_lock_shared`, `unlock`. save/load: take an exclusive `try_lock` on the project file (or a `.lock` sidecar) on open so a second Pixhaus instance can't corrupt the same `.phx` — `try_lock` fails fast instead of blocking the UI.

```rust
// save/load: refuse to open a project a second instance already holds
let file = File::open(&project_path)?;
if file.try_lock().is_err() {
    return Err(ProjectError::AlreadyOpen(project_path));
}
// ... read; file.unlock()? on close (lock also drops with the File)
```

These are *advisory* locks — they coordinate between processes that both call `lock`, not a hard OS-level block on every reader. That is the right model for a single-user file-based editor: it stops your own second window, not `cat`.

## time

`Duration::from_mins` and `from_hours` (1.91), and `Duration::from_nanos_u128` (1.93). The first two read better than `from_secs(n * 60)` for an autosave interval or a job timeout. `from_nanos_u128` takes a `u128`, so a nanosecond count that overflows `u64` (about 584 years) no longer needs manual splitting — relevant only for absurd spans, but it removes the cast.

```rust
// NEW (1.91): autosave cadence reads as what it is
const AUTOSAVE_EVERY: Duration = Duration::from_mins(5);
```

## ptr

The big one for buffer math: `offset_from_unsigned` and `byte_offset_from_unsigned` (1.87) on `*const`, `*mut`, and `NonNull`. When you know `lhs >= rhs` (later element minus earlier), these return a `usize` directly instead of `offset_from` giving an `isize` you then assert-non-negative and cast. hot-path: computing a pixel's flat index from two pointers into the same `Vec<u8>` stride buffer.

```rust
// OLD: signed distance, then check + cast
let delta = unsafe { cur.offset_from(start) }; // isize
debug_assert!(delta >= 0);
let idx = delta as usize;

// NEW (1.87): unsigned distance directly, given cur >= start
let idx = unsafe { cur.offset_from_unsigned(start) }; // usize
```

`NonNull::from_ref` / `from_mut` (1.89) build a `NonNull` from a reference without `NonNull::new(...).unwrap()` — and `unwrap` is banned in this codebase outside tests, so this is the sanctioned constructor. The strict-provenance `NonNull` methods also landed: `without_provenance`, `with_exposed_provenance`, `expose_provenance` (1.89). `impl Default for *const T`/`*mut T` (1.88) yields a null pointer.

```rust
// NEW (1.89): no Option, no unwrap (which clippy forbids here anyway)
let nn: NonNull<Pixel> = NonNull::from_ref(&pixel);
```

`ptr::fn_addr_eq` (1.85) is the sanctioned way to compare two function pointers by address, since plain `==` on fn pointers is now linted as unreliable (the optimizer merges and duplicates functions). Rarely needed; if you are comparing fn pointers to dispatch, an enum or a `dyn` trait object is usually the better design.

When not to: all of the above is `unsafe`-pointer territory, and `unsafe` is forbidden workspace-wide. These are for the few audited spots inside `render` that already justify raw pointers; in normal code, index a slice or use an iterator.

## mem

`Box::new_zeroed` / `new_zeroed_slice`, and the `Rc`/`Arc` equivalents (1.92), allocate `MaybeUninit` memory the allocator zeroes for you — one `calloc`-style allocation instead of allocate-then-`write_bytes`. save/load and hot-path: a zeroed RGBA scratch layer or a decode target.

```rust
// OLD: allocate, then zero a frame-sized buffer by hand
let mut buf = vec![0u8; w * h * 4];

// NEW (1.92): one zeroed heap allocation, no element-by-element init
let buf: Box<[u8]> = unsafe {
    Box::<[u8]>::new_zeroed_slice(w * h * 4).assume_init()
};
```

That `assume_init` is `unsafe` — zero is a valid `u8`, so it is sound, but it still needs the audited-`unsafe` treatment this repo requires. For an ordinary owned pixel buffer, `vec![0u8; n]` is simpler and just as fast; reach for `new_zeroed_slice` only where the `Box<[u8]>`/`MaybeUninit` shape is what you already hold.

`Box<MaybeUninit<T>>::write` (1.87) writes a value into a boxed uninit slot and returns the initialized `Box<T>`. `MaybeUninit<[T; N]>` now converts to and from `[MaybeUninit<T>; N]` via `From`/`AsRef`/`AsMut` (1.95), which lets you fill an array slot-by-slot then transpose to an initialized array. `Pin<Box<T>>`/`Pin<Rc<T>>`/`Pin<Arc<T>>` got `Default` (1.91).

The validity guarantee worth knowing (1.92): `MaybeUninit<T>` is now *documented* to share `T`'s size, alignment, and ABI, with any bit pattern valid. That makes `[MaybeUninit<u8>; N]` scratch buffers officially sound to use the way the GPU upload path already does — a guarantee, not new syntax.

## error

`Result::flatten` (1.89) collapses `Result<Result<T, E>, E>` to `Result<T, E>`. Small, but it removes a `?`-inside-`map` or a nested match when an operation returns a `Result` whose `Ok` is itself fallible.

```rust
// NEW (1.89)
let inner: Result<Cel, LoadError> = parse_outer(bytes).flatten();
```

When not to: most error plumbing here is `?` with `thiserror` in libs and `anyhow` in the binary. `flatten` only fits the exact nested-same-error shape; don't restructure functions to manufacture it.

`PanicHookInfo::payload_as_str` (1.91) returns the panic payload as `&str` when it is a `&str` or `String`, so a custom panic hook can route the message into `tracing` without the `downcast_ref::<&str>()`-then-`downcast_ref::<String>()` dance. Relevant if `app/` installs a panic hook that funnels panics into the one log sink — see `pixhaus-tracing`.

## hash

`BuildHasherDefault::new` (1.85) is a const constructor for the default `BuildHasher`, so you can build one in a `const` or `static` without first having a `HashMap`. Niche; it matters when you pre-declare a hasher for a fixed-seed map.

## net

`Ipv4Addr::from_octets`, `Ipv6Addr::from_octets`, `Ipv6Addr::from_segments` (1.91) construct addresses from a byte/segment array directly. async: relevant if a provider config parses raw address bytes from a settings file before opening a connection. `Result::flatten` aside, the only other net item in-window is Linux-specific: `TcpStream` `quickack`/`set_quickack` (1.89) toggles `TCP_QUICKACK`. Skip it unless you are tuning a Linux-only socket — the AI backends go through `reqwest`, which owns its sockets, so you will almost never touch raw `TcpStream` here.

---

Reference for the whole window: `midpoint` on integers and floats (1.85, signed integers 1.87) computes `(a + b) / 2` without the intermediate overflow you get from writing it out — handy for a canvas-coordinate or color blend. And `hint::select_unpredictable` (1.88) plus `hint::cold_path` (1.95) are branch hints for the optimizer; reserve them for a measured hot loop, never sprinkle them speculatively.
