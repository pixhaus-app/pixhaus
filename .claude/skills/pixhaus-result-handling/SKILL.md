---
name: pixhaus-result-handling
description: >
  Use when handling a `Result` or `Option` at the CALL SITE in Pixhaus — the
  question "what do I write instead of `unwrap()`/`expect()`?" Trigger this for
  ANY "replace this unwrap", "this expect won't pass clippy", "handle the None
  case", "bubble this error up", "give it a fallback / default value", "early
  return on error", "recover from a failed parse", "log the error but keep
  going", "is `todo!`/`unimplemented!`/`unreachable!`/`panic!` allowed here", or
  "my async error isn't Send" task, and whenever you see `.unwrap()`,
  `.expect(`, `let Ok(..) = .. else`, `if let Ok(..)`, `unwrap_or`,
  `unwrap_or_else`, `unwrap_or_default`, `ok_or`, `ok_or_else`, `map_err`,
  `inspect_err`, or a bare `?`. The workspace DENIES `unwrap`/`expect`/`panic!`
  via clippy and `disallowed-methods`, so this is the toolbox for getting past
  the gate cleanly — and the panic-macro rules here are repo-specific (`todo!`
  and `unimplemented!` are `warn` but the Stop gate's `-D warnings` blocks a
  clean session on them; `unreachable!` slips the lint but is still a runtime
  panic). This is the consuming side of a `Result`. For DEFINING the error type
  (the enum, `#[from]`, `transparent`) see `pixhaus-thiserror`; for the overall
  thiserror-vs-anyhow policy see `pixhaus-rust-conventions`.
---

# Handling Result and Option in Pixhaus

This is the call-site toolbox: you have a `Result<T, E>` or an `Option<T>` in
hand and need to get the `T` out — or propagate the failure — without reaching
for `unwrap()`. The workspace makes that a hard requirement, not a preference:

```toml
# workspace Cargo.toml [workspace.lints.clippy]
unwrap_used = "deny"
expect_used = "deny"
panic       = "deny"
```

```toml
# clippy.toml — even more specific, with the suggested replacement baked in
disallowed-methods = [
  { path = "std::option::Option::unwrap", reason = "use `?`, `unwrap_or`, or `ok_or` to surface errors" },
  { path = "std::result::Result::unwrap", reason = "use `?` or pattern-match the error" },
  { path = "std::option::Option::expect", reason = "use `ok_or_else` with a typed error" },
  { path = "std::result::Result::expect", reason = "use `?` or `map_err` with context" },
]
```

So `unwrap()` and `expect()` don't compile in non-test code — the question is
never "should I unwrap" but "which replacement fits this site." Test code is
exempt (the crate root sets `#![cfg_attr(test, allow(clippy::unwrap_used, ...))]`),
so inside `#[cfg(test)]` you `unwrap` freely. Everything below is about the
non-test paths.

This skill assumes the error *type* already exists. Defining it — the enum, the
`#[error("...")]` messages, `#[from]`, `transparent` — is `pixhaus-thiserror`.
Choosing thiserror (library) vs anyhow (the `shell` binary) is
`pixhaus-rust-conventions`.

## Pick the replacement by what the call site needs

| The call site needs… | Reach for | Notes |
|---|---|---|
| To propagate the failure to the caller | `?` | The default. Works on `Result` and `Option` (in a fn that returns one). |
| Early return, but the error value doesn't matter | `let Ok(x) = .. else { return .. }` | Flattens nested code; you choose the return value. |
| To recover inline and keep going | `if let Ok(x) = .. { } else { .. }` or `match` | Use when both arms do real work. |
| A constant fallback value | `unwrap_or(default)` | `default` is always evaluated — keep it cheap. |
| A computed/expensive fallback | `unwrap_or_else(\|e\| ..)` | Closure runs only on the error path; can see `e`. |
| The type's `Default` as fallback | `unwrap_or_default()` | Only when `Default` is genuinely the right empty value. |
| To turn `None` into a typed error | `ok_or(err)` / `ok_or_else(\|\| err)` | Bridges `Option` → `Result` so `?` can carry it. |
| To convert one error type into another | `map_err(\|e\| ..)` | The manual version of `#[from]`; use when no `From` impl fits. |
| To log/inspect the error but still propagate | `inspect_err(\|e\| ..)?` | Side effect only; returns the `Result` untouched. |

The rest of the skill is one section per row, with the why.

## `?` — the default for propagation

If the function returns a `Result` (or `Option`) and the failure should travel
to the caller, `?` is the answer. It's terse, it runs the `From` conversion for
you (so an upstream error becomes your crate's `Error` via `#[from]`), and it
keeps the happy path flat:

```rust
fn handle_request(req: &Request) -> Result<ValidatedRequest> {
    validate_headers(req)?;
    validate_body(req)?;
    let body = Body::try_from(req)?;   // TryFrom error converts via #[from]
    ValidatedRequest::try_from((req, body))   // last expr — no `?` + Ok(..) needed
}
```

Don't wrap the final fallible expression in `Ok(expr?)` — return it directly.
Reach for `.context()` (anyhow, in `shell`) only when the bare error would read
as vague; that judgment call lives in `pixhaus-rust-conventions`.

## `let-else` — early return without binding the error

When you want out of the function on failure and the error value is irrelevant
(you're substituting your own), `let Ok(x) = .. else` reads better than a
`match` and keeps the success binding in the outer scope:

```rust
// The parse failed; we don't care why — report our own typed error.
let Ok(frame) = parse_frame(&bytes) else {
    return Err(Error::CorruptFrame { offset });
};
// `frame` is in scope here, unindented.
use_frame(frame);
```

The `else` block must diverge — `return`, `break`, `continue`, or `panic`-family.
Use this over `if let Ok(..) { .. } else { return .. }` when there's nothing to do
in the success arm except continue with the value: it removes a level of nesting
for the entire rest of the function.

## `if let` / `match` — recover and keep going

When the error path does real work rather than bailing, branch explicitly:

```rust
let palette = match Palette::load(&path) {
    Ok(p) => p,
    Err(_) => Palette::default_16(),   // recover with a known-good fallback
};
```

If you only need the value-or-fallback shape and don't need the error, the
`unwrap_or*` family below says the same thing in one line. Use `match`/`if let`
when the arms are non-trivial or you want to log, branch, or rebuild state.

## `unwrap_or` / `unwrap_or_else` / `unwrap_or_default` — fallback values

These are the legitimate, lint-passing cousins of `unwrap` — they don't panic,
they substitute a value. Pick by cost and source of the fallback:

```rust
// constant fallback — `0` is trivial to construct, eager eval is fine
let count = parse_count(s).unwrap_or(0);

// expensive or error-dependent fallback — closure runs only on Err
let cfg = Config::load(&path).unwrap_or_else(|e| {
    tracing::warn!("config load failed ({e}); using defaults");
    Config::default()
});

// the type's Default is the right empty value
let layers = doc.layers().cloned().unwrap_or_default();   // empty Vec
```

`unwrap_or(expensive())` is the trap: the argument is evaluated even when the
value is `Ok`/`Some`, so anything non-trivial belongs in `unwrap_or_else`.

## `ok_or` / `ok_or_else` — turn `None` into a typed error

`Option` has no error to carry, so `?` alone can't give a caller a reason. Attach
one with `ok_or_else` (lazy) so the `None` case becomes a real `Error` variant:

```rust
fn active_layer(&self) -> Result<&Layer> {
    self.layers
        .get(self.active)
        .ok_or_else(|| Error::LayerIndexOutOfRange { index: self.active })
}
```

Prefer `ok_or_else(|| ..)` over `ok_or(..)` whenever the error is non-trivial to
build — same eager-vs-lazy rule as `unwrap_or`. This is exactly the replacement
`clippy.toml` points you to for `Option::expect`.

## `map_err` — convert when `#[from]` doesn't fit

`?` converts errors through `From`, which `#[from]` generates. When there's no
`From` impl — because two variants would wrap the same source type, or you want
to enrich the error — `map_err` does it by hand:

```rust
let decoded = zstd::decode_all(&bytes[..])
    .map_err(|e| Error::Decompress { stage: "project body", source: e })?;
```

If you find yourself writing the *same* `map_err` at every call site, that's the
signal to add a `#[from]` variant instead (see `pixhaus-thiserror`).

## `inspect_err` — log without consuming

To observe an error (log it, bump a metric) while still propagating it, use
`inspect_err` — it borrows the error, runs the side effect, and hands the
`Result` back unchanged, so it chains cleanly before `?`:

```rust
let frame = decode(&bytes)
    .inspect_err(|e| tracing::warn!("decode failed: {e}"))?;
```

This beats the old `map_err(|e| { log(&e); e })` dance and makes the intent —
"log, don't transform" — obvious. (`Option` has `inspect` / the `Err`-less
counterpart accordingly.)

## Panic macros: which are allowed, and what the gate does

`panic!`, `unwrap()`, and `expect()` are the same thing to the lints — denied.
But the three panic *macros* have different fates here, and the Stop gate
(`cargo clippy --workspace --all-targets -- -D warnings`) is what actually
decides whether your session goes green:

| Macro | clippy level | Survives the `-D warnings` gate? | Use it when |
|---|---|---|---|
| `panic!` | `deny` | No — hard error | Never in non-test code. Return an `Err` instead. |
| `todo!` | `warn` | No — `-D warnings` promotes it | Mid-edit scaffolding only. Must be gone before the session is clean. |
| `unimplemented!` | `warn` | No — same | A branch you've decided not to build yet, with a reason. Also blocks the gate. |
| `unreachable!` | not linted | Yes — compiles and passes | A branch you've *proven* impossible — and even then, prefer the type system. |

Don't be talked out of the `todo!`/`unimplemented!` row — it's the one people
get wrong. The tempting (false) reasoning is "I pinned `todo = "warn"` in
`[workspace.lints.clippy]`, and `-D warnings` only promotes *default* warnings,
so a lint I explicitly set to `warn` stays a warning and the gate stays green."
It does not. `-D warnings` denies the entire `warnings` group, which sweeps in
any lint currently at `warn` level — pinned or not. clippy says so to your face:

```
error: `todo` should not be present in production code
  = note: `-D clippy::todo` implied by `-D warnings`
error: could not compile `lint-test` due to 1 previous error
```

That's a real run on this toolchain (clippy 0.1.95) against a crate with
`todo = "warn"` and a `todo!()` in it — exit code 101, gate red. `unimplemented!`
behaves identically.

Two practical consequences:

- `todo!` and `unimplemented!` are fine while you're actively writing code, but
  they are not a finish line — the gate will reject the session until you've
  either implemented the branch or turned it into a real `Err`. Don't reach for
  them to "satisfy the compiler" and move on; they'll come back at the Stop hook.
- `unreachable!` is the one panic macro that slips through both clippy and the
  gate, which makes it the dangerous one: it's a live `panic!` wearing a
  permission slip. Use it only when a branch is genuinely impossible, and prefer
  to make it impossible *in the type system* so the macro isn't needed at all —
  newtypes, enums, and the type-state pattern (`pixhaus-type-state`) turn
  "this can't happen" from a runtime gamble into a compile error. If you do use
  it, leave a comment stating the invariant that makes the branch unreachable.

## Test error paths, not just happy paths

A `Result`-returning function isn't covered until a test drives it into `Err`.
Most Pixhaus errors don't derive `PartialEq` (they wrap sources that don't), so
assert on the rendered message instead of the value:

```rust
#[test]
fn divide_by_zero_is_reported() {
    let err = divide(10.0, 0.0).unwrap_err();   // unwrap_err is fine in tests
    assert_eq!(err.to_string(), "division by zero");
}
```

When an error *does* derive `PartialEq` (a simple data-only variant), assert on
the value directly. Either way the message is the contract a user sees, so a
test that pins it catches accidental wording or formatting drift. Test layout,
fixtures, and the `#[rstest]` case table for exercising several error inputs at
once live in `pixhaus-testing-conventions`.

## Async: errors crossing `.await` must be `Send + Sync + 'static`

An error returned from a future that gets `tokio::spawn`ed travels across
threads, so it has to satisfy `Send + Sync + 'static`. thiserror-derived enums
get there automatically *as long as every field does* — the trap is wrapping a
non-`Send` source (a `Rc`, a raw `dyn Error` without bounds, some FFI handle).
Keep error fields to owned, thread-safe data and this stays a non-issue.

This matters most for the AI backend trait, whose results return over channels
to the UI loop. The error type the channel carries must be `Send`:

```rust
// the error a spawned task sends back must be Send + Sync + 'static
async fn run(&self, inputs: VerbInputs) -> Result<VerbOutput, BackendError>;
```

If a `Send` bound won't hold, that's a design smell to surface, not to paper
over with `Box<dyn Error>` (forbidden in public APIs anyway) or a `?Send`
escape hatch. See `pixhaus-async-trait` for the dyn-dispatch side of that trait
and `pixhaus-tokio` for the spawn/channel mechanics.

## Quick checklist

- [ ] No `.unwrap()` / `.expect()` outside `#[cfg(test)]` — clippy denies them.
- [ ] `?` for propagation; `let-else` for error-agnostic early return.
- [ ] `unwrap_or_else` (not `unwrap_or`) when the fallback is expensive.
- [ ] `ok_or_else` to give a `None` a typed reason before `?`.
- [ ] `inspect_err` to log-and-propagate; don't transform with `map_err` just to log.
- [ ] No `todo!` / `unimplemented!` left at session end — the Stop gate fails on them.
- [ ] `unreachable!` only on a proven-impossible branch, with a comment; prefer encoding it in the type.
- [ ] Every `Result`-returning fn has a test that reaches its `Err` path.
- [ ] Errors crossing a spawned `.await` are `Send + Sync + 'static`.
