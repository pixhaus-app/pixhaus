# serde_json 1.0.150 API reference

The complete public surface, grouped by area. Read the `SKILL.md` first for the
four decisions that actually matter in Pixhaus (typed structs over `Value`, no
`json!` in library code, `from_slice` over `from_reader`, `.get()` over `[ ]`); come
here for exact signatures and the long tail.

All signatures are verbatim from docs.rs 1.0.150. License `MIT OR Apache-2.0`. Only
`std` is enabled by default.

## Table of contents

- [Crate-root functions](#crate-root-functions)
- [The `json!` macro](#the-json-macro)
- [`Value`](#value)
- [`Number`](#number)
- [`Map`](#map)
- [`Error`, `Category`, `Result`](#error-category-result)
- [`Deserializer` and `StreamDeserializer`](#deserializer-and-streamdeserializer)
- [`RawValue`](#rawvalue)
- [The `ser` module: custom formatting](#the-ser-module-custom-formatting)
- [Cargo features](#cargo-features)
- [JSON ↔ Rust type mapping](#json--rust-type-mapping)

## Crate-root functions

Every function returns `serde_json::Result<T>` = `Result<T, serde_json::Error>`. The
`to_*` and reader/writer functions are gated on the `std` feature (on by default);
`from_str`/`from_slice` work under `alloc` alone.

### Deserialize

```rust
// Parse from in-memory text/bytes. Output may BORROW from the input (the 'a tie),
// so &str / &RawValue fields can be zero-copy.
pub fn from_str<'a, T>(s: &'a str) -> Result<T>      where T: Deserialize<'a>;
pub fn from_slice<'a, T>(v: &'a [u8]) -> Result<T>   where T: Deserialize<'a>;

// Parse from a reader. Note DeserializeOwned (no borrowing), and the two gotchas:
// usually SLOWER than read-to-String + from_str, and it does not return until EOF.
pub fn from_reader<R, T>(rdr: R) -> Result<T>
where R: Read, T: DeserializeOwned;

// Interpret an existing Value as T (no string round-trip).
pub fn from_value<T>(value: Value) -> Result<T>      where T: DeserializeOwned;
```

### Serialize

All serializers take the value by reference with a `?Sized` bound, **except**
`to_value`, which takes it by value. The `?Sized` bound is what lets you pass `&str`,
`&[u8]` slices, and trait objects.

```rust
pub fn to_string<T>(value: &T) -> Result<String>          where T: ?Sized + Serialize;
pub fn to_string_pretty<T>(value: &T) -> Result<String>   where T: ?Sized + Serialize;
pub fn to_vec<T>(value: &T) -> Result<Vec<u8>>            where T: ?Sized + Serialize;
pub fn to_vec_pretty<T>(value: &T) -> Result<Vec<u8>>     where T: ?Sized + Serialize;

pub fn to_writer<W, T>(writer: W, value: &T) -> Result<()>
where W: Write, T: ?Sized + Serialize;
pub fn to_writer_pretty<W, T>(writer: W, value: &T) -> Result<()>
where W: Write, T: ?Sized + Serialize;

// Build a Value from any Serialize. Returns Result where json! would PANIC.
pub fn to_value<T>(value: T) -> Result<Value, Error>      where T: Serialize;
```

Notes the docs call out:
- The serializers error if the `Serialize` impl fails **or** the value contains a
  **map with non-string keys** (e.g. `HashMap<i32, _>`). This is a runtime `Err`, not
  a compile error.
- `to_writer` only ever writes valid UTF-8 to the sink. Wrap unbuffered writers in
  `BufWriter`.
- `from_reader` is "usually slower than reading a file completely into memory and then
  applying `from_str`/`from_slice`," and it waits for the stream to end. Wrap
  unbuffered readers (e.g. `File`, sockets) in `BufReader`; don't point it at a stream
  that doesn't EOF.
- Pretty output uses two-space indentation (see `PrettyFormatter` below to change it).

## The `json!` macro

```rust
let v: Value = json!({ "code": 200, "items": ["a", "b"], "ok": true, "meta": null });
```

- Builds a `serde_json::Value` from inline JSON syntax, checked at compile time.
- **Interpolation:** any Rust expression can appear as a value; it must implement
  `Serialize`. Expressions can also be keys, as long as they resolve to something
  `Into<String>` (object keys must be string-convertible).
- **Trailing commas** are allowed in arrays and objects.
- **Panics** if an interpolated value's `Serialize` fails, or if you interpolate a map
  with non-string keys. Because of the no-panic rule ([[pixhaus-rust-conventions]]),
  prefer `to_value(x)?` in library code; reserve `json!` for tests and infallible
  literals.

```rust
let id = make_id();
let names = vec!["x", "y"];
let v = json!({
    "id": id,                 // any Serialize value
    "first": names[0],        // indexing expression
    names[1]: true,           // expression key (must be Into<String>)
    "trailing": [1, 2, 3,],   // trailing comma ok
});
```

## `Value`

```rust
pub enum Value {
    Null,
    Bool(bool),
    Number(Number),
    String(String),
    Array(Vec<Value>),
    Object(Map<String, Value>),   // serde_json::Map, NOT HashMap
}
```

### Type predicates — `&self -> bool`

```rust
is_null  is_boolean  is_number  is_string  is_array  is_object
is_i64   is_u64      is_f64
```

`is_i64`/`is_u64`/`is_f64` delegate to the contained `Number` and return `false` for
non-`Number` variants. **`is_f64` is true only for non-integer numbers** — an integer
returns `false` from `is_f64` even though `as_f64()` will still yield a value for it.
Test with `as_f64()` when you just want a float.

### Accessors — all return `Option`

```rust
pub fn as_null(&self) -> Option<()>;
pub fn as_bool(&self) -> Option<bool>;
pub fn as_i64(&self) -> Option<i64>;
pub fn as_u64(&self) -> Option<u64>;
pub fn as_f64(&self) -> Option<f64>;
pub fn as_str(&self) -> Option<&str>;
pub fn as_number(&self) -> Option<&Number>;
pub fn as_array(&self) -> Option<&Vec<Value>>;          // &Vec, not &[Value]
pub fn as_array_mut(&mut self) -> Option<&mut Vec<Value>>;
pub fn as_object(&self) -> Option<&Map<String, Value>>;
pub fn as_object_mut(&mut self) -> Option<&mut Map<String, Value>>;
```

Each returns `None` on a variant mismatch.

### `get` / `get_mut` — generic over `Index`

```rust
pub fn get<I: Index>(&self, index: I) -> Option<&Value>;
pub fn get_mut<I: Index>(&mut self, index: I) -> Option<&mut Value>;
```

`I: Index` is satisfied by `usize` (array element), `&str`/`String` (object key), and
`&T where T: Index`. Returns `None` for out-of-bounds, missing key, or an index whose
kind doesn't fit the value (a `usize` into a `String`). **`get` never panics** — it's
the safe probe.

### `pointer` / `pointer_mut` — JSON Pointer (RFC 6901)

```rust
pub fn pointer(&self, pointer: &str) -> Option<&Value>;
pub fn pointer_mut(&mut self, pointer: &str) -> Option<&mut Value>;
```

A `/`-separated path of tokens, starting with `/`. `""` returns the whole document.
Within a token, `~1` decodes to `/` and `~0` decodes to `~` (escape `~` as `~0`
first). Array indices are decimal. Returns `None` if the path doesn't resolve or is
malformed.

```rust
let data = json!({ "x": { "y": ["z", "zz"] } });
assert_eq!(data.pointer("/x/y/1"), Some(&json!("zz")));
assert_eq!(data.pointer("/a/b/c"), None);
```

### `take`

```rust
pub fn take(&mut self) -> Value;   // moves the value out, leaves Value::Null behind
```

### Indexing with `[ ]` — the Null trap

`Value` implements `Index<I>` returning `&Value` (not `Option`). The documented rule:

> Square-bracket indexing returns `Value::Null` wherever `get` would return `None` — a
> missing object key, an out-of-bounds array index, or indexing into a `Null`.

So a miss anywhere in `v["a"]["b"][0]` collapses the rest of the chain to `Null` and
**does not panic** — but indexing into a scalar (`String`/`Number`/`Bool`) *does*
panic, since scalars can't be indexed. A present-but-`null` field is indistinguishable
from an absent one under `[ ]`. Use `.get()` when you need that distinction.

### Display / pretty-printing

`impl Display for Value` writes **compact** JSON: `value.to_string()` /
`format!("{value}")`. The alternate flag pretty-prints: `format!("{value:#}")`.
Equivalently, `serde_json::to_string(&value)` (compact) or `to_string_pretty(&value)`.

## `Number`

Stores a JSON number as an integer or a float without exposing which. Default build:
internally `u64` / `i64` / `f64`. With `arbitrary_precision`, stored as a decimal
string (holds values outside i64/u64/f64 range, exact round-trip).

```rust
pub fn is_i64(&self) -> bool;   // integer within i64 range
pub fn is_u64(&self) -> bool;   // integer within [0, u64::MAX] (small +ints: both is_i64 and is_u64)
pub fn is_f64(&self) -> bool;   // true iff NOT is_i64 and NOT is_u64 (i.e. a non-integer)

pub fn as_i64(&self) -> Option<i64>;
pub fn as_u64(&self) -> Option<u64>;
pub fn as_f64(&self) -> Option<f64>;   // guaranteed Some when is_f64() is true; also works for ints
pub fn as_i128(&self) -> Option<i128>;
pub fn as_u128(&self) -> Option<u128>;

pub fn from_f64(f: f64) -> Option<Number>;   // None for NaN / ±Infinity (not JSON numbers)
pub fn from_i128(i: i128) -> Option<Number>; // None if out of range without arbitrary_precision
pub fn from_u128(i: u128) -> Option<Number>;
```

Values smaller than `i64::MIN` or larger than `u64::MAX` can only be represented when
`arbitrary_precision` is enabled. With that feature on, the `is_*` predicates describe
representability and `as_*` parse on demand (can return `None` for out-of-range
targets) while the `Number` still serializes exactly.

## `Map`

`serde_json::Map<String, Value>` is the type behind `Value::Object`. Backing store
depends on the feature: **`BTreeMap` by default (keys sorted)**, `IndexMap` with
`preserve_order` (insertion order). The public API is identical either way; only the
iteration/serialization order differs.

```rust
pub fn new() -> Self;
pub fn with_capacity(capacity: usize) -> Self;

pub fn get<Q>(&self, key: &Q) -> Option<&Value>
where String: Borrow<Q>, Q: ?Sized + Ord + Eq + Hash;
pub fn get_mut<Q>(&mut self, key: &Q) -> Option<&mut Value> /* same bounds */;
pub fn contains_key<Q>(&self, key: &Q) -> bool             /* same bounds */;
pub fn remove<Q>(&mut self, key: &Q) -> Option<Value>      /* same bounds */;

pub fn insert(&mut self, k: String, v: Value) -> Option<Value>;  // returns old value if present
pub fn entry<S>(&mut self, key: S) -> Entry<'_> where S: Into<String>;

pub fn keys(&self) -> Keys<'_>;
pub fn values(&self) -> Values<'_>;
pub fn iter(&self) -> Iter<'_>;
pub fn len(&self) -> usize;
pub fn is_empty(&self) -> bool;
```

The `Q` bounds carry `Ord + Eq + Hash` so the same signature compiles under both
backings; in practice you call with `&str`. Under `preserve_order`, `remove` is a
swap-remove that can reorder; `shift_remove`/`swap_remove` variants exist if removal
order matters.

## `Error`, `Category`, `Result`

```rust
pub type Result<T> = Result<T, Error>;   // single param; error type fixed to serde_json::Error
```

### `Error`

```rust
pub fn line(&self) -> usize;        // 1-based line of the failure
pub fn column(&self) -> usize;      // 1-based column
pub fn classify(&self) -> Category; // Io | Syntax | Data | Eof
pub fn is_io(&self) -> bool;
pub fn is_syntax(&self) -> bool;
pub fn is_data(&self) -> bool;
pub fn is_eof(&self) -> bool;
pub fn io_error_kind(&self) -> Option<std::io::ErrorKind>;  // std only; Some(..) iff is_io()
```

Implements `std::error::Error` (with `source()`), `Display` (message + line/column),
`serde::de::Error`, `serde::ser::Error`, and `From<Error> for io::Error`
(Syntax/Data → `InvalidData`, Eof → `UnexpectedEof`, Io → original kind). Wrap it in a
`thiserror` variant and log `line`/`column`/`classify` — see SKILL.md.

### `Category`

```rust
pub enum Category { Io, Syntax, Data, Eof }   // Clone, Copy, Debug, PartialEq, Eq
```

- `Io` — failed to read/write bytes on the underlying stream.
- `Syntax` — input was not syntactically valid JSON. (Often an HTML error page or a
  plaintext rate-limit body returned where you expected JSON.)
- `Data` — valid JSON but semantically wrong for the target type (missing field,
  number out of range, wrong variant). Usually struct/API drift on your side.
- `Eof` — input ended prematurely. For incremental stream readers, retry once more
  data is available (see `StreamDeserializer::byte_offset`).

Path: `serde_json::error::Category`, re-exported as `serde_json::Category` (use the
short path in code).

## `Deserializer` and `StreamDeserializer`

Drive these directly when the free functions aren't enough — rejecting trailing junk,
or reading several JSON values from one stream.

```rust
// Constructors
Deserializer::from_str(s: &str)        -> Deserializer<StrRead<'_>>
Deserializer::from_slice(bytes: &[u8]) -> Deserializer<SliceRead<'_>>
Deserializer::from_reader<R: Read>(r: R) -> Deserializer<IoRead<R>>   // std only

// After deserializing one value, assert the input is exhausted (only trailing
// whitespace left). This is how you reject "garbage after the JSON".
pub fn end(&mut self) -> Result<()>;

// Turn it into an iterator of sequential values.
pub fn into_iter<T>(self) -> StreamDeserializer<'de, R, T>
where R: Read<'de>, T: Deserialize<'de>;
```

```rust
pub struct StreamDeserializer<'de, R, T> { /* ... */ }
impl Iterator { type Item = Result<T, Error>; }

pub fn new(read: R) -> Self;
pub fn byte_offset(&self) -> usize;  // bytes consumed into successful Ts so far
```

`StreamDeserializer` reads a stream of **self-delineating** JSON values — objects,
arrays, strings, or whitespace-separated scalars concatenated together (NDJSON,
concatenated log JSON). On a `Category::Eof`, splice new bytes onto
`old[stream.byte_offset()..]` and retry. This is **not** for SSE: SSE has its own
`data:`/`event:` line framing you must strip first (SKILL.md, Decision 3).

```rust
let data = r#"{"k":3}1"cool""stuff" 3{}  [0,1,2]"#;
for value in Deserializer::from_str(data).into_iter::<Value>() {
    let value = value?;  // each concatenated JSON value, in turn
}
```

## The `ser` module: custom formatting

`std`-only. Reach here only to control output layout (custom indent, tabs); the free
functions cover the common cases.

```rust
// Serializer<W, F = CompactFormatter>
Serializer::new(writer: W) -> Self;                         // compact (default formatter)
Serializer::pretty(writer: W) -> Serializer<W, PrettyFormatter<'_>>;  // 2-space pretty
Serializer::with_formatter(writer: W, formatter: F) -> Self;          // custom F: Formatter
pub fn into_inner(self) -> W;

// trait Formatter      — implement for fully custom JSON layout (defaults provided)
// struct CompactFormatter — no extra whitespace (the default)
// struct PrettyFormatter<'a>
PrettyFormatter::new() -> Self;                  // two-space indent
PrettyFormatter::with_indent(indent: &[u8]) -> Self;   // e.g. b"\t" or b"    "
```

Tab-indented output:

```rust
let mut buf = Vec::new();
let fmt = serde_json::ser::PrettyFormatter::with_indent(b"\t");
let mut ser = serde_json::Serializer::with_formatter(&mut buf, fmt);
value.serialize(&mut ser)?;
```

## `RawValue`

`serde_json::value::RawValue` — a borrowed range of bytes holding one valid JSON value,
carried verbatim without parsing into a `Value`. Use it to forward a subtree unchanged
(an opaque provider blob, a node graph) or to defer parse cost.

```toml
serde_json = { version = "1", features = ["raw_value"] }   # OFF by default
```

```rust
pub const NULL:  &'static RawValue;
pub const TRUE:  &'static RawValue;
pub const FALSE: &'static RawValue;

pub fn from_string(json: String) -> Result<Box<RawValue>, Error>;  // validates one JSON value
pub fn get(&self) -> &str;                                         // the underlying JSON text
```

Two gotchas:
- **Feature-gated** (`raw_value`), off by default.
- **`!Sized`** — you only ever hold `&RawValue` or `Box<RawValue>`. Serializing one
  preserves its original formatting verbatim (no reformatting).

Borrowed vs owned in a struct:

```rust
#[derive(Deserialize)]
struct Borrowed<'a> {
    #[serde(borrow)]
    raw: &'a RawValue,        // works with from_str / from_slice (zero-copy)
}

#[derive(Deserialize)]
struct Owned {
    raw: Box<RawValue>,       // required with from_reader (no buffer to borrow from)
}
```

## Cargo features

8 flags, only `std` default.

| Feature | Default | Effect |
|---|---|---|
| `std` | yes | Standard library: the `to_*` / reader / writer functions and `std` paths. |
| `alloc` | no | `no_std` + allocator: `String`/`Vec`/`Value` without `std`. |
| `preserve_order` | no | Back `Map` with `indexmap` to keep key insertion order (else `BTreeMap`, sorted). Pulls in `indexmap`. |
| `arbitrary_precision` | no | Numbers held as decimal strings — arbitrary size/precision, exact round-trip. Interacts badly with `#[serde(flatten)]` / untagged enums. |
| `float_roundtrip` | no | Parse/emit f64 so values reparse to identical bits. Slight parse cost. |
| `raw_value` | no | Enables `RawValue` (verbatim JSON passthrough). |
| `unbounded_depth` | no | Removes the recursion-depth guard. **Never enable for untrusted input** — stack-overflow DoS. |

## JSON ↔ Rust type mapping

How serde maps JSON onto common Rust types (via the derive impls):

| JSON | Rust (deserialize target) | Notes |
|---|---|---|
| `null` | `Option::None`, `()` | A missing field also maps to `None` with `Option<T>`. |
| `true`/`false` | `bool` | |
| number (int) | `i8..=i64`, `u8..=u64`, `i128`/`u128` | Out-of-range → `Category::Data` error. |
| number (float) | `f32`, `f64` | `NaN`/`Infinity` are not valid JSON and can't be emitted. |
| string | `String`, `&str`, `char`, `Cow<str>` | `&str` borrows from the input (only with `from_str`/`from_slice`). |
| array | `Vec<T>`, `[T; N]`, tuples, `VecDeque<T>`, … | |
| object | structs, `HashMap<String, V>`, `BTreeMap<String, V>`, `Map<String, Value>` | Object keys must be strings; map key types must serialize to a string. |
| any | `serde_json::Value`, `Box<RawValue>` | The escape hatch for unknown shape. |

For the attributes that shape this mapping (`rename`, `rename_all`, `default`,
`skip_serializing_if`, `flatten`, `tag`/`untagged` enums), see [[pixhaus-serde]].
