# Sprite-sheet export config builder

This is a clean fit for the type-state builder pattern: two required fields
(`format`, `output_path`) that must be set before `build()`, and one optional
field (`columns`, default 1). Forgetting a required field is a compile error,
not a runtime panic.

## How the gating works

`ExportConfigBuilder<HasFormat, HasPath>` carries two type parameters, each a
`Set` / `Unset` marker tracked with `PhantomData`. The setters are generic over
the *other* parameter:

- `format(..)` is on `impl<HasPath> ExportConfigBuilder<Unset, HasPath>` and
  returns `<Set, HasPath>` — it flips only `HasFormat` and carries `HasPath`
  through untouched.
- `output_path(..)` is on `impl<HasFormat> ExportConfigBuilder<HasFormat, Unset>`
  and returns `<HasFormat, Set>`.

Because each setter is generic over the parameter it doesn't touch, they compose
in any order, and we avoid writing out every `impl Builder<A, B>` combination.

`build()` is implemented **only** for `ExportConfigBuilder<Set, Set>`. So:

- `builder().format(..).build()` → `<Set, Unset>` → no `build` method.
- `builder().output_path(..).build()` → `<Unset, Set>` → no `build` method.
- `builder().build()` → `<Unset, Unset>` → no `build` method.

Each is a compile error (`E0599: no method named build`), verified below.

`columns(..)` lives on the fully-generic
`impl<HasFormat, HasPath> ExportConfigBuilder<HasFormat, HasPath>`: it moves no
type parameter, so it's available in every state and required in none, and it
defaults to 1 in `new()`.

## The Pixhaus-specific detail: no Option, no unwrap

The naive type-state builder stores required fields as `Option<T>` and unwraps
them in `build()` with `unwrap()`/`unreachable!()`, leaning on "the type proves
it's set." That violates the workspace no-unwrap / no-panic rule (clippy-denied)
and throws away the type-level proof to re-check it at runtime.

Instead the required fields hold harmless placeholders in `new()`
(`Format::Png`, an empty `PathBuf`) that the typed setters overwrite before
`<Set, Set>` is ever reached. `build()` reads plain values — there is no
`Option`, nothing to unwrap, no `unreachable!`, and no `unsafe`. The only
constructor for `ExportConfig` is the builder, so the invariant holds.

`columns` is a genuine default rather than a sentinel, so it's a plain `u32`
seeded to 1.

## Conventions applied

- No `unwrap`/`expect`/`unreachable!`/`panic!` outside tests.
- No `unsafe`.
- Edition 2024, toolchain 1.95.
- Marker types and the enum derive only what they need; no `Box<dyn Trait>`
  where generics fit.
- `output_path` takes `impl Into<PathBuf>` so callers can pass `&str`, `String`,
  or `PathBuf` — standard ergonomic builder input.
- `Default` is implemented for the empty builder (clippy's
  `new_without_default`), delegating to `new()`.

When this lands in the real `io` crate, the `Set` / `Unset` markers can stay
crate-private (the skill's "don't let callers forge a state" note) since nothing
downstream should name them; only `ExportConfig`, `ExportConfigBuilder`, and
`Format` need to be `pub`.

## Compile status

`rustc --edition 2024 --crate-type lib export_builder.rs` — **compiles clean,
exit 0, no warnings.**

The four `#[cfg(test)]` tests pass (`cargo`-free: `rustc --edition 2024 --test`),
covering both-fields-set, any-order, and the optional `columns` override.

The three illegal calls were checked from a separate crate and each fails to
compile for the right reason:

```
error[E0599]: no method named `build` found for struct `ExportConfigBuilder<Set, Unset>`
error[E0599]: no method named `build` found for struct `ExportConfigBuilder<Unset, Set>`
error[E0599]: no method named `build` found for struct `ExportConfigBuilder<Unset, Unset>`
```

So omitting `format` or `output_path` (or both) is a compile error, exactly as
required.
