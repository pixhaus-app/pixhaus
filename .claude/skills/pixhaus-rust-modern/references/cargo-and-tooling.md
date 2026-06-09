# Modern Rust: Cargo, rustc, and rustdoc workflow changes (1.85-1.96)

Toolchain-workflow changes — the [lints] table, automatic cache GC, the build-dir split, rustc flag changes, and rustdoc strictness. Part of the `pixhaus-rust-modern` skill; start at its `SKILL.md` for the shortlist and the per-version cheat sheet.

This section covers what changed in the toolchain itself — `Cargo.toml` fields, Cargo flags, the lints table, rustc flags, and rustdoc — across the 1.85 to 1.96 window. We pin 1.96 and edition 2024, so everything here is usable today. The rule for this section: adopt a workflow change when it removes friction we already feel (cache bloat, manifest-scattered lint config, FFI surprises), not because it is new. Working build setups stay as they are.

### Edition 2024 is the baseline; migrate with `cargo fix`

The 2024 edition is stable (1.85) and is what this repo declares. The why: edition is per-crate and never splits the ecosystem — a 2024 crate links a 2021 dependency fine — so the cost of moving is bounded to your own crate, and 2024 is what unlocks `let` chains and the tightened temporary scopes below. How to apply: set it in the manifest, and when porting an older crate run the automated migration rather than hand-editing.

```toml
# OLD — a crate still on the prior edition
[package]
edition = "2021"
```

```toml
# NEW — what every Pixhaus crate declares
[package]
edition = "2024"
```

```bash
# Migrate an imported crate instead of hand-fixing the diffs
cargo fix --edition
```

Two 2024-gated behavior changes touch day-to-day code. `let` chains (1.88) only compile on 2024, because the drop-order change for chained temporaries could not be retrofitted to older editions — that is why the editor's `if let Some(cel) = layer.cel(frame) && cel.is_dirty()` flattening only works here. And 2024 restricts the temporary scope of `pin!`, `format_args!`, `write!`, and `writeln!` (1.91), with that extension further disabled for non-extended `pin!` and formatting-macro arguments (1.92) — temporaries that older editions kept alive can now drop earlier. If a `write!` into our log buffer suddenly fails to borrow-check after a toolchain bump, this is why.

### The `[lints]` table and `lints.workspace`: lint policy lives in the manifest

Across this window rustc kept promoting footgun lints, several straight to deny-by-default. Centralize lint policy in the manifest instead of scattering `#![deny(...)]` across crate roots. The why: a single workspace-level table means a new deny-by-default lint is allow-listed or addressed in one place, not eight, and the policy is visible next to the dependency list. How to apply: set the policy in the root `Cargo.toml` and inherit it per crate.

```toml
# Root Cargo.toml — one policy for the workspace
[workspace.lints.rust]
unsafe_code = "forbid"        # unsafe is forbidden workspace-wide anyway
unused_visibilities = "warn"  # see below

[workspace.lints.clippy]
unwrap_used = "deny"          # the no-unwrap rule, enforced
```

```toml
# crates/core/Cargo.toml — inherit it
[lints]
workspace = true
```

The lints that moved in-window and why they matter to this codebase:

- `dangerous_implicit_autorefs` (1.88) warns, then becomes deny-by-default (1.89): implicit autoref of a place behind a raw pointer. In our `Vec<u8>` stride math, take an explicit `&raw const`/`&raw mut` rather than letting `(*ptr).field` autoref.
- `unpredictable_function_pointer_comparisons` (1.85), extended to comparisons originating in external macros (1.89): comparing `fn` pointers is unreliable because the optimizer merges or duplicates functions. Relevant if a command or verb registry ever keys on `fn`-pointer identity — don't; use `ptr::fn_addr_eq` only when an address compare is genuinely intended.
- `missing_abi` warns by default (1.86): bare `extern { ... }` now wants an explicit `extern "C"`.
- `double_negations` (1.86): `--x` is two negations, not a decrement — graduated from clippy into rustc.
- `dangling_pointers_from_locals` (1.91): a `*const i32` returned from a local that just dropped.
- `integer_to_ptr_transmutes` (1.91): integer-to-pointer `transmute` loses provenance; prefer `ptr::with_exposed_provenance` (const-stable in 1.91).
- `const_item_interior_mutations` and `function_casts_as_integer` (1.93): mutating an interior-mutable `const` operates on a temporary copy; `my_fn as usize` skips the `fn`-pointer step.
- `unused_visibilities` (1.94): `pub const _: () = ();` — the visibility is meaningless on an unnameable item.
- never-type fallback lints `never_type_fallback_flowing_into_unsafe` and `dependency_on_unit_never_type_fallback` become deny-by-default (1.92): annotate the diverging value's type explicitly.
- `mismatched_lifetime_syntaxes` (1.89) replaces `elided_named_lifetimes`: warns when an output lifetime is elided in one form but tied to an input written in another. Write `fn f(x: &T) -> ContainsLifetime<'_>` so the borrow is visible at the signature.

```rust
// 1.89 — make the elided-but-borrowing output lifetime visible
// OLD: the return borrows `frame` but the signature hides it
fn active_cel(frame: &Frame) -> CelRef { /* ... */ }

// NEW: the '_ states the borrow at the signature
fn active_cel(frame: &Frame) -> CelRef<'_> { /* ... */ }
```

Most of these are warn-by-default and surface in `cargo clippy` output without breaking the build. The deny-by-default ones (1.89 autoref, 1.92 never-type, 1.93 `deref_nullptr`) can fail a clean session at the Stop gate after a toolchain bump — fix them by annotating, not by allow-listing.

### Boolean `cfg` predicates and richer `cfg`

`cfg(true)` and `cfg(false)` are accepted as predicates (1.88), giving an always-on / always-off gate without inventing a feature. The why: `#[cfg(any())]` for "never compile this" reads as a puzzle; `#[cfg(false)]` says what it means.

```rust
// OLD — "always false" spelled as an empty any()
#[cfg(any())]
fn never_compiled() {}

// NEW (1.88)
#[cfg(false)]
fn never_compiled() {}
#[cfg(true)]
fn always_compiled() {}
```

Adjacent `cfg` changes: build scripts get `CARGO_CFG_FEATURE` listing enabled features (1.85) and `CARGO_CFG_DEBUG_ASSERTIONS` keyed off the profile (1.93); `target_env = "macabi"` / `target_env = "sim"` replace the older `target_abi` values (1.91); a macro `expr` metavariable can be forwarded into `cfg` (1.96); and using a keyword as a `cfg` predicate name is now an error (1.93). One trap from 1.85: rustc stopped treating `test` as a built-in cfg, so a hand-written `#[cfg(test)]`-style custom cfg may need declaring via `[lints]` or `--check-cfg` to avoid `unexpected_cfgs`.

### Cargo: automatic cache GC, build directory split, and publishing

The cache and artifact-layout changes are the ones that change how the build feels day to day.

- Automatic cache garbage collection is stabilized (1.88): Cargo cleans old registry sources and downloaded files from its global cache on a schedule, so `~/.cargo` stops growing without bound. No action needed; it just happens.
- `build.build-dir` is stabilized (1.91): point intermediate artifacts somewhere separate from the final `target` output dir. Note the coupled behavior — with `build.build-dir` set, `cargo publish` no longer leaves `.crate` tarballs as final artifacts (1.91), and the same `.crate`-not-kept behavior applies even when it is unset (1.93).
- Multi-package publishing is stabilized (1.90): publish several workspace packages in one `cargo publish`. For a workspace this size, that is the difference between one release command and one per crate.
- A dependency may now name both a git repository and an alternate registry (1.96).

```toml
# .cargo/config.toml — keep intermediates out of the shipped target dir (1.91)
[build]
build-dir = "target/intermediate"
```

Smaller Cargo quality-of-life items in-window: trailing flags after the subcommand take precedence (1.85); `cargo login` deprecates the inline token argument to keep tokens out of shell history (1.86); `cargo fix` and `cargo clippy --fix` default to the same target selection as a normal build (1.89), so they fix what a build compiles instead of a wider set; doctests run for cross-compiled targets via stabilized `doctest-xcompile` (1.89); `--target host-tuple` means the host (1.91); `cargo clean --workspace` (1.93); the config `include` key for loading extra config files, TOML v1.1 parsing for manifests, and `CARGO_BIN_EXE_<crate>` available at runtime (1.94); and `target.'cfg(..)'.rustdocflags` (1.96). The gzip backend moved to pure-Rust `zlib-rs` (1.88) and `cargo package` uses `gix` (1.90) — invisible unless you watch build internals.

### rustc flags: `-O` semantics, DWARF version, jump tables

Three rustc changes alter binary output directly.

- `-O` now means `-C opt-level=3`, not `2` (1.86), matching Cargo's release default. Anything in the build pipeline passing `-O` to mean opt-level 2 now gets a more aggressive optimization — usually what you wanted, but worth knowing if you benchmark.
- `-C dwarf-version` is stabilized (1.88): select the DWARF debug-info version. Useful when a profiler or debugger on the target platform wants a specific version.
- `-C jump-tables=<bool>` is stabilized (1.93), replacing unstable `-Z no-jump-tables`.
- `-C panic=abort` on Linux now produces usable backtraces by default (1.92), because unwind tables are generated even under abort. The trade is slightly larger binaries; disable with `-C force-unwind-tables=no` if size matters more than the backtrace.
- lld is the default linker on `x86_64-unknown-linux-gnu` (1.90), cutting link times. If a custom linker script or exotic flag breaks, opt back out with `-C linker-features=-lld`.
- `--remap-path-scope` is stabilized (1.95) for controlling how paths get remapped in the binary — relevant if we ever ship reproducible or path-scrubbed builds.

The i128/u128 FFI story settled in-window: `i128`/`u128` no longer trigger `improper_ctypes_definitions` (1.89), so passing them across `extern "C"` to our Unity-side or native boundaries is no longer linted. Don't read that as a guarantee of a match with every C compiler's `__int128`; it relaxes the lint, nothing more.

### rustdoc: collapsing, search, and doctest control

rustdoc changes are about reading our own docs and controlling doctests, not about API.

- A doc comment on an `impl` block shows its first line when the block is collapsed (1.85), and you can collapse all `impl` blocks via a Summary button or shift+`-` (1.90). For a crate like `core` with wide trait surfaces, that is the difference between a wall of impls and a scannable page.
- Doctests can be ignored per target via `ignore-<target>` attributes (1.88), and `--test-runtool` / `--test-runtool-arg` control how doctests run (1.88). `#![doc(test(attr(..)))]` can sit in more locations than the crate root (1.89).
- `rustdoc::invalid_doc_attributes` is deny-by-default (1.93): a malformed `html_logo_url`, `html_favicon_url`, `issue_tracker_base_url`, etc. now fails the doc build. Since `cargo doc` is part of our Stop gate, a typo in a crate-level doc attribute will stop the session — fix the attribute.
- Search and rendering polish: type-based search handles references and raw pointers (1.90/1.91), unstable and deprecated items can be ranked lower or hidden (1.95), `unsafe(...)`-wrapped attributes render as such (1.90), and `missing_doc_code_examples` no longer fires on impl items (1.96).

Net for our workflow: the doc build is stricter (1.93 deny-by-default attribute lint) and the rendered output is easier to navigate. Neither is a reason to touch doc comments that already build clean.

### What not to reach for

Skip the per-architecture target-feature stabilizations (AVX512/SHA512/SM3/SM4 in 1.89, RISC-V batches in 1.94, the s390x/PowerPC asm work) unless a specific SIMD pixel kernel needs them — they are not general workflow. Don't set `build.build-dir` or `-C dwarf-version` speculatively; add them when cache layout or a debugger actually demands it. And don't churn `Cargo.toml` to a `[lints]` table mid-feature — fold it in when you next touch the manifest, then it pays for itself the next time a deny-by-default lint lands.
