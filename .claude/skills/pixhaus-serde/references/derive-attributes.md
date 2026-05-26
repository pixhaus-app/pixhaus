# serde derive attribute reference

Every `#[serde(...)]` attribute, plus the four enum representations. Verified against
serde.rs for serde 1.0.228. Enable with `serde = { version = "1", features = ["derive"] }`
and `#[derive(Serialize, Deserialize)]`.

Three scopes: container (on the `struct`/`enum`), variant (on an enum variant), field (on
a struct or variant field). Attributes stack. Every direction-split attribute (`rename`,
`rename_all`, `rename_all_fields`, `bound`) also takes a
`(serialize = "...", deserialize = "...")` form to set the two directions independently.

## Table of contents

- [Container attributes](#container-attributes)
- [Variant attributes](#variant-attributes)
- [Field attributes](#field-attributes)
- [`*_with` function signatures](#with-function-signatures)
- [The four enum representations](#the-four-enum-representations)
- [Common combinations](#common-combinations)

## Container attributes

On a `struct` or `enum`.

| Attribute | Effect |
|---|---|
| `rename = "name"` | Serialize/deserialize the container under this name. |
| `rename_all = "convention"` | Rename all fields/variants by a case convention (values below). |
| `rename_all_fields = "convention"` | Apply a convention to the fields of all struct variants of an enum. |
| `deny_unknown_fields` | Error on any unknown field. **Do not put on persisted types** — it blocks forward compatibility. Incompatible with `flatten`. |
| `tag = "type"` | Internally tagged enum representation. |
| `tag = "t", content = "c"` | Adjacently tagged enum representation. |
| `untagged` | Untagged enum representation. |
| `bound = "T: Trait"` | Override the generated `where` bounds. |
| `default` | Missing fields come from the type's `Default`. |
| `default = "path"` | Missing fields come from `path()` (`fn() -> Self`). |
| `remote = "Type"` | Derive ser/de for a type defined in another crate. |
| `transparent` | Serialize a 1-field struct/newtype as just the inner field. |
| `from = "FromType"` | Deserialize `FromType`, then `From::from`. |
| `try_from = "FromType"` | Deserialize `FromType`, then `TryFrom::try_from` (fallible). |
| `into = "IntoType"` | Serialize by cloning `Into<IntoType>` (needs `Clone`). |
| `crate = "path"` | Path to the serde crate (for macro re-exports). |
| `expecting = "..."` | Custom "expected" text in error messages. |
| `variant_identifier` / `field_identifier` | Derive a deserializer reading variants/fields by string or integer tag. |

**`rename_all` accepted values (exact strings):** `"lowercase"`, `"UPPERCASE"`,
`"PascalCase"`, `"camelCase"`, `"snake_case"`, `"SCREAMING_SNAKE_CASE"`, `"kebab-case"`,
`"SCREAMING-KEBAB-CASE"`. Same set for `rename_all_fields` and variant-level `rename_all`.

## Variant attributes

On an enum variant.

| Attribute | Effect |
|---|---|
| `rename = "name"` | Use this name for the variant. In a persisted enum, variant names are part of the format — pin them with this if you rename the Rust identifier. |
| `alias = "name"` | Also accept this name on deserialize. Repeatable (stack multiple). |
| `rename_all = "convention"` | Case convention for this struct variant's fields. |
| `skip` / `skip_serializing` / `skip_deserializing` | Never (de)serialize this variant. Serializing a `skip_serializing` variant is an error. |
| `serialize_with` / `deserialize_with` / `with` | Custom (de)serialization for the variant. |
| `bound = "..."` | Override bounds for this variant's generated code. |
| `borrow` | Borrow data from the deserializer (zero-copy) for a variant field. |
| `other` | Catch-all variant for internally/adjacently tagged enums when the tag matches nothing else. Must be a unit variant. |
| `untagged` | Treat just this variant as untagged. |

## Field attributes

On a struct field or enum-variant field.

| Attribute | Effect |
|---|---|
| `rename = "name"` | Alternate wire name. |
| `alias = "name"` | Also accept this name on deserialize. Repeatable. The way to rename a field without breaking saved files. |
| `default` | `Default::default()` if the field is missing. The bedrock of schema evolution. |
| `default = "path"` | Call `path()` (`fn() -> T`) for the default. |
| `flatten` | Inline this field's keys into the parent map. For nested structs or a trailing map that captures extra keys. **Buffers through a map — breaks on non-self-describing binary formats and is incompatible with `deny_unknown_fields`.** |
| `skip` / `skip_serializing` / `skip_deserializing` | Drop from both / one direction. `skip` uses `Default` on read. |
| `skip_serializing_if = "path"` | Skip serializing when `path(&field) == true`. Common: `"Option::is_none"`, `"Vec::is_empty"`, `"str::is_empty"`. |
| `serialize_with = "path"` / `deserialize_with = "path"` | Custom functions for this field (signatures below). |
| `with = "module"` | Shorthand for `module::serialize` + `module::deserialize`. Keeps the pair together so they can't disagree. |
| `borrow` / `borrow = "'a + 'b"` | Borrow this field's data zero-copy (for `&str`/`&[u8]`/`Cow`). |
| `bound = "..."` | Override bounds for this field. |
| `getter = "..."` | Accessor for a private field when deriving a remote type. |

## `*_with` function signatures

The exact shapes the named functions must have. (These are the established serde contract;
the dedicated docs page links subtopics rather than quoting them verbatim — confirm against
an example if a compile error is cryptic.)

```rust
// #[serde(serialize_with = "path")] — called as path(&field, serializer)
fn serialize<S>(value: &T, serializer: S) -> Result<S::Ok, S::Error>
where S: Serializer;

// #[serde(deserialize_with = "path")]
fn deserialize<'de, D>(deserializer: D) -> Result<T, D::Error>
where D: Deserializer<'de>;
```

`#[serde(with = "module")]` expects a module exposing both `serialize` and `deserialize`
with exactly those signatures. (`serde_bytes` is the canonical example — it's a module you
point `with` at for `Vec<u8>` fields; see pixhaus-rmp-serde.)

## The four enum representations

Given `enum Msg { Request { id: String, method: String }, Ping }`:

### Externally tagged (default, no attribute)

```json
{"Request": {"id": "1", "method": "m"}}
"Ping"
```

The variant name wraps the content. Works in **every** format and for **every** variant
kind. This is what persisted Pixhaus enums should use — the only representation that's safe
on MessagePack.

### Internally tagged — `#[serde(tag = "type")]`

```json
{"type": "Request", "id": "1", "method": "m"}
{"type": "Ping"}
```

Struct, unit, and newtype-wrapping-a-struct variants only — **not** tuple variants, and a
newtype's inner value must serialize as a map. Buffers content on read (needs `alloc`).
Fragile on binary formats.

### Adjacently tagged — `#[serde(tag = "t", content = "c")]`

```json
{"t": "Request", "c": {"id": "1", "method": "m"}}
{"t": "Ping", "c": null}
```

Works for all variant kinds (content can be any shape). Buffers on read.

### Untagged — `#[serde(untagged)]`

```json
{"id": "1", "method": "m"}
```

No tag; content is emitted bare. On deserialize, serde tries each variant **in declaration
order** and takes the **first that succeeds** — overlapping shapes silently pick the earlier
variant, a quiet correctness trap. Buffers on read.

**The last three are for JSON, not the project file.** They rely on buffering into a
self-describing intermediate and re-inspecting it; non-self-describing/binary formats can't
disambiguate, and number widths and variant choice can't be recovered. On MessagePack, use
externally tagged. (See pixhaus-rmp-serde.)

## Common combinations

**Optional, omitted when absent** — two attributes, symmetric round-trip:

```rust
#[serde(default, skip_serializing_if = "Option::is_none")]
note: Option<String>,
```

**JSON-style naming for a whole struct:**

```rust
#[serde(rename_all = "camelCase")]
struct Layer { blend_mode: BlendMode, opacity_pct: u8 }   // blendMode, opacityPct
```

**Capture arbitrary extra keys** (no `deny_unknown_fields`):

```rust
struct Document {
    #[serde(flatten)]
    extra: HashMap<String, serde_json::Value>,   // JSON only — flatten breaks on MessagePack
}
```

**Added field, old files still load:**

```rust
struct ProjectV2 {
    canvas: Canvas,
    #[serde(default)]      // older files lack this; decode as Default
    onion_skin: OnionSkin,
}
```

**Renamed field, old files still load:**

```rust
#[serde(alias = "opacity")]   // new name is the field ident; "opacity" was the old wire name
opacity_pct: u8,
```
