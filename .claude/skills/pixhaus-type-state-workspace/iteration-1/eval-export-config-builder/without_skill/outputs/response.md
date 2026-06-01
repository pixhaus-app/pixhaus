# Sprite-sheet export config builder

The requirement — "forgetting `format` or `output_path` should be a compile
error, not a runtime panic" — is exactly the type-state builder pattern. The
trick: encode "is this field set yet?" in the builder's type parameters, and
implement `build()` only on the variant where both required fields are `Set`.
Omit one, and the method literally doesn't exist on that type, so the compiler
rejects the call.

## How it works

```rust
pub struct ExportConfigBuilder<FormatState, PathState> { ... }
```

Two zero-sized marker types, `Missing` and `Set`, ride along as type
parameters. `new()` returns `ExportConfigBuilder<Missing, Missing>`. Each setter
consumes `self` and returns a builder with that parameter flipped to `Set`:

- `.format(..)` -> `ExportConfigBuilder<Set, PathState>`
- `.output_path(..)` -> `ExportConfigBuilder<FormatState, Set>`

Because the setters are generic over the *other* parameter, you can call them in
either order and the states accumulate. `build()` is gated:

```rust
impl ExportConfigBuilder<Set, Set> {
    pub fn build(self) -> ExportConfig { ... }
}
```

It exists only when both parameters are `Set`. Try to build with a field
missing and you get `E0599: no method named build found for
ExportConfigBuilder<Missing, Set>` — a compile error, verified below.

`columns` is the easy case: it's not required, so it doesn't touch the
type-state. It's stored with a default of `1` in `new()` and a plain
`columns(self, u32) -> Self` setter overrides it.

## Repo-convention notes

- **No panics.** `unwrap()`/`expect()`/`panic!()` are clippy-denied outside
  tests. The internal storage still uses `Option<ExportFormat>` and
  `Option<PathBuf>` (the markers are separate zero-sized types, not the data
  itself), so `build()` has to get the values out. Rather than `unwrap()` —
  which would be safe-by-construction but still trips the lint and leaves a
  panic path in the binary — `build()` destructures with `match` and maps the
  by-construction-impossible `None` arms to harmless defaults. No panic branch
  exists, so the lint stays honest and there's nothing to blow up at runtime
  even if someone later subverts the type-state with unsafe transmutes (which
  the workspace forbids anyway).
- **No unsafe.** The pattern is entirely safe; `PhantomData<(FormatState,
  PathState)>` carries the type parameters without storing a value.
- **`impl Into<PathBuf>`** on `output_path` so callers can pass `&str`, `String`,
  or `PathBuf` without ceremony — matches the ergonomic, iterator-friendly house
  style.
- **`#[must_use]`** on the builder constructors/setters and on `build()`: a
  builder you don't consume is a bug worth a warning.
- **Edition 2024 / 1.95.** Nothing edition-specific is leaned on; it compiles
  clean under `--edition 2024`.

## Alternative considered

A runtime-checked builder (`build(self) -> Result<ExportConfig, BuildError>`
with `thiserror`) is the other idiomatic option and is the right call when the
required set is large or configured dynamically (e.g. from deserialized data),
because N type parameters get unwieldy fast. Here there are exactly two required
fields set through a fluent chain, the failure is a programmer mistake rather
than bad input, and the task explicitly asked for a *compile* error — so
type-state is the better fit. If this builder later grows many required fields,
revisit and switch to the `Result` form.

## Compile + test status

- `rustc --edition 2024 --crate-type lib export_builder.rs` — exit 0, no
  warnings.
- `rustc --edition 2024 --test export_builder.rs` then run — 4 tests pass.
- Negative check: appending a `build()` call on a builder that skipped
  `.format(..)` fails with `error[E0599]: no method named build found for
  struct ExportConfigBuilder<Missing, Set>`. The gate works.
