# Modern Rust: const-context stabilizations (1.85-1.96)

What became callable in const context across the window, and where compile-time tables and constructors pay off in this codebase. Part of the `pixhaus-rust-modern` skill; start at its `SKILL.md` for the shortlist and the per-version cheat sheet.

The rule: when an operation becomes usable in const, move the work that never changes to compile time. IDs, palette entries, lookup tables, and layout math that the old code built once at startup can now be `const` items or `static`s with no runtime initialization path. The win is real but narrow: a `const` value is computed by the compiler, lives in read-only data, and carries a guarantee that it has no runtime cost and no init order to get wrong. Apply it to constructors and tables that are genuinely fixed; do not rewrite a working `OnceLock` cache just because a method went const this window.

The trend across the window is steady, not dramatic. Each release promotes another layer of "obvious" operations into const: first the layout and float primitives, then `str`/`Vec`/`String` accessors, then `Cell` and pointer ops, then slice rotations and float rounding. Nothing here is a headline feature. Taken together they mean a const fn can now do real work: validate a byte length, walk a slice, round a coordinate, build a fixed table. Below is what landed per version, then where it pays off in this codebase.

### Per-version: what became const-callable

- **1.85** — the layout and float groundwork. `mem::size_of_val`, `mem::align_of_val`, the whole `Layout` family (`for_value`, `align_to`, `pad_to_align`, `extend`, `array`), `mem::swap`, `ptr::swap`, `NonNull::new`, `MaybeUninit::write`, `HashMap::with_hasher` / `HashSet::with_hasher`, `BuildHasherDefault::new`, and the float math set `recip`, `to_degrees`, `to_radians`, `max`, `min`, `clamp`, `abs`, `signum`, `copysign`.
- **1.86** — `hint::black_box`, `io::Cursor::get_mut` / `set_position`, and the `str` splitting set: `is_char_boundary`, `split_at`, `split_at_checked`, `split_at_mut`, `split_at_mut_checked`.
- **1.87** — `str::from_utf8_mut`, `<[T]>::copy_from_slice`, the `SocketAddr*` setters (`set_ip`, `set_port`, `set_flowinfo`, `set_scope_id`), `char::is_digit`, `char::is_whitespace`, `<[[T; N]]>::as_flattened` / `as_flattened_mut`, and a large `String`/`Vec` accessor set: `String::{into_bytes, as_str, capacity, as_bytes, len, is_empty, as_mut_str, as_mut_vec}` and `Vec::{as_ptr, as_slice, capacity, len, is_empty, as_mut_slice, as_mut_ptr}`.
- **1.88** — `Cell::{replace, get, get_mut, from_mut, as_slice_of_cells}`, `NonNull::replace`, `<*mut T>::replace`, and `ptr::swap_nonoverlapping`.
- **1.89** — `<[T; N]>::as_mut_slice`, plus case-insensitive byte and string comparison: `<[u8]>::eq_ignore_ascii_case` and `str::eq_ignore_ascii_case`.
- **1.90** — `<[T]>::reverse` and float rounding for both `f32` and `f64`: `floor`, `ceil`, `trunc`, `fract`, `round`, `round_ties_even`.
- **1.91** — `<[T; N]>::each_ref` / `each_mut`, `OsString::new`, `PathBuf::new`, `TypeId::of`, and `ptr::with_exposed_provenance` / `with_exposed_provenance_mut`.
- **1.92** — `<[T]>::rotate_left` and `<[T]>::rotate_right`.
- **1.93** — no items in a dedicated const-stabilization list; the only const-context change was an internal const-eval one (copying pointers byte-by-byte during const evaluation).
- **1.94** — `f32::mul_add` and `f64::mul_add`.
- **1.95** — `fmt::from_fn`, `ControlFlow::is_break`, `ControlFlow::is_continue`.
- **1.96** — no const-stabilization list on the page.

### Const constructors for IDs and newtypes

The convention here is newtype wrappers for type safety. Once the wrapped operation is const, the wrapper's constructor can be `const fn`, so well-known ids become `const` items instead of values built at startup. `TypeId::of` going const (1.91) lets a type-keyed registry key be a `const`.

```rust
// OLD: a runtime constructor, so a "well-known" id is a lazily-built value
pub struct LayerId(u32);
impl LayerId {
    pub fn new(raw: u32) -> Self { Self(raw) }
}
// callers built sentinels at runtime
let background = LayerId::new(0);
```

```rust
// NEW: const constructor -> the sentinel is a compile-time constant in .rodata
pub struct LayerId(u32);
impl LayerId {
    pub const fn new(raw: u32) -> Self { Self(raw) }
}
pub const BACKGROUND: LayerId = LayerId::new(0);
```

### Palettes and lookup tables computed at compile time

A palette swatch or a small fixed color table is exactly the kind of data that should be `const`. With `<[u8]>::copy_from_slice` const (1.87) and the float rounding/`mul_add` set const (1.90, 1.94), a const fn can assemble RGBA entries and precompute derived tables — a sRGB-to-linear ramp, a gamma curve, a dither matrix — at compile time instead of filling a `Vec<u8>` on the first frame.

```rust
// OLD: built once at startup, then cached behind a OnceLock
fn srgb_to_linear_table() -> [f32; 256] {
    let mut t = [0.0; 256];
    for (i, slot) in t.iter_mut().enumerate() {
        let c = i as f32 / 255.0;
        *slot = if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) };
    }
    t
}
```

```rust
// NEW: a const lookup table, computed by the compiler, no runtime init path
// Uses const float arithmetic stabilized across the window (round/mul_add since 1.90/1.94).
const fn quantize_8(c: f32) -> u8 {
    // round became const in 1.90; mul_add in 1.94
    (c.mul_add(255.0, 0.5)).round() as u8
}
const GREY_RAMP: [u8; 4] = [
    quantize_8(0.0),
    quantize_8(0.25),
    quantize_8(0.5),
    quantize_8(1.0),
];
```

For a fixed palette, hold the bytes as a `const` directly. The buffer convention stays `Vec<u8>` with explicit stride for the mutable document; the const is the read-only source the document copies from.

```rust
// A 4-color fixed palette as RGBA bytes, no allocation, no startup cost.
const DB16_HEAD: [u8; 16] = [
    0x14, 0x0c, 0x1c, 0xff, // dark plum
    0x44, 0x24, 0x34, 0xff, // wine
    0x30, 0x34, 0x6d, 0xff, // navy
    0x4e, 0x4a, 0x4e, 0xff, // slate
];
```

These 16 bytes are a real `[u8; 16]` that compiles; the point is the table lives at compile time, not on the first frame.

### const in statics: layout and config without a startup phase

`const fn` is what lets a `static` hold a non-trivial value with no initializer running at load. The `Layout` family going const (1.85) means uniform-buffer and vertex sizing math can be a `static`; `HashMap::with_hasher` / `BuildHasherDefault::new` const (1.85) means a registry's empty map can be a `static` without a `lazy`/`OnceLock` wrapper when the hasher is the deterministic default.

```rust
// OLD: layout math deferred to runtime, stashed behind a OnceLock
static UNIFORM_LAYOUT: OnceLock<Layout> = OnceLock::new();
fn uniform_layout() -> Layout {
    *UNIFORM_LAYOUT.get_or_init(|| Layout::new::<CanvasUniforms>())
}
```

```rust
// NEW: the layout is a const, no lazy init, no lock
// Layout::array / for_value became const in 1.85.
const UNIFORM_LAYOUT: Layout = Layout::new::<CanvasUniforms>();
```

### Where this helps in this codebase, and where it does not

Reach for const here when the value is genuinely fixed: built-in `LayerId`/`FrameId` sentinels, the default palette, a gamma or dither table, vertex/uniform layout sizes, and ASCII-keyed lookups now that `eq_ignore_ascii_case` is const (1.89). These move off the first-frame path entirely and gain the "no runtime cost, no init order" guarantee. const-callable `str` validation (`is_char_boundary`, `split_at_checked` since 1.86; `from_utf8_mut` since 1.87) lets a parser const-check fixed format tags.

Do not convert state that is actually loaded, computed from a document, or shared across threads. The undo stack, the active document's pixel buffers, anything keyed on a project that opens at runtime — none of that is const, and a `OnceLock` or owned field is still the right tool. const promotion is for the fixed tables and constructors, not for the live model. And do not churn a working `OnceLock` table into a const just to use the feature: the payoff is a value that was always fixed, not a rewrite of one that initializes fine today.

One caveat on the const-as-pattern boundary, since it bites adjacent to this work: across the window the compiler tightened which consts may appear in `match` patterns. 1.90 rejects a const that references mutable or external memory in pattern position, and 1.96 fixed a 1.94 regression so a `const` of type `ManuallyDrop<T>` is usable as a pattern again. If a `const` palette entry or id is matched on rather than compared, keep it a plain value type and watch for these.
