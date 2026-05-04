# Unity handoff format

Pixhaus exports two artifact pairs for Unity consumption: a sprite sheet (PNG + JSON) and a tilemap (TMX + tileset PNGs). This document specifies both, pins the Aseprite JSON compatibility surface, and documents edge cases the Unity importer must handle.

The exporter (S10 — PNG sprite sheet + JSON export) and the Unity importer package (S39) consume this document as their interface contract. Changes to either format require a version bump here first.

---

## Sprite sheet

### Output files

```
{sprite_name}.png    packed sprite sheet image
{sprite_name}.json   frame metadata
```

Both files sit in the same output directory. The JSON `meta.image` field contains the PNG filename with no path component — importers resolve it relative to the JSON file.

### JSON schema

Pixhaus emits the Aseprite JSON **array-of-frames** variant. The array variant (rather than the hash-of-frames variant) is required because it preserves frame order unambiguously and is what most Unity importers expect.

Complete example:

```json
{
  "frames": [
    {
      "filename":         "hero 0",
      "frame":            { "x": 0,  "y": 0, "w": 16, "h": 16 },
      "rotated":          false,
      "trimmed":          false,
      "spriteSourceSize": { "x": 0,  "y": 0, "w": 16, "h": 16 },
      "sourceSize":       { "w": 16, "h": 16 },
      "duration":         200
    },
    {
      "filename":         "hero 1",
      "frame":            { "x": 16, "y": 0, "w": 16, "h": 16 },
      "rotated":          false,
      "trimmed":          false,
      "spriteSourceSize": { "x": 0,  "y": 0, "w": 16, "h": 16 },
      "sourceSize":       { "w": 16, "h": 16 },
      "duration":         100
    }
  ],
  "meta": {
    "app":     "Pixhaus",
    "version": "1.0",
    "image":   "hero.png",
    "format":  "RGBA8888",
    "size":    { "w": 64, "h": 16 },
    "scale":   "1",
    "frameTags": [
      {
        "name":      "idle",
        "from":      0,
        "to":        0,
        "direction": "forward",
        "repeat":    0,
        "color":     "#000000ff"
      },
      {
        "name":      "walk",
        "from":      1,
        "to":        3,
        "direction": "forward",
        "repeat":    0,
        "color":     "#000000ff"
      }
    ],
    "layers": [
      { "name": "shadow", "opacity": 128, "blendMode": "multiply" },
      { "name": "body",   "opacity": 255, "blendMode": "normal"   }
    ],
    "slices": [
      {
        "name":  "head",
        "color": "#0000ffff",
        "keys": [
          {
            "frame":  0,
            "bounds": { "x": 4, "y": 0, "w": 8, "h": 8 },
            "center": { "x": 1, "y": 1, "w": 6, "h": 6 }
          }
        ]
      },
      {
        "name":  "root",
        "color": "#0000ffff",
        "keys": [
          {
            "frame":  0,
            "bounds": { "x": 0, "y": 0, "w": 16, "h": 16 },
            "pivot":  { "x": 8, "y": 15 }
          }
        ]
      }
    ]
  }
}
```

#### `frames` array

Each entry describes one frame of the sprite.

| Field              | Type   | Description                                                          |
|--------------------|--------|----------------------------------------------------------------------|
| `filename`         | string | `"{sprite_name} {frame_index}"`. Zero-indexed.                       |
| `frame`            | Rect   | Rectangle of this frame in the packed PNG. `{ x, y, w, h }`.        |
| `rotated`          | bool   | Always `false`. Pixhaus does not rotate frames in the atlas.         |
| `trimmed`          | bool   | `true` if the frame was alpha-trimmed; `false` for untrimmed export. |
| `spriteSourceSize` | Rect   | Canvas region covered (same as `frame` when not trimmed).            |
| `sourceSize`       | Size   | Full canvas size. `{ w, h }`.                                        |
| `duration`         | number | Display duration in milliseconds. Sourced from `Frame.duration_ms`.  |

The `frame` rect is in the packed PNG's coordinate space. When frames are trimmed, `spriteSourceSize` describes where the trimmed region sits within the original canvas, and `sourceSize` gives the full canvas extent — importers use these to reconstruct the correct pivot/bounds in world space.

#### `meta` fields

| Field       | Type   | Description                                                            |
|-------------|--------|------------------------------------------------------------------------|
| `app`       | string | Always `"Pixhaus"`.                                                    |
| `version`   | string | Handoff schema version. Currently `"1.0"`.                             |
| `image`     | string | PNG filename, basename only — no directory component.                  |
| `format`    | string | Always `"RGBA8888"`. Indexed sprites are promoted to RGBA on export.   |
| `size`      | Size   | Packed sheet dimensions.                                               |
| `scale`     | string | Always `"1"`. Kept for Aseprite importer compatibility.                |
| `frameTags` | array  | Named frame ranges from `Sprite.frame_tags`. See below.                |
| `layers`    | array  | Layer list from `Sprite.layers`. See below.                            |
| `slices`    | array  | Named regions from `Sprite.slices`. See below.                         |

#### `meta.frameTags`

Maps to `Sprite.frame_tags`. Each entry is one `FrameTag`.

| Field       | Type   | Description                                                               |
|-------------|--------|---------------------------------------------------------------------------|
| `name`      | string | `FrameTag.name`.                                                          |
| `from`      | number | First frame index, inclusive. `FrameTag.range.start`.                    |
| `to`        | number | Last frame index, inclusive. `FrameTag.range.end`.                       |
| `direction` | string | Loop direction. See mapping below.                                        |
| `repeat`    | number | `FrameTag.repeat`. `0` means loop forever; positive values bound cycles.  |
| `color`     | string | `"#RRGGBBAA"`. Defaults to `"#000000ff"` when not set.                    |

Loop direction mapping:

| `LoopDirection` (data model) | `direction` (JSON)   |
|------------------------------|----------------------|
| `forward`                    | `"forward"`          |
| `reverse`                    | `"reverse"`          |
| `ping_pong`                  | `"pingpong"`         |
| `ping_pong_reverse`          | `"pingpong_reverse"` |

`"pingpong_reverse"` was introduced in Aseprite 1.3. Strict pre-1.3 importers that reject unrecognized direction values should be updated to treat unknown directions as `"forward"`. The Unity importer (S39) must handle all four values.

#### `meta.layers`

Lists all non-reference, non-empty-group layers in bottom-to-top order. Reference layers (`LayerKind::Reference`) are excluded by default.

| Field       | Type   | Description                                       |
|-------------|--------|---------------------------------------------------|
| `name`      | string | `Layer.name`.                                     |
| `opacity`   | number | `Layer.opacity`. Range `0`–`255`.                 |
| `blendMode` | string | `Layer.blend_mode` serialized as a JSON string.   |

Blend mode mapping:

| Data model    | JSON string      |
|---------------|------------------|
| `Normal`      | `"normal"`       |
| `Darken`      | `"darken"`       |
| `Multiply`    | `"multiply"`     |
| `ColorBurn`   | `"color_burn"`   |
| `Lighten`     | `"lighten"`      |
| `Screen`      | `"screen"`       |
| `ColorDodge`  | `"color_dodge"`  |
| `Addition`    | `"addition"`     |
| `Overlay`     | `"overlay"`      |
| `SoftLight`   | `"soft_light"`   |
| `HardLight`   | `"hard_light"`   |
| `Difference`  | `"difference"`   |
| `Exclusion`   | `"exclusion"`    |
| `Subtract`    | `"subtract"`     |
| `Divide`      | `"divide"`       |
| `Hue`         | `"hue"`          |
| `Saturation`  | `"saturation"`   |
| `Color`       | `"color"`        |
| `Luminosity`  | `"luminosity"`   |

Group layers appear in the list (by name) and carry no pixel content.

#### `meta.slices`

Maps to `Sprite.slices`. Each entry is one `Slice`.

| Field              | Type   | Description                                                             |
|--------------------|--------|-------------------------------------------------------------------------|
| `name`             | string | `Slice.name`.                                                           |
| `color`            | string | Always `"#0000ffff"`. Reserved for future color-coding of slice groups. |
| `keys[].frame`     | number | Frame from which this key takes effect. `SliceKey.frame`.               |
| `keys[].bounds`    | Rect   | Slice rectangle in canvas coordinates. `SliceKey.bounds`.               |
| `keys[].center`    | Rect   | Nine-slice center patch relative to `bounds.origin`. Present when `SliceKey.nine_slice` is set; omitted otherwise. |
| `keys[].pivot`     | IVec2  | Pivot offset from `bounds.origin`. Present when `SliceKey.pivot` is set; omitted otherwise. |

`center` maps to `NineSlice.center`. `pivot` maps to `Pivot.offset` as `{ "x": ..., "y": ... }`.

---

## Tilemap

### Output files

```
{map_name}.tmx           Tiled map document
{tileset_name}.tsx       Tileset descriptor (one file per tileset)
{tileset_name}.png       Tileset atlas (referenced from the TSX)
```

All files sit in the same output directory. The TMX references TSX files by relative path; each TSX references its PNG by relative path.

### TMX document

Pixhaus emits Tiled 1.10-compatible TMX XML. One `<layer>` element is emitted per tilemap layer in the sprite. Non-tilemap layers are not represented in the TMX — the tilemap export is tile-only.

```xml
<?xml version="1.0" encoding="UTF-8"?>
<map version="1.10" tiledversion="1.10.0"
     orientation="orthogonal" renderorder="right-down"
     width="8" height="8"
     tilewidth="16" tileheight="16"
     infinite="0" nextlayerid="3" nextobjectid="1">

  <tileset firstgid="1" source="dungeon.tsx"/>

  <layer id="1" name="ground" width="8" height="8">
    <data encoding="csv">
1,1,1,1,1,1,1,1,
1,2,2,2,2,2,2,1,
1,2,3,3,3,3,2,1,
1,2,3,0,0,3,2,1,
1,2,3,0,0,3,2,1,
1,2,3,3,3,3,2,1,
1,2,2,2,2,2,2,1,
1,1,1,1,1,1,1,1
    </data>
  </layer>
</map>
```

#### Map attributes

| Attribute      | Value                                                         |
|----------------|---------------------------------------------------------------|
| `version`      | `"1.10"`                                                      |
| `tiledversion` | `"1.10.0"`                                                    |
| `orientation`  | `"orthogonal"`                                                |
| `renderorder`  | `"right-down"`                                                |
| `width`        | Map width in tiles. `TilemapData.width` of the first layer.   |
| `height`       | Map height in tiles. `TilemapData.height` of the first layer. |
| `tilewidth`    | Tile width in pixels. `Tileset.tile_size.width`.              |
| `tileheight`   | Tile height in pixels. `Tileset.tile_size.height`.            |
| `infinite`     | `"0"`. Pixhaus tilemaps are bounded.                          |

If a sprite has multiple tilemap layers backed by tilesets with different tile sizes, the map attributes use the dimensions of the first tileset. The Unity importer must handle per-layer tile sizes if it needs to support this.

#### Tileset references

Each distinct tileset used in the map emits one `<tileset>` element. Tilesets are assigned ascending `firstgid` values starting at `1`. If a sprite has two tilesets with `tile_count` values `T1` and `T2`, their `firstgid` values are `1` and `1 + T1`.

```xml
<tileset firstgid="1"       source="ground.tsx"/>
<tileset firstgid="17"      source="objects.tsx"/>
```

#### Tile data encoding

Tile data is emitted as CSV (`encoding="csv"`). Each row is one line; all rows except the last end with a trailing comma. Each value is a 32-bit unsigned integer.

Bit layout:

| Bits  | Meaning                                          |
|-------|--------------------------------------------------|
| 0–28  | Global tile ID (GID). `0` = empty cell.          |
| 29    | `TileFlags::FLIP_DIAGONAL` — diagonal flip.      |
| 30    | `TileFlags::FLIP_Y` — vertical flip.             |
| 31    | `TileFlags::FLIP_X` — horizontal flip.           |

Encoding formula for a cell with `(index, flags)` against a tileset with `firstgid`:

```
gid = 0                           when index == 0 (empty tile)

gid = index + firstgid - 1        otherwise, before flags

gid |= 0x80000000                 when TileFlags::FLIP_X is set
gid |= 0x40000000                 when TileFlags::FLIP_Y is set
gid |= 0x20000000                 when TileFlags::FLIP_DIAGONAL is set
```

The `index + firstgid - 1` formula places tile index 1 at GID `firstgid`, tile index 2 at GID `firstgid + 1`, and so on. Index 0 (empty) always encodes as GID 0 regardless of `firstgid`.

Example encoded values (single tileset, firstgid=1):

| `TileIndex` | `TileFlags`              | Encoded value |
|-------------|--------------------------|---------------|
| 0           | empty                    | `0`           |
| 1           | none                     | `1`           |
| 2           | none                     | `2`           |
| 1           | `FLIP_X`                 | `2147483649`  |
| 2           | `FLIP_Y`                 | `1073741826`  |
| 3           | `FLIP_DIAGONAL`          | `536870915`   |
| 4           | `FLIP_X \| FLIP_Y`      | `3221225476`  |

The bit layout matches Tiled's native format, so standard Tiled tooling (including SuperTiled2Unity) decodes it correctly without any special handling.

#### TSX tileset file

```xml
<?xml version="1.0" encoding="UTF-8"?>
<tileset version="1.10" tiledversion="1.10.0"
         name="dungeon"
         tilewidth="16" tileheight="16"
         spacing="0" margin="0"
         tilecount="6" columns="6">
  <image source="dungeon.png" width="96" height="16"/>
</tileset>
```

Tiles are packed in a **single horizontal row** in the PNG atlas. Atlas width is `tile_size.width * tile_count`; atlas height is `tile_size.height`. Tile index 0 (the empty tile) occupies the leftmost column and is fully transparent. Importers must skip index 0 when constructing Unity tile assets.

#### TSX attributes

| Attribute    | Value                                                         |
|--------------|---------------------------------------------------------------|
| `name`       | `Tileset.name`.                                               |
| `tilewidth`  | `Tileset.tile_size.width`.                                    |
| `tileheight` | `Tileset.tile_size.height`.                                   |
| `spacing`    | `0`. No gaps between tiles in the atlas.                      |
| `margin`     | `0`. No border around the atlas.                              |
| `tilecount`  | `Tileset.tile_count`. Includes the empty tile at index 0.     |
| `columns`    | `Tileset.tile_count`. Single row, N columns.                  |

---

## Edge cases

### Missing frames

A frame with no cel on any layer composites to fully transparent pixels. The exporter still includes the frame in the sprite sheet (as a transparent region) and in the JSON `frames` array with its duration. Importers must not skip zero-content frames.

### Indexed color mode

Sprites in `ColorMode::Indexed` are converted to RGBA during export. The active palette's entries map to RGBA pixel values. The JSON `meta.format` field is always `"RGBA8888"` regardless of source color mode; there is no indexed PNG export path.

### Multiple animations in one file

`meta.frameTags` lists all `FrameTag` entries. The frame strip covers every frame across all tags, laid out contiguously in frame-index order. Importers that need a single animation select by `name` and read `from`/`to` to slice the strip. Tags may overlap (a "combo" tag spanning frames also covered by "walk" and "attack" tags); that is valid and importers must not assume tags are non-overlapping.

### Animated tiles

Tiled 1.10 supports `<animation>` blocks inside `<tile>` elements. Pixhaus v1.0 does not emit `<animation>` elements. If a tileset tile is animated in the source sprite, each animation frame is a distinct tile entry in the atlas — the Unity importer is responsible for assembling Unity `Tile.AnimationSpeed` / `Tile.AnimationFrames` from those entries. A future format version may emit `<animation>` blocks.

### Reference layers

`LayerKind::Reference` layers are excluded from the composited sprite sheet and from `meta.layers`. They exist in the `.pixhaus` project only as non-destructive visual guides. A future export flag may allow opt-in inclusion.

### Group layers

`LayerKind::Group` layers appear in `meta.layers` with their name, opacity, and blend mode. Their pixel content is the composite of their children. The exporter flattens the group's children using the group's blend mode and opacity before packing the sheet; the JSON layer entry is metadata only and carries no pixel rect of its own.

### Sprites with no animations

If `Sprite.frame_tags` is empty, `meta.frameTags` is an empty array. The importer treats the entire frame strip as a single un-tagged sequence.

---

## Compatibility notes

### Aseprite JSON importers

`meta.app` is set to `"Pixhaus"` rather than Aseprite's canonical `"https://www.aseprite.org/"`. Most Unity Aseprite importers do not gate on this field — they parse frame data regardless of the source application. Strict importers that reject non-Aseprite origins must be patched to allow `"Pixhaus"`. The field is purely cosmetic.

`meta.version` is `"1.0"` (Pixhaus handoff schema version), not an Aseprite version string. Importers must not use it for Aseprite-version capability negotiation.

The Aseprite JSON spec does not include a `repeat` field on frame tags in pre-1.3 versions. Importers that do not recognize `repeat` should ignore it and default to infinite looping.

### SuperTiled2Unity

SuperTiled2Unity reads TMX via the standard Tiled XML format. The CSV tile encoding and flip-flag bit layout used here match SuperTiled2Unity's expectations verbatim. Test target: SuperTiled2Unity 2.0+ with Unity 2022.3 LTS.

Known gap: SuperTiled2Unity does not import Tiled object layers or Wang tiles, which Pixhaus does not emit anyway. No compatibility issue.

### Tiled editor

The emitted TMX and TSX target Tiled 1.10. Tiled 1.9 is also supported; the only difference is the `version` / `tiledversion` attribute values. No Tiled-exclusive features beyond basic orthogonal tilemaps are used. Files can be opened in Tiled for inspection and debugging.

---

## Version history

| Version | Date       | Changes                                                         |
|---------|------------|-----------------------------------------------------------------|
| 1.0     | 2026-05-04 | Initial spec. Sprite sheet + tilemap. No animated tile support. |
