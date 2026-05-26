# bytemuck: allocation, checked, must_cast, offset_of (1.25.0)

The four areas beyond the everyday derive-and-cast surface. Signatures here were verified
against the 1.25.0 source.

## `bytemuck::allocation` — feature `extern_crate_alloc`

Casts and zero-allocations for owned heap containers. Gated behind `extern_crate_alloc`
(enable it for the render crate). The container cast functions move ownership; on failure
the `try_` forms hand the original allocation back so you don't lose it.

### Zeroed allocation — the useful ones in Pixhaus

```rust
pub fn zeroed_box<T: Zeroable>() -> Box<T>
pub fn zeroed_slice_box<T: Zeroable>(length: usize) -> Box<[T]>
pub fn zeroed_vec<T: Zeroable>(length: usize) -> Vec<T>

pub fn try_zeroed_box<T: Zeroable>() -> Result<Box<T>, ()>
pub fn try_zeroed_slice_box<T: Zeroable>(length: usize) -> Result<Box<[T]>, ()>
pub fn try_zeroed_vec<T: Zeroable>(length: usize) -> Result<Vec<T>, ()>
```

`zeroed_vec::<Rgba8>(w * h)` is the clean way to make a fresh transparent canvas without
building it byte-by-byte. The `try_` forms fail only on allocation failure, with `Err(())`
(no `PodCastError` — there's no cast happening); the plain forms abort on alloc failure.

### Container casts (move, no reallocation)

```rust
pub fn cast_box<A: NoUninit, B: AnyBitPattern>(input: Box<A>) -> Box<B>
pub fn cast_vec<A: NoUninit, B: AnyBitPattern>(input: Vec<A>) -> Vec<B>
pub fn cast_slice_box<A: NoUninit, B: AnyBitPattern>(input: Box<[A]>) -> Box<[B]>

pub fn try_cast_box<A: NoUninit, B: AnyBitPattern>(input: Box<A>)
    -> Result<Box<B>, (PodCastError, Box<A>)>
pub fn try_cast_vec<A: NoUninit, B: AnyBitPattern>(input: Vec<A>)
    -> Result<Vec<B>, (PodCastError, Vec<A>)>
pub fn try_cast_slice_box<A: NoUninit, B: AnyBitPattern>(input: Box<[A]>)
    -> Result<Box<[B]>, (PodCastError, Box<[A]>)>
```

These reuse the existing allocation, so `A` and `B` must have **identical alignment** (else
`AlignmentMismatch`) and the byte length must divide evenly into `B`. A `Vec<u8>` pixel
buffer is 1-aligned, so `cast_vec::<u8, Rgba8>` will *fail* on alignment — to own typed
pixels, allocate as `Vec<Rgba8>` from the start (or `zeroed_vec`), and use the borrowing
`cast_slice` for byte views. The `Rc`/`Arc` variants (`cast_rc`, `cast_arc`, `cast_slice_rc`,
`cast_slice_arc`) bound both sides `NoUninit + AnyBitPattern` because a shared handle is
readable from either type.

### Collect

```rust
pub fn pod_collect_to_vec<A: NoUninit, B: NoUninit + AnyBitPattern>(src: &[A]) -> Vec<B>
```

Copies `src`'s bytes into a fresh `Vec<B>` (reading as many whole `B` as fit). Unlike
`cast_vec` it allocates and copies, so there's no alignment constraint on the source — the
escape hatch when you need an owned re-typed buffer from a misaligned or borrowed source.

## `bytemuck::checked` — runtime bit-pattern validation

Mirror of the top-level cast functions, but the target is `CheckedBitPattern` instead of
`AnyBitPattern`, so each call validates the bit pattern at runtime and refuses an invalid
one instead of producing UB. Same size/alignment rules as the top-level functions, plus the
validity check. Use this — not plain `cast` — whenever bytes become a `bool`, `char`, or a
`#[derive(CheckedBitPattern)]` enum.

```rust
// panicking
pub fn cast<A: NoUninit, B: CheckedBitPattern>(a: A) -> B
pub fn cast_ref<A: NoUninit, B: CheckedBitPattern>(a: &A) -> &B
pub fn cast_mut<A: NoUninit + AnyBitPattern, B: NoUninit + CheckedBitPattern>(a: &mut A) -> &mut B
pub fn cast_slice<A: NoUninit, B: CheckedBitPattern>(a: &[A]) -> &[B]
pub fn cast_slice_mut<A: NoUninit + AnyBitPattern, B: NoUninit + CheckedBitPattern>(a: &mut [A]) -> &mut [B]
pub fn from_bytes<T: CheckedBitPattern>(s: &[u8]) -> &T
pub fn from_bytes_mut<T: NoUninit + CheckedBitPattern>(s: &mut [u8]) -> &mut T
pub fn pod_read_unaligned<T: CheckedBitPattern>(bytes: &[u8]) -> T
// fallible siblings: try_cast / try_cast_ref / try_cast_mut / try_cast_slice /
// try_cast_slice_mut / try_from_bytes / try_from_bytes_mut / try_pod_read_unaligned
//   -> Result<_, CheckedCastError>
```

```rust
pub enum CheckedCastError {
    PodCastError(PodCastError),   // size/alignment failure, same as the plain casts
    InvalidBitPattern,            // the bytes didn't pass is_valid_bit_pattern
}
```

## `must_cast` — feature `must_cast`, compile-time, never panics

The cast's size/alignment compatibility is checked as a `const` assertion: if it doesn't
hold, the code **fails to compile** — no runtime panic, no `Result`. Off by default in
Pixhaus; enable only if you want a static guarantee on a fixed-layout cast.

```rust
pub const fn must_cast<A: NoUninit, B: AnyBitPattern>(a: A) -> B            // sizes must be equal
pub const fn must_cast_ref<A: NoUninit, B: AnyBitPattern>(a: &A) -> &B      // equal size, B align <= A align
pub const fn must_cast_mut<A: NoUninit + AnyBitPattern, B: NoUninit + AnyBitPattern>(a: &mut A) -> &mut B
pub const fn must_cast_slice<A: NoUninit, B: AnyBitPattern>(a: &[A]) -> &[B]
pub const fn must_cast_slice_mut<A: NoUninit + AnyBitPattern, B: NoUninit + AnyBitPattern>(a: &mut [A]) -> &mut [B]
```

The three cast families compared: top-level `cast_*` (`Pod`/`AnyBitPattern`, **panic** at
runtime on mismatch) → `checked::*` (`CheckedBitPattern`, **panic or `CheckedCastError`**
plus a bit-pattern check) → `must_*` (`NoUninit`/`AnyBitPattern`, **compile error** on
mismatch, no runtime check at all).

## `offset_of!` macro

```rust
offset_of!($instance:expr, $Type:path, $field:tt)   // 3-arg, explicit instance
offset_of!($Type:path, $field:tt)                   // 2-arg, uses Default::default()
```

Returns a field's byte offset as `usize`. **Prefer `core::mem::offset_of!`** (stable since
Rust 1.77) — it's std, needs no instance and no `Default`, and computing the offset of a
`#[repr(packed)]` field with bytemuck's macro requires `unsafe` (which `forbid` rejects).
Reach for bytemuck's only if you specifically need its 2-arg `Default`-based form.

## Feature flags recap

| Feature | Unlocks | Pixhaus |
|---|---|---|
| `derive` | the derive macros | **on** — the only legal way to mark types (see derives.md) |
| `extern_crate_alloc` | the `allocation` module | **on** for the render crate (`zeroed_vec`, container casts) |
| `must_cast` | `must_cast*` | off unless a static-assert cast is wanted |
| `const_zeroed` | `const fn zeroed` | off; use `Zeroable::zeroed()` |
| `zeroable_atomics`, `zeroable_maybe_uninit` | `Zeroable`/`AnyBitPattern` for atomics / `MaybeUninit` | off unless needed |
| `min_const_generics` | `[T; N]: Pod/Zeroable` for all `N` | on by default in modern bytemuck |
| `wasm_simd`, `aarch64_simd` | SIMD-type impls | off (desktop targets; not relevant) |
