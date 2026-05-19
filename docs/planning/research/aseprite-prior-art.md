# Aseprite prior-art dossier and porting roadmap

**Status:** research, not yet committed to a stream
**Audience:** anyone considering a port from Aseprite into Pixhaus
**Last touched:** 2026-05-19

## Preamble

This dossier records what we learned from a close read of the Aseprite source tree at `/Users/luismorales/project/pixhaus-app/aseprite`. The goal is not a competitive-research write-up — we have those already under `docs/planning/pixel-art-editors/`. The goal is a porting roadmap: what to lift, what to reconstruct fresh, and what to walk past.

Aseprite is the dominant prior art in the pixel-art-editor space. Its data model, file format, rendering pipeline, and algorithm collection have been refined for two decades. A meaningful fraction of that body of code is MIT-licensed, and that licensing posture is the single most important fact in this dossier — without it we would be doing clean-room observation. With it, we have a concrete porting target.

### How this dossier is organized

Every subsystem section follows the same shape:

- **What it does** — a one-paragraph functional summary.
- **License status** — MIT (importable) or EULA (read-only inspiration). Source file paths are included so future contributors can verify the license header themselves before porting anything.
- **How it's decomposed** — the class or module breakdown observed upstream.
- **Why the decomposition pays off** — the design constraint it solves. Two decades of refinement mean most decompositions exist because something else was tried and failed.
- **Our equivalent today** — a pointer into `core/`, `io/`, `ai/`, `app/`, `ui/`, or "not yet" if we have nothing analogous.
- **Port plan** — for MIT subsystems, the file-to-module mapping and the idiom translation (virtual hierarchies become Rust enums or trait objects; template traits become Rust generics; raw pointers become borrowed references and `Arc<CelData>` only at the ownership boundary). For EULA subsystems, the design idea we reconstruct fresh.
- **Attribution checklist** — for MIT ports, the exact copyright lines to carry into the ported file header.

### Scope

In scope: the document model, rendering pipeline, file I/O, color and palette infrastructure, selection mask algorithms, brush and tool decomposition, undo command structure, animation timeline, tilemap, scripting API surface, plugin / extension contract, and the embedded `laf` / `clip` modules.

Out of scope: the Aseprite installer flow, the platform-specific UI shell, the Skia / pixman renderer (we use WebGL2), Lua 5.1-specific semantics (we use `mlua`), and any business logic in `src/app/` that we'd rebuild from scratch anyway because our stack is different.

### A note on naming

The user explicitly authorized naming Aseprite in this dossier because the relevant subtrees are MIT-licensed and integration is therefore permitted with attribution. The earlier draft of this document treated the source as anonymous prior art; that framing was abandoned once the per-subtree license picture was understood. Naming the source is also better engineering: future contributors evaluating a port should not have to play license detective on an unnamed reference.

## License posture summary

Aseprite ships under a three-tier license arrangement, documented in the upstream `README.md`:

1. **End-User License Agreement (EULA)** for official binary releases and for the `src/app/` and `src/main/` portions of the source. The EULA is a proprietary license: it permits personal use, source modification "for your own personal purpose or to propose a contribution," and forbids redistribution. We can read these files for ideas. We cannot copy code from them.
2. **MIT license** for the document, rendering, I/O, UI, observable, undo, fixmath, flic, cfg, net, desktop, gen, and steam libraries inside `src/`, plus the externally-hosted `laf` and `clip` submodules. We can port these into Pixhaus with attribution.
3. **Steam Subscriber Agreement** for releases distributed through Steam. Not relevant to the source tree.

The per-file header is the authoritative signal. MIT-licensed files start with:

```
// This file is released under the terms of the MIT license.
// Read LICENSE.txt for more information.
```

EULA-licensed files start with:

```
// This program is distributed under the terms of
// the End-User License Agreement for Aseprite.
```

When porting, the first thing to do for any candidate file is open it and confirm which header it carries. The grouping by directory holds in practice but not by guarantee — a file moved between subtrees might still carry its original license header, and the header is what binds.

### What we owe upstream

The MIT permission grant is short. Practically, satisfying it means four things:

1. **Preserve copyright notices.** Any Rust translation of an upstream file must carry the original Igara Studio / David Capello copyright lines in its header, marked as a port.
2. **Include the license text.** The full MIT notice with both copyright lines is recorded at `LICENSES/aseprite-MIT.txt` in this tree. That file must remain so long as any port derived from upstream remains.
3. **Note the derivation.** Each ported file declares that it is a port and points at `LICENSES/aseprite-MIT.txt`. The `LICENSES/NOTICE.txt` file lists the broader attribution at repo level so a downstream consumer scanning the tree once can find the upstream record.
4. **Don't alter the upstream copyright.** Even if we substantially rewrite a ported file, the upstream notice stays. Substantial rewriting may reduce how much of the original expression survives, but it does not extinguish copyright on whatever structure does survive, and the courteous and conservative move is to keep the line.

### Header template for ports

Use this exact form at the top of any Rust file ported from MIT-licensed Aseprite source. The Bresenham line in brackets only applies to files that contain Zingl's line-drawing implementations.

```rust
// Ported from Aseprite (https://github.com/aseprite/aseprite)
//   Upstream: src/<path-to-original.cpp-or-h>
//   Copyright (c) 2018-2025 Igara Studio S.A.
//   Copyright (c) 2001-2018 David Capello
//   Released under the MIT license. See LICENSES/aseprite-MIT.txt.
// [Bresenham line-drawing portions:
//   Copyright (c) 2012-2016 Alois Zingl, MIT.]
//
// This Rust port: Copyright (c) 2026 Pixhaus contributors. MIT.
```

The same pattern, with `//` swapped for `--` or `#` as appropriate, applies to any Lua, shader, or TOML port.

## License audit matrix

The table below maps every upstream subtree of interest to its license, our integration verdict, and the upstream files most worth reading. Verdicts are:

- **Port** — translate to Rust with attribution. The subsystem solves a concrete problem we have, and the upstream solution is the one we'd choose anyway.
- **Adopt** — translate the structure but write the implementation against our stack. Used when upstream targets a different runtime (Skia, custom UI framework) but the design factors cleanly out of its host.
- **Inspire** — read for ideas, build fresh. Used for EULA territory and for anything where the upstream code is so tied to the upstream object graph that a port would be more rework than rewrite.
- **Avoid** — don't read for code, don't copy. EULA-only modules.

| Upstream subtree | License | Verdict | Notes |
|------------------|---------|---------|-------|
| `src/doc/` (document model) | MIT | Port | Highest-value port. Sprite, Layer, Cel, Image, Palette, Tileset, Tag, Slice, Mask all live here. |
| `src/doc/algorithm/` (image algorithms) | MIT | Port | Bresenham, floodfill, polygon, flip, shift, resize, rotate, RotSprite, shrink_bounds. |
| `src/render/` (rendering pipeline) | MIT | Adopt | Algorithms (quantize, dither, gradient) port directly; composition logic adopts the structure but runs on WebGL2. |
| `src/dio/` (file I/O) | MIT | Port | `.aseprite` decoder/encoder, foreign-format dispatchers. |
| `src/ui/` (widget framework) | MIT | Inspire | Educational only — we use Solid + WebGL2 inside Tauri. |
| `src/observable/` (signal/slot) | MIT | Inspire | Rust has better idioms (channels, traits). |
| `src/undo/` (undo library) | MIT | Adopt | Branching command-tree design ports; mixin pattern reconstructed in Rust. |
| `src/fixmath/` (fixed-point math) | MIT | Skip | Rust has `fixed` crate if we ever need it. |
| `src/flic/` (FLI/FLC loader) | MIT | Skip | FLI is historical; we don't target it. |
| `src/cfg/` (INI files) | MIT | Skip | We use TOML via `serde`. |
| `src/net/` (HTTP) | MIT | Skip | We use `reqwest`. |
| `src/psd/` (PSD reader) | per-file | Verify before porting | License needs per-file confirmation. |
| `src/app/` (application layer) | EULA | Inspire / Avoid copying | Tool decomposition, command system, scripting bindings are all here. Read for architecture, write fresh. |
| `src/main/` (entry point) | EULA | Avoid | Tauri replaces this. |
| `laf/` (platform layer) | MIT (external) | Skip | Rust crates (`uuid`, `image`, `winit` via Tauri) cover the same surface. |
| `clip` (clipboard) | MIT (external) | Skip | Tauri clipboard plugin covers it. |
| Third-party deps in `third_party/` | per-package | N/A | Each has its own license; Pixhaus picks its own equivalents (`image`, `png`, `gif`, `webp`, `mlua`). |
| `docs/ase-file-specs.md` | (no explicit license) | Reference | The format spec itself is published. Document a reference, not a port. |

Subtrees marked "Skip" are functionally covered by Rust crates we either already use or would prefer to depend on rather than port. The MIT license still applies to anything we do port from them.

## Tree-level architectural diagram

Upstream tags its directories by dependency level (0 through 5) in `src/README.md`. The level numbers are useful because they tell us *what we can port first* — Level 0 has no upstream dependencies, Level 1 depends on Level 0, and so on. Porting strictly in level order means each port lands on a foundation that is already ours.

```
Level 0 — independent
  doc dependencies inside, but otherwise free-standing:
    clip, fixmath, flic, laf/base, laf/gfx, observable, undo, scripting

Level 1 — depend on Level 0
  cfg (base), gen (base), net (base), laf/os (base, gfx, wacom)

Level 2 — depend on 0-1
  doc (base, fixmath, gfx) — the document model
  ui (base, gfx, os)       — the widget framework
  updater (base, cfg, net) — update checker

Level 3 — depend on 0-2
  dio (base, doc, fixmath, flic) — file I/O
  filters (base, doc, gfx)        — image effects
  render (base, doc, gfx)         — rendering pipeline
  view (base, doc)                — abstract timeline helpers

Level 4 — depend on 0-3
  app (base, doc, dio, filters, fixmath, flic, gfx, pen, render, scripting, os, ui, undo, updater, view)
    THIS IS THE EULA LAYER. Everything below it can be ported under MIT.
  desktop (base, doc, dio, render)

Level 5 — depend on 0-4
  main (app, base, os, ui) — EULA
```

The cut between Level 3 and Level 4 is exactly where MIT ends and EULA begins. That's not coincidence — when Igara relicensed Aseprite from GPL to its current EULA in 2016, the application layer became proprietary while the lower-level libraries kept their MIT terms specifically so that the file format and document model could continue to be used by third parties (game engines, sprite-sheet importers, exporters). Our port strategy benefits directly from that cut: Levels 0 through 3 are where the interesting algorithm and data-structure work lives, and they're all importable.

## 1. Document model — `src/doc/` (MIT)

### What it does

The document model is the in-memory representation of a sprite: a tree of layers, each containing per-frame cels, each cel pointing at an image (or a tilemap, or a reference to another cel's data). It also owns palettes (potentially one per frame for palette animation), tilesets, tags, slices, and free-form user data. Every operation in the editor either reads this tree or commits a Cmd that mutates it.

### License status

MIT. Per-file headers say "This file is released under the terms of the MIT license. Read LICENSE.txt for more information." Upstream files of interest:

- `src/doc/sprite.h`, `sprite.cpp`
- `src/doc/layer.h`, `layer.cpp`
- `src/doc/cel.h`, `cel.cpp`
- `src/doc/cel_data.h`, `cel_data.cpp`
- `src/doc/image.h`, `image_impl.h`, `image_bits.h`
- `src/doc/palette.h`, `palette.cpp`
- `src/doc/tileset.h`, `tilesets.h`, `tile.h`
- `src/doc/tag.h`, `tags.h`
- `src/doc/slice.h`, `slices.h`
- `src/doc/mask.h`, `mask.cpp`, `mask_boundaries.h`
- `src/doc/user_data.h`
- `src/doc/object.h` (UUID-based identity for all doc objects)

### How it's decomposed

The object graph rooted at `Sprite`:

```
Sprite
├── LayerGroup (root layer — every sprite has a root group)
│   ├── LayerImage           — raster layer with cels per frame
│   ├── LayerImage           — flagged as Background, Reference, etc.
│   ├── LayerTilemap         — uses a tileset, stores tile indices
│   └── LayerGroup
│       └── LayerImage
├── palettes[]               — frame-indexed; multiple palettes for palette animation
├── tilesets[]               — one or more shared tilesets, addressed by ID
├── tags[]                   — named frame ranges for animation playback
├── slices[]                 — named rectangular regions (9-slice metadata, pivots)
└── user_data                — sprite-wide free-form properties
```

A **Cel** sits at the intersection of (Layer, Frame). It owns a position offset, an opacity, a Z-index for inter-layer override, and either inlines a `CelData` or shares one with another cel. The Cel/CelData split is the **linked cel** mechanism: when you duplicate a static frame, the duplicate cel points at the same `CelData` as the original until either one mutates, at which point the writer clones first.

A **CelData** owns the image (or tilemap), its bounds, and metadata. Multiple Cels can share one `CelData` via reference counting.

An **Image** owns the pixel buffer for a CelData. It is parameterized by color mode (RGBA, Indexed, Grayscale, Tilemap) and exposes its pixels through a template `LockImageBits<ImageTraits>` iterator. Row stride is configurable so that scanlines can be padded for SIMD alignment.

### Why the decomposition pays off

Three of these design choices are non-obvious and worth understanding before porting:

**Linked cels (Cel referencing shared CelData).** Without sharing, a 60-frame animation with a static background layer requires 60 copies of the background image in memory. With sharing, it requires one. The sharing also propagates to disk: the `.aseprite` format has a Cel chunk type 1 (Linked Cel) that records only the frame number to link to, not the pixel data. Decoding a Linked Cel is constant-time. The cost is that any write must first clone, which is `CelData::isUnique()` + `clone()` — a check that costs one branch and a refcount read. The trade favors animation memory by a large factor.

**Layer as a hierarchy, not a flat list.** Aseprite supports nested layer groups so artists can collapse "all background props" or "all character limbs" as one unit. This also matters for blending: a group can be set to "composite separately first," which means children are flattened into a temporary buffer using their own blend modes before that buffer is composited onto the parent with the group's blend mode. Without group-flatten-first semantics, modes like `Hue` and `Color` produce different results depending on layer order in ways artists find counterintuitive.

**Frame as a small integer.** A `frame_t` is a `u32`. Iteration over frame ranges is a tight loop. Tag ranges, selection sets, and onion-skin spans all reduce to integer arithmetic. Compare this to designs where each frame is a heap-allocated object — those don't scale well to long animations.

### Our equivalent today

Spec only / partial. The Pixhaus `core/project/` data model has been spec'd in `docs/planning/work/bedrock.md` (B2) but the implementation has been refactored away from the upstream's class-based shape toward a flatter, ID-based representation more idiomatic in Rust. The bedrock spec already calls out the linked-cel optimization but the implementation should be reviewed against this dossier before any port.

### Port plan

Translate the upstream class hierarchy into Rust this way:

```rust
// core/project/sprite.rs
pub struct Sprite {
    pub id: SpriteId,                // UUID
    pub size: Size,                  // canvas dimensions in pixels
    pub color_mode: ColorMode,       // RGBA | Indexed | Grayscale
    pub pixel_ratio: PixelRatio,
    pub transparent_index: u8,       // only meaningful for indexed sprites
    pub color_space: Option<ColorSpace>,
    pub grid: Option<Grid>,
    pub root_layer: LayerGroup,      // every sprite has a root group
    pub palettes: Vec<Palette>,      // frame-indexed; one palette = all frames
    pub tilesets: TilesetRegistry,   // indexed by TilesetId
    pub tags: Vec<Tag>,
    pub slices: Vec<Slice>,
    pub user_data: UserData,
}
```

Replace the virtual `Layer` hierarchy with a Rust enum to make exhaustive matching the default:

```rust
pub enum Layer {
    Image(LayerImage),
    Group(LayerGroup),
    Tilemap(LayerTilemap),
}

pub struct LayerImage {
    pub id: LayerId,
    pub name: String,
    pub flags: LayerFlags,           // Visible, Editable, LockMove, Background, ...
    pub blend_mode: BlendMode,
    pub opacity: u8,
    pub cels: BTreeMap<FrameId, Cel>,
    pub user_data: UserData,
}
```

For linked cels, factor out the data behind a shared owner. `Arc<CelData>` is appropriate only here, at the data-only boundary. The Cel itself stays uniquely owned by its parent layer:

```rust
pub struct Cel {
    pub position: Point,
    pub opacity: u8,
    pub z_index: i16,
    pub data: Arc<CelData>,
}

pub struct CelData {
    pub bounds: Rect,
    pub image: Image,
    pub user_data: UserData,
}

impl Cel {
    fn make_unique(&mut self) -> &mut CelData {
        Arc::make_mut(&mut self.data)
    }
}
```

`Arc::make_mut` gives the exact copy-on-write semantics upstream gets from `CelData::isUnique()` + `clone()`, with the cloning done implicitly by the standard library. The pattern is well understood by Rust readers, so the structure is more legible than the upstream C++ equivalent.

For frames, use a strong type wrapping `u32`:

```rust
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FrameId(pub u32);
```

The newtype prevents accidentally adding a frame index to a layer index — a class of bugs upstream avoids only by convention.

### Attribution checklist

Files in `core/project/` that derive from upstream `src/doc/` carry the header template from the License posture summary. Concretely:

- `core/project/sprite.rs` → upstream `src/doc/sprite.h`, `sprite.cpp`
- `core/project/layer.rs` → upstream `src/doc/layer.h`, `layer.cpp`
- `core/project/cel.rs` → upstream `src/doc/cel.h`, `cel.cpp`, `cel_data.h`, `cel_data.cpp`
- `core/project/image.rs` → upstream `src/doc/image.h`, `image_impl.h`, `image_bits.h`
- `core/project/palette.rs` → upstream `src/doc/palette.h`, `palette.cpp`
- `core/project/tag.rs` → upstream `src/doc/tag.h`, `tags.h`
- `core/project/slice.rs` → upstream `src/doc/slice.h`, `slices.h`
- `core/project/mask.rs` → upstream `src/doc/mask.h`, `mask.cpp`
- `core/project/user_data.rs` → upstream `src/doc/user_data.h`
- `core/tilemap/tileset.rs` → upstream `src/doc/tileset.h`, `tilesets.h`, `tile.h`

## 2. Pixel buffer representation — `src/doc/image*.h` (MIT)

### What it does

Stores the raw pixels for a CelData and exposes them through an iterator interface that is generic over color mode. Three lock modes (Read, Write, ReadWrite) let multiple readers run concurrently against the same image while serializing writers. Row stride is configurable so a row of 13 RGBA pixels can be stored as 16-pixel-aligned 64 bytes (`13 * 4 = 52` rounded up to 64) for SIMD-friendly access.

### License status

MIT. Files:

- `src/doc/image.h`
- `src/doc/image_impl.h`
- `src/doc/image_bits.h` — the `LockImageBits<ImageTraits>` template
- `src/doc/image_traits.h` — the trait classes for each color mode
- `src/doc/image_buffer.h` — pooled buffer reuse
- `src/doc/primitives.h`, `primitives.cpp` — get_pixel / put_pixel helpers

### How it's decomposed

`Image` is a virtual base. Concrete subclasses are `ImageImpl<ImageTraits>` for each `ImageTraits` (RgbTraits, GrayscaleTraits, IndexedTraits, TilemapTraits). The traits class declares:

- `pixel_t` — the storage type (`uint32_t` for RGBA, `uint16_t` for grayscale, `uint8_t` for indexed, `tile_t` for tilemap)
- `bits_per_pixel` — for byte calculations
- `bytes_per_row` — stride helper
- `color_t to_color(pixel_t)` and `pixel_t from_color(color_t)` — conversion

The `LockImageBits<ImageTraits>` is a templated iterator yielding `ImageTraits::pixel_t` references. It acquires the image's lock on construction and releases it on destruction (RAII). The lock contention is unlikely in practice — most editor operations are sequential — but it does prevent the rendering thread from observing torn writes from a tool stroke.

### Why the decomposition pays off

**Templated iteration beats virtual dispatch in the inner loop.** A blend operation that touches every pixel is hot. With virtual dispatch on `Image`, each pixel access incurs a vtable lookup. With template specialization on the color mode, the compiler inlines the access and unrolls the loop. The cost is binary size — there are four specializations of every algorithm. Upstream eats that cost willingly.

**Configurable stride enables aligned SIMD.** Indexed images at 256 pixels wide are well-aligned. Indexed images at 257 pixels wide are not, and a Floyd-Steinberg dither over them would either misalign or pay a per-row branch. Stride padding solves it once.

**Lock semantics for safety, not concurrency.** Aseprite is single-threaded except for the renderer. The locks exist so that the renderer can take a `ReadLock` on the document while the editor's tool loop is between input events, and any attempt by the tool loop to commit a write under the read lock gets caught immediately instead of producing a torn frame on screen. This is a defensive use of locking, not a parallelism primitive.

### Our equivalent today

`core/canvas/buffer.rs` holds pixel buffers as `Vec<u8>` with stride. Color modes are an enum, not a trait parameter. The current implementation pays no lock cost — `&` and `&mut` borrows from the Rust borrow checker do the same work statically. The trade-off is that algorithms must either be generic over a `PixelFormat` trait or branch on the runtime color mode.

### Port plan

Translate `ImageTraits` to a Rust `trait PixelFormat`:

```rust
// core/canvas/pixel.rs
pub trait PixelFormat: Copy + 'static {
    type Pixel: Copy + Default + Eq;
    const BYTES_PER_PIXEL: usize;
    const COLOR_MODE: ColorMode;

    fn pack(rgba: Rgba8) -> Self::Pixel;
    fn unpack(p: Self::Pixel) -> Rgba8;
}

pub struct Rgba;
pub struct Indexed;
pub struct Grayscale;
pub struct Tile;

impl PixelFormat for Rgba {
    type Pixel = u32;
    const BYTES_PER_PIXEL: usize = 4;
    const COLOR_MODE: ColorMode = ColorMode::Rgba;
    fn pack(rgba: Rgba8) -> u32 { /* ... */ }
    fn unpack(p: u32) -> Rgba8 { /* ... */ }
}
// ... and so on for Indexed, Grayscale, Tile.
```

`Image<P: PixelFormat>` then carries a `Vec<u8>` plus `width`, `height`, and `stride` fields. The locking machinery disappears: a `&Image<P>` lets readers run; a `&mut Image<P>` excludes them. The borrow checker enforces what upstream enforces at runtime.

Where an algorithm must be runtime-polymorphic (the file decoder needs to decide which `Image<P>` to construct based on the sprite's color mode), use a wrapper:

```rust
pub enum AnyImage {
    Rgba(Image<Rgba>),
    Indexed(Image<Indexed>),
    Grayscale(Image<Grayscale>),
    Tile(Image<Tile>),
}
```

Most algorithms take `&Image<P>` and let the caller dispatch on `AnyImage` once at the boundary. This is the same pattern Rust crates use when wrapping format-generic data (`image::DynamicImage` versus `image::ImageBuffer<P, _>`).

### Attribution checklist

- `core/canvas/buffer.rs` → upstream `src/doc/image.h`, `image_impl.h`
- `core/canvas/pixel.rs` → upstream `src/doc/image_traits.h`, `image_bits.h`
- `core/canvas/primitives.rs` → upstream `src/doc/primitives.h`, `primitives.cpp`

## 3. Native file format — `.aseprite` and `src/dio/aseprite_*` (MIT)

### What it does

Reads and writes the `.aseprite` binary file format. The format predates Aseprite — it descends from Allegro's `.fli` / `.flc` animation files, with a different magic number and an extended chunk vocabulary. The format is the single most important interop target for any pixel-art editor, because every existing pixel-art asset ships with `.aseprite` source on the artist's drive even when the deliverable is PNG.

### License status

MIT. Files:

- `src/dio/aseprite_decoder.h`, `aseprite_decoder.cpp`
- `src/dio/aseprite_encoder.h`, `aseprite_encoder.cpp`
- `src/dio/aseprite_common.h`
- `src/dio/detect_format.h`, `detect_format.cpp`
- `src/dio/file_format.h`
- `docs/ase-file-specs.md` — the human-readable specification, openly published

### How it's decomposed

The format is structured as:

```
128-byte header
For each frame:
    16-byte frame header
    For each chunk:
        DWORD chunk size
        WORD  chunk type
        BYTE[] chunk data
```

Chunk types observed in current files:

| Type   | Name                  | Purpose                                                  |
|--------|-----------------------|----------------------------------------------------------|
| 0x0004 | Old palette           | Pre-1.1, 0-255 RGB. Kept for backward compatibility.    |
| 0x0011 | Old palette v2        | 0-63 RGB. Even older.                                    |
| 0x2004 | Layer                 | Layer metadata: flags, type, blend mode, opacity, name. |
| 0x2005 | Cel                   | Cel position, opacity, image data (raw / linked / zlib).|
| 0x2006 | Cel extra             | Subpixel-precision bounds for the latest cel.           |
| 0x2007 | Color profile         | sRGB / fixed gamma / embedded ICC.                       |
| 0x2008 | External files        | References to external palettes, tilesets, extensions.  |
| 0x2018 | Tags                  | Named frame ranges with direction and repeat count.     |
| 0x2019 | Palette               | New RGBA palette (with optional per-entry name).        |
| 0x2020 | User data             | Free-form properties attached to the previous chunk.    |
| 0x2022 | Slice                 | Named rectangle with optional 9-patch and pivot data.   |
| 0x2023 | Tileset               | Tileset definition with optional embedded tile pixels.  |

The decoder walks chunks in order and dispatches on `chunk type`. Most chunks attach to the most recently decoded parent (a User Data chunk after a Cel chunk applies to that Cel; a User Data chunk after a Tags chunk applies to the first tag of that Tags chunk, with subsequent User Data chunks applying to subsequent tags). This stateful "last seen" pattern is the format's main quirk and is easy to get wrong on a first implementation.

### Why the decomposition pays off

**Chunk vocabulary instead of a fixed schema.** When a new feature is added (e.g., subpixel cel bounds, tilemap-flip flags, per-entry palette names), it lands as a new chunk type or a new flag on an existing chunk. Old readers ignore the chunks they don't recognize and produce correct (if feature-poor) output. The format has stayed binary-compatible across more than a decade of feature additions.

**Per-chunk zlib instead of whole-file compression.** Each image cel is compressed independently. A reader scanning for the third frame can seek through the file by chunk sizes without decompressing anything it doesn't need. Random-access partial loads are practical. The downside is slightly worse compression ratio than a whole-file zstd — the wins of avoiding 60-frame decompression on a "show me frame 30" query outweigh the loss in our context.

**Old chunks kept indefinitely.** When the format outgrew the 0x0004 palette chunk (no alpha, no per-entry names), Igara added 0x2019 instead of replacing 0x0004. Files written by current Aseprite write both 0x0004 and 0x2019 when the palette fits the old format, so a 1.0-era reader still gets a usable palette. The cost is a few hundred extra bytes per file. The benefit is that no `.aseprite` file ever becomes unreadable.

### Our equivalent today

`io/aseprite/` exists with read support for a subset of chunk types. Write support has not landed. The decoder predates this dossier; review it against the upstream chunk list before assuming feature parity.

### Port plan

The port is the highest-value single piece of work this dossier identifies. A direct translation of `aseprite_decoder.cpp` and `aseprite_encoder.cpp` gets us read/write parity with the canonical pixel-art file format, which directly drives interop with every artist on the platform. Approach:

1. **Spec the chunks in Rust types first.** One struct per chunk type, plus a top-level `AsepriteFile` that owns a header and a `Vec<Frame>` and each frame owns a `Vec<Chunk>`. This mirrors the binary layout. Encoding becomes "serialize this tree as little-endian bytes."

2. **Use `binrw` or a hand-rolled little-endian reader.** Aseprite is Intel byte order throughout. Most chunks have a fixed prefix and a variable tail (image data, string, optional flag-gated fields). `binrw` handles the fixed prefixes with derive macros; variable tails are a manual closure.

3. **Per-chunk zlib via `flate2`.** The Compressed Image and Compressed Tilemap chunks wrap raw pixel data in zlib (RFC 1950, DEFLATE-based). `flate2::read::ZlibDecoder` on a sized sub-reader handles it cleanly.

4. **Walk the "last seen" stateful association in a single pass.** After each chunk, update a `LastSeen` enum (`LastSeen::Cel(cel_id)`, `LastSeen::Tag(start_tag_idx)`, etc.) so a following User Data chunk can attach to the right parent.

5. **Test against round-trip fixtures.** Embed a handful of small `.aseprite` files (donated CC0 fixtures, or files we make ourselves) in `io/aseprite/tests/fixtures/` and assert `decode(encode(decode(f))) == decode(f)`. The double-decode is to normalize away encoder freedom (e.g., the encoder's choice of whether to write the old 0x0004 palette chunk).

6. **Preserve format-specific quirks.** The Cel chunk uses a `z_index` ordering trick documented in NOTE.5 of `ase-file-specs.md`:

   ```c++
   // From docs/ase-file-specs.md NOTE.5:
   int order() const { return layerIndex + zIndex; }
   bool operator<(const Layer& b) const {
       return (order() < b.order()) ||
              (order() == b.order() && (zIndex < b.zIndex));
   }
   ```

   The Rust render order must match this exactly for any sprite that uses z-index overrides to render the same as in Aseprite. Don't "fix" it.

### Attribution checklist

- `io/aseprite/decoder.rs` → upstream `src/dio/aseprite_decoder.h`, `aseprite_decoder.cpp`
- `io/aseprite/encoder.rs` → upstream `src/dio/aseprite_encoder.h`, `aseprite_encoder.cpp`
- `io/aseprite/chunks.rs` → upstream `src/dio/aseprite_common.h`
- `io/aseprite/detect.rs` → upstream `src/dio/detect_format.h`, `detect_format.cpp`
- `io/aseprite/format.rs` (the public `FileFormat`-equivalent surface) → upstream `src/dio/file_format.h`

The `docs/ase-file-specs.md` file itself isn't a port target — we link to the upstream copy as reference documentation. Cite it in the docs of `io/aseprite/decoder.rs`.

## 4. Tool system — `src/app/tools/` (EULA, inspire-only)

### What it does

Translates user input (mouse, pen, keyboard modifiers) into drawing operations on the document. The tool system mediates between raw stroke data and the document model, applying brush shapes, blend modes, symmetry, and pixel-perfect smoothing along the way. It also handles non-destructive preview rendering so the artist sees what a stroke will look like before committing it.

### License status

**EULA — read only, do not copy.** All files in `src/app/tools/` carry the EULA header. We learn the decomposition; we implement fresh.

Files of interest (for architecture reading only):

- `src/app/tools/tool.h` — base class for tools
- `src/app/tools/tool_box.h` — registry
- `src/app/tools/ink.h`, `inks.h` — ink (pixel transform) abstraction
- `src/app/tools/point_shape.h` — how a single dab is painted
- `src/app/tools/controller.h` — mouse → stroke
- `src/app/tools/intertwine.h` — path interpolation between stroke points
- `src/app/tools/dynamics.h` — pressure / tilt / velocity modulation
- `src/app/tools/symmetry.h` — symmetry expansion
- `src/app/tools/tool_loop.h`, `tool_loop_manager.cpp` — per-stroke session
- `src/app/tools/stroke.h` — stroke point data

### How it's decomposed

Upstream factors the tool system along four orthogonal axes:

1. **Ink** — what pixel transform happens at the dab site. Examples: Normal (alpha-blend the brush color over the destination), Opaque (overwrite with the brush color, no blend), Eraser (replace with transparent), Shading (cycle through palette entries based on the dab order), Outline (draw at the edge of the touched region, not at the dab), Copy (sample from the brush image), Merge (blend modes between source and dest).
2. **PointShape** — what shape the dab is. Examples: Pixel (a single pixel), Brush (a circle, square, or line from the brush configuration), Image (a custom brush image), Spray (a randomized scatter).
3. **Controller** — how raw input becomes stroke geometry. Examples: Freehand (every input point is a stroke point), Line (only first and last points matter), Bezier (control-point curve), Polygon (closed shape from line segments).
4. **Intertwine** — how to interpolate between adjacent stroke points. Examples: AsLines (draw line segments between every pair of points), AsCurves (cubic interpolation), Spray (skip the interpolation; draw dabs only at sample times).

A **Tool** is a tuple `(Ink, PointShape, Controller, Intertwine)` plus per-button overrides (left mouse vs. right mouse can have different inks for the eraser hold-shift behavior). Tools themselves are stateless factories.

Per-stroke state lives in **ToolLoop** and **ToolLoopManager**. The Manager owns the stroke session: it receives input events, asks the Controller for stroke points, asks the Intertwine for interpolated points between them, asks the PointShape for the brush footprint at each point, and asks the Ink for the pixel transform. It builds up a single Cmd that represents the entire stroke and commits it on stroke end.

The **Stroke** struct carries per-point data — position, pressure, tilt, velocity, button state, modifier keys — and is the unit of communication between Controller and Intertwine.

### Why the decomposition pays off

**Four-axis orthogonality dramatically reduces N.** A naive design ("pencil tool", "eraser tool", "line tool", ...) explodes when features cross. With Aseprite's decomposition, the eraser is `(EraserInk, BrushShape, FreehandController, AsLinesIntertwine)` and the line-mode eraser is `(EraserInk, BrushShape, LineController, AsLinesIntertwine)`. The Controller swap is free; no new code. There are roughly a dozen Inks, half a dozen PointShapes, half a dozen Controllers, and a handful of Intertwines, so the matrix is hundreds of usable tools without writing them all out.

**Stateless tools + stateful loops gives one-shot undo per stroke.** The ToolLoopManager accumulates the stroke as a single Cmd. The artist undoes the whole stroke, not individual pixels. This is non-negotiable for pixel art — partial-stroke undo is unusable.

**Non-destructive preview without buffer copies.** The Manager renders the stroke into a preview overlay on top of the unmodified document. Only on stroke-end does the Cmd touch the actual document. This means the cost of `cancel-stroke` is zero (just drop the preview) and `commit-stroke` is one allocation (the Cmd's pre-image for undo).

### Our equivalent today

Partial. The current `app/tools/` has Pencil, Eraser, Line, Rectangle, Ellipse, Fill, and Pick. They are implemented closer to the naive "one struct per tool" pattern rather than the four-axis decomposition. The orthogonality is missing.

### Port plan

This is EULA territory, so do not transcribe upstream code. Reconstruct the four axes as Rust traits and dispatch tools through them.

```rust
// app/tools/ink.rs
pub trait Ink {
    fn apply(&self, dst: &mut Image<P>, src_color: Rgba8, brush_alpha: u8, x: i32, y: i32);
}

pub struct NormalInk;
pub struct EraserInk;
pub struct CopyInk;
pub struct ShadingInk { /* palette cycle state */ }
// ...
```

```rust
// app/tools/point_shape.rs
pub trait PointShape {
    fn footprint(&self, x: i32, y: i32, brush: &Brush) -> impl Iterator<Item = (i32, i32, u8)>;
}

pub struct PixelShape;
pub struct BrushShape;
pub struct SprayShape { /* RNG state */ }
```

```rust
// app/tools/controller.rs
pub trait Controller {
    fn on_input(&mut self, event: InputEvent) -> ControllerOutput;
    fn finish(&mut self) -> Stroke;
}
```

```rust
// app/tools/intertwine.rs
pub trait Intertwine {
    fn interpolate(&self, stroke: &Stroke) -> Vec<StrokePoint>;
}

pub struct AsLines;
pub struct AsCurves;
pub struct SprayIntertwine;
```

A `Tool` is then:

```rust
pub struct Tool {
    pub id: ToolId,
    pub left: ToolConfig,
    pub right: ToolConfig,
}

pub struct ToolConfig {
    pub ink: Box<dyn Ink>,
    pub point_shape: Box<dyn PointShape>,
    pub controller: Box<dyn Controller>,
    pub intertwine: Box<dyn Intertwine>,
}
```

The `Box<dyn _>` is acceptable here because tool dispatch happens at most once per input event, not once per pixel. The hot path stays in `Ink::apply` which is called directly with no dispatch overhead because the call site can take `&dyn Ink` and use `Vec` of dab coordinates instead of `Vec` of `Box<dyn Ink>`.

ToolLoop becomes a session struct:

```rust
pub struct ToolLoop {
    sprite_snapshot: SpriteSnapshot,   // for undo pre-image
    preview: PreviewBuffer,            // overlay
    target_cel: CelRef,
    config: ToolConfig,
    stroke: Stroke,
}

impl ToolLoop {
    pub fn on_input(&mut self, event: InputEvent) -> ToolLoopState {
        // 1. Pass event to controller
        // 2. If controller emits points, run them through intertwine
        // 3. For each interpolated point, paint a dab via point_shape + ink into preview
        // 4. Return whether stroke is still in progress
    }

    pub fn commit(self) -> Cmd { /* build the single command */ }
    pub fn cancel(self) { /* drop the preview */ }
}
```

### Attribution checklist

None — this is EULA, so no upstream attribution is required (because no upstream code is copied). The new files carry only the Pixhaus copyright.

## 5. Symmetry — `src/app/tools/symmetry.cpp` (EULA, inspire-only)

### What it does

When symmetry is enabled, every brush dab the artist makes is mirrored or rotated, producing multiple symmetric dabs from one input. Aseprite supports a single symmetry axis (vertical, horizontal, both), with the axis position configurable on the canvas. The result is a single command in undo (mirrored dabs commit atomically).

### License status

EULA. Concept-only.

### How it's decomposed

The Intertwine stage emits the original stroke points. A "symmetry pass" between Intertwine and PointShape duplicates each point across each enabled axis, transforming coordinates as it goes. The PointShape and Ink stages see N points instead of one and paint them all. Because the duplication is a pure coordinate transform, it composes with any tool — symmetric line, symmetric fill, symmetric brush all work without special-casing.

### Why the decomposition pays off

**Symmetry as a coordinate filter avoids tool-specific code.** A naive design ("symmetric pencil tool") doesn't compose with the rectangle tool, the ellipse tool, the spray tool, etc. Putting symmetry between Intertwine and PointShape means every tool gets it for free. The same trick works for tiled-mode (when "tile mode" is on, each dab is duplicated 8 times across the seam) — both features share infrastructure.

**One command per symmetric stroke.** The PointShape paints all N symmetric dabs into the preview, then the entire collection commits as one Cmd. Undoing the stroke undoes the symmetry too.

### Our equivalent today

Not yet. The data model can represent a symmetry axis but there is no editor support.

### Port plan

Add a `SymmetryAxis` to the Sprite (or to the ToolLoop session if symmetry is per-stroke):

```rust
pub struct SymmetryAxis {
    pub kind: SymmetryKind,   // Horizontal | Vertical | Both
    pub position: Point,      // where the axis crosses the canvas
}
```

Wire it as a transform-points stage in ToolLoop:

```rust
fn expand_symmetry(points: &[StrokePoint], axis: &SymmetryAxis) -> Vec<StrokePoint> {
    let mut out = Vec::with_capacity(points.len() * 4);
    for p in points {
        out.push(p.clone());
        if axis.kind.is_horizontal() {
            out.push(reflect_x(p, axis.position.x));
        }
        if axis.kind.is_vertical() {
            out.push(reflect_y(p, axis.position.y));
        }
        if axis.kind.is_both() {
            out.push(reflect_x(&reflect_y(p, axis.position.y), axis.position.x));
        }
    }
    out
}
```

Rotational symmetry (4-fold, 6-fold, 8-fold) is the same shape with more reflection points. Add when the verb library asks for it; not core for v1.

### Attribution checklist

None — EULA territory, no upstream code copied.

## 6. Pixel-perfect line drawing — `src/doc/algorithm/` (MIT, with Zingl attribution)

### What it does

Rasterizes a line between two integer-coordinate points using a Bresenham-derived algorithm with corner-cleanup so that the resulting pixel line doesn't have visible "stair-step" doubled pixels at diagonal turns. The plain Bresenham line produces shapes like:

```
##
 ##
  ##
   ##
```

The corner-cleanup variant removes the corner doubles to produce:

```
#
 #
  #
   #
```

The result looks correct under the pixel-art aesthetic, where the doubled pixel reads as a slope discontinuity rather than as antialiasing.

### License status

MIT (twice). The Aseprite document-library MIT applies. The Bresenham implementations originally written by Alois Zingl (2012-2016, also MIT) are vendored into the upstream tree and acknowledged in `docs/LICENSES.md`. A port must preserve both copyright lines.

Files:

- `src/doc/algorithm/polygon.cpp` (uses the Bresenham primitive)
- `src/doc/algorithm/floodfill.cpp` (uses scanlines, related family)
- `src/doc/primitives.cpp` — the line-drawing primitive itself
- `third_party/zingl-bresenham/` or equivalent vendor location

### Algorithm walkthrough

Plain Bresenham line, integer-only:

```
function line(x0, y0, x1, y1):
    dx = abs(x1 - x0)
    dy = -abs(y1 - y0)
    sx = sign(x1 - x0)
    sy = sign(y1 - y0)
    err = dx + dy
    loop:
        plot(x0, y0)
        if x0 == x1 and y0 == y1: break
        e2 = 2 * err
        if e2 >= dy:
            err = err + dy
            x0 = x0 + sx
        if e2 <= dx:
            err = err + dx
            y0 = y0 + sy
```

This produces the doubled-corner shape. The "pixel-perfect" cleanup is a second pass that removes the second pixel of any two-pixel L: if `(x, y)`, `(x+sx, y)`, and `(x+sx, y+sy)` are all set, clear `(x+sx, y)`.

Aseprite's pixel-perfect freehand mode applies the cleanup not at the end of the line but live during the stroke. As the artist drags, the most recent dab is sometimes removed if the next dab would form an L with it. The artist sees a clean line without thinking about it. The state needed is: the last two dab positions and a one-frame lookahead, which Aseprite implements as a queue of pending dabs that commit only when the next dab arrives.

### Why the decomposition pays off

**Pixel-perfect-as-postprocess vs. pixel-perfect-as-rasterizer.** Aseprite's choice is the postprocess path: rasterize Bresenham normally, then remove corner doubles. This is simpler than designing a new rasterizer that avoids them in the first place, and it composes with the brush size (a 3-pixel brush gets its centerline pixel-perfected, the surrounding ring stays a normal Bresenham circle).

**Zingl's implementations are well-tested.** Zingl's "easyfilter" Bresenham collection covers lines, ellipses, Bezier curves, and antialiased variants. Each is small, well-commented, and widely cited. Porting them gets us a known-correct foundation for line, ellipse, and curve tools all at once.

### Our equivalent today

The current `core/canvas/` has a line-drawing primitive that calls into a generic rasterizer. Pixel-perfect mode is missing.

### Port plan

Port `primitives.cpp`'s line/circle/ellipse functions into `core/canvas/raster.rs`:

```rust
// core/canvas/raster.rs
// Ported from Aseprite (https://github.com/aseprite/aseprite)
//   Upstream: src/doc/primitives.cpp
//   Copyright (c) 2018-2025 Igara Studio S.A.
//   Copyright (c) 2001-2018 David Capello
//   Released under the MIT license. See LICENSES/aseprite-MIT.txt.
// Bresenham line-drawing portions:
//   Copyright (c) 2012-2016 Alois Zingl, MIT.
//
// This Rust port: Copyright (c) 2026 Pixhaus contributors. MIT.

pub fn line(mut x0: i32, mut y0: i32, x1: i32, y1: i32, mut plot: impl FnMut(i32, i32)) {
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = (x1 - x0).signum();
    let sy = (y1 - y0).signum();
    let mut err = dx + dy;

    loop {
        plot(x0, y0);
        if x0 == x1 && y0 == y1 { break; }
        let e2 = 2 * err;
        if e2 >= dy { err += dy; x0 += sx; }
        if e2 <= dx { err += dx; y0 += sy; }
    }
}
```

The pixel-perfect postprocess is a separate small function:

```rust
pub fn remove_corner_doubles(stroke: &mut Vec<(i32, i32)>) {
    // If stroke[i-1], stroke[i], stroke[i+1] form an L,
    // remove stroke[i] when it's the shared-corner pixel.
    let mut i = 1;
    while i + 1 < stroke.len() {
        let (xa, ya) = stroke[i - 1];
        let (xb, yb) = stroke[i];
        let (xc, yc) = stroke[i + 1];
        // L-shape detection
        if (xb == xa && yb == yc) || (yb == ya && xb == xc) {
            stroke.remove(i);
        } else {
            i += 1;
        }
    }
}
```

Use it from the FreehandController + AsLines path when `pixel_perfect` is enabled on the tool config.

### Attribution checklist

- `core/canvas/raster.rs` → upstream `src/doc/primitives.cpp`, with the Zingl Bresenham copyright preserved if the line / ellipse routines are direct ports.

## 7. Brush dynamics — `src/app/tools/dynamics.*` (EULA, inspire-only)

### What it does

Modulates dab size, opacity, and angle based on continuous input channels — primarily pen pressure, pen tilt, and stroke velocity. The artist sets up a curve for each (e.g., "pressure 0 -> size 1, pressure 1.0 -> size 12, linear") and the curve is applied per dab during a stroke.

### License status

EULA. Concept-only.

### How it's decomposed

Three input channels (pressure, tilt, velocity) map to three output parameters (size, opacity, angle) via a 3x3 matrix of opt-in per-channel curves. Each cell of the matrix is either "off" or "curve(input -> output)". Most artists enable two or three cells (pressure -> size, pressure -> opacity, velocity -> size for taper) and leave the rest off.

The Stroke struct carries pressure, tilt, and velocity per point. The Dynamics object reads from the Stroke and overrides the corresponding PointShape parameters at each dab. The override happens before the PointShape computes its footprint, so dynamics are upstream of the brush shape, not downstream.

### Why the decomposition pays off

**Matrix of curves keeps the data model flat.** A nested "this channel modulates this parameter via this function" tree gets unwieldy fast. The 3x3 matrix with per-cell opt-in curve is dense enough that the UI is "nine toggles plus nine curve widgets" — visually scannable, easy to author presets against.

**Velocity is a derived channel.** Pressure and tilt come from the pen driver. Velocity is computed in the Controller from time-stamped input events. Putting velocity on the same footing as the hardware channels means the Dynamics layer doesn't care where the channel originated.

### Our equivalent today

Not yet. Pressure data is captured via the browser PointerEvent (`event.pressure`); tilt is also available but unused; velocity is not derived.

### Port plan

Add to the Stroke struct:

```rust
pub struct StrokePoint {
    pub pos: Point,
    pub pressure: f32,    // 0.0 to 1.0
    pub tilt_x: f32,      // -1.0 to 1.0
    pub tilt_y: f32,      // -1.0 to 1.0
    pub velocity: f32,    // pixels per millisecond, derived
    pub timestamp_ms: u32,
}
```

Compute velocity in the FreehandController from successive timestamped events.

The dynamics config sits on the ToolConfig:

```rust
pub struct Dynamics {
    pub size:    DynamicsRow,
    pub opacity: DynamicsRow,
    pub angle:   DynamicsRow,
}

pub struct DynamicsRow {
    pub from_pressure: Option<Curve>,
    pub from_tilt:     Option<Curve>,
    pub from_velocity: Option<Curve>,
}

pub struct Curve {
    pub points: Vec<(f32, f32)>,  // (input, output) control points
    pub interpolation: CurveInterp,  // Linear | Spline | Step
}
```

Apply in ToolLoop just before PointShape:

```rust
fn resolve_dab(point: &StrokePoint, brush: &Brush, dynamics: &Dynamics) -> ResolvedDab {
    ResolvedDab {
        pos: point.pos,
        size: dynamics.size.apply(brush.size, point),
        opacity: dynamics.opacity.apply(brush.opacity, point),
        angle: dynamics.angle.apply(brush.angle, point),
    }
}
```

### Attribution checklist

None.

## 8. Selection mask model — `src/doc/mask.h` (MIT)

### What it does

Stores the selected pixels of a sprite as a bitmap mask plus a bounding rectangle. The mask is independent of the layer data — selection cuts across all layers — and supports the standard set algebra (union, intersection, difference) over rectangular selections, lasso polygons, magic-wand floodfills, and color-range matches. Modify-selection operations (expand, contract, border, feather) compute on the mask in place. The mask boundary is extracted on demand for marching-ants rendering.

### License status

MIT. Files:

- `src/doc/mask.h`, `mask.cpp`
- `src/doc/mask_boundaries.h`, `mask_boundaries.cpp`
- `src/doc/algorithm/modify_selection.h`, `modify_selection.cpp`
- `src/doc/algorithm/floodfill.h`, `floodfill.cpp` (used to seed magic-wand selection from a color match)
- `src/doc/algorithm/polygon.h`, `polygon.cpp` (used to seed lasso selection from a polygon)
- `src/doc/algorithm/stroke_selection.h`, `stroke_selection.cpp`
- `src/doc/algorithm/fill_selection.h`, `fill_selection.cpp`
- `src/doc/algorithm/shrink_bounds.h`, `shrink_bounds.cpp`

### How it's decomposed

A `Mask` owns:

- `bounds: gfx::Rect` — bounding rectangle of selected pixels (or empty for an empty mask)
- `bitmap: ImageRef` — a 1-bit image the size of `bounds` (zero outside `bounds`, by definition)
- `frozen: bool` — when frozen, mutating operations are deferred until unfreeze. Batch operations (apply 100 selection edits) freeze first and unfreeze once at the end.

Boolean operations work bitmap-on-bitmap, computing a new bounding rect by union or intersection of the operands' bounds and walking the overlap region only.

Magic wand seeds from a flood-fill into a fresh mask: starting at `(x, y)`, tag every pixel reachable by 4-connected (or 8-connected) traversal whose color is within tolerance of the seed. The flood-fill itself is the scanline algorithm — see section 9.

Lasso seeds from a polygon scan-conversion: for each scan line crossing the polygon, fill the runs between odd-even edge crossings.

Color-range matches every pixel of every layer (or the active layer) whose color is within tolerance of the picked color. Computationally identical to magic wand but without the connectivity constraint.

The boundary extraction (`mask_boundaries.cpp`) walks the mask and emits a list of edge segments. Marching-ants rendering animates a 1-bit dash pattern along these segments.

The modify-selection operations are textbook morphology:

- **Expand by N pixels** — dilate the mask by an N-radius structuring element (a 4-connected square or 8-connected square).
- **Contract by N pixels** — erode by the same element.
- **Border** — `expand(mask, N) AND NOT contract(mask, M)` for a ring of inner+outer width.
- **Feather** — convert the mask to a soft alpha by Gaussian-blurring the 1-bit bitmap into a `u8` mask. (Aseprite generally stays 1-bit; feather is the one operation that returns a soft mask.)

### Why the decomposition pays off

**Bounding rect + bitmap is the right space-time tradeoff.** A bare bitmap covering the full sprite would be wasteful when the selection is a small region. A pure bounding rect would lose interior detail (you couldn't lasso a U-shape). The combination is the minimum representation.

**Frozen flag for batched edits.** A "select all in palette index 7 across all 60 frames" operation could otherwise trigger 60 boundary recalculations. Freeze suppresses them, the final unfreeze does one.

**Boundary extraction is on demand, not maintained.** The mask doesn't keep the boundary in sync with mutations. The renderer asks for the boundary when it needs to draw marching ants, and the boundary code walks the bitmap fresh. This is fine because boundaries are tiny relative to mask size and the walk is O(perimeter), not O(area).

### Our equivalent today

`core/selection/` has masks with bounds + bitmap. Frozen flag and boundary extraction are present. The modify-selection operations exist for expand and contract but not border or feather.

### Port plan

Most of the surface area is already implemented. The port is to align the algorithm collection with upstream:

```rust
// core/selection/mask.rs
impl Mask {
    pub fn expand(&mut self, radius: u32) { /* dilate */ }
    pub fn contract(&mut self, radius: u32) { /* erode */ }
    pub fn border(&mut self, outer: u32, inner: u32) { /* expand AND NOT contract */ }
    pub fn feather(&self, radius: f32) -> SoftMask { /* Gaussian blur to alpha */ }
    pub fn stroke(&self, brush: &Brush) -> Cmd { /* paint a brush stroke along the boundary */ }
    pub fn fill(&self, color: Rgba8) -> Cmd { /* fill the selected region with color */ }
}
```

Magic-wand and lasso are seeding functions returning a new Mask:

```rust
pub fn magic_wand(image: &Image<P>, seed: Point, tolerance: u8, connectivity: Connectivity) -> Mask {
    let mut mask = Mask::empty(image.size());
    floodfill_into_mask(image, seed, tolerance, connectivity, &mut mask);
    mask
}

pub fn lasso(polygon: &[Point]) -> Mask { /* scan-convert polygon into a mask */ }

pub fn color_range(image: &Image<P>, target: Rgba8, tolerance: u8) -> Mask {
    let mut mask = Mask::empty(image.size());
    for (x, y, p) in image.pixels() {
        if color_distance(p, target) <= tolerance {
            mask.set(x, y);
        }
    }
    mask
}
```

For dilate/erode, the structuring element is small (radius typically 1-32 pixels), so a brute-force convolution suffices. Boundary extraction stays as it is.

### Attribution checklist

- `core/selection/mask.rs` → upstream `src/doc/mask.h`, `mask.cpp`
- `core/selection/boundary.rs` → upstream `src/doc/mask_boundaries.h`, `mask_boundaries.cpp`
- `core/selection/modify.rs` → upstream `src/doc/algorithm/modify_selection.h`, `modify_selection.cpp`
- `core/selection/stroke_fill.rs` → upstream `src/doc/algorithm/stroke_selection.cpp`, `fill_selection.cpp`

## 9. Flood fill — `src/doc/algorithm/floodfill.cpp` (MIT)

### What it does

Starting from a seed pixel, marks every pixel reachable via 4-connected (orthogonal-only) or 8-connected (orthogonal + diagonal) traversal whose color matches the seed within a tolerance. Used for the paint-bucket tool, magic-wand selection, and any operation that wants to identify a contiguous region.

### License status

MIT. File: `src/doc/algorithm/floodfill.cpp` (590 lines — substantial, because it handles tolerance, contiguous vs. non-contiguous, and the various color modes).

### Algorithm walkthrough

The naive recursive flood fill (DFS) blows the stack on large regions. Aseprite uses the scanline floodfill, which is the standard correct implementation:

```
function floodfill(image, seed_x, seed_y, target_color, tolerance):
    queue = [(seed_x, seed_y)]
    while queue not empty:
        (x, y) = queue.pop()
        if not within_tolerance(image[x, y], target_color, tolerance):
            continue
        # Extend left
        left = x
        while left > 0 and within_tolerance(image[left - 1, y], target_color, tolerance):
            left -= 1
        # Extend right
        right = x
        while right < width - 1 and within_tolerance(image[right + 1, y], target_color, tolerance):
            right += 1
        # Mark scanline as filled
        for fx in left..=right:
            mark(fx, y)
        # Push above and below scanlines
        for fx in left..=right:
            if y > 0 and within_tolerance(image[fx, y - 1], target_color, tolerance) and not marked(fx, y - 1):
                queue.push((fx, y - 1))
            if y < height - 1 and within_tolerance(image[fx, y + 1], target_color, tolerance) and not marked(fx, y + 1):
                queue.push((fx, y + 1))
```

The scanline trick reduces the queue size by a factor of (region width), because instead of pushing every pixel of a wide region, we push only the seeds for the rows above and below. On a 1024×1024 fully-connected fill, the queue stays under a few thousand entries instead of a million.

Tolerance is L∞ distance in RGB (max of |dr|, |dg|, |db|) or in palette index for indexed images. Aseprite's tolerance handling distinguishes "tolerance to the original seed color" (default) from "tolerance to each just-touched pixel" (which would let the fill drift across smooth gradients — generally unwanted).

The 8-connected variant adds diagonal scanline-seeds. Most pixel-art work is 4-connected because 8-connected fills bleed through single-pixel diagonal gaps in a way artists rarely want.

### Why the decomposition pays off

**Iterative + scanline avoids stack blowup.** A 4096×4096 sprite with a full-canvas fill on a recursive implementation crashes. The scanline iterative version handles it in tens of milliseconds.

**Tolerance from the seed color, not the running color.** Pixel-art palettes have abrupt boundaries by design. Drifting tolerance would let the fill leak into neighboring palette entries. Fixed-from-seed tolerance keeps the fill within the artist's intended region.

### Our equivalent today

`core/canvas/floodfill.rs` exists with the 4-connected scanline algorithm. The 8-connected variant and the per-color-mode tolerance handling should be reviewed against upstream.

### Port plan

The current implementation is close; the port is incremental rather than wholesale. Verify:

1. The 8-connected variant exists and is exposed through the API.
2. Tolerance is computed in the source color space (RGB for RGBA images, value for grayscale, palette-index distance for indexed).
3. The "contiguous" vs. "non-contiguous" flag is supported (non-contiguous is "fill all pixels matching the color, regardless of reachability" — that's just `color_range` from section 8 but applied as a fill, not a selection).
4. The result respects the existing selection (the fill is clipped to the active mask).

### Attribution checklist

- `core/canvas/floodfill.rs` → upstream `src/doc/algorithm/floodfill.h`, `floodfill.cpp`

## 10. Undo command system — `src/app/cmd*` (EULA) + `src/undo/` (MIT)

### What it does

Records every document-mutating operation as a Command object that knows how to apply itself, how to invert itself (for undo), and how to estimate its own memory cost. The undo library underneath provides a non-linear undo history — you can undo, branch into a new edit, and the previous branch is preserved as a sibling rather than discarded.

### License status

**Split.** The 97 concrete Cmd subclasses in `src/app/cmd/` are EULA. The generic undo library in `src/undo/` (and its external GitHub mirror) is MIT. We port the library and reconstruct the 97 commands fresh.

Files:

- MIT: `src/undo/undo_history.h`, `undo_history.cpp`, `undo_command.h`, `undo_state.h`
- EULA: `src/app/cmd.h`, `src/app/cmd_sequence.h`, `src/app/cmd/*.h`, `src/app/cmd/*.cpp` (97 files)

### How it's decomposed

Generic undo library (`src/undo/`):

- `UndoCommand` — abstract base with `onExecute`, `onUndo`, `onRedo`, `memSize` virtual hooks.
- `UndoHistory` — owns a tree of `UndoState` nodes, each wrapping an `UndoCommand`. The tree supports linear traversal (the most common case) and branching: when you undo into the middle of history and then make a new edit, the prior linear continuation becomes a sibling branch rather than being discarded.
- Memory bounding: the history can be told to bound its memory consumption; when the bound is exceeded, the oldest leaf states are evicted.
- Observer signals (provided via the `observable` library) fire on undo, redo, branch creation, eviction.

Aseprite application layer (`src/app/cmd*`):

- `Cmd` extends `UndoCommand` with Aseprite-specific context: a `Context*` pointing at the host document, hooks for `onFireNotifications` to notify observers, and the `WithSprite`, `WithCel`, `WithImage` mixins for typed access to the document under modification.
- `CmdSequence` is a `Cmd` that aggregates child Cmds. The whole sequence undoes as one. This is how a single stroke (which might modify multiple cels for symmetry) commits atomically.
- The 97 concrete subclasses cover: layer add/remove/move/rename/setblend/setopacity, frame add/remove/duplicate/move, cel add/remove/move/copy/clear/replaceimage, palette add/remove/replace/setname/remapcolors, tag add/remove/move/rename, slice add/remove/move/rename, tileset add/remove/replacetile, user-data property set/remove. Each subclass stores the pre-image and post-image of its target and replays them on undo/redo.

The mixin pattern reduces boilerplate. A typical Cmd that mutates a Cel inherits from `WithCel`, which provides typed `cel()` access and handles cel ID resolution. Without mixins each Cmd would copy that boilerplate; with them the concrete class is ~30 lines instead of ~70.

### Why the decomposition pays off

**Tree history, not linear stack.** A linear undo stack discards "future" history on the first new edit after an undo. That's the source of "I lost my work when I tried something different" complaints. The branching tree keeps the alternate timeline available; the artist can switch back to it via a history-tree UI (Aseprite has one) or via keyboard shortcuts. Pixel art is iterative — branching matters.

**Memory bounding by leaf eviction, not by truncation.** When the history exceeds its memory budget, the oldest *leaves* of the tree get pruned, not the entire deepest line. This preserves the structure of recent edits while letting old experiments fall away.

**Mixins for typed access.** Without `WithCel`, every Cel-mutating Cmd would replicate the cel-ID lookup, the typed cast, the not-found error handling. With it, those 12 lines are written once. The 97 commands are tractable to maintain only because of mixins.

### Our equivalent today

`core/undo/` has a command-tree history. Branching is supported. The mixin pattern is not present — each Cmd defines its own boilerplate.

### Port plan

**Step 1: Port the generic undo library.** This is the MIT bit. The Rust translation:

```rust
// core/undo/history.rs (port of src/undo/undo_history.{h,cpp})
pub trait UndoCommand: Send + 'static {
    fn execute(&mut self);
    fn undo(&mut self);
    fn redo(&mut self) { self.execute(); }
    fn mem_size(&self) -> usize;
    fn label(&self) -> &str;
}

pub struct UndoHistory {
    root: UndoNode,
    current: NodeId,
    memory_bound: usize,
    on_change: Signal<HistoryEvent>,
}

pub struct UndoNode {
    cmd: Box<dyn UndoCommand>,
    parent: Option<NodeId>,
    children: Vec<NodeId>,
}

impl UndoHistory {
    pub fn add(&mut self, cmd: Box<dyn UndoCommand>) { /* push as new child of current */ }
    pub fn undo(&mut self) { /* call current.cmd.undo(), move current to parent */ }
    pub fn redo(&mut self) { /* if current has children, move to one and call execute */ }
    pub fn switch_branch(&mut self, child_idx: usize) { /* pick a different child */ }
    pub fn evict_to_fit(&mut self) { /* drop oldest leaves until under memory_bound */ }
}
```

**Step 2: Reconstruct the mixin pattern in Rust.** Mixins translate naturally to trait blanket impls:

```rust
// core/undo/cmd.rs

pub trait WithSprite {
    fn sprite_id(&self) -> SpriteId;
}

pub trait WithCel: WithSprite {
    fn layer_id(&self) -> LayerId;
    fn frame_id(&self) -> FrameId;
}

pub trait WithImage: WithCel {
    fn image_pre(&self) -> &Image<Rgba>;
    fn image_post(&self) -> &Image<Rgba>;
}

// A concrete command
pub struct SetCelOpacity {
    pub sprite: SpriteId,
    pub layer: LayerId,
    pub frame: FrameId,
    pub old_opacity: u8,
    pub new_opacity: u8,
}

impl WithSprite for SetCelOpacity { fn sprite_id(&self) -> SpriteId { self.sprite } }
impl WithCel for SetCelOpacity {
    fn layer_id(&self) -> LayerId { self.layer }
    fn frame_id(&self) -> FrameId { self.frame }
}

impl UndoCommand for SetCelOpacity {
    fn execute(&mut self) { /* set cel.opacity = self.new_opacity */ }
    fn undo(&mut self) { /* set cel.opacity = self.old_opacity */ }
    fn mem_size(&self) -> usize { size_of::<Self>() }
    fn label(&self) -> &str { "Set cel opacity" }
}
```

The 97 commands won't all come at once — port them as the corresponding features land in Pixhaus. The bedrock is the trait taxonomy plus a handful of canonical commands (`ReplaceCelImage`, `AddLayer`, `RemoveLayer`, `AddFrame`, `RemoveFrame`, `ReplacePalette`, `AddCel`, `RemoveCel`, `SetUserDataProperty`, `CmdSequence`) that cover most of the operations.

### Attribution checklist

- `core/undo/history.rs` → upstream `src/undo/undo_history.h`, `undo_history.cpp`, `undo_command.h`
- `core/undo/state.rs` → upstream `src/undo/undo_state.h`

The concrete Cmd subclasses are not ports (EULA territory), so they carry only the Pixhaus copyright.

## 11. Color and palette — `src/doc/color.h`, `palette.h`, `rgbmap*.h` (MIT)

### What it does

Stores color values, palette tables (frame-indexed for palette-animation support), and the fast RGB→indexed lookup table used during indexed-mode painting and palette quantization. The color modes (RGBA, Grayscale, Indexed) are handled uniformly through the same APIs.

### License status

MIT. Files:

- `src/doc/color.h` — color_t type and shift/mask helpers
- `src/doc/palette.h`, `palette.cpp` — Palette class
- `src/doc/rgbmap.h`, `rgbmap_algorithm.h` — abstract RgbMap interface
- `src/doc/rgbmap_rgb5a3.h`, `rgbmap_rgb5a3.cpp` — 16-bit-quantized lookup table
- `src/doc/octree_map.h`, `octree_map.cpp` — octree-based progressive lookup
- `src/doc/color_scales.h` — bit-depth scaling helpers

### How it's decomposed

`color_t` is a `uint32_t` with three layouts:

- **RGBA mode:** R in bits 0-7, G in 8-15, B in 16-23, A in 24-31. Macros `rgba(r,g,b,a)`, `rgba_getr(c)`, etc. extract fields.
- **Grayscale mode:** Value in bits 0-7, Alpha in 8-15. Other bits zero.
- **Indexed mode:** Palette index in bits 0-7. Other bits zero.

A `Palette` owns up to 256 `color_t` entries plus a `frame_t` indicating which frame this palette applies to. A Sprite owns one or more palettes ordered by frame; the active palette for frame F is the most recent palette with `frame <= F`. This is how palette animation works at the data-model level.

The `RgbMap` is an abstract base providing `mapColor(r, g, b, a) -> index`: given an RGBA, return the best matching palette index. Two implementations:

- **RgbMapRGB5A3** — a 64KB lookup table indexed by RGB5A3 (5 bits R, 5 G, 5 B, 3 A). For each RGB5A3 cell, store the nearest palette index. Lookup is O(1). Construction is O(palette_size * 32768).
- **OctreeMap** — a sparse octree of RGB samples, where each leaf records the dominant palette index for its octant. Progressive: you can build it from a sample of pixels (e.g., 1 in 16) and refine it later. Used for quantization, where the source is millions of RGB samples and the target is 256 indices.

The `color_scales.h` helpers bridge bit depths — 5-bit-to-8-bit value scaling, etc. — used when converting from RGB5 to RGB8 and back.

### Why the decomposition pays off

**RGB5A3 lookup table is the cheapest correct nearest-neighbor query.** A linear scan of 256 palette entries on every pixel of a 4096×4096 sprite is ~4 billion compares. A 64KB lookup table is 16M cells, each filled once at palette change, and queried with one memory access per pixel. The win is two to three orders of magnitude.

**Frame-indexed palettes for palette animation.** Indexed sprites often cycle their palette across frames — e.g., a flickering torch where the palette shifts in a 4-frame loop while the indexed image stays static. The most-recent-palette-by-frame rule lets the same image bytes look different per frame at no memory cost.

**Octree quantization for the build phase.** Median cut (covered in section 13) and octree quantization both build a palette from a source image. Octree is better for images with many similar colors clustered (typical pixel-art import from PNG); median cut is better for evenly-distributed gradients. Aseprite has both; pick by content.

### Our equivalent today

`core/color/` has palette types and an indexed-to-RGBA conversion path. Frame-indexed palettes are present at the data model. The RgbMap cache is missing; nearest-neighbor lookup is linear today.

### Port plan

**Phase 1: RgbMapRGB5A3.** Build the 64KB lookup table on palette change, query it on every indexed-mode pixel write or RGB→indexed conversion.

```rust
// core/color/rgbmap.rs (port of src/doc/rgbmap_rgb5a3.{h,cpp})

pub struct RgbMap {
    table: Box<[u8; 32 * 32 * 32 * 8]>,  // 5 bits R, 5 G, 5 B, 3 A
    palette: Palette,
}

impl RgbMap {
    pub fn build(palette: &Palette) -> Self {
        let mut table = vec![0u8; 32 * 32 * 32 * 8].into_boxed_slice().try_into().unwrap();
        for r5 in 0..32 {
            for g5 in 0..32 {
                for b5 in 0..32 {
                    for a3 in 0..8 {
                        let r = scale_5_to_8(r5);
                        let g = scale_5_to_8(g5);
                        let b = scale_5_to_8(b5);
                        let a = scale_3_to_8(a3);
                        table[index_of(r5, g5, b5, a3)] = nearest_in_palette(palette, r, g, b, a);
                    }
                }
            }
        }
        RgbMap { table, palette: palette.clone() }
    }

    pub fn lookup(&self, color: Rgba8) -> u8 {
        let r5 = color.r >> 3;
        let g5 = color.g >> 3;
        let b5 = color.b >> 3;
        let a3 = color.a >> 5;
        self.table[index_of(r5, g5, b5, a3)]
    }
}
```

**Phase 2: OctreeMap for quantization paths.** Used when constructing a palette from source pixels rather than mapping into an existing palette.

**Phase 3: Frame-indexed palette accessor.** Cache the active palette per frame for fast lookup during rendering:

```rust
impl Sprite {
    pub fn palette_at(&self, frame: FrameId) -> &Palette {
        self.palettes
            .iter()
            .rev()
            .find(|p| p.frame <= frame)
            .unwrap_or(&self.palettes[0])
    }
}
```

### Attribution checklist

- `core/color/color.rs` → upstream `src/doc/color.h`
- `core/color/palette.rs` → upstream `src/doc/palette.h`, `palette.cpp`
- `core/color/rgbmap.rs` → upstream `src/doc/rgbmap.h`, `rgbmap_algorithm.h`, `rgbmap_rgb5a3.h`, `rgbmap_rgb5a3.cpp`
- `core/color/octree.rs` → upstream `src/doc/octree_map.h`, `octree_map.cpp`
- `core/color/scales.rs` → upstream `src/doc/color_scales.h`

## 12. Quantization — `src/render/quantization.cpp` (MIT)

### What it does

Reduces a source image to a target palette of N colors, picking the N colors that minimize the total color error across the source. Used when converting an RGBA image to indexed mode, and when generating a new palette from a multi-color source.

### License status

MIT. Files:

- `src/render/quantization.h`, `quantization.cpp`
- `src/render/median_cut.h` — median-cut implementation
- `src/render/color_histogram.h` — histogram for sample counting

### Algorithm walkthrough: median cut

Median cut is the textbook quantization algorithm:

1. **Build a histogram** of every RGB sample in the source. Each unique color counts how many pixels have it.
2. **Initialize one bucket** containing every sample, with bounds the full RGB cube.
3. **Repeat until you have N buckets:**
   a. Pick the bucket with the largest range along any RGB axis.
   b. Sort its samples along the largest-range axis.
   c. Split the bucket at the median sample (or the population-weighted median for better fidelity).
4. **For each bucket, output the average color** of its samples (or the population-weighted average) as one palette entry.

The split-at-median rule is what gives the algorithm its name. The intuition: a bucket that is wide along R is being asked to represent many R values with one color; split it along R to halve the error.

Aseprite's implementation in `median_cut.h` follows this approach with two refinements:

- **Importance weighting.** Bright colors and saturated colors get slightly higher weight than dull ones, on the assumption that artists notice errors in saturated colors more than errors in muted ones. The weight is a small multiplier on the sample count.
- **Reserve transparent entry.** When quantizing for an indexed sprite, palette index 0 is reserved as transparent. The algorithm produces N-1 buckets, with index 0 left as a fixed "transparent" color.

### Algorithm walkthrough: octree quantization

Octree quantization is the alternative for content with many clustered colors:

1. **Build an octree** with 8 levels (one per bit of the 8-bit color channels). Each leaf represents one of 16M possible RGB values. Each internal node aggregates the samples of its 8 children.
2. **For each source pixel, descend the tree**, incrementing the count at each level. By the end, every relevant subtree has a population.
3. **Reduce by collapsing leaves into parents** until the leaf count is N. At each step, collapse the leaf with the smallest population — that minimizes the worst-case error.
4. **Output one palette entry per remaining leaf**, with the entry's color being the average of the collapsed colors.

Octree is good when the source is a photo-like image with many clustered colors. Median cut is better when the source is a gradient or a synthetic image with evenly distributed colors.

### Why the decomposition pays off

**Two algorithms because content varies.** Aseprite users can hand-paint sprites (clustered, octree-friendly) or import photographic references (continuous, median-cut-friendly). Offering both with a UI toggle costs little code and serves both audiences.

**Histogram-first construction.** Both algorithms work from a histogram, not from raw pixels. The histogram pass is O(pixels); the quantization pass is O(unique_colors * log(unique_colors)), often orders of magnitude smaller. Separating them means the expensive pass scales with image variety, not image size.

### Our equivalent today

`core/color/` has a quantization entry point but only median cut. Octree is missing.

### Port plan

```rust
// core/color/quantize.rs
pub enum QuantizeAlgorithm {
    MedianCut,
    Octree,
}

pub fn quantize(image: &Image<Rgba>, target_count: usize, algo: QuantizeAlgorithm) -> Palette {
    let histogram = ColorHistogram::from_image(image);
    match algo {
        QuantizeAlgorithm::MedianCut => median_cut(histogram, target_count),
        QuantizeAlgorithm::Octree => octree_quantize(histogram, target_count),
    }
}

fn median_cut(hist: ColorHistogram, n: usize) -> Palette { /* ... */ }
fn octree_quantize(hist: ColorHistogram, n: usize) -> Palette { /* ... */ }
```

### Attribution checklist

- `core/color/quantize.rs` → upstream `src/render/quantization.h`, `quantization.cpp`
- `core/color/median_cut.rs` → upstream `src/render/median_cut.h`
- `core/color/histogram.rs` → upstream `src/render/color_histogram.h`

## 13. Dithering — `src/render/ordered_dither.*`, `error_diffusion.*` (MIT)

### What it does

Converts a high-bit-depth color value to a low-bit-depth palette entry by spatially distributing the error so that the local average matches the source even when individual pixels can only pick from a small palette. The two flavors are *ordered* (use a pre-computed threshold matrix to decide which way to round each pixel) and *error diffusion* (propagate the rounding error to neighboring pixels).

### License status

MIT. Files:

- `src/render/ordered_dither.h`, `ordered_dither.cpp`
- `src/render/error_diffusion.h`, `error_diffusion.cpp`
- `src/render/dithering.h` — common dispatch
- `src/render/dithering_matrix.h` — Bayer matrices
- `src/render/dithering_algorithm.h` — enum of algorithms

### Algorithm walkthrough: ordered dither (Bayer)

The threshold matrix is a small (often 4×4 or 8×8) grid of values in [0, 1) chosen so that any rectangular sub-region averages to roughly 0.5. The classic 4×4 Bayer matrix:

```
( 0  8  2 10) / 16
(12  4 14  6) / 16
( 3 11  1  9) / 16
(15  7 13  5) / 16
```

For each pixel `(x, y)` with source value `v` (a float in [0, 1]):

```
threshold = bayer[(y mod 4), (x mod 4)]
output_index = nearest_palette_index(v + (threshold - 0.5) * dither_strength)
```

The `+ (threshold - 0.5)` shifts the rounding boundary by up to half a quantization step in each direction. Pixels in "high" cells of the matrix round one way; pixels in "low" cells round the other. The local 4×4 average matches `v`.

Ordered dither has two crucial properties for pixel art:

- **Stable under animation.** A pixel at `(x, y)` always uses the same threshold regardless of the surrounding image. When the frame changes (because the artist drew somewhere else), the dither doesn't ripple. This is what makes ordered dither the pixel-art default.
- **Tileable.** A 4×4 Bayer matrix wraps; a tiled rendering of a dithered region tiles seamlessly. Error diffusion does not have this property.

### Algorithm walkthrough: Floyd-Steinberg error diffusion

Walk pixels in scanline order. For each pixel:

1. Pick the nearest palette entry to the current value.
2. Compute the error `e = source - chosen_palette_value`.
3. Propagate the error to forward neighbors:
   - `(x+1, y)` gets `7/16 * e`
   - `(x-1, y+1)` gets `3/16 * e`
   - `(x, y+1)` gets `5/16 * e`
   - `(x+1, y+1)` gets `1/16 * e`

The weights sum to 16/16 = 1, so the total error is conserved across the propagation. The asymmetric weighting (more error pushed forward and down) is what gives Floyd-Steinberg its characteristic "moving wavefront" look — areas being rasterized later carry the accumulated error of areas already rasterized.

Floyd-Steinberg produces higher-fidelity output than ordered dither for static images. The downsides:

- **Unstable under animation.** A change in pixel A propagates as error to pixels B, C, D, which propagate to E, F, G — a single-pixel edit causes a long ripple. Two frames of an animation dithered separately don't line up; the dithered regions twinkle visibly.
- **Not tileable.** The wavefront depends on starting position, so tiled output has visible seams.

### Why the decomposition pays off

**Ordered as the default, error diffusion as the option.** Aseprite ships both. The default for indexed-mode painting and palette-reduction is ordered dither because of its animation stability. Error diffusion is available as an export-time option for single-frame static deliverables.

**Per-pixel-threshold lookup.** Computing the threshold from `(x, y)` is a pair of modulos and a table fetch. The hot path is two instructions plus the palette lookup itself. Suitable for full-image processing on a single thread without breaking a sweat.

### Our equivalent today

`core/color/` has a placeholder for dithering. Bayer matrices and error diffusion are both missing as shipped algorithms; they're spec'd in B3.

### Port plan

```rust
// core/color/dither.rs

pub enum DitherAlgorithm {
    Ordered(BayerSize),
    FloydSteinberg,
    JarvisJudsonNinke,
}

pub enum BayerSize { B2x2, B4x4, B8x8 }

pub fn dither_to_indexed(
    src: &Image<Rgba>,
    palette: &Palette,
    rgbmap: &RgbMap,
    algo: DitherAlgorithm,
) -> Image<Indexed> {
    match algo {
        DitherAlgorithm::Ordered(size) => ordered_dither(src, palette, rgbmap, size),
        DitherAlgorithm::FloydSteinberg => fs_dither(src, palette, rgbmap),
        DitherAlgorithm::JarvisJudsonNinke => jjn_dither(src, palette, rgbmap),
    }
}

const BAYER_4: [[u8; 4]; 4] = [
    [ 0,  8,  2, 10],
    [12,  4, 14,  6],
    [ 3, 11,  1,  9],
    [15,  7, 13,  5],
];

fn ordered_dither(src: &Image<Rgba>, palette: &Palette, rgbmap: &RgbMap, size: BayerSize) -> Image<Indexed> {
    let mut out = Image::<Indexed>::new(src.size());
    for (x, y, rgba) in src.pixels() {
        let t = BAYER_4[(y as usize) & 3][(x as usize) & 3] as f32 / 16.0;
        let shifted = shift_rgba(rgba, (t - 0.5) * (255.0 / 16.0));
        out.set(x, y, rgbmap.lookup(shifted));
    }
    out
}

fn fs_dither(src: &Image<Rgba>, palette: &Palette, rgbmap: &RgbMap) -> Image<Indexed> {
    // Work on a float-precision scratch buffer so propagated error survives.
    let mut scratch: Vec<[f32; 4]> = src.pixels().map(|(_, _, c)| [c.r as f32, c.g as f32, c.b as f32, c.a as f32]).collect();
    let mut out = Image::<Indexed>::new(src.size());
    let w = src.size().w as usize;
    let h = src.size().h as usize;
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let p = scratch[i];
            let chosen = rgbmap.lookup(rgba_from(p));
            out.set(x as i32, y as i32, chosen);
            let target = palette.get(chosen);
            let err = [p[0] - target.r as f32, p[1] - target.g as f32, p[2] - target.b as f32, p[3] - target.a as f32];
            // 7/16 to (x+1, y)
            if x + 1 < w { add(&mut scratch[y * w + (x + 1)], err, 7.0 / 16.0); }
            // 3/16 to (x-1, y+1)
            if y + 1 < h && x > 0 { add(&mut scratch[(y + 1) * w + (x - 1)], err, 3.0 / 16.0); }
            // 5/16 to (x, y+1)
            if y + 1 < h { add(&mut scratch[(y + 1) * w + x], err, 5.0 / 16.0); }
            // 1/16 to (x+1, y+1)
            if y + 1 < h && x + 1 < w { add(&mut scratch[(y + 1) * w + (x + 1)], err, 1.0 / 16.0); }
        }
    }
    out
}
```

Jarvis-Judson-Ninke is the same shape with a 12-neighbor weight set instead of FS's 4-neighbor set.

### Attribution checklist

- `core/color/dither.rs` → upstream `src/render/ordered_dither.h`, `ordered_dither.cpp`, `error_diffusion.h`, `error_diffusion.cpp`, `dithering.h`, `dithering_matrix.h`, `dithering_algorithm.h`

## 14. RotSprite rotation — `src/doc/algorithm/rotsprite.cpp` (MIT)

### What it does

Rotates a pixel-art image by an arbitrary angle while preserving the pixel-art edge characteristics. Standard bilinear rotation softens edges and dilutes colors with neighbors; nearest-neighbor rotation produces stair-stepping; RotSprite produces clean, pixel-art-aesthetic results at non-cardinal angles.

### License status

MIT. Files:

- `src/doc/algorithm/rotsprite.h`, `rotsprite.cpp`
- `src/doc/algorithm/rotate.h`, `rotate.cpp` — generic rotation (used by RotSprite internally)

Note from `rotsprite.cpp` header comment: the implementation uses EPX/Scale2x as the upscaling step. References to Scale2x and EPX algorithms are public-domain or in the supplementary materials of academic publications; the source in the upstream tree is original Aseprite code carrying the MIT license.

### Algorithm walkthrough: RotSprite

The Xenowhirl RotSprite algorithm is:

1. **Upscale 8× using EPX (or Scale2× applied three times).** EPX is a deterministic pixel-art upscaler: for each source pixel, look at its 4-connected neighbors and decide whether to break the upscaled 2×2 block into a smoother arrangement based on which neighbors match.
2. **Rotate the upscaled image using nearest-neighbor at the target angle.** At 8× scale, the pixel grid is dense enough that nearest-neighbor rotation produces convincing curves and diagonals.
3. **Downscale back to 1× by 8× area averaging — but with a pixel-art-aware cleanup.** Each downsampled output pixel takes the most-common color from its 8×8 source block, not the average. This preserves palette membership; bilinear averaging would invent new colors that aren't in the palette.

Step 3 is where RotSprite differs from "upscale, rotate, downscale" approaches that produce smoothed-but-not-pixel-art output. The mode-not-mean downsampling is what keeps the result on-palette.

### Algorithm walkthrough: EPX upscale

EPX 2× takes one source pixel `P` and its four 4-neighbors `A` (above), `B` (right), `C` (left), `D` (below). It outputs a 2×2 block:

```
    A
  C P B
    D

Output 2x2:
  | 1 | 2 |
  | 3 | 4 |

1 = (C == A and C != D and A != B) ? A : P
2 = (A == B and A != C and B != D) ? B : P
3 = (D == C and D != B and C != A) ? C : P
4 = (B == D and B != A and D != C) ? D : P
```

The rule says "if my upper-left corner is where two matching neighbors meet (suggesting a corner of a region), output the neighbor color; otherwise output the center." This catches "L" shapes and produces smoother diagonal transitions.

8× is just 2× applied three times: 1× → 2× → 4× → 8×.

### Why the decomposition pays off

**Pixel-art rotation is not generic image rotation.** Off-the-shelf libraries (Skia, Cairo, ImageMagick) produce wrong results for pixel art because they assume continuous-tone source. RotSprite encodes the "edges are discrete, not soft" assumption directly.

**Upscale-then-downscale beats direct rotation.** A direct nearest-neighbor rotation at 1× produces stairstepping that's visible at every angle except multiples of 90°. The 8× intermediate gives the rotation enough subpixel headroom that the stairsteps disappear when downsampled.

**Mode-based downsampling keeps the palette.** Mean-based downsampling would average across palette boundaries, producing colors not in the palette. Mode-based downsampling picks the most-common color in each block, which is always a palette member.

### Our equivalent today

`core/transforms/` has a rotation primitive but it's currently nearest-neighbor at 1×. Per the project planning docs, RotSprite is on the bedrock spec but not yet implemented. Verify before assuming.

### Port plan

```rust
// core/transforms/rotsprite.rs

pub fn rotsprite(src: &Image<Indexed>, angle_radians: f32) -> Image<Indexed> {
    let up = epx_upscale_8x(src);                  // 8x
    let rotated = nearest_neighbor_rotate(&up, angle_radians);
    mode_downsample_8x(&rotated)
}

fn epx_upscale_2x<P: PixelFormat>(src: &Image<P>) -> Image<P> {
    let mut dst = Image::<P>::new(src.size() * 2);
    for y in 0..src.size().h as i32 {
        for x in 0..src.size().w as i32 {
            let p = src.get(x, y);
            let a = src.get_or(x, y - 1, p);
            let b = src.get_or(x + 1, y, p);
            let c = src.get_or(x - 1, y, p);
            let d = src.get_or(x, y + 1, p);
            let q1 = if c == a && c != d && a != b { a } else { p };
            let q2 = if a == b && a != c && b != d { b } else { p };
            let q3 = if d == c && d != b && c != a { c } else { p };
            let q4 = if b == d && b != a && d != c { d } else { p };
            dst.set(x * 2,     y * 2,     q1);
            dst.set(x * 2 + 1, y * 2,     q2);
            dst.set(x * 2,     y * 2 + 1, q3);
            dst.set(x * 2 + 1, y * 2 + 1, q4);
        }
    }
    dst
}

fn epx_upscale_8x<P: PixelFormat>(src: &Image<P>) -> Image<P> {
    epx_upscale_2x(&epx_upscale_2x(&epx_upscale_2x(src)))
}

fn mode_downsample_8x<P: PixelFormat>(src: &Image<P>) -> Image<P> {
    let mut dst = Image::<P>::new(src.size() / 8);
    for y in 0..dst.size().h as i32 {
        for x in 0..dst.size().w as i32 {
            let mut counts: HashMap<P::Pixel, u32> = HashMap::new();
            for dy in 0..8 {
                for dx in 0..8 {
                    *counts.entry(src.get(x * 8 + dx, y * 8 + dy)).or_default() += 1;
                }
            }
            let mode = counts.into_iter().max_by_key(|&(_, c)| c).map(|(p, _)| p).unwrap();
            dst.set(x, y, mode);
        }
    }
    dst
}
```

For RGBA mode the mode is taken over RGBA tuples rather than palette indices; the rest is unchanged.

### Attribution checklist

- `core/transforms/rotsprite.rs` → upstream `src/doc/algorithm/rotsprite.h`, `rotsprite.cpp`
- `core/transforms/epx.rs` → upstream `src/doc/algorithm/rotate.cpp` (EPX/Scale2x portions)

## 15. Onion-skinning — `src/render/onionskin_*.h`, `src/render/render.cpp` (MIT)

### What it does

Renders past and future frames as semi-transparent ghosts behind the active frame so the artist can see the surrounding context while drawing. Each ghost frame can have its own opacity, tinting (warmer for past, cooler for future is conventional), and the number of past / future frames to show is configurable.

### License status

MIT. Files:

- `src/render/onionskin_options.h`
- `src/render/onionskin_type.h`
- `src/render/onionskin_position.h`
- Composition logic in `src/render/render.cpp` (within the Render class's frame composition)

### How it's decomposed

`OnionskinOptions` carries:

- `frames_before`, `frames_after` — how many frames to show on each side of the active frame
- `opacity_base` — base opacity for the nearest ghost frame
- `opacity_step` — decrement per frame distance (so further frames are more transparent)
- `position` — Behind (ghosts behind active), In Front, Both
- `type` — Merge (composite all ghosts into one ghost layer) vs. Red/Blue Tint (past=red, future=blue, no flatten)
- `loop_tag` — optional name of an animation Tag to restrict ghosting within (so ghosts don't show frames from a different animation)

The Render class composes a frame in passes:

1. (optional) past ghost frames composited at their reduced opacity, possibly tinted
2. active frame's layers, in layer order, with their blend modes
3. (optional) future ghost frames composited at their reduced opacity, possibly tinted
4. selection mask overlay (marching ants)
5. brush preview overlay (if a tool is mid-stroke)

The pass structure means onion-skin is fully separable from the main composition. A renderer that doesn't care about onion-skin (e.g., a sprite-sheet exporter) skips passes 1 and 3.

### Why the decomposition pays off

**Onion skin as a render pass, not a layer mutation.** A naive implementation might add temporary "ghost layers" to the sprite during rendering. That breaks: it inflates the document tree, confuses tools that count layers, and tangles the rendering data with the document data. The pass-based approach keeps onion-skin entirely in the renderer; the document is unchanged.

**Loop-tag scoping prevents visual noise.** When the artist sets up multiple animation cycles in one sprite (a walk cycle and an idle cycle), they don't want walk-cycle frames showing as ghosts when editing the idle cycle. The loop-tag option scopes the ghosting to the active tag's range.

### Our equivalent today

`ui/` has WebGL2 viewport rendering. Onion skin support is spec'd but not implemented in the viewport.

### Port plan

The C++ Render class composites to a Skia/pixman target. Our viewport composites to WebGL2. The algorithms don't port directly; the structure does.

```rust
// app/render/onionskin.rs

pub struct OnionskinOptions {
    pub frames_before: u32,
    pub frames_after: u32,
    pub opacity_base: f32,
    pub opacity_step: f32,
    pub position: OnionskinPosition,
    pub kind: OnionskinKind,
    pub loop_tag: Option<TagId>,
}

pub enum OnionskinPosition { Behind, InFront, Both }
pub enum OnionskinKind { Merge, RedBlueTint }

impl ViewportRenderer {
    fn render_frame(&self, sprite: &Sprite, active: FrameId, opts: &OnionskinOptions) {
        // Pass 1: behind ghosts
        if matches!(opts.position, OnionskinPosition::Behind | OnionskinPosition::Both) {
            for i in 1..=opts.frames_before {
                if let Some(f) = active.checked_sub(i) {
                    if self.in_loop_tag(sprite, f, opts.loop_tag) {
                        let opacity = (opts.opacity_base - (i - 1) as f32 * opts.opacity_step).max(0.0);
                        self.render_frame_at_opacity(sprite, f, opacity, opts.kind.tint_past());
                    }
                }
            }
        }
        // Pass 2: active frame, full opacity
        self.render_frame_at_opacity(sprite, active, 1.0, None);
        // Pass 3: in-front ghosts
        if matches!(opts.position, OnionskinPosition::InFront | OnionskinPosition::Both) {
            // mirror of Pass 1, scanning forward
        }
        // Pass 4-5: selection + brush preview overlays (separate concerns)
    }
}
```

In WebGL2 each pass is a quad draw with a shader that handles opacity and optional tint.

### Attribution checklist

- `app/render/onionskin.rs` → upstream `src/render/onionskin_options.h`, `onionskin_type.h`, `onionskin_position.h`, and the onion-skin sections of `src/render/render.cpp`

## 16. Tilemap and tileset — `src/doc/tileset.h`, `layer_tilemap.h`, `tile.h` (MIT)

### What it does

Supports tile-based layers where the layer stores tile indices rather than raw pixels, and a shared tileset stores the actual tile graphics. Editing a tile updates every cell that uses it. Tiles can be flipped along X, Y, or the diagonal as flags on the cell, multiplying the visual variations available without growing the tileset.

### License status

MIT. Files:

- `src/doc/tile.h` — `tile_t` packing and flag masks
- `src/doc/tileset.h`, `tileset.cpp` — Tileset class
- `src/doc/tilesets.h`, `tilesets.cpp` — registry of tilesets per sprite
- `src/doc/layer_tilemap.h`, `layer_tilemap.cpp` — the tilemap layer variant
- `src/doc/tile_primitives.h`, `tile_primitives.cpp` — tile-aware get/set helpers

### How it's decomposed

A `tile_t` is a 32-bit value with packed fields:

```
bits 0-27 — tile ID (28 bits = ~268M tiles, well beyond any practical tileset)
bit 28    — flip X
bit 29    — flip Y
bit 30    — flip diagonal (swap axes)
bit 31    — reserved
```

The bit positions are exposed through masks in `tile.h`. A `Tileset` owns:

- a list of tile images (each a small Image, conventionally 16×16)
- the grid size (tile dimensions)
- a name
- optional external-file linkage (for tilesets shared across sprites)
- a hash table mapping tile-image-hash → tile-ID, for deduplication during edits ("if I draw something that matches an existing tile, link to it rather than create a new one")

A `LayerTilemap` stores a 2D array of `tile_t` values plus a reference to the tileset it uses. Rendering a tilemap layer means looking up each tile_t's tile in the tileset, applying the flip flags, and blitting at the cell position.

### Why the decomposition pays off

**Tileset deduplication for level-design content.** A 64×64 tile-based level using 8×8 tiles has 4096 cells. If every cell stored its own 8×8 pixels, that's 256KB. With a tileset of 256 unique tiles (a generous count for one level), the storage is 4096 indices = 4KB plus 256 × 64 = 16KB of tile pixels — a 12× reduction. The reduction compounds with animation (one level shared across frames stores once).

**Flip flags as cell metadata, not tileset multiplication.** A naive tileset for an isometric ground plane might contain "grass corner top-left", "grass corner top-right", "grass corner bottom-left", "grass corner bottom-right" as four separate tiles. With flip flags, one tile plus four orientations suffices. The tileset stays one quarter the size.

**Auto-matching modified tiles.** Tileset flag 8 ("Aseprite will try to match modified tiles with their X flipped version automatically in Auto mode") means that when the artist edits a tile, if the edited result matches an existing tile via flip, the modified cells get the flag set rather than duplicating the tile. This keeps the tileset minimal under iteration.

### Our equivalent today

`core/tilemap/` is implemented with Wang autotile rules per the bedrock spec. The Aseprite-compatible flip-flag representation should be reviewed against the current model — particularly bit positions if we want to round-trip through `.aseprite` files.

### Port plan

The data model port:

```rust
// core/tilemap/tile.rs
pub struct Tile(pub u32);

impl Tile {
    const ID_MASK:    u32 = 0x0FFFFFFF;
    const FLIP_X:     u32 = 1 << 28;
    const FLIP_Y:     u32 = 1 << 29;
    const FLIP_DIAG:  u32 = 1 << 30;

    pub fn id(self) -> u32 { self.0 & Self::ID_MASK }
    pub fn flip_x(self) -> bool { self.0 & Self::FLIP_X != 0 }
    pub fn flip_y(self) -> bool { self.0 & Self::FLIP_Y != 0 }
    pub fn flip_diag(self) -> bool { self.0 & Self::FLIP_DIAG != 0 }
}

// core/tilemap/tileset.rs
pub struct Tileset {
    pub id: TilesetId,
    pub name: String,
    pub tile_size: Size,
    pub tiles: Vec<Image<Rgba>>,  // index 0 is conventionally the empty tile
    pub hash_index: HashMap<TileHash, u32>,  // for dedup
    pub external: Option<ExternalTileset>,
}

// core/project/layer.rs (variant)
pub struct LayerTilemap {
    pub id: LayerId,
    pub tileset_id: TilesetId,
    pub cells: BTreeMap<FrameId, TilemapCels>,
}

pub struct TilemapCels {
    pub grid_size: Size,         // in tiles
    pub tiles: Vec<Tile>,        // row-major, grid_size.w * grid_size.h
}
```

Round-trip with the `.aseprite` format is the main reason to honor the upstream bit layout exactly.

### Attribution checklist

- `core/tilemap/tile.rs` → upstream `src/doc/tile.h`
- `core/tilemap/tileset.rs` → upstream `src/doc/tileset.h`, `tileset.cpp`, `tilesets.h`
- `core/project/layer.rs` (LayerTilemap variant) → upstream `src/doc/layer_tilemap.h`, `layer_tilemap.cpp`
- `core/tilemap/primitives.rs` → upstream `src/doc/tile_primitives.h`, `tile_primitives.cpp`

## 17. Animation timeline data — `src/doc/tag.h`, `frame.h` (MIT)

### What it does

Stores per-frame duration, named frame ranges (tags) with playback direction and repeat count, and the bookkeeping for animated palettes and animated tilesets. The data model is small but precise — every detail (ping-pong, repeat count, tag color) is exercised by exporters and engines.

### License status

MIT. Files:

- `src/doc/frame.h` — `frame_t` typedef
- `src/doc/tag.h`, `tag.cpp` — Tag class
- `src/doc/tags.h`, `tags.cpp` — Tags collection on a Sprite
- `src/doc/anidir.h` — animation direction enum
- `src/doc/selected_frames.h` — frame selection sets

### How it's decomposed

A `Tag` carries:

- `from: frame_t`, `to: frame_t` — inclusive frame range
- `name: string`
- `aniDir: AniDir` — Forward, Reverse, PingPong, PingPongReverse
- `repeat: u16` — 0 = unspecified (default), 1 = play once, N = play N times
- `color: rgba` — for UI labeling
- `user_data: UserData` — extensible

Frame duration is stored on the **frame** itself (technically on the sprite's frame-duration table, indexed by frame_t), in milliseconds. Per-frame duration is per the file format an integer; in memory it's a `u32`.

`SelectedFrames` is a sparse set of frames represented as a sorted list of inclusive ranges. Operations on selections (move all selected, copy all selected, etc.) iterate in range order, which is more cache-friendly than iterating arbitrary frame indices.

### Why the decomposition pays off

**Repeat count enables short loops in long animations.** A character that breathes once for every 60 frames of idle animation can have a 5-frame "breath" tag with repeat=12. Without repeat, the artist either duplicates the breath frames or builds a separate animation file. With repeat, the data model expresses the intent directly.

**Ping-pong as a built-in direction.** "Frames 1, 2, 3, 4, 3, 2, 1" is awkward to express as a flat playback list. Ping-pong direction expresses it concisely and the player knows to handle the bounce.

**Tag color for UI clarity.** Animations often have many tags. Tag color in the timeline UI lets the artist scan quickly to find "the run cycle" by recognizing its color band. The color is just metadata, not playback-relevant.

### Our equivalent today

`core/project/animation.rs` has tags and frame duration. Repeat count and ping-pong variants should be verified against the current implementation; they may already match.

### Port plan

```rust
// core/project/animation.rs

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FrameId(pub u32);

pub enum AniDir { Forward, Reverse, PingPong, PingPongReverse }

pub struct Tag {
    pub from: FrameId,
    pub to: FrameId,
    pub name: String,
    pub direction: AniDir,
    pub repeat: u16,        // 0 = unspecified
    pub color: Rgba8,
    pub user_data: UserData,
}

pub struct FrameDurations(Vec<u32>);  // milliseconds, indexed by FrameId

pub struct SelectedFrames {
    ranges: Vec<RangeInclusive<FrameId>>,
}

impl SelectedFrames {
    pub fn contains(&self, frame: FrameId) -> bool { /* binary search */ }
    pub fn iter(&self) -> impl Iterator<Item = FrameId> + '_ { /* flatten ranges */ }
}
```

### Attribution checklist

- `core/project/animation.rs` → upstream `src/doc/frame.h`, `tag.h`, `tag.cpp`, `tags.h`, `tags.cpp`, `anidir.h`, `selected_frames.h`

## 18. Scripting API surface — `src/app/script/*` (EULA, inspire-only)

### What it does

Exposes the document model and the editor commands to Lua scripts. Scripts can read and mutate every documented part of a sprite (layers, cels, palettes, tags, slices), execute any registered command (`app.command()`), register custom panels and tools, and respond to editor events.

### License status

**EULA. The bindings are EULA. The API shape is observable from the documentation, but the binding code itself is upstream-EULA.** We reconstruct the API surface fresh against `mlua`.

Files (for shape reference only):

- `src/app/script/engine.h`, `engine.cpp` — Lua state lifecycle
- `src/app/script/luacpp.h` — Lua/C++ marshaling helpers
- `src/app/script/userdata.h` — generic userdata wrapping
- `src/app/script/docobj.h` — exposes document objects to Lua
- `src/app/script/registry.h` — function registration
- `src/app/script/values.h` — type conversions
- `src/app/script/canvas_widget.h` — Lua-callable custom canvas
- `src/app/script/security.h` — sandboxing

The publicly-documented Lua API surface lives at https://aseprite.org/api/ and is the authoritative spec — that's what artists code against.

### How it's decomposed (API shape)

Top-level globals exposed to Lua:

- `app` — the editor handle. Properties: `activeSprite`, `activeLayer`, `activeFrame`, `activeCel`, `activeImage`, `activeTag`, `activeTool`, `pixelColor`, `bgColor`, `fgColor`, `range`. Methods: `command()`, `transaction()`, `refresh()`, `alert()`, `useTool()`.
- `Sprite` — class with constructor `Sprite(w, h, mode)`. Methods to manipulate the document tree.
- `Layer`, `Cel`, `Image`, `Palette`, `Tileset`, `Tag`, `Slice` — classes for each document object type.
- `Color` — RGBA/HSV/HSL constructors and mutators.
- `Point`, `Size`, `Rectangle` — geometry types.
- `Brush`, `Selection`, `Tool` — editor-state types.
- Globals: `BlendMode`, `ColorMode`, `ToolID`, `MouseButton` — enum-like tables.

A typical script:

```lua
local spr = app.activeSprite
local frame = app.activeFrame
local cel = spr.layers[1]:cel(frame.frameNumber)
local img = cel.image
for x = 0, img.width - 1 do
    for y = 0, img.height - 1 do
        local color = img:getPixel(x, y)
        if app.pixelColor.rgbaA(color) > 0 then
            img:drawPixel(x, y, app.pixelColor.rgba(255, 0, 0, 255))
        end
    end
end
spr:refresh()
```

The API is direct and imperative. Document objects act as Lua userdata; field access and method calls operate on the underlying C++ object through the metatable. Garbage collection is wired such that Lua tracks references but the C++ objects own themselves — Lua values are "handles", not "values."

### Why the decomposition pays off

**Direct object exposure beats an event-bus design.** Some plugin APIs offer only "send command X with args Y" semantics, which forces every operation through a serialization layer. Aseprite exposes object handles, so scripts read field values directly with no marshaling. This dramatically improves script ergonomics for pixel manipulation, which is the most common script use case.

**`app.command()` for command-catalog automation.** Anything an artist can do from a menu, a script can do via `app.command("NewFrame")` or `app.command("ChangePixelFormat", { format = "indexed" })`. The catalog is the user-facing command list — automating a workflow doesn't require new bindings, just script-level wrapping of existing commands.

**Sandbox by capability.** Scripts default to a limited filesystem and network capability set. Trusted scripts can be granted broader access via a permissions prompt. This is the right shape for a plugin ecosystem.

### Our equivalent today

`scripting/` has `mlua` integration with a partial API surface. The full Aseprite-compatible API is on the roadmap.

### Port plan

Reconstruct the API surface as Rust-side bindings using `mlua::UserData`. Each document type wraps a stable reference (typically an ID + a borrow into the document store):

```rust
// scripting/api/sprite.rs

pub struct LuaSprite {
    pub id: SpriteId,
    pub doc: Arc<DocumentStore>,
}

impl mlua::UserData for LuaSprite {
    fn add_fields<'lua, F: mlua::UserDataFields<'lua, Self>>(fields: &mut F) {
        fields.add_field_method_get("width", |_, this| {
            Ok(this.doc.read(this.id).size.w)
        });
        fields.add_field_method_get("height", |_, this| {
            Ok(this.doc.read(this.id).size.h)
        });
        // ... activeLayer, layers, palettes, ...
    }

    fn add_methods<'lua, M: mlua::UserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_method("newLayer", |_, this, ()| {
            this.doc.mutate(this.id, |s| s.add_layer(...))?;
            Ok(...)
        });
        // ... newFrame, cel, refresh, ...
    }
}
```

Compatibility with existing Aseprite scripts is a separate decision — the API shape is large and worth committing to only if we believe artists will reuse their existing script libraries with Pixhaus. The dossier surfaces the option; the call is the user's.

### Attribution checklist

None — EULA territory, no upstream code copied. The new files carry only the Pixhaus copyright. The *API shape* (function names, parameter order, semantics) is not copyrightable — only the implementation is — so we can mirror it freely for compatibility.

## 19. Plugin and extension model — `src/app/extensions.*` (EULA, inspire-only)

### What it does

Loads plugins ("extensions") from a known directory under the user data folder. Each extension is a directory containing a `package.json` manifest, Lua scripts, themes, and optionally locale data. Extensions can register new tools, inks, panels, commands, and command-line aliases. They can ship with Aseprite or be installed by the user.

### License status

EULA. Concept only.

### How it's decomposed

The manifest declares:

- `name`, `displayName`, `version`, `author`
- `contributes` — what the extension adds:
  - `scripts` — Lua files to load on activation
  - `commands` — new editor commands with bindings
  - `tools` — new tools (each declares ink, point shape, controller, intertwine — see section 4)
  - `inks` — new ink kinds
  - `themes` — UI themes
  - `keys` — keybindings to add
  - `dithering-matrices` — new Bayer matrices for ordered dither

At load time, the extension manager scans each extension directory, validates the manifest, loads its Lua scripts in a fresh Lua state, and dispatches their `init()` exports. The exported `contributes` are registered against the global editor state — tools go into the ToolBox, commands into the command catalog, themes into the theme list.

Hot reload is supported in development by watching the extension directory for changes.

### Why the decomposition pays off

**Manifest-driven contribution surfaces.** A plugin doesn't call ad-hoc registration APIs; it declares what it contributes in `package.json`. The editor reads the manifest and wires everything up. This means plugin behavior is auditable without running the plugin — a user inspecting a plugin's manifest before installing knows what it can do.

**Per-extension Lua state.** Two plugins can't accidentally collide on global names. Each plugin sees its own clean Lua environment.

**Theme as a contribution.** Themes are XML skin files; an extension that contributes a theme is just shipping new XML. No Lua code required. This lowers the bar for non-programmer contributors.

### Our equivalent today

`plugins/` has loader scaffolding per the bedrock spec. We use `extism` for WASM-isolated plugins and `mlua` for Lua plugins. Manifest is `plugin.toml`. Hot reload is on the roadmap.

### Port plan

The Pixhaus model is more isolated than Aseprite's (WASM sandboxing via extism vs. Aseprite's per-plugin Lua state). The contract translates approximately:

```toml
# plugin.toml
[plugin]
name = "my-plugin"
display_name = "My Plugin"
version = "1.0.0"
authors = ["Artist <artist@example.com>"]

[contributes]
scripts = ["scripts/main.lua"]

[[contributes.commands]]
id = "my-plugin/cleanup"
title = "Cleanup edges"
key = "Ctrl+Shift+E"

[[contributes.tools]]
id = "my-plugin/spray-circle"
ink = "normal"
point_shape = "circle"
controller = "spray"
intertwine = "spray"

[[contributes.dithering_matrices]]
id = "my-plugin/checker"
size = "2x2"
matrix = [[0, 2], [3, 1]]

[capabilities]
filesystem = "readonly"
network = false
```

The manifest is declarative — the loader can audit it before granting capabilities. Lua and WASM scripts get sandboxed environments per their declared capabilities.

### Attribution checklist

None — EULA territory.

## 20. UI framework — `src/ui/` (MIT, but skip)

### What it does

Implements a portable widget toolkit (windows, buttons, labels, text entries, lists, sliders, scrollbars, layout grids, themes) on top of the `laf` platform layer. Aseprite uses this framework for every part of its UI — menus, dialogs, the timeline, the palette editor, the color picker.

### License status

MIT. Files in `src/ui/`. The framework is independently usable; that's why it carries its own LICENSE.txt in the directory.

### Why we're skipping it

Pixhaus runs on Tauri 2 with a Solid + WebGL2 frontend. Porting a desktop-toolkit-style widget framework to the web has limited value — we have HTML/CSS for layout, Solid for state, and WebGL2 for the canvas. The Aseprite UI framework is well-designed for its host (Skia / pixman renderer, OS event loop) and not a fit for ours.

The framework is worth reading once if you're curious about how a small, focused widget toolkit composes. Notable features include the dock manager (workspace tabs and panels), the skin-based theming (`data/skins/` XML files), and the message-loop dispatch (`src/ui/manager.h`). None of these have direct counterparts in our stack.

### Our equivalent today

`ui/` has Solid components for the document UI, with WebGL2 viewport rendering. The Aseprite framework provides zero portable value here.

### Port plan

None. Read for education; do not port.

### Attribution checklist

None.

## 21. Rendering pipeline composition — `src/render/render.h`, `render.cpp` (MIT)

### What it does

Composes the document tree (layers, cels, blend modes, opacities) into a flat pixel image suitable for display or export. Handles zoom, onion skin, selection overlay, brush preview, and the rendering of tilemap layers. Tracks dirty regions so unchanged parts of the screen don't need to be recomposed.

### License status

MIT. Files:

- `src/render/render.h`, `render.cpp` — the Render class
- `src/render/zoom.h`, `zoom.cpp` — zoom transform
- `src/render/projection.h` — world-to-screen projection
- `src/render/rasterize.h`, `rasterize.cpp` — vector-to-mask rasterization for selection overlays
- `src/render/gradient.h`, `gradient.cpp` — gradient rendering
- `src/render/bg_options.h`, `bg_type.h` — checkerboard background options

### How it's decomposed

The Render class is a procedure-rich object that knows how to composite the document at a given zoom and viewport into a destination buffer. Its inputs are:

- the Sprite (read-only)
- the active frame
- onion skin options (section 15)
- zoom and viewport rectangle
- selection mask (optional, for marching-ants overlay)
- brush preview (optional, for tool-loop preview overlays)
- background options (checkerboard color, solid color, transparent)

Its outputs are the rendered pixels in the destination Image.

Internally it walks the layer tree depth-first, computing each layer's effective opacity (group opacity × layer opacity), applying blend modes against the accumulating composite buffer. For group layers with "composite separately first" set, it allocates a temporary buffer, recursively composites the group's children into that buffer, then composites the buffer into the parent.

Dirty regions are tracked via a `Region` (an arbitrary union of axis-aligned rectangles). When a document mutation marks a sub-rectangle dirty, the next render restricts its work to that region.

### Why the decomposition pays off

**Single render object instead of per-target methods.** A naive design might split rendering across "render to screen", "render to PNG", "render to GIF frame". Aseprite's Render class is the single source of composition truth; output destinations differ only in pixel format and clipping.

**Group-flatten-first for blend-mode correctness.** Without flatten-first, blend modes that aren't associative (Hue, Color, Saturation, Luminosity) give different results based on layer order in ways artists don't intuit. Flatten-first matches Photoshop's behavior and what artists expect.

**Region-based dirty tracking.** The viewport renders ~60 frames per second. If every render recomposes the entire sprite, large sprites at high zoom stutter. Tracking the dirty region (often a small rectangle around the cursor or the active cel) reduces the per-frame work proportionally.

### Our equivalent today

`ui/` viewport handles its own composition in WebGL2. The Aseprite Render class targets Skia/pixman, so the algorithms don't port one-to-one. The structure (frame composition with onion skin + overlay passes, region-based dirty tracking) does port.

### Port plan

Adopt the structure, implement against WebGL2:

```rust
// app/render/composite.rs

pub struct CompositePlan {
    pub sprite: SpriteId,
    pub frame: FrameId,
    pub onionskin: Option<OnionskinOptions>,
    pub selection: Option<MaskRef>,
    pub brush_preview: Option<BrushPreviewRef>,
    pub background: BackgroundOptions,
    pub zoom: f32,
    pub viewport: Rect,
    pub dirty: Region,
}

pub trait Renderer {
    fn render(&mut self, plan: &CompositePlan);
}
```

The WebGL2 implementation walks the layer tree, issues draw calls per layer (with appropriate blend-mode shaders), and clips to the dirty region. Group-flatten-first is implemented by rendering the group to a framebuffer texture and then sampling that texture into the parent.

### Attribution checklist

- `app/render/composite.rs` → upstream `src/render/render.h`, `render.cpp`
- `app/render/zoom.rs` → upstream `src/render/zoom.h`, `zoom.cpp`, `projection.h`
- `app/render/background.rs` → upstream `src/render/bg_options.h`, `bg_type.h`
- `app/render/gradient.rs` → upstream `src/render/gradient.h`, `gradient.cpp`

## 22. Stroke commit model — `src/app/tools/tool_loop_manager.cpp` (EULA, inspire-only)

### What it does

Coordinates per-stroke state: tracks input events, renders preview overlays, builds a single Cmd for undo, and commits or cancels on stroke end. This is the glue that makes "one stroke = one undo step" work.

### License status

EULA. Concept only.

### How it's decomposed

The ToolLoopManager has a small state machine:

1. **Idle** — no stroke in progress.
2. **Started** — first input event received; created a preview buffer, started the Cmd's pre-image capture.
3. **Active** — receiving input events, updating the preview, accumulating dab coordinates.
4. **Committing** — flushed dabs into the Cmd, applied the Cmd to the document, pushed it onto the undo history.
5. **Cancelled** — discarded the preview, did not push to undo.

Preview rendering happens in pass-4 of the Render pipeline (section 21). The preview buffer is the same size as the active cel and stores the dabs that would be applied on commit. The renderer composites it over the document for display only.

On commit, the preview is converted into a Cmd subclass (typically `ReplaceCelImage` or `CmdSequence` of multiple such commands for multi-layer strokes), and the Cmd is pushed onto the undo history.

### Why the decomposition pays off

**Preview as a separate buffer prevents read-modify-write contention.** The renderer and the tool loop both touch pixel data — but the tool loop writes to the preview buffer, never the document. The renderer reads both. This serializes naturally without locks.

**One Cmd per stroke means undo is artist-meaningful.** "I drew a curve, undo." not "I drew the 47th pixel of a curve, undo 47 times."

**Cancellable strokes for keyboard escape.** The artist hits Escape mid-stroke; the preview is discarded, the document is unchanged, no undo entry is created. This is impossible if strokes are committed pixel-by-pixel.

### Our equivalent today

Partial. The current tool layer has stroke handling but the preview-vs-commit separation is uneven.

### Port plan

Reconstruct fresh in Rust per section 4's ToolLoop sketch. The state machine is small enough that explicit `enum ToolLoopState { Idle, Started, Active, Committing, Cancelled }` works without a state-machine library.

### Attribution checklist

None — EULA.

## 23. Memory ownership philosophy

### What it does (and why it matters)

This isn't a subsystem — it's a translation principle. C++ codebases written in 2001-2018 lean heavily on heap-of-pointers ownership: `Sprite*` owns `Layer*`s, which own `Cel*`s, which own `Image*`s, all via raw pointers with discipline. Rust ports of such code must convert pointer graphs into ownership trees, which sometimes requires re-shaping the data model.

### License status

N/A — discussion of porting practice, not a code section.

### How Aseprite's ownership looks

The upstream tree has:

- `Sprite` owns a `LayerGroup` (root) by value-in-pointer.
- `LayerGroup` owns its `Layer*` children in a `std::vector<Layer*>` and deletes them in its destructor.
- `Layer` owns its `Cel*`s in a `CelList`.
- `Cel` owns a `CelData*` via reference count (`std::shared_ptr<CelData>` analog implemented manually).
- `CelData` owns its `Image*` by value-in-pointer.
- `Image*` references the underlying pixel buffer via `ImageBuffer*`, which is sometimes pooled.

Concurrency posture: single-threaded except for the renderer's read pass. Locks (the Read/Write/ReadWrite locks on Image) exist to catch torn writes if the rendering thread overlaps with the tool loop's commit; they are not used to enable parallelism.

### Translation principles for Rust

**Single owner per object.** A Sprite owns a Layer tree directly. A LayerGroup owns its children as `Vec<Layer>`, not `Vec<Box<Layer>>` (unless `Layer` is a recursive enum, in which case the boxing is for size, not for ownership). A Cel owns its position, opacity, and a shared CelData. The shared CelData is the only `Arc<>` in the picture — and it's an `Arc<CelData>`, not `Arc<Mutex<CelData>>`. Cel mutation uses `Arc::make_mut` for copy-on-write.

**`&` and `&mut` instead of locks.** The Rust borrow checker provides at compile time what Aseprite enforces at runtime via the lock semantics. A `&Image<P>` is a read lock; a `&mut Image<P>` is a write lock. No runtime cost, no runtime risk.

**Avoid `Arc<Mutex<>>` in the hot path.** Pixel write loops should be working against `&mut Image<P>` directly. Reach for `Arc<Mutex<>>` only at the app boundary where the Tauri command handler hands the editor state to a worker — and even there, prefer message passing over shared mutable state.

**Pixel buffers as `Vec<u8>` with stride.** Not `Vec<Vec<u8>>` (cache-unfriendly), not `[[u8; W]; H]` (incompatible with runtime sizes), not `nalgebra` matrix (overkill). A flat `Vec<u8>` with width / height / stride explicitly tracked is the right primitive.

**IDs and indirection where the C++ uses pointers.** A `LayerId` is a `u64` (or a UUID, for `.aseprite` compatibility). Operations that need a layer take `&Sprite` and `LayerId`, look up the layer via the sprite. This breaks pointer dependencies that would otherwise require self-referential data structures.

### Our equivalent today

The Pixhaus data model already follows these principles per `CLAUDE.md`'s memory section. The dossier reinforces them as the translation idiom from C++.

### Attribution checklist

N/A.

## 24. Embedded MIT submodules — `laf`, `clip`

### What they do

`laf` (Library for Aseprite Framework) is the platform abstraction layer: file system, threading, UUID generation, SHA-1, UTF-8 handling, geometry primitives (Point, Size, Rect, Region, Color), OS window and input event abstractions, Wacom tablet support. `clip` is the cross-platform clipboard I/O library — copy/paste text and images on Windows, macOS, Linux.

### License status

Both MIT. Hosted at https://github.com/aseprite/laf and https://github.com/aseprite/clip as separate repositories.

### Why we're skipping them

The Rust ecosystem already has high-quality, well-maintained equivalents for every component of `laf`:

| `laf` module | Rust equivalent |
|--------------|-----------------|
| `base::UUID` | `uuid` crate |
| `base::SHA-1` | `sha1` crate (or `sha2` for SHA-256) |
| `base::UTF-8` | std library |
| `base::file_system` | `std::fs`, `walkdir`, `notify` |
| `base::threading` | `std::thread`, `tokio`, `rayon` |
| `gfx::Point/Size/Rect/Region` | `euclid` or hand-rolled (we have these in `core/`) |
| `gfx::Color` | `palette` or hand-rolled |
| `os::window/input` | Tauri's window plugin |
| `os::wacom` | Browser PointerEvent API via Tauri |

For `clip`, Tauri ships a first-class clipboard plugin that handles all three desktop platforms with a consistent API. No port needed.

### Our equivalent today

All `laf` functionality is already covered by Rust crates we use. `clip` is covered by Tauri.

### Port plan

None. If we ever find a `laf` utility that has no Rust equivalent (unlikely), port that one utility and credit the upstream — but the survey suggests every relevant piece is already available.

### Attribution checklist

N/A unless a specific utility is ported in the future.

## 25. What we'd intentionally not adopt

A few upstream choices are worth flagging as explicit non-goals for Pixhaus:

**The custom UI framework.** Already covered in section 20. Pixhaus is web-stack-on-Tauri; the desktop widget toolkit doesn't fit.

**Skia / pixman renderer.** The upstream Render class composites onto a Skia or pixman backbuffer depending on platform. We use WebGL2. The composition *structure* ports (section 21); the renderer backend does not.

**Lua 5.1-specific semantics.** Upstream targets Lua 5.1 via the bundled Lua interpreter. We use `mlua`, which can target 5.1 through 5.4 and Luau. If we want script compatibility with existing Aseprite scripts, we configure `mlua` for 5.1-compat; otherwise we pick 5.4 or Luau for new scripts. The version is a configuration choice, not a port decision.

**EULA-licensed installer flow.** Aseprite ships its own auto-updater (`src/updater/`), crash reporter, Wacom installer, Steam integration. Pixhaus uses Tauri's updater plugin and Sentry for crash reporting. Steam isn't a target.

**The XML-skin theme system.** Aseprite themes are XML files in `data/skins/` describing color and font for each widget. We use CSS in the Solid frontend. No port.

**FLI / FLC support.** Historical Autodesk Animator format. Aseprite supports it for legacy compatibility; Pixhaus has no obligation to.

**Wacom-API-specific tablet handling.** Upstream uses the Wintab API on Windows for pen pressure. Tauri exposes pressure via the browser PointerEvent, which is sufficient.

**The "data recovery" and "anticrash" features.** Aseprite has an on-disk autosave with a recovery prompt on launch after a crash. This is worth re-implementing in Pixhaus (per the project's launch checklist), but it's a feature to design fresh, not a port — the upstream implementation is heavily tied to its own threading and file-system layer.

## 26. Notable smaller pieces worth mentioning

A few subsystems are too small to warrant their own section but are worth knowing about.

**Image flipping — `src/doc/algorithm/flip_image.{h,cpp}` (MIT, ~225 lines).** Horizontal and vertical flips. Bit-exact reversibility matters here: a flip-then-flip must produce the original, no rounding allowed.

**Image shifting — `src/doc/algorithm/shift_image.{h,cpp}` (MIT).** Tiling/wraparound shift used for the seamless-pattern preview ("tiled mode").

**Image resizing — `src/doc/algorithm/resize_image.{h,cpp}` (MIT).** Nearest-neighbor (the right default for pixel art) and bilinear (for smooth preview only). Aseprite resists offering bicubic because it's almost always wrong for pixel art.

**Shrink bounds — `src/doc/algorithm/shrink_bounds.{h,cpp}` (MIT).** Computes the tightest bounding rectangle of non-transparent pixels in an image. Used to optimize cel storage (a cel with a 64×64 image bounded to a 5×7 region of opaque pixels can store the 5×7 sub-image and offset, saving 70× memory).

**Polygon rasterization — `src/doc/algorithm/polygon.{h,cpp}` (MIT).** Scan-converts a closed polygon into a mask. Used for lasso selection.

**Gradient rendering — `src/render/gradient.{h,cpp}` (MIT).** Linear and radial gradients between two colors with optional dithering. The dither pass uses the same ordered-dither machinery from section 13.

**RotSprite's siblings in `src/doc/algorithm/rotate.{h,cpp}` (MIT).** General rotation primitives including affine matrices and the EPX/Scale2x upscaler that RotSprite uses internally. The `rotate.cpp` file is 1033 lines and worth its own port pass alongside RotSprite.

Each of these is a candidate for direct porting if and when a feature lands that needs it. They are MIT-licensed and largely self-contained.

## 27. Gap list and proposed queue tasks

This section converts the dossier into actionable work. Tasks are grouped by bucket; each carries the upstream license, a one-line rationale, and a proposed entry for `work/queue.md`.

### Port tasks (MIT — direct translation)

**P1. `.aseprite` decoder/encoder port.**
- License: MIT.
- Rationale: full read/write compatibility with the canonical pixel-art file format is the single highest-value port in this dossier; every existing pixel-art asset on every artist's drive lands cleanly into Pixhaus.
- Upstream: `src/dio/aseprite_decoder.{h,cpp}`, `aseprite_encoder.{h,cpp}`, `aseprite_common.h`, `docs/ase-file-specs.md`.
- Pixhaus target: `io/aseprite/`.
- Proposed queue entry: `S53 — port .aseprite full read/write from upstream MIT source, with attribution; round-trip fixture tests; align with B3 file-format spec.`

**P2. Quantization + dithering port.**
- License: MIT.
- Rationale: the indexed-mode pipeline needs median-cut + octree quantization and ordered + Floyd-Steinberg + JJN dithering for credible palette workflows. Upstream's implementations are well-tested.
- Upstream: `src/render/quantization.{h,cpp}`, `median_cut.h`, `ordered_dither.{h,cpp}`, `error_diffusion.{h,cpp}`, `dithering_matrix.h`, `color_histogram.h`.
- Pixhaus target: `core/color/quantize.rs`, `core/color/dither.rs`, `core/color/histogram.rs`.
- Proposed queue entry: `S54 — port quantization (median-cut + octree) and dithering (ordered + FS + JJN) from upstream MIT source; benchmark against existing placeholders; expose via verb runtime.`

**P3. RotSprite + EPX upscaler port.**
- License: MIT.
- Rationale: pixel-art-aware rotation is a defining feature of pixel-art editors. Without RotSprite, rotation in Pixhaus is unusable on pixel art.
- Upstream: `src/doc/algorithm/rotsprite.{h,cpp}`, `rotate.{h,cpp}`.
- Pixhaus target: `core/transforms/rotsprite.rs`, `core/transforms/epx.rs`.
- Proposed queue entry: `S55 — port RotSprite + EPX upscaler from upstream MIT source; cover by snapshot tests at 15°, 30°, 45° intervals.`

**P4. RgbMapRGB5A3 lookup table port.**
- License: MIT.
- Rationale: indexed-mode painting and RGB→indexed conversion are unacceptably slow without a precomputed lookup; the upstream 5/5/5/3 table is the standard solution.
- Upstream: `src/doc/rgbmap.h`, `rgbmap_rgb5a3.{h,cpp}`, `rgbmap_algorithm.h`.
- Pixhaus target: `core/color/rgbmap.rs`.
- Proposed queue entry: `S56 — port RgbMap RGB5A3 lookup from upstream MIT source; build on palette change; bench against current linear scan.`

**P5. Bresenham line + Zingl ellipse port.**
- License: MIT (with Zingl MIT attribution alongside Aseprite MIT).
- Rationale: pixel-perfect freehand needs the Bresenham primitive and the corner-cleanup postprocess. The Zingl implementations are widely-used reference code.
- Upstream: `src/doc/primitives.{h,cpp}` line/ellipse routines, with the Zingl 2012-2016 attribution preserved.
- Pixhaus target: `core/canvas/raster.rs`.
- Proposed queue entry: `S57 — port Bresenham line + Zingl ellipse primitives with corner-cleanup postprocess; wire to pixel-perfect freehand mode.`

**P6. Selection algorithms parity.**
- License: MIT.
- Rationale: the modify-selection toolkit (expand, contract, border, feather) plus magic-wand floodfill seeding bring our selection model to feature parity with the upstream baseline.
- Upstream: `src/doc/algorithm/modify_selection.{h,cpp}`, `floodfill.{h,cpp}`, `polygon.{h,cpp}`, `stroke_selection.{h,cpp}`, `fill_selection.{h,cpp}`, `shrink_bounds.{h,cpp}`.
- Pixhaus target: `core/selection/modify.rs`, `core/canvas/floodfill.rs`, `core/canvas/polygon.rs`.
- Proposed queue entry: `S58 — port modify-selection + floodfill + polygon + shrink-bounds from upstream MIT source; align with existing core/selection types.`

**P7. Generic undo library port.**
- License: MIT.
- Rationale: branching command-tree history is the right shape and upstream's implementation has been refined under real artist use.
- Upstream: `src/undo/undo_history.{h,cpp}`, `undo_command.h`, `undo_state.h`.
- Pixhaus target: `core/undo/history.rs`, `core/undo/state.rs`.
- Proposed queue entry: `S59 — port generic undo library (branching command tree, leaf eviction, observer signals) from upstream MIT source; replace current placeholder.`

**P8. Image flip, shift, shrink-bounds, gradient port.**
- License: MIT.
- Rationale: small, self-contained primitives that any pixel-art editor needs. Cheap to port, useful immediately.
- Upstream: `src/doc/algorithm/flip_image.{h,cpp}`, `shift_image.{h,cpp}`, `shrink_bounds.{h,cpp}`, `src/render/gradient.{h,cpp}`.
- Pixhaus target: `core/transforms/`, `core/canvas/`.
- Proposed queue entry: `S60 — port image-primitive bundle (flip, shift, shrink-bounds, gradient) from upstream MIT source.`

### Inspiration tasks (EULA — rebuild fresh)

**I1. Four-axis tool decomposition.**
- License: EULA (no copying).
- Rationale: the Ink × PointShape × Controller × Intertwine factoring is the right architecture for a serious pixel-art tool layer; the matrix of combinations gets us hundreds of tools without writing them all out.
- Pixhaus target: `app/tools/` redesign.
- Proposed queue entry: `S61 — refactor app/tools/ to the four-axis decomposition (Ink + PointShape + Controller + Intertwine); migrate Pencil, Eraser, Line, Rectangle, Ellipse, Fill, Pick.`

**I2. Stroke commit + preview model.**
- License: EULA.
- Rationale: "one stroke = one undo step" with a cancellable preview is a non-negotiable artist UX; rebuild against our Cmd taxonomy.
- Pixhaus target: `app/tools/tool_loop.rs`.
- Proposed queue entry: `S62 — implement ToolLoopManager with preview buffer, single-Cmd commit, escape-to-cancel.`

**I3. Cmd taxonomy with WithSprite/WithCel/WithImage mixin traits.**
- License: EULA (concept only).
- Rationale: trait-based mixins reduce boilerplate across the dozens of Cmd subclasses we'll write as features land.
- Pixhaus target: `core/undo/cmd.rs`.
- Proposed queue entry: `S63 — define WithSprite/WithCel/WithImage mixin traits; port representative commands (ReplaceCelImage, AddLayer, RemoveLayer, AddFrame, RemoveFrame, ReplacePalette, AddCel, RemoveCel, SetUserDataProperty, CmdSequence).`

**I4. Symmetry as stroke transform.**
- License: EULA.
- Rationale: brush-agnostic symmetry composing through the tool loop is the right shape.
- Pixhaus target: `app/tools/symmetry.rs`.
- Proposed queue entry: `S64 — add symmetry axis to ToolLoop with horizontal / vertical / both expansion; verify single-Cmd commit.`

**I5. Brush dynamics matrix.**
- License: EULA.
- Rationale: pressure / tilt / velocity modulation is artist-table-stakes; the matrix-of-curves shape keeps the UI flat.
- Pixhaus target: `app/tools/dynamics.rs`.
- Proposed queue entry: `S65 — derive velocity in FreehandController, add Dynamics struct (3 inputs × 3 outputs of curves), apply pre-PointShape.`

**I6. Aseprite-script API surface (decision required).**
- License: EULA (concept; API shape itself is not copyrightable).
- Rationale: existing artist scripts could run on Pixhaus if we ship a compatible Lua API. Large surface; commit only if compatibility is a goal.
- Pixhaus target: `scripting/api/`.
- Proposed queue entry: `S66 — DECIDE: scope Aseprite-script API compatibility (full / partial / none). If full or partial, expand scripting/api/ to match.`

**I7. Plugin manifest with capability declarations.**
- License: EULA (concept).
- Rationale: declarative manifest with capability gating is right for an audit-before-install plugin ecosystem.
- Pixhaus target: `plugins/loader.rs`, `plugin.toml` spec.
- Proposed queue entry: `S67 — finalize plugin.toml manifest schema with declarative contribution points (commands, tools, dithering matrices, themes, keys) and capability gating (filesystem, network, document mutation).`

### Documentation tasks

**D1. Update `docs/planning/work/bedrock.md`.**
- Rationale: B3 (file format), B7 (Aseprite compat) should reference this dossier and the port tasks above.
- Proposed queue entry: `S68 — update bedrock.md B3 and B7 to reference docs/planning/research/aseprite-prior-art.md and cite the proposed S53 / S57 / S66 work.`

**D2. Update `docs/planning/work/streams.md`.**
- Rationale: insert S53-S67 entries with proper dependency annotations.
- Proposed queue entry: `S69 — add S53-S67 to streams.md with dependency graph; mark S53 (file format port) as critical-path for Aseprite-asset interop.`

**D3. Verify B10 reference-sheet system survives port plan.**
- Rationale: B10 introduced anchor metadata that the document model needs to carry across `.aseprite` round-trip. Confirm the chunk strategy.
- Proposed queue entry: `S70 — verify B10 reference-sheet anchors round-trip through .aseprite User Data chunks (0x2020) with extension Entry ID; document the extension manifest entry needed.`

### Bucket totals

- **Port tasks:** 8 (P1-P8). All MIT, with attribution. P1 (file format) and P2 (quantization + dither) are highest leverage.
- **Inspiration tasks:** 7 (I1-I7). All EULA-conceptual. I1 (four-axis tools) is the biggest refactor.
- **Documentation tasks:** 3 (D1-D3). All cheap; do alongside the first port that lands.

The ordering for execution: D1+D2 first (so the queue reflects the plan), then P7 (undo library, foundational), then P1 (file format, highest leverage), then I1 (tool refactor, unblocks several features), then everything else in opportunistic order.

## 28. Verification of the dossier

For anyone reviewing this dossier before greenlighting ports:

- **License audit reproducibility.** Every section pairs its license claim with a specific upstream file path. Confirm by reading the file header in the upstream tree.
- **Algorithm walkthroughs.** The Bresenham, RotSprite/EPX, Floyd-Steinberg, and median-cut walkthroughs match the published algorithms; the upstream code is the same algorithm. Verify by sampling.
- **No EULA code is quoted.** The dossier contains pseudocode (typed in Rust by us) for algorithms whose ideas come from upstream EULA sections. Pseudocode is not a copyright violation; we wrote it. No verbatim copy of upstream source appears in this document.
- **Attribution targets are explicit.** Every port-tagged section names the upstream files and the proposed Pixhaus file paths. The attribution checklist makes the copyright lines easy to copy into headers.
- **Gap list maps to queue.** Section 27's S53-S70 proposals match the existing `work/queue.md` numbering convention. Confirm against the live queue before committing tasks.

## 29. Closing

The Aseprite source tree is two decades of refinement applied to the exact problem Pixhaus is solving. Reading it well saves us years. The MIT licensing on the document, render, and I/O libraries makes it possible to do more than read — we can lift the algorithmic heart of the editor into our tree with attribution, and we should, because reinventing median-cut quantization or Floyd-Steinberg dithering or the `.aseprite` chunk reader from scratch is wasted effort.

The cut between Levels 0-3 (MIT, portable) and Levels 4-5 (EULA, inspiration-only) is unusually favorable for our purposes. The upstream community knew that downstream consumers would want to read `.aseprite` files in their own engines and exporters; they kept the data and I/O layers permissive so that ecosystem could exist. Pixhaus benefits directly from that decision.

The work that's left after the ports is the work that always was: a tool system that fits our stack, a UI that's web-native, an AI verb runtime that has no upstream analog. Those parts we build fresh. The pixel-pushing primitives we port, with attribution, and we ship the better editor sooner.







