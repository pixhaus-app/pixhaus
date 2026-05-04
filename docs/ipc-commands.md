# IPC command catalog

All Tauri commands exposed to the UI. Commands are grouped by category and
sorted alphabetically within each group.

The Rust implementations live in `app/src/commands/`. TypeScript wrappers live
in `ui/src/lib/commands/`. This document is the canonical contract between
the two sides.

## Latency contracts

| Label | Meaning |
|---|---|
| `<1 ms` | Synchronous in-memory read or mutation. Never touches disk or a pixel buffer. |
| `<50 ms` | Light I/O or small buffer work. Runs on the tokio thread pool. |
| `<500 ms` | Disk I/O or non-trivial computation. Callers should show a spinner. |
| `stub` | Not yet implemented. Rejects with `Unimplemented { stream }`. |

## Error contract

Every command in this catalog returns `Result<T, AppCommandError>`. The TS side receives a discriminated union with the wire shape `{ kind, message? }`; the Rust enum is `app/src/error.rs::AppCommandError` and the generated TS type is `ui/src/lib/types/AppCommandError.ts`.

Variants:

| `kind` | Payload | When |
|---|---|---|
| `no_active_project` | — | The active document is `None` (no project open). |
| `not_found` | `entity: String, id: u64` | Lookup by integer ID failed (sprite, layer, frame, palette, etc.). |
| `not_found_by_name` | `entity: String, name: String` | Lookup by name failed (frame tag, animation). |
| `out_of_range` | `detail: String` | Index, position, or count outside valid bounds. |
| `conflict` | `detail: String` | Duplicate name or other invariant clash. |
| `unimplemented` | `stream: String` | Stub command awaiting the named stream. |
| `validation` | `detail: String` | Argument validation failure (overflow, malformed input). |

UI surfaces should switch on `kind` and localise the user-visible text — never string-match `message`. The `message` field is for diagnostics and may include identifiers, indexes, or other detail that varies per call.

Per-command **Errors** rows below name the variants a given command can emit; the message text is illustrative, not contractual.

---

## Project

Commands that manage the project lifecycle and its top-level sprite collection.

### `project_new`

Creates a new empty project, replacing any currently open document.

| | |
|---|---|
| **Arguments** | `name: String` |
| **Returns** | `ProjectStatus` |
| **Errors** | — |
| **Latency** | `<1 ms` |
| **Side effects** | Replaces the active document. Resets the ID counter. Sets `dirty = true`. |

`ProjectStatus`:
```ts
{
  metadata: ProjectMetadata;
  path: string | null;
  dirty: boolean;
  sprite_count: number;
}
```

### `project_open`

Opens a project from disk. **Stub** — requires B3 (`.pixhaus` format).

| | |
|---|---|
| **Arguments** | `path: String` |
| **Returns** | `ProjectStatus` |
| **Errors** | Always: `unimplemented` |
| **Latency** | `stub` → `<500 ms` when implemented |

### `project_save`

Saves the active project to the given path. **Stub** — requires B3.

| | |
|---|---|
| **Arguments** | `path: Option<String>` — uses current path if `null` |
| **Returns** | `()` |
| **Errors** | Always: `unimplemented` |
| **Latency** | `stub` → `<500 ms` when implemented |

### `project_close`

Closes the active project, discarding all in-memory state.

| | |
|---|---|
| **Arguments** | — |
| **Returns** | `()` |
| **Errors** | — |
| **Latency** | `<1 ms` |
| **Side effects** | Clears the document, path, and dirty flag. |

### `project_get`

Returns the active project's status, or `null` if no project is open.

| | |
|---|---|
| **Arguments** | — |
| **Returns** | `ProjectStatus \| null` |
| **Errors** | — |
| **Latency** | `<1 ms` |

### `sprite_add`

Adds a new empty sprite to the active project.

| | |
|---|---|
| **Arguments** | `args: SpriteAddArgs` |
| **Returns** | `Sprite` |
| **Errors** | `no_active_project` |
| **Latency** | `<1 ms` |

`SpriteAddArgs`:
```ts
{
  name: string;
  canvas_width: number;
  canvas_height: number;
  color_mode: ColorMode;
}
```

### `sprite_delete`

Removes a sprite from the active project by ID.

| | |
|---|---|
| **Arguments** | `sprite_id: SpriteId` |
| **Returns** | `()` |
| **Errors** | `no_active_project`, `not_found` (sprite) |
| **Latency** | `<1 ms` |

### `sprite_list`

Returns all sprites in the active project.

| | |
|---|---|
| **Arguments** | — |
| **Returns** | `Sprite[]` |
| **Errors** | `no_active_project` |
| **Latency** | `<1 ms` |

---

## Canvas

Commands that act on the canvas viewport and pixel data.

### `canvas_draw_stroke`

Paints a freehand stroke. **Stub** — requires S01 (pixel buffers).

| | |
|---|---|
| **Arguments** | `args: DrawStrokeArgs` |
| **Returns** | `()` |
| **Errors** | Always: `unimplemented` |
| **Latency** | `stub` → `<50 ms` for typical strokes when implemented |

`DrawStrokeArgs`:
```ts
{
  sprite_id: SpriteId;
  layer_id: LayerId;
  frame_index: number;
  points: Array<[number, number]>;
  color: Rgba;
  pressure: number[];
}
```

### `canvas_fill`

Flood-fills a contiguous region. **Stub** — requires S01.

| | |
|---|---|
| **Arguments** | `args: FillArgs` |
| **Returns** | `()` |
| **Errors** | Always: `unimplemented` |
| **Latency** | `stub` → `<50 ms` when implemented |

`FillArgs`:
```ts
{
  sprite_id: SpriteId; layer_id: LayerId; frame_index: number;
  x: number; y: number; color: Rgba; tolerance: number;
}
```

### `canvas_transform`

Applies translate/flip/rotate to a cel. **Stub** — requires S01.

| | |
|---|---|
| **Arguments** | `args: TransformArgs` |
| **Returns** | `()` |
| **Errors** | Always: `unimplemented` |
| **Latency** | `stub` → `<50 ms` when implemented |

`TransformArgs`:
```ts
{
  sprite_id: SpriteId; layer_id: LayerId; frame_index: number;
  translate_x: number; translate_y: number;
  flip_x: boolean; flip_y: boolean;
  rotate_cw90: number;
}
```

### `canvas_set_selection`

Sets the canvas selection. Pass `null` for `region` to clear.

| | |
|---|---|
| **Arguments** | `region: SelectionRegion \| null`, `anchor_layer: LayerId \| null` |
| **Returns** | `SelectionState` |
| **Errors** | `no_active_project` |
| **Latency** | `<1 ms` |

### `canvas_set_viewport`

Replaces the entire canvas viewport state.

| | |
|---|---|
| **Arguments** | `canvas: CanvasState` |
| **Returns** | `CanvasState` |
| **Errors** | `no_active_project` |
| **Latency** | `<1 ms` |
| **Side effects** | Persisted in the project so save/load restores the viewport. |

---

## Layers

Commands that manage a sprite's layer stack.

### `layer_add`

Adds a new layer to a sprite. Appended above all existing layers.

| | |
|---|---|
| **Arguments** | `args: LayerAddArgs` |
| **Returns** | `Layer` |
| **Errors** | `no_active_project`, `not_found` (sprite) |
| **Latency** | `<1 ms` |

`LayerAddArgs`:
```ts
{
  sprite_id: SpriteId;
  name: string;
  kind: LayerKind;
}
```

### `layer_delete`

Removes a layer and all its cels.

| | |
|---|---|
| **Arguments** | `sprite_id: SpriteId`, `layer_id: LayerId` |
| **Returns** | `()` |
| **Errors** | `no_active_project`, `not_found` (sprite or layer) |
| **Latency** | `<1 ms` |

### `layer_list`

Returns all layers in a sprite, bottom to top.

| | |
|---|---|
| **Arguments** | `sprite_id: SpriteId` |
| **Returns** | `Layer[]` |
| **Errors** | `no_active_project`, `not_found` (sprite) |
| **Latency** | `<1 ms` |

### `layer_rename`

Renames a layer.

| | |
|---|---|
| **Arguments** | `sprite_id: SpriteId`, `layer_id: LayerId`, `name: String` |
| **Returns** | `LayerRenamed { layer_id, name }` |
| **Errors** | `no_active_project`, `not_found` (sprite or layer) |
| **Latency** | `<1 ms` |

### `layer_reorder`

Moves a layer to a new stack position. `new_index` is the **final position the layer lands at** in the resulting stack, clamped to `[0, len-1]`. After the call, `sprite.layers[new_index].id == layer_id`.

| | |
|---|---|
| **Arguments** | `sprite_id: SpriteId`, `layer_id: LayerId`, `new_index: u32` |
| **Returns** | `()` |
| **Errors** | `no_active_project`, `not_found` (sprite or layer) |
| **Latency** | `<1 ms` |

### `layer_set_blend_mode`

| | |
|---|---|
| **Arguments** | `sprite_id`, `layer_id`, `blend_mode: BlendMode` |
| **Returns** | `()` |
| **Errors** | `no_active_project`, `not_found` (sprite or layer) |
| **Latency** | `<1 ms` |

### `layer_set_opacity`

| | |
|---|---|
| **Arguments** | `sprite_id`, `layer_id`, `opacity: u8` (0–255) |
| **Returns** | `()` |
| **Latency** | `<1 ms` |

### `layer_set_visibility`

| | |
|---|---|
| **Arguments** | `sprite_id`, `layer_id`, `visible: bool` |
| **Returns** | `()` |
| **Latency** | `<1 ms` |

### `layer_set_locked`

| | |
|---|---|
| **Arguments** | `sprite_id`, `layer_id`, `locked: bool` |
| **Returns** | `()` |
| **Latency** | `<1 ms` |

---

## Frames

Commands that manage a sprite's frame timeline and frame tags.

### `frame_add`

Appends a new frame at the end of the timeline.

| | |
|---|---|
| **Arguments** | `sprite_id: SpriteId`, `duration_ms: u32` |
| **Returns** | `FrameAddResult { frame: Frame, index: FrameIndex }` |
| **Errors** | `no_active_project`, `not_found` (sprite) |
| **Latency** | `<1 ms` |

### `frame_delete`

Deletes a frame and all its cels.

| | |
|---|---|
| **Arguments** | `sprite_id`, `frame_index: FrameIndex` |
| **Returns** | `()` |
| **Errors** | `no_active_project`, `not_found` (sprite), `out_of_range` |
| **Latency** | `<1 ms` |

### `frame_duplicate`

Duplicates a frame, inserting the copy immediately after the source.

| | |
|---|---|
| **Arguments** | `sprite_id`, `frame_index` |
| **Returns** | `FrameAddResult` |
| **Latency** | `<1 ms` |

### `frame_reorder`

Moves a frame from one timeline position to another.

| | |
|---|---|
| **Arguments** | `sprite_id`, `from_index`, `to_index` |
| **Returns** | `()` |
| **Latency** | `<1 ms` |

### `frame_set_duration`

Updates the display duration for a single frame.

| | |
|---|---|
| **Arguments** | `sprite_id`, `frame_index`, `duration_ms: u32` |
| **Returns** | `()` |
| **Latency** | `<1 ms` |

### `frame_list`

Returns all frames in a sprite's timeline.

| | |
|---|---|
| **Arguments** | `sprite_id` |
| **Returns** | `Frame[]` |
| **Latency** | `<1 ms` |

### `frame_tag_create`

Creates a named frame tag. Tags with duplicate names are rejected.

| | |
|---|---|
| **Arguments** | `args: FrameTagCreateArgs` |
| **Returns** | `FrameTag` |
| **Errors** | `no_active_project`, `not_found` (sprite), `conflict` |
| **Latency** | `<1 ms` |

`FrameTagCreateArgs`:
```ts
{
  sprite_id: SpriteId; name: string; range: FrameRange;
  loop_direction: LoopDirection; repeat: number;
}
```

### `frame_tag_delete`

Removes a named frame tag.

| | |
|---|---|
| **Arguments** | `sprite_id`, `tag_name: String` |
| **Returns** | `()` |
| **Errors** | `no_active_project`, `not_found` (sprite), `not_found_by_name` (frame_tag) |
| **Latency** | `<1 ms` |

---

## Tiles

Tilemap editing commands. All are **stubs** until S06 (tilemap data structures) lands.

### `tile_place`

Places a tile cell on a tilemap layer. **Stub** — requires S06.

| | |
|---|---|
| **Arguments** | `args: TilePlaceArgs` |
| **Returns** | `()` |
| **Latency** | `stub` |

### `tile_erase`

Erases a tile cell on a tilemap layer. **Stub** — requires S06.

| | |
|---|---|
| **Arguments** | `args: TileEraseArgs` |
| **Returns** | `()` |
| **Latency** | `stub` |

### `tile_autotile_apply`

Applies autotile rules to a region. **Stub** — requires S06.

| | |
|---|---|
| **Arguments** | `args: AutotileArgs` |
| **Returns** | `()` |
| **Latency** | `stub` |

---

## Palette

Commands that manage a sprite's palettes and swatches.

### `palette_add`

Adds a new empty palette to a sprite.

| | |
|---|---|
| **Arguments** | `sprite_id`, `name: String` |
| **Returns** | `Palette` |
| **Errors** | `no_active_project`, `not_found` (sprite) |
| **Latency** | `<1 ms` |

### `palette_delete`

Removes a palette from a sprite.

| | |
|---|---|
| **Arguments** | `sprite_id`, `palette_id` |
| **Returns** | `()` |
| **Latency** | `<1 ms` |

### `palette_add_color`

Appends a color to a palette. Returns the new swatch index.

| | |
|---|---|
| **Arguments** | `args: PaletteAddColorArgs` |
| **Returns** | `number` (swatch index) |
| **Latency** | `<1 ms` |

`PaletteAddColorArgs`:
```ts
{ sprite_id, palette_id, color: Rgba, name?: string | null }
```

### `palette_remove_color`

Removes the swatch at a given index.

| | |
|---|---|
| **Arguments** | `sprite_id`, `palette_id`, `index: u32` |
| **Returns** | `()` |
| **Errors** | `no_active_project`, `not_found` (sprite or palette), `out_of_range` |
| **Latency** | `<1 ms` |

### `palette_set_color`

Replaces the color (and optionally the name) at a specific index.

| | |
|---|---|
| **Arguments** | `args: PaletteSetColorArgs` |
| **Returns** | `()` |
| **Latency** | `<1 ms` |

### `palette_swap`

Swaps the positions of two palettes in a sprite's palette list.

| | |
|---|---|
| **Arguments** | `sprite_id`, `from_id`, `to_id` |
| **Returns** | `PaletteSwapResult { from_id, to_id }` |
| **Latency** | `<1 ms` |

### `palette_list`

Returns all palettes in a sprite.

| | |
|---|---|
| **Arguments** | `sprite_id` |
| **Returns** | `Palette[]` |
| **Latency** | `<1 ms` |

---

## Verbs

AI verb invocation commands. Invoke and cancel are **stubs** until B5 (verb
plugin protocol) lands. `verb_list` returns an empty array as a safe default.

### `verb_invoke`

Invokes a registered AI verb. **Stub** — requires B5.

| | |
|---|---|
| **Arguments** | `args: VerbInvokeArgs { name: string, context: unknown }` |
| **Returns** | `VerbResult { verb_id: string, status: VerbStatus }` |
| **Latency** | `stub` → verb-dependent when implemented |

`VerbStatus` is a tagged union:
```ts
| { kind: "pending" }
| { kind: "done" }
| { kind: "error"; message: string }
```

### `verb_list`

Lists all registered verbs. Returns `[]` until B5 populates the registry.

| | |
|---|---|
| **Arguments** | — |
| **Returns** | `VerbInfo[]` |
| **Latency** | `<1 ms` |

`VerbInfo`:
```ts
{ name: string; description: string; required_backends: string[] }
```

### `verb_cancel`

Cancels an in-progress verb invocation. **Stub** — requires B5.

| | |
|---|---|
| **Arguments** | `verb_id: String` |
| **Returns** | `()` |
| **Latency** | `stub` |

---

## Schema evolution

- **Additive**: adding optional fields to arg or result types is backward-compatible.
- **Renaming a command**: requires a deprecation period; old name kept as a forwarding alias.
- **Removing a command**: bump the catalog version and document in this file.
- **Changing a return type incompatibly**: coordinate with S13 (app shell) and S14 (viewport).

## Auto-generation

The plan is to generate TypeScript wrappers from Rust via `tauri-specta` once
stream S04 (specta integration) lands. Until then, the hand-mirrored wrappers
in `ui/src/lib/commands/` serve as the contract.

After S04 merges, `ui/src/lib/commands/index.ts` will re-export from the
generated `bindings.ts` instead. The function signatures will stay the same;
only the source of truth for the types changes.
