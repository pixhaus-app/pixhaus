---
title: IPC commands
description: Tauri IPC commands available to the UI and plugins.
---

import { Aside } from "@astrojs/starlight/components";

<Aside>
This page is an abbreviated reference. The full command catalog with all parameters and error variants is in `docs/ipc-commands.md` in the repository.
</Aside>

The Pixhaus UI communicates with the Rust backend via Tauri IPC commands. Plugin scripts that need low-level access can invoke these commands directly.

## Command categories

### Project

| Command | Description |
|---|---|
| `project_new` | Create a new empty project |
| `project_open` | Open a `.pixhaus` or `.aseprite` file |
| `project_save` | Save the active project |
| `project_save_as` | Save to a new path |
| `project_close` | Close the active project |
| `project_export_sprite_sheet` | Export frames as PNG + JSON |
| `project_export_aseprite` | Export to `.aseprite` format |
| `project_export_gif` | Export animated GIF |
| `project_export_tmx` | Export Tiled `.tmx` tilemap |

### Layers

| Command | Description |
|---|---|
| `layer_new` | Add a new raster or tilemap layer |
| `layer_delete` | Delete a layer by ID |
| `layer_reorder` | Move a layer to a new position |
| `layer_rename` | Rename a layer |
| `layer_set_blend_mode` | Change blend mode |
| `layer_set_opacity` | Change opacity |
| `layer_set_visibility` | Show or hide |
| `layer_set_locked` | Lock or unlock |

### Frames

| Command | Description |
|---|---|
| `frame_new` | Insert a new frame |
| `frame_delete` | Delete a frame |
| `frame_duplicate` | Duplicate a frame |
| `frame_set_duration` | Set frame duration in ms |
| `frame_tag_new` | Create a frame tag |
| `frame_tag_delete` | Delete a frame tag |

### Palette

| Command | Description |
|---|---|
| `palette_set_color` | Set a palette color by index |
| `palette_add_color` | Append a color to the palette |
| `palette_delete_color` | Remove a palette entry |
| `palette_load` | Load a palette file |
| `palette_save` | Save the palette to a file |

## Error handling

Every command returns `Result<T, AppCommandError>`. Error variants include `no_active_project`, `not_found`, `out_of_range`, `conflict`, `unimplemented`, and `validation`. UI code switches on the `kind` field of the error; never string-match `message`.

## TypeScript wrappers

Every command has a typed TypeScript wrapper in `ui/src/lib/commands/`. These wrappers handle serialization and surface the error union as a TypeScript discriminated union.
