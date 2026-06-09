# Per-version changelog: Rust 1.85 to 1.96

The reference of record: every stabilized language feature, API, const promotion, Cargo change, and compatibility note, release by release, as crawled from releases.rs. Part of the `pixhaus-rust-modern` skill; start at its `SKILL.md` for the shortlist and the per-version cheat sheet.

## Rust 1.85.0 (2025-02-20)

**Headline:** The Rust 2024 Edition is now stable; async closures (RFC 3668) are stabilized; #[diagnostic::do_not_recommend] is stabilized

**Language**
- Rust 2024 Edition stable — The 2024 Edition is now stable and selectable via edition = "2024" in Cargo.toml. It is the largest edition to date, bundling many opt-in changes (RPIT lifetime capture, if-let temporary scope, unsafe attributes/extern blocks, gen blocks reserved, etc.). Set per-crate; existing crates keep their declared edition.
- async closures (RFC 3668) — Stabilizes `async || { ... }` closures and the AsyncFn/AsyncFnMut/AsyncFnOnce trait family. Unlike a closure that returns an async block, an async closure can borrow from its captures across the returned future, so it can lend out data with a lifetime tied to each call.
- #[diagnostic::do_not_recommend] — Stabilizes the `#[diagnostic::do_not_recommend]` attribute on trait impls. It tells the compiler not to suggest that impl in trait-error diagnostics, so library authors can hide internal/blanket impls from misleading 'the trait is not implemented' hints.
- unpredictable_function_pointer_comparisons lint — New lint that warns when you compare function pointers for equality, because such comparisons are unreliable: the optimizer may merge or duplicate functions, so two fn pointers that 'should' be equal may compare unequal or vice versa.
- Lint on combining #[no_mangle] and #[export_name] — The compiler now lints when an item carries both `#[no_mangle]` and `#[export_name]`. The two conflict — `#[export_name]` sets the symbol name while `#[no_mangle]` says to keep the original — so the combination is contradictory and one attribute is silently ignored.

**Stabilized APIs**
- `BuildHasherDefault::new` — Const-friendly constructor for the default BuildHasher, usable without a HashMap.
- `ptr::fn_addr_eq` — Compare two function pointers by address explicitly, the sanctioned alternative to `==` flagged by the new fn-pointer-comparison lint.
- `io::ErrorKind::QuotaExceeded` — New ErrorKind variant for disk/space quota exceeded errors.
- `io::ErrorKind::CrossesDevices` — New ErrorKind variant for operations that cross filesystem device boundaries (e.g. a rename across devices).
- `{float}::midpoint` — Compute the midpoint (a + b) / 2 of two floats without intermediate overflow.
- `{integer}::midpoint` — Midpoint of two unsigned integers without overflow (unsigned integer types).
- `NonZeroU*::midpoint` — Midpoint for the NonZero unsigned integer types.
- `<impl Extend for tuples (arity 1-12)>` — impl std::iter::Extend for tuples of arity 1 through 12, so you can extend a tuple of collections from an iterator of tuples.
- `<impl FromIterator<(A, ...)> for tuples (arity 1-12)>` — FromIterator for tuples of arity 1 through 12, letting you `collect()` an iterator of tuples into a tuple of collections.
- `std::task::Waker::noop` — A no-op Waker (const fn) that does nothing when woken, handy for polling a future manually or in tests.

**Const-stabilized:** mem::size_of_val, mem::align_of_val, Layout::for_value, Layout::align_to, Layout::pad_to_align, Layout::extend, Layout::array, std::mem::swap, std::ptr::swap, NonNull::new, HashMap::with_hasher, HashSet::with_hasher, BuildHasherDefault::new, <float>::recip, <float>::to_degrees, <float>::to_radians, <float>::max, <float>::min, <float>::clamp, <float>::abs, <float>::signum, <float>::copysign, MaybeUninit::write

**Cargo / rustc / rustdoc**
- Cargo: add a future-incompatibility warning against using keywords in cfg names/values, and add support for raw identifiers (r#...) in cfgs.
- Cargo: stabilize higher-precedence trailing flags, so flags passed after the command (trailing) take precedence over earlier ones.
- Cargo: pass CARGO_CFG_FEATURE to build scripts, exposing enabled features as a cfg to build.rs.
- Rustdoc: a doc comment on an impl block now shows its first line when the impl is collapsed.

**Compatibility notes**
- rustc no longer treats the `test` cfg as a well-known (built-in) cfg; configuring it manually may now warn via the unexpected_cfgs lint unless declared.
- Disabled potentially incorrect type inference when a type is constrained by mixed where-clauses; some code relying on the old inference may now fail to compile.
- std::env::home_dir() on Windows is fixed to ignore the non-standard $HOME environment variable, so it may return a different (correct) path than before.
- core::ffi::c_char signedness changed on some Tier 2/3 targets to match the platform C ABI; code assuming a fixed signedness may break.
- Nested macro_rules from external crates now use the external crate's edition rather than the consuming crate's edition.
- The Solaris baseline was increased to Solaris 11.4.
- The abi_unsupported_vector_types lint is now shown in future-breakage (future-incompatibility) reports.
- Error on multiple super-trait `dyn Trait` instantiations that require associated types; previously-accepted ambiguous code is now rejected.
- powerpc64-ibm-aix default code model changed to `large`.
- Panics in the standard library now have a leading `library/` in their reported path.
- Compiler: the unstable flag -Zpolymorphize has been removed.
- Platform: powerpc64le-unknown-linux-musl promoted to Tier 2 with host tools.

---

## Rust 1.85.1 (2025-03-18) — point release (bug/security fixes only)

**Headline:** Rust 1.85.1 is a point release containing five bug fixes. No language features, stabilized APIs, or const stabilizations.; Fixes the doctest-merging feature of the 2024 Edition.; Relaxes some target_feature checks when generating docs.; Fixes errors in std::fs::rename on Windows 10, version 1607.; Downgrades bootstrap cc to fix custom targets.; Skips submodule updates when building Rust from a source tarball.

**Cargo / rustc / rustdoc**
- rustdoc: Fix the doctest-merging feature of the 2024 Edition.
- rustdoc: Relax some target_feature checks when generating docs.
- bootstrap: Downgrade bootstrap cc to fix custom targets.
- bootstrap: Skip submodule updates when building Rust from a source tarball.

**Compatibility notes**
- std::fs::rename on Windows 10 version 1607 could produce errors; 1.85.1 fixes this regression (behavioral fix, not a new break).

---

## Rust 1.86.0 (2025-04-03)

**Headline:** Trait upcasting to supertraits is stable; Safe functions can now carry #[target_feature]; -O now means opt-level=3 to match Cargo defaults; Disjoint mutable access: <[_]>::get_disjoint_mut and HashMap::get_disjoint_mut; SSE2 now required for i686 32-bit x86 hard-float targets

**Language**
- Trait upcasting to supertraits — Stabilizes coercing a `dyn Trait` to a `dyn Supertrait` when one trait has the other as a supertrait. An unsizing coercion from `&dyn Sub` (or Box/Arc/etc.) to `&dyn Super` now compiles, so you can erase down to a base trait object without a wrapper trait or a manual `as_super()` method. (#134367)
- #[target_feature] on safe functions — Safe functions may now be annotated with `#[target_feature(enable = "...")]`. Previously only `unsafe fn` could carry the attribute. Such a safe function is callable without `unsafe` from a context that already statically guarantees the feature (e.g. another function with the same `#[target_feature]`), but still requires `unsafe` to call from a context that does not. (#134090)
- missing_abi lint now warns by default — Writing `extern { ... }` or `extern fn` without an explicit ABI string now triggers the `missing_abi` lint as a warning by default. Spell the ABI out, e.g. `extern "C"`. (#132397)
- double_negations lint — New built-in `double_negations` lint catches `--x`, which is not a prefix-decrement operator in Rust (it parses as two unary negations and is a no-op on the value). Previously only available as `clippy::double_neg`, now in rustc. (#126604)
- More not-null pointer detection in const eval — Const evaluation now detects more pointers as definitely non-null based on their alignment, allowing more null checks to be resolved at compile time. (#133700)
- Empty repr() on invalid items rejected — An empty `#[repr()]` attribute applied to items where a representation makes no sense is now correctly rejected rather than silently accepted. (#133925)
- Inner attributes #![test] and #![rustfmt::skip] restricted — The inner attributes `#![test]` and `#![rustfmt::skip]` are no longer accepted in more places than intended; their accepted positions were tightened. (#134276)

**Stabilized APIs**
- `f32::next_down` — Returns the next representable float toward negative infinity (applies to {float}, i.e. f32 and f64).
- `f64::next_down` — Returns the next representable float toward negative infinity.
- `f32::next_up` — Returns the next representable float toward positive infinity (applies to {float}, i.e. f32 and f64).
- `f64::next_up` — Returns the next representable float toward positive infinity.
- `<[_]>::get_disjoint_mut` — Returns multiple mutable references to disjoint elements/subslices by indices, checked for disjointness.
- `<[_]>::get_disjoint_unchecked_mut` — Unchecked version returning multiple mutable references to disjoint elements without bounds/disjointness checks.
- `slice::GetDisjointMutError` — Error type returned by the checked get_disjoint_mut when indices are out of bounds or overlapping.
- `HashMap::get_disjoint_mut` — Returns multiple mutable references to disjoint values for a set of keys.
- `HashMap::get_disjoint_unchecked_mut` — Unchecked version returning multiple mutable references to disjoint values by keys.
- `NonZero::count_ones` — Counts the set bits of a NonZero integer, returning a NonZero<u32> result.
- `Vec::pop_if` — Pops and returns the last element only if the given predicate returns true for it.
- `sync::Once::wait` — Blocks the current thread until the Once's initialization routine has completed.
- `sync::Once::wait_force` — Like wait, but also returns even if the Once was poisoned during initialization.
- `sync::OnceLock::wait` — Blocks until the OnceLock is initialized and returns a reference to its value.

**Const-stabilized:** hint::black_box, io::Cursor::get_mut, io::Cursor::set_position, str::is_char_boundary, str::split_at, str::split_at_checked, str::split_at_mut, str::split_at_mut_checked

**Cargo / rustc / rustdoc**
- Cargo: when merging configuration, replace rather than combine config keys that refer to program paths and arguments (#15066)
- Cargo: error if both --package and --workspace are provided but the requested package is missing (#15071)
- Cargo: deprecate the token argument to `cargo login` to avoid leaking tokens into shell history (#15057)
- Cargo: simplify SourceID comparison implementation, which may affect alternative registry behavior (#14980)
- Rustdoc: add a sans-serif font setting (#133636)
- Compiler/rustc: -O now means -C opt-level=3 instead of -C opt-level=2, matching Cargo's defaults (#135439)

**Compatibility notes**
- The `wasm_c_abi` future-compatibility warning is now a hard error; wasm-bindgen users must upgrade to at least version 0.2.89 (#133951)
- Removed the long-deprecated no-op attributes #![no_start] and #![crate_id] (#134300)
- The `cenum_impl_drop_cast` future-incompatibility lint is now a hard error (#135964)
- SSE2 is now required for i686 32-bit x86 hard-float targets; disabling it warns now and will eventually be a hard error — use the i586 target for pre-SSE2 32-bit x86 (#137037)
- -O now maps to -C opt-level=3 instead of -C opt-level=2, so binaries built with -O may differ in optimization (#135439)
- The `missing_abi` lint now warns by default on extern blocks/functions without an explicit ABI (#132397)
- New `double_negations` lint warns on `--x` patterns (#126604)
- Empty `#[repr()]` on invalid items is now rejected (#133925)
- Inner attributes #![test] and #![rustfmt::skip] are no longer accepted in some positions they previously were (#134276)
- std::fs::remove_file now removes read-only files on recent Windows versions, a behavior change from prior failure (#134679)
- FromBytesWithNulError (from CStr::from_bytes_with_nul) changed from an opaque struct to an enum (#134143)
- Removed RustcDecodable and RustcEncodable derives (#134272)
- libtest's --logfile option is deprecated (#134283)
- Replaced the i686-unknown-redox target with i586-unknown-redox (#136698)
- Raw pointers are now debug-asserted non-null on access, which can trip debug assertions in code relying on previously-UB null access (#134424)

---

## Rust 1.87.0 (2025-05-15)

**Headline:** asm_goto stabilized: assembly that jumps into Rust labels; use<...> precise capturing now allowed on RPITIT (impl Trait in trait return position); Anonymous pipe API stabilized: io::pipe, PipeReader, PipeWriter; Many std::arch intrinsics callable from safe code when target features are enabled; Vec/LinkedList extract_if and a family of slice split_off methods stabilized; Unsigned pointer offset_from_unsigned and integer cast_signed/cast_unsigned

**Language**
- Stabilize `asm_goto` feature — Inline assembly can now jump to Rust labels via the `label` operand in `asm!`, allowing assembly blocks to transfer control flow back into Rust code blocks.
- Allow parsing open beginning ranges (`..EXPR`) after unary operators `!`, `-`, and `*` — The parser now accepts an open-beginning range expression directly after a unary `!`, `-`, or `*` operator, changing how such token sequences are grouped.
- Don't require method impls for methods with `Self: Sized` bounds in `impl`s for unsized types — When implementing a trait for an unsized type, you no longer have to provide method bodies for trait methods that carry a `Self: Sized` bound, since those methods can never be called on the unsized type anyway.
- Stabilize `feature(precise_capturing_in_traits)` allowing `use<...>` bounds on return position `impl Trait` in `trait`s — `use<...>` precise-capturing bounds are now allowed on return-position `impl Trait` in traits (RPITIT), letting a trait method's returned opaque type precisely specify which generic parameters and lifetimes it captures.

**Stabilized APIs**
- `Vec::extract_if` — Drain-like iterator that removes and yields elements matching a predicate.
- `vec::ExtractIf` — The iterator type returned by Vec::extract_if.
- `LinkedList::extract_if` — Removes and yields elements of a LinkedList matching a predicate.
- `linked_list::ExtractIf` — The iterator type returned by LinkedList::extract_if.
- `<[T]>::split_off` — Splits a shared slice at an index range, returning the split-off subslice.
- `<[T]>::split_off_mut` — Mutable variant of slice split_off.
- `<[T]>::split_off_first` — Returns the first element and the remainder of a shared slice.
- `<[T]>::split_off_first_mut` — Mutable variant returning the first element and remainder.
- `<[T]>::split_off_last` — Returns the last element and the remainder of a shared slice.
- `<[T]>::split_off_last_mut` — Mutable variant returning the last element and remainder.
- `String::extend_from_within` — Appends a copy of a byte range of the String onto its end.
- `os_str::Display` — Display helper type for OsStr/OsString lossy display.
- `OsString::display` — Returns a Display adapter for an OsString.
- `OsStr::display` — Returns a Display adapter for an OsStr.
- `io::pipe` — Creates an anonymous pipe, returning a (PipeReader, PipeWriter) pair.
- `io::PipeReader` — Read end of an anonymous pipe.
- `io::PipeWriter` — Write end of an anonymous pipe.
- `impl From<PipeReader> for OwnedHandle` — Convert a PipeReader into an OS OwnedHandle (Windows).
- `impl From<PipeWriter> for OwnedHandle` — Convert a PipeWriter into an OS OwnedHandle (Windows).
- `impl From<PipeReader> for Stdio` — Use a PipeReader as process Stdio.
- `impl From<PipeWriter> for Stdio` — Use a PipeWriter as process Stdio.
- `impl From<PipeReader> for OwnedFd` — Convert a PipeReader into an OwnedFd (Unix).
- `impl From<PipeWriter> for OwnedFd` — Convert a PipeWriter into an OwnedFd (Unix).
- `Box<MaybeUninit<T>>::write` — Writes a value into a boxed MaybeUninit and returns the initialized Box<T>.
- `impl TryFrom<Vec<u8>> for String` — Fallible conversion from a UTF-8 byte vector to a String.
- `<*const T>::offset_from_unsigned` — Computes the unsigned distance in elements between two pointers where lhs >= rhs.
- `<*const T>::byte_offset_from_unsigned` — Unsigned byte distance between two const pointers.
- `<*mut T>::offset_from_unsigned` — Unsigned element distance between two mut pointers.
- `<*mut T>::byte_offset_from_unsigned` — Unsigned byte distance between two mut pointers.
- `NonNull::offset_from_unsigned` — Unsigned element distance between two NonNull pointers.
- `NonNull::byte_offset_from_unsigned` — Unsigned byte distance between two NonNull pointers.
- `<uN>::cast_signed` — Reinterprets an unsigned integer's bits as the same-width signed integer.
- `NonZero::<uN>::cast_signed` — Reinterprets a NonZero unsigned integer as a NonZero signed integer.
- `<iN>::cast_unsigned` — Reinterprets a signed integer's bits as the same-width unsigned integer.
- `NonZero::<iN>::cast_unsigned` — Reinterprets a NonZero signed integer as a NonZero unsigned integer.
- `<uN>::is_multiple_of` — Returns true if the unsigned integer is a multiple of the argument.
- `<uN>::unbounded_shl` — Left shift that returns 0 when the shift amount is >= bit width, no panic/wrap.
- `<uN>::unbounded_shr` — Right shift that returns 0 when the shift amount is >= bit width.
- `<iN>::unbounded_shl` — Signed left shift that returns 0 for out-of-range shift amounts.
- `<iN>::unbounded_shr` — Signed arithmetic right shift saturating to 0/-1 for out-of-range shifts.
- `<iN>::midpoint` — Computes the midpoint of two signed integers without overflow.
- `<str>::from_utf8` — Inherent str associated fn converting a byte slice to &str, validating UTF-8.
- `<str>::from_utf8_mut` — Inherent str associated fn converting a mutable byte slice to &mut str.
- `<str>::from_utf8_unchecked` — Inherent unchecked conversion from bytes to &str without validation.
- `<str>::from_utf8_unchecked_mut` — Inherent unchecked conversion from mutable bytes to &mut str.

**Const-stabilized:** core::str::from_utf8_mut, <[T]>::copy_from_slice, SocketAddr::set_ip, SocketAddr::set_port, SocketAddrV4::set_ip, SocketAddrV4::set_port, SocketAddrV6::set_ip, SocketAddrV6::set_port, SocketAddrV6::set_flowinfo, SocketAddrV6::set_scope_id, char::is_digit, char::is_whitespace, <[[T; N]]>::as_flattened, <[[T; N]]>::as_flattened_mut, String::into_bytes, String::as_str, String::capacity, String::as_bytes, String::len, String::is_empty, String::as_mut_str, String::as_mut_vec, Vec::as_ptr, Vec::as_slice, Vec::capacity, Vec::len, Vec::is_empty, Vec::as_mut_slice, Vec::as_mut_ptr

**Cargo / rustc / rustdoc**
- Cargo: Add terminal integration via ANSI OSC 9;4 sequences (progress reporting in supporting terminals)
- Cargo: bump openssl to v3
- Cargo: feat(package) add --exclude-lockfile flag
- Rustdoc: no rustdoc-specific changes listed for this release

**Compatibility notes**
- Rust now raises an error for macro invocations inside the `#![crate_name]` attribute
- Unstable fields are now always considered to be inhabited
- Macro arguments of unary operators followed by open beginning ranges may now be matched differently
- Make `Debug` impl of raw pointers print metadata if present (changes Debug output of fat/raw pointers)
- Warn against function pointers using unsupported ABI strings in dependencies
- Associated types on `dyn` types are no longer deduplicated
- Forbid attributes on `..` inside of struct patterns
- Make `ptr_cast_add_auto_to_object` lint into a hard error
- Many `std::arch` intrinsics are now safe to call in some contexts, so there may now be new `unused_unsafe` warnings in existing codebases
- Limit `width` and `precision` formatting options to 16 bits on all targets
- Turn order-dependent trait objects future-incompat warning into a hard error
- Denote `ControlFlow` as `#[must_use]` (new unused-must-use warnings)
- Windows: the standard library no longer links `advapi32`, except on win7
- Proc macros can no longer observe expanded `cfg(true)` attributes
- Start changing the internal representation of pasted tokens
- Don't allow flattened format_args in const

---

## Rust 1.88.0 (2025-06-26)

**Headline:** let chains stabilized in the 2024 edition; naked functions stabilized; boolean literals usable as cfg predicates; Cargo automatic cache garbage collection stabilized

**Language**
- let chains (let_chains) in the 2024 edition — &&-chaining of `let` statements inside `if` and `while` conditions, intermixed with boolean expressions, is now stable. Requires the 2024 edition because of a drop-order change.
- naked functions (naked_functions) — Functions marked `#[unsafe(naked)]` with a body that is a single `naked_asm!` block get no compiler-generated prologue/epilogue; you write the entire function body in assembly.
- boolean literals as cfg predicates (cfg_boolean_literals) — `cfg(true)` and `cfg(false)` are now accepted as configuration predicates, giving an always-on / always-off condition without a custom feature flag.
- #[bench] attribute fully de-stabilized — The unstable `#[bench]` attribute is now a hard error unless `#![feature(custom_test_frameworks)]` is enabled, finishing its removal from the stable surface.
- dangerous_implicit_autorefs lint — New warn-by-default lint that fires on implicit autoref of a raw pointer dereference, which can silently create a reference to a place reached through a raw pointer.
- invalid_null_arguments lint — New lint that catches invalid usage of null pointers where a non-null pointer is required, preventing undefined behavior from passing null where it is disallowed.
- trait impl candidate preference change for builtin impls and trivial where-clauses — Trait solver now changes which impl candidate it prefers between builtin impls and trivial where-clauses. A compiler/trait-resolution behavior change rather than new syntax.
- check types of generic const parameter defaults — The compiler now type-checks the defaults of generic const parameters, rejecting previously accepted ill-typed defaults.

**Stabilized APIs**
- `Cell::update` — Update the value inside a Cell by applying a function to it.
- `impl Default for *const T` — Default impl for const raw pointers (yields a null pointer).
- `impl Default for *mut T` — Default impl for mut raw pointers (yields a null pointer).
- `HashMap::extract_if` — Iterator that removes and yields entries matching a predicate from a HashMap.
- `HashSet::extract_if` — Iterator that removes and yields elements matching a predicate from a HashSet.
- `hint::select_unpredictable` — Branchless select hint for values the optimizer should treat as unpredictable (constant-time / side-channel oriented selection).
- `proc_macro::Span::line` — Line number of the span's start (proc-macro span introspection).
- `proc_macro::Span::column` — Column of the span's start.
- `proc_macro::Span::start` — Start location of the span.
- `proc_macro::Span::end` — End location of the span.
- `proc_macro::Span::file` — File path string for the span.
- `proc_macro::Span::local_file` — Local file path for the span, if available.
- `<[T]>::as_chunks` — Split a slice into a slice of N-element arrays plus a remainder, returning shared references.
- `<[T]>::as_chunks_mut` — Mutable version of as_chunks.
- `<[T]>::as_chunks_unchecked` — Unchecked as_chunks assuming the length divides evenly (unsafe).
- `<[T]>::as_chunks_unchecked_mut` — Mutable unchecked as_chunks (unsafe).
- `<[T]>::as_rchunks` — Split a slice into N-element array chunks from the end plus a leading remainder.
- `<[T]>::as_rchunks_mut` — Mutable version of as_rchunks.
- `mod ffi::c_str` — The core::ffi::c_str / std::ffi::c_str module is now stable, exposing CStr/CString-related items in that path.

**Const-stabilized:** NonNull::<T>::replace is now const, <*mut T>::replace is now const, std::ptr::swap_nonoverlapping is now const, Cell::replace is now const, Cell::get is now const, Cell::get_mut is now const, Cell::from_mut is now const, Cell::as_slice_of_cells is now const

**Cargo / rustc / rustdoc**
- Cargo: automatic cache garbage collection is now stabilized — Cargo automatically cleans up old cached data in its global cache (registry sources, downloaded files) on a schedule.
- Cargo: now uses the zlib-rs (pure-Rust zlib) backend for gzip compression instead of a C implementation.
- Rustdoc: doctests can be ignored based on target names using `ignore-*` attributes (e.g. ignore-<target>).
- Rustdoc: stabilized the `--test-runtool` and `--test-runtool-arg` CLI options for controlling how doctests are executed (the runner tool and its arguments).
- Compiler: stabilized `-Cdwarf-version` to select the version of DWARF debug information to generate.

**Compatibility notes**
- Finished changing the internal representation of pasted tokens; certain invalid declarative (macro_rules!) macros that previously slipped through are now correctly rejected.
- `#[bench]` is fully de-stabilized and is now a hard error on stable without `#![feature(custom_test_frameworks)]`.
- Borrow checking of some always-true patterns was fixed where the borrow checker was previously overly permissive; previously-accepted code may now be rejected.
- The minimum supported external LLVM version was raised to 19.
- Using a vector type with a non-Rust ABI without the required target feature enabled is now a hard error.
- Platform support: `i686-pc-windows-gnu` was demoted to Tier 2.
- Libraries: backticks were removed from the `#[should_panic]` test failure message (affects code matching on the message text).
- Libraries: the libtest flag `--nocapture` is deprecated in favor of `--no-capture`.
- Trait solver: the preference between builtin impls and trivial where-clauses changed, which can alter impl selection in some code.
- The compiler now checks the types of generic const parameter defaults, which can reject previously-accepted code.

---

## Rust 1.89.0 (2025-08-07)

**Headline:** Explicitly inferred const arguments (`_`) are stable; `#[repr(u128)]` / `#[repr(i128)]` enum representations stabilized (`repr128`); Large family of AVX512 / SHA512 / SM3 / SM4 / KL x86 target features and intrinsics stabilized; New warn-by-default `mismatched_lifetime_syntaxes` lint replaces `elided_named_lifetimes`; `File` advisory locking (`File::lock`, `try_lock`, `unlock`, and shared variants) stabilized; `extern "C"` on `wasm32-unknown-unknown` now uses a standards-compliant ABI (breaking)

**Language**
- Explicitly inferred const arguments (`generic_arg_infer`) — You can now write `_` in const-generic argument position and let the compiler infer the const value, the same way `_` already infers a type argument.
- `mismatched_lifetime_syntaxes` lint (warn-by-default) — New warn-by-default lint that flags when an output lifetime is elided in one syntactic form but tied to an input lifetime written in a different form, making the lifetime relationship hard to see at the signature.
- `unpredictable_function_pointer_comparisons` extended to external macros — The existing lint against comparing function pointers now also fires when the comparison originates in an external macro, closing a gap where unreliable fn-pointer equality was hidden behind macro expansion.
- `dangerous_implicit_autorefs` lint now deny-by-default — The lint catching implicit autoref of a place behind a raw pointer (which can create a reference to an unaligned or dangling place) is promoted from warn to deny by default.
- `repr128` stabilized (`#[repr(u128)]` / `#[repr(i128)]`) — Enums may now choose a 128-bit integer discriminant representation.
- Stabilize avx512 target features — The `avx512*` x86/x86_64 target features can now be enabled in stable Rust (via `#[target_feature]` / `-C target-feature`), unlocking the matching stable AVX512 intrinsics.
- Stabilize `kl` and `widekl` target features (x86) — The x86 Key Locker (`kl`) and wide Key Locker (`widekl`) target features are now stable.
- Stabilize `sha512`, `sm3`, and `sm4` target features (x86) — The x86 `sha512`, `sm3`, and `sm4` cryptographic target features are now stable, enabling the corresponding stable intrinsics.
- Stabilize LoongArch target features — The LoongArch target features `f`, `d`, `frecipe`, `lasx`, `lbt`, `lsx`, and `lvz` are now stable.
- Remove `i128`/`u128` from `improper_ctypes_definitions` — `i128` and `u128` no longer trigger the improper-ctypes lint in `extern` definitions, reflecting that their C ABI is now settled.
- Allow `#![doc(test(attr(..)))]` everywhere — The `#![doc(test(attr(..)))]` attribute, used to apply attributes to doctests, can now be placed in more locations rather than only at the crate root.
- Temporary lifetime extension through tuple struct / tuple variant constructors — Temporary lifetime extension now also applies through tuple struct and tuple variant constructors, so temporaries wrapped in e.g. `Some(&temp)` or `Wrapper(&temp)` in a `let` live as long as the binding.
- `extern "C"` ABI on `wasm32-unknown-unknown` is now standards-compliant — `extern "C"` functions on the `wasm32-unknown-unknown` target now follow the standard C ABI for wasm instead of the previous non-conforming behavior.

**Stabilized APIs**
- `NonZero<char>` — A `NonZero` wrapper specialization usable with `char`.
- `AVX512 intrinsics` — The std::arch AVX512 intrinsic family, stabilized alongside the avx512 target features (x86/x86_64).
- `SHA512 intrinsics` — std::arch x86 SHA512 cryptographic intrinsics.
- `SM3 intrinsics` — std::arch x86 SM3 cryptographic intrinsics.
- `SM4 intrinsics` — std::arch x86 SM4 cryptographic intrinsics.
- `File::lock` — Acquire an exclusive advisory lock on the file, blocking until available.
- `File::lock_shared` — Acquire a shared advisory lock on the file, blocking until available.
- `File::try_lock` — Try to acquire an exclusive advisory lock without blocking.
- `File::try_lock_shared` — Try to acquire a shared advisory lock without blocking.
- `File::unlock` — Release an advisory lock held on the file.
- `NonNull::from_ref` — Construct a `NonNull<T>` from a shared reference `&T`.
- `NonNull::from_mut` — Construct a `NonNull<T>` from a mutable reference `&mut T`.
- `NonNull::without_provenance` — Create a `NonNull` from an address with no provenance (strict-provenance API).
- `NonNull::with_exposed_provenance` — Create a `NonNull` from an address using exposed provenance.
- `NonNull::expose_provenance` — Expose the provenance of a `NonNull` pointer, returning its address.
- `OsString::leak` — Consume the `OsString` and leak it, returning a `&'static mut OsStr`.
- `PathBuf::leak` — Consume the `PathBuf` and leak it, returning a `&'static mut Path`.
- `Result::flatten` — Flatten a `Result<Result<T, E>, E>` into a `Result<T, E>`.
- `std::os::linux::net::TcpStreamExt::quickack` — Read the Linux TCP_QUICKACK option state on a `TcpStream`.
- `std::os::linux::net::TcpStreamExt::set_quickack` — Set the Linux TCP_QUICKACK option on a `TcpStream`.

**Const-stabilized:** `<[T; N]>::as_mut_slice` is now usable in const contexts, `<[u8]>::eq_ignore_ascii_case` is now usable in const contexts, `str::eq_ignore_ascii_case` is now usable in const contexts

**Cargo / rustc / rustdoc**
- Cargo: `cargo fix` and `cargo clippy --fix` now default to the same target selection as other build commands (so they fix the same targets a normal build would compile, instead of a different/wider default set)
- Cargo: doctest-xcompile is stabilized, so doctests run for cross-compiled targets
- Rustdoc: on mobile, the sidebar is now full width and line-wraps

**Compatibility notes**
- `missing_fragment_specifier` is now an unconditional hard error (previously a lint/future-incompat warning); macro_rules patterns missing a fragment specifier will fail to compile
- Enabling the `neon` target feature on `aarch64-unknown-none-softfloat` now causes a warning
- Sized Hierarchy: Part I introduces a small breaking change affecting `?Sized` bounds on impls
- The `elided_named_lifetimes` lint is superseded by the new `mismatched_lifetime_syntaxes` lint
- The type checker now errors on recursive opaque types earlier
- Type inference now has side effects from requiring the element types of array repeat expressions (`[x; N]`) to be `Copy`, which can change inference in some code
- `std::intrinsics::{copy, copy_nonoverlapping, write_bytes}` are now proper intrinsics (the deprecated wrappers changed); use the `std::ptr` equivalents
- The long-deprecated `std::intrinsics::drop_in_place` was removed; use `std::ptr::drop_in_place`
- Well-formedness predicates are no longer coinductive, which can reject some previously-accepted recursive bounds
- Removed a hack when checking impl method compatibility, which may reject previously-accepted impls
- Removed unnecessary type inference due to built-in trait object impls, which can change inference results
- Now lints against the `"stdcall"`, `"fastcall"`, and `"cdecl"` ABIs on non-x86-32 targets
- Future-incompatibility warnings relating to the never type are now reported in dependencies
- `std::ptr::copy_*` intrinsics now perform static self-init checks
- `extern "C"` functions on the `wasm32-unknown-unknown` target now have a standards-compliant ABI (breaking change for FFI relying on the old behavior)
- `x86_64-apple-darwin` is in the process of being demoted to Tier 2 with host tools
- Compiler now defaults to non-leaf frame pointers on aarch64-linux, enables non-leaf frame pointers for Arm64EC Windows, and sets Apple frame pointers by architecture (changes stack-walking/perf-profiling expectations)

---

## Rust 1.90.0 (2025-09-18)

**Headline:** lld is now the default linker on x86_64-unknown-linux-gnu, cutting link times; u*::*_sub_signed family stabilized: subtract a signed integer from an unsigned one with checked/overflowing/saturating/wrapping semantics; f32/f64 rounding (floor, ceil, trunc, fract, round, round_ties_even) and <[T]>::reverse are now const; CStr/CString/Cow<CStr> can now be compared against each other directly; x86_64-apple-darwin (Intel macOS) demoted to Tier 2 with host tools

**Language**
- Finer-grained diagnostic-attribute lints — The unknown_or_malformed_diagnostic_attributes lint is split into four narrower lints: unknown_diagnostic_attributes, misplaced_diagnostic_attributes, malformed_diagnostic_attributes, and malformed_diagnostic_format_literals. This lets you allow/deny each failure mode independently rather than the whole group.
- Constants referencing mutable/external memory — A constant whose final value contains references to mutable or external memory is now allowed, but using such a constant as a pattern is rejected.
- Volatile access to non-Rust memory — Volatile reads/writes to memory not managed by Rust are now allowed, including address 0 (the null address). Supports MMIO and similar low-level hardware access.

**Stabilized APIs**
- `u{n}::checked_sub_signed` — Checked subtraction of a signed integer from an unsigned one; returns None on overflow. Available on all unsigned integer types.
- `u{n}::overflowing_sub_signed` — Subtract a signed integer from an unsigned one, returning the result and a bool indicating wraparound.
- `u{n}::saturating_sub_signed` — Subtract a signed integer from an unsigned one, saturating at the numeric bounds instead of overflowing.
- `u{n}::wrapping_sub_signed` — Subtract a signed integer from an unsigned one, wrapping around at the boundary.
- `impl Copy for IntErrorKind` — IntErrorKind now implements Copy.
- `impl Hash for IntErrorKind` — IntErrorKind now implements Hash.
- `impl PartialEq<&CStr> for CStr` — Allow comparing a CStr against a &CStr.
- `impl PartialEq<CString> for CStr` — Allow comparing a CStr against a CString.
- `impl PartialEq<Cow<CStr>> for CStr` — Allow comparing a CStr against a Cow<CStr>.
- `impl PartialEq<&CStr> for CString` — Allow comparing a CString against a &CStr.
- `impl PartialEq<CStr> for CString` — Allow comparing a CString against a CStr.
- `impl PartialEq<Cow<CStr>> for CString` — Allow comparing a CString against a Cow<CStr>.
- `impl PartialEq<&CStr> for Cow<CStr>` — Allow comparing a Cow<CStr> against a &CStr.
- `impl PartialEq<CStr> for Cow<CStr>` — Allow comparing a Cow<CStr> against a CStr.
- `impl PartialEq<CString> for Cow<CStr>` — Allow comparing a Cow<CStr> against a CString.

**Const-stabilized:** <[T]>::reverse, f32::floor, f32::ceil, f32::trunc, f32::fract, f32::round, f32::round_ties_even, f64::floor, f64::ceil, f64::trunc, f64::fract, f64::round, f64::round_ties_even

**Cargo / rustc / rustdoc**
- Cargo: add http.proxy-cainfo config to point at proxy CA certificates
- Cargo: use gix (the pure-Rust git implementation) for cargo package
- Cargo: stabilize multi-package publishing (publishing several workspace packages in one cargo publish invocation)
- Rustdoc: collapse all impl blocks via a Summary button and the shift+"-" keyboard shortcut
- Rustdoc: display unsafe attributes wrapped in unsafe() syntax

**Compatibility notes**
- lld is now the default linker on x86_64-unknown-linux-gnu; behavior differences vs GNU ld can surface (custom linker scripts, exotic flags). Override with -C linker-features=-lld or by setting an explicit linker.
- core::iter::Fuse's Default impl now constructs I::default() internally instead of always producing an empty iterator; code relying on Fuse::default() being empty changes behavior.
- MSG_NOSIGNAL is now set for UnixStream; programs that relied on receiving SIGPIPE signals should update their socket error handling.
- On Unix, std::env::home_dir now uses a fallback when the HOME environment variable is set but empty.
- Unsupported extern "{abi}" specifications are now rejected consistently in all positions.
- Const-eval now errors when initializing a static writes to that same static.
- The proc_macro_derive macro's arguments are now checked for correctness when it is applied to the crate root.
- Tier 3 musl targets now link dynamically by default (mips64-unknown-linux-muslabi64, powerpc64-unknown-linux-musl, powerpc-unknown-linux-musl, powerpc-unknown-linux-muslspe, riscv32gc-unknown-linux-musl, s390x-unknown-linux-musl, thumbv7neon-unknown-linux-musleabihf), changing the previous static-linking default.
- x86_64-apple-darwin (Intel macOS) demoted to Tier 2 with host tools, lowering its support guarantees.
- Some unsized tuple impls were removed since unsized tuples can't actually be constructed.

---

## Rust 1.91.0 (2025-10-30)

**Headline:** Promote aarch64-pc-windows-msvc to Tier 1, and gnullvm Windows targets to Tier 2 with host tools; Stabilize C-style variadic functions for sysv64, win64, efiapi, and aapcs ABIs; Large batch of stabilized APIs: AtomicPtr pointer/byte arithmetic, integer strict_* operations, char-boundary helpers, Ipv4Addr/Ipv6Addr::from_octets/from_segments, BTreeMap/BTreeSet::extract_if; Cargo stabilizes build.build-dir for intermediate build artifacts; Edition 2024: temporary scope restrictions for pin!/format_args!/write!/writeln!

**Language**
- C-style variadic functions for sysv64, win64, efiapi, and aapcs ABIs — Declaration of C-style variadic functions (`...`) is stabilized for the `sysv64`, `win64`, `efiapi`, and `aapcs` calling conventions, broadening beyond the previously-supported C ABI.
- Pattern binding lowering and drop order based on primary bindings — Pattern bindings are now lowered in the order they are written, and drop order is based on the order of the primary bindings.
- dangling_pointers_from_locals lint — New lint `dangling_pointers_from_locals` warns against creating dangling pointers from local variables.
- semicolon_in_expressions_from_macros upgraded to deny — The `semicolon_in_expressions_from_macros` lint was upgraded from warn-by-default to deny-by-default.
- LoongArch32 inline assembly — Inline assembly (`asm!`) is stabilized for the LoongArch32 architecture.
- integer_to_ptr_transmutes lint — New warn-by-default lint `integer_to_ptr_transmutes` flags transmuting an integer directly to a pointer.
- sse4a and tbm target features — The `sse4a` and `tbm` x86 target features are stabilized for use with `#[target_feature]` and `cfg(target_feature)`.
- target_env = "macabi" and target_env = "sim" cfgs — Added `target_env = "macabi"` and `target_env = "sim"` cfgs as replacements for the `target_abi` cfgs with the same values.

**Stabilized APIs**
- `Path::file_prefix` — Returns the prefix of the file name before the first dot.
- `AtomicPtr::fetch_ptr_add` — Atomically adds to the pointer using pointer arithmetic.
- `AtomicPtr::fetch_ptr_sub` — Atomically subtracts from the pointer using pointer arithmetic.
- `AtomicPtr::fetch_byte_add` — Atomically adds a byte offset to the pointer.
- `AtomicPtr::fetch_byte_sub` — Atomically subtracts a byte offset from the pointer.
- `AtomicPtr::fetch_or` — Atomically ORs a mask into the pointer's address bits.
- `AtomicPtr::fetch_and` — Atomically ANDs a mask into the pointer's address bits.
- `AtomicPtr::fetch_xor` — Atomically XORs a mask into the pointer's address bits.
- `{integer}::strict_add` — Strict addition that panics on overflow.
- `{integer}::strict_sub` — Strict subtraction that panics on overflow.
- `{integer}::strict_mul` — Strict multiplication that panics on overflow.
- `{integer}::strict_div` — Strict division that panics on overflow.
- `{integer}::strict_div_euclid` — Strict Euclidean division that panics on overflow.
- `{integer}::strict_rem` — Strict remainder that panics on overflow.
- `{integer}::strict_rem_euclid` — Strict Euclidean remainder that panics on overflow.
- `{integer}::strict_neg` — Strict negation that panics on overflow.
- `{integer}::strict_shl` — Strict left shift that panics if the shift amount is too large.
- `{integer}::strict_shr` — Strict right shift that panics if the shift amount is too large.
- `{integer}::strict_pow` — Strict exponentiation that panics on overflow.
- `i{N}::strict_add_unsigned` — Strict addition of an unsigned value to a signed integer, panicking on overflow.
- `i{N}::strict_sub_unsigned` — Strict subtraction of an unsigned value from a signed integer, panicking on overflow.
- `i{N}::strict_abs` — Strict absolute value that panics on overflow.
- `u{N}::strict_add_signed` — Strict addition of a signed value to an unsigned integer, panicking on overflow.
- `u{N}::strict_sub_signed` — Strict subtraction of a signed value from an unsigned integer, panicking on overflow.
- `PanicHookInfo::payload_as_str` — Returns the panic payload as a string slice if it is a &str or String.
- `core::iter::chain` — Free function form of Iterator::chain that joins two iterators.
- `u{N}::checked_signed_diff` — Computes the signed difference of two unsigned integers, returning None on overflow.
- `core::array::repeat` — Creates an array by repeating a cloned value N times.
- `PathBuf::add_extension` — Appends an additional extension to the path's file name.
- `PathBuf::with_added_extension` — Returns a new PathBuf with an additional extension appended.
- `Duration::from_mins` — Creates a Duration from a number of minutes.
- `Duration::from_hours` — Creates a Duration from a number of hours.
- `impl PartialEq<str> for PathBuf` — Compare a PathBuf against a str directly.
- `impl PartialEq<String> for PathBuf` — Compare a PathBuf against a String directly.
- `impl PartialEq<str> for Path` — Compare a Path against a str directly.
- `impl PartialEq<String> for Path` — Compare a Path against a String directly.
- `impl PartialEq<PathBuf> for String` — Compare a String against a PathBuf directly.
- `impl PartialEq<Path> for String` — Compare a String against a Path directly.
- `impl PartialEq<PathBuf> for str` — Compare a str against a PathBuf directly.
- `impl PartialEq<Path> for str` — Compare a str against a Path directly.
- `Ipv4Addr::from_octets` — Constructs an Ipv4Addr from a 4-byte octet array.
- `Ipv6Addr::from_octets` — Constructs an Ipv6Addr from a 16-byte octet array.
- `Ipv6Addr::from_segments` — Constructs an Ipv6Addr from an array of eight 16-bit segments.
- `impl<T> Default for Pin<Box<T>>` — Default impl producing a pinned, boxed default value.
- `impl<T> Default for Pin<Rc<T>>` — Default impl producing a pinned, Rc-wrapped default value.
- `impl<T> Default for Pin<Arc<T>>` — Default impl producing a pinned, Arc-wrapped default value.
- `Cell::as_array_of_cells` — Views a &Cell<[T; N]> as a &[Cell<T>; N].
- `u{N}::carrying_add` — Addition with carry-in and carry-out for wide-integer arithmetic.
- `u{N}::borrowing_sub` — Subtraction with borrow-in and borrow-out for wide-integer arithmetic.
- `u{N}::carrying_mul` — Full-width multiplication returning low and high halves with a carry.
- `u{N}::carrying_mul_add` — Full-width multiply-add returning low and high halves with a carry.
- `BTreeMap::extract_if` — Removes and yields entries matching a predicate via a draining iterator.
- `BTreeSet::extract_if` — Removes and yields elements matching a predicate via a draining iterator.
- `impl Debug for windows::ffi::EncodeWide<'_>` — Debug formatting for the Windows EncodeWide iterator.
- `str::ceil_char_boundary` — Rounds a byte index up to the nearest char boundary.
- `str::floor_char_boundary` — Rounds a byte index down to the nearest char boundary.
- `impl Sum for Saturating<u{N}>` — Summing an iterator of Saturating<u{N}> values with saturating arithmetic.
- `impl Sum<&Self> for Saturating<u{N}>` — Summing an iterator of &Saturating<u{N}> references.
- `impl Product for Saturating<u{N}>` — Multiplying an iterator of Saturating<u{N}> values with saturating arithmetic.
- `impl Product<&Self> for Saturating<u{N}>` — Multiplying an iterator of &Saturating<u{N}> references.

**Const-stabilized:** <[T; N]>::each_ref, <[T; N]>::each_mut, OsString::new, PathBuf::new, TypeId::of, ptr::with_exposed_provenance, ptr::with_exposed_provenance_mut

**Cargo / rustc / rustdoc**
- Cargo: Stabilize build.build-dir configuration for intermediate build artifacts (separating intermediate artifacts from the final output dir).
- Cargo: The --target flag and build.target configuration now accept the literal "host-tuple" string to mean the host target.
- Cargo: cargo publish no longer keeps .crate tarballs as final build artifacts when build.build-dir is set.
- Cargo: Adjust Cargo messages to match rustc diagnostic style.
- Rustdoc: In search results, rank doc aliases lower than non-alias items with the same name.
- Rustdoc: Raw pointers now work in type-based search like references.

**Compatibility notes**
- Always require coroutine captures to be drop-live.
- Apple: Always pass the SDK root when linking with cc, via the SDKROOT env var.
- Relaxed bounds in associated type bound position are now correctly forbidden.
- Add unstable #[sanitize(xyz = "on|off")] built-in attribute.
- Fix the drop checker being more permissive for bindings declared with let-else.
- Be more strict when parsing attributes, erroring on many invalid attributes.
- Mark all deprecation lints in name resolution as deny-by-default.
- semicolon_in_expressions_from_macros lint is now deny-by-default.
- Trait impl modifiers in inherent impls are no longer syntactically valid.
- Start reporting future breakage for ill_formed_attribute_input in dependencies.
- Restrict the scope of temporaries created by pin!, format_args!, write!, and writeln! in Rust Edition 2024.
- Invalid numeric literal suffixes in indexing and struct field positions are now rejected.
- Closures marked with the static keyword are now syntactically invalid.
- Shebangs inside --cfg and --check-cfg arguments are no longer allowed.
- Add a future-incompatibility lint for temporary lifetime shortening in Rust 1.92.
- cargo publish no longer keeps .crate tarballs as final build artifacts when build.build-dir is set.
- Adjust Cargo messages to match rustc diagnostic style.
- Tools relying on internal build-dir details may require updates.
- Compiler: Don't warn on never-to-any as casts as unreachable.
- Upgrade semicolon_in_expressions_from_macros from warn to deny.

---

## Rust 1.91.1 (2025-11-10) — point release (bug/security fixes only)

**Headline:** Patch release with two bug fixes: illumos Cargo build-directory file locking, and a cross-crate wasm_import_module fix for WASM linker errors; No language features or stabilized APIs in this point release

**Cargo / rustc / rustdoc**
- Enable file locking support on illumos (rust-lang/rust#148322): fixes Cargo not locking the build directory on illumos

**Compatibility notes**
- Fix `wasm_import_module` attribute cross-crate (rust-lang/rust#148363): fixes linker errors on WASM targets where the attribute was not honored across crate boundaries

---

## Rust 1.92.0 (2025-12-11)

**Headline:** MaybeUninit representation and validity now documented (a guarantee, not just docs); &raw const / &raw mut now allowed on union fields in safe code; Two never-type lints became deny-by-default, plus invalid_macro_export_arguments; -C panic=abort now produces usable backtraces on Linux by default; RwLockWriteGuard::downgrade and a batch of new_zeroed constructors stabilized

**Language**
- Documented MaybeUninit representation and validity — MaybeUninit<T> now has a documented, guaranteed layout and validity contract: it has the same size, alignment, and ABI as T, and any bit pattern (including uninitialized) is valid. This turns previously-informal behavior into a stable guarantee you can rely on for FFI and low-level buffer work.
- &raw const / &raw mut on union fields in safe code — You can now take a raw pointer to a union field via &raw const u.field or &raw mut u.field in safe code. Previously forming the raw pointer required an unsafe block even though only the pointer (not a read) was being created.
- Item bounds of associated types prioritized over where-bounds for auto-traits and Sized — When resolving auto-trait (e.g. Send/Sync) and Sized obligations on an associated type, the compiler now prefers the bounds declared on the associated type itself over where-clause bounds. This makes trait resolution more predictable and can newly accept (or reject) some generic code.
- Multiple bounds for the same associated item — You can now specify several bounds for the same associated item in a single bound list (e.g. multiple constraints on one associated type), except inside trait objects.
- Combining #[track_caller] and #[no_mangle] — A function may now carry both #[track_caller] and #[no_mangle] at once, which previously conflicted. Useful for exported entry points that still want accurate caller-location reporting.
- never_type_fallback_flowing_into_unsafe and dependency_on_unit_never_type_fallback now deny-by-default — Both never-type-fallback lints are now deny-by-default. They fire when the eventual change of never-type fallback (from () to !) would change which code runs in or around unsafe blocks, forcing you to annotate the type explicitly instead of relying on inference.
- unused_must_use no longer warns on uninhabited Result/ControlFlow — The unused_must_use lint stops warning when the unused value is Result<(), Uninhabited> or ControlFlow<Uninhabited, ()> — both are effectively infallible, so dropping them carries no risk worth a warning.
- Prevented materialization of X in [X; 0] when X is unsizing a const — The compiler no longer materializes the element value X in a zero-length array [X; 0] when X would require unsizing a const. Closes a soundness/codegen corner case around zero-length arrays of consts.
- Strengthened higher-ranked region handling in coherence — Coherence checking now handles higher-ranked (for<'a>) regions slightly more strictly, which can change whether certain overlapping-impl checks pass.

**Stabilized APIs**
- `NonZero::<u{N}>::div_ceil` — Ceiling division for NonZero unsigned integers, returning NonZero.
- `core::panic::Location::file_as_c_str` — Get the panic Location's file path as a &CStr (nul-terminated), useful for FFI/no_std error reporting.
- `std::sync::RwLockWriteGuard::downgrade` — Atomically downgrade a write guard into a read guard without releasing the lock in between.
- `Box::new_zeroed` — Allocate a Box<MaybeUninit<T>> with zeroed memory.
- `Box::new_zeroed_slice` — Allocate a Box<[MaybeUninit<T>]> slice with zeroed memory.
- `Rc::new_zeroed` — Allocate an Rc<MaybeUninit<T>> with zeroed memory.
- `Rc::new_zeroed_slice` — Allocate an Rc<[MaybeUninit<T>]> slice with zeroed memory.
- `Arc::new_zeroed` — Allocate an Arc<MaybeUninit<T>> with zeroed memory.
- `Arc::new_zeroed_slice` — Allocate an Arc<[MaybeUninit<T>]> slice with zeroed memory.
- `std::collections::btree_map::Entry::insert_entry` — Insert a value into a BTreeMap entry and return the OccupiedEntry instead of just &mut V.
- `std::collections::btree_map::VacantEntry::insert_entry` — Insert into a vacant BTreeMap entry and get back the resulting OccupiedEntry.
- `impl Extend<proc_macro::Group> for proc_macro::TokenStream` — Extend a TokenStream directly from an iterator of proc_macro Groups (proc-macro authoring).
- `impl Extend<proc_macro::Literal> for proc_macro::TokenStream` — Extend a TokenStream directly from an iterator of proc_macro Literals.
- `impl Extend<proc_macro::Punct> for proc_macro::TokenStream` — Extend a TokenStream directly from an iterator of proc_macro Puncts.
- `impl Extend<proc_macro::Ident> for proc_macro::TokenStream` — Extend a TokenStream directly from an iterator of proc_macro Idents.

**Const-stabilized:** <[T]>::rotate_left is now usable in const contexts, <[T]>::rotate_right is now usable in const contexts

**Cargo / rustc / rustdoc**
- Cargo: added a new chapter, 'Optimizing Build Performance', to the Cargo book
- Rustdoc: when trait items appear in search results, impl items are now hidden to reduce redundancy
- Rustdoc: relaxed identifier search rules — searches no longer require valid Rust syntax
- Compiler: mips64el-unknown-linux-muslabi64 now links dynamically
- Compiler: removed code for embedding command-line args in PDB files

**Compatibility notes**
- Backtraces with -C panic=abort on Linux are fixed by generating unwind tables by default; this increases binary size slightly. Disable with -C force-unwind-tables=no if you must.
- The invalid_macro_export_arguments lint is now deny-by-default; macros with invalid #[macro_export] arguments will fail to compile.
- The never_type_fallback_flowing_into_unsafe and dependency_on_unit_never_type_fallback lints are now deny-by-default; never-type fallback near unsafe may break builds until types are annotated explicitly.
- Minimum supported external LLVM bumped to version 20.
- Downstream impl DerefMut for Pin<LocalType> is now prevented (closes a Pin soundness hole); such impls no longer compile.
- Temporary lifetime extension is disabled for non-extended pin! and for formatting-macro arguments; temporaries that were previously kept alive may now drop earlier, which can change borrow/drop behavior.
- iter::Repeat::last and iter::Repeat::count now panic instead of looping forever — code that previously hung will now abort with a panic.
- unused_must_use no longer warns on Result<(), Uninhabited> or ControlFlow<Uninhabited, ()> (reduced lint noise, not a break).

---

## Rust 1.93.0 (2026-01-22)

**Headline:** Stabilized s390x vector target features and is_s390x_feature_detected! macro; C-style variadic functions declarable for the system ABI; asm_cfg stabilized; Many new MaybeUninit slice, raw-parts, unchecked-arithmetic, and slice-as-array APIs; Two new warn-by-default lints: const_item_interior_mutations and function_casts_as_integer; -Cjump-tables=bool stabilized (was -Zno-jump-tables)

**Language**
- s390x vector target features and is_s390x_feature_detected! — Stabilizes several s390x vector-related target features along with the is_s390x_feature_detected! runtime feature-detection macro.
- C-style variadic functions for the system ABI — Allows declaring C-style variadic functions (using ...) for the system ABI, not just C/cdecl.
- Error on keyword used as a cfg predicate — Using certain keywords as a cfg predicate now emits an error instead of being accepted.
- asm_cfg — Stabilizes asm_cfg, allowing cfg attributes on individual operands and lines within asm! / global_asm!.
- const-eval: copy pointers byte-by-byte — During const-evaluation, copying pointers byte-by-byte is now supported.
- LUB coercions for function item types and differing safeties — Least-upper-bound coercions now correctly handle function item types and functions with differing safeties.
- const items containing mutable references to static — Allows const items that contain mutable references to a static. This is very unsafe but is not always UB, so it is now permitted.
- const_item_interior_mutations lint — New warn-by-default lint that warns against calls which mutate interior-mutable const items.
- function_casts_as_integer lint — New warn-by-default lint that warns when a function item or fn pointer is cast directly to an integer.

**Stabilized APIs**
- `<[MaybeUninit<T>]>::assume_init_drop` — Drops the initialized contents of a slice of MaybeUninit<T> in place.
- `<[MaybeUninit<T>]>::assume_init_ref` — Returns &[T] viewing the slice as initialized.
- `<[MaybeUninit<T>]>::assume_init_mut` — Returns &mut [T] viewing the slice as initialized.
- `<[MaybeUninit<T>]>::write_copy_of_slice` — Copies the elements from a &[T] into the MaybeUninit slice via bitwise copy.
- `<[MaybeUninit<T>]>::write_clone_of_slice` — Clones the elements from a &[T] into the MaybeUninit slice.
- `String::into_raw_parts` — Decomposes a String into its raw pointer, length, and capacity.
- `Vec::into_raw_parts` — Decomposes a Vec into its raw pointer, length, and capacity.
- `<iN>::unchecked_neg` — Negation without checking for overflow (unsafe).
- `<iN>::unchecked_shl` — Left shift without checking that the shift amount is in range (unsafe).
- `<iN>::unchecked_shr` — Right shift without checking that the shift amount is in range (unsafe).
- `<uN>::unchecked_shl` — Left shift without checking that the shift amount is in range (unsafe).
- `<uN>::unchecked_shr` — Right shift without checking that the shift amount is in range (unsafe).
- `<[T]>::as_array` — Returns Option<&[T; N]> viewing a prefix of the slice as a fixed-size array.
- `<[T]>::as_mut_array` — Returns Option<&mut [T; N]> viewing a prefix of the slice as a fixed-size array.
- `<*const [T]>::as_array` — Converts a raw slice pointer to Option<*const [T; N]>.
- `<*mut [T]>::as_mut_array` — Converts a raw mutable slice pointer to Option<*mut [T; N]>.
- `VecDeque::pop_front_if` — Pops the front element only if it satisfies a predicate.
- `VecDeque::pop_back_if` — Pops the back element only if it satisfies a predicate.
- `Duration::from_nanos_u128` — Constructs a Duration from a u128 number of nanoseconds.
- `char::MAX_LEN_UTF8` — Associated constant: the maximum number of bytes a char occupies when UTF-8 encoded.
- `char::MAX_LEN_UTF16` — Associated constant: the maximum number of u16 code units a char occupies when UTF-16 encoded.
- `std::fmt::from_fn` — Creates a Display/Debug-able value from a closure that writes to a Formatter.
- `std::fmt::FromFn` — The type returned by std::fmt::from_fn implementing the formatting traits.

**Cargo / rustc / rustdoc**
- Cargo: Enable CARGO_CFG_DEBUG_ASSERTIONS in build scripts based on the profile.
- Cargo: In `cargo tree`, support long forms for `--format` variables.
- Cargo: Add `--workspace` to `cargo clean`.
- Rustdoc: Remove `#![doc(document_private_items)]`.
- Rustdoc: Include attribute and derive macros in the search filters for "macros".
- Rustdoc: Include extern crates in the search filters for `import`.
- Rustdoc: Validate usage of crate-level doc attributes; html_favicon_url, html_logo_url, html_playground_url, issue_tracker_base_url, or html_no_source with a missing/unexpected/wrong-type value now emit the deny-by-default lint rustdoc::invalid_doc_attributes.
- Compiler: Stabilize `-Cjump-tables=bool` (previously the unstable `-Zno-jump-tables`).

**Compatibility notes**
- Standard library stops internally using `specialization` on the `Copy` trait (it is unsound with lifetime-dependent Copy impls). Some std APIs may now call `Clone::clone` instead of doing bitwise copies, which may cause performance regressions.
- The global allocator may now use thread-local storage and `std::thread::current()`.
- BTree::append no longer updates existing keys when appending an entry whose key already exists.
- vec::IntoIter<T>: UnwindSafe no longer requires T: RefUnwindSafe.
- Introduce `pin_v2` into the builtin attributes namespace.
- Update bundled musl to 1.2.5.
- On Emscripten, the panic=unwind ABI changed from the JS exception handling ABI to the wasm exception handling ABI. Linking C/C++ object files with Rust objects now requires passing `-fwasm-exceptions` to the linker. The old behavior is available on nightly via `-Zwasm-emscripten-eh=false -Zbuild-std` but will be removed in a future release.
- The `#[test]` attribute is no longer ignored in places where it has no meaning (e.g. trait methods, types, structs); applying it there is now an error and may also cause errors when generating rustdoc.
- Cargo now sets CARGO_CFG_DEBUG_ASSERTIONS in more situations, causing crates depending on `static-init` 1.0.1 to 1.0.3 to fail compilation with "failed to resolve: use of unresolved module or unlinked crate `parking_lot`".
- User-written types in the `offset_of!` macro are now checked to be well-formed.
- `cargo publish` no longer emits `.crate` files as a final artifact for user access when the `build.build-dir` config is unset.
- Upgrade the `deref_nullptr` lint from warn-by-default to deny-by-default.
- Add a future-incompatibility warning for `...` function parameters without a pattern outside of `extern` blocks.
- Introduce a future-compatibility warning for `repr(C)` enums whose discriminant values do not fit into a `c_int` or `c_uint`.
- Introduce a future-compatibility warning against ignoring `repr(C)` types as part of `repr(transparent)`.
- Using certain keywords as a `cfg` predicate now emits an error.

---

## Rust 1.93.1 (2026-02-12) — point release (bug/security fixes only)

**Headline:** Patch release of Rust 1.93 containing only bug fixes; no new features or stabilizations.; Fixes an ICE from trying to recover a keyword as a non-keyword identifier (notably affected rustfmt).; Fixes a clippy::panicking_unwrap false-positive on field access with implicit deref.; Reverts a CI wasm-dependency update that caused file descriptor leaks on the wasm32-wasip2 target.

---

## Rust 1.94.0 (2026-03-05)

**Headline:** 29 additional RISC-V target features stabilized, covering large parts of the RVA22U64 / RVA23U64 profiles; New warn-by-default unused_visibilities lint flags visibility modifiers on `const _` declarations; Unicode tables updated to Unicode 17; Slice array_windows and element_offset, plus LazyCell/LazyLock getters, stabilized; Several dyn-type, closure-capture, and macro-import behaviors tightened with compatibility notes

**Language**
- Impl/impl-item dead_code lint inheritance — Impls and impl items now inherit the dead_code lint level of the corresponding traits and trait items, so silencing dead_code on a trait or trait item also covers its implementations.
- 29 additional RISC-V target features stabilized — Stabilizes 29 more RISC-V target features, including large portions of the RVA22U64 / RVA23U64 application profiles, for use with target_feature/cfg.
- unused_visibilities lint — Adds a warn-by-default unused_visibilities lint that flags a visibility modifier written on a `const _` declaration, where the visibility has no effect.
- Update to Unicode 17 — Updates the standard library's Unicode tables to Unicode 17, affecting char classification and case mapping.
- Avoid incorrect lifetime errors for closures — The compiler no longer emits some incorrect lifetime errors for closures, so code that was wrongly rejected now compiles.

**Stabilized APIs**
- `<[T]>::array_windows` — Iterator over overlapping windows of a const-generic array length over a slice.
- `<[T]>::element_offset` — Returns the index of an element within a slice given a reference to it.
- `LazyCell::get` — Returns a reference to the value if the LazyCell has been initialized.
- `LazyCell::get_mut` — Returns a mutable reference to the value if the LazyCell has been initialized.
- `LazyCell::force_mut` — Forces evaluation and returns a mutable reference to the contained value.
- `LazyLock::get` — Returns a reference to the value if the LazyLock has been initialized.
- `LazyLock::get_mut` — Returns a mutable reference to the value if the LazyLock has been initialized.
- `LazyLock::force_mut` — Forces evaluation and returns a mutable reference to the contained value.
- `impl TryFrom<char> for usize` — Fallible conversion from a char to usize via its scalar value.
- `std::iter::Peekable::next_if_map` — Consumes and maps the next item if a closure returns Some for it.
- `std::iter::Peekable::next_if_map_mut` — Like next_if_map but passes the peeked item by mutable reference to the closure.
- `x86 avx512fp16 intrinsics` — x86/x86_64 AVX-512 FP16 SIMD intrinsics in core::arch.
- `AArch64 NEON fp16 intrinsics` — AArch64 NEON half-precision floating-point intrinsics in core::arch.
- `f32::consts::EULER_GAMMA` — The Euler-Mascheroni constant as an f32.
- `f64::consts::EULER_GAMMA` — The Euler-Mascheroni constant as an f64.
- `f32::consts::GOLDEN_RATIO` — The golden ratio constant as an f32.
- `f64::consts::GOLDEN_RATIO` — The golden ratio constant as an f64.

**Const-stabilized:** f32::mul_add, f64::mul_add

**Cargo / rustc / rustdoc**
- Cargo: stabilize the config `include` key for loading additional configuration files.
- Cargo: stabilize the `pubtime` field in the registry index, recording publication timestamps.
- Cargo: now parses TOML v1.1 for manifests and configuration files.
- Cargo: make `CARGO_BIN_EXE_<crate>` available at runtime (in addition to build time).

**Compatibility notes**
- Forbid freely casting lifetime bounds of `dyn`-types.
- Make closure capturing have consistent and correct behaviour around patterns (may change what a closure captures).
- Standard library macros are now imported via the prelude, not via an injected `#[macro_use]`.
- Don't strip the shebang line in expression-context `include!(...)`s.
- Ambiguous glob reexports are now also visible cross-crate.
- Don't normalize where-clauses before checking well-formedness.
- Introduce a future-compatibility warning on codegen attributes placed on body-free trait methods.
- On Windows, `std::time::SystemTime::checked_sub_duration` now returns `None` for times before the Windows epoch (1/1/1601).
- Lifetime identifiers such as `'a` are now NFC normalized.
- Overhaul filename handling for cross-compiler consistency.

---

## Rust 1.94.1 (2026-03-26) — point release (bug/security fixes only)

**Headline:** Patch release: fixes a wasm32-wasip1-threads regression in std::thread::spawn; Reverts unstable methods added to std::os::windows::fs::OpenOptionsExt; Clippy ICE fix in match_same_arms; Cargo bundles tar 0.4.45 for CVE-2026-33055 / CVE-2026-33056 (crates.io users unaffected)

**Cargo / rustc / rustdoc**
- Cargo: update the bundled tar crate to 0.4.45, addressing CVE-2026-33055 and CVE-2026-33056. Users of crates.io are not affected.

**Compatibility notes**
- Fix std::thread::spawn on the wasm32-wasip1-threads target (regression fix; spawning threads there was broken).
- Remove the new (unstable) methods added to std::os::windows::fs::OpenOptionsExt. The methods were unstable, but the trait itself is not sealed and cannot be extended with non-default methods, so they had to be reverted.
- Clippy: fix an internal compiler error (ICE) in the match_same_arms lint.

---

## Rust 1.95.0 (2026-04-16)

**Headline:** Stabilize if let guards on match arms; core::range types (RangeInclusive) stabilized under mod core::range; cfg_select! macro stabilized; Inline assembly stabilized for PowerPC and PowerPC64; Atomic update/try_update methods stabilized across all atomic types; Vec/VecDeque/LinkedList *_mut push/insert methods return a reference to the inserted element

**Language**
- if let guards on match arms — `if let` guards are now stable in match arms, letting a match arm both refine a pattern and bind from a fallible match in the guard position.
- irrefutable_let_patterns lint no longer fires on let chains — The `irrefutable_let_patterns` lint stops linting on let chains, so an irrefutable `let` pattern used inside a let chain no longer triggers a warning.
- Import path-segment keywords with renaming — You can now import path-segment keywords with renaming in a `use`, supporting `use` paths that pass through keyword segments via an alias.
- Inline assembly for PowerPC and PowerPC64 — `asm!`/`global_asm!` inline assembly is stabilized for the PowerPC and PowerPC64 architectures.
- const-eval typed-copy padding consistency — const-eval is now more consistent in how padding bytes behave during typed copies; the operational semantics of padding during a typed copy were tightened.
- Const blocks no longer force fallible-op promotion checks — Const blocks are no longer evaluated to determine whether expressions involving fallible operations can be implicitly const-promoted, changing when implicit constant promotion applies.
- Pattern-matching semantics independent of crate and module — The operational semantics of pattern matching are now independent of the crate and module in which the match occurs, removing crate/module-dependent matching behavior.

**Stabilized APIs**
- `MaybeUninit<[T; N]>: From<[MaybeUninit<T>; N]>` — Convert an array of MaybeUninit into a MaybeUninit of an array.
- `MaybeUninit<[T; N]>: AsRef<[MaybeUninit<T>; N]>` — Borrow a MaybeUninit array as an array reference.
- `MaybeUninit<[T; N]>: AsRef<[MaybeUninit<T>]>` — Borrow a MaybeUninit array as a slice reference.
- `MaybeUninit<[T; N]>: AsMut<[MaybeUninit<T>; N]>` — Mutably borrow a MaybeUninit array as an array reference.
- `MaybeUninit<[T; N]>: AsMut<[MaybeUninit<T>]>` — Mutably borrow a MaybeUninit array as a slice reference.
- `[MaybeUninit<T>; N]: From<MaybeUninit<[T; N]>>` — Convert a MaybeUninit of an array into an array of MaybeUninit.
- `Cell<[T; N]>: AsRef<[Cell<T>; N]>` — View a Cell of an array as an array of Cells.
- `Cell<[T; N]>: AsRef<[Cell<T>]>` — View a Cell of an array as a slice of Cells.
- `Cell<[T]>: AsRef<[Cell<T>]>` — View a Cell of a slice as a slice of Cells.
- `bool: TryFrom<{integer}>` — Fallibly convert an integer to bool (0 -> false, 1 -> true, otherwise error).
- `AtomicPtr::update` — Atomically apply a closure to update the pointer value.
- `AtomicPtr::try_update` — Atomically apply a fallible closure to update the pointer value.
- `AtomicBool::update` — Atomically apply a closure to update the bool value.
- `AtomicBool::try_update` — Atomically apply a fallible closure to update the bool value.
- `AtomicI8/I16/I32/I64/Isize::update` — Atomically apply a closure to update signed integer atomics (page renders this as AtomicIn::update).
- `AtomicI8/I16/I32/I64/Isize::try_update` — Atomically apply a fallible closure to update signed integer atomics (page renders this as AtomicIn::try_update).
- `AtomicU8/U16/U32/U64/Usize::update` — Atomically apply a closure to update unsigned integer atomics (page renders this as AtomicUn::update).
- `AtomicU8/U16/U32/U64/Usize::try_update` — Atomically apply a fallible closure to update unsigned integer atomics (page renders this as AtomicUn::try_update).
- `cfg_select!` — Macro that selects one of several token trees based on cfg predicates, like a match over cfg conditions.
- `mod core::range` — Stabilizes the core::range module hosting the new range types.
- `core::range::RangeInclusive` — The new ergonomic inclusive range type in core::range.
- `core::range::RangeInclusiveIter` — Iterator over the new core::range::RangeInclusive.
- `core::hint::cold_path` — Hint to the optimizer that the current code path is cold/unlikely.
- `<*const T>::as_ref_unchecked` — Convert a const raw pointer to a shared reference without the null check.
- `<*mut T>::as_ref_unchecked` — Convert a mut raw pointer to a shared reference without the null check.
- `<*mut T>::as_mut_unchecked` — Convert a mut raw pointer to a mutable reference without the null check.
- `Vec::push_mut` — Push a value and return a mutable reference to the newly inserted element.
- `Vec::insert_mut` — Insert a value at an index and return a mutable reference to it.
- `VecDeque::push_front_mut` — Push to the front and return a mutable reference to the inserted element.
- `VecDeque::push_back_mut` — Push to the back and return a mutable reference to the inserted element.
- `VecDeque::insert_mut` — Insert at an index and return a mutable reference to the inserted element.
- `LinkedList::push_front_mut` — Push to the front and return a mutable reference to the inserted element.
- `LinkedList::push_back_mut` — Push to the back and return a mutable reference to the inserted element.
- `Layout::dangling_ptr` — Return a well-aligned dangling pointer for the layout.
- `Layout::repeat` — Compute the layout of an array of n copies, returning the layout and stride/offset.
- `Layout::repeat_packed` — Compute the packed (no inter-element padding) layout of n repeats.
- `Layout::extend_packed` — Extend a layout with another, packed without trailing padding.

**Const-stabilized:** fmt::from_fn, ControlFlow::is_break, ControlFlow::is_continue

**Cargo / rustc / rustdoc**
- Rustdoc: rank unstable items lower in search results
- Rustdoc: add a new "hide deprecated items" setting
- Compiler: stabilize --remap-path-scope for controlling the scoping of how paths get remapped in the resulting binary
- Compatibility: JSON target specs destabilized and now require -Z unstable-options
- Compatibility: arguments of #[feature] attributes on invalid targets are now checked

**Compatibility notes**
- Array coercions may now result in fewer inference constraints than before
- Importing `$crate` without renaming via `use $crate::{self};` is no longer permitted
- const-eval: more consistent behavior of padding during typed copies
- `ambiguous_glob_imported_traits` future-incompatibility warning is now reported
- Lifetime bounds of types mentioning only type parameters are now checked
- More visibility-related ambiguous import errors are now reported
- `Eq::assert_receiver_is_total_eq` is deprecated; manual impls emit future-compatibility warnings
- powerpc64: the ELF ABI version is now taken from the target spec instead of being guessed
- Matching on a `#[non_exhaustive]` enum now reads the discriminant
- `mut ref` and `mut ref mut` patterns are now correctly feature-gated as unstable
- Future-compatibility warning added for derive helper attributes that conflict with built-in attributes
- JSON target specs destabilized and now require `-Z unstable-options`
- Arguments of `#[feature]` attributes on invalid targets are now checked

---

## Rust 1.96.0 (28 May, 2026)

**Headline:** Iterate over ranges of NonZero integers; assert_matches!/debug_assert_matches! stabilized; New core::range iterator types (Range, RangeFrom, RangeToInclusive and their *Iter); Allow passing the expr metavariable to cfg; Cargo: a dependency may specify both a git repository and an alternate registry (plus CVE-2026-5222/5223 fixes)

**Language**
- Allow passing `expr` metavariable to `cfg` — A macro `expr` metavariable (a fragment captured by a `macro_rules!` matcher) can now be passed through to `cfg`, so a captured expression fragment is accepted where `cfg` expects its argument.
- Always coerce never types in tuple expressions — Elements of type `!` (never) inside a tuple expression are now always coerced to the expected element type, making tuple expressions consistent with other coercion sites.
- Avoid incorrect inference guidance of function arguments in rare cases — Type inference no longer takes misleading guidance from function arguments in certain rare cases, fixing wrong or surprising inferred types.
- Support s390x vector registers in inline assembly — `asm!` on s390x targets can now use the platform's vector registers as operands.
- Allow using constants of type `ManuallyDrop` as patterns (fixing a regression introduced in 1.94.0) — A constant whose type is `ManuallyDrop<T>` may again be used in pattern position, restoring behavior that regressed in 1.94.0.

**Stabilized APIs**
- `assert_matches!` — Assert that an expression matches a given pattern, panicking with a diagnostic if it does not.
- `debug_assert_matches!` — Debug-only variant of assert_matches!; the pattern assertion is compiled out in optimized builds like debug_assert!.
- `From<T> for AssertUnwindSafe<T>` — Construct std::panic::AssertUnwindSafe<T> directly from a T via From/Into.
- `From<T> for LazyCell<T, F>` — Build a LazyCell that is already initialized with the given value (From the contained T).
- `From<T> for LazyLock<T, F>` — Build a LazyLock that is already initialized with the given value (From the contained T).
- `core::range::RangeToInclusive` — core::range version of the ..=end range type.
- `core::range::RangeToInclusiveIter` — Iterator type produced by a core::range RangeToInclusive.
- `core::range::RangeFrom` — core::range version of the start.. range type.
- `core::range::RangeFromIter` — Iterator type produced by a core::range RangeFrom.
- `core::range::Range` — core::range version of the start..end range type.
- `core::range::RangeIter` — Iterator type produced by a core::range Range.

**Cargo / rustc / rustdoc**
- Cargo: Allow a dependency to specify both a git repository and an alternate registry
- Cargo: Added `target.'cfg(..)'.rustdocflags` support in configuration
- Cargo: Fixed CVE-2026-5222 and CVE-2026-5223
- Rustdoc: Deprecation notes are now rendered like any other documentation
- Rustdoc: Don't emit rustdoc `missing_doc_code_examples` lint on impl items
- Rustdoc: Separate methods and associated functions in the sidebar

**Compatibility notes**
- Fix layout of `#[repr(Int)]` enums in some edge cases involving fields of uninhabited zero-sized types
- Prevent unsize-coercing into `Pin<Foo>` where `Foo` doesn't implement `Deref`
- rustc: Stop passing `--allow-undefined` on wasm targets
- Gate the accidentally stabilized `#![reexport_test_harness_main]` attribute
- Error on return-position-impl-trait-in-traits whose types are too private
- Report the `uninhabited_static` lint in dependencies and make it deny-by-default
- Distributed builds now contain non-split debuginfo for windows-gnu
- Check const generic arguments are correctly typed in more positions
- Remove -Csoft-float
- Importing structs with `::{self [as name]}` is now no longer permitted
- For `export_name`, `link_name`, and `link_section` attributes, the first occurrence takes precedence
- Update the minimum external LLVM to 21
- On `avr` targets, `c_double` changed to `f32` to match C's double

---

