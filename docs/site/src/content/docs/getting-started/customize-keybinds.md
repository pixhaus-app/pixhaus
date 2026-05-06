---
title: Customize keybinds and themes
description: Remap shortcuts, load a preset, and switch between light and dark themes.
---

import { Steps, Aside } from "@astrojs/starlight/components";

Pixhaus ships with three keybind presets (Pixhaus default, Aseprite-compatible, Photoshop-compatible) and a theme system with light, dark, and a Pixhaus default dark theme. This tutorial walks through remapping a shortcut and switching themes.

## Keybinds

<Steps>
1. **Open the keybind editor.** `Edit > Keybinds` or `Ctrl+Shift+K`. The editor lists every command with its current binding.

2. **Load a preset.** At the top of the keybind editor, click the `Preset` dropdown. Three presets are available:
   - **Pixhaus default** — the out-of-box layout
   - **Aseprite** — mirrors Aseprite's keyboard defaults so you can switch between editors without re-learning shortcuts
   - **Photoshop** — mirrors Photoshop's defaults for artists coming from that workflow

   Select one and click **Apply**. The binding list updates immediately. The change is not permanent until you click **Save**.

3. **Search for a command.** Type `pencil` in the search box at the top of the keybind list. The list filters to show every command that matches.

4. **Change a binding.** Click the row for **Tool: Pencil**. The current binding (`P`) is highlighted. Click the binding field and press the new key combination. For example, press `B` to mirror Photoshop's brush shortcut.

   If the new combination is already used by another command, a conflict warning appears below. You can proceed (the old command loses its binding) or pick a different key.

5. **Save.** Click **Save** to commit all changes. Click **Reset to preset** to undo unsaved changes and return to the loaded preset.

6. **Export your keybinds.** Click **Export** to save your current bindings as a `.json` file you can share or version-control. **Import** loads a `.json` file — useful for moving settings between machines.
</Steps>

<Aside>
The command palette (`Ctrl+K` / `Cmd+K`) lists every command alongside its binding. If you forget a shortcut, open the palette and type the command name.
</Aside>

## Per-tool modifier shortcuts

Some tools accept modifier keys during use that are not listed in the keybind editor:

| Tool | Modifier | Effect |
|---|---|---|
| Pencil | `Shift+click` | Draw a straight line from last point |
| Pencil | `Alt` | Switch to eyedropper temporarily |
| Rectangle | `Shift` | Constrain to square |
| Ellipse | `Shift` | Constrain to circle |
| Selection | `Shift` | Add to selection |
| Selection | `Alt` | Subtract from selection |
| Move | `Shift` | Constrain move to horizontal/vertical |

These modifier behaviors are fixed and not remappable in the current release.

## Themes

<Steps>
1. **Open preferences.** `Edit > Preferences` or `Ctrl+,`. Navigate to the **Appearance** tab.

2. **Switch theme.** The `Theme` dropdown has three options:
   - **Pixhaus dark** — the default dark theme with the Pixhaus brand accent color
   - **Dark** — a neutral dark theme with no brand accent
   - **Light** — a light theme for high-ambient-light environments

   Select a theme. The editor re-skins immediately without restart.

3. **Adjust the accent color.** Below the theme dropdown, the **Accent color** picker lets you override the active theme's accent color. This affects button highlights, selection borders, and the active-tool indicator.

4. **Adjust panel scaling.** The **UI scale** slider controls the overall UI density. At 100% the default spacing is used; at 125% or 150% everything is larger — useful on high-DPI screens where OS scaling feels insufficient.
</Steps>

## Custom themes

Themes are CSS custom property sets stored in `~/.pixhaus/themes/`. To create a custom theme:

1. Copy an existing theme file from the Pixhaus installation directory (find it via `Help > Show data folder`) into `~/.pixhaus/themes/`.
2. Edit the CSS custom properties in the file. Property names follow the pattern `--ph-<element>-<state>-<property>` (e.g., `--ph-panel-bg`, `--ph-button-hover-bg`).
3. Restart Pixhaus. The new theme appears in the `Theme` dropdown.

The theme format is documented in [reference/keybinds](/reference/keybinds/).

## Next steps

- [Write your first Lua script](/getting-started/first-lua-script/)
- See the full [keybinds reference](/reference/keybinds/)
