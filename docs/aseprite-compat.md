# Aseprite format compatibility

This document defines exactly which Aseprite file format features Pixhaus reads and
writes. It is the contract that stream S08 (`.aseprite` read/write) implements against.

The Aseprite binary format specification lives at
[aseprite/aseprite docs/ase-file-specs.md](https://github.com/aseprite/aseprite/blob/main/docs/ase-file-specs.md).
All section and field names in this document match that spec.

## Goal

90% of indie pixel artists' `.aseprite` files open in Pixhaus without warnings. The
minimum bar for that is: raster layers, group layers, frame tags, palettes, slices,
blend modes, layer opacity, and tileset chunks. Linked cels, opacity per cel, and the
four standard loop directions must also work — they appear in the majority of files
from Hollow Knight-era workflows onward.

## Support levels

| Level | Meaning |
|---|---|
| **read+write** | Pixhaus reads and writes this feature with full fidelity |
| **read-only** | Pixhaus reads and preserves data in memory; does not write it back |
| **ignored** | Pixhaus parses enough to advance the file cursor, discards the data |
| **unsupported** | Pixhaus does not parse this; may emit a warning |

"Read-only" and "ignored" both survive a round-trip open — the file opens without
error. "Read-only" means the data surfaces somewhere in the UI or model (e.g., an
external tileset reference becomes an inline tileset). "Ignored" means the data is
silently dropped with no UI consequence.

## File header

Pixhaus reads and validates all header fields. The table below documents disposition
per field.

| Field | Disposition | Notes |
|---|---|---|
| Magic number (0xA5E0) | Required | Reject file if wrong |
| Frames | read+write | Frame count |
| Width / Height | read+write | Canvas size → `Sprite.canvas_size` |
| Color depth | read+write | 32=RGBA, 16=Grayscale, 8=Indexed → `Sprite.color_mode` |
| Flags bit 1 (layer opacity valid) | read | Enables per-layer opacity parsing |
| Flags bit 2 (group blend modes valid) | read | Enables blend mode on group layers |
| Flags bit 4 (layers have UUID) | ignored | UUIDs not stored in Pixhaus data model |
| Speed (deprecated) | ignored | Frame duration comes from per-frame Duration field |
| Transparent color index | read+write | Indexed mode: maps to `Sprite.transparent_color_index` |
| Number of colors | read | Palette size hint |
| Pixel width / height (ratio) | ignored | Pixhaus assumes 1:1 square pixels; warn if ratio ≠ 1:1 |
| Grid X / Y / Width / Height | ignored | No grid concept in Pixhaus data model |

## Frame header

| Field | Disposition | Notes |
|---|---|---|
| Magic number (0xF1FA) | Required | Reject frame if wrong |
| Duration (ms) | read+write | → `Frame.duration_ms` |
| Chunk count | read | Use old count unless it is 0xFFFF; then use new count field |

## Chunk type summary

| Hex | Name | Support | Notes |
|---|---|---|---|
| 0x0004 | Old Palette | read-only | Legacy format; read for compat, never written |
| 0x0011 | Old Palette (0–63 range) | read-only | Same as above |
| 0x2004 | Layer | read+write | All types and flags; see detail below |
| 0x2005 | Cel | read+write | All cel types; z-index ignored |
| 0x2006 | Cel Extra | ignored | Float transform bounds not stored |
| 0x2007 | Color Profile | ignored | sRGB assumed; ICC data discarded with warning |
| 0x2008 | External Files | read-only | Used to resolve external tileset references only |
| 0x2016 | Mask (deprecated) | ignored | Deprecated selection format; Aseprite no longer writes it |
| 0x2017 | Path (never used) | ignored | Aseprite has never written this chunk |
| 0x2018 | Tags | read+write | All four loop directions, repeat count |
| 0x2019 | Palette | read+write | RGBA + per-entry names |
| 0x2020 | User Data | partial | Text and color only; properties map ignored |
| 0x2022 | Slice | read+write | Bounds, nine-slice, pivot |
| 0x2023 | Tileset | read+write (inline) | External link read-only in v1 |

## Per-chunk detail

### 0x2004 — Layer

**Support: read+write**

| Field | Disposition | Mapping |
|---|---|---|
| Visible flag (bit 0) | read+write | `Layer.visible` |
| Editable flag (bit 1) | read+write | `Layer.editable` |
| Lock movement flag (bit 2) | ignored | No movement-lock concept in Pixhaus |
| Background flag (bit 3) | read+write | `Layer.is_background` |
| Prefer linked cels flag (bit 4) | ignored | Pixhaus handles linked cels explicitly |
| Group collapsed flag (bit 5) | read+write | `LayerKind::Group { collapsed }` |
| Reference layer flag (bit 6) | read+write | `LayerKind::Reference` |
| Layer type 0 (normal/raster) | read+write | `LayerKind::Raster` |
| Layer type 1 (group) | read+write | `LayerKind::Group` |
| Layer type 2 (tilemap) | read+write | `LayerKind::Tilemap` |
| Child level | read+write | Used to reconstruct parent–child hierarchy |
| Blend mode (0–18) | read+write | See blend mode table |
| Opacity | read+write | `Layer.opacity` |
| Name | read+write | `Layer.name` |
| Tileset index (tilemap layers) | read+write | → `LayerKind::Tilemap { tileset }` |
| UUID (if flag bit 4 set) | ignored | Not stored |

Pixhaus writes layer flags bit 0 (visible), bit 1 (editable), bit 3 (background), bit
5 (collapsed), bit 6 (reference). It does not set bit 4 (prefer linked cels) or the
UUID flag.

### 0x2005 — Cel

**Support: read+write**

| Field | Disposition | Mapping |
|---|---|---|
| Layer index | read+write | Identifies owning `Layer` |
| X / Y position | read+write | `Cel.position` |
| Opacity | read+write | `Cel.opacity` |
| Type 0 (raw image) | read-only | Treated identically to type 2 on load; not written |
| Type 1 (linked cel) | read+write | `CelData::Linked { source_frame }` |
| Type 2 (compressed image) | read+write | `CelData::Raster`; ZLIB-decompressed on load |
| Type 3 (compressed tilemap) | read+write | `CelData::Tilemap`; see tilemap detail below |
| Z-index | ignored | Pixhaus uses layer ordering; cel-level z-index not stored |

**Tilemap cel (type 3) detail:**

Aseprite stores each tile as a 32-bit value with bitmasks for tile ID, X-flip, Y-flip,
and diagonal-flip. Default masks are 0x1FFFFFFF (tile ID), 0x20000000 (X flip),
0x40000000 (Y flip), and 0x80000000 (diagonal flip). Pixhaus reads the bitmasks from
the chunk header rather than hardcoding them.

Mapping to `TilemapData`:
- Tile ID bits → `TileCell.index`
- X flip bit → `TileFlags::FLIP_X`
- Y flip bit → `TileFlags::FLIP_Y`
- Diagonal flip bit → `TileFlags::FLIP_DIAGONAL`

The base index from the owning Tileset chunk (see 0x2023) is subtracted when reading
so that Pixhaus tile indices are 0-based. It is added back when writing.

### 0x2006 — Cel Extra

**Support: ignored**

Contains floating-point precise bounds for transformed cels. Pixhaus stores integer
positions; the float bounds are dropped without warning.

### 0x2007 — Color Profile

**Support: ignored**

Pixhaus operates display-referred in sRGB. On read:
- Type 0 (no profile): no action
- Type 1 (sRGB): no action, no warning
- Type 2 (ICC profile): discard with `[warn] ICC color profile discarded; Pixhaus
  operates display-referred`

Pixhaus does not write a Color Profile chunk. Aseprite will treat the file as having
no color profile, which is equivalent to unmanaged sRGB for nearly all workflows.

### 0x2008 — External Files

**Support: read-only**

Pixhaus reads this chunk only to resolve external tileset references (Tileset flag 1).
The external file path is resolved relative to the `.aseprite` file on disk. If the
external file is not found, Pixhaus emits a warning and creates an empty inline
tileset as a placeholder.

Pixhaus does not write this chunk in v1. External tilesets are inlined on first save;
the user sees: `[warn] External tileset "<name>" has been inlined; original external
file link is lost`.

### 0x2018 — Tags

**Support: read+write**

| Field | Disposition | Mapping |
|---|---|---|
| From / To frame | read+write | `FrameTag.range` |
| Loop direction 0 (Forward) | read+write | `LoopDirection::Forward` |
| Loop direction 1 (Reverse) | read+write | `LoopDirection::Reverse` |
| Loop direction 2 (Ping-pong) | read+write | `LoopDirection::PingPong` |
| Loop direction 3 (Ping-pong Reverse) | read+write | `LoopDirection::PingPongReverse` |
| Repeat count | read+write | `FrameTag.repeat` (0 = unspecified / infinite) |
| Deprecated RGB color | ignored | Not stored; written as `[0, 0, 0]` |
| Tag name | read+write | `FrameTag.name` |

Per-tag User Data chunks (immediately following each tag in 1.3+ files) are parsed as
User Data (see 0x2020).

### 0x2019 — Palette

**Support: read+write**

| Field | Disposition | Mapping |
|---|---|---|
| First / Last color index | read+write | Range of entries being defined |
| R / G / B / A per entry | read+write | `PaletteEntry.color` |
| Color name per entry | read+write | `PaletteEntry.name` |

Per-palette-entry User Data is ignored.

A single sprite may have multiple Palette chunks across frames (Aseprite writes one
per frame if colors change). Pixhaus reads all of them and resolves to the palette
state at each frame boundary. On write, Pixhaus emits one Palette chunk in the first
frame only, since all palette changes are stored in the Pixhaus project model as
discrete events.

### 0x2020 — User Data

**Support: partial**

| Field | Disposition | Mapping |
|---|---|---|
| Text (flag bit 0) | read+write | `UserData.text` |
| Color RGBA (flag bit 1) | read+write | `UserData.color` |
| Properties map (flag bit 2) | ignored | Extension data; not stored |

Pixhaus writes User Data for layers, cels, tags, slices, and tilesets where the
entity has a non-empty `UserData`. It does not write a sprite-level User Data chunk
in v1.

**Association rules.** User Data chunks do not self-identify their target entity.
Association is positional within each frame's chunk sequence:

1. The first chunk in the first frame is user data for the sprite itself.
2. A User Data chunk immediately following a Layer chunk annotates that layer.
3. A User Data chunk immediately following a Cel chunk annotates that cel.
4. After the Tags chunk, one User Data chunk per tag follows in order.
5. After the Palette chunk, one optional User Data per palette entry follows in order.
6. A User Data chunk immediately following a Slice chunk annotates that slice.
7. A User Data chunk immediately following a Tileset chunk annotates that tileset.

### 0x2022 — Slice

**Support: read+write**

| Field | Disposition | Mapping |
|---|---|---|
| Slice name | read+write | `Slice.name` |
| Per-key frame index | read+write | `SliceKey.frame` |
| Per-key slice bounds | read+write | `SliceKey.bounds` |
| Nine-slice center rect | read+write | `SliceKey.nine_slice` |
| Pivot point | read+write | `SliceKey.pivot` |

Nine-slice and pivot are optional in both Aseprite and Pixhaus. A slice with neither
is written with the nine-slice flag cleared.

### 0x2023 — Tileset

**Support: read+write (inline), read-only (external link)**

| Field | Disposition | Mapping |
|---|---|---|
| Tileset ID | read+write | `Tileset.id` |
| Tile count | read+write | `Tileset.tile_count` |
| Tile width / height | read+write | `Tileset.tile_size` |
| Base index | read+write | Stored; used to offset tile indices on read/write |
| Tileset name | read+write | `Tileset.name` |
| Flag 1 (external file link) | read-only | Resolved via 0x2008; inlined on save (see 0x2008 notes) |
| Flag 2 (inline tiles) | read+write | `TilesetSource::Inline` |
| Flag 4 (tile ID 0 = empty) | read+write | Preserved; empty tile convention |
| Flags 8 / 16 / 32 (auto-flip) | ignored | Auto-flip matching not implemented in v1 |

The embedded tile image (when flag 2 is set) is ZLIB-compressed, with tiles packed
vertically: one column of `tile_width × tile_height × tile_count` pixels in the
sprite's color mode.

## Blend mode table

All 19 Aseprite blend modes map to Pixhaus `BlendMode` variants.

| Aseprite value | Aseprite name | Pixhaus variant |
|---|---|---|
| 0 | Normal | `Normal` |
| 1 | Multiply | `Multiply` |
| 2 | Screen | `Screen` |
| 3 | Overlay | `Overlay` |
| 4 | Darken | `Darken` |
| 5 | Lighten | `Lighten` |
| 6 | Color Dodge | `ColorDodge` |
| 7 | Color Burn | `ColorBurn` |
| 8 | Hard Light | `HardLight` |
| 9 | Soft Light | `SoftLight` |
| 10 | Difference | `Difference` |
| 11 | Exclusion | `Exclusion` |
| 12 | Hue | `Hue` |
| 13 | Saturation | `Saturation` |
| 14 | Color | `Color` |
| 15 | Luminosity | `Luminosity` |
| 16 | Addition | `Addition` |
| 17 | Subtract | `Subtract` |
| 18 | Divide | `Divide` |

## Write-side output format

When Pixhaus saves a file as `.aseprite`, it emits the following chunks. Aseprite
must open the result without errors.

**First frame only:**
- 0x2020 User Data (sprite-level, if populated)
- 0x2004 Layer chunks for all layers, in order
- 0x2019 Palette chunk for the full palette

**Every frame:**
- 0x2005 Cel chunks for each (layer, frame) with content
  - Raster cels: type 2 (compressed image), ZLIB level 6
  - Linked cels: type 1
  - Tilemap cels: type 3 (compressed tilemap)
- 0x2020 User Data immediately after each cel with non-empty user data

**First frame only (continued, after cels):**
- 0x2018 Tags chunk (all frame tags in one chunk)
- 0x2020 User Data for each tag, in order
- 0x2022 Slice chunks, one per slice
- 0x2020 User Data for each slice
- 0x2023 Tileset chunks, one per tileset
- 0x2020 User Data for each tileset

Chunks not written: Old Palette (0x0004, 0x0011), Cel Extra (0x2006), Color Profile
(0x2007), External Files (0x2008), Mask (0x2016), Path (0x2017).

## Known gaps

These are documented limitations of v1. Each has an issue reference or a stream that
will address it.

| Gap | Impact | Workaround |
|---|---|---|
| ICC color profile stripped on save | Color-managed workflows see visual shift on reload | Warn on open; users working display-referred see no impact |
| External tileset references inlined | Original external file path lost | Warn on save; re-export from Pixhaus to restore |
| User data properties map dropped | Aseprite extension metadata (e.g., custom physics props) lost | Warn if properties present; text+color preserved |
| Z-index on cels ignored | Rare; only used for multi-layer composite tricks | Warn if any cel has non-zero z-index |
| Pixel ratio (non-square pixels) ignored | Projects for non-square pixel displays render incorrectly | Warn if ratio ≠ 1:1 |
| Grid settings not preserved | Grid overlay resets to Aseprite default on reload | No warning; grid is editor state, not project state |
| Layer UUID not preserved | Aseprite extension workflows using UUIDs lose references | No warning in v1; UUIDs are an extension feature |
| Cel Extra (float bounds) dropped | Transform-cel workflows lose sub-pixel positioning | No warning; affects only files using Aseprite's transform tools |
| Auto-flip tileset matching ignored | Auto-mirroring of tiles in tilemap layers | Warn if auto-flip flags are set |

## Round-trip summary

Opening an `.aseprite` file in Pixhaus and saving it back to `.aseprite`:

- All raster and tilemap layer content preserved
- All frame tags, loop directions, and repeat counts preserved
- Full palette (RGBA + names) preserved
- All slices (bounds, nine-slice, pivot) preserved
- All blend modes and opacities preserved
- Linked cels preserved
- User data text and color preserved
- ICC color profile stripped (warning)
- External tileset inlined (warning if applicable)
- User data properties map stripped (warning if applicable)
- Z-index lost (warning if applicable)

A file that opens in Aseprite, saves to Pixhaus, and reopens in Aseprite should
look identical for the common case. The warnings above flag the cases where it
will not.

## Reference fixtures

The `examples/aseprite-roundtrip/` directory contains test fixtures demonstrating
the supported feature set. See `examples/aseprite-roundtrip/README.md` for the
fixture inventory.

The S08 stream (`.aseprite` read/write implementation) is responsible for generating
these fixtures and writing the round-trip test suite against them.
