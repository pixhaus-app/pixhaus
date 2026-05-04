# Pixhaus Unity package

First-party Unity importer and runtime helpers for Pixhaus exports.

Minimum Unity version: **2022.3 LTS**. Primary target: **Unity 6**.

## Install

Via OpenUPM (once published):

```bash
openupm add app.pixhaus.unity
```

Via Git URL — add to `Packages/manifest.json`:

```json
"app.pixhaus.unity": "https://github.com/pixhaus-app/pixhaus.git?path=/unity"
```

## What's included

### Sprite sheet import

Drop a `.pixhaussprite` file (your Pixhaus JSON export renamed from `.json`) and its
co-located PNG into the Assets folder. Unity automatically uses `PixhausSpriteImporter`,
which produces:

- **Texture2D** (main asset) — the packed sprite sheet
- **Sprite** per frame — sub-assets, sliced from the sheet with pivots from the `root`
  slice if present, otherwise bottom-center
- **AnimationClip** per frame tag — sub-assets for use with Unity's Animator

Import settings (Inspector on the `.pixhaussprite` file):

| Setting | Default | Notes |
|---|---|---|
| Pixels Per Unit | 16 | Match your project's PPU setting |
| Mesh Type | FullRect | FullRect is safe for pixel art |
| Filter Mode | Point | Keeps pixels sharp |
| Generate Mip Maps | false | Rarely needed for pixel art |

### Tilemap import

Drop a `.tmx` file and its co-located TSX and tileset PNG files into the Assets folder.
Unity uses `TmxImporter`, which produces a **Grid prefab** (main asset) with child
Tilemap GameObjects and Tile sub-assets. Drag the prefab into a scene.

Supported: CSV encoding, all eight flip/rotate flag combinations, multiple tilesets per
map, multiple layers.

### Runtime animator

`PixhausAnimator` is a MonoBehaviour for scripted animation without a Unity Animator:

```csharp
using Pixhaus.Runtime;
using UnityEngine;

[RequireComponent(typeof(PixhausAnimator))]
public class HeroController : MonoBehaviour
{
    private PixhausAnimator anim;

    private void Awake() => anim = GetComponent<PixhausAnimator>();

    private void Update()
    {
        bool moving = Input.GetAxisRaw("Horizontal") != 0;
        anim.Play(moving ? "walk" : "idle");
    }
}
```

Populate the `tags` array from the Sprite sub-assets in the Inspector, or assign it from
code. For blend trees and full Animator integration, use the AnimationClip sub-assets
directly with a standard AnimatorController.

## Samples

Install the **Sprite sheet import** sample via the Package Manager to get a working
`.pixhaussprite` file and setup instructions.

## Layout

| Path | Contents |
|---|---|
| `Editor/` | Importers and editor tooling (editor-only) |
| `Runtime/` | PixhausAnimator and supporting types (included in player builds) |
| `Samples~/SpriteSheetImport/` | Sample sprite sheet and README |
