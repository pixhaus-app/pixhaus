# OpenToonz — source-code research and learning candidates for Pixhaus

## Context

OpenToonz is the open-source descendant of Toonz, the 2D animation tool that Studio Ghibli customized and shipped feature films through from the early 2000s. Dwango Co., Ltd. released the source as BSD-3-Clause in 2016. The repo at `/Users/luismorales/project/pixhaus-app/opentoonz` is a working clone.

A separate document at `docs/planning/frame-by-frame/opentoonz.md` already surveys OpenToonz as a tool — its UX, workflow, and feature list as competition. This document does the orthogonal thing: it reads the source code and asks what algorithms, data models, and engineering choices in OpenToonz could inform Pixhaus's own Rust code.

Three reasons this is worth doing now:

- **License is permissive.** BSD-3 lets us adapt code with attribution. The legal cost is bookkeeping.
- **The algorithms are production-tested.** Ghibli pushed real feature films through this pipeline. Edge cases were paid for in tears, not in unit tests.
- **Pixhaus has shipped its v1 surface.** All bedrock specs (B1–B10) and feature streams (S01–S52) are DONE per `work/queue.md`. Post-launch refinement is where OpenToonz pays off — palette animation, morphological anti-aliasing, gap-closing flood fill, procedural inbetweening, centerline vectorization, color-quantization quality, brush stabilization. None of those are blocking v1; all of them would make v1.x sharper.

One up-front correction. The casual impression is that OpenToonz is "C code." It is not. The codebase is overwhelmingly C++: roughly 1,075 `.cpp` files and 1,123 `.h` files against ~32 pure `.c` files. The C-style code is concentrated in image-codec wrappers (`image/tif/`, `image/ffmpeg/`) and a handful of `common/trop/` utilities. The interesting algorithms are C++, but they are mostly Qt-light — they live below the UI layer and can be read on their own. The translation cost to Rust is bounded by the C++ template machinery, not by GUI plumbing.

## Repo map at a glance

```
toonz/sources/
  common/        Shared libraries
    traster/       Template raster (pixel buffer)
    trop/          Raster operations — antialias, autoclose, blur, brush, quickput
    tvrender/      Render-side utilities — including tinbetween.cpp
    tvectorimage/  Vector layer model
    tcore/         Core utilities — TUndo, persistence, smart pointers
    tcolor/        Color math
  image/         File-format I/O — tif, png, ffmpeg, exr, psd
  toonzlib/      Animation and project logic — xsheet, levels, palette utilities
  tnztools/      Drawing tools — brushes, fill, selection
  toonzqt/       Qt GUI layer
  toonz/         Application binary entry
  stdfx/         Standard effects — including igs_color_blend.cpp (22+ modes)
  tcleanupper/   Line cleanup / vectorization
  include/       Public headers — tpalette.h, tstroke.h, traster.h
thirdparty/      Vendored deps — Boost, libpng, libtiff, libjpeg, libmypaint, GLEW,
                 kiss_fft, OpenBLAS, SuperLU, tinyexr, Lz4
stuff/           Assets, brush library, layouts
plugins/         Sample plugins
doc/             Build instructions per platform
```

Build is CMake 3.5+, Qt 5.15.2. Vendored deps in `thirdparty/` have their own licenses; the project's BSD-3 grant only covers the first-party code.

The internal documentation is sparse. `doc/` is build instructions per platform plus a small contributor guide. In-code documentation is Doxygen-style, with rationale paragraphs on the bigger headers (`tpalette.h` has the most). The official OpenToonz documentation lives in a separate repo (`opentoonz/opentoonz_docs`) and is user-facing.

## Pixhaus ↔ OpenToonz subsystem map

| Pixhaus | OpenToonz | Transfer potential |
|---|---|---|
| `core/src/canvas/blend.rs` (19 modes) | `stdfx/igs_color_blend.cpp` (22+ modes) | High — mode set is a superset |
| `core/src/canvas/buffer.rs` | `common/trop/quickput.cpp` (173 KB) | Medium — SIMD loop shape |
| `core/src/canvas/composite.rs` | `common/trop/quickput.cpp` + `tropP.h` | Medium |
| `core/src/transforms/rotate.rs` (RotSprite + bilinear) | `common/trop/tantialias.cpp` (morphological AA) | High — edge-aware AA is missing in Pixhaus |
| `core/src/selection/algorithms.rs` (magic wand) | `common/trop/tautoclose.cpp` (gap closing) | High — Pixhaus has no gap-closing path |
| `core/src/project/palette.rs` (entries + names) | `include/tpalette.h` (pages, animation, shortcuts) | High — Pixhaus palette is structurally thinner |
| `core/src/color/space.rs` (`palette` crate, 8-bit storage) | `include/tcolorutils.h` + `tcolorfunctions.h` | Low — Pixhaus is already on solid color-math foundations |
| `core/src/tilemap/` | no direct analog — OpenToonz is not tile-oriented | None |
| `io/aseprite/` | `image/tnz/` (Toonz TLV format) | Low — Pixhaus has its own native format |
| `ai/src/verbs/inbetween/` (AI-only) | `common/tvrender/tinbetween.cpp` (procedural) | High — procedural fallback is missing |
| `ai/src/verbs/cleanup/` (AI-only) | `toonzlib/tcenterlinevectorizer.cpp` (procedural) | High — same shape as above |
| `io/export/` (GIF/WebP/MP4) | `toonzlib/cleanuppalette.cpp` (palette reduction) | Low-Medium — quality cross-check |
| `scripting/` (Lua bindings skeleton) | no equivalent — OpenToonz uses Qt-driven Sandor effects | None |
| `app/` (Tauri commands) | `toonz/sources/{toonzqt, toonz}` (Qt) | None — different UI model |

The transferable cluster is `common/trop/` plus `common/tvrender/tinbetween.cpp` plus `include/tpalette.h` plus `toonzlib/tcenterlinevectorizer.cpp` and `toonzlib/cleanuppalette.cpp`. Everything else is either already well-served in Pixhaus, structurally incompatible, or Qt-bound.

## Deep dives — algorithm by algorithm

Each dive cites OpenToonz files at the surveyed clone (no commit pin; the clone is local). Code excerpts are quoted verbatim from those files for traceability. Adaptation sketches are signatures only — not implementations.

### Blend modes — `toonz/sources/stdfx/igs_color_blend.cpp`

OpenToonz implements at least 22 distinct blend modes, with a header comment that cross-references the PDF blend spec (January 2006 addendum), Toonz's own FX layer modes, and Photoshop's adjustment-layer modes. The file opens with a comparison table:

```
------ Light Mode (Contrast) ------
オーバーレイ	Overlay		-		Overlay	     12 overlay
ソフトライト	Soft Light	-		SoftLight    13 soft_light
ハードライト	Hard Light	-		HardLight    14 hard_light
ビビッドライト	Vivid Light	-		-	     15 vivid_light
リニアライト	Linear Light	-		-	     16 linear_light
ピンライト	Pin Light	-		-	     17 pin_light
ハードミックス	Hard Mix	-		-	     18 hard_mix
```

(`stdfx/igs_color_blend.cpp:26-33`)

The math itself is pre-multiplied RGBA in `double`. Channel math is separated from the alpha-composition step via a `blend_transp_` helper:

```cpp
double blend_transp_(const double bl, const double dn, const double dn_a,
                     const double up, const double up_a,
                     const double up_opacity) {
  double bl2 = bl * ((dn_a < up_a) ? dn_a / up_a : up_a / dn_a);
  bl2 += (up_a < dn_a) ? (dn / dn_a * (dn_a - up_a) / dn_a) : 0.0;
  bl2 += (dn_a < up_a) ? (up / up_a * (up_a - dn_a) / up_a) : 0.0;
  bl2 *= up_a + dn_a * (1.0 - up_a);
  return dn * (1.0 - up_opacity) + bl2 * up_opacity;
}
```

(`stdfx/igs_color_blend.cpp:97-110`)

Pixhaus's `BlendMode` enum at `core/src/project/blend.rs` carries 19 modes (Aseprite parity). Cross-checking against the OpenToonz list, Pixhaus does not have: **Linear Burn**, **Darker Color**, **Linear Dodge** (as distinct from Addition), **Lighter Color**, **Vivid Light**, **Linear Light**, **Pin Light**, **Hard Mix**. The first four are arithmetic combinators that map cleanly to Pixhaus's existing channel-math infrastructure; the last four are "contrast" modes that require a per-pixel branch on the comparison color.

**Rust adaptation sketch.** Add the missing variants to `BlendMode`, implement under `core/src/canvas/blend.rs`. Each new mode is ~10 lines. Confirm Aseprite-roundtrip behavior — `.aseprite` files do not encode these modes, so a fallback to `Normal` on export is required. **Effort: S.**

### Morphological anti-aliasing — `toonz/sources/common/trop/tantialias.cpp`

The file header credits Alexander Reshetov's "Morphological Antialiasing" (Intel Labs) and explains the model:

```cpp
/*
See Alexander Reshetov's "Morphological Antialiasing" paper on Intel Labs site.

Basically, this antialiasing algorithm is based on the following ideas:

 - Suppose that our image is just made up of flat colors. Then, a simple
   antialiasing approach is that of assuming that the 'actual' line separating
   two distinct colors is the polyline that passes through the midpoint of
   each edge of its original jaggy counterpart.
   As pixels around the border are cut through by the polyline, the area of
   the pixel that is filled of a certain color is its weight in the output
   filtered pixel.
*/
```

(`common/trop/tantialias.cpp:5-20`)

The classifier is template-parameterized on pixel type and on a "selector" trait that defines equality. For `TPixel32` this is a per-channel max-diff threshold; for the cleanup format `TPixelCM32` it is ink-id-aware:

```cpp
bool areEqual(const TPixelCM32 &a, const TPixelCM32 &b) const {
  return (a.getInk() == b.getInk()) &&
         (abs(a.getTone() - b.getTone()) < m_thresh);
}
```

(`common/trop/tantialias.cpp:70-73`)

The driver passes once by rows then once by columns:

```cpp
// First, filter by rows
for (y = 0; y < ly_1; ++y) {
  processLine(y, lx, ly, src->pixels(y), src->pixels(y + 1), dst->pixels(y),
              dst->pixels(y + 1), 1, src->getWrap(), 1, 1, true, hStart,
              slope, sel);
}
// Then, go by columns
for (x = 0; x < lx_1; ++x) {
  processLine(x, ly, lx, src->pixels(0) + x, src->pixels(0) + x + 1,
              dst->pixels(0) + x, dst->pixels(0) + x + 1, src->getWrap(), 1,
              dst->getWrap(), dst->getWrap(), false, hStart, slope, sel);
}
```

(`common/trop/tantialias.cpp:357-370`)

Pixhaus today rotates via RotSprite for integer multiples and bilinear for arbitrary angles (`core/src/transforms/rotate.rs`). Bilinear is a one-shot box filter; it does not preserve sharp edges. Morphological AA is the opposite trade-off — it preserves edges and softens only where the algorithm classifies a run as a "separation line."

**Rust adaptation sketch.** A new module `core/src/transforms/antialias.rs` exposing:

```rust
pub fn morphological_antialias(
    src: &PixelBuffer,
    dst: &mut PixelBuffer,
    threshold: u8,
    softness: u8,
);
```

Both passes are embarrassingly parallel by scan line; Pixhaus's existing `rayon` use in `core/src/transforms/scale.rs` is a workable model. The branchy classifier suggests scalar code is fine for v1 — SIMD work belongs to the broader buffer audit in the SIMD stream below. **Effort: M.**

Attribution: BSD-3 + cite Reshetov's paper in module header.

### Gap-closing flood fill — `toonz/sources/common/trop/tautoclose.cpp`

This is the algorithm that lets an artist drop a paint bucket inside an ink outline that is *almost* closed and have the fill respect the intended contour rather than escape through a one-pixel gap. The implementation runs over a packed byte raster with per-bit semantics:

```cpp
UCHAR inline neighboursCode(UCHAR *seed) {
  return ((swPix(seed) & 0x1) | ((sPix(seed) & 0x1) << 1) |
          ((sePix(seed) & 0x1) << 2) | ((wPix(seed) & 0x1) << 3) |
          ((ePix(seed) & 0x1) << 4) | ((nwPix(seed) & 0x1) << 5) |
          ((nPix(seed) & 0x1) << 6) | ((nePix(seed) & 0x1) << 7));
}
```

(`common/trop/tautoclose.cpp:54-59`)

The 8-bit `neighboursCode` is an index into a skeleton lookup table (`skeletonlut.h`) that classifies each pixel as endpoint, border, branch, or interior. Endpoints become seed candidates; the closer searches outward for a paired endpoint within `m_closingDistance` (default 10 pixels) and at an angle within `m_spotAngle` (default 90°). When a pair is found, it rasterizes a connecting segment with a Bresenham-style macro:

```cpp
#define DRAW_SEGMENT(a, b, da, db, istr1, istr2, block) \
  { \
    d      = 2 * db - da; \
    incr_1 = 2 * db; \
    incr_2 = 2 * (db - da); \
    while (a < da) { \
      if (d <= 0) { d += incr_1; a++; istr1; } \
      else        { d += incr_2; a++; b++; istr2; } \
      block; \
    } \
  }
```

(`common/trop/tautoclose.cpp:112-130`)

Pixhaus's selection algorithms today (`core/src/selection/algorithms.rs`) implement magic-wand with 4- and 8-connectivity plus a per-channel tolerance, but no gap-closing pass. A user filling a sketch with a 1- or 2-pixel break gets the fill leaking — they have to clean up the outline first.

**Rust adaptation sketch.** A new module `core/src/selection/autoclose.rs` exposing:

```rust
pub fn close_gaps(
    buffer: &PixelBuffer,
    threshold: u8,
    closing_distance: u32,
    closing_angle_rad: f32,
) -> SelectionMask;

pub fn magic_wand_with_gap_close(
    buffer: &PixelBuffer,
    seed: IVec2,
    tolerance: u8,
    connectivity: Connectivity,
    gap_config: Option<GapCloseConfig>,
) -> Result<SelectionMask>;
```

The skeleton LUT is ~256 entries and can be a `static` table. Bresenham segment-rasterization is already a one-pager. **Effort: M.**

Attribution: BSD-3 in the LUT module header.

### Fast raster operations — `toonz/sources/common/trop/quickput.cpp`

The 173 KB `quickput.cpp` is the OpenToonz raster equivalent of a hot kitchen. The first non-trivial function shows the pattern — apply a transform `aff` to source `up` and composite into destination `dn`, with fixed-point inverse-mapping:

```cpp
void doQuickPutFilter(const TRaster32P &dn, const TRaster32P &up,
                      const TAffine &aff) {
  if ((aff.a11 * aff.a22 - aff.a12 * aff.a21) == 0) return;
  const int PADN  = 16;
  const int MASKN = (1 << PADN) - 1;
  TRectD boundingBoxD = TRectD(convert(dn->getSize())) *
                        (aff * TRectD(0, 0, up->getLx() - 2, up->getLy() - 2));
  // ...
  TAffine invAff = inv(aff);
  double deltaXD = invAff.a11;
  double deltaYD = invAff.a21;
  int deltaXL = tround(deltaXD * (1 << PADN));
```

(`common/trop/quickput.cpp:76-119`)

Two patterns are worth pulling out. First, the file is template-heavy — each variant of pixel type, blend mode, and filter strategy is a specialization, generated by macros from `loop_macros.h`. This is the C++ analogue of Rust's monomorphization. Second, the inner loops use 16-bit fixed-point for sub-pixel addressing — a clear signal that the original authors profiled this and chose integer math over floating-point on the hot path.

Pixhaus's `core/src/canvas/buffer.rs` is ~20 KB of buffer code today. The transforms (`scale.rs`, `rotate.rs`) and the blend dispatcher (`core/src/canvas/composite.rs`) are organized around scalar loops over `Rgba`. There has not been a performance audit pass.

**Rust adaptation sketch.** This is not a port — it is a model for an audit. The output would be a survey doc listing each hot loop in `core/src/canvas/` and `core/src/transforms/` with three columns: current shape, whether `std::simd` would map cleanly, and whether the existing `palette` and `rayon` integrations get in the way. Tooling: `cargo bench` with `criterion`, perf flamegraphs on representative workloads.

**Effort: M for the audit, L if it turns into a rewrite of the buffer module.** Do not start until v1 ships — premature SIMD work locks the buffer shape before usage patterns are known.

### Stroke inbetweening — `toonz/sources/common/tvrender/tinbetween.cpp`

This is the algorithm worth the most for Pixhaus. The bedrock idea is small and elegant: given two strokes, parameterize them, sample matched points along arc length, average them, and reject outliers using variance. The outlier-rejection step is the part that distinguishes a usable in-between from a smeared one:

```cpp
double average = sum / size;
double variance = 0;
for (j = 0; j < size; j++) {
  variance += (average - values[j]) * (average - values[j]);
}
variance /= size;

double err;
int acceptedNum = 0;
sum             = 0;
for (j = 0; j < size; j++) {
  err = values[j] - average;
  err *= err;
  if (err <= range * variance) {
    sum += values[j];
    acceptedNum++;
  }
}

assert(acceptedNum > 0);
return (acceptedNum > 0) ? sum / (double)acceptedNum : average;
```

(`common/tvrender/tinbetween.cpp:33-54`)

The default `range = 2.5` is a "values inside 2.5σ count, rest are dropped" rule. The weighted variant on the next page does the same with per-sample weights for emphasis. This is procedural — no model, no backend, no API key. It runs on a laptop in microseconds.

Pixhaus's `ai/src/verbs/inbetween/mod.rs` is AI-only. Its docstring is explicit: "Takes two pixel-art frames (A and B) and generates N intermediate frames using a frame-interpolation backend (RIFE-class or video diffusion)." There is no procedural fallback. If the user has no backend configured, or wants a deterministic preview before the AI call, or wants to skip the network round-trip for short animations, they have nothing.

**Rust adaptation sketch.** Either a sibling verb `inbetween_procedural` or a `mode` argument on the existing `inbetween` verb:

```rust
pub enum InbetweenMode {
    Procedural { variance_range: f32 },
    AiInterpolation { backend: BackendId },
    AiInterpolationWithProceduralPreview,
}
```

The algorithm in this file is stroke-based, not raster-based, but the variance-rejection idea is the transferable part. For raster frames the loop is: detect changed regions, compute per-pixel weighted averages between A and B, reject samples whose error from the mean exceeds the variance threshold. **Effort: M for procedural raster path; L if we want to handle non-rigid motion well.**

Attribution: BSD-3 + cite the original Toonz inbetweening literature.

### Palette model — `toonz/sources/include/tpalette.h`

OpenToonz's palette is a richer structure than Pixhaus's. Two ideas stand out: **pages** (grouped views over a flat style list) and **style animation** (per-style keyframed color changes over the timeline):

```cpp
class DVAPI Page {
  friend class TPalette;
private:
  std::wstring m_name;           //!< Name of the page to be displayed.
  int m_index;                   //!< Index of the page in the palette's pages collection.
  TPalette *m_palette;           //!< (not owned) Palette the page refers to.
  std::vector<int> m_styleIds;   //!< Palette style ids contained in the page.
};
```

(`include/tpalette.h:94-101`)

And the animation table itself:

```cpp
typedef std::map<int, TColorStyleP> StyleAnimation;        //!< Style keyframes list.
typedef std::map<int, StyleAnimation> StyleAnimationTable; //!< Style keyframes list per style id.

StyleAnimationTable m_styleAnimationTable;  //!< Table of style animations (per style).
int m_currentFrame;                          //!< Palette's current frame in style animations.
```

(`include/tpalette.h:177-194`)

The header rationale section is worth reading in full at `tpalette.h:45-83`. The notion is that "color" in a Toonz file is not a direct pixel value; it is a *style id* that resolves to a color through the palette, and the resolution can change over the timeline. This is what enables palette cycling — a single sprite cel can shift through dozens of palette states without modifying any pixel data.

Pixhaus's `core/src/project/palette.rs` is structurally a flat `Vec<PaletteEntry>` with `id` and `name`:

```rust
pub struct Palette {
    pub id: PaletteId,
    pub name: String,
    // colors: Vec<PaletteEntry>, ...
}

pub struct PaletteEntry {
    pub color: Rgba,
    pub name: Option<String>,
}
```

(reconstructed from `core/src/project/palette.rs:38-55`)

There are no pages, no animation, no keyboard shortcuts. Style indirection is implicit in indexed-mode sprites, but the indirection is one-deep — palette index resolves to RGBA, the end.

**Rust adaptation sketch.** Two separable additions:

```rust
pub struct PalettePage {
    pub name: String,
    pub style_ids: Vec<PaletteIndex>,
}

pub struct PaletteAnimation {
    pub keyframes: BTreeMap<FrameIndex, BTreeMap<PaletteIndex, Rgba>>,
}
```

The first ships with the palette UI; the second is bigger and intersects with the cel/frame timeline. Bedrock B2 (the project data model) is locked, so this lands as a B2 extension — `PaletteAnimation` is a new top-level field on `Sprite`, gated behind a serde-default. **Effort: M for pages alone; M-L if animation is included.**

Attribution: BSD-3 + cite Toonz palette model in the type docs.

### Centerline vectorization — `toonz/sources/toonzlib/tcenterlinevectorizer.cpp`

OpenToonz's vectorization pipeline takes a raster ink layer and recovers vector strokes from it. The entry point reads:

```cpp
TVectorImageP VectorizerCore::centerlineVectorize(
    TImageP &image, const CenterlineConfiguration &configuration,
    TPalette *palette) {
  // ...
  VectorizerCoreGlobals globals;
  globals.currConfig = &configuration;

  Contours polygons;
  polygonize(ras, polygons, globals);

  // Most time-consuming part of vectorization, 'this' is passed to inform of
  // partial progresses
  SkeletonList *skeletons = skeletonize(polygons, this, globals);
  // ...
  organizeGraphs(skeletons, globals);
  // ...
  calculateSequenceColors(ras, globals);
  conversionToStrokes(sortibleResult, globals);
  applyStrokeColors(sortibleResult, ras, palette, globals);
  result = copyStrokes(sortibleResult);
```

(`toonzlib/tcenterlinevectorizer.cpp:158-220`)

The pipeline is: raster → polygons (contours) → skeleton graphs (medial axis) → organized graphs (junction recovery) → strokes with per-stroke styles. Each stage is a separate file in `toonzlib/` and a separate body of code.

Pixhaus's `ai/src/verbs/cleanup/mod.rs` is AI-driven — it sends the layer to a model and accepts the result. There is no procedural path. For users who do not want AI in the loop, or who need bit-exact output, or who want vector export, this gap matters.

**Rust adaptation sketch.** A new module — probably a sibling crate `vectorize` rather than a `core/` submodule, given the size — exposing:

```rust
pub struct CenterlineConfig {
    pub max_thickness: f32,
    pub thickness_ratio: f32,
    pub min_segment_length: u32,
    pub corner_threshold_rad: f32,
}

pub fn centerline_vectorize(
    raster: &PixelBuffer,
    palette: &Palette,
    config: &CenterlineConfig,
) -> VectorImage;
```

This is the largest item in the candidate list. The polygonize / skeletonize / organize-graphs trio is ~3,000 lines of C++ between `tcenterlinevectP.h` and the implementation files in `toonzlib/`. **Effort: L.** Worth it only if we commit to procedural cleanup as a non-AI offering, and if SVG export becomes a Unity-handoff requirement.

Attribution: BSD-3 in the new crate's `Cargo.toml` license notes and per-file headers.

### Color quantization — `toonz/sources/toonzlib/cleanuppalette.cpp`

This file is short (~150 lines) and shows the cleanup-palette shape: a special palette type with `TBlackCleanupStyle` and `TColorCleanupStyle` styles that hold per-style brightness, contrast, color thresholds, and white thresholds:

```cpp
void TargetColors::update(TPalette *palette, bool noAntialias) {
  m_colors.clear();
  TargetColor transparent(TPixel32(255, 255, 255, 0), 0, 0, 0, 0, 0);
  m_colors.push_back(transparent);

  for (int i = 0; i < palette->getPage(0)->getStyleCount(); i++) {
    int styleId     = palette->getPage(0)->getStyleId(i);
    TColorStyle *cs = palette->getStyle(styleId);
    if (!cs) continue;
    if (TBlackCleanupStyle *blackStyle = dynamic_cast<TBlackCleanupStyle *>(cs)) {
      TargetColor tc(
          blackStyle->getMainColor(), styleId, (int)blackStyle->getBrightness(),
          noAntialias ? 100 : (int)blackStyle->getContrast(),
          blackStyle->getColorThreshold(), blackStyle->getWhiteThreshold());
      m_colors.push_back(tc);
    } else if (TColorCleanupStyle *colorStyle = dynamic_cast<TColorCleanupStyle *>(cs)) {
      TargetColor tc(colorStyle->getMainColor(), styleId,
                     (int)colorStyle->getBrightness(),
                     noAntialias ? 100 : (int)colorStyle->getContrast(),
                     colorStyle->getHRange(), colorStyle->getLineWidth());
      m_colors.push_back(tc);
    }
  }
}
```

(`toonzlib/cleanuppalette.cpp:94-123`)

The interesting part is not the cleanup workflow itself — Pixhaus does not have that workflow at all — but the model that **palette reduction is a per-target-color optimization** with brightness, contrast, hue range, and white-threshold knobs. Pixhaus's GIF/WebP export today uses a one-pass quantizer; a future "quantize to a specific named palette" feature (already implicit in the AI cleanup verb) would benefit from this knob set.

**Rust adaptation sketch.** Optional. Worth holding in reserve until users actually complain about export-time palette mapping. **Effort: S if simply borrowing the knob shape; M if implementing the full per-target-color matching.**

### Brush stroke rasterization — `toonz/sources/toonzlib/rasterstrokegenerator.cpp`

The relevant detail in this 18 KB file is not the rasterizer itself — Pixhaus is pixel-grid, not stroke-based, so the rasterizer would not port. The relevant detail is the *midpoint smoothing* used when accumulating brush points:

```cpp
void RasterStrokeGenerator::add(const TThickPoint &p) {
  TThickPoint pp = p;
  TThickPoint mid((m_points.back() + pp) * 0.5,
                  (p.thick + m_points.back().thick) * 0.5);
  m_points.push_back(mid);
  m_points.push_back(pp);
}
```

(`toonzlib/rasterstrokegenerator.cpp:41-47`)

Each input point inserts itself plus a midpoint between it and the previous. This is the cheapest possible stroke stabilization — a kindergarten-grade chaikin's algorithm — and it materially smooths jittery tablet input. Pixhaus's brush state lives in `app/` and is not surfaced in `core/`; there is no stroke-smoothing pass before the brush dabs hit the buffer.

**Rust adaptation sketch.** A two-line helper in `core/src/canvas/tools/` (or wherever the brush bookkeeping lives) that wraps the user-input point stream. **Effort: S.** Lowest-hanging fruit in this entire document — except the tablet-pressure dimension is genuinely missing, so even after smoothing the input is one-dimensional.

## What we deliberately don't take

Several large categories of OpenToonz code are out of scope and should stay out of scope.

**The Qt UI layer.** `toonz/sources/toonzqt/` and `toonz/sources/toonz/` are Qt Widgets + Signal/Slot. Pixhaus is Tauri 2 + Solid.js. Nothing here ports. The closest reuse opportunity — Qt-painted brush previews — would also fight Pixhaus's WebGL2 viewport. Leave it.

**The vector-on-raster level model.** Toonz cels can be vector, raster, or both at once. `tvectorimage/` and the `TXshLevel` machinery in `toonzlib/` exist to coordinate this. Pixhaus is raster-only by design — `CLAUDE.md` § "What Pixhaus is not" is explicit. Adopting the dual model is a rearchitecture of B2, not a refinement.

**Multi-engine film concerns.** OpenToonz exports for film pipelines (EXR sequences, multipass renders, color-management workflows). Pixhaus targets Unity 2022.3+ and exports sprite sheets. The export model in `io/aseprite/` and `io/export/` is correct for that target and should not pick up film concerns.

**The full `thirdparty/` tree.** OpenToonz vendors Boost, libpng, libjpeg-turbo, libtiff, libmypaint, GLEW, GLUT, kiss_fft, OpenBLAS, SuperLU, tinyexr, Lz4, plus platform-specific WinTab and QuickTime shims. Pixhaus already has matching Rust-ecosystem choices for everything we need (`image`, `rmp-serde`, `zstd`, `palette`, `rayon`). The OpenToonz `thirdparty/` notices stay in OpenToonz; we do not transitively pull them in.

**GPL/LGPL/AGPL-tainted code.** OpenToonz itself is BSD-3, but specific files in `thirdparty/` carry their own licenses. Before adapting any snippet, the contributor must verify the per-file license header and confirm BSD-3-compatibility. The MyPaint brush integration is the obvious example: `thirdparty/libmypaint/` is LGPL — we do not pull from it.

## License and attribution

The verbatim BSD-3 grant from `/Users/luismorales/project/pixhaus-app/opentoonz/LICENSE.txt:25-31`:

> Redistribution and use in source and binary forms, with or without modification, are permitted provided that the following conditions are met:
>
> 1. Redistributions of source code must retain the above copyright notice, this list of conditions and the following disclaimer.
> 2. Redistributions in binary form must reproduce the above copyright notice, this list of conditions and the following disclaimer in the documentation and/or other materials provided with the distribution.
> 3. Neither the name of the copyright holder nor the names of its contributors may be used to endorse or promote products derived from this software without specific prior written permission.

Two consequences for Pixhaus:

- **Ideas are free.** Reading the OpenToonz source to understand how morphological AA works and then writing an independent Rust implementation does not require attribution. Copyright covers expression, not ideas.
- **Structural ports require attribution.** Where the Rust code mirrors the OpenToonz function decomposition, variable naming, or control flow, the BSD-3 conditions apply. The convention we will follow:
  - The adapted file carries a module-level comment: `// Adapted from OpenToonz <path-in-opentoonz> under BSD-3-Clause. See THIRD_PARTY_NOTICES.md.`
  - The repo-level `THIRD_PARTY_NOTICES.md` (to be created when the first adaptation lands) carries the full BSD-3 text plus the Dwango copyright line.
  - Adapted code does not invoke OpenToonz, Toonz, Dwango, or Studio Ghibli in product-facing marketing — clause 3 forbids endorsement.

The first stream that adapts code is the one that creates `THIRD_PARTY_NOTICES.md`. This document does not — it adapts no code.

## Proposed follow-up streams

Seven candidates. Each is a one-paragraph scope sketch with a rough size estimate. None is a commitment — they are inputs to a planning conversation. Stream numbers (S53–S59) extend the existing sequence and do not appear in `docs/planning/work/streams.md` yet.

**S53 — Palette pages and palette animation.** Extend `core/src/project/palette.rs` with `PalettePage` (grouped style views) and `PaletteAnimation` (per-style keyframed colors over a frame range). Wire the UI in `ui/src/panels/palette/` to read/write pages and to scrub animation in the timeline. The pages dimension is mechanical; the animation dimension is a B2 schema extension and warrants its own bedrock revision review. Reference: `include/tpalette.h:94-194`. **Size: M for pages, L if animation lands together.**

**S54 — Missing blend modes.** Add `LinearBurn`, `DarkerColor`, `LinearDodge`, `LighterColor`, `VividLight`, `LinearLight`, `PinLight`, `HardMix` to `core/src/project/blend.rs` and implement them in `core/src/canvas/blend.rs`. Document the `.aseprite` round-trip fallback (export downgrades to `Normal`). Reference: `stdfx/igs_color_blend.cpp` blend-mode table at top of file. **Size: S.**

**S55 — Morphological anti-aliasing for transforms.** A new `core/src/transforms/antialias.rs` exposing edge-aware AA as an alternative to bilinear in rotate/scale. Threshold and softness as user-facing parameters. Reference: `common/trop/tantialias.cpp`. **Size: M.**

**S56 — Gap-closing magic wand.** Extend `core/src/selection/algorithms.rs` (or a sibling module) with a skeleton-LUT-based gap-closing pass that runs before flood-fill. UI surface: a "max gap (px)" slider in the magic-wand options. Reference: `common/trop/tautoclose.cpp`. **Size: M.**

**S57 — Procedural inbetween fallback.** Add a `Procedural` mode to `ai/src/verbs/inbetween/` that runs variance-rejected weighted averaging between two frames with no backend. Either as a fallback when no backend is configured, or as a "preview" mode artists can run before paying for the AI call. Reference: `common/tvrender/tinbetween.cpp:21-98`. **Size: M.**

**S58 — Centerline vectorization crate.** New crate `vectorize/` (alongside `core/`, `io/`, `ai/`) exposing raster → vector centerline extraction. Feeds the cleanup verb's procedural path and an eventual SVG export. Largest item; defer until v1 user feedback confirms demand. Reference: `toonzlib/tcenterlinevectorizer.cpp`. **Size: L.**

**S59 — SIMD hot-path audit.** Not a port, an audit. Survey `core/src/canvas/` and `core/src/transforms/` hot loops; measure baselines with `criterion`; evaluate `std::simd` or `wide` per loop. Output is a follow-up plan, not code. Reference: shape of `common/trop/quickput.cpp`. **Size: M for the audit, L if it triggers a buffer rewrite.**

If a maintainer agrees, the next step is to add these stream entries to `docs/planning/work/streams.md` in a separate PR — outside the scope of this document.

## References

- OpenToonz source clone: `/Users/luismorales/project/pixhaus-app/opentoonz/` (no upstream commit pinned)
- OpenToonz license: `/Users/luismorales/project/pixhaus-app/opentoonz/LICENSE.txt` (BSD-3-Clause)
- OpenToonz feature/UX survey (sibling doc): `docs/planning/frame-by-frame/opentoonz.md`
- Pixhaus existing palette: `core/src/project/palette.rs`
- Pixhaus existing blend modes: `core/src/project/blend.rs` + `core/src/canvas/blend.rs`
- Pixhaus existing transforms: `core/src/transforms/`
- Pixhaus existing selection: `core/src/selection/algorithms.rs`
- Pixhaus inbetween verb: `ai/src/verbs/inbetween/mod.rs`
- Pixhaus cleanup verb: `ai/src/verbs/cleanup/mod.rs`
- Reshetov, "Morphological Antialiasing" — Intel Labs (cited in `common/trop/tantialias.cpp:5`)
- Project planning conventions: `CLAUDE.md`, `.claude/skills/pixhaus-claude-code-workflow/SKILL.md`
