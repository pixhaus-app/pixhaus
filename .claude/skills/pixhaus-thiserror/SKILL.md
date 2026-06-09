---
name: pixhaus-thiserror
description: >
  Use when defining or reviewing error types in Pixhaus library crates (`core`,
  `io`, `render`, and any future logic crate) with the `thiserror` derive macro.
  Trigger this for ANY "add an error variant", "make a crate Error enum", "derive
  Error", "convert this upstream error with `?`", "wrap an io::Error / png /
  rmp_serde error", "forward Display to an inner error", "give this error a source
  chain", "why won't `?` convert my error", or "transparent error" task, and
  whenever you see `#[derive(Error)]`, `#[error("...")]`, `#[from]`, `#[source]`,
  `#[error(transparent)]`, `#[backtrace]`, or `use thiserror::Error`. thiserror's
  `#[error("{field}")]` interpolates a STRUCT FIELD, not a local variable like
  `format!` does, and `#[from]` has strict one-field rules that the compiler
  errors are vague about — reach for this skill to get the attributes right
  rather than guessing. The binary (`shell`) uses `anyhow`, not thiserror; see
  `pixhaus-rust-conventions` for that boundary.
---

# thiserror for Pixhaus

thiserror is how every Pixhaus library crate defines its error type. It's a derive
macro for `std::error::Error` that writes the `Display`, `Error`, and `From` impls
for you from attributes on the enum — so a typed error is a few lines of data plus
annotations, not a page of hand-written trait impls.

The split is fixed and not negotiable (`pixhaus-rust-conventions`): **library
crates use thiserror, the `shell` binary uses `anyhow`.** Library code keeps rich
typed errors so callers can match on them; the binary collapses them into a
`.context()` chain for display. Never pull `anyhow` into `core`/`io`/`render`, and
never expose `Box<dyn Error>` from a public API — define a real enum.

## Version and license

| Crate | Version | License | cargo deny |
|---|---|---|---|
| `thiserror` | 2.0 | `MIT OR Apache-2.0` | passes the MIT lock |

```toml
# in each library crate's Cargo.toml
thiserror = "2"
```

thiserror is a `proc-macro` dependency only — nothing from it ends up in your
public API. Swapping a hand-written `Error` impl for thiserror (or back) is not a
breaking change, which is why it's safe to standardize on.

## The Pixhaus pattern: one Error + Result per crate

Each library crate exports its own `Error` and `Result` from `lib.rs`:

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("invalid pixel format: {0}")]
    InvalidPixelFormat(String),

    #[error("buffer size mismatch: expected {expected}, got {actual}")]
    SizeMismatch { expected: usize, actual: usize },

    #[error("I/O error")]
    Io(#[from] std::io::Error),

    #[error("PNG decode error")]
    PngDecode(#[from] png::DecodingError),
}

pub type Result<T> = std::result::Result<T, Error>;
```

`#[derive(Error)]` requires `Debug` too — derive both. The `#[from]` variants let
`?` convert an upstream error automatically, so `std::fs::read(path)?` inside a
function returning `crate::Result<T>` just works.

## Attribute reference

### `#[error("...")]` — the Display message (required on every variant)

Each variant (or the whole struct) needs an `#[error("...")]`, which generates the
`Display` impl. The string interpolates the variant's own fields:

```rust
#[error("the data for key `{0}` is not available")]   // tuple field .0
Redaction(String),

#[error("invalid header (expected {expected:?}, found {found:?})")] // named fields, Debug fmt
InvalidHeader { expected: String, found: String },
```

Interpolation forms:

| In the string | Expands to | Use for |
|---|---|---|
| `{0}`, `{1}` | `self.0`, `self.1` | tuple-struct/variant fields |
| `{name}` | `self.name` | named fields |
| `{0:?}`, `{name:?}` | `Debug` of that field | when `Display` isn't enough |
| `{}` + trailing args | a positional arg you supply | expressions, see below |

For anything beyond a bare field — a method call, indexing, arithmetic — use a
positional `{}` and pass the expression as a trailing argument, referencing fields
with `.0` / `.field`:

```rust
#[error("chunk too large: {} bytes (max {})", .0.len(), MAX_CHUNK)]
ChunkTooLarge(Vec<u8>),
```

**The gotcha that bites everyone:** in thiserror, `#[error("{width}")]` reads the
**field** `self.width`. In a normal `format!("{width}")` it captures a **local
variable** `width`. They look identical and behave differently. If you write
`#[error("{count}")]` and there's no field `count`, it does not fall through to a
local — it fails to compile. Name the field, or use the trailing-arg form.

Every field you interpolate must impl the formatting trait you ask for: `Display`
for `{x}`, `Debug` for `{x:?}`. A `Vec<u8>` has no `Display` — interpolate it with
`{:?}` or a method.

### `#[from]` — auto-generate `From` (and the source link)

`#[from]` on a field generates `impl From<ThatType> for Error`, which is what makes
`?` convert upstream errors. It also implicitly marks the field as the error
`source` — you do **not** add `#[source]` alongside it.

```rust
#[error("I/O error")]
Io(#[from] std::io::Error),
```

Strict rule the compiler is vague about: **a `#[from]` variant may contain only the
source field** (plus an optional backtrace field — see below). The moment you add a
second meaningful field, `From` can't be generated and you'll get a confusing
error. If a variant needs extra context (a path, an index), drop `#[from]`, build
the variant explicitly, and use `.map_err(...)` at the call site:

```rust
// Can't use #[from] here — there's a second field. Build it by hand.
#[error("failed to read frame {index}")]
FrameRead { index: u32, #[source] source: std::io::Error },

// at the call site:
std::fs::read(path).map_err(|source| Error::FrameRead { index, source })?;
```

Also note: two `#[from]` variants for the **same** source type collide — you can
only have one `From<io::Error>`. If two situations both stem from `io::Error` and
need to be distinguished, only one can use `#[from]`; build the other by hand.

### `#[source]` — the error chain without `From`

`#[source]` marks the field returned by `Error::source()`, building the chain that
`anyhow` walks in the binary to print "failed to export: failed to encode: invalid
format". Use it when you want the chain but not an auto `From` (e.g. the
`FrameRead` example above). A field literally named `source` is picked up as the
source automatically, so `source: io::Error` needs no attribute at all.

Keep the wrapper's own `#[error("...")]` message about *its* layer and let the
source carry the lower-level detail — don't paste the source's message into the
parent string, or it prints twice in the chain.

### `#[error(transparent)]` — forward Display and source to the inner error

`transparent` forwards both `Display` and `source()` straight to the single wrapped
error and adds no message of its own. Use it for a catch-all variant that should
read exactly like whatever it wraps:

```rust
#[derive(Debug, Error)]
pub enum Error {
    #[error("PNG decode error")]
    PngDecode(#[from] png::DecodingError),

    // Anything else, surfaced verbatim with no extra wording.
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}
```

A `transparent` variant must have exactly one field and no message string. Reach
for it sparingly in library crates — a too-broad `Other(anyhow::Error)` throws away
the typed-error benefit. It's most useful when re-exporting a sub-module's error
unchanged.

### `#[backtrace]` and `Backtrace` — nightly only, skip it on stable

thiserror can wire up `Error::provide` to hand out a `std::backtrace::Backtrace`,
and `#[from]` with a `Backtrace` field captures one automatically:

```rust
Io {
    #[from]
    source: io::Error,
    backtrace: Backtrace,
},
```

**But the `provide` mechanism (`error_generic_member_access`) is still nightly.**
Pixhaus builds on stable (toolchain 1.96, see `pixhaus-rust-conventions`), so
`#[backtrace]` and the auto-`provide` do nothing useful here — don't add backtrace
fields expecting them to surface. If you need a stack trace for debugging, that's a
tracing/logging concern, not the error type.

### `#[error(fmt = ...)]` — formatting logic too complex for a string (2.0)

thiserror 2.0 lets a variant delegate its `Display` to a function instead of an
inline string, for when the message needs real branching. Reach for the docs for
the exact signature if you hit a case that warrants it — most Pixhaus errors are a
plain string and don't.

## Structs, not just enums

A crate's top-level error is almost always an enum, but thiserror also derives on
structs (named, tuple, or unit) — handy for a single focused failure:

```rust
#[derive(Debug, Error)]
#[error("palette index {index} out of range (len {len})")]
pub struct PaletteIndexError {
    pub index: usize,
    pub len: usize,
}
```

## How the chain surfaces to the user

Library errors travel up typed, then the `shell` binary turns them into a context
chain with `anyhow`. You don't print errors in library code — you return them.

```rust
// in shell (binary), anyhow walks the source chain thiserror built:
let frame = pixhaus_io::read_frame(path)
    .context("failed to open the project's first frame")?;
// prints: failed to open the project's first frame: I/O error: <os error>
```

This is exactly why `#[from]`/`#[source]` matter: they're what populate that chain.
A variant with no source and a hand-typed message that swallows the cause breaks
the chain — preserve the source.

## Common mistakes

- **`#[from]` on a multi-field variant.** Only the source (plus optional backtrace)
  is allowed. Need more context? Drop `#[from]`, use `#[source]` + `.map_err`.
- **Two `#[from]` for the same source type.** Only one `From<T>` can exist. Build
  the second variant by hand.
- **Expecting `{x}` to read a local variable.** It reads the field `self.x`. Use
  trailing args for expressions.
- **Interpolating a non-`Display` field with `{x}`.** `Vec<u8>`, most foreign
  structs — use `{x:?}` or a method via trailing args.
- **Reaching for `anyhow` in a library crate.** Libraries define typed errors;
  `anyhow` is binary-only.
- **`unwrap()` on a `Result` outside tests.** Clippy denies it workspace-wide
  (`pixhaus-rust-conventions`). Propagate with `?` into the crate's `Error`.
- **Restating the source's message in the parent.** Causes double-printing in the
  chain. Describe only your layer.

## Decision shortcut

```
Defining an error in a Pixhaus library crate (core / io / render / logic)?
├─ Wrapping ONE upstream error, want `?` to just convert it?
│    └─ #[error("...")] Variant(#[from] UpstreamError)
├─ Want the source chain but NOT an auto From (extra context fields)?
│    └─ #[error("...{ctx}")] Variant { ctx, #[source] source: UpstreamError }
│       + .map_err(...) at the call site
├─ Catch-all that should read exactly like the inner error, no new wording?
│    └─ #[error(transparent)] Variant(#[from] InnerError)   // use sparingly
├─ A self-contained failure with its own data, no upstream cause?
│    └─ #[error("...{field}")] Variant { field: T }   // or a tuple variant
└─ In the shell binary, not a library crate?  → anyhow, not thiserror.
```
