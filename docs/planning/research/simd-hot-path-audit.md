# SIMD hot-path audit

Audit of every hot loop in `core/src/canvas/`, `core/src/transforms/`,
and `core/src/selection/`, with notes on `std::simd` eligibility, the
conflict surface against the `palette` crate's typed color API, and a
1-10 payoff rank. The audit is paired with a criterion baseline suite
under `core/benches/` so a future SIMD stream has measurements to
beat.

This is research only. No production code lands with S60.

## Methodology

### Bench scaffold

Criterion 0.8 with `harness = false` per bench binary. Throughput is
reported in pixels per second so the numbers are comparable across
buffer sizes. Inputs are wrapped in `black_box` so LLVM cannot
constant-fold them away across iterations.

Three bench binaries:

- `core/benches/composite.rs` — 256×256 single-layer composite for
  `BlendMode::Normal`, `Multiply`, and `Overlay`, plus the pre-existing
  N-layer stack benches that exercise the rayon row fan-out.
- `core/benches/transforms.rs` — `rotate_bilinear` at 45 degrees,
  `scale_nearest` upscale by 1.5× and 2×, `scale_integer` 2×, the
  RotSprite full path at 45 degrees, flip, and translate.
- `core/benches/selection.rs` — `magic_wand` over a dense connected
  blob and over a sparse multi-blob image. Both inputs are 256×256.

The `scale_bilinear 1.5×` slot in the original spec was substituted
with `scale_nearest 1.5×` (256×256 → 384×384) because the current
public scale API in `core::transforms` exposes only nearest, integer
multiple, and integer divisor. A bilinear scale arrives with a future
non-pixel-art bench when one is added.

### Run procedure

```text
cargo bench -p pixhaus-core --bench composite
cargo bench -p pixhaus-core --bench transforms
cargo bench -p pixhaus-core --bench selection
```

CI gates the suite on `cargo bench --no-run -p pixhaus-core`. The
numbers themselves are advisory; they exist to establish a baseline a
future SIMD implementation can beat.

### Dev host

```text
uname -a:  Darwin 25.3.0 arm64 (Apple Silicon)
CPU:       Apple M4 Pro
Cores:     14 (10 performance + 4 efficiency)
SIMD:      NEON (128-bit) baseline; SVE not exposed
Rust:      stable channel, opt-level=3 release builds for bench
```

Apple Silicon NEON is 128-bit. The SIMD analysis below assumes a
128-bit vector width — eight `u8` lanes for per-channel work or four
`f32` lanes for affine sampling. `std::simd` on stable today is the
portable-SIMD nightly-only proposal; the eligibility column counts a
loop as "SIMD eligible" if the shape would map cleanly to portable
SIMD once stabilized, or via `std::arch::aarch64::*` intrinsics today.

### What the numbers are not

They are not absolute targets. They are not a ranking of "the editor
feels slow when this loop runs." They are the baseline against which
a future SIMD stream measures its win.

## Composite hot loops

The composite path lives in three files: `blend.rs` does per-pixel
math, `composite.rs` does the rayon row fan-out, and `buffer.rs`
exposes the byte slices. Every entry below names the inner loop that
dominates wall time.

The "palette conflict" column tracks whether replacing the existing
`Rgba { r, g, b, a: u8 }` with the `palette` crate's typed color API
(e.g. `Srgba<u8>`) would block SIMD vectorization for that loop.

| Loop | Shape | SIMD eligible | Palette conflict | Rank |
|---|---|---|---|---|
| `composite::blend_row` outer loop | Iterates 4-byte RGBA chunks; short-circuits on `s.a == 0`; dispatches `blend(mode, s, d, opacity)` per pixel | Y — outer iteration is data-parallel by 4-pixel groups; inner blend dispatch is the gating factor | N — slice-of-bytes is the natural form for both | 9 |
| `blend::blend_normal` per-channel mix | `b + (s - b) * sa / ra` in `i32`; runs for r, g, b after `r_a` is known | Y — three identical integer expressions over independent lanes | N — operates on raw u8 channels | 9 |
| `blend::mul_un8` | `(a*b + 0x80)` then two shifts; branch-free | Y — saturating u16 multiply, shift, narrow; maps to `vqdmulh` family on NEON | N | 10 |
| `blend::channel_multiply` | `mul_un8(b, s)` per channel | Y — three lanes of `mul_un8` | N | 10 |
| `blend::channel_screen` | `b + s - mul_un8(b, s)`; widens to u16 | Y — widen / multiply / narrow pattern | N | 10 |
| `blend::channel_darken` / `channel_lighten` | `min` / `max` per channel | Y — direct NEON `umin`/`umax` | N | 10 |
| `blend::channel_addition` / `channel_subtract` | Saturating add / subtract per channel | Y — direct NEON `uqadd`/`uqsub` | N | 10 |
| `blend::channel_difference` | `abs_diff` per channel | Y — direct NEON `uabd` | N | 10 |
| `blend::channel_exclusion` | `b + s - 2*mul_un8(b, s)` | Y — composes existing eligible primitives | N | 9 |
| `blend::channel_color_dodge` | Branchy: returns 0 if `b == 0`, 255 if `b + s >= 255`, else `div_un8`. The branch is per-pixel | Partial — mask + blend at u8 level recovers the three branches; the integer divide is the rate-limiter | N | 7 |
| `blend::channel_color_burn` | Symmetric to color_dodge: branchy with one divide | Partial — same shape as color_dodge | N | 7 |
| `blend::channel_divide` | Three-way branch on `b == 0`, `b >= s`, else `div_un8` | Partial — mask + blend; divide cost dominates | N | 6 |
| `blend::channel_overlay` / `channel_hard_light` | `s < 128 ? multiply : screen` branch | Partial — compute both, blend by the mask; widens working width to u16 | N | 8 |
| `blend::channel_soft_light` | Per-pixel f64 polynomial with a `b <= 0.25` branch | Partial — vectorizable with f32 lanes and predicate; significant accuracy work | N (uses raw u8) | 6 |
| `blend::channel_linear_burn` / `channel_linear_dodge` | `sum = b + s`; saturate at 0 or 255 | Y — saturating add / subtract families on NEON | N | 10 |
| `blend::channel_vivid_light` | `s < 128` → color_burn(b, 2s); else color_dodge(b, 2s-255) | Partial — branchy, plus chained partially-eligible primitives | N | 5 |
| `blend::channel_linear_light` | `s < 128` → linear_burn; else linear_dodge | Partial — branchy but the leaves are eligible | N | 7 |
| `blend::channel_pin_light` | `s < 128` → darken(b, 2s); else lighten(b, 2s-255) | Partial — branchy, leaves are eligible | N | 7 |
| `blend::channel_hard_mix` | `vivid_light(b, s) < 128 ? 0 : 255` | Partial — runs vivid_light first, then threshold | N | 4 |
| `blend::rgba_darker_color` / `rgba_lighter_color` | Per-pixel Rec.709 luma compare; whole-pixel pick, not per-channel | Y — luma is a fused-multiply-add of three u8 channels; compare-and-select is one NEON op | N | 8 |
| `blend::channels_hue` / `channels_saturation` / `channels_color` / `channels_luminosity` | f64 HSL non-separable math; `set_sat` is a tag-dispatch tree on min/max channel | N — control flow is per-pixel and chained; restructuring would change byte-exact output | N | 2 |
| `composite::composite_onto` row fan-out | `par_chunks_mut` over rows; one rayon task per row | N/A — coarse-grained parallelism, already optimized for thread fan-out | N | n/a |
| Per-pixel `Rgba::new(...)` reconstruction in `blend_row` | Builds an `Rgba` from four byte loads, blends, writes back four bytes | Y — gather/scatter or aligned 4-byte loads; would benefit from SoA layout | Y — palette's `Srgba<u8>` keeps AoS too, but a switch to its `cast::*` helpers would invalidate the byte-slice fast path | 8 |

### What "palette conflict" means

The `palette` crate ships typed wrappers (`Srgba<u8>`, `LinSrgba<f32>`,
`Hsla<f32>`) that carry color-space metadata. The current
`Rgba { r, g, b, a }` is a transparent 4-byte newtype — the bytes are
`r, g, b, a` in memory and a `&[Rgba]` view of a `Vec<u8>` is a sound
reinterpret. A future palette-typed channel layer must keep that
property if SIMD is to stay viable. Concretely: any conversion that
forces a per-pixel `into_format::<u8>()` call breaks the AoS fast
path. The audit flags zero loops as conflicting because the existing
Rgba layout is `repr(C)` and trivially reinterpretable; the risk is
forward-looking, not present.

## Transform hot loops

Every transform that touches more than O(1) pixels is included. The
exact lossless cases (`rotate_90_*`, `rotate_180`, `flip_*`,
`scale_integer*`, `translate`) are pure index permutations and gain
nothing from per-pixel SIMD beyond what a tuned memcpy already
delivers.

| Loop | Shape | SIMD eligible | Palette conflict | Rank |
|---|---|---|---|---|
| `rotate::rotate_90_cw` / `_ccw` / `rotate_180` | Index remap; `out.set_pixel(nx, ny, buf.pixel(sx, sy))` | N — memory-bound transpose; cache tiling beats SIMD lanes | N | 2 |
| `rotate::rotate_bilinear` outer loop | Per-output-pixel inverse rotate → bilinear_sample | Y — affine map is four f32 lanes; sample math is already lane-parallel; cubic over 4 taps | N | 9 |
| `rotate::bilinear_sample` | Four sample-corner gathers + per-channel u8 lerps | Y — four lerps of four lanes each maps directly to NEON `umlal`/`urhadd`; gather is the awkward part | N | 8 |
| `rotate::rotate_rotsprite` | Calls `scale_integer` ×2 (Scale2x), then `rotate_bilinear`, then `scale_nearest` | Y — gains derive from the bilinear core; the surrounding orchestration is fixed | N | 7 |
| `scale::scale_nearest` | `for dy / for dx`: integer-multiply nearest index | Y at the destination loop — 8 lanes of `dx * src_w / new_w` map to NEON `umull`/`udiv`; gather from source is awkward | N | 5 |
| `scale::scale_integer` | Per-source-pixel `factor × factor` block fill | Y — block fill is a broadcast plus aligned stores; for 2x and 4x this is a memcpy variant | N | 6 |
| `scale::scale_integer_down` | Per-output-pixel top-left sample | N — pure index permutation, no arithmetic on pixel values | N | 2 |
| `flip::flip_horizontal` / `_vertical` | Index-remap row or column reverse | N — `slice::reverse` is already SIMD-tuned in libstd | N | 1 |
| `skew::skew_x` / `skew_y` | Per-row (or per-column) shift; `(factor * y).round() as i32` | N — shift is amortized over a full row; the body is a memmove with an offset | N | 3 |
| `translate::translate` outer loop | Index remap with optional mask gate | N — memory-bound; mask path adds branches that don't vectorize | N | 2 |
| `perspective::perspective` outer loop | Per-output-pixel apply 3×3 inverse homography, then bilinear_sample | Y — projective divide is one f32 lane; the four-lane sample reuses the bilinear core | N | 8 |
| `antialias::process_line` | Two-pass morphological antialias; per-edge classifier driven by `PixelSelector::are_equal` (per-channel max-abs-diff) | Partial — `are_equal` is four u8 abs-diffs and a max; the surrounding edge run-finding is sequential | N | 5 |
| `antialias::PixelSelector::are_equal` | `max(abs(a.r - b.r), abs(a.g - b.g), abs(a.b - b.b), abs(a.a - b.a)) < threshold` | Y — four lanes of `uabd` then a horizontal reduce | N | 8 |

## Selection hot loops

Selection algorithms split between shape rasterization (`select_*`),
flood fill (`magic_wand`), color matching (`color_range`), and
morphology (`expand`/`contract`/`feather`).

| Loop | Shape | SIMD eligible | Palette conflict | Rank |
|---|---|---|---|---|
| `algorithms::magic_wand` BFS body | `VecDeque<(u32, u32)>` of frontier pixels; `colors_match` per visit | N — irreducibly sequential frontier expansion; SIMD would need a parallel BFS rewrite | N | 1 |
| `algorithms::colors_match` (per-channel tolerance) | `\|b - s\| <= tolerance` for each of r, g, b, a, then `&&` | Y in isolation — four lanes of `uabd` and a compare-mask; in BFS context the call rate is low | N | 6 (low call rate ceiling) |
| `algorithms::color_range` | Linear scan over every pixel; per-pixel `colors_match` | Y — embarrassingly parallel over a flat byte slice; one mask per 4-byte group | N | 9 |
| `algorithms::select_rect` | Double loop, `mask.set(x, y, 255)` | Y — row fills are `slice::fill` which is already SIMD-tuned | N | 3 |
| `algorithms::select_ellipse` | Per-pixel point-in-ellipse test | Y — eight lanes of `x*x + y*y < r2` mapped to a coverage mask | N | 5 |
| `algorithms::select_polygon` | Scanline fill with horizontal range merge | Partial — the range emit is sequential, but the fill phase per scanline is row-parallel | N | 4 |
| `morphology::expand` (dilation) | For each pixel, scan a disc of radius `by`; on first hit, mark and break | Y — convert the disc to a row mask, compute the per-pixel OR over the mask via shifted SIMD loads | N | 8 |
| `morphology::contract` (erosion) | Symmetric to `expand` but requires all-true | Y — same shape as expand with AND replacing OR | N | 8 |
| `morphology::feather` (two-pass box blur) | Horizontal pass then vertical pass; running sum already O(N) per axis | Partial — the running-sum form is sequential by definition; a fixed-radius variant would map to a sum-of-shifted-loads | N | 6 |
| `autoclose::close_gaps` distance transform | Two-pass chamfer / sequential | N — sequential by construction | N | 2 |

## Findings summary

Ranked from highest expected SIMD payoff to lowest. The rank reflects
a combination of "hot loop on the editor's critical path" and "shape
is friendly to portable SIMD."

1. **Separable per-channel blend math (rank 10).** `mul_un8`,
   `channel_multiply`, `channel_screen`, `channel_darken`/`lighten`,
   `channel_addition`/`subtract`, `channel_difference`,
   `channel_linear_burn`/`dodge` are all branch-free per-channel
   integer arithmetic. Eight u8 lanes on NEON is two full RGBA pixels
   per instruction. This is the biggest single SIMD lever in the
   composite path.
2. **`blend_normal` per-channel mix (rank 9).** Three identical
   `i32` lane expressions per pixel; vectorizes cleanly once the
   per-pixel `s_a` and `r_a` are materialized as broadcast values.
3. **`composite::blend_row` outer loop (rank 9).** Wrapping
   per-pixel byte loads/writes around vectorized blend math is the
   single biggest source of overhead per row. A row-vectorized
   composite would lift the entire compositor path.
4. **`rotate_bilinear` outer + `bilinear_sample` (rank 8-9).** The
   affine map is four-lane f32; the four-corner bilinear lerp is the
   classic NEON sample-and-blend pattern. RotSprite's quality
   ceiling is set by this core.
5. **`color_range` linear scan (rank 9).** Pure byte-slice scan over
   the source; per-pixel coverage mask write to `SelectionMask`. The
   simplest SIMD win in the whole codebase.
6. **Branched contrast modes (rank 7-8).** Overlay, hard_light,
   linear_light, pin_light — each has a single `s < 128` branch.
   Mask-and-blend recovers the branch at the cost of computing both
   sides, which is still faster than per-pixel dispatch.
7. **Morphology expand / contract (rank 8).** Disc-mask
   neighbourhood scan; convert the disc to a per-row bit pattern and
   the inner loop becomes a small fixed number of SIMD loads.
8. **`rgba_darker_color` / `rgba_lighter_color` (rank 8).** Rec.709
   luma compare across two pixels; fused multiply-add plus pick. The
   per-channel work is dwarfed by the per-pixel dispatch in the
   compositor, so the win compounds with #3.
9. **`PixelSelector::are_equal` in morphological antialias (rank
   8).** Four u8 abs-diffs and a horizontal max; lifts the rate of
   the surrounding `process_line` even though the orchestration is
   sequential.
10. **Color-dodge / color-burn / divide (rank 6-7).** Branchy and
    each includes one per-pixel divide. The mask-and-blend trick
    works but the divide is the rate limiter; gains are real but
    smaller.
11. **HSL non-separable modes (rank 2).** Hue, saturation, color,
    luminosity each call `set_sat` whose control flow is a
    tag-dispatch tree on the channel ordering. Restructuring would
    break byte-exact parity with Aseprite. Out of scope for any
    near-term SIMD effort.
12. **Pure index permutations (rank 1-3).** `rotate_90_*`,
    `rotate_180`, `flip_*`, `translate`, `scale_integer_down` — no
    arithmetic on pixel values. libstd's existing slice copies and
    `slice::reverse` already SIMD-vectorize where it pays.

## Next-stream sketch

A future S61 stream would convert the highest-rank loops behind a
narrow surface. No commitment is made here; this is the shape such a
stream would take.

### Scope

- A new `core::canvas::blend_simd` module gated by a `simd-blend`
  Cargo feature. The feature is off by default; enabled in release
  builds for desktop targets.
- Row-level entry points only: `blend_normal_row(dst, src, opacity)`,
  `blend_multiply_row(...)`, etc. for the rank-9-and-up modes. The
  per-pixel `blend(mode, s, d, opacity)` function stays as the
  fallback; `composite::blend_row` dispatches to the SIMD row
  function when the feature is on and the mode is supported, falling
  back to the scalar path otherwise.
- One row of `core::transforms::rotate_bilinear_simd` for the affine
  + four-corner-lerp inner. The orchestration in `rotate_rotsprite`
  is unchanged.
- A `core::selection::color_range_simd` with a vectorized linear
  scan. `magic_wand` is not touched — the BFS frontier is the wrong
  shape for SIMD.

### What stays scalar

- HSL non-separable modes (hue/saturation/color/luminosity) — byte-
  exact parity with Aseprite outweighs the small expected win.
- Morphology expand/contract — wait for measurements; if the
  morphology path is not on the editor's critical path the
  complexity isn't worth it.
- Soft light — f64 polynomial with a branch; revisit only if a
  benchmark proves it's hot.

### Engineering shape

`std::simd` portable SIMD is unstable as of this audit. Two viable
paths:

1. **Wait for portable SIMD to stabilize.** Pros: one
   implementation, all targets. Cons: timeline is not under our
   control.
2. **Ship NEON + AVX2 paths today via `std::arch` intrinsics, behind
   `#[cfg(target_arch = "...")]` and a runtime CPU feature probe.**
   Pros: ships now, predictable codegen. Cons: two intrinsic-level
   paths to maintain.

Recommendation: option 2 for the first SIMD stream, with a clean
trait boundary so a future portable-SIMD path can be slotted in
without touching the call sites. The fallback is always the existing
scalar implementation.

### Verification

- `image-compare` snapshot tests at the row level: the SIMD path
  must agree with the scalar path byte-for-byte on a fuzzed buffer
  with `proptest`.
- Each enabled mode runs through the existing Aseprite parity test
  set in `core/src/canvas/blend.rs`.
- `criterion` regressions against the S60 baselines tracked in this
  PR's bench output as a non-gating advisory.

### What this stream is not

It is not the runtime CPU-feature probe (build it once when the
first SIMD stream lands). It is not a rewrite of the pixel buffer to
SoA or premultiplied storage. It is not a port of OpenToonz's
`quickput.cpp` template tower — that file is the contrast pattern,
not a transcription target.

## References

- `toonz/sources/common/trop/quickput.cpp` (BSD-3-Clause) — the
  contrast pattern for hot-path template specialization and 16-bit
  fixed-point sub-pixel addressing. No code adapted.
- `toonz/sources/stdfx/igs_color_blend.cpp` (BSD-3-Clause) — origin
  of the eight contrast/linear blend modes added in S55.
- `core/src/canvas/blend.rs` — every per-channel and per-pixel blend
  function audited above.
- `core/src/canvas/composite.rs` — rayon row fan-out and the
  per-pixel dispatch in `blend_row`.
- `core/src/transforms/{rotate, scale, flip, skew, perspective,
  translate, antialias}.rs` — transform hot loops.
- `core/src/selection/{algorithms, morphology}.rs` — selection hot
  loops.
- `core/benches/{composite, transforms, selection}.rs` — the
  baseline measurements this audit pairs with.
- `docs/planning/work/s54-s60-opentoonz-adoption.md#s60` — the
  stream brief.
