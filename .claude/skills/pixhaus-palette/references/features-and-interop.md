# palette 0.7.6 — features and interop

Source of truth: the crate's `Cargo.toml` at tag 0.7.6 (verified verbatim), cross-checked
against the docs.rs features tab.

## Cargo features

Verbatim `[features]` block:

```toml
default        = ["named_from_str", "std", "approx"]
named_from_str = ["named", "phf"]
named          = []
random         = ["rand"]
serializing    = ["serde", "std"]
find-crate     = ["palette_derive/find-crate"]
std            = ["alloc", "approx?/std"]
alloc          = []
```

Optional-dep features (each turns on the same-named crate): `bytemuck`, `wide`, `libm`,
`serde`, `rand`, `approx`, `phf`.

| Feature | Default? | Enables |
|---|---|---|
| `std` | **yes** | std library; pulls `alloc`. Turn OFF for no_std. |
| `alloc` | yes (via std) | allocating types (`Vec`, `Box`) without full std. |
| `approx` | **yes** | approximate float comparison on colors (`approx ^0.5`). |
| `named` | yes (via named_from_str) | CSS color-name constants in the `named` module. |
| `named_from_str` | **yes** | `named::from_str` string→color lookup; pulls `named` + `phf`. |
| `serializing` | no | **the serde feature you want** — `serde ^1` + `std`; adds `Serialize`/`Deserialize`. |
| `serde` | no | raw optional `serde` dep. Prefer `serializing` (the documented public switch). |
| `bytemuck` | no | `Pod`/`Zeroable` impls for casting colors to/from bytes. |
| `wide` | no | `wide ^0.7.3` SIMD element types (batch processing). |
| `libm` | no | software float math (`libm ^0.2.1`). **Required for no_std.** |
| `random` | no | `rand ^0.8` random color generation. |
| `find-crate` | no | lets the derive macros find `palette` when renamed in Cargo.toml. |

**Default set:** `named_from_str`, `std`, `approx` (transitively `named`, `phf`,
`alloc`). So named colors and string lookup come for free — don't add them.

**no_std recipe:** `default-features = false`, then add `libm` (and `alloc` for Vec/Box).
serde is unavailable without std — `serializing` forces `std`.

## MSRV and license

- `rust-version = "1.60.0"` (declared floor; CI also tests 1.71.0). Edition 2018.
- `license = "MIT OR Apache-2.0"` — clears the [[project-v2-native-restart]] MIT lock,
  no copyleft. `cargo deny` will pass it.

## Interop for the wgpu pixel editor

**`bytemuck`** — with this feature, color types get `Pod`/`Zeroable`, so `Srgb<u8>`,
`Srgba<u8>`, `LinSrgb<f32>`, `LinSrgba<f32>` cast to `&[u8]` for `queue.write_buffer` or
a texture upload. See [[pixhaus-bytemuck]]. Caveat: `Pod` requires no padding; the packed
layouts above are safe casting targets, but `LinSrgb<f32>` is 12 bytes and won't match a
16-byte-aligned GPU `vec4` — pad/align on your side, exactly the std140/std430 trap from
[[pixhaus-glam]]. Validate a chosen type with a `bytemuck::cast_slice` compile check;
treat the per-type `Pod` list as confirmed only for the common RGB(A) forms. `bytemuck`
does not pull in `std`.

**`serializing`** — `Serialize`/`Deserialize` on color types, so a swatch or palette
drops into the `.pixhaus` MessagePack file via [[pixhaus-rmp-serde]]. Store as
`Srgb<u8>` for compactness. Forces `std` on.

**`wide`** — a color's component type can be a `wide` SIMD vector instead of scalar
`f32`; palette's color math is generic over the `num` traits, so the same ops run
lane-parallel. Opt-in, independent of bytemuck/serde, and the path for batching many
pixels per instruction over an 8K canvas ([[project-8k-perf-constraint]]).

## Recommended Cargo.toml

```toml
palette = { version = "0.7.6", features = ["serializing", "bytemuck"] }
```

`named`/`named_from_str`/`std`/`approx` are already default — don't list them. Add `wide`
only when you do SIMD pixel batching; add the separate `enterpolation` crate for
multi-stop gradients (see `gradients-and-named.md`). For no_std: `default-features =
false, features = ["libm", "alloc", "bytemuck"]` (and drop `serializing`).

## Verification notes

Feature block and MSRV/license are verified verbatim against Cargo.toml. The exact
per-color-type `Pod`/`Zeroable` impl list was not enumerated from the fetched docs —
confirmed conceptually (the feature adds the impls) but validate your specific type with
a compile-time cast. Use 1.60.0 as the contractual MSRV floor.
