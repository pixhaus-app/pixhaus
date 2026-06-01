# async-trait API reference

The exhaustive surface for `async-trait` 0.1.x. The SKILL.md covers the decisions
and the Pixhaus patterns; this file is the macro's full behavior, the desugaring,
every supported trait feature, and the edge cases. Read top-down or jump to a
section.

## Table of contents

1. The macro and its forms
2. How it desugars (the full expansion)
3. Send vs `?Send` — the generated bounds in detail
4. Supported trait features (the complete list)
5. Self receivers
6. Lifetimes and elision
7. Limitations and edge cases
8. Interactions: dyn, mockall, recursion, `Self: Sized`
9. Native `async fn` in traits — when it replaces async-trait

## 1. The macro and its forms

`async-trait` exports exactly one item: the attribute macro `async_trait`. There
are no types, no functions, no traits to import beyond the macro.

```rust
use async_trait::async_trait;
```

Two invocation forms:

| Form | Future bound | Use |
|---|---|---|
| `#[async_trait]` | `Pin<Box<dyn Future + Send>>` | default; futures are `Send`, can cross threads / be spawned |
| `#[async_trait(?Send)]` | `Pin<Box<dyn Future>>` | futures are not required `Send`; single-thread only |

The attribute goes on the **trait definition** and on **every `impl` block** of
that trait. It is not inherited from the trait to the impl — both sides carry it,
and both sides must use the same form (`?Send` on the trait requires `?Send` on
the impl).

## 2. How it desugars (the full expansion)

The macro rewrites each `async fn` into a plain `fn` returning a boxed, pinned,
dynamically-typed future. Given:

```rust
#[async_trait]
trait Advertisement {
    async fn run(&self);
}

#[async_trait]
impl Advertisement for AutoplayingVideo {
    async fn run(&self) {
        let stream = connect(&self.media_url).await;
        stream.play().await;
        Modal.run().await;
    }
}
```

the impl expands to roughly:

```rust
impl Advertisement for AutoplayingVideo {
    fn run<'async_trait>(
        &'async_trait self,
    ) -> Pin<Box<dyn core::future::Future<Output = ()> + Send + 'async_trait>>
    where
        Self: Sync + 'async_trait,
    {
        Box::pin(async move {
            /* original method body, with `self` captured by the async move block */
        })
    }
}
```

Key points of the expansion:

- A fresh lifetime `'async_trait` is introduced. The returned future borrows for
  that lifetime, and the receiver/borrowed args are tied to it. You never write
  `'async_trait`, but it appears in compiler errors.
- The return type is `Pin<Box<dyn Future<Output = T> + Send + 'async_trait>>`
  (the `+ Send` is absent under `?Send`).
- A `where Self: Sync + 'async_trait` (for `&self`) / `Self: Send + 'async_trait`
  (for `self`/`&mut self`, under default Send) clause is generated so the boxed
  future is actually `Send`.
- The body becomes `Box::pin(async move { ... })`. This is the per-call heap
  allocation. No `unsafe` is used anywhere in the expansion.

## 3. Send vs `?Send` — the generated bounds in detail

Under default `#[async_trait]`, for the boxed future to be `Send`, everything it
captures must be `Send`:

- **`Self` must be `Send`/`Sync`.** `&self` methods generate `Self: Sync`;
  `self`/`&mut self` methods generate `Self: Send`. That is why the trait is
  usually declared `trait Backend: Send + Sync`.
- **Every argument must be `Send`.** Arguments are moved into the `async move`
  block, so a non-`Send` argument makes the future non-`Send` → compile error
  ("future cannot be sent between threads safely").
- **A `&T` argument requires `T: Sync`,** because a shared reference is `Send`
  only when `T: Sync`. This can surprise you with an otherwise-unused reference
  argument; the bound is still required because the reference is captured.

Under `#[async_trait(?Send)]`, none of these `Send`/`Sync` bounds are generated;
the future is `Pin<Box<dyn Future + 'async_trait>>`. Use it only when the future
is confined to one thread (never `tokio::spawn`-ed onto a multi-thread runtime).

## 4. Supported trait features (the complete list)

The macro aims to support everything a normal trait can do. Confirmed supported:

- Self by value (`self`), by reference (`&self`), by mutable reference
  (`&mut self`), or no receiver (associated/static methods).
- Any number of arguments and any return type.
- Generic type parameters and lifetime parameters on the method.
- Generic type parameters and lifetimes on the trait itself.
- Associated types and associated consts.
- `where` clauses, including `where Self: Sized` (see §8).
- A mix of `async` and non-`async` methods in the same trait — non-async methods
  are passed through untouched (no boxing).
- Default method implementations in the trait.
- Elided lifetimes in `&` / `&mut` receivers and arguments (see §6).
- Methods that take or return references whose lifetimes the macro threads
  through `'async_trait`.

## 5. Self receivers

```rust
#[async_trait]
trait Example {
    async fn by_ref(&self);          // Self: Sync bound generated (default Send)
    async fn by_mut(&mut self);      // Self: Send bound generated
    async fn by_value(self);         // Self: Send bound generated
    async fn no_self();              // associated fn, no receiver bound
}
```

`by_value(self)` consumes the receiver — usable through `Box<dyn Example>` only
with the right setup; commonly the by-value form is reserved for `Self: Sized`
helpers. `&self` and `&mut self` are the trait-object-friendly forms.

## 6. Lifetimes and elision

async-fn syntax forbids lifetime elision anywhere except directly on a `&` /
`&mut`. async-trait inherits this. So:

```rust
// OK — reference receivers/args elide normally
async fn f(&self, other: &Frame);

// REJECTED — elided lifetime inside a type that has a lifetime parameter
async fn g(&self, c: Cow<str>);      // error: lifetime must be named or `'_`

// FIX — name it or use the placeholder
async fn g1<'c>(&self, c: Cow<'c, str>);
async fn g2(&self, c: Cow<'_, str>);
```

The rule: if a lifetime would be elided in a position that is not a bare
reference, write `'_` (or a named lifetime). The compiler error is explicit about
this.

## 7. Limitations and edge cases

- **It allocates per call.** Every invocation is a `Box::pin`. Fine at an I/O
  boundary, wrong on a hot loop. There is no way to avoid the allocation while
  keeping `dyn` compatibility — that's the trade async-trait makes.
- **Dynamic dispatch through a vtable.** Boxed-future methods are virtual; the
  compiler cannot inline across the call.
- **Both trait and impl must carry the attribute,** with matching `Send`/`?Send`.
  A missing or mismatched attribute yields a signature-mismatch error.
- **`impl Trait` in argument or return position** inside an async-trait method is
  not the way to reduce allocation — the whole method is boxed regardless.
- **Auto-trait leakage:** because the return type is erased to `dyn Future`, the
  caller only knows it's `Send` (or not) — not other auto traits. If a caller
  needs the future to be e.g. `'static`, ensure all captured data is `'static`.
- **Drop order of ignored arguments:** historically (pre-0.1.55) ignored
  arguments with `Drop` glue had subtle ordering issues; current versions capture
  arguments to match real `async fn` behavior. Not something you write around in
  current versions, but worth knowing if you see odd drop timing.
- **Edge cases are numerous;** the upstream intent is that all trait features
  work. If you hit an unexpected borrow-checker or type error specifically from
  the expansion, that's a known class — simplify the signature or file upstream.

## 8. Interactions

**dyn traits.** This is the reason the crate exists. After `#[async_trait]`, a
trait with async methods is object-safe:

```rust
#[async_trait]
trait Backend: Send + Sync {
    async fn run(&self, inputs: VerbInputs) -> Result<()>;
}

let backends: Vec<Box<dyn Backend>> = vec![/* ... */];
for b in &backends {
    b.run(inputs.clone()).await?;       // dynamic dispatch, boxed future
}
```

**`Self: Sized` methods stay un-boxed.** A method bounded `where Self: Sized` is
excluded from the vtable and left as a real `async fn`. Use this for constructors
or helpers you only call on a concrete type, not through `dyn`:

```rust
#[async_trait]
trait Backend {
    async fn run(&self) -> Result<()>;               // in the vtable, boxed
    async fn connect(url: &str) -> Result<Self>       // not in vtable
    where
        Self: Sized;
}
```

**Recursion is already handled.** An async-trait method calling itself does not
need a manual `Box::pin` around the recursive call — the method is already boxed.

**mockall.** Put `#[automock]` *above* `#[async_trait]` so mockall reads the
original async signatures:

```rust
#[automock]
#[async_trait]
trait Backend: Send + Sync {
    async fn run(&self, inputs: VerbInputs) -> Result<()>;
}
// MockBackend::expect_run().returning(|_| Box::pin(async { Ok(()) }));
```

## 9. Native `async fn` in traits — when it replaces async-trait

Rust 1.75 stabilized `async fn` in traits (AFIT). The workspace toolchain is well
past that, so the native form is the default and async-trait is the exception.

What native AFIT does and doesn't do:

- **Static dispatch: fully supported, zero-cost.** Generic bounds
  (`fn run(f: &impl Filter)`, `<F: Filter>`) and single concrete impls work with
  no allocation, no macro. Prefer this.
- **`dyn Trait`: not supported directly.** A trait with a native `async fn` is
  not dyn-compatible — you get `E0038`. This is the gap async-trait fills.
- **Send bound across a generic boundary:** native AFIT returns an opaque future
  whose `Send`-ness isn't named by default. Requiring it generically needs
  return-type-notation / explicit `Send` bounds, which is more friction than
  async-trait's "always boxed `+ Send`". For a trait-object registry that must be
  spawned, async-trait is simpler.

Decision: native AFIT for static dispatch; `#[async_trait]` only when you need a
trait object (the Pixhaus AI backend registry) — and there, the per-call
allocation is negligible against the network I/O it wraps.
