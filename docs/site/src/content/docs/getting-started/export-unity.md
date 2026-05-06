---
title: Export to Unity
description: Pack a sprite sheet and import it into Unity with the Pixhaus importer package.
---

import { Steps, Aside, Tabs, TabItem } from "@astrojs/starlight/components";

This tutorial exports an animated character to Unity using the Pixhaus sprite sheet format and the Pixhaus Unity importer package. By the end you will have a GameObject in your Unity scene with an `AnimationClip` for each animation tag.

**Starter file:** `examples/tutorials/export-unity-start.pixhaus` — a 32×32 character with `idle` (4 frames), `walk` (8 frames), and `attack` (6 frames) animation tags.

## Before you start

The Pixhaus Unity importer must be installed in your Unity project. Install it via the Unity Package Manager:

1. Open **Window > Package Manager** in Unity.
2. Click the `+` button and choose **Add package by name**.
3. Enter `app.pixhaus.importer` and confirm.

Alternatively, add it via OpenUPM:

```bash
openupm add app.pixhaus.importer
```

The importer requires Unity 2022.3 LTS or later.

## Exporting from Pixhaus

<Steps>
1. **Open the starter file.** `File > Open` and navigate to `examples/tutorials/export-unity-start.pixhaus`.

2. **Check animation tags.** Open the timeline (`View > Timeline`). Three tags should be visible: `idle`, `walk`, `attack`. If any tags are missing, add them by selecting the relevant frames, right-clicking the frame header, and choosing `Tag selection`.

3. **Export a sprite sheet.** `File > Export > Sprite sheet (PNG)`.

   Settings to use:
   - **Layout:** Grid
   - **Cell size:** 32×32 (matches the sprite)
   - **Include frame tags:** enabled
   - **Include slices:** enabled (if you have pivot data)
   - **Output directory:** choose a folder inside your Unity project's `Assets/` directory (e.g., `Assets/Sprites/Knight/`)

   Pixhaus writes two files: `<sprite-name>.png` (the sheet) and `<sprite-name>.json` (the frame metadata).

4. **Confirm the output.** Open the output folder in your OS file manager. You should see `export-unity-start.png` and `export-unity-start.json`.
</Steps>

## Importing into Unity

<Steps>
1. **Switch to Unity.** Unity detects the new files and starts a reimport automatically.

2. **Check the Sprite asset.** In the Unity Project panel, click `export-unity-start.png`. The Inspector shows the Sprite asset. The importer has already set:
   - **Sprite mode:** Multiple (one sub-sprite per frame)
   - **Pixels per unit:** 32 (matching the sprite dimensions for a 1-unit-per-tile scale)
   - **Filter mode:** Point (correct for pixel art)
   - **Compression:** None

3. **Verify the slices.** Click **Sprite Editor** in the Inspector. Each frame should be sliced into a separate sub-sprite named `<tag>_<frame>` (e.g., `idle_0`, `idle_1`, `walk_0`).

4. **Check the AnimationClips.** In the Project panel, expand the `export-unity-start.png` asset. Below it you will see one `AnimationClip` per animation tag: `idle`, `walk`, `attack`. Each clip plays at the frame rate defined in the Pixhaus export — frame durations are preserved.

5. **Create an Animator Controller.** Right-click in the Project panel and choose `Create > Animator Controller`. Name it `KnightController`. Open it and drag the three clips into the Animator graph.

6. **Wire up transitions.** Add parameters (`IsWalking`, `IsAttacking`, etc.) and connect the state transitions. The specific logic depends on your game; the clips themselves are ready to use.

7. **Add the character to the scene.** Drag the `idle_0` sprite from the Project panel onto the scene to create a SpriteRenderer GameObject. Assign `KnightController` to its Animator component. Press Play — the idle animation plays.
</Steps>

<Aside>
If you move the `.pixhaus` source file after export, re-export using the same output path. The importer reads the JSON metadata, not the `.pixhaus` file directly, so moving the source does not break the Unity side — only re-exporting updates Unity's assets.
</Aside>

## Re-exporting after edits

When you update the animation in Pixhaus and re-export, Unity auto-detects the changed PNG and JSON and reimports. Existing `AnimationClip` references in your Animator Controller and scene are preserved — you do not need to rewire anything.

## Troubleshooting

**Sprites appear blurry.** The importer sets Filter Mode to Point by default. If you changed it, reset it in the Texture Import Settings Inspector.

**AnimationClips are missing.** Check that the animation tags in the JSON match the tags in the Pixhaus export. Tags with spaces or special characters may be sanitized — check the clip names in Unity against the tag names in Pixhaus.

**Pixels per unit is wrong.** Set the correct PPU in the Pixhaus importer settings: select the PNG, find the `Pixhaus Importer` section in the Inspector, and override `Pixels per unit`.

## Next steps

- [Customize keybinds and themes](/getting-started/customize-keybinds/)
- Read the full [export formats reference](/animation/export/)
- See the [Unity importer package documentation](https://github.com/pixhaus-app/pixhaus/tree/main/unity)
