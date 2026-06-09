---
name: pixhaus-rust-modern
description: Use when writing, reviewing, or modernizing Rust in the Pixhaus repo and you want the newest language and standard-library features the pinned toolchain offers — everything stabilized across Rust 1.85 through 1.96 on edition 2024. Reach for this whenever an older idiom appears that a recent feature now does better: a nested `if let` pyramid (let chains, 1.88), `last_mut().unwrap()` after a push (`push_mut`, 1.95), `chunks_exact(4)` over an RGBA buffer (`as_chunks::<4>`, 1.88), `split_at_mut` just to get two `&mut` (`get_disjoint_mut`, 1.86), `(a+b)/2` that can overflow (`midpoint`), a one-off `Display` newtype (`fmt::from_fn`, 1.93), a `remove`-and-collect loop (`extract_if`), or `#[async_trait]` where native async-in-trait now fits. Also load it when bumping the toolchain (the migration checklist), when deciding whether an API is in-window and which version stabilized it, or when a new clippy/rustc lint starts failing the build after a toolchain change. Covers the curated highest-value shortlist, language features, new std APIs by domain, const-context gains, Cargo/rustdoc workflow changes, and a per-version changelog. Complements `pixhaus-rust-conventions` (the general style floor); this skill is specifically about taking advantage of the newest the language offers.
---

# Modern Rust for Pixhaus (1.85-1.96)

The workspace pins toolchain **1.96** (`rust-toolchain.toml`) on **edition 2024**,
so every feature stabilized from Rust 1.85 through 1.96 compiles today. This skill
is the map of what is new and worth reaching for, drawn release-by-release from
releases.rs. It is the companion to `pixhaus-rust-conventions`: that skill is the
style floor every contribution reaches for; this one is specifically about using
the latest the language offers instead of an older idiom that a recent feature now
does better.

The one adoption rule, stated up front so it governs everything below: **a feature
earns its place by deleting a real footgun or a layer of indirection, not by being
new.** Adopt one at the next *edit* of a spot it improves — where it removes a
`.unwrap()` the no-unwrap rule bans, a nesting level, or a widen-then-narrow dance.
Do not open a churn PR to spend new keywords across working code. `rust-toolchain.toml`
stays the source of truth, so check it before reaching for anything past 1.96.

## The shortlist worth memorizing

The ~15 features that actually pay off in this codebase — an egui/wgpu shell,
`Vec<u8>` pixel buffers with explicit stride, the `Command`/undo model, tokio-owned
async, and the `Arc<dyn Backend>` AI runtime. The long form, with old-way/new-way
for each, is in `references/curated-shortlist.md`.

1. **let chains** in `if`/`while` (1.88) — collapse a nested `if let` + bool-guard
   pyramid into one condition. The highest-frequency cleanup in command/tool code.
2. **`if let` guards on match arms** (1.95) — bind a fallible lookup in the guard
   (`arm if let Some(x) = lookup()`), so the arm body stops re-doing it with an `unwrap`.
3. **async closures + the `AsyncFn*` traits** (1.85) — an `async ||` can borrow from
   its captures across the returned future; bound by `AsyncFn`, not `Fn() -> impl Future`.
   This is the AI-runtime callback boundary.
4. **`extract_if`** (Vec 1.87, Hash 1.88, BTree 1.91) — remove the entries matching a
   predicate *and* get them back, in one pass. Layer/cel deletion, cache and job pruning.
5. **`pop_if` / `pop_front_if` / `pop_back_if`** (Vec 1.86, VecDeque 1.93) — conditional
   pop. "Coalesce the last undo command only if it merges"; "take the next job only if free."
6. **`get_disjoint_mut`** (slice + `HashMap`, 1.86) — two `&mut` into one buffer at once,
   checked, for swap/blend commands — no `split_at_mut` index math.
7. **`as_chunks::<4>()`** (1.88) — view a flat RGBA8 `&[u8]` as `&[[u8; 4]]` pixels, no
   per-channel indexing, plus the ragged remainder. Direct hit on the pixel loop.
8. **`midpoint`** (int/float 1.85, signed int 1.87) — `(a + b) / 2` without the
   intermediate overflow. Deletes the widen-to-`u16`-then-narrow dance in color/coord math.
9. **`push_mut` / `insert_mut`** (1.95) — push or insert and get the `&mut` to the new
   element straight back, killing the `last_mut().unwrap()` the no-unwrap rule bans.
10. **`strict_*` integer ops** (1.91) — panic on overflow in *every* profile, not just
    debug. For a stride or buffer-length computation a release-mode wrap is latent corruption.
11. **`fmt::from_fn`** (1.93) — a one-off `Display`/`Debug` from a closure, no newtype +
    trait impl just to format a pixel run or a layer summary once.
12. **`RwLockWriteGuard::downgrade`** (std 1.92; `parking_lot` guards already have it) —
    atomic write-to-read on a shared cache/registry, no gap for another writer.
13. **const everywhere** — `Layout`, float math, `str`, and `Cell` ops went const across
    the window. Bake palettes, sRGB/gamma ramps, and sentinel IDs at compile time instead
    of building them on the first frame.
14. **`assert_matches!` / `debug_assert_matches!`** (1.96) — pattern assertions in tests
    with a real failure message; sharper than `assert!(matches!(...))`.
15. **trait upcasting** (1.86) — coerce `&dyn UndoableVerb` to `&dyn Verb` directly; delete
    the `as_verb()`-style shims in the command/verb hierarchy.

Note on `async fn` in traits (stabilized pre-window): it is fine for *static* dispatch
(`<B: Backend>`), but the `Arc<dyn Backend>` registry still needs `#[async_trait]` boxing
at the `dyn` seam. See `pixhaus-async-trait` and `pixhaus-generics-dispatch`.

## Per-version cheat sheet

One headline line per release. The full changelog — every API, const promotion, Cargo
change, and compatibility note — is `references/per-version-changelog.md`.

| Release | Worth knowing |
|---|---|
| **1.85** | Edition 2024 stable; async closures + `AsyncFn*`; `midpoint`; `Waker::noop`; the Layout/float const batch |
| **1.86** | Trait upcasting; `get_disjoint_mut`; `pop_if`; `OnceLock::wait`; `#[target_feature]` on safe fns; float `next_up`/`next_down` |
| **1.87** | RPITIT precise capturing (`use<..>`); `Vec::extract_if`; `split_off` family; inherent `str::from_utf8`; `as_chunks` precursors; big const batch |
| **1.88** | **let chains**; `as_chunks`; `HashMap`/`HashSet::extract_if`; `Cell::update`; `cfg(true/false)`; naked fns; cache GC; `Box::new_zeroed` |
| **1.89** | `repr(u128/i128)`; inferred `_` const args; `NonZero<char>`; `File` locking; `Result::flatten`; `mismatched_lifetime_syntaxes` lint |
| **1.90** | `lld` default linker on Linux; const float rounding; multi-package `cargo publish`; collapse-all-impls in rustdoc |
| **1.91** | `strict_*` integer family; `BTree::extract_if`; `iter::chain`; `Duration::from_mins/from_hours`; `Path::file_prefix`; `build.build-dir`; `AtomicPtr` arithmetic |
| **1.92** | `RwLockWriteGuard::downgrade`; repeated assoc-item bounds; `Box/Rc/Arc::new_zeroed`; const slice rotate; never-type fallback denies |
| **1.93** | `VecDeque::pop_*_if`; `fmt::from_fn`; `Vec/String::into_raw_parts`; `as_array`; the `MaybeUninit` slice toolkit; `system` ABI variadics |
| **1.94** | `LazyCell/Lock::get`/`force_mut`; `array_windows`; `Peekable::next_if_map`; `GOLDEN_RATIO`/`EULER_GAMMA`; closure-capture fix; const `mul_add` |
| **1.95** | `push_mut`/`insert_mut`; **`if let` guards on match arms**; atomic `update`/`try_update`; `cfg_select!`; `core::range`; `hint::cold_path` |
| **1.96** | `assert_matches!`; `From<T>` for `LazyCell`/`LazyLock`; tuple `!` coercion; `bool: TryFrom<int>`; macro `expr` into `cfg` |

Point releases (1.85.1, 1.91.1, 1.93.1, 1.94.1) carry only bug/security fixes — no
new features.

## When you are bumping the toolchain

A toolchain bump in this `-D warnings` workspace is a code change, not a config edit:
the Stop gate runs `cargo clippy --workspace --all-targets -- -D warnings` plus
`cargo test`, and any lint that moved into the default set or warn-to-deny fails it.
`rust-toolchain.toml` is also guarded by a `conclaude` hook — bumping the pin is a
reviewed chore change, not a silent edit. Before assuming red is real breakage, walk
`references/migration-to-1.96.md`: it categorizes every lint promotion, hard error,
and silent behavior change across the window, with the correct fix for each (the
explicit raw reference, the `static` instead of `const`, the spelled-out ABI).

## Reference files

Read the one that fits the question; each is the reference of record for its slice.

- **`references/curated-shortlist.md`** — the 15 above in full, each with old-way/new-way
  and the exact spot it pays off here. Start here when picking what to adopt.
- **`references/language-features.md`** — every language-level change: let chains, if-let
  guards, async closures, trait upcasting, precise capturing, `#[unsafe(...)]` attributes,
  `repr128`, closure-capture corrections, and the lints that became errors.
- **`references/stdlib-iter-collections.md`** — new methods on `Vec`, slices, arrays,
  iterators, and the map/set/deque/list collections, grouped by type.
- **`references/stdlib-str-numeric.md`** — `str`/`char`/integer/float/formatting APIs:
  the `strict_*` and `*_sub_signed` families, `midpoint`, const float math, `fmt::from_fn`.
- **`references/stdlib-systems.md`** — sync/atomic, io, os/path/fs, time, ptr/mem, error,
  hash, net — flagged for the threaded-pixel, save/load, and async-channel paths.
- **`references/const-contexts.md`** — what became const-callable, and where compile-time
  tables and constructors pay off.
- **`references/cargo-and-tooling.md`** — Cargo/rustc/rustdoc workflow: the `[lints]`
  table, cache GC, the build-dir split, rustc flag changes, rustdoc strictness.
- **`references/migration-to-1.96.md`** — the `-D warnings` bump checklist (above).
- **`references/per-version-changelog.md`** — the full crawl: every release, every item.
