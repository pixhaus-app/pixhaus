# Modern Rust: language and edition features (1.85-1.96)

Every language-level change in the window — let chains, if-let guards, async closures, trait upcasting, precise capturing, the unsafe-attribute syntax, repr128, and the lints that became errors — with old way, new way, and when to reach for each. Part of the `pixhaus-rust-modern` skill; start at its `SKILL.md` for the shortlist and the per-version cheat sheet.

This is the reference of record for every language-level change between Rust 1.85 and 1.96. The workspace pins toolchain 1.96 and edition 2024, so every feature in the window — 1.85 through 1.96 — compiles today. Each feature gives the old way, the new way, and when to reach for it. The rule throughout: a new feature earns its place by removing a real footgun or saying the same thing more clearly. Do not rewrite working code just to spend a new keyword.

## Edition 2024 is the baseline

We are on edition 2024 (1.85), set once per crate in `Cargo.toml`. Editions never split the ecosystem — a 2024 crate links against a 2021 dependency and vice versa — so this is a per-crate decision, not an ecosystem fork. The reason to know which edition you are on: several features below are edition-gated, and 2024 also changed runtime behavior (drop order, `if let` temporary scope, RPIT lifetime capture) that a `cargo fix --edition` migration handles mechanically.

```rust
// old: edition = "2021"
// new:
edition = "2024"
```

When to reach for it: it is already the default for every crate in this workspace. New crates inherit it. Do not author a crate on an older edition without a reason.

## let chains and if-let chains (1.88, edition 2024)

`&&`-chain `let` bindings with boolean tests inside `if` and `while` conditions, no nesting. This is the single biggest readability win in window for our command and tool code, where you constantly peel an `Option` then test it.

```rust
// old: a pyramid for every two-step guard
if let Some(cel) = layer.active_cel() {
    if cel.bounds().contains(cursor) {
        paint(cel, cursor);
    }
}

// new (1.88, edition 2024):
if let Some(cel) = layer.active_cel() && cel.bounds().contains(cursor) {
    paint(cel, cursor);
}
```

It is edition-gated to 2024 because temporaries in the chain drop at the end of the enclosing block, a change that could not be made in older editions. We are on 2024, so use it freely. When to reach for it: any `if let` whose body is a second `if`, or any guard that mixes a pattern match with a boolean. When not to: a single `if let` with no follow-up test reads fine as is — chaining one binding gains nothing.

## if let guards on match arms (1.95)

A match arm guard can now itself be an `if let`, binding new names visible in that arm's body. This is distinct from let-chains: it lives in the guard position of a `match`, not an `if`.

```rust
// old: re-match inside the arm, often with an unwrap you must justify
match tool_event {
    ToolEvent::Click(p) if pick_layer(p).is_some() => {
        let layer = pick_layer(p).expect("just checked");
        select(layer);
    }
    _ => {}
}

// new (1.95):
match tool_event {
    ToolEvent::Click(p) if let Some(layer) = pick_layer(p) => {
        select(layer);
    }
    _ => {}
}
```

When to reach for it: a guard that calls a fallible lookup and then the body redoes that lookup — a classic no-unwrap pain point. The `irrefutable_let_patterns` lint also stopped firing on let chains in 1.95, so an always-matching `let` inside a chain no longer warns.

## async closures and the AsyncFn family (1.85)

`async || { ... }` is now a first-class closure, with the `AsyncFn` / `AsyncFnMut` / `AsyncFnOnce` traits in the prelude on all editions. The difference that matters: an async closure can borrow from its captures across the returned future, so it can lend out data tied to each call's lifetime. A plain closure returning an `async move` block cannot.

```rust
// old: closure returns an async block; borrowing across the future is awkward
let run = || async move { backend.run(req).await };

// new (1.85):
let run = async || backend.run(req).await;
```

Bound these with the `AsyncFn*` traits, not `Fn() -> impl Future`, to get the borrowing behavior:

```rust
async fn dispatch<F>(backends: &[Arc<dyn Backend>], op: F)
where
    F: AsyncFn(&Arc<dyn Backend>) -> Result<Output, BackendError>,
{
    for b in backends {
        let _ = op(b).await;
    }
}
```

When to reach for it: higher-order async over our `Arc<dyn Backend>` runtime — a retry wrapper, a per-backend mapper, anything that takes an async callback and needs to pass it a borrowed argument. When not to: a one-shot `tokio::spawn` of a single future does not need a closure at all.

## async fn in traits and return-position impl Trait in traits (1.75, pre-window; RPITIT `use<..>` in 1.87)

Native `async fn` in traits and RPITIT both stabilized in 1.75, before this window opens, so they are not corpus items — but the relevant in-window change is precise capturing on them. As of 1.87 you can write a `use<...>` bound on a return-position `impl Trait` in a trait, naming exactly which generics and lifetimes the returned opaque type captures.

```rust
// old: RPITIT captured everything in scope, over-constraining the return
trait Thumbnailer {
    fn render(&self) -> impl Iterator<Item = Rgba>;
}

// new (1.87): say precisely what is captured
trait Thumbnailer {
    fn render(&self) -> impl Iterator<Item = Rgba> + use<Self>;
}
```

When to reach for it: a trait method whose returned iterator or future should not borrow `&self` (or some generic), but the default capture rules tie it down anyway. This is the trait-method parallel to the free-function `use<..>` bound that edition 2024 already applies. For the `async fn in trait` + `dyn` story (our `Arc<dyn Backend>`), load `pixhaus-async-trait` — native async-in-trait is static-dispatch only, so the trait-object case still needs care.

## Repeated bounds on one associated item (1.92)

`T: Trait<Assoc: A, Assoc: B>` — naming the same associated item twice in one bound list — now compiles, where before you combined them (`Assoc: A + B`) or pushed one into a `where` clause. The associated-item-bounds sugar itself predates this window; 1.92 is specifically about letting the bounds on one item accumulate as separate entries.

```rust
// old: combine the bounds on Item, or split to a where clause
fn sorted<I>(it: I) -> Vec<I::Item>
where I: Iterator<Item: Ord + Clone> { /* ... */ }

// new (1.92): the same associated item can carry separate bound entries
fn sorted<I: Iterator<Item: Ord, Item: Clone>>(it: I) -> Vec<I::Item> { /* ... */ }
```

When to reach for it: generated or macro-composed bounds where merging into `A + B` is awkward. For hand-written bounds, `Assoc: A + B` still reads fine. The broader generic-vs-`dyn` decision is `pixhaus-generics-dispatch`.

## gen blocks and yield

Not in window. The corpus reserves `gen` as a keyword in edition 2024 (part of the 2024 bundle) but does not stabilize `gen` blocks, `gen fn`, or `yield` anywhere from 1.85 to 1.96. Do not write `gen { ... }` or `yield` — they will not compile; the keyword is reserved, not stabilized. Build iterators by hand or with the `Iterator` adapters. When generators land, this section gets the entry; until then, treat the keyword as reserved-only.

## Trait upcasting to supertraits (1.86)

Coerce a `dyn Sub` to a `dyn Super` when `Sub: Super`, directly, with no helper method. The vtable carries the supertrait, so the upcast is a coercion.

```rust
// old: a hand-written climb to the base trait object
trait Verb {}
trait UndoableVerb: Verb {
    fn as_verb(&self) -> &dyn Verb;
}
let v: &dyn Verb = cmd.as_verb();

// new (1.86):
trait Verb {}
trait UndoableVerb: Verb {}
let cmd: &dyn UndoableVerb = &resize;
let v: &dyn Verb = cmd; // upcast coercion
```

When to reach for it: our command/verb trait hierarchies, where an undoable command is-a command and you want to hand the base trait object to code that does not care about undo. Delete the `as_super()`-style shims it replaces. The coercion only climbs to a transitive supertrait; it cannot reach an unrelated trait.

## Explicitly inferred const arguments with `_` (1.89)

Write `_` in const-generic position and let the compiler infer the value, exactly as `_` already infers a type.

```rust
// old: spell out the const, even when it is obvious from the initializer
let kernel: [f32; 3] = [0.25, 0.5, 0.25];

// new (1.89):
let kernel: [f32; _] = [0.25, 0.5, 0.25];
```

When to reach for it: fixed-size kernels, tile dimensions, and array-shaped data where the length is unambiguous from context and repeating it is noise. When not to: if the length is the thing a reader needs to see at a glance (a stride, a palette size), spell it out.

## 128-bit enum discriminants: `#[repr(u128)]` / `#[repr(i128)]` (1.89)

An enum can now carry a 128-bit discriminant. Niche here — our tag enums (`Tool`, `BlendMode`, command kinds) fit in a byte, and a `Copy` enum is sized by its largest variant anyway. But if a generated id space or a bitflag-style discriminant genuinely needs more than 64 bits of tag, the representation is stable rather than something you hand-pack into a `[u64; 2]`.

```rust
// new (1.89): a 128-bit discriminant, only where one is actually needed
#[repr(u128)]
enum FeatureBit { Base = 1, Far = 1 << 100 }
```

When to reach for it: almost never in this codebase — listed so you reach for it knowingly rather than reinventing it by hand.

## Never type and diverging changes

The never type `!` is not stabilized as a nameable type in window, but several diverging behaviors changed:

- 1.92 promoted `never_type_fallback_flowing_into_unsafe` and `dependency_on_unit_never_type_fallback` to deny-by-default. When never-type fallback (the eventual `()` to `!` change) would alter what runs in or near an `unsafe` block, you must annotate the diverging value's type explicitly. Fix by writing the type:

```rust
// new (1.92): annotate rather than lean on fallback
let _: () = return;
```

- 1.89 began reporting never-type future-incompatibility warnings inside dependencies.
- 1.96 makes `!` elements in a tuple expression always coerce to the expected element type, fixing inconsistent inference like `(1, return)` where the `!` element previously could fail to coerce.

When to reach for it: you do not reach for these — they are correctness tightenings. When the deny-by-default lints fire, add the annotation; do not `#[allow]` them.

## Unsafe attribute syntax: `#[unsafe(...)]` (edition 2024)

Attributes that can cause undefined behavior now carry an explicit `unsafe(...)` wrapper in edition 2024 — `#[unsafe(no_mangle)]`, `#[unsafe(export_name = ...)]`, `#[unsafe(naked)]`, `unsafe extern` blocks. The point is that an attribute can be as load-bearing for soundness as an `unsafe` block, so it should look like one. Rustdoc in 1.90 even renders these wrapped in `unsafe()`.

```rust
// old:
#[no_mangle]
pub extern "C" fn entry() {}

// new (edition 2024):
#[unsafe(no_mangle)]
pub extern "C" fn entry() {}
```

When to reach for it: any FFI export surface. We have almost none — Pixhaus is single-binary, not a `cdylib` — so this mostly matters when you touch the rare `extern` boundary. The migration is mechanical; `cargo fix --edition` applies it.

## The `#[diagnostic::*]` attribute family

A diagnostics-only namespace: these attributes change error messages, never trait resolution. Unknown options in the namespace are ignored rather than erroring, which is why 1.90 split the umbrella lint into `unknown_diagnostic_attributes`, `misplaced_diagnostic_attributes`, `malformed_diagnostic_attributes`, and `malformed_diagnostic_format_literals` so you can allow or deny each failure mode.

`#[diagnostic::do_not_recommend]` (1.85) tells the compiler not to suggest a particular impl in trait-error hints — useful for hiding a blanket or internal impl that would otherwise produce a misleading "trait not implemented" suggestion.

```rust
// new (1.85): keep an internal blanket impl out of error suggestions
#[diagnostic::do_not_recommend]
impl<T: InternalMarker> Exportable for T {}
```

When to reach for it: a library trait in `crates/ui` or `crates/services` whose blanket impl pollutes downstream error messages. It is cosmetic — it never changes what compiles.

## naked functions (1.88)

`#[unsafe(naked)]` with a body that is a single `naked_asm!` block gives a function no compiler-generated prologue or epilogue; you write the whole body in assembly.

```rust
// old: hand-written prologue-free functions needed global_asm! or external .s files

// new (1.88):
#[unsafe(naked)]
extern "C" fn trampoline() {
    core::arch::naked_asm!("ret");
}
```

When to reach for it: essentially never in this codebase. We are a sprite editor on `egui`/`wgpu`, not a runtime or a bootloader; `unsafe` is forbidden workspace-wide. Listed for completeness — if you find yourself wanting it, you are almost certainly solving the wrong problem.

## boolean literals as cfg predicates (1.88)

`cfg(true)` and `cfg(false)` are accepted predicates — an always-on or always-off gate without inventing a feature flag.

```rust
// old: the obscure always-false idiom
#[cfg(any())]
fn scratch() {}

// new (1.88):
#[cfg(false)]
fn scratch() {}
```

When to reach for it: temporarily fencing off a module or experiment more legibly than `any()`. When not to: do not leave `#[cfg(false)]` dead code in a merged PR — delete it. Note 1.93 made using a keyword as a cfg predicate name an error, and 1.96 allows passing a macro `expr` metavariable through to `cfg`.

## impl Trait precise capturing in the wild

Beyond RPITIT above, the general `use<..>` capture bound is part of the edition 2024 bundle for free functions. 1.89 also relaxed `#![doc(test(attr(..)))]` placement and 1.89 widened temporary lifetime extension through tuple struct and tuple variant constructors, so a temporary wrapped in `Some(&make())` or `Wrapper(&make())` in a `let` lives as long as the binding:

```rust
// new (1.89): the inner temporary is now extended through the tuple constructor
let held = Wrapper(&compute_palette());
```

When to reach for it: you mostly benefit passively. Be aware of the matching 1.91/1.92 restriction — temporary scope for `pin!`, `format_args!`, `write!`, and `writeln!` arguments was tightened under edition 2024, so a temporary you used to rely on living past one of those macros may now drop earlier. If a `write!(buf, "{}", temp_ref)` starts failing the borrow checker after the bump, this is why.

## Macro, match, and pattern improvements

- C-style variadic function declarations widened: `sysv64`, `win64`, `efiapi`, `aapcs` ABIs in 1.91, and the `system` ABI in 1.93. FFI-only; we have little of it.
- `assert_matches!` and `debug_assert_matches!` stabilized in 1.96 — assert that an expression matches a pattern, with a diagnostic on failure. Reach for these in tests over a hand-rolled `match { _ => panic!() }`.
- 1.91 lowered pattern bindings in written order and bases drop order on primary bindings — observable only when a binding's `Drop` has side effects.
- 1.95 made pattern-matching semantics independent of the crate and module they appear in, and made matching a `#[non_exhaustive]` enum read the discriminant.
- 1.93 made the `#[test]` attribute an error where it has no meaning (trait methods, types, structs) instead of silently ignored.
- Several macro tightenings became hard errors or deny-by-default: `missing_fragment_specifier` (hard error, 1.89), `semicolon_in_expressions_from_macros` (deny, 1.91), `invalid_macro_export_arguments` (deny, 1.92).

When to reach for them: `assert_matches!` in tests after the 1.96 bump is the one with daily value. The rest are guardrails — they fail builds that were already relying on something fragile.

## Closure capture corrections

1.94 made closure capturing consistent and correct around patterns, which can change exactly what a closure captures (relevant under edition 2024's disjoint-closure-capture, where a closure borrows individual fields rather than the whole struct). 1.94 also stopped emitting some incorrect lifetime errors on closures, so code wrongly rejected before now compiles.

When to reach for it: you do not — these are silent improvements. The one thing to watch: if a closure that touches one field of our document struct starts borrowing differently after a toolchain bump, the 1.94 capture fix is the cause; the fix is usually to bind the field you want explicitly before the closure.

## Raw lifetimes and identifier handling

The corpus does not stabilize raw lifetimes (`'r#...`) anywhere in 1.85 through 1.96, so there is nothing to use. Adjacent in-window changes: 1.94 NFC-normalizes lifetime identifiers like `'a`, and edition 2024 reserved `gen` as a keyword. If you genuinely need an identifier that collides with a keyword, raw identifiers (`r#...`) predate this window and still apply; Cargo also gained `r#...` support in cfg names in 1.85.

## `#[target_feature]` on safe functions (1.86)

A safe `fn` can carry `#[target_feature(enable = "...")]`. It is callable without `unsafe` from a context that already statically guarantees the feature (another fn with the same `#[target_feature]`), and still requires `unsafe` from a context that does not.

```rust
// old: every SIMD helper was unsafe fn, so every caller wrote unsafe { }
#[target_feature(enable = "avx2")]
unsafe fn blend_row(dst: &mut [u8], src: &[u8]) { /* ... */ }

// new (1.86):
#[target_feature(enable = "avx2")]
fn blend_row(dst: &mut [u8], src: &[u8]) { /* ... */ }
// callable without unsafe from another #[target_feature(enable = "avx2")] fn
```

When to reach for it: a feature-gated inner loop in `crates/render` or a pixel-blending hot path, where you want the helper to compose with other gated helpers without `unsafe` noise at every call. The hard rule stands: calling it from a context that does not statically enable the feature still requires `unsafe`, because running an unsupported instruction is UB — and `unsafe` is forbidden here, so the realistic use is the all-safe-within-a-gated-island pattern, fronted by a runtime `is_x86_feature_detected!` dispatch at the boundary.

## Lints that became errors or denials — know these before they bite

These are not features to use; they are tightenings that can break a build on bump. The ones most likely to touch our code:

- `dangerous_implicit_autorefs`: warn in 1.88, deny in 1.89. Fires on implicit autoref of a place behind a raw pointer. Take an explicit `&raw const` / `&raw mut` instead.
- `missing_abi`: warn-by-default in 1.86 on `extern` blocks without an explicit ABI string. Write `extern "C"`.
- `double_negations` (1.86): catches `--x`, which is two negations and a no-op, not a decrement. Write `x -= 1`.
- `unpredictable_function_pointer_comparisons` (1.85, extended to external macros in 1.89): comparing `fn` pointers with `==` is unreliable. Use `core::ptr::fn_addr_eq` only when an address comparison is truly intended.
- `integer_to_ptr_transmutes` (1.91): transmuting an integer to a pointer loses provenance. Use `core::ptr::with_exposed_provenance`.
- `function_casts_as_integer` (1.93): `my_fn as usize` skips the fn-pointer step; write `my_fn as fn() as usize`.
- `const_item_interior_mutations` (1.93): mutating an interior-mutable `const` operates on a temporary copy — a long-standing footgun. Use a `static`.

When to reach for them: you read them when a clippy-clean build suddenly is not after a toolchain change. The repo runs `clippy --workspace --all-targets -- -D warnings` in the Stop gate, so a new warn-by-default lint stops the session — fix the code, do not silence the lint.

