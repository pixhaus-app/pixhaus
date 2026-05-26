---
name: pixhaus-type-state
description: Use when modeling a thing that moves through states in Pixhaus Rust — a builder that must collect required fields before build(), a connection/request/session lifecycle (disconnected→connected, draft→submitted), a resource that's open vs closed or initialized vs uninitialized, or any API where calling a method in the wrong order should be a compile error rather than a runtime check. Trigger this for ANY "make illegal states unrepresentable", "enforce this order at compile time", "the builder should require X before build", "encode state in the type", "marker type", "PhantomData", "state machine in the type system", "this method should only exist after connect/open/init", or "stop callers from using an uninitialized X" task — even when the user doesn't say "type-state". Type-state encodes states as types so the compiler rejects illegal transitions. Reach for this skill to get the Pixhaus-correct form (state carries its data, so NO Option-plus-unreachable and NO unwrap/panic), and to judge when type-state earns its verbosity versus when a plain enum or runtime check is the right call.
---

# Type-state in Pixhaus

Encode each state of a thing as a distinct type, so the methods that are
legal in one state simply don't exist in the others. Calling `send` before
`connect`, or `build` before the required fields are set, becomes a compile
error instead of a runtime check or a panic.

The payoff is real but narrow: you trade verbosity for a guarantee the
compiler enforces for free. Spend it where an illegal call order is a likely
bug with a costly failure mode. Don't spend it on trivial state that a plain
enum models just as safely.

## Decide first: is type-state the right tool?

Reach for it when **all** of these hold:

- The states are known at compile time and form a small, fixed set.
- Some operations are only valid in some states, and calling them in the
  wrong state is a real bug, not a theoretical one.
- The transitions are driven by code you control, not by runtime input you
  can't predict.

Stay with a plain `enum` + `match` when:

- The current state is decided at runtime (parsed from a file, chosen by the
  user, read off the network). You can't pick a type for a value you don't
  know yet — see "Runtime-chosen state" below.
- The state set is open or churns often. Every new state multiplies the
  `impl` blocks.
- All operations are valid in all states. There's nothing to forbid, so
  there's nothing to gain.
- The thing is short-lived and local. The compile-time guarantee buys little
  when the whole lifecycle is five lines in one function.

Pixhaus prefers the simplest construct that's still correct. Type-state is a
sharp tool — `pixhaus-rust-conventions` already lists it under idioms, right
next to "don't reach for `Box<dyn Trait>` when generics fit". Same spirit:
use it when it removes a class of bugs, not for cleverness.

## The core pattern

States are zero-sized marker types. The thing is generic over its state. Each
`impl` block hangs the methods that are legal in that state off the matching
type. A `PhantomData<S>` field ties the type parameter to the struct without
storing anything — it compiles away to nothing.

```rust
use std::marker::PhantomData;

pub struct Disconnected;
pub struct Connected;

pub struct Client<State> {
    addr: String,
    _state: PhantomData<State>,
}

impl Client<Disconnected> {
    pub fn new(addr: impl Into<String>) -> Self {
        Client { addr: addr.into(), _state: PhantomData }
    }

    // Consumes the Disconnected client, hands back a Connected one.
    pub fn connect(self) -> Client<Connected> {
        Client { addr: self.addr, _state: PhantomData }
    }
}

impl Client<Connected> {
    // `send` exists only on Client<Connected>. There is no way to call it
    // on a Client<Disconnected> — it isn't a runtime error, it won't compile.
    pub fn send(&self, _msg: &str) { /* ... */ }
}
```

Transitions take `self` by value and return the next type. The old state is
consumed, so a stale handle in the wrong state can't linger.

## The Pixhaus rule: state carries its data — no Option, no unreachable

This is where the textbook version goes wrong for this repo. The common
write-up stores state-specific data in an `Option` on a single struct and
reaches for `unreachable!()` to unwrap it once the state "guarantees" it's
there:

```rust
// DO NOT do this in Pixhaus.
struct File<State> {
    handle: Option<std::fs::File>,   // None when closed, Some when open
    _state: PhantomData<State>,
}

impl File<Opened> {
    fn read(&mut self) -> io::Result<String> {
        let Some(handle) = self.handle.as_mut() else {
            unreachable!("state guarantees the file is open"); // clippy-denied
        };
        // ...
    }
}
```

`unwrap`, `expect`, `unreachable!`, and `panic!` are denied workspace-wide
outside tests (`pixhaus-rust-conventions`). And the `Option` is a tell: you
proved the invariant at the type level, then immediately threw the proof away
and re-checked it at runtime.

Put the data the state owns *inside the state type*. Then the field only
exists when the state does, the compiler enforces it, and there's nothing to
unwrap:

```rust
use std::path::PathBuf;

pub struct Closed;
pub struct Open {
    handle: std::fs::File,   // exists only in the Open state
}

pub struct File<S> {
    path: PathBuf,
    state: S,
}

impl File<Closed> {
    pub fn open(path: PathBuf) -> std::io::Result<File<Open>> {
        let handle = std::fs::File::open(&path)?;
        Ok(File { path, state: Open { handle } })
    }
}

impl File<Open> {
    // No Option, no unreachable!. `self.state.handle` is always valid here
    // because File<Open> can only be constructed with a real handle.
    pub fn read(&mut self) -> std::io::Result<String> {
        use std::io::Read;
        let mut s = String::new();
        self.state.handle.read_to_string(&mut s)?;
        Ok(s)
    }

    pub fn path(&self) -> &PathBuf {
        &self.path
    }
}
```

Use `PhantomData<S>` only for states that carry no data (markers like
`Connected` above). The moment a state owns a value, make it a real field on
the state struct. That's the difference between type-state that merely
*documents* the invariant and type-state that *enforces* it.

## Builder with compile-time required fields

The most common Pixhaus use: a builder where `build()` must be unreachable
until every required field is set, while optional fields stay optional. Track
each required field with its own type parameter.

```rust
use std::marker::PhantomData;

pub struct Set;
pub struct Unset;

#[derive(Debug)]
pub struct Brush {
    size: u32,
    color: [u8; 4],
    name: Option<String>,   // genuinely optional
}

pub struct BrushBuilder<HasSize, HasColor> {
    size: u32,
    color: [u8; 4],
    name: Option<String>,
    _size: PhantomData<HasSize>,
    _color: PhantomData<HasColor>,
}

impl BrushBuilder<Unset, Unset> {
    pub fn new() -> Self {
        BrushBuilder {
            size: 1,
            color: [0, 0, 0, 255],
            name: None,
            _size: PhantomData,
            _color: PhantomData,
        }
    }
}

// `size` flips only the HasSize parameter; the other is carried through
// unchanged, so the two setters compose in any order.
impl<HasColor> BrushBuilder<Unset, HasColor> {
    pub fn size(self, size: u32) -> BrushBuilder<Set, HasColor> {
        BrushBuilder {
            size,
            color: self.color,
            name: self.name,
            _size: PhantomData,
            _color: PhantomData,
        }
    }
}

impl<HasSize> BrushBuilder<HasSize, Unset> {
    pub fn color(self, color: [u8; 4]) -> BrushBuilder<HasSize, Set> {
        BrushBuilder {
            size: self.size,
            color,
            name: self.name,
            _size: PhantomData,
            _color: PhantomData,
        }
    }
}

// Optional fields take `&mut self`-style or `self` in any state — they don't
// move a type parameter. Available everywhere, required nowhere.
impl<HasSize, HasColor> BrushBuilder<HasSize, HasColor> {
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }
}

// build() exists only once both required fields are Set. No unwrap needed:
// the fields are plain values, not Options.
impl BrushBuilder<Set, Set> {
    pub fn build(self) -> Brush {
        Brush { size: self.size, color: self.color, name: self.name }
    }
}
```

Then:

```rust
let b = BrushBuilder::new().size(4).color([255, 0, 0, 255]).build();      // ok
let b = BrushBuilder::new().color([255, 0, 0, 255]).size(4).build();      // ok, any order
let b = BrushBuilder::new().size(4).name("ink").color([0; 4]).build();    // ok, optional set
let b = BrushBuilder::new().size(4).build();   // compile error: build() not on <Set, Unset>
```

Note the defaults in `new()` (`size: 1`, `color: black`): the required
fields hold harmless placeholders until set, so there's no `Option` to unwrap
in `build()`. This generic-over-the-other-parameter form is half the code of
writing out every `impl Builder<A, B>` combination, and it scales — three
required fields means three type parameters, not eight `impl` blocks.

The standard library uses this exact shape; `pixhaus-rust-conventions` shows
the `Request<Draft>` / `Request<Submitted>` lifecycle variant. Cross-link
those when reviewing.

## State machine: transitions consume self

For a lifecycle (an AI verb job, an export pipeline, a session), each
transition takes `self` and returns the next state. Consuming `self` is what
makes the old state unusable — there's no way to keep painting on a
`Session<Closed>` because the value that was `Session<Open>` is gone.

```rust
pub struct Draft;
pub struct Running { handle: tokio::task::JoinHandle<String> }
pub struct Done { output: String }

pub struct Job<S> {
    prompt: String,
    state: S,
}

impl Job<Draft> {
    // Draft carries no extra data, so it's a bare marker. Running owns the
    // join handle, so the handle lives on the Running state itself.
    pub fn submit(self, rt: &tokio::runtime::Handle) -> Job<Running> {
        let prompt = self.prompt.clone();
        let handle = rt.spawn(async move { run_inference(prompt).await });
        Job { prompt: self.prompt, state: Running { handle } }
    }
}
```

When a transition can fail, return `Result<Job<Next>, Error>` (or, to hand the
caller back a usable handle on failure, `Result<Job<Next>, Job<Prev>>`). Never
panic to signal a bad transition — the type already made the truly illegal
ones impossible, so the only failures left are real runtime errors that
deserve a typed `Error`.

## Runtime-chosen state: return an enum, never unsafe

A real trap, and one the textbook framing gets wrong: when the state is
decided at runtime, you cannot return `Thing<State>` for a `State` you don't
know at compile time. The fix is **not** `unsafe` (forbidden workspace-wide).
Wrap the possible typed states in an enum and `match` on it:

```rust
pub enum AnyClient {
    Disconnected(Client<Disconnected>),
    Connected(Client<Connected>),
}

// The caller can't know statically whether the saved session was live, so we
// hand back an enum and let them match. Each arm still holds a fully-typed
// client with its state guarantees intact.
pub fn restore(saved: &SavedSession) -> AnyClient {
    if saved.was_connected {
        AnyClient::Connected(Client::new(&saved.addr).connect())
    } else {
        AnyClient::Disconnected(Client::new(&saved.addr))
    }
}
```

If you find yourself wanting `unsafe` to "return different types from one
branch", stop — that's the signal you've hit a runtime-state boundary, and
the answer is an enum (or, for a uniform interface, `Box<dyn Trait>`), never a
transmute.

## Pitfalls

- **PhantomData variance.** A bare `PhantomData<S>` where `S` is only ever a
  marker is fine. If `S` ever appears in a function signature or you hit
  variance/drop-check errors, use `PhantomData<fn() -> S>` to get covariance
  without implying ownership of an `S`.
- **Type signatures leak into callers.** `BrushBuilder<Set, Unset>` shows up
  in error messages and any stored field. Keep the marker names short and
  self-explaining (`Set`/`Unset`, `Open`/`Closed`) so those signatures read.
- **Don't make states `pub` if callers shouldn't name them.** Export the
  generic type and the constructor, keep the marker types crate-private (or
  behind a sealed trait — `pixhaus-rust-conventions`) so downstream code can't
  forge a `Client<Connected>` it never connected.
- **Resist applying it everywhere once it clicks.** Two states with one
  forbidden method is the sweet spot. Five states with mostly-shared methods
  is a `match` wearing a costume.

## Quick checklist

- [ ] States are compile-time-known and few; transitions are code-driven, not input-driven
- [ ] State-specific data lives in the state type, not an `Option` + `unreachable!`
- [ ] No `unwrap` / `expect` / `panic!` / `unreachable!` anywhere (clippy-denied)
- [ ] Transitions consume `self` so stale-state handles can't linger
- [ ] Fallible transitions return `Result`, not a panic
- [ ] Runtime-chosen state returns an enum (or `dyn`), never `unsafe`
- [ ] Marker types are crate-private (or sealed) if callers shouldn't construct states directly
- [ ] You can name the bug class this prevents — if not, a plain enum is simpler
