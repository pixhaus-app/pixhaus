# rmp-serde 1.3 API reference

The complete public surface, grouped by module. Read the `SKILL.md` first for the
two decisions that actually matter in Pixhaus (named maps, byte buffers); come
here for exact signatures and the long tail.

## Table of contents

- [Crate root re-exports](#crate-root-re-exports)
- [encode module](#encode-module)
- [decode module](#decode-module)
- [config module](#config-module)
- [serde_bytes companion crate](#serde_bytes-companion-crate)
- [The MessagePack type mapping](#the-messagepack-type-mapping)

## Crate root re-exports

The crate root re-exports the four functions you reach for 95% of the time, plus
the two builder types.

| Item | Kind | What it is |
|---|---|---|
| `rmp_serde::to_vec` | fn | Serialize to `Vec<u8>`, structs as **arrays** (compact, no field names). |
| `rmp_serde::to_vec_named` | fn | Serialize to `Vec<u8>`, structs as **maps** (field names included). |
| `rmp_serde::from_slice` | fn | Deserialize from a `&[u8]`, borrowing (zero-copy) where the target allows. |
| `rmp_serde::from_read` | fn | Deserialize from any `impl Read` (owned data only). |
| `rmp_serde::Serializer` | struct | The configurable encoder. Re-export of `encode::Serializer`. |
| `rmp_serde::Deserializer` | struct | The configurable decoder. Re-export of `decode::Deserializer`. |
| `rmp_serde::MSGPACK_EXT_STRUCT_NAME` | const | Magic struct name that triggers MessagePack Ext encoding. Niche. |

Modules: `encode`, `decode`, `config`.

## encode module

### Free functions

```rust
// Compact: structs -> MessagePack arrays. Smallest output, position-dependent schema.
pub fn to_vec<T>(val: &T) -> Result<Vec<u8>, Error>
where T: Serialize + ?Sized;

// Named: structs -> MessagePack maps keyed by field name. Larger, schema-evolvable.
pub fn to_vec_named<T>(val: &T) -> Result<Vec<u8>, Error>
where T: Serialize + ?Sized;

// Stream variants write into an io::Write instead of allocating a Vec.
pub fn write<W, T>(wr: &mut W, val: &T) -> Result<(), Error>
where W: Write, T: Serialize + ?Sized;          // structs as arrays

pub fn write_named<W, T>(wr: &mut W, val: &T) -> Result<(), Error>
where W: Write, T: Serialize + ?Sized;          // structs as maps
```

The free functions give you no hook to set `BytesMode` — for that, build a
`Serializer` by hand (see below). `serde_bytes` field annotations work with all
four, so prefer those for byte buffers.

### Serializer

```rust
pub struct Serializer<W, C = DefaultConfig> { /* ... */ }

impl<W: Write> Serializer<W> {
    pub fn new(wr: W) -> Self;            // default config: structs as arrays, binary, Normal bytes
}

impl<W, C> Serializer<W, C> {
    // Config builders. Each consumes self and returns a Serializer with a new C.
    pub fn with_struct_map(self)   -> Serializer<W, StructMapConfig<C>>;   // structs -> maps
    pub fn with_struct_tuple(self) -> Serializer<W, StructTupleConfig<C>>; // structs -> arrays (default)
    pub fn with_human_readable(self) -> Serializer<W, HumanReadableConfig<C>>; // is_human_readable() = true
    pub fn with_binary(self)       -> Serializer<W, BinaryConfig<C>>;      // is_human_readable() = false (default)

    // Prefer encoding u8 sequences as the `bin` type instead of int arrays.
    pub const fn with_bytes(self, mode: BytesMode) -> Self;

    // Writer access.
    pub fn get_ref(&self) -> &W;
    pub fn get_mut(&mut self) -> &mut W;
    pub fn into_inner(self) -> W;
}
```

`Serializer<W, C>` implements `serde::Serializer`, so you drive it with
`value.serialize(&mut serializer)`.

Helper types `ExtSerializer` / `ExtFieldSerializer` and the `UnderlyingWrite`
trait exist for MessagePack Ext support — ignore them unless you are emitting Ext
types.

### encode::Error

```rust
pub enum Error {
    InvalidValueWrite(ValueWriteError), // underlying write of a value failed
    UnknownLength,                      // a seq/map was serialized without a known length
    DepthLimitExceeded,
    Syntax(String),                     // catch-all from Serialize impls (custom messages land here)
}
```

Implements `std::error::Error` + `Display`, and `serde::ser::Error`. Wrap it in
your `io` crate's `thiserror` enum with `#[from]`.

## decode module

### Free functions

```rust
// Borrowing deserialize from a slice. T may borrow from `input` (zero-copy) when
// it contains &str / &[u8] fields; otherwise it just reads the slice.
pub fn from_slice<'a, T>(input: &'a [u8]) -> Result<T, Error>
where T: Deserialize<'a>;

// Deserialize from a reader. T must own its data (DeserializeOwned) — you cannot
// borrow out of a streaming reader.
pub fn from_read<R, T>(rd: R) -> Result<T, Error>
where R: Read, T: DeserializeOwned;
```

### Deserializer

```rust
pub struct Deserializer<R, C = DefaultConfig> { /* ... */ }

impl<R: Read> Deserializer<R, DefaultConfig> {
    pub fn new(rd: R) -> Self;
}

impl<'de, R, C> Deserializer<R, C> {
    pub fn with_human_readable(self) -> Deserializer<R, HumanReadableConfig<C>>;
    pub fn with_binary(self)         -> Deserializer<R, BinaryConfig<C>>;
    pub fn into_inner(self) -> R;
    pub fn get_ref(&self) -> &R;
    // position(), and slice-based constructors for borrowing readers, also exist.
}
```

`Deserializer` implements `serde::Deserializer`. Building one by hand is mostly
for reading several MessagePack messages from one stream in sequence — the free
functions assume exactly one message.

Companion read types: `ReadReader` (owned), `ReadRefReader` (borrowing),
`Reference` (unifies borrowed/owned slices), and the `ReadSlice` trait that lets a
reader hand back borrowed slices for zero-copy.

### decode::Error

```rust
pub enum Error {
    InvalidMarkerRead(io::Error),  // I/O error reading a MessagePack marker byte
    InvalidDataRead(io::Error),    // I/O error reading encoded data (includes premature EOF)
    TypeMismatch(Marker),          // decoded marker did not match the expected type
    OutOfRange,                    // numeric cast overflowed the target type
    LengthMismatch(u32),           // array length did not match the expected fixed length
    Uncategorized(String),
    Syntax(String),                // general deserialize failure with a message
    Utf8Error(Utf8Error),          // a string field was not valid UTF-8
    DepthLimitExceeded,
}
```

There is no dedicated "trailing bytes" variant: the free `from_slice` / `from_read`
decode exactly one message and ignore anything after it. If you need to detect or
reject trailing data, drive a `Deserializer` directly and inspect what remains.

## config module

These are the type-level config wrappers the `Serializer`/`Deserializer` builders
produce. You rarely name them directly — `with_struct_map()` etc. return them — but
they show up in type signatures.

| Type | Effect |
|---|---|
| `DefaultConfig` | Structs as arrays, binary (`is_human_readable() == false`), `BytesMode::Normal`. |
| `StructMapConfig<C>` | Structs serialize/deserialize as maps with field names. |
| `StructTupleConfig<C>` | Structs serialize/deserialize as arrays (the default). |
| `HumanReadableConfig<C>` | Forces `is_human_readable()` to `true`. |
| `BinaryConfig<C>` | Forces `is_human_readable()` to `false`. |
| `SerializerConfig` (trait) | Implemented by all the above; the bound the builders use. |

### BytesMode

Controls when a sequence of `u8` is written as the MessagePack `bin` type versus a
sequence of integers.

```rust
pub enum BytesMode {
    Normal,          // default: bytes only when Serde asks (i.e. serde_bytes)
    ForceIterables,  // bytes for slices, Vec, and other Iterator-based seqs of u8;
                     // NOT fixed-length arrays. May break some Deserialize impls.
    ForceAll,        // bytes for everything that looks like a container of u8.
                     // Breaks some Deserialize impls.
}
```

`Normal` is the safe default and the one to keep. The `Force*` modes are a blunt
workaround for not annotating fields — they change the wire format of *every* `u8`
container in the type, so a value written with `ForceAll` only round-trips if the
reader also expects `bin`. Prefer per-field `serde_bytes` over `Force*`.

## serde_bytes companion crate

Not part of rmp-serde, but the recommended way to make byte buffers compact.
Separate crate, dual MIT/Apache-2.0 — passes the workspace MIT lock.

```toml
serde_bytes = "0.11"
```

```rust
// On a struct field: works with to_vec AND to_vec_named, no Serializer rebuild.
#[serde(with = "serde_bytes")]
pixels: Vec<u8>,

// Standalone wrapper types when you need a value, not a field:
serde_bytes::ByteBuf   // owned, like Vec<u8>
serde_bytes::Bytes     // borrowed, like &[u8]
```

A `Vec<u8>` field without this annotation serializes as N MessagePack integers —
one marker byte (often more) per pixel byte. The annotation makes it a single
`bin` blob with a length header. For Pixhaus pixel buffers this is the difference
between a sane file and a multi-hundred-megabyte one.

## The MessagePack type mapping

How Serde concepts land on the wire, so you can reason about size and compatibility:

| Rust / Serde | MessagePack |
|---|---|
| struct (default / `to_vec`) | array of field values, positional |
| struct (`to_vec_named` / `with_struct_map`) | map of field-name -> value |
| enum variant | map `{ variant => payload }` (externally tagged) |
| `Vec<T>`, slices, tuples | array |
| `HashMap` / `BTreeMap` | map |
| `Vec<u8>` (plain) | array of ints — wasteful |
| `Vec<u8>` (`serde_bytes`) | `bin` blob |
| `Option<T>` | `nil` for `None`, else the value |
| `()` / unit struct | `nil` |
