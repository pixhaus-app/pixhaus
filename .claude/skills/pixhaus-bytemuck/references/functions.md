# bytemuck top-level functions (1.25.0)

The rule for the whole module: each plain function has a `try_` sibling that returns
`Result<_, PodCastError>`. **The plain function is the `try_` one with `.unwrap()` — it
panics on exactly the conditions the `try_` returns `Err` for.** So "when does `cast_slice`
panic?" is answered by "what does `try_cast_slice` return `Err` for?".

Pixhaus guidance: use the panicking forms for casts you control (your own structs, your own
buffers) — a mismatch there is a bug you want to fail loudly in dev. Use `try_*` only when
the bytes or sizes are external/uncertain (a loaded file, a length from elsewhere) and a
mismatch is recoverable; map the `PodCastError` into a `thiserror` variant per
`pixhaus-rust-conventions`. Never `unwrap()` a `try_*` to dodge that rule — call the
panicking form directly if a panic is what you mean.

## Value casts (copy)

```rust
pub fn cast<A: NoUninit, B: AnyBitPattern>(a: A) -> B
pub fn try_cast<A: NoUninit, B: AnyBitPattern>(a: A) -> Result<B, PodCastError>
```
Copies the bytes of `a` into a `B`. Only failure: `size_of::<A>() != size_of::<B>()` →
`SizeMismatch`. No alignment constraint (the value lands in a fresh `B`-aligned slot).

## Reference casts (in place, no copy)

```rust
pub fn cast_ref<A: NoUninit, B: AnyBitPattern>(a: &A) -> &B
pub fn cast_mut<A: NoUninit + AnyBitPattern, B: NoUninit + AnyBitPattern>(a: &mut A) -> &mut B
// + try_cast_ref / try_cast_mut -> Result<_, PodCastError>
```
Reinterpret a reference. Failure: different size → `SizeMismatch`; the reference isn't
aligned for `B` → `AlignmentMismatch`. `cast_mut` needs both types fully `Pod`-grade
(`NoUninit + AnyBitPattern`) because the target is writable.

## Slice casts (in place, length recomputed)

```rust
pub fn cast_slice<A: NoUninit, B: AnyBitPattern>(a: &[A]) -> &[B]
pub fn cast_slice_mut<A: NoUninit + AnyBitPattern, B: NoUninit + AnyBitPattern>(a: &mut [A]) -> &mut [B]
// + try_cast_slice / try_cast_slice_mut -> Result<_, PodCastError>
```
Same byte span, new length `a.len() * size_of::<A>() / size_of::<B>()`. Three failures:
1. `B` needs greater alignment and the slice isn't aligned → `AlignmentMismatch`.
2. The byte length isn't a whole number of `B` → `OutputSliceWouldHaveSlop`.
3. Casting between a ZST and a non-ZST → `SizeMismatch`.

This is the workhorse for GPU upload (`&[Vertex] -> &[u8]`) and pixel views
(`&[u8] -> &[Rgba8]`). The `u8 -> Rgba8` direction needs `bytes.len() % 4 == 0` and the
bytes aligned for `Rgba8` (4-aligned) — true for the start of any `Vec<u8>`.

## Single value ↔ bytes

```rust
pub fn bytes_of<T: NoUninit>(t: &T) -> &[u8]
pub fn bytes_of_mut<T: NoUninit + AnyBitPattern>(t: &mut T) -> &mut [u8]
pub fn from_bytes<T: AnyBitPattern>(s: &[u8]) -> &T
pub fn from_bytes_mut<T: NoUninit + AnyBitPattern>(s: &mut [u8]) -> &mut T
// + try_from_bytes / try_from_bytes_mut -> Result<_, PodCastError>
```
`bytes_of` views one value as bytes — the everyday way to feed a single uniform block to
`queue.write_buffer`. It doesn't fail for a normal sized `T` (a ZST yields an empty slice
whose pointer needn't match the input). `from_bytes` is the reverse: it needs the slice
**exactly** `size_of::<T>()` long (else `SizeMismatch`) and **aligned** for `T` (else
`AlignmentMismatch`). Read-only `from_bytes` needs only `T: AnyBitPattern`; the `_mut` forms
add `NoUninit`.

## Unaligned read (copy out, alignment-free)

```rust
pub fn pod_read_unaligned<T: AnyBitPattern>(bytes: &[u8]) -> T
pub fn try_pod_read_unaligned<T: AnyBitPattern>(bytes: &[u8]) -> Result<T, PodCastError>
```
Copies `bytes` into an owned `T` **without** an alignment requirement — only
`bytes.len() == size_of::<T>()` matters (else `SizeMismatch`). This is the right tool when
reading a value out of bytes that may sit at an arbitrary offset (a header field partway
through a buffer): `from_bytes` would panic on alignment, `pod_read_unaligned` won't.

## Alignment splitting (safe align_to)

```rust
pub fn pod_align_to<T: NoUninit, U: AnyBitPattern>(vals: &[T]) -> (&[T], &[U], &[T])
pub fn pod_align_to_mut<T: NoUninit + AnyBitPattern, U: NoUninit + AnyBitPattern>(vals: &mut [T]) -> (&mut [T], &mut [U], &mut [T])
```
Safe analogue of `slice::align_to`: returns `(unaligned_prefix, aligned_middle,
unaligned_suffix)`. Never panics or errors — the prefix/suffix absorb whatever doesn't
align. Niche; reach for it only when you must process the maximal aligned middle of an
oddly-aligned buffer in bulk.

## Zeroing

```rust
fn Zeroable::zeroed() -> Self            // always available — prefer this
pub fn write_zeroes<T: Zeroable>(target: &mut T)
pub fn fill_zeroes<T: Zeroable>(slice: &mut [T])
pub const fn zeroed<T: Zeroable>() -> T  // feature `const_zeroed` only
```
`T::zeroed()` is the always-available zero value. `write_zeroes`/`fill_zeroes` overwrite an
existing value/slice with zeros **including padding bytes** (a plain `*t = T::zeroed()` may
leave padding untouched) — relevant if you hash or byte-compare the result. The free
`bytemuck::zeroed()` fn is `const` but gated behind `const_zeroed`; in non-const code just
use the trait method. For an owned zeroed heap buffer, see `allocation` (`zeroed_vec`,
`zeroed_box`) in `references/alloc-and-checked.md`.

## `PodCastError` (the panic/error set)

```rust
pub enum PodCastError {
    TargetAlignmentGreaterAndInputNotAligned, // reinterpret needs more alignment than input has
    OutputSliceWouldHaveSlop,                 // byte length not a whole number of target elements
    SizeMismatch,                             // value/ref source and target sizes differ (incl. ZST<->non-ZST)
    AlignmentMismatch,                        // alignments must match exactly (mainly Box/Vec casts)
}
```
It implements `Display`/`Error`, so it slots straight into a `thiserror` `#[from]` or
`#[error(transparent)]` variant. The checked module wraps it — see
`references/alloc-and-checked.md` for `CheckedCastError`.

Note on exact variant-per-condition: `SizeMismatch`, `AlignmentMismatch`, and
`OutputSliceWouldHaveSlop` are confirmed from the `try_cast_slice`/`try_from_bytes` pages;
the per-condition variant for the `_ref`/`_mut`/unaligned siblings is the same consistent
mapping (those pages state the condition in prose and defer to the sibling). If a specific
mapping is load-bearing, confirm with `cargo doc` once vendored.
