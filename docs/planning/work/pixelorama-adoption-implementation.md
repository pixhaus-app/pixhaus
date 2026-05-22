# Pixelorama adoption — implementation reconciliation

This doc reconciles the 39-entry catalog in
`docs/planning/research/pixelorama-adoption.md` (and the subset the user
asked for) against the code that already exists, then sequences the
genuine gaps for implementation.

The headline: most of the requested ideas are already shipped, and
several existing implementations are richer than the catalog's
suggestions. The user qualified several items with "only if it's not
contradicting our current version" and "only if it's better than what we
currently have" — those qualifiers resolve most items to keep-current.

Branch: `feat/pixelorama-adoption`.

## Verdict table

| # | Requested item | Current state | Verdict |
|---|---|---|---|
| 1 | Project file format + core data model | `.pixhaus` (MessagePack + zstd) + full B2 model in `core/src/project/` | Done — keep |
| 2 | Sparse palette (`HashMap<u16,_>`) | Dense `Vec<PaletteEntry>` with pages + animation | Keep current — sparse contradicts and loses pages/animation integration |
| 3 | Indexed color mode | `ColorMode::Indexed`, `IndexedBuffer`, `Sprite::transparent_color_index` | Done — keep |
| 4 | Cel linking via link-set IDs | `CelData::Linked { source_frame }` (same-layer) | Keep current; link-sets deferred (see Deferred) |
| 5 | Frame duration float multiplier | `Frame::duration_ms: u32` (absolute) | Keep current — absolute ms round-trips Aseprite cleanly; multiplier would regress interop |
| 6 | Animation tags `{name,from,to}` + direction at export | `FrameTag { name, range, loop_direction, repeat }` | Done — `loop_direction` already on the tag; export honors it |
| 7 | Non-destructive layer effects `Vec<LayerEffect>` | No `effects` field on `Layer` | **GAP — implement** |
| 8 | Drawing/selection/transform algorithms | flood fill, shapes, stroke, selection (algorithms/mask/morphology/autoclose), transforms (rotate/scale/skew/flip/perspective/antialias) | Keep current — already solid |
| 9 | Color similarity via squared distance | `nearest_color_index` uses squared distance; flood fill uses per-channel | **Minor gap — add shared `similar_colors` helper** |
| 10 | Seven rotation algorithms | `rotate_rotsprite` (+ 90/180/270, bilinear) | Keep current — RotSprite is the best general-purpose option; more variants deferred |
| 11 | Transform handles as floating overlay | `ui/src/canvas/transform/` (8 resize + rotate + body) | Done — keep |
| 12 | Onion skin `opacity = base / dist` + red/blue tint | `renderer/index.ts` `renderOnionSkin` + `ONION_FRAG` | Done — keep |
| 13 | Tile cell `{index, flip_h, flip_v, transpose}` | `TileCell { index, flags }`, `FLIP_X/Y/DIAGONAL` | Done — keep (diagonal == transpose) |
| 14 | TileSet shape + offset axis (iso/hex) | `Tileset` is square-only | **GAP — implement** |
| 15 | Autotile via peering bitmask | Rule-based `AutotileKind` (Blob47/Corner16/Minimal4/Custom) | Keep current — richer than a peering bitmask |
| 16 | Smart-slice spritesheet import | none | **GAP — implement** |
| 17 | Aseprite parser as chunk state machine | `io/src/aseprite/` (archive 84k, read/write) | Done — keep |
| 18 | Action-and-profile keyboard system | `ui/src/keybinds/` presets (Aseprite/Photoshop) + custom overrides | Done — keep |

## Gaps to implement (this branch)

Ordered by dependency then value. Each lands with unit tests and passes
`cargo clippy --tests -p <crate> -- -D warnings`.

### G1. Shared color-similarity helper (`core`)

`color::ops::similar_colors(a, b, tol)` using squared Euclidean distance
over normalized channels, matching the upstream default tolerance. Add it
next to `nearest_color_index`. Low risk, foundational for fill/select
consistency. Ported-algorithm attribution in `THIRD_PARTY_NOTICES.md`.

### G2. Non-destructive layer effects (`core` + `app` + `ui`)

The marquee gap and the one most aligned with Pixhaus's AI-overlay model
(the catalog notes verbs can emit overlay effects).

- `core/src/project/layer.rs`: add
  `#[serde(default, skip_serializing_if = "Vec::is_empty")] pub effects: Vec<LayerEffect>`
  to `Layer`. New `LayerEffect` enum: `Outline`, `DropShadow`,
  `Brightness`, `Invert` to start (CPU, deterministic, testable).
- `core/src/canvas/effects.rs`: pure functions
  `apply_effect(buffer, &LayerEffect) -> PixelBuffer` and
  `apply_effects(buffer, &[LayerEffect])`. Outline/drop-shadow operate on
  alpha; brightness/invert per-channel. Snapshot/image tests.
- `core/src/canvas/composite.rs`: `LayerInput` gains an `effects: &[LayerEffect]`
  field; `composite_onto` applies effects to a working copy before blending.
  Existing call sites pass `&[]`.
- `app`: extend layer commands with `set_layer_effects` and apply effects
  in the render path; expose via IPC.
- `ui`: layer panel gains a small effects affordance (add/remove/reorder).

Additive and back-compatible: old files deserialize with no effects.

### G3. TileSet shape + hex offset axis (`core`)

- `core/src/project/tileset.rs`: add `TileShape { Square, Isometric, HexPointy, HexFlat }`
  and `HexOffsetAxis`, plus `shape` + `hex_offset` fields on `Tileset`
  (serde-defaulted to `Square`/`None` so existing files load unchanged).
- `core/src/tilemap/`: `cell_to_pixel(shape, hex_offset, tile_size, x, y)`
  geometry with tests for the four shapes.

### G4. Smart-slice spritesheet import (`core` + `io`)

- `core/src/import/smart_slice.rs`: `detect_frames(&PixelBuffer, opts) -> Vec<Rect>`
  using the existing flood fill to mark background, connected-component
  labelling of foreground, bbox per component, optional grid snap.
- Wire into `io` sprite-sheet import where the PNG importer lives.

## Deferred (documented, not built here)

- **Cel link-sets** (#4): current same-layer `source_frame` linking covers
  the common idle-reuse case; cross-cel link sets with hue-rotate are a
  larger timeline/undo change. Revisit if requested.
- **Extra rotation variants** (CleanEdge/OmniScale) (#10): RotSprite is the
  default; add only on demand.
- **ZIP container format** (catalog entry 1): `stack.md` locks MessagePack;
  needs an ADR to revisit, out of scope here.

## Attribution

Ported algorithms (G1, G4) and adopted designs (G2, G3) get entries in
`THIRD_PARTY_NOTICES.md` under the existing Pixelorama section, per the
MIT-compliance mechanics in the research doc.
