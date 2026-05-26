---
name: pixhaus-serde
description: >
  Use for the serde core in Pixhaus — deriving Serialize/Deserialize, choosing
  #[serde(...)] attributes (rename, default, skip, flatten, alias, with, tag,
  untagged), the four enum representations, hand-writing a custom Serialize or
  Deserialize impl (the Visitor pattern), the serde data model, the 'de lifetime /
  borrowing / DeserializeOwned, and building or parsing JSON for the AI backends with
  serde_json. Trigger this for ANY "derive serde on this type", "what serde attribute
  do I use", "this field should be optional / renamed / skipped", "round-trip this
  struct", "write a custom Deserialize", "parse the API response", or "make this enum
  serialize as X" request even when the user doesn't say "serde". serde's attribute
  names are precise and its binary-vs-JSON behavior is unintuitive, so reach for this
  skill rather than guessing. For the on-disk format specifically — MessagePack encoding,
  serde_bytes for pixel buffers, the named-map decision — use pixhaus-rmp-serde; for the
  compression wrapping it, pixhaus-zstd.
---

# serde for Pixhaus

serde is a compile-time serialization framework: your types implement `Serialize`/
`Deserialize` (almost always via `#[derive]`), data formats implement `Serializer`/
`Deserializer`, and the two meet through a fixed 29-type data model. No runtime
reflection — the format and the type are wired together by the compiler and usually
optimize away.

This skill is the serde *core* — the part every format shares: the derive macros, the
`#[serde(...)]` attribute vocabulary, the data model, and custom impls. The format-specific
skills build on it:

- **`pixhaus-rmp-serde`** — the `.pixhaus` project file (MessagePack): the named-map
  decision, `serde_bytes` for pixel buffers, the file pipeline. Pair with it for anything
  that persists to disk.
- **`pixhaus-serde-json`** — JSON for the AI backend adapters (Anthropic, OpenAI, etc.):
  `Value`, `json!`, request/response bodies, the `from_reader`/indexing traps.
- **`pixhaus-zstd`** — the compression wrapping the MessagePack bytes.

Reach into those when you're touching a specific format. Stay here for "how do I express
this shape in serde" — the derive, attributes, and custom impls that all three share.

When you need the full API surface for an area, open the matching file in `references/`.
Don't guess attribute names or trait signatures from memory — the names are precise and
the dynamic features (`flatten`, `untagged`) have format-dependent traps. The references
were derived from serde.rs and docs.rs for serde 1.0.228 / serde_json 1.0.150.

## Versions

Pinned in the workspace `Cargo.toml` `[workspace.dependencies]`; crates inherit with
`serde = { workspace = true }`.

| Crate | Version | Role |
|---|---|---|
| `serde` | `"1"` (feature `derive`) | The framework + derive macros |
| `serde_json` | `"1"` | JSON for AI backend HTTP |

serde has been API-stable across all of 1.x. The `derive` feature is non-negotiable here —
use it rather than adding `serde_derive` as a separate dependency, so the versions can't
drift apart.

## The mental model

Three facts drive almost every correct decision.

1. **Derive first; hand-write almost never.** `#[derive(Serialize, Deserialize)]` plus
   attributes covers the overwhelming majority of types. Reach for a manual `impl` only
   when the wire shape genuinely can't be expressed with attributes — a foreign invariant,
   a "string or full object" field, a packed layout. A hand-written impl is more code to
   keep correct across both directions and every format. When you do need one, follow
   `references/custom-impls.md` exactly rather than improvising the Visitor; the default
   `visit_*` methods error, so a partial impl fails at runtime, not compile time.

2. **The format decides the rules, and binary is the strict one.** JSON (`serde_json`,
   `is_human_readable() == true`) is forgiving: self-describing, carries field names and
   type tags, tolerates the dynamic tricks. MessagePack (`rmp-serde`,
   `is_human_readable() == false`) is not. The features that buffer-then-reinspect —
   `#[serde(flatten)]`, `#[serde(untagged)]`, internally-tagged `#[serde(tag = "...")]` —
   are fragile or broken on binary formats. If a type round-trips in JSON but fails only
   in the project file with "invalid type" or "missing field", suspect one of those three.
   Persisted types stay plain; see `pixhaus-rmp-serde` for the on-disk consequences.

3. **Borrowing is tied to the input's lifetime.** A `Deserialize<'de>` type can borrow
   `&str`/`&[u8]` straight out of the input buffer (zero-copy) — but only when the
   deserializer reads from an in-memory slice (`from_slice`, `from_str`), never from a
   streaming reader. Anything that must outlive the input, or cross a channel back to the
   egui loop from a background task, must be owned (`DeserializeOwned` — no borrowed
   fields). The Pixhaus async model returns task results over channels the frame loop
   drains, so values handed across a channel own their data. Detail in
   `references/data-model-and-traits.md`.

## Deriving: the everyday patterns

Most types are a clean derive. The attributes you reach for constantly:

```rust
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]      // blendMode, opacityPct on the wire
struct Layer {
    blend_mode: BlendMode,
    opacity_pct: u8,

    // Optional + omitted-when-absent is TWO attributes. default fills None on the way in;
    // skip_serializing_if drops the key on the way out. Use both for a symmetric round-trip.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    note: Option<String>,
}
```

- `#[serde(rename = "x")]` / `#[serde(rename_all = "camelCase")]` — wire names. The
  accepted `rename_all` values are exact strings: `"lowercase"`, `"UPPERCASE"`,
  `"PascalCase"`, `"camelCase"`, `"snake_case"`, `"SCREAMING_SNAKE_CASE"`, `"kebab-case"`,
  `"SCREAMING-KEBAB-CASE"`.
- `#[serde(default)]` / `#[serde(default = "path")]` — fill a missing field instead of
  erroring. The bedrock of schema evolution.
- `#[serde(alias = "old")]` — also accept an old name on read; the way to rename a field
  without breaking saved files.
- `#[serde(skip)]` / `skip_serializing` / `skip_deserializing` — drop a field from one or
  both directions.
- `#[serde(with = "module")]` — route a field through a custom `serialize`/`deserialize`
  pair (keep them together so they can't disagree on the wire shape).

The full container/variant/field attribute reference, and the four enum representations
with worked examples and their limits, is `references/derive-attributes.md`.

## Enums: pick the representation deliberately

serde offers four ways to encode an enum, and the default is the safe one:

- **Externally tagged (default)** — `{ "Variant": payload }`. Works in every format and
  for every variant kind. This is what persisted Pixhaus enums should use.
- **Internally tagged** `#[serde(tag = "type")]` — `{ "type": "Variant", ...fields }`.
  Struct/unit variants only; buffers on read.
- **Adjacently tagged** `#[serde(tag = "t", content = "c")]` — `{ "t": "Variant", "c": ... }`.
- **Untagged** `#[serde(untagged)]` — bare content; tries variants in declaration order and
  takes the first that deserializes. Overlapping shapes silently pick the earlier variant.

The last three buffer through a self-describing intermediate, so they're for JSON, not the
binary project file. In persisted enums, also treat variant *names* as part of the format —
pin them with `#[serde(rename = "...")]` if you rename the Rust identifier. Details and
example output per representation in `references/derive-attributes.md`.

## JSON for the AI backends

JSON (the AI adapter wire format) has its own skill: **`pixhaus-serde-json`**, covering
`serde_json` — `Value`, `json!`, request/response bodies, the `from_reader`/indexing traps.
The serde-core knowledge here applies (the same derives and attributes drive JSON), but go
there for the `serde_json` API itself. The one cross-cutting fact: `serde_json` is
human-readable (`is_human_readable() == true`), the opposite of the project file's
MessagePack — which is why a type can encode differently in each (see
`references/data-model-and-traits.md`).

## Rules that prevent the recurring bugs

- **Optional-and-omitted is two attributes.** `#[serde(default, skip_serializing_if =
  "Option::is_none")]` — `default` on read, `skip_serializing_if` on write. One without
  the other is an asymmetric round-trip.
- **A `serialize_with`/`deserialize_with` pair must agree on the wire shape.** Mismatched
  custom functions are a silent round-trip break. Prefer `#[serde(with = "module")]` so
  the two live together. Signatures are in `references/custom-impls.md`.
- **Don't enable the `rc` feature to persist `Arc`/`Rc`.** serde's `rc` feature does *not*
  preserve sharing — two `Arc`s to the same data serialize as two copies and deserialize
  as two distinct allocations. If a persisted type holds an `Arc`, fix the model (single
  owner — see CLAUDE.md), don't reach for `rc`.
- **A partial custom `Deserialize` fails at runtime, not compile time.** The `Visitor`
  trait's `visit_*` methods default to returning a type error, so a hand-written impl that
  forgets `visit_map` (or `visit_seq`) compiles fine and then errors only when that input
  shape arrives. Implement every shape your format can hand you.
- **`from_reader` needs `DeserializeOwned`; `from_slice`/`from_str` can borrow.** Want
  zero-copy borrowed fields? Read from an in-memory slice and keep the buffer alive. Value
  crosses a thread or channel? It must own its data.
- **Test the round-trip, not just one direction.** A `proptest` asserting
  `from_slice(to_vec(x)) == x` catches attribute mistakes the type system can't. See
  pixhaus-testing-conventions for the harness.

## References

Open the file for the area you're working in; each is a dense API reference for the pinned
versions.

| File | Covers |
|---|---|
| `references/data-model-and-traits.md` | The 29-type data model, `Serialize`/`Deserialize`/`Serializer`/`Deserializer`, the `'de` lifetime + zero-copy, `DeserializeOwned`, `is_human_readable()` |
| `references/derive-attributes.md` | Every `#[serde(...)]` container / variant / field attribute, `rename_all` values, the four enum representations with examples and limits |
| `references/custom-impls.md` | Hand-writing `Serialize`/`Deserialize`, the Visitor pattern, `SeqAccess`/`MapAccess`/`DeserializeSeed`, error construction, string-or-struct, `*_with` signatures |

Format-specific skills build on this core: `pixhaus-rmp-serde` (the `.pixhaus` file —
MessagePack, `serde_bytes`, named maps, schema evolution), `pixhaus-serde-json` (JSON for
the AI backends), and `pixhaus-zstd` (compression). See also [[project-v2-native-restart]]
for why the project format is MessagePack + zstd at all.

A standing caution: the references record the documented API faithfully, but a few deep
signatures were flagged during research as reconstructed-from-convention rather than quoted
verbatim (noted inline). When one is load-bearing, confirm it against
https://docs.rs/serde/latest/ or the source before depending on it.
