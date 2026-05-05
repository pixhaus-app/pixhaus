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
| `project_save` | Save the active project (optional path argument for save-as) |
| `project_close` | Close the active project |
| `project_get` | Returns the active `ProjectStatus` or `None` |

Export commands (sprite sheet, `.aseprite`, GIF, TMX) live behind individual streams and are not yet exposed on the IPC surface. This page is updated as those streams ship.

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
| `palette_add` | Add a new palette to a sprite |
| `palette_delete` | Remove a palette by id |
| `palette_add_color` | Append a color to a palette |
| `palette_set_color` | Replace a color at a specific index |
| `palette_remove_color` | Remove a color by index |
| `palette_swap` | Swap two palettes' positions |
| `palette_list` | List all palettes on a sprite |

Palette file I/O (`.gpl`, `.hex`, `.pal`, Lospec) currently runs entirely in the UI without a backend round-trip; see the palette panel for usage.

## Error handling

Every command returns `Result<T, AppCommandError>`. Error variants include `no_active_project`, `not_found`, `out_of_range`, `conflict`, `unimplemented`, and `validation`. UI code switches on the `kind` field of the error; never string-match `message`.

## TypeScript wrappers

Every command has a typed TypeScript wrapper in `ui/src/lib/commands/`. These wrappers handle serialization and surface the error union as a TypeScript discriminated union.
