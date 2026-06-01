# bytemuck traits (1.25.0)

Every trait here is an `unsafe trait`. In Pixhaus you never `unsafe impl` them by hand —
`forbid(unsafe_code)` rejects that — so you reach a trait either through a `#[derive]`
(`references/derives.md`) or through a blanket impl bytemuck already provides. This file is
the contract each trait encodes, so you know *why* a derive demands what it demands and
which trait a given API needs.

## The hierarchy

```
Zeroable           all-zero bytes is a valid value
   ▲
   │ supertrait
AnyBitPattern      every bit pattern is a valid value        — READ side  (bytes -> &T)
   ▲
   │
  Pod              AnyBitPattern + NoUninit + Copy + 'static  — both sides (incl. &mut)
   ▲   ▲
   │   │ blanket: impl<T: Pod> NoUninit / AnyBitPattern for T
NoUninit           no padding / uninit bytes                  — WRITE side (&T -> bytes)
```

- `T: Pod` ⟹ `T: NoUninit`, `T: AnyBitPattern`, `T: Zeroable` (all via blanket impls).
- The reverse never holds. A type with padding can be `Zeroable` but not `Pod`; a `bool`
  is `NoUninit` and `Zeroable` but not `AnyBitPattern` (so not `Pod`).
- `CheckedBitPattern` is the runtime-checked stand-in for `AnyBitPattern` when only *some*
  bit patterns are valid.

## `Zeroable`

```rust
pub unsafe trait Zeroable: Sized {
    fn zeroed() -> Self { /* core::mem::zeroed() */ }
}
```

Contract: the type is inhabited, and an all-zero byte pattern is a valid value. The
all-zero value may differ from `Default`. Use `T::zeroed()` for an always-available
zero value (the free fn `bytemuck::zeroed` is the same thing but gated behind the
`const_zeroed` feature — prefer the trait method).

std implementors include: all integers and floats, `bool`, `char`, `()`, raw pointers,
`Option<T: ZeroableInOption>`, `PhantomData`, `ManuallyDrop`, `MaybeUninit`, `Wrapping`,
`Cell`/`UnsafeCell`, atomics (feature `zeroable_atomics`), tuples up to 8, and `[T; N]`
where `T: Zeroable`. Derivable for structs (all fields `Zeroable`), enums with an explicit
int/C repr and a zero-discriminant variant, and unions.

## `Pod`

```rust
pub unsafe trait Pod: Zeroable + Copy + 'static {}
```

Contract — all of: inhabited; **any** bit pattern is valid (so no `bool`, `char`, enums,
`NonZero*`); **no padding/uninit bytes** anywhere; every field is `Pod`; `#[repr(C)]` or
`#[repr(transparent)]`; **no interior mutability and no pointers** (no `Cell`, `UnsafeCell`,
atomics, `*const`/`*mut`, references). This is the trait your wgpu vertex/uniform/instance
structs want.

std implementors: integers, floats, `()`, `Wrapping`, `ManuallyDrop`, `PhantomData`,
`PhantomPinned`, `[T; N]` where `T: Pod`. Derivable for structs only (see derives).

## `NoUninit` — write side

```rust
pub unsafe trait NoUninit: Sized + Copy + 'static {}
```

Contract: inhabited, no padding bytes (interior or trailing), every field `NoUninit`, a
C/transparent (struct) or int/C (enum) repr, no interior mutability. Lets you cast an
**immutable** `&T`/`&[T]` *out* to bytes, but not a `&mut` *in* (that needs the read side
too). Blanket: `impl<T: Pod> NoUninit for T`. Also implemented for `bool`, `char`, and all
`NonZero*` — types that have no padding but do have illegal bit patterns, so they're
write-safe but not read-safe. Derivable for structs and enums.

## `AnyBitPattern` — read side

```rust
pub unsafe trait AnyBitPattern: Zeroable + Sized + Copy + 'static {}
```

Contract: inhabited, every field `AnyBitPattern`, no interior mutability. Lets you build
a `&T` (or owned `T`) *from* arbitrary bytes, but **not** cast a `&mut T` in place (that's
`Pod`). Blanket: `impl<T: Pod> AnyBitPattern for T`; also `MaybeUninit<T: AnyBitPattern>`
(feature `zeroable_maybe_uninit`). Derivable for structs (the derive documents only the
"every field is `AnyBitPattern`" requirement; in practice pair with `#[repr(C)]`).

## `CheckedBitPattern` (module `bytemuck::checked`, re-exported at root)

```rust
pub unsafe trait CheckedBitPattern: Copy {
    type Bits: AnyBitPattern;
    fn is_valid_bit_pattern(bits: &Self::Bits) -> bool;
}
```

For types where only some bit patterns are valid. `Bits` is the raw `AnyBitPattern`
companion (same layout); the `checked::*` cast functions read the bytes as `Bits`, call
`is_valid_bit_pattern`, and only then hand back `&Self` — otherwise they fail with
`CheckedCastError::InvalidBitPattern` instead of producing UB. Blanket: every
`T: AnyBitPattern` is `CheckedBitPattern` with `Bits = Self` and an always-true check.
Explicit impls for `bool`, `char`, all `NonZero*`. Derivable for structs (C/transparent
repr) and **fieldless or field-bearing enums with an explicit `#[repr(Int)]`** — this is
how you safely round-trip an enum through bytes. See `references/alloc-and-checked.md`.

## `Contiguous`

```rust
pub unsafe trait Contiguous: Copy + 'static {
    type Int: Copy + Ord;
    const MAX_VALUE: Self::Int;
    const MIN_VALUE: Self::Int;
    fn from_integer(value: Self::Int) -> Option<Self> { /* provided — do not override */ }
    fn into_integer(self) -> Self::Int          { /* provided — do not override */ }
}
```

Marks a type (typically a fieldless enum) whose valid values are a contiguous integer
range `[MIN_VALUE, MAX_VALUE]`. `from_integer` becomes a safe, checked
integer-to-enum conversion — useful for turning a `u8` tool-id or blend-mode read from a
file into the enum without a hand-written match. Contract: same non-zero size as `Int`,
`Int` is a primitive integer, every value in range is a unique valid instance, none falls
outside. Derivable for fieldless `#[repr(Int)]` enums with contiguous discriminants.

## `TransparentWrapper<Inner: ?Sized>`

```rust
pub unsafe trait TransparentWrapper<Inner: ?Sized> {
    fn wrap_ref(s: &Inner) -> &Self;      fn peel_ref(s: &Self) -> &Inner;
    fn wrap_mut(s: &mut Inner) -> &mut Self;  fn peel_mut(s: &mut Self) -> &mut Inner;
    fn wrap(s: Inner) -> Self where Self: Sized, Inner: Sized;
    fn peel(s: Self) -> Inner where Self: Sized, Inner: Sized;
    // + wrap_slice / peel_slice (and _mut)
}
```

For a `#[repr(transparent)]` newtype: convert between `&Inner` and `&Newtype` for free.
Contract: identical representation to `Inner`, any extra fields are zero-sized `Zeroable`
ZSTs, no added alignment, no method overrides. Derivable; for a single-field struct the
inner type is inferred, otherwise annotate `#[transparent(Inner)]`. Niche in Pixhaus — most
newtypes can just derive `Pod` directly.

## `ZeroableInOption` / `PodInOption`

```rust
pub unsafe trait ZeroableInOption: Sized {}
pub unsafe trait PodInOption: ZeroableInOption + Copy + 'static {}
```

These are what make `Option<T>` itself `Zeroable`/`Pod` for niche-optimized `T`: the `None`
case is the all-zero pattern. **Not derivable** — implemented by bytemuck for the types
that have the niche: `ZeroableInOption` for `NonNull`, `&T`, `&mut T`, `Box<T>` (feature
`extern_crate_alloc`), `NonZero*`, and fn pointers; `PodInOption` for the `NonZero*` family
only (the pointer-bearing ones are not `Pod`). You rarely name these directly; you benefit
from them when you put `Option<NonZeroU32>` in a struct.
