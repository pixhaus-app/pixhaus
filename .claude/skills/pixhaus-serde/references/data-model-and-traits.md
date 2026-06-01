# serde data model and core traits

The conceptual layer under every serde operation. Derived from serde.rs and docs.rs for
serde 1.0.228; signatures are stable across all of 1.x.

## Table of contents

- [The four traits](#the-four-traits)
- [The 29-type data model](#the-29-type-data-model)
- [Serialize](#serialize)
- [Deserialize](#deserialize)
- [Serializer](#serializer)
- [Deserializer](#deserializer)
- [The `'de` lifetime and zero-copy](#the-de-lifetime-and-zero-copy)
- [`DeserializeOwned`](#deserializeowned)
- [`is_human_readable()`](#is_human_readable)

## The four traits

serde splits into data structures and data formats:

- **Data structures** implement `Serialize` / `Deserialize` — your types, usually via
  `#[derive]`. Defined in `serde::ser` / `serde::de`, re-exported at the crate root.
- **Data formats** implement `Serializer` / `Deserializer` — `serde_json`, `rmp-serde`,
  etc. You almost never implement these in Pixhaus; you consume them.

The interaction is wired at compile time through the data model below — no runtime
reflection, and it often optimizes away entirely.

## The 29-type data model

Every `Serialize` maps a Rust type *into* one of these 29 types; the `Serializer` maps
that *out* to a format. Deserialize runs it in reverse. Knowing the model explains why
some shapes serialize the way they do and why binary formats are strict.

**14 primitives:** `bool`, `i8`/`i16`/`i32`/`i64`/`i128`, `u8`/`u16`/`u32`/`u64`/`u128`,
`f32`/`f64`, `char`.

**15 composites:**

| Model type | Rust example / meaning |
|---|---|
| string | UTF-8 bytes, length-prefixed, may contain NULs. Three deserialize flavors (transient/borrowed/owned). |
| byte array | `&[u8]` — like strings, same three flavors. serde has no first-class bytes type, which is why `Vec<u8>` needs `serde_bytes` for the project file (see pixhaus-rmp-serde). |
| option | `Option<T>` — none or some. |
| unit | `()` — no data. |
| unit_struct | `struct Unit;`, `PhantomData<T>`. |
| unit_variant | `E::A` in `enum E { A, B }`. |
| newtype_struct | `struct Millimeters(u8)`. |
| newtype_variant | `E::N(u8)`. |
| seq | `Vec<T>`, `HashSet<T>` — variable length, possibly unknown up front. |
| tuple | `(u8, bool)`, `[u64; 10]` — length statically known. |
| tuple_struct | `struct Rgb(u8, u8, u8)`. |
| tuple_variant | `E::T(u8, u8)`. |
| map | `BTreeMap<K, V>` — variable-length key/value pairs, arbitrary keys. |
| struct | `struct S { r: u8, g: u8 }` — compile-time-constant string keys. |
| struct_variant | `E::S { r: u8, g: u8 }`. |

Two distinctions the model encodes, both load-bearing:

- **seq vs tuple:** seq length may be unknown before iterating; tuple length is known
  without looking at the data. This is why `serialize_seq` takes `Option<usize>` and
  `serialize_tuple` takes `usize`.
- **map vs struct:** struct keys are compile-time-constant strings; map keys are dynamic.
  A format can therefore treat a struct's fields specially (e.g. MessagePack's
  named-vs-array struct encoding — see pixhaus-rmp-serde).

## Serialize

```rust
pub trait Serialize {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer;
}
```

The type drives the serializer: `serialize` inspects `&self` and calls exactly one
`Serializer::serialize_*` method, recursing into fields for composites. serde ships impls
for primitives and most std types; derive generates the rest. Not dyn-compatible — there's
no `dyn Serialize`.

## Deserialize

```rust
pub trait Deserialize<'de>: Sized {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>;
}
```

`'de` is the lifetime of data the result may borrow from the input (see below). The type
calls a `deserializer.deserialize_*` hint method and passes a **Visitor** whose `visit_*`
callbacks the deserializer invokes with the values it found. This inverts control: for a
self-describing format the deserializer picks which `visit_*` to call; otherwise the hint
method tells it what to expect. Not dyn-compatible.

> `deserialize_in_place` exists in serde's source as a `#[doc(hidden)]` provided method for
> reusing an allocation, but it does not render on the public docs and is not public API.

## Serializer

The trait a format implements. Associated types:

```rust
type Ok;     // success value: () for streaming writers, String/Value for in-memory
type Error;  // implements serde::ser::Error
// plus seven compound builders, each sharing Ok/Error:
type SerializeSeq; type SerializeTuple; type SerializeTupleStruct;
type SerializeTupleVariant; type SerializeMap; type SerializeStruct;
type SerializeStructVariant;
```

Scalar methods take `self` and return `Result<Self::Ok, Self::Error>`:
`serialize_bool/i8../i128/u8../u128/f32/f64/char/str/bytes/none/unit`, plus
`serialize_some<T>(self, &T)`, `serialize_unit_struct(name)`,
`serialize_unit_variant(name, index: u32, variant)`,
`serialize_newtype_struct(name, &T)`,
`serialize_newtype_variant(name, index, variant, &T)`.

Compound methods return one of the builder associated types; you feed elements in, then
call `.end()`:

```rust
serialize_seq(self, len: Option<usize>)            -> SerializeSeq      // len may be unknown
serialize_tuple(self, len: usize)                  -> SerializeTuple
serialize_tuple_struct(name, len: usize)           -> SerializeTupleStruct
serialize_tuple_variant(name, idx, variant, len)   -> SerializeTupleVariant
serialize_map(self, len: Option<usize>)            -> SerializeMap
serialize_struct(name, len: usize)                 -> SerializeStruct
serialize_struct_variant(name, idx, variant, len)  -> SerializeStructVariant
```

Variant methods carry both `variant_index: u32` and `variant: &'static str` so a format can
pick a compact index (binary) or the name (human-readable).

## Deserializer

Each method takes `self` plus a `visitor: V where V: Visitor<'de>` and returns
`Result<V::Value, Self::Error>`. Two groups:

- **Type hints:** `deserialize_bool`, `deserialize_i8..i128`, `deserialize_u8..u128`,
  `deserialize_f32/f64`, `deserialize_char`, `deserialize_str`, `deserialize_string`,
  `deserialize_bytes`, `deserialize_byte_buf`, `deserialize_option`, `deserialize_unit`,
  `deserialize_unit_struct(name, V)`, `deserialize_newtype_struct(name, V)`,
  `deserialize_seq`, `deserialize_tuple(len, V)`, `deserialize_tuple_struct(name, len, V)`,
  `deserialize_map`, `deserialize_struct(name, fields: &'static [&'static str], V)`,
  `deserialize_enum(name, variants: &'static [&'static str], V)`,
  `deserialize_identifier`, `deserialize_ignored_any`.
- **`deserialize_any`:** "look at the input and tell me." Drives the Visitor by whatever
  the input contains.

**Self-describing vs not** — the distinction that governs Pixhaus's two formats:

- **Self-describing** (JSON, MessagePack): can inspect the bytes to know what they
  represent, so `deserialize_any` works and dynamic values like `serde_json::Value`
  deserialize. JSON often routes every hint to `deserialize_any`.
- **Non-self-describing** (bincode, postcard): must be told the type via the hint methods;
  cannot deserialize `deserialize_any`-based dynamic values.

When hand-writing `Deserialize`, prefer the specific hint over `deserialize_any` unless you
genuinely need the format to tell you the type — relying on `deserialize_any` breaks on
non-self-describing formats. (MessagePack via rmp-serde is self-describing but reports
`is_human_readable() == false`; see below and pixhaus-rmp-serde.)

## The `'de` lifetime and zero-copy

`'de` bounds how long borrowed data must live. It lets a deserialized type hold references
*into* the input instead of allocating:

```rust
#[derive(Deserialize)]
struct User<'a> {
    id: u32,
    name: &'a str,   // borrowed straight from the input buffer — no copy
}
```

Three string/byte flavors a Visitor distinguishes:

- **Transient** — `visit_str(&str)`: does not outlive the call (IO buffer, unescaped temp).
- **Borrowed** — `visit_borrowed_str(&'de str)`: lives as long as `'de`, can be borrowed by
  the result.
- **Owned** — `visit_string(String)`: the deserializer hands over an allocation.

**Only in-memory deserializers can borrow.** `from_slice` / `from_str` can hand out
`&'de` references; `from_reader` cannot (the input is a stream), so it requires owned data.

Two bound styles:

- `T: Deserialize<'de>` — caller picks `'de` (the input's lifetime); result may borrow.
  Used by `serde_json::from_str`, `rmp_serde::from_slice`.
- `T: DeserializeOwned` — result borrows nothing. Used by `from_reader`-style APIs.

`&str`/`&[u8]` fields borrow implicitly; other borrowing types need `#[serde(borrow)]`.
Avoid putting `'static` near `Deserialize`.

## DeserializeOwned

```rust
pub trait DeserializeOwned: for<'de> Deserialize<'de> {}
```

A marker for "deserializable without borrowing from the input." It's exactly the
higher-ranked bound `for<'de> Deserialize<'de>`. Require it whenever the decoded value must
outlive the input buffer — reading through a `zstd` stream decoder (`from_read`), or
handing a result back to the egui frame loop over a channel. In Pixhaus, anything that
crosses a thread boundary is `DeserializeOwned` in practice (owns its data).

## is_human_readable()

A provided method on **both** `Serializer` and `Deserializer`:

```rust
fn is_human_readable(&self) -> bool   // default: true
```

It lets a `Serialize`/`Deserialize` impl pick between a readable form (a UUID as a
hyphenated string, a timestamp as RFC 3339) and a compact binary form (16 raw bytes, packed
ints). Text formats leave the default `true` (`serde_json`); binary formats override to
`false` (`rmp-serde`).

The consequence that bites: **the same struct can round-trip through both `serde_json` and
`rmp-serde` and produce different wire shapes.** A type whose `Serialize` assumes the string
branch can break under MessagePack. The serializer's and deserializer's `is_human_readable()`
must agree for a given format, or the round-trip fails. You normally want the binary branch
on disk — don't force `with_human_readable()` without a reason (see pixhaus-rmp-serde).
