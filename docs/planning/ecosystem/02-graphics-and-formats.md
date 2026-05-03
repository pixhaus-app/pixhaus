# Rust Graphics, GPU, and File Format Crates Reference - Pixhaus (May 2026)

**Date:** May 2, 2026  
**Target:** Tauri 2.x desktop app (Rust backend + TypeScript/Solid frontend + WebGL2 canvas)  
**Scope:** Image processing, GPU APIs, color management, file formats, serialization

This document covers 50+ Rust crates for pixel art editing: image processing, GPU/CPU rendering, color and palette operations, file format support (PNG, JPEG, GIF, WebP, AVIF, Aseprite, PSD, TMX), text rendering, vector graphics, geometry, and serialization. Each entry includes current status (May 2026), maintenance level, and Pixhaus-specific use cases mapped to streams S01-S12 and S14-S20.

---

## Table of Contents

1. [Image Processing](#image-processing)
2. [GPU Rendering and Compute](#gpu-rendering-and-compute)
3. [CPU-Based Rendering](#cpu-based-rendering)
4. [Color and Palette Management](#color-and-palette-management)
5. [Vector Graphics and SVG](#vector-graphics-and-svg)
6. [Text and Font Rendering](#text-and-font-rendering)
7. [Image File Formats](#image-file-formats)
8. [Sprite-Specific and Aseprite](#sprite-specific-and-aseprite)
9. [Photoshop and PSD](#photoshop-and-psd)
10. [Tilemaps and Tiled Format](#tilemaps-and-tiled-format)
11. [Geometry and Linear Algebra](#geometry-and-linear-algebra)
12. [Compression and Serialization](#compression-and-serialization)

---

## Image Processing

### image

- **Purpose:** The de facto Rust image codec library. Encode and decode PNG, JPEG, GIF, WebP, TIFF, BMP, ICO, PNM, and others. Basic image operations (crop, thumbnail, rotate, flip).
- **Crates.io:** https://crates.io/crates/image
- **Docs:** https://docs.rs/image/latest/image/
- **Repo:** https://github.com/image-rs/image
- **License:** MIT or Apache-2.0
- **Maintenance (May 2026):** Active. Version 0.25.9 (November 2025). ~5.5M monthly downloads.
- **When to use:** Pixhaus relies on `image` for all standard file format I/O except Aseprite/PSD (S10 sprite sheet export, S11 animated GIF/WebP, S07 .pixhaus format fallback for simple PNG export). Load/save PNG, JPEG, GIF. The crate is the gold standard and has no competitor.
- **Alternatives:** `image-webp` (WebP only), format-specific crates (not worth maintaining separately).
- **Notes:**
  - Version 0.25.x is the current stable line. Supports full metadata reading (EXIF, ICC profiles).
  - Plugin API for custom formats (Pixhaus could register a `.pixhaus` decoder, though binary format is better handled via dedicated code).
  - Does not include advanced filters or effects; use `imageproc` for those.
  - No GPU acceleration; pure CPU.
- **Pixhaus streams using it:** S07, S10, S11, S14 (texture loading for preview), S49 (testing)

### imageproc

- **Purpose:** Image processing operations: Gaussian blur, median filter, Laplacian sharpening, edge detection, drawing primitives (lines, rectangles, circles, text), morphological ops, template matching.
- **Crates.io:** https://crates.io/crates/imageproc
- **Docs:** https://docs.rs/imageproc/latest/imageproc/
- **Repo:** https://github.com/image-rs/imageproc
- **License:** MIT or Apache-2.0
- **Maintenance (May 2026):** Active. Version 0.24+ (released April 2026). Part of the image-rs ecosystem.
- **When to use:** S01 (pixel buffer helpers for edge detection if needed for selection algorithms), S14 (canvas debugging/visualization), S27 (cleanup verb — snap to palette via median filter). Not a core dependency; Pixhaus implements most ops directly on PixelBuffer.
- **Alternatives:** Write custom filter code (recommended for pixel art where each filter must preserve palette discipline).
- **Notes:**
  - Filters operate on `image::ImageBuffer<P>` types, compatible with `image` crate output.
  - Optional `text` feature for text rendering on images (uses `rusttype`; inferior to cosmic-text or fontdue).
  - No SIMD acceleration in most filters; use `fast_image_resize` for resizing specifically.
- **Pixhaus streams using it:** S14 (optional, mostly pass)

### fast_image_resize

- **Purpose:** SIMD-accelerated image resizing. Nearest-neighbor, box, bilinear, and catmull-rom filters. Multi-threaded via rayon. Handles u8, u16, i32, f32 per channel; RGB, RGBA, LA, etc.
- **Crates.io:** https://crates.io/crates/fast_image_resize
- **Docs:** https://docs.rs/fast_image_resize/latest/fast_image_resize/
- **Repo:** https://github.com/Cykooz/fast_image_resize
- **License:** MIT or Apache-2.0
- **Maintenance (May 2026):** Active. Version 0.3.x. Benchmarks show AVX2 (x86_64), NEON (ARM), WASM SIMD128 support.
- **When to use:** S04 (transform operations, especially scaling). When the user scales a sprite 2x or 0.5x at runtime, use this. Pixel-art note: use nearest-neighbor filter to preserve blocky aesthetic; bilinear is OK for previews.
- **Alternatives:** `image` crate's `thumbnail()` (slower, good for one-off exports), write custom nearest-neighbor scaler (overkill).
- **Notes:**
  - No colorspace conversion during resize (preserves sRGB gamma).
  - Supports colorspace mappers (sRGB, gamma 2.2) via `PixelComponentMapper`.
  - Thread-pool resizing via rayon feature.
  - Outperforms libvips equivalents on some architectures.
- **Pixhaus streams using it:** S04 (transform scaling), S10 (sprite sheet packing resize), S14 (preview scaling)

### texture-packer / sprite-pack / guillotiere

- **Purpose:** Rectangle bin-packing for sprite atlases. Skyline and guillotière algorithms to pack many small sprites into one large texture.
- **Crates.io:** 
  - `texture-packer`: https://crates.io/crates/texture-packer
  - `guillotiere`: https://crates.io/crates/guillotiere
  - (sprite-pack not on crates.io; alternative implementations exist)
- **Docs:** 
  - `texture-packer`: https://docs.rs/texture-packer/latest/texture_packer/
  - `guillotiere`: https://docs.rs/guillotiere/latest/guillotiere/
- **Repo:** 
  - `texture-packer`: https://github.com/hassieswift621/texture-packer
  - `guillotiere`: https://github.com/nical/guillotiere
- **License:** MIT or Apache-2.0
- **Maintenance (May 2026):** 
  - `texture-packer`: Maintained, but updates are infrequent (last release Jan 2024).
  - `guillotiere`: Active (maintained by Nical, the author of lyon; part of Linebender ecosystem work).
- **When to use:** S10 (PNG sprite sheet export). When exporting an animation to a sprite sheet, pack frames tightly. Guillotière is newer and often yields tighter packing than skyline.
- **Alternatives:** Implement maxrects or skyline yourself (not recommended; existing crates are solid), use external TexturePacker tool (overkill for build pipeline).
- **Notes:**
  - `guillotiere` is simpler and more performant; recommend it over `texture-packer`.
  - Input: list of rectangles with IDs. Output: packed rectangles with XY offsets.
  - No image manipulation; just rectangle layouts. Pixhaus supplies the actual image compositing.
- **Pixhaus streams using it:** S10 (sprite sheet packing), S12 (tilemap sprite sheet generation)

### nine-patch / nine_slice

- **Purpose:** Nine-patch (NLP) image scaling — stretches edges and corners independently. Used in UI and game design to scale buttons, panels, etc. without distortion.
- **Crates.io:** 
  - `nine-patch`: https://crates.io/crates/nine-patch
  - `ninepatch`: https://crates.io/crates/ninepatch
- **Docs:** 
  - `nine-patch`: https://docs.rs/nine-patch/latest/nine_patch/
- **Repo:** 
  - `nine-patch`: https://github.com/kettle11/nine-patch
- **License:** MIT or Apache-2.0
- **Maintenance (May 2026):** Niche crate. Active but infrequent updates.
- **When to use:** S10 (UI sprite export, if Pixhaus supports NLP encoding), S15 (brush/tool panel scaling). Low priority. Pixel art often doesn't use NLP (static UI), but if Pixhaus ships a theme system with scalable UI sprites, this is the tool.
- **Alternatives:** Manual edge/corner detection (tedious), skip (Pixhaus doesn't need NLP if the UI is resolution-independent CSS).
- **Notes:**
  - NLP format: regions defined by 1-pixel lines in the source image (or metadata). This crate reads and applies them.
  - Both crates are minimal; pick whichever has better docs/examples for your use.
- **Pixhaus streams using it:** S10 (maybe, stretch goal)

---

## GPU Rendering and Compute

### wgpu

- **Purpose:** Safe, portable cross-platform graphics API. Rust binding to WebGPU, targeting Vulkan, Metal, DirectX 12, OpenGL ES, WebGL2, WebGPU in browsers. Modern compute shader support.
- **Crates.io:** https://crates.io/crates/wgpu
- **Docs:** https://docs.rs/wgpu/latest/wgpu/
- **Repo:** https://github.com/gfx-rs/wgpu
- **License:** MIT or Apache-2.0
- **Maintenance (May 2026):** Active. Version 0.20.x (2025). MSRV: 1.87 (Rust 1.93 for tests/examples). Part of gfx-rs and used by major projects (Bevy, Dioxus, etc.).
- **When to use:** S14 (canvas viewport compositing of layers into tiles), optional compute for tile-based dirty rendering. Pixhaus's canvas backend (TypeScript + WebGL2) will not directly use wgpu; however, the Rust core can use wgpu for tile compositing if performance requires it (e.g., compositing 50 layers into a 256x256 tile in <1ms). For now, CPU-side compositing (rayon + SIMD blend ops) is likely sufficient; defer wgpu usage to a later optimization pass.
- **Alternatives:** `pixels` (higher-level, single framebuffer only), CPU-only (feasible for pixel art), directx-rs / metal-rs / vulkan (lower-level, more complex).
- **Notes:**
  - WebGPU spec is still WD (working draft) as of May 2026. wgpu tracks the spec but may diverge on details.
  - WGSL (WebGPU Shading Language) is mandatory; no GLSL/HLSL.
  - Zero-cost abstraction when compiled to GPU-native targets (Vulkan, Metal, DX12); WebGL2 incurs some overhead.
  - Excellent for compute; prefix-sum algorithms, texture filtering, etc.
- **Pixhaus streams using it:** S14 (optional, deferred), S21 (AI inference on GPU, if needed)

### pixels

- **Purpose:** Minimal GPU-backed pixel frame buffer. Expose a single 2D pixel buffer (u32 per pixel, 0RGB format). Render to screen with wgpu backend. Trivial API.
- **Crates.io:** https://crates.io/crates/pixels
- **Docs:** https://docs.rs/pixels/latest/pixels/
- **Repo:** https://github.com/parasyte/pixels
- **License:** MIT or Apache-2.0
- **Maintenance (May 2026):** Active. Version 0.15.0. Battle-tested, ~369k downloads.
- **When to use:** If Pixhaus wants to move away from WebGL2 on the frontend and instead render pixel data from Rust to a native window, `pixels` is the go-to. Currently not applicable (Pixhaus uses Tauri webview + Solid.js UI). But if a standalone native Pixhaus client is built later, this is the tool.
- **Alternatives:** `softbuffer` (CPU-backed, more portable), wgpu directly (more control, more complexity).
- **Notes:**
  - Powered by wgpu under the hood.
  - Supports custom shaders for special effects.
  - Minimal boilerplate; great for learning GPU graphics.
- **Pixhaus streams using it:** None currently (S14 uses WebGL2 via Tauri webview)

### softbuffer

- **Purpose:** CPU-backed pixel buffer. Write pixels from Rust, no GPU required. Cross-platform window integration via `raw-window-handle`.
- **Crates.io:** https://crates.io/crates/softbuffer
- **Docs:** https://docs.rs/softbuffer/latest/softbuffer/
- **Repo:** https://github.com/rust-windowing/softbuffer
- **License:** MIT or Apache-2.0
- **Maintenance (May 2026):** Active. Part of rust-windowing org.
- **When to use:** Fallback if GPU is unavailable (VMs, old hardware, misconfigured drivers). For Pixhaus, relevant only in a native (non-Tauri) version. Acceptable for pixel art editing (low framerate is OK; pixel-pushing is not GPU-critical).
- **Alternatives:** GPU-backed (`pixels`, wgpu), or custom framebuffer submission per OS.
- **Notes:**
  - Pixel format is u32 (0RGB, zero-byte padding).
  - No GPU features; synchronous buffer swap.
  - Cross-platform via `raw-window-handle` crate.
- **Pixhaus streams using it:** None currently (S14 uses webview)

---

## CPU-Based Rendering

### tiny-skia

- **Purpose:** CPU-based 2D vector path rendering. Fills, strokes, gradients, patterns, clipping, image compositing, anti-aliasing. No GPU required. Pure CPU.
- **Crates.io:** https://crates.io/crates/tiny-skia
- **Docs:** https://docs.rs/tiny-skia/latest/tiny_skia/
- **Repo:** https://github.com/linebender/tiny-skia
- **License:** Linebender dual license (MIT or Apache-2.0)
- **Maintenance (May 2026):** Active. Linebender project. Version 0.12+ (2025). Part of resvg ecosystem.
- **When to use:** S27 (cleanup verb — vector-based antialiasing removal if needed), S15 (brush engine, non-pixel modes like gradient / airbrush can use tiny-skia to render shapes, then snap to palette). Low priority for core pixel art; more relevant if Pixhaus ships vector editing tools.
- **Alternatives:** Cairo (C library, no Rust binding is popular), Skia (C++, large), implement custom rasterizer (overkill).
- **Notes:**
  - Scope designed for SVG rendering; excellent for path ops.
  - Algorithms borrowed from Google Skia; pixel-perfect output.
  - `tiny-skia-path` provides path ops independently of rasterization (stroking, dashing, Bézier math).
  - Not suitable for real-time 3D; perfect for static 2D rendering.
- **Pixhaus streams using it:** S15 (optional, brush engine), S27 (optional, cleanup)

### vello

- **Purpose:** GPU compute-centric 2D vector renderer. Modern GPU-accelerated vector graphics (fills, strokes, gradients, text). Part of Linebender. Three implementations: full GPU compute, CPU fallback, hybrid.
- **Crates.io:** https://crates.io/crates/vello
- **Docs:** https://docs.rs/vello/latest/vello/ (linked from https://github.com/linebender/vello)
- **Repo:** https://github.com/linebender/vello
- **License:** Apache-2.0 or MIT
- **Maintenance (May 2026):** Active. Linebender project. Recent updates (Dec 2025 TMIL reports show active development).
- **When to use:** S14 (canvas overlay rendering — grid, marching ants, brush previews can use vello for crisp vector graphics). S15 (advanced brush shapes). Not a core replacement for pixel buffer compositing, but a companion for UI overlays. Also applicable to S21/S22 if AI-generated vector layers are supported (future).
- **Alternatives:** tiny-skia (CPU fallback), wgpu directly (lower-level, more control), direct WebGL2 shaders (frontend only).
- **Notes:**
  - Requires GPU with compute shader support. CPU fallback available (but slower than tiny-skia).
  - Experimental as of early 2026; API may change.
  - Better performance than tiny-skia on modern GPUs; worse on old/integrated GPUs.
  - Three implementations (full GPU, CPU, hybrid) available; pick based on target hardware.
- **Pixhaus streams using it:** S14 (optional, deferred to optimization pass), S15 (optional)

---

## Color and Palette Management

### palette

- **Purpose:** Color space conversions and calculations. Supports HSL, HSV, HWB, CIE L*a*b*, CIE L*C*h, Okhsl, Oklab, Oklch, CIE CAM16, Luma, and many others. Linear and non-linear. Type-safe.
- **Crates.io:** https://crates.io/crates/palette
- **Docs:** https://docs.rs/palette/latest/palette/
- **Repo:** https://github.com/Ogeon/palette
- **License:** MIT or Apache-2.0
- **Maintenance (May 2026):** Active. Version 0.7.6 (2025).
- **When to use:** S02 (color and palette ops). Palette is essential for indexed-mode work: indexed↔RGBA conversion, color harmony (split-complement, triad, tetrad, analogous), palette swap operations, color ramp generation. Palette's type-system enforces correctness.
- **Alternatives:** Ad-hoc color math (error-prone), basic `Rgb::from_hex()` parsing (insufficient).
- **Notes:**
  - Type parameters for precision (f32, f64) and RGB standards (sRGB, linear, custom white point).
  - Supports user-defined color spaces via trait impls.
  - Zero runtime overhead; conversions are compile-time resolved.
  - Excellent for color harmony tools.
- **Pixhaus streams using it:** S02 (core), S18 (palette panel color picker), S27 (palette cleanup)

### colorgrad

- **Purpose:** Color gradients. Linear interpolation between colors across the spectrum (HSV, RGB, CIE L*a*b*, etc.). Preset gradients (viridis, turbo, etc.).
- **Crates.io:** https://crates.io/crates/colorgrad
- **Docs:** https://docs.rs/colorgrad/latest/colorgrad/
- **Repo:** https://github.com/mazznoer/colorgrad
- **License:** MIT or Apache-2.0
- **Maintenance (May 2026):** Active. Version 1.5+ (2025).
- **When to use:** S02 (color ramp generation for palettes), S15 (gradient tool / airbrush). If Pixhaus ships a palette-generation-from-image tool, colorgrad can help interpolate dominant colors.
- **Alternatives:** Manual lerp in palette crate (possible but verbose).
- **Notes:**
  - Presets (viridis, plasma, etc.) are well-tested.
  - Supports different color spaces for interpolation.
  - Lightweight; no heavy dependencies.
- **Pixhaus streams using it:** S02 (ramp generation), S15 (gradient tool)

### color (Linebender)

- **Purpose:** W3C-aligned color types and conversions. Modern replacement for older color crates. Part of Linebender's push toward standards-aligned graphics.
- **Crates.io:** https://crates.io/crates/color
- **Docs:** https://docs.rs/color/latest/color/
- **Repo:** https://github.com/linebender/color
- **License:** Apache-2.0 or MIT
- **Maintenance (May 2026):** Active (Linebender project, early stage). Version 0.x (2025).
- **When to use:** S02 (future-proofing). If Pixhaus wants to align with W3C CSS Color specs (OKLch, relative colors, etc.), this is the official Rust crate. Currently, `palette` is more mature; `color` is watch-for-2026+ adoption.
- **Alternatives:** `palette` (mature, more features), ad-hoc color math.
- **Notes:**
  - Linebender's effort to unify color handling across their ecosystem (vello, cosmic-text, etc.).
  - API still stabilizing (0.x versions).
  - Worth migrating to once it reaches 1.0 and gains more tooling.
- **Pixhaus streams using it:** S02 (optional, future)

### prisma

- **Purpose:** Perceptually uniform color space and color manipulation. Focus on human perception vs. computer math.
- **Crates.io:** https://crates.io/crates/prisma
- **Docs:** https://docs.rs/prisma/latest/prisma/
- **Repo:** https://github.com/ralfbiedert/prisma
- **License:** MIT or Apache-2.0
- **Maintenance (May 2026):** Niche crate. Active but infrequent updates.
- **When to use:** S18 (palette panel color picker, if human perception is prioritized). For pixel art, perceptual uniformity is less critical than palette discipline. Low priority.
- **Alternatives:** `palette` (better maintained, broader feature set).
- **Notes:**
  - Good for UI color pickers that feel "right" to users.
  - Smaller crate; fewer color spaces than `palette`.
- **Pixhaus streams using it:** S18 (optional, deferred)

---

## Vector Graphics and SVG

### resvg

- **Purpose:** SVG rendering library. Pure Rust. Deterministic output (pixel-for-pixel consistency across platforms). Supports most SVG features except scripts/animations.
- **Crates.io:** https://crates.io/crates/resvg
- **Docs:** https://docs.rs/resvg/latest/resvg/
- **Repo:** https://github.com/linebender/resvg
- **License:** MPL-2.0 (public domain)
- **Maintenance (May 2026):** Active. Linebender project. Version 0.47+ (2025).
- **When to use:** S28 (tile generation verb — if users import SVG logos to turn into tileset art). S15 (brush shapes from SVG). S27 (cleanup, if vector edge-snapping is needed). Not a core stream, but a nice-to-have for import workflows.
- **Alternatives:** librsvg (C library, no popular Rust binding), Inkscape (overkill for this use case).
- **Notes:**
  - Uses tiny-skia for rendering; excellent output quality.
  - No animation support (SVG `<animate>` ignored).
  - Scripts and styles minimal support.
  - Command-line tool also available (`resvg` CLI).
- **Pixhaus streams using it:** S28 (optional, verb implementation), S15 (optional)

### usvg

- **Purpose:** SVG parser and tree representation. Simplifies SVG documents (resolves styles, applies defaults, flattens groups). Foundation for resvg.
- **Crates.io:** https://crates.io/crates/usvg
- **Docs:** https://docs.rs/usvg/latest/usvg/
- **Repo:** https://github.com/linebender/resvg (usvg is part of resvg repo)
- **License:** MPL-2.0
- **Maintenance (May 2026):** Active (part of resvg).
- **When to use:** S28 (tile generation, if vector parsing is needed before rasterization). Usually paired with resvg (which uses usvg internally).
- **Alternatives:** Manual SVG parsing (tedious), svg crate (lighter, less feature-complete).
- **Notes:**
  - Usvg normalizes SVG to a canonical tree; easier to work with than raw XML.
  - Can be used independently for analysis (e.g., extracting bounding boxes).
- **Pixhaus streams using it:** S28 (optional, implied by resvg)

---

## Text and Font Rendering

### fontdue

- **Purpose:** Fast CPU-based font rasterization. No harfbuzz dependency. Renders individual glyphs to bitmaps. Lowest latency for glyph-by-glyph rendering.
- **Crates.io:** https://crates.io/crates/fontdue
- **Docs:** https://docs.rs/fontdue/latest/fontdue/
- **Repo:** https://github.com/mooman219/fontdue
- **License:** MIT
- **Maintenance (May 2026):** Active. Version 0.10+ (2025).
- **When to use:** S15 (brush engine, if text labels on brushes/tools), S14 (canvas debug text), S18 (palette panel text). Not a core stream; UI text is handled by TypeScript/Solid. Use fontdue if pixel-art-style crisp text rendering is desired in the Rust side for previews.
- **Alternatives:** `ab_glyph` (similar), `cosmic-text` (higher-level, includes shaping), `swash` (more features, slower).
- **Notes:**
  - Pure Rust; no C dependencies.
  - Fast glyph-by-glyph lookup.
  - No shaping (ligatures, bidirectional text) — suitable for simple English labels.
  - Author recommends cosmic-text for full-featured text layout.
- **Pixhaus streams using it:** S15 (optional), S14 (optional)

### swash

- **Purpose:** Font introspection, complex text shaping (ligatures, diacritics, bidirectional text), and glyph rendering. Cross-platform. Pure Rust.
- **Crates.io:** https://crates.io/crates/swash
- **Docs:** https://docs.rs/swash/latest/swash/
- **Repo:** https://github.com/dfrg/swash
- **License:** MIT or Apache-2.0
- **Maintenance (May 2026):** Active. Linebender project. Foundation for cosmic-text.
- **When to use:** S15 (if Pixhaus UI supports fancy text rendering), S14 (advanced text overlays). Most text in Pixhaus is handled by TypeScript; only relevant for Rust-side debug/preview rendering.
- **Alternatives:** `cosmic-text` (higher-level wrapper), fontdue (simpler, no shaping).
- **Notes:**
  - More feature-rich than fontdue; includes shaping.
  - Used as the glyph provider in cosmic-text.
  - Good for complex text (CJK, RTL).
- **Pixhaus streams using it:** S15 (optional), S14 (optional)

### ab_glyph

- **Purpose:** Glyph rasterization. Newer, faster rewrite of rusttype. CPU-based font rendering to bitmaps.
- **Crates.io:** https://crates.io/crates/ab_glyph
- **Docs:** https://docs.rs/ab_glyph/latest/ab_glyph/
- **Repo:** https://github.com/alexheretic/ab-glyph
- **License:** MIT
- **Maintenance (May 2026):** Maintained. Version 0.2.x (2025).
- **When to use:** Alternative to fontdue. Lighter footprint; similar performance. Choose based on feature needs (fontdue vs. ab_glyph trade-offs).
- **Alternatives:** fontdue, cosmic-text.
- **Notes:**
  - Simpler API than swash.
  - No shaping.
- **Pixhaus streams using it:** S15 (optional, alternative to fontdue)

### cosmic-text

- **Purpose:** Complete multi-line text handling. Layout, shaping (HarfRust-based), rendering (via swash). Bidirectional text, ligatures, color emoji. Pure Rust.
- **Crates.io:** https://crates.io/crates/cosmic-text
- **Docs:** https://docs.rs/cosmic-text/latest/cosmic_text/ (also https://pop-os.github.io/cosmic-text/)
- **Repo:** https://github.com/pop-os/cosmic-text
- **License:** MPL-2.0
- **Maintenance (May 2026):** Active. Pop!_OS project. Version 0.13+ (2025). Feature-complete for most text use cases.
- **When to use:** S15 (if Pixhaus implements a text tool for pixel art), S17 (layer panel text rendering, if custom), S14 (canvas text overlays). Most UI is TypeScript, so this is secondary. But if Pixhaus wants crisp, palette-snapped text rendering on the Rust side, cosmic-text is the go-to.
- **Alternatives:** fontdue (lighter, no layout), swash (lower-level).
- **Notes:**
  - Integrates all components: font loading, shaping, layout, rendering.
  - Best-in-class text support for Rust graphics apps.
  - No GPU rendering (CPU rasterization to atlas).
  - Layout supports wrapping, alignment, bidirectional text.
- **Pixhaus streams using it:** S15 (optional, text tool), S17 (optional, panel text), S14 (optional, canvas text)

---

## Image File Formats

### PNG

- **Purpose:** Lossless image compression. Part of the `image` crate ecosystem. Encoders and decoders via `image::codecs::png`.
- **Crates.io:** Included in `image`
- **When to use:** S07 (.pixhaus native format can optionally store PNG tiles for fallback), S10 (sprite sheet export), S11 (animated PNG export, if supported). PNG is Pixhaus's primary sprite sheet format.
- **Maintenance (May 2026):** Stable. PNG encoder/decoder in `image` crate is well-tested.
- **Notes:**
  - Supports Adam7 interlacing, ancillary chunks (gamma, EXIF, etc.).
  - Lossless; suitable for pixel art.
  - No animation support in base PNG (APNG is a separate extension).
- **Pixhaus streams using it:** S10 (core), S07 (fallback), S11 (maybe)

### JPEG

- **Purpose:** Lossy compression. Useful for photographic content but not for pixel art (artifacts). Part of `image` crate.
- **Crates.io:** Included in `image` (encoder via `image::codecs::jpeg`, decoder)
- **When to use:** S10 (sprite sheet export, not recommended; PNG is better). S11 (video frame export, if supported). Avoid for pixel art due to compression artifacts.
- **Maintenance (May 2026):** Stable.
- **Notes:**
  - JPEG 2000 not widely used; classic JPEG baseline is standard.
  - Artifacts are unacceptable for pixel art.
- **Pixhaus streams using it:** S10 (fallback, not recommended), S11 (video frames, optional)

### GIF

- **Purpose:** Animated image format. Lossless per frame, palette-based. Included in `image` crate.
- **Crates.io:** Included in `image`
- **When to use:** S11 (animated GIF export). Pixhaus supplies frame data; `image` crate handles encoding.
- **Maintenance (May 2026):** Stable. GIF is legacy but well-understood.
- **Notes:**
  - Max 256 color palette per frame (can vary frame-to-frame).
  - Dithering required for 256+ color palettes.
  - Widely supported; good for sharing animations.
- **Pixhaus streams using it:** S11 (GIF export with palette quantization)

### WebP

- **Purpose:** Modern lossy/lossless compression. Smaller than PNG/GIF, good for web. VP8/VP9 codec.
- **Crates.io:** 
  - `image-webp`: https://crates.io/crates/image-webp (pure Rust decoder/lossless encoder)
  - `webp`: https://crates.io/crates/webp (wrapper around libwebp-sys, supports lossy)
- **Docs:** 
  - `image-webp`: https://docs.rs/image-webp/latest/image_webp/
  - `webp`: https://docs.rs/webp/latest/webp/
- **License:** MIT or Apache-2.0
- **Maintenance (May 2026):** 
  - `image-webp`: Active, pure Rust.
  - `webp`: Active, maintained by Linebender community.
- **When to use:** S11 (animated WebP export, if supported). WebP is modern and efficient; better than GIF/PNG for smaller file sizes.
- **Alternatives:** GIF (more compatible), PNG (lossless fallback).
- **Notes:**
  - `image-webp` supports lossless only; no lossy encoder (basic codec).
  - `webp` crate supports lossy via libwebp binding (faster compression, larger file).
  - For pixel art, lossless WebP is preferable (image-webp).
- **Pixhaus streams using it:** S11 (animated WebP export, optional)

### AVIF

- **Purpose:** Modern image format (AV1 codec). Excellent compression, superior to WebP. Emerging standard (2025+).
- **Crates.io:** 
  - `ravif`: https://crates.io/crates/ravif (pure Rust encoder, rav1e-based)
  - `libavif`: https://crates.io/crates/libavif (wrapper around libavif C library)
- **Docs:** 
  - `ravif`: https://docs.rs/ravif/latest/ravif/
  - `libavif`: https://docs.rs/libavif/latest/libavif/
- **License:** `ravif`: MIT or Apache-2.0. `libavif`: dual license.
- **Maintenance (May 2026):** 
  - `ravif`: Active. ~2.3M monthly downloads.
  - `libavif`: Maintained.
- **When to use:** S11 (AVIF export, future-proofing). AVIF is emerging as the standard for modern web image delivery. Recommend for sprite sheet export alongside PNG/WebP.
- **Alternatives:** WebP, PNG.
- **Notes:**
  - `ravif` is pure Rust, slightly slower encoding, good for build pipelines.
  - `libavif` uses the official C codec, faster but C dependency.
  - Recommend `ravif` for reproducibility.
  - Browser support: Chrome/Edge 85+, Firefox 113+, Safari 16+. Good coverage by May 2026.
- **Pixhaus streams using it:** S11 (AVIF export, stretch goal)

### OpenEXR (EXR)

- **Purpose:** High dynamic range (HDR) image format. 16-bit and 32-bit float per channel. Used in VFX/animation.
- **Crates.io:** 
  - `exr`: https://crates.io/crates/exr (pure Rust, 100% safe code)
  - `openexr`: https://crates.io/crates/openexr (bindings to ASWF OpenEXR C++ library)
- **Docs:** 
  - `exr`: https://docs.rs/exr/latest/exr/
  - `openexr`: https://docs.rs/openexr/latest/openexr/
- **License:** `exr`: MIT or Apache-2.0. `openexr`: BSD-3-Clause.
- **Maintenance (May 2026):** 
  - `exr`: Active (pure Rust). Note: missing deep data and DWA compression.
  - `openexr`: Maintained by vfx-rs. Bindings to OpenEXR 3.0.5.
- **When to use:** S07 (native .pixhaus format can optionally store HDR layers), S10 (export to EXR for VFX artists). Not core for pixel art, but a nice-to-have for professional workflows.
- **Alternatives:** TIFF with 16-bit (less standardized for animation), PNG (8-bit only).
- **Notes:**
  - `exr` is pure Rust, simpler API, but missing some compression algorithms.
  - `openexr` is more complete, but C dependency.
  - Recommend `exr` for most use cases; use `openexr` if deep data/DWA is critical.
  - EXR is the de facto standard in animation/VFX.
- **Pixhaus streams using it:** S07 (optional, future), S10 (optional, VFX export)

### BMP, ICO, TIFF, PNM

- **Purpose:** Legacy and specialty formats. BMP (uncompressed, simple), ICO (Windows icons), TIFF (multi-page, high-quality), PNM (plaintext images).
- **Crates.io:** Included in `image` crate as `image::codecs::{bmp, ico, tiff, pnm}`
- **When to use:** 
  - BMP: S10 (fallback export, not recommended).
  - ICO: S10 (Windows icon export, if supported).
  - TIFF: S10 (archival export, if needed).
  - PNM: Rare; skip.
- **Maintenance (May 2026):** Stable (part of `image` crate).
- **Notes:**
  - BMP is uncompressed, large files, no advantage over PNG.
  - ICO is Windows-only; limited adoption.
  - TIFF is complex (many variants); use only if compatibility is critical.
- **Pixhaus streams using it:** S10 (optional, fallback formats)

---

## Sprite-Specific and Aseprite

### aseprite-loader

- **Purpose:** Zero-copy parser for Aseprite `.aseprite` (binary format) files. Implements the Aseprite File Format specification.
- **Crates.io:** https://crates.io/crates/aseprite-loader
- **Docs:** https://docs.rs/aseprite-loader/latest/aseprite_loader/
- **Repo:** Unknown (not listed in search results, but docs.rs entry exists)
- **License:** Unknown (check on crates.io)
- **Maintenance (May 2026):** Active, maintained, used in game engines.
- **When to use:** S08 (.aseprite read/write). This is the canonical Aseprite parser. Use it directly; do not rely on JSON export paths.
- **Alternatives:** 
  - `asefile` (similar, different API).
  - `aseprite` crate (ggez version, JSON export only, not recommended).
  - Implement parser from scratch (not recommended; spec is complex).
- **Notes:**
  - Zero-copy design: parses without allocating unnecessary buffers.
  - Handles all chunk types (pixel data, layers, tags, palettes, etc.).
  - Fast and efficient.
- **Pixhaus streams using it:** S08 (core)

### asefile

- **Purpose:** Binary Aseprite file parser. Direct format specification implementation.
- **Crates.io:** https://crates.io/crates/asefile
- **Docs:** https://docs.rs/asefile/latest/asefile/
- **Repo:** Unknown
- **License:** Unknown
- **Maintenance (May 2026):** Active.
- **When to use:** S08 (alternative to aseprite-loader). Similar scope; pick based on API preference.
- **Alternatives:** aseprite-loader.
- **Notes:**
  - Another valid choice; feature parity with aseprite-loader.
- **Pixhaus streams using it:** S08 (optional, alternative to aseprite-loader)

### aseprite (ggez)

- **Purpose:** Aseprite JSON export parser. Loads only JSON output from Aseprite, not the binary format.
- **Crates.io:** https://crates.io/crates/aseprite
- **Docs:** https://docs.rs/aseprite/latest/aseprite/
- **Repo:** https://github.com/ggez/aseprite
- **License:** MIT or Apache-2.0
- **Maintenance (May 2026):** Maintained, but JSON-only approach is limiting.
- **When to use:** Not recommended. JSON export is inconvenient for users (requires two-step export from Aseprite). S08 should use binary format via aseprite-loader.
- **Alternatives:** aseprite-loader.
- **Notes:**
  - Simpler than binary format parsing, but forces users to manually export JSON.
  - Skip in favor of binary support.
- **Pixhaus streams using it:** S08 (not recommended, prefer aseprite-loader)

### Aseprite Format Reference

- The official Aseprite file format spec is at: https://github.com/aseprite/aseprite/blob/main/docs/ase-file-specs.md
- Pixhaus S08 must implement full read/write support per `docs/aseprite-compat.md` bedrock spec.
- Blend modes (26 standard + custom) must match Aseprite's `src/doc/blend_funcs.cpp` exactly.

---

## Photoshop and PSD

### psd

- **Purpose:** Photoshop `.psd` file parser. Reads layer hierarchy, blend modes, opacity, masks, and pixel data.
- **Crates.io:** https://crates.io/crates/psd
- **Docs:** https://docs.rs/psd/latest/psd/
- **Repo:** https://github.com/chinedufn/psd
- **License:** MIT or Apache-2.0
- **Maintenance (May 2026):** Active. Maintained by chinedufn. Version 0.x (2025).
- **When to use:** S09 (.psd import). Read-only for now; write-side is out of scope per the spec.
- **Alternatives:** Implement from scratch using Adobe's PSD format spec (tedious), skip PSD support entirely (OK; Aseprite import is the priority).
- **Notes:**
  - Comprehensive PSD parsing; layers, groups, blend modes, masks, pixel data.
  - Some advanced features (smart objects, layer effects, adjustment layers) are not fully supported.
  - Can be compiled to WebAssembly.
  - Recommended approach: use the psd crate, accept limitations, document unsupported features.
- **Pixhaus streams using it:** S09 (core)

---

## Tilemaps and Tiled Format

### tiled

- **Purpose:** Tiled map editor `.tmx` and `.tsx` file parser. Supports XML and JSON formats. Layer types, tilesets, object layers, properties.
- **Crates.io:** https://crates.io/crates/tiled
- **Docs:** https://docs.rs/tiled/latest/tiled/
- **Repo:** https://github.com/mapeditor/rs-tiled
- **License:** MIT or Apache-2.0
- **Maintenance (May 2026):** Active. Part of the Tiled editor ecosystem. Minimum TMX version support: 0.13.
- **When to use:** S12 (TMX tilemap export). When Pixhaus exports a tilemap, produce a `.tmx` file that can be imported into Tiled or Unity's SuperTiled2Unity. Also useful for S06 (tilemap layer data structures can leverage tiled types).
- **Alternatives:** `tiled_parse` (nom-based parser, lighter), custom XML parsing (tedious).
- **Notes:**
  - Comprehensive TMX support including tilesets, properties, object layers.
  - Flexible resource reading for WASM and custom loaders.
  - Well-maintained; recommended.
- **Pixhaus streams using it:** S12 (TMX export), S06 (optional, tilemap data structures)

### tiled_parse

- **Purpose:** TMX parser using nom. Alternative to the `tiled` crate.
- **Crates.io:** https://crates.io/crates/tiled_parse
- **Docs:** https://docs.rs/tiled_parse/latest/tiled_parse/
- **Repo:** Unknown
- **License:** Unknown
- **Maintenance (May 2026):** Niche crate. Active but less popular than `tiled`.
- **When to use:** S12 (alternative to `tiled` if nom-based parsing is preferred). Generally, stick with `tiled`.
- **Alternatives:** `tiled`.
- **Notes:**
  - Lighter footprint than `tiled` if custom parser integration is needed.
- **Pixhaus streams using it:** S12 (optional, alternative)

### tmx

- **Purpose:** TMX parser for Tiled maps. Supports embedded textures and tilesets.
- **Crates.io:** https://crates.io/crates/tmx
- **Docs:** https://docs.rs/tmx/latest/tmx/
- **Repo:** https://github.com/swatteau/tmx
- **License:** Unknown
- **Maintenance (May 2026):** Maintained but less popular than `tiled`.
- **When to use:** S12 (alternative to `tiled`). Recommendation: use `tiled` crate (more mature, better maintained).
- **Alternatives:** `tiled`.
- **Notes:**
  - Support for embedded textures is useful for asset bundling.
- **Pixhaus streams using it:** S12 (optional, alternative)

---

## Geometry and Linear Algebra

### glam

- **Purpose:** Simple and fast SIMD-accelerated linear algebra for games and graphics. Vectors, matrices, quaternions. Type system ensures compile-time correctness.
- **Crates.io:** https://crates.io/crates/glam
- **Docs:** https://docs.rs/glam/latest/glam/
- **Repo:** https://github.com/bitshifter/glam-rs
- **License:** MIT or Apache-2.0
- **Maintenance (May 2026):** Active. Version 0.28.x (2025). SIMD on x86, x86_64, ARM (NEON), WASM32.
- **When to use:** S04 (transform operations — rotations, scaling, translations), S14 (canvas viewport math — pan, zoom, viewport-to-world conversions), S16 (selection and transform UI — handle math, bounding boxes). Essential for any 2D geometry.
- **Alternatives:** `nalgebra` (more general, heavier), `euclid` (geometry-focused, lighter), `bevy_math` (game engine variant of glam).
- **Notes:**
  - Vec3A, Vec4, Quat, Mat2, Mat3A, Mat4, Affine2, Affine3A use SIMD (128-bit vectors).
  - Column-major matrix storage; v' = Mv convention.
  - Zero-overhead abstractions; most operations are inlined to CPU instructions.
  - No GPU shader generation; purely CPU math.
  - Best-in-class performance for 2D/3D graphics math in Rust.
- **Pixhaus streams using it:** S04 (core), S14 (core), S16 (core)

### nalgebra

- **Purpose:** General-purpose linear algebra library. Matrices, vectors, decompositions (QR, SVD, Cholesky), solver for linear systems.
- **Crates.io:** https://crates.io/crates/nalgebra
- **Docs:** https://docs.rs/nalgebra/latest/nalgebra/
- **Repo:** https://github.com/dimforge/nalgebra
- **License:** MIT or Apache-2.0
- **Maintenance (May 2026):** Active. Version 0.34.x (2025).
- **When to use:** S33 (auto-mesh-deformation verb, if implemented; requires eigenvalue decomposition or similar advanced linear algebra). Not needed for basic pixel art operations. Prefer `glam` for simple transforms.
- **Alternatives:** `glam` (faster for simple ops, SIMD), `ndarray` (NumPy-like arrays, general data science).
- **Notes:**
  - More comprehensive than glam, but heavier and slower for simple vector ops.
  - Compile-time shape checking via type system.
  - Good for scientific computing, but overkill for graphics.
- **Pixhaus streams using it:** S33 (optional, advanced verb), none in core

### ndarray

- **Purpose:** N-dimensional arrays, similar to NumPy. General data science operations.
- **Crates.io:** https://crates.io/crates/ndarray
- **Docs:** https://docs.rs/ndarray/latest/ndarray/
- **Repo:** https://github.com/rust-ndarray/ndarray
- **License:** MIT or Apache-2.0
- **Maintenance (May 2026):** Active. Version 0.15.x (2025).
- **When to use:** S01 (pixel buffer as a 2D array for batch operations), S23-S36 (AI verbs, if AI models require tensor operations). Not core for basic pixel art, but useful if heavy data science is needed.
- **Alternatives:** `glam` (simpler, for graphics), `tensor` crates (specialized for ML).
- **Notes:**
  - Good for bulk pixel operations and matrix math.
  - Supports advanced indexing and slicing.
- **Pixhaus streams using it:** S01 (optional, if heavy batch ops), S23-S36 (if AI inference requires tensors)

### euclid

- **Purpose:** 2D and 3D geometry types. Rectangles, points, transforms, etc. Type-safe (points vs. vectors distinguished at compile time).
- **Crates.io:** https://crates.io/crates/euclid
- **Docs:** https://docs.rs/euclid/latest/euclid/
- **Repo:** https://github.com/servo/euclid (Servo project)
- **License:** MIT or Apache-2.0
- **Maintenance (May 2026):** Maintained (Servo project). Version 0.22.x (2025).
- **When to use:** S14 (canvas viewport, if strong type distinctions are preferred), S16 (selection bounding boxes, if type safety is critical). Alternative to glam for a lighter, more geometrically-focused approach.
- **Alternatives:** `glam` (SIMD, more comprehensive), ad-hoc math (tedious).
- **Notes:**
  - Type parameters distinguish points from vectors, and logical units from physical pixels.
  - Lighter than glam; no SIMD.
  - Good for precision and correctness; worse for performance.
- **Pixhaus streams using it:** S14 (optional, alternative to glam), S16 (optional)

### lyon

- **Purpose:** 2D geometry, path tessellation, stroke expansion, clipping. Foundation for vector graphics rendering. Pure Rust.
- **Crates.io:** https://crates.io/crates/lyon
- **Docs:** https://docs.rs/lyon/latest/lyon/
- **Repo:** https://github.com/nical/lyon (Linebender project)
- **License:** MIT or Apache-2.0
- **Maintenance (May 2026):** Active. Linebender project. Version 1.0+ (stable).
- **When to use:** S15 (brush engine, if custom shape stroking is needed), S14 (if selection marching ants use path stroking). Foundation for vello and tiny-skia; likely used indirectly.
- **Alternatives:** Implement path ops manually (not recommended), use vello/tiny-skia (higher-level).
- **Notes:**
  - Excellent tessellation algorithms for arbitrary paths.
  - Stroke expansion with configurable line joins/caps.
  - Path clipping.
  - Pure Rust, no C dependencies.
  - Stable and well-tested.
- **Pixhaus streams using it:** S15 (optional, path stroking), S14 (optional, marching ants)

---

## Compression and Serialization

### rmp-serde

- **Purpose:** MessagePack serialization via serde. Compact binary format (~70% of the size of JSON), faster than JSON to deserialize.
- **Crates.io:** https://crates.io/crates/rmp-serde
- **Docs:** https://docs.rs/rmp-serde/latest/rmp_serde/
- **Repo:** https://github.com/3Hren/rmp-serde
- **License:** MIT or Apache-2.0
- **Maintenance (May 2026):** Active. Version 1.1.x (2025).
- **When to use:** S07 (native .pixhaus format serialization of project metadata). MessagePack is compact and human-inspectable (if needed). The `.pixhaus` format uses: magic bytes + version + MessagePack-encoded model + zstd-compressed pixel payloads.
- **Alternatives:** `bincode` (faster, not human-readable), `postcard` (embedded-friendly), serde_json (human-readable, larger).
- **Notes:**
  - Good balance between size, speed, and debuggability.
  - Serde integration makes it easy to drop in.
  - MessagePack is a well-known standard.
- **Pixhaus streams using it:** S07 (native format serialization)

### bincode

- **Purpose:** Fast binary serialization. Smallest output size and fastest encode/decode in benchmarks.
- **Crates.io:** https://crates.io/crates/bincode
- **Docs:** https://docs.rs/bincode/latest/bincode/
- **Repo:** https://github.com/bincode-org/bincode
- **License:** MIT or Apache-2.0
- **Maintenance (May 2026):** Active. Version 1.3.x (stable), 2.0 in beta (breaking changes).
- **When to use:** S07 (alternative to MessagePack for raw speed). If Pixhaus benchmarks show speed is critical and 10% smaller output is necessary, bincode is the answer. For most use cases, MessagePack is fine.
- **Alternatives:** `rmp-serde` (good balance), `postcard` (embedded).
- **Notes:**
  - Bincode v1.3 is stable and widely used.
  - Bincode v2.0 has breaking changes; wait for stabilization.
  - Not self-describing; schema changes are risky.
- **Pixhaus streams using it:** S07 (optional, if speed is critical)

### postcard

- **Purpose:** Compact, embedded-friendly serialization. No std lib required (no_std). ~70% size of MessagePack, slower deserialization.
- **Crates.io:** https://crates.io/crates/postcard
- **Docs:** https://docs.rs/postcard/latest/postcard/
- **Repo:** https://github.com/jamesmunns/postcard
- **License:** MIT or Apache-2.0
- **Maintenance (May 2026):** Active. Version 1.0+ (stable, 2025).
- **When to use:** S07 (if embedded/WASM output is a goal). Pixhaus is not embedded, so postcard is less critical. But if .pixhaus format must support WebAssembly exports, postcard is a good fit.
- **Alternatives:** `bincode`, `rmp-serde`.
- **Notes:**
  - Optimized for embedded systems (microcontrollers, firmware).
  - no_std + no_alloc variants available.
  - Slower deserialization (~1.5x slower than bincode), but still fast.
  - Good size/speed trade-off for constrained environments.
- **Pixhaus streams using it:** S07 (optional, if WASM serialization is needed)

### zstd

- **Purpose:** Zstandard compression. High compression ratio, fast decompression. Modern replacement for gzip.
- **Crates.io:** https://crates.io/crates/zstd
- **Docs:** https://docs.rs/zstd/latest/zstd/
- **Repo:** https://github.com/gyscos/zstd-rs
- **License:** MIT or Apache-2.0 (bindings to Facebook's Zstandard library)
- **Maintenance (May 2026):** Active. Version 0.13.x (2025).
- **When to use:** S07 (native .pixhaus format pixel buffer compression). The spec says: ".pixhaus = magic + version + MessagePack metadata + zstd-compressed pixel payloads." Use zstd for the payloads.
- **Alternatives:** gzip (older, slower), lz4 (faster decompression, less compression), brotli (better compression, slower).
- **Notes:**
  - Trade-off: compression ratio vs. speed. Zstd is balanced.
  - Facebook/Meta standard; widely adopted.
  - Fast decompression is critical for project loading.
  - Configurable compression level (1-22; default 3 is fast).
- **Pixhaus streams using it:** S07 (pixel payload compression)

---

## Summary Table

| Crate | Purpose | Streams | Priority | Notes |
|-------|---------|---------|----------|-------|
| image | Image codecs (PNG, JPEG, GIF, WebP) | S10, S07, S11 | Core | De facto standard; use everywhere |
| imageproc | Filters, drawing, edge detection | S14, S27 | Optional | Lightweight complement to image |
| fast_image_resize | SIMD resizing | S04, S10, S14 | High | Essential for performance |
| texture-packer | Sprite sheet packing (skyline) | S10, S12 | High | Tightly pack frames |
| guillotiere | Rectangle bin-packing | S10, S12 | High | Better than skyline; recommend |
| nine-patch | NLP image scaling | S10, S15 | Low | Stretch goal for UI |
| wgpu | GPU graphics API | S14, S21 | Medium | Optional; CPU sufficient for now |
| pixels | GPU framebuffer | None | Low | For native client (future) |
| softbuffer | CPU framebuffer | None | Low | Fallback GPU option |
| tiny-skia | CPU vector renderer | S15, S27 | Low | Vector path rendering |
| vello | GPU vector renderer | S14, S15 | Medium | Modern GPU alternative to tiny-skia |
| palette | Color spaces and conversions | S02, S18, S27 | Core | Essential for palette ops |
| colorgrad | Color gradients | S02, S15 | Medium | Ramp generation |
| color | W3C-aligned color types | S02 | Low | Future migration candidate |
| prisma | Perceptual color space | S18 | Low | Enhanced color picker (nice-to-have) |
| resvg | SVG rendering | S28, S15 | Low | Import SVG shapes |
| usvg | SVG parser | S28 | Low | Implied by resvg |
| fontdue | Fast glyph rasterization | S15, S14 | Low | Text labels on canvas |
| swash | Font shaping and rendering | S15, S14 | Low | Advanced text (ligatures, CJK) |
| ab_glyph | Glyph rasterization | S15, S14 | Low | Alternative to fontdue |
| cosmic-text | Full text handling (layout, shaping) | S15, S17, S14 | Medium | Complete text solution |
| PNG, JPEG, GIF, WebP, AVIF | File format support | S10, S11 | Core | Via image, ravif, webp crates |
| OpenEXR (exr) | HDR images | S07, S10 | Low | VFX export (nice-to-have) |
| aseprite-loader | Aseprite binary format | S08 | Core | Canonical parser |
| asefile | Aseprite parser alt. | S08 | High | Alternative to aseprite-loader |
| aseprite (ggez) | Aseprite JSON export | S08 | Low | Not recommended; binary is better |
| psd | PSD import | S09 | Medium | Read-only Photoshop support |
| tiled | TMX tilemap format | S12, S06 | High | Tilemap export standard |
| tiled_parse | TMX parser (nom) | S12 | Low | Alternative to tiled |
| tmx | TMX parser alt. | S12 | Low | Alternative to tiled |
| glam | SIMD linear algebra | S04, S14, S16 | Core | Transform math, essential |
| nalgebra | General linear algebra | S33 | Low | Advanced decompositions (future) |
| ndarray | N-D arrays | S01, S23-S36 | Low | Batch ops, AI tensors |
| euclid | Geometry types | S14, S16 | Low | Type-safe alternative to glam |
| lyon | Path tessellation | S15, S14 | Low | Path stroking (implied by vello) |
| rmp-serde | MessagePack serialization | S07 | High | .pixhaus format metadata |
| bincode | Fast binary serialization | S07 | Medium | Speed alternative to rmp-serde |
| postcard | Embedded serialization | S07 | Low | WASM-friendly (future) |
| zstd | Zstandard compression | S07 | High | .pixhaus pixel payload compression |

---

## Dependency Graph and Integration

**Core Streams (S01-S02):**
- S01 (pixel buffer) uses: glam (transforms), image (texture loading), fast_image_resize (scaling)
- S02 (color ops) uses: palette (color math), colorgrad (ramps), rmp-serde (serialization)

**I/O Streams (S07-S12):**
- S07 (.pixhaus) uses: rmp-serde, zstd, image (fallback)
- S08 (aseprite) uses: aseprite-loader, image (texture loading), palette (blend modes)
- S09 (PSD) uses: psd, image (texture loading)
- S10 (PNG export) uses: image, texture-packer/guillotiere, fast_image_resize
- S11 (GIF/WebP) uses: image (GIF), webp (WebP), ravif (AVIF)
- S12 (TMX) uses: tiled, texture-packer, image

**Canvas (S14):**
- Uses: glam (viewport math), pixels/wgpu (optional GPU), tiny-skia/vello (vector overlays), fast_image_resize (zoom/pan scaling)

**Transforms (S04, S16):**
- Uses: glam (math), fast_image_resize (scaling), lyon (path ops)

**Serialization (All):**
- Uses: rmp-serde, bincode, postcard, zstd

---

## Notes for Maintainers (May 2026 Edition)

1. **Tauri 2.x Migration:** Ensure all crates are compatible with Tauri 2.0+ (WebView2 on Windows, WebKit2GTK on Linux, WebKit on macOS).

2. **MSRV (Minimum Supported Rust Version):** Plan for MSRV 1.87 (glam's MSRV). Most crates track recent stable; expect 1.80+.

3. **Pure Rust Preference:** Prefer pure Rust crates (e.g., exr, aseprite-loader) over C bindings (libavif, OpenEXR) to simplify the build and avoid cross-compilation headaches.

4. **SIMD and Performance:** glam, fast_image_resize, and palette all use SIMD where available. Expect measurable speedups on modern CPUs. Profile and bench before and after significant changes.

5. **Web Export (WASM):** If Pixhaus ever supports WASM export, ensure crates support no_std or can be shimmed (postcard, wasm-bindgen integration, etc.).

6. **Compatibility Testing:** Maintain a test fixture suite (sample Aseprite, PSD, Tiled files) to catch format parser regressions.

7. **Future Considerations:**
   - Monitor linebender/color for W3C compliance.
   - Watch wgpu's WebGPU spec alignment for stability.
   - Consider AI model serialization formats (ONNX, safetensors) if S21-S36 verbs require on-disk model storage.

---

## References

- Image Crate: https://docs.rs/image/
- Wgpu Documentation: https://docs.rs/wgpu/
- Palette Crate: https://docs.rs/palette/
- Glam Repository: https://github.com/bitshifter/glam-rs
- Aseprite-Loader: https://docs.rs/aseprite-loader/
- Tiled Map Editor: https://doc.mapeditor.org/
- Linebender Project: https://linebender.org/
