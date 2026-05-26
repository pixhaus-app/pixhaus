# Hand-writing Serialize and Deserialize

For the rare type whose wire shape attributes can't express. Verified against serde.rs and
docs.rs for serde 1.0.228. Reach for this only after confirming the derive plus attributes
(`references/derive-attributes.md`) won't do — a manual impl is more surface to keep correct
across both directions and every format.

## Table of contents

- [Manual Serialize](#manual-serialize)
- [Manual Deserialize: the Visitor pattern](#manual-deserialize-the-visitor-pattern)
- [The Visitor trait](#the-visitor-trait)
- [SeqAccess and MapAccess](#seqaccess-and-mapaccess)
- [DeserializeSeed](#deserializeseed)
- [Error construction](#error-construction)
- [The string-or-struct pattern](#the-string-or-struct-pattern)

## Manual Serialize

Call **exactly one** method on the `Serializer`. Compound forms return a builder you feed,
then `.end()`.

```rust
use serde::ser::{Serialize, SerializeStruct, Serializer};

impl Serialize for Color {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where S: Serializer {
        let mut s = serializer.serialize_struct("Color", 3)?;
        s.serialize_field("r", &self.r)?;
        s.serialize_field("g", &self.g)?;
        s.serialize_field("b", &self.b)?;
        s.end()
    }
}
```

Other shapes:

```rust
// seq — pass Some(len) if known, None if not
let mut seq = serializer.serialize_seq(Some(self.len()))?;
for e in self { seq.serialize_element(e)?; }
seq.end()

// map — serialize_entry, or serialize_key then serialize_value
let mut map = serializer.serialize_map(Some(self.len()))?;
for (k, v) in self { map.serialize_entry(k, v)?; }
map.end()

// enum variants — all carry (name, index: u32, variant: &'static str, ...)
match self {
    E::Unit         => s.serialize_unit_variant("E", 0, "Unit"),
    E::Newtype(v)   => s.serialize_newtype_variant("E", 1, "Newtype", v),
    E::Tuple(a, b)  => {
        let mut tv = s.serialize_tuple_variant("E", 2, "Tuple", 2)?;
        tv.serialize_field(a)?; tv.serialize_field(b)?; tv.end()
    }
    E::Struct { x } => {
        let mut sv = s.serialize_struct_variant("E", 3, "Struct", 1)?;
        sv.serialize_field("x", x)?; sv.end()
    }
}
```

Plus the scalars: `serialize_bool/i64/u64/f64/str/bytes/unit`, `serialize_some(&v)` /
`serialize_none()` for `Option`.

## Manual Deserialize: the Visitor pattern

A `Deserializer` is data-driven: it calls back into the `Visitor` method matching what the
input actually contains. You implement a `Visitor`, then call the `deserialize_*` hint that
tells the format the shape you want. **Implement every shape the format can hand you** — for
a struct that means both `visit_seq` (a format that sends arrays) and `visit_map` (one that
sends maps). The default `visit_*` methods error, so a missing one fails at runtime, not
compile time.

Full struct example (the canonical serde.rs `Duration`): a `Field` enum deserialized via
`deserialize_identifier`, then the main visitor with both `visit_seq` and `visit_map`:

```rust
use std::fmt;
use serde::de::{self, Deserialize, Deserializer, Visitor, SeqAccess, MapAccess};

impl<'de> Deserialize<'de> for Duration {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where D: Deserializer<'de> {
        enum Field { Secs, Nanos }

        impl<'de> Deserialize<'de> for Field {
            fn deserialize<D>(deserializer: D) -> Result<Field, D::Error>
            where D: Deserializer<'de> {
                struct FieldVisitor;
                impl<'de> Visitor<'de> for FieldVisitor {
                    type Value = Field;
                    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                        f.write_str("`secs` or `nanos`")
                    }
                    fn visit_str<E>(self, value: &str) -> Result<Field, E>
                    where E: de::Error {
                        match value {
                            "secs"  => Ok(Field::Secs),
                            "nanos" => Ok(Field::Nanos),
                            _ => Err(de::Error::unknown_field(value, FIELDS)),
                        }
                    }
                }
                deserializer.deserialize_identifier(FieldVisitor)
            }
        }

        struct DurationVisitor;
        impl<'de> Visitor<'de> for DurationVisitor {
            type Value = Duration;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("struct Duration")
            }

            fn visit_seq<V>(self, mut seq: V) -> Result<Duration, V::Error>
            where V: SeqAccess<'de> {
                let secs = seq.next_element()?
                    .ok_or_else(|| de::Error::invalid_length(0, &self))?;
                let nanos = seq.next_element()?
                    .ok_or_else(|| de::Error::invalid_length(1, &self))?;
                Ok(Duration::new(secs, nanos))
            }

            fn visit_map<V>(self, mut map: V) -> Result<Duration, V::Error>
            where V: MapAccess<'de> {
                let mut secs = None;
                let mut nanos = None;
                while let Some(key) = map.next_key()? {
                    match key {
                        Field::Secs => {
                            if secs.is_some() { return Err(de::Error::duplicate_field("secs")); }
                            secs = Some(map.next_value()?);
                        }
                        Field::Nanos => {
                            if nanos.is_some() { return Err(de::Error::duplicate_field("nanos")); }
                            nanos = Some(map.next_value()?);
                        }
                    }
                }
                let secs = secs.ok_or_else(|| de::Error::missing_field("secs"))?;
                let nanos = nanos.ok_or_else(|| de::Error::missing_field("nanos"))?;
                Ok(Duration::new(secs, nanos))
            }
        }

        const FIELDS: &[&str] = &["secs", "nanos"];
        deserializer.deserialize_struct("Duration", FIELDS, DurationVisitor)
    }
}
```

`next_key()` / `next_value()` infer their target from context (here `next_key` yields a
`Field` because the match forces it).

A scalar visitor implements only the `visit_*` it accepts; the rest error if called:

```rust
impl<'de> Visitor<'de> for I32Visitor {
    type Value = i32;
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("an integer between -2^31 and 2^31")
    }
    fn visit_i32<E>(self, v: i32) -> Result<i32, E> where E: de::Error { Ok(v) }
    fn visit_i64<E>(self, v: i64) -> Result<i32, E> where E: de::Error {
        i32::try_from(v).map_err(|_| E::custom(format!("i32 out of range: {v}")))
    }
}
// deserializer.deserialize_i32(I32Visitor)
```

## The Visitor trait

```rust
type Value;                                            // what the visitor produces
fn expecting(&self, f: &mut Formatter) -> fmt::Result; // required
```

Every `visit_*` has a default. Two kinds of default:

**Default = type error** (override to accept that form): `visit_bool`, `visit_i64`,
`visit_i128`, `visit_u64`, `visit_u128`, `visit_f64`, `visit_str`, `visit_bytes`,
`visit_none`, `visit_some<D>`, `visit_unit`, `visit_newtype_struct<D>`, `visit_seq<A>`,
`visit_map<A>`, `visit_enum<A>`.

**Default = delegate to a wider method** (override only for the narrow/borrowed form):
`visit_i8/i16/i32` → `visit_i64`; `visit_u8/u16/u32` → `visit_u64`; `visit_f32` →
`visit_f64`; `visit_char` → `visit_str`; `visit_borrowed_str` → `visit_str`; `visit_string`
(owned `String`) → `visit_str`; `visit_borrowed_bytes` → `visit_bytes`; `visit_byte_buf`
(owned `Vec<u8>`) → `visit_bytes`.

Scalar shape: `fn visit_x<E>(self, v: X) -> Result<Self::Value, E> where E: de::Error`. The
ones receiving a sub-deserializer or accessor (`visit_some`, `visit_newtype_struct`,
`visit_seq/map/enum`) return `Result<Self::Value, D::Error>` / `A::Error`. The `borrowed`
variants hand you data alive for the full `'de` lifetime — implement them for zero-copy.

## SeqAccess and MapAccess

```rust
// SeqAccess<'de>
fn next_element_seed<T: DeserializeSeed<'de>>(&mut self, seed: T)
    -> Result<Option<T::Value>, Self::Error>;            // required
fn next_element<T: Deserialize<'de>>(&mut self)
    -> Result<Option<T>, Self::Error>;                   // provided; Some=item, None=done
fn size_hint(&self) -> Option<usize>;                    // provided

// MapAccess<'de>
fn next_key_seed<K: DeserializeSeed<'de>>(&mut self, seed: K)
    -> Result<Option<K::Value>, Self::Error>;            // required
fn next_value_seed<V: DeserializeSeed<'de>>(&mut self, seed: V)
    -> Result<V::Value, Self::Error>;                    // required
fn next_entry<K, V>(&mut self) -> Result<Option<(K, V)>, Self::Error>; // provided
fn next_key<K>(&mut self)   -> Result<Option<K>, Self::Error>;         // provided
fn next_value<V>(&mut self) -> Result<V, Self::Error>;                 // provided
fn size_hint(&self) -> Option<usize>;                                 // provided
```

Contract: call `next_key` and only call `next_value` after a key returned `Some`.
`next_entry` does both at once. Use `size_hint().unwrap_or(0)` to pre-size a collection:

```rust
fn visit_map<M>(self, mut access: M) -> Result<Self::Value, M::Error>
where M: MapAccess<'de> {
    let mut map = MyMap::with_capacity(access.size_hint().unwrap_or(0));
    while let Some((k, v)) = access.next_entry()? { map.insert(k, v); }
    Ok(map)
}
```

## DeserializeSeed

```rust
pub trait DeserializeSeed<'de>: Sized {
    type Value;
    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where D: Deserializer<'de>;
}
```

Like `Deserialize::deserialize`, but `self` carries seed state. Every `T: Deserialize<'de>`
has a blanket `DeserializeSeed` impl — that's why the non-seed `next_element`/`next_key`
work. Reach for it only when deserialization needs state the plain impl can't see — the
classic case is deserializing into a pre-allocated buffer by passing `&mut Vec<T>` as the
seed. Drive it through `next_element_seed`/`next_key_seed`/`next_value_seed`.

## Error construction

```rust
// serde::de::Error
fn custom<T: Display>(msg: T) -> Self;                            // required
fn invalid_type(unexp: Unexpected, exp: &dyn Expected) -> Self;
fn invalid_value(unexp: Unexpected, exp: &dyn Expected) -> Self;
fn invalid_length(len: usize, exp: &dyn Expected) -> Self;
fn unknown_field(field: &str, expected: &'static [&'static str]) -> Self;
fn unknown_variant(variant: &str, expected: &'static [&'static str]) -> Self;
fn missing_field(field: &'static str) -> Self;
fn duplicate_field(field: &'static str) -> Self;

// serde::ser::Error
fn custom<T: Display>(msg: T) -> Self;
```

`&dyn Expected` is satisfied by `&self` inside a visitor (a `Visitor` is `Expected` via its
`expecting`), so `de::Error::invalid_length(0, &self)` is the idiom. Message convention: not
capitalized, no trailing period. `Error::custom` also bridges cross-format errors, e.g.
inside a `serialize_with`: `serde_json::to_string(v).map_err(ser::Error::custom)?`.

`Unexpected<'a>` (the `unexp` arg) has 18 variants: `Bool`, `Unsigned(u64)`, `Signed(i64)`,
`Float`, `Char`, `Str`, `Bytes`, `Unit`, `Option`, `NewtypeStruct`, `Seq`, `Map`, `Enum`,
`UnitVariant`, `NewtypeVariant`, `TupleVariant`, `StructVariant`, `Other(&str)`. Ints
collapse to `Unsigned`/`Signed` — no per-width variant.

## The string-or-struct pattern

A field that is either a bare string or a full map. One visitor implements both `visit_str`
and `visit_map`; the map branch re-enters normal deserialization via
`MapAccessDeserializer`:

```rust
use std::{fmt, marker::PhantomData, str::FromStr};
use serde::de::{self, Deserialize, Deserializer, MapAccess, Visitor};

fn string_or_struct<'de, T, D>(deserializer: D) -> Result<T, D::Error>
where
    T: Deserialize<'de> + FromStr<Err = std::convert::Infallible>,
    D: Deserializer<'de>,
{
    struct SoS<T>(PhantomData<fn() -> T>);

    impl<'de, T> Visitor<'de> for SoS<T>
    where T: Deserialize<'de> + FromStr<Err = std::convert::Infallible> {
        type Value = T;
        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("string or map")
        }
        fn visit_str<E>(self, value: &str) -> Result<T, E> where E: de::Error {
            Ok(T::from_str(value).unwrap())   // Err = Infallible, can't fail
        }
        fn visit_map<M>(self, map: M) -> Result<T, M::Error> where M: MapAccess<'de> {
            Deserialize::deserialize(de::value::MapAccessDeserializer::new(map))
        }
    }
    deserializer.deserialize_any(SoS(PhantomData))
}
```

Apply with `#[serde(deserialize_with = "string_or_struct")]`. It uses `deserialize_any` so
the (self-describing) format reports whether it found a string or a map — which is why this
is a JSON pattern, not a project-file one.

## Verification note

The `serialize_struct`/variant builder method names and the `*_with` signatures are the
established serde contract; serde.rs links rather than quoting some of them verbatim.
Everything in the Visitor / `SeqAccess` / `MapAccess` / error sections was quoted from the
rendered docs. If a manual impl produces a cryptic compile error on a builder method,
confirm the signature against https://docs.rs/serde/latest/serde/ser/ before assuming the
example is wrong.
