# bytemuck derive macros (1.25.0, `derive` feature)

In Pixhaus these are the **only** way to mark a type castable — `unsafe_code = "forbid"`
rejects a hand-written `unsafe impl` (verified against the workspace lint config; the error
is "implementation of an `unsafe` trait ... requested ... with `-F unsafe-code`"). The
derives are sound here because the generated `unsafe impl` lives in the `bytemuck_derive`
proc-macro crate, so its `unsafe` token doesn't fall under your crate's `forbid`. Beyond
legality, each derive emits compile-time assertions that the type genuinely satisfies the
trait — a derive that compiles is a proof, not a promise.

A derive that *fails* always names a real layout problem (wrong repr, a non-conforming
field, padding, or a generic you can't allow). The fix is to the type, never to reach for
unsafe. The common fixes are in the SKILL.md "recurring bugs" list.

`#[derive(...)]` always needs the trait's own prerequisites in scope too: `Pod` requires
`Copy` (so derive `Copy, Clone` alongside it), and most uses want `#[repr(C)]`.

## `Zeroable`

Works on structs, enums, unions.
- Struct: every field must be `Zeroable`. No repr requirement.
- Enum: needs an explicit `#[repr(Int)]`, `#[repr(C)]`, or `#[repr(C, Int)]`, **and** a
  variant whose discriminant is 0; the fields of that zero variant (if any) must be
  `Zeroable`.
- Union: always succeeds.
- Helper `#[zeroable(bound = "...")]` opts into per-field ("perfect derive") bounds for
  generics instead of bounding every type parameter.

```rust
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Zeroable)]
struct Color { r: u8, g: u8, b: u8, a: u8 }
```

## `Pod`

**Structs only.** Requirements:
- Every field is `Pod`.
- `#[repr(C)]` or `#[repr(transparent)]`.
- **No padding bytes** (the most common failure — see below).
- No generic parameters **unless** `#[repr(transparent)]`; a transparent generic is `Pod`
  only when its parameter is `Pod`.

```rust
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex { pos: [f32; 2], uv: [f32; 2] }   // 16 bytes, no padding — ok

#[repr(transparent)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Texel<T: bytemuck::Pod>(T);              // generics allowed only when transparent
```

Fails to compile: a `#[repr(C)]` generic struct; a `#[repr(transparent)]` generic whose
param isn't `Pod`; **any struct with implicit padding**. Example that fails —

```rust
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Bad { flag: u8, value: u32 }   // 3 implicit pad bytes after `flag` -> derive error
```

Fix by ordering fields large-to-small and/or adding an explicit pad field:

```rust
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Good { value: u32, flag: u8, _pad: [u8; 3] }   // padding is now an explicit field
```

## `NoUninit`

Structs and enums. Like `Pod` but no `AnyBitPattern` requirement, so it admits types with
illegal bit patterns (`bool`, `char`, fieldless enums) — you can read their bytes out, you
just can't build them from arbitrary bytes.
- Struct: every field `NoUninit`; `#[repr(C)]`/`#[repr(transparent)]`; no padding; **no
  generic parameters** (transparent does *not* grant the generics exemption it does for
  `Pod`).
- Enum: every field `NoUninit`; explicit `#[repr(Int)]` and/or `#[repr(C)]`; no padding in
  any variant; all variants the same size; no padding between discriminant and fields; no
  generics.

Use it for a type you only ever upload (write side) — e.g. a vertex containing a
`#[repr(u32)]` enum flag — paired with `ByteEq`/`ByteHash` if you want byte equality.

## `AnyBitPattern`

Structs. The doc lists exactly one requirement: every field is `AnyBitPattern`. It does not
require `Copy` syntactically the way `Pod` does, and the page is silent on repr — but to be
meaningfully castable you'll pair it with `#[repr(C)]`/`#[repr(transparent)]`. Reach for it
over `Pod` only when a field is `AnyBitPattern` but not `NoUninit` (e.g. it may contain
padding you're willing to read as arbitrary bytes). For ordinary GPU structs, prefer `Pod`.

## `CheckedBitPattern`

Structs and enums — the safe way to round-trip a type with illegal bit patterns.
- Struct: every field `CheckedBitPattern`; `#[repr(C)]`/`#[repr(transparent)]`; no generics.
- Enum: explicit `#[repr(Int)]`; variant fields `CheckedBitPattern`; no generics.

Generates the `Bits` companion type and `is_valid_bit_pattern`. After deriving, cast via the
`bytemuck::checked::*` functions, which validate and return `CheckedCastError` instead of UB.

```rust
#[repr(u8)]
#[derive(Copy, Clone, bytemuck::CheckedBitPattern)]
enum BlendMode { Normal = 0, Multiply = 1, Screen = 2 }

let b: &BlendMode = bytemuck::checked::from_bytes(&[1]);   // Ok -> Multiply
// bytemuck::checked::try_from_bytes::<BlendMode>(&[7]) -> Err(InvalidBitPattern)
```

## `Contiguous`

Fieldless enums only. Requires `#[repr(Int)]` and contiguous discriminants (no gaps).
Gives you checked `from_integer` / `into_integer`. Good for mapping a small integer read
from a file or UI into an enum without a hand-written match.

```rust
#[repr(u8)]
#[derive(Copy, Clone, bytemuck::Contiguous)]
enum Tool { Pencil = 0, Eraser = 1, Bucket = 2, Eyedropper = 3 }
// Tool::from_integer(2) == Some(Tool::Bucket); Tool::from_integer(9) == None
```

## `TransparentWrapper`

Requires `#[repr(transparent)]`. Single-field structs infer the inner type; with extra
fields (which must be `Zeroable` ZSTs like `PhantomData`) annotate `#[transparent(Inner)]`
with the exact inner-type tokens. Enables the `wrap_ref`/`peel_ref` conversions. Rarely the
right tool in Pixhaus — a plain `Pod` newtype is usually simpler.

## `ByteEq` / `ByteHash`

Implement `PartialEq`+`Eq` / `Hash` by comparing or hashing the raw bytes. The type must be
byte-comparable, i.e. `NoUninit` (every example pairs them with `NoUninit` + `#[repr(C)]`);
padding would make the byte view ill-defined. Fast for blittable structs, with two caveats:
float comparison/hash is bitwise (so `NaN != NaN` differently than usual, `+0.0`/`-0.0`
differ), and the result does **not** match the standard library's derived `Eq`/`Hash`. Use
them only when the byte semantics are what you actually want; otherwise derive the normal
`PartialEq`/`Hash`.
