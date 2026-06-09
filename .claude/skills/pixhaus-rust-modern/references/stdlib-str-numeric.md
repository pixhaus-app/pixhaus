# Modern Rust: str, char, integer, float, and formatting APIs (1.85-1.96)

Newly stabilized scalar-and-text APIs — the strict_* and *_sub_signed integer families, midpoint, const float math, char/str helpers, and const formatting. Part of the `pixhaus-rust-modern` skill; start at its `SKILL.md` for the shortlist and the per-version cheat sheet.

This is the reference of record for the scalar-and-text corner of std that opened up between Rust 1.85 and 1.96. The rule: the toolchain pins 1.96 / edition 2024, so every API below is callable today. Reach for them when they delete code you'd otherwise hand-write — a midpoint that can't overflow, a strict op that panics instead of silently wrapping, a const formatter. Do not go rewrite working `(a + b) / 2` arithmetic or a working `format!` just to plant a new name; the win is correctness and const-ness at points where the old code was fragile, not novelty.

Const-ness is called out per item because it decides whether a value can move into a `const` palette table, a compile-time stride calculation, or a `const fn` on `pixhaus-core`. When something became usable in const context in a later release than it stabilized, both versions are noted.

### Integers

The strict-arithmetic family (1.91) is the one to internalize. A strict op panics on overflow in every build profile, where the plain operator only panics in debug and wraps in release. For a pixel-stride or buffer-length computation that must never silently wrap into a tiny allocation, that release-mode wrap is a latent corruption bug; `strict_*` turns it into a loud panic everywhere.

```rust
// OLD: silent wrap in release, debug-only panic. A bad stride sails through.
let stride = width * bytes_per_pixel;          // u32 * u32

// NEW (1.91): panics on overflow in every profile.
let stride = width.strict_mul(bytes_per_pixel); // {integer}::strict_mul
```

The full strict set (1.91): `strict_add`, `strict_sub`, `strict_mul`, `strict_div`, `strict_div_euclid`, `strict_rem`, `strict_rem_euclid`, `strict_neg`, `strict_shl`, `strict_shr`, `strict_pow`, plus the mixed-sign forms `i{N}::strict_add_unsigned`, `i{N}::strict_sub_unsigned`, `i{N}::strict_abs`, `u{N}::strict_add_signed`, and `u{N}::strict_sub_signed`. When you instead want to *detect* overflow rather than abort, `u{N}::checked_signed_diff` (1.91) gives the signed gap between two unsigned values, `None` on overflow — exact for "how far did the cursor move", which can go negative.

Midpoint without the overflow trap landed in stages: unsigned `{integer}::midpoint` and `NonZeroU*::midpoint` at 1.85, then signed `<iN>::midpoint` at 1.87. Use it for the center of two coordinates or a binary-search pivot.

```rust
// OLD: (a + b) can overflow before the divide.
let mid = (a + b) / 2;

// NEW: no intermediate overflow. 1.85 unsigned, 1.87 signed.
let mid = a.midpoint(b);
```

Bit-reinterpretation got first-class names at 1.87: `<uN>::cast_signed` and `<iN>::cast_unsigned` (and the `NonZero` forms) flip an integer's interpretation without the `as` cast that reads as a possibly-lossy conversion at a glance. Also at 1.87: `<uN>::is_multiple_of` ("is this width a multiple of the tile size") and the saturating-shift pair `unbounded_shl` / `unbounded_shr` (signed and unsigned), which return 0 (or -1 for a signed arithmetic right shift) when the shift amount meets or exceeds the bit width instead of panicking or wrapping the amount.

For subtracting a signed delta from an unsigned base — clamping a `u32` coordinate by a signed pan offset — the `*_sub_signed` family arrived at 1.90: `u{n}::checked_sub_signed`, `overflowing_sub_signed`, `saturating_sub_signed`, `wrapping_sub_signed`. `saturating_sub_signed` is the one you usually want at a canvas edge.

```rust
// NEW (1.90): clamp at 0 instead of underflowing the coordinate.
let x = origin_x.saturating_sub_signed(pan_dx); // u32, i32 delta
```

Wide-integer building blocks for big-number or hash work came at 1.91: `u{N}::carrying_add`, `borrowing_sub`, `carrying_mul`, and `carrying_mul_add`. Two raw unchecked shifts/negation landed at 1.93 — `<iN>::unchecked_neg`, and `unchecked_shl` / `unchecked_shr` on both signedness — all `unsafe` (you promise the shift amount is in range); the workspace forbids `unsafe`, so these stay off-limits unless a future exception is granted, and `strict_*` or `unbounded_*` covers the same ground safely.

`NonZero::count_ones` (1.86) counts set bits and returns a `NonZero<u32>`, so the result type carries the "at least one bit set" fact forward.

`NonZero::<u{N}>::div_ceil` (1.92) does ceiling division on `NonZero` unsigned integers and returns a `NonZero`, so a "how many tiles of size T cover N pixels" computation keeps the nonzero guarantee end to end instead of dividing, adding one, and re-wrapping.

`bool: TryFrom<{integer}>` (1.95) converts an integer to a `bool` fallibly — `0` to `false`, `1` to `true`, anything else an `Err`. Reach for it decoding a packed flag byte out of a `.phx` file or a settings field, where `n != 0` silently accepts a stray `2` that should have been rejected.

### Floats

`{float}::midpoint` (1.85) is the float twin of the integer midpoint — center of two positions without the overflow-prone sum. It is const as of 1.85.

Float neighbor-stepping arrived at 1.86: `f32::next_up` / `f64::next_up` and `next_down` give the next representable value toward +inf or -inf. Use them when nudging a zoom factor or an epsilon by the smallest possible step.

```rust
// NEW (1.86): the smallest representable increment, no magic epsilon.
let nudged = zoom.next_up();
```

A wave of float methods became `const` even though the methods themselves were already stable. At 1.85: `recip`, `to_degrees`, `to_radians`, `max`, `min`, `clamp`, `abs`, `signum`, `copysign`. At 1.90 the rounding family went const: `floor`, `ceil`, `trunc`, `fract`, `round`, `round_ties_even` (all on `f32` and `f64`). At 1.94, `f32::mul_add` / `f64::mul_add` became const. That matters here: a `const` color-space or DPI-scaling table that needs `to_radians` or `round` can now be computed at compile time instead of lazily at first use.

```rust
// NEW: round/clamp/to_radians usable in const context.
//   to_radians const since 1.85, round const since 1.90.
const QUARTER_TURN_RAD: f32 = 90.0_f32.to_radians();
```

New float constants at 1.94: `f32::consts::EULER_GAMMA` / `f64::consts::EULER_GAMMA`, and `f32::consts::GOLDEN_RATIO` / `f64::consts::GOLDEN_RATIO`. Reach for `GOLDEN_RATIO` if a procedural palette or layout wants the golden angle; don't paste a hand-typed `1.618...` literal next to it.

### char

`NonZero<char>` (1.89) lets a `char` field carry the "not NUL" guarantee in its type, the same way `NonZeroU32` does for integers — useful for a niche-optimized key or token where NUL is meaningless.

Two encoding-length constants landed at 1.93: `char::MAX_LEN_UTF8` (max bytes a char takes in UTF-8) and `char::MAX_LEN_UTF16` (max u16 code units). Size a fixed scratch buffer for encoding one char from the constant instead of a hardcoded `4`.

```rust
// NEW (1.93): name the bound instead of writing 4.
let mut buf = [0u8; char::MAX_LEN_UTF8];
let s = ch.encode_utf8(&mut buf);
```

`impl TryFrom<char> for usize` (1.94) gives a fallible char-to-`usize` via the scalar value — cleaner than `c as u32 as usize` when indexing a glyph table. Two `char` predicates became const at 1.87: `char::is_digit` and `char::is_whitespace`, so a compile-time character-class check is now possible.

### str and String

The biggest str change is inherent UTF-8 constructors (1.87): `<str>::from_utf8`, `from_utf8_mut`, `from_utf8_unchecked`, `from_utf8_unchecked_mut`. Call them as `str::from_utf8(bytes)` instead of routing through `std::str::from_utf8` — same validation, method-position syntax. `core::str::from_utf8_mut` is also const as of 1.87.

```rust
// OLD: free function in a different module.
let s = std::str::from_utf8(&bytes)?;

// NEW (1.87): inherent associated fn on str.
let s = str::from_utf8(&bytes)?;
```

`impl TryFrom<Vec<u8>> for String` (1.87) is the owned, fallible counterpart — consume a byte vector into a `String`, getting the bytes back in the error on invalid UTF-8, without `String::from_utf8` ceremony at the call site.

Char-boundary helpers (1.91): `str::ceil_char_boundary` and `str::floor_char_boundary` round a byte index up or down to the nearest valid char boundary. This is the right tool for truncating a prompt or a layer name to a byte budget without splitting a multi-byte character and panicking.

```rust
// NEW (1.91): clamp a byte index to a safe split point.
let end = name.floor_char_boundary(max_bytes);
let shown = &name[..end];
```

`String::extend_from_within` (1.87) appends a copy of one of the string's own byte ranges onto its end, no temporary allocation. `String::into_raw_parts` (1.93) decomposes a `String` into pointer, length, capacity for FFI or a custom buffer hand-off — the inverse rebuild is `unsafe`, so this is a boundary-only tool given the no-`unsafe` rule.

A run of `String` accessors became const at 1.87: `len`, `is_empty`, `capacity`, `as_str`, `as_bytes`, `as_mut_str`, `as_mut_vec`, `into_bytes`. And `<[u8]>::eq_ignore_ascii_case` plus `str::eq_ignore_ascii_case` became const at 1.89 — a compile-time case-insensitive compare of a format tag or extension is now in reach.

Several `str` slicing methods went const at 1.86: `is_char_boundary`, `split_at`, `split_at_checked`, `split_at_mut`, `split_at_mut_checked`.

### Formatting

`std::fmt::from_fn` (1.93) builds a `Display`/`Debug` value from a closure that writes straight to the `Formatter`, with `std::fmt::FromFn` as the returned type. Use it for a throwaway adapter — formatting a `Vec<u8>` pixel run as hex, or a layer summary — without declaring a newtype and a trait impl just to get one `{}`.

```rust
// OLD: a newtype plus an impl for a one-off Display.
struct Hex<'a>(&'a [u8]);
impl std::fmt::Display for Hex<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for b in self.0 { write!(f, "{b:02x}")?; }
        Ok(())
    }
}
println!("{}", Hex(&pixels));

// NEW (1.93): inline closure, no type to name.
let hex = std::fmt::from_fn(|f| {
    pixels.iter().try_for_each(|b| write!(f, "{b:02x}"))
});
println!("{hex}");
```

`std::fmt::from_fn` is also usable in const context as of 1.95.

Two adjacent assertion macros stabilized at 1.96: `assert_matches!` and `debug_assert_matches!` assert a value matches a pattern and panic with a diagnostic otherwise — sharper than `assert!(matches!(...))` because the failure message shows the actual value. The `debug_` form compiles out in release like `debug_assert!`. These are first-class in tests, where the testing conventions already lean on pattern assertions.

```rust
// NEW (1.96): pattern assertion with a real failure message.
assert_matches!(cmd.apply(&mut doc), Ok(Effect::Repaint { .. }));
```

### When not to reach

- Don't swap a correct `(a + b) / 2` for `midpoint` in code where the operands are provably small (a 0-255 channel pair) — the overflow it prevents can't happen there, and the rename is churn.
- `strict_*` is for arithmetic that must abort on overflow. If the surrounding logic already handles a `None` or a saturating result, keep `checked_*` / `saturating_*` — don't trade a recoverable path for a panic.
- The `unchecked_*` integer shifts (1.93) and the raw `String`/`str` `*_unchecked` constructors are `unsafe`; the workspace forbids `unsafe` everywhere, so leave them out unless a specific FFI boundary earns a documented exception.
