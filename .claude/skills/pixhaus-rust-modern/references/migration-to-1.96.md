# Migrating the toolchain to 1.96: a -D warnings checklist

The checklist behind the bump: every lint promotion, hard error, and silent behavior change a -D warnings workspace crosses moving up to 1.96 (and from an older pin across the whole window). Part of the `pixhaus-rust-modern` skill; start at its `SKILL.md` for the shortlist and the per-version cheat sheet.

The rule: a toolchain bump in a `-D warnings` workspace is a code change, not a config edit. Every lint that moved into the default set, and every lint promoted from warn to deny, becomes a build failure the moment you flip the pin — the Stop hook runs `cargo clippy --workspace --all-targets -- -D warnings` and won't pass until you've cleared them. So treat the bump as its own branch, change one line in `rust-toolchain.toml`, and walk this list before you assume the red is real breakage versus a lint you just need to satisfy.

Why this matters here specifically: Pixhaus now pins 1.96 (bumped from 1.95) on edition 2024, so the edition-gated changes (let chains, the if-let temporary scope, RPIT capture) were already in force — the bump stepped across one minor version, not an edition. The breakage surface for 1.95 -> 1.96 was small, and the clippy gate confirmed it clean. But the broader 1.85-1.96 window is documented below too, because someone bumping from an older pin (a stale CI image, a contributor's machine) crosses all of it at once, and the deny-by-default promotions stack.

How to apply: bump the pin, run `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace`, then triage each failure against the categories below. Most are mechanical. None justify rewriting working code beyond silencing the lint correctly.

### Step 1: the deny-by-default lint promotions — these fail the build outright

A lint that was warn-by-default in an older pin and is deny-by-default at your new one is the single biggest source of a green-to-red flip. Under `-D warnings` even the warn ones already fail you, so the practical impact is the same — but these are the ones that fail even outside `-D warnings`, so they're the ones a downstream consumer feels too.

`dangerous_implicit_autorefs` went warn (1.88) to deny (1.89). It fires when `(*raw_ptr).field` implicitly takes a reference to a place reached through a raw pointer. Take an explicit raw reference instead:

```rust
// OLD: implicit autoref of a place behind a raw pointer (now denied)
let len = (*pixels_ptr).len();

// NEW: form the raw reference explicitly, then read through it
let len = (&raw const (*pixels_ptr)).len(); // (1.89)
```

`semicolon_in_expressions_from_macros` went warn to deny (1.91). A `macro_rules!` arm that swallows a trailing semicolon in expression position now errors. Fix the macro body, not the call site.

`never_type_fallback_flowing_into_unsafe` and `dependency_on_unit_never_type_fallback` are both deny-by-default (1.92). They fire when never-type fallback near an `unsafe` block would change which code runs. Annotate the diverging value's type:

```rust
// OLD: relied on never-type fallback to infer the type (now denied near unsafe)
let outcome = if cfg!(test) { return } else { compute() };

// NEW: state the type so fallback can't change behavior
let outcome: PixelOp = if cfg!(test) { return } else { compute() }; // (1.92)
```

`invalid_macro_export_arguments` is deny-by-default (1.92): a `#[macro_export]` with invalid arguments now fails to compile.

`deref_nullptr` went warn to deny (1.93). And in name resolution, all deprecation lints became deny-by-default (1.91) — a deprecated import that only warned before now errors.

`uninhabited_static` is deny-by-default and now also reported in dependencies (1.96). This is the one most likely to surprise you on the final hop, because a dependency you don't control can trip it.

### Step 2: new warn-by-default lints — under -D warnings, these also fail

These were not lints (or were Clippy-only, or were future-incompat) at an older pin and are warn-by-default at 1.96. In a normal workspace they're yellow; under `-D warnings` they're red. Triage each.

`function_casts_as_integer` (1.93) flags casting a function item or fn pointer straight to an integer. Make the fn-pointer step explicit:

```rust
// OLD: implicit fn-item-to-int cast (now warns -> fails under -D warnings)
let addr = run_blend_pass as usize;

// NEW: go through the fn pointer
let addr = run_blend_pass as fn() as usize; // (1.93)
```

`const_item_interior_mutations` (1.93) catches mutating an interior-mutable `const` — a long-standing footgun where you're mutating a throwaway copy. The fix is almost always "make it a `static`, not a `const`."

`integer_to_ptr_transmutes` (1.91) flags `transmute`-ing an integer to a pointer, which loses provenance. Use the provenance-correct constructor:

```rust
// OLD: integer-to-pointer transmute, no provenance (now warns)
let p: *const u8 = unsafe { core::mem::transmute(addr) };

// NEW: provenance-correct
let p: *const u8 = core::ptr::with_exposed_provenance(addr); // (1.91)
```

`dangling_pointers_from_locals` (1.91) warns when you return a raw pointer into a local that's about to drop. `unused_visibilities` (1.94) flags a visibility modifier on a `const _` declaration, where it does nothing — delete the `pub`.

`mismatched_lifetime_syntaxes` (1.89, warn) replaces the old `elided_named_lifetimes` lint. It fires when an output lifetime is elided in one syntactic form but actually tied to an input. Make the syntax consistent:

```rust
// OLD: elided output hides that it borrows from the input
fn region(&self) -> SelectionView { /* ... */ }

// NEW: write the lifetime so the borrow is visible at the signature
fn region(&self) -> SelectionView<'_> { /* ... */ } // (1.89)
```

`missing_abi` went warn-by-default (1.86): an `extern` block or `extern fn` without an explicit ABI string now warns. Spell it out — the implicit ABI was `"C"`, so behavior is unchanged:

```rust
// OLD
extern { fn unity_callback(); }

// NEW
extern "C" { fn unity_callback(); } // (1.86)
```

`double_negations` (1.86) catches `--x`, which is `-(-x)`, not a decrement. `unpredictable_function_pointer_comparisons` (1.85, extended to external macros in 1.89) warns on comparing fn pointers with `==`; use `core::ptr::fn_addr_eq` (1.85) if an address comparison is genuinely intended, but prefer redesigning so you don't compare fn-pointer identity. `dangerous_implicit_autorefs` and `invalid_null_arguments` both arrived as warn-by-default in 1.88.

One de-noiser worth knowing so you don't go hunting for a phantom change: `unused_must_use` stopped warning on `Result<(), Uninhabited>` and `ControlFlow<Uninhabited, ()>` (1.92), and the `#[must_use]` on `ControlFlow` (1.87) plus `Once`/`OnceLock` additions can newly fire `unused_must_use` elsewhere — so a command or job that returns one of those may newly warn or newly stop warning depending on the exact type.

### Step 3: hard errors — things that compiled at an older pin and now refuse to

These are not lints you can `#[allow]`; the code stops compiling. Hit only if you're crossing the version where each landed.

- `#[bench]` is a hard error on stable without `#![feature(custom_test_frameworks)]` (1.88). If any crate still carries a `#[bench]` benchmark, move it to a `criterion`-style external harness. The bible already routes profiling through `#[instrument]` span durations, so this should not bite, but check before the bump.
- `missing_fragment_specifier` is an unconditional hard error (1.89): a `macro_rules!` pattern missing a fragment specifier fails. The 1.88 and 1.91 token-representation changes also reject some previously-accepted invalid `macro_rules!` that slipped through.
- `wasm_c_abi` warning became a hard error (1.86); `cenum_impl_drop_cast` became a hard error (1.86); order-dependent trait-object and `ptr_cast_add_auto_to_object` future-incompat warnings became hard errors (1.87). We're desktop-only and `unsafe`-free workspace-wide, so most of these can't reach our code — but a dependency can.
- The `#[test]` attribute is no longer ignored where it's meaningless — on trait methods, types, structs — and is now an error there (1.93). A stray `#[test]` on a non-fn will fail.
- Importing structs with `::{self}` / `::{self as name}` is no longer permitted (1.95 for `$crate`, 1.96 for structs generally). Rewrite the `use`.
- For `export_name`, `link_name`, and `link_section`, the first occurrence now takes precedence (1.96) rather than the previous behavior; combining `#[no_mangle]` with `#[export_name]` has warned since 1.85 (keep only `#[export_name]`).

### Step 4: silent behavior changes — no compile error, different runtime or build behavior

These are the dangerous ones precisely because the build stays green. Read them and decide if any touch us.

Optimization and linking:
- `-O` now means `-C opt-level=3`, not `2` (1.86), matching Cargo's defaults. Binaries built with a bare `-O` outside Cargo will differ. Inside Cargo this is a no-op.
- `lld` is the default linker on `x86_64-unknown-linux-gnu` (1.90). Faster links, but custom linker scripts or exotic flags can behave differently. Override with `-C linker-features=-lld` if a link breaks.
- `-C panic=abort` now produces usable backtraces on Linux by default (1.92), at a small binary-size cost from unwind tables. Disable with `-C force-unwind-tables=no` if size matters.

Standard-library behavior our code could observe:
- `std::env::home_dir()` on Windows stopped honoring a non-standard `$HOME` (1.85), and on Unix now falls back when `HOME` is set but empty (1.90); it was also un-deprecated (1.87). Our platform crate's path logic should go through the `directories` crate anyway, but audit any direct `home_dir()` call.
- `core::iter::Fuse`'s `Default` now builds `I::default()` instead of always being empty (1.90). If any iterator adapter relied on `Fuse::default()` being empty, it changes.
- `iter::Repeat::last` and `iter::Repeat::count` now panic instead of looping forever (1.92) — a hang becomes a panic, which is strictly better but is a behavior change.
- Raw-pointer `Debug` now prints pointer metadata for fat pointers (1.87), and `FromBytesWithNulError` changed from a struct to an enum (1.86). Snapshot tests (`insta`) that capture either will need their snapshots re-accepted.
- Temporary lifetime extension is disabled for non-extended `pin!` and for formatting-macro (`format_args!`/`write!`/`writeln!`) arguments in edition 2024 (1.91, 1.92). Temporaries that were kept alive may now drop earlier — relevant only if you held a borrow across one of these. A future-incompat lint for further temporary-lifetime shortening lands in 1.92 pointing at later releases.
- The standard library stopped using specialization on `Copy` internally (1.93); some std APIs may now call `Clone::clone` instead of a bitwise copy, a possible perf regression in hot paths. For our `Vec<u8>` pixel buffers with explicit stride this is unlikely to matter, but if a blend or copy hot path regresses after the bump, this is a suspect.
- `BTreeMap::append` no longer updates existing keys when the appended key already exists (1.93). If any registry or cache merges `BTreeMap`s, verify the semantics.

Cargo and build-script behavior:
- `CARGO_CFG_DEBUG_ASSERTIONS` is now set in build scripts based on the profile (1.93). The corpus flags a concrete fallout: crates depending on `static-init` 1.0.1-1.0.3 fail to compile after this change. Check `cargo tree` if a build script starts failing.
- `cargo fix` and `cargo clippy --fix` now default to the same target selection as a normal build (1.89) — they fix what a build compiles, not a wider set.
- Automatic cache garbage collection is on (1.88), and `cargo` switched its zlib and git backends to pure-Rust (`zlib-rs` 1.88, `gix` 1.90); behavior is the same, but CI cache assumptions can shift.

### Step 5: trait resolution and type inference — subtle, can newly accept or reject

The trait solver and inference engine changed several times in-window. None of these is a lint; each can silently change which `impl` is selected or fail to compile generic code that compiled before. If a generic function or `impl` in `core` or `services` breaks after the bump with no obvious cause, this is the bucket.

- Trait-solver preference between builtin impls and trivial where-clauses changed (1.88).
- Item bounds on an associated type are now prioritized over where-bounds for auto-traits (`Send`/`Sync`) and `Sized` (1.92). This is the one most likely to touch the `Arc<dyn Backend>` AI runtime, where auto-trait bounds on associated types decide whether a future is `Send`.
- Well-formedness predicates are no longer coinductive (1.89), recursive opaque types error earlier (1.89), and higher-ranked region handling in coherence got stricter (1.92) — each can reject a previously-accepted recursive bound or overlapping impl.
- Array-repeat `[x; N]` now requires the element type to be `Copy` as a side effect of inference (1.89), and array coercions may produce fewer inference constraints (1.95) — both can change what gets inferred.
- The type checker now checks generic const parameter defaults (1.88) and const generic argument types in more positions (1.96), rejecting ill-typed defaults that previously slipped through.
- Closure capture got "consistent and correct" around patterns (1.94) — what a closure captures can change. Pattern binding/drop order is now by written order (1.91). If a closure or a `match` arm with `Drop` side effects behaves differently, this is why.

### What not to do

Don't rewrite working code to adopt 1.96 features as part of the bump. let chains (1.88, edition 2024), `if let` guards on match arms (1.95), `assert_matches!` (1.96), the `core::range` iterator types (1.95/1.96), and async closures (1.85) are all available — but the migration is about clearing warnings and behavior changes, not modernizing. Adopt new features in their own commits, judged on their own merit, where they make a specific call site clearer. A bump branch that also rewrites three modules to use let chains is a bump branch nobody can review.

Don't blanket-`#[allow]` a new lint to make the build green. Each lint above has a correct fix — the explicit raw reference, the `static` instead of `const`, the spelled-out ABI. Reach for `#[allow]` only when you've read the specific case and the lint is a genuine false positive, and leave a `//` comment saying why, per the recording-decisions rule.

Don't trust a green build alone on this bump. The Step 4 and Step 5 changes pass `cargo clippy`; they're caught by `cargo test --workspace` and by re-accepting `insta` snapshots, which is exactly why the Stop hook runs both. Run the full gate, not just clippy, before you open the PR.
