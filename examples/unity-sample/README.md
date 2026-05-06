# Pixhaus Unity sample project

A minimal Unity 6 project demonstrating the Pixhaus → Unity import pipeline
end-to-end: sprite sheet import, tilemap import, scripted animation playback,
and player movement.

The art is synthetic placeholder data (solid-color tiles). The structure —
frame layout, animation tags, pivot slices, tilemap layers — is the real
deliverable. Drop in final art from `examples/samples/` once the Pixhaus
editor is functional.

## Requirements

- Unity 6 (6000.0.x)
- Git (for the `app.pixhaus.unity` package reference)
- Node.js 18+ (only if regenerating the placeholder PNGs)

## Opening the project

1. Clone the Pixhaus repository.
2. Open Unity Hub → Add → Add project from disk → select
   `examples/unity-sample/`.
3. Unity will:
   - Download the `app.pixhaus.unity` package via the git URL in
     `Packages/manifest.json`.
   - Run `PixhausSpriteImporter` on `knight.pixhaussprite` and
     `slime.pixhaussprite`, producing Texture2D, Sprite, and AnimationClip
     sub-assets for each.
   - Run `TmxImporter` on `forest.tmx`, producing a Grid prefab with two
     Tilemap children (`ground` and `decoration`) and Tile sub-assets.
4. Open `Assets/Scenes/SampleScene.unity`.

## What the importer produces

### From `knight.pixhaussprite`

| Asset | Description |
|---|---|
| Texture2D (`knight`) | The packed 256×160 sprite sheet |
| Sprite per frame (`knight 0` … `knight 32`) | Sub-assets, sliced with the `root` pivot at bottom-center |
| AnimationClip per tag | `idle`, `walk`, `run`, `attack`, `hurt`, `death` |

### From `slime.pixhaussprite`

| Asset | Description |
|---|---|
| Texture2D (`slime`) | The packed 112×48 sprite sheet |
| Sprite per frame (`slime 0` … `slime 20`) | Sub-assets |
| AnimationClip per tag | `idle`, `hop`, `hit`, `split` |

### From `forest.tmx`

| Asset | Description |
|---|---|
| Grid prefab (`forest`) | Main asset — drag into a scene to place the level |
| Tilemap `ground` | Terrain layer (grass, dirt, stone, water, trees) |
| Tilemap `decoration` | Sparse items (chests, flowers) on top of terrain |
| Tile sub-assets | One `UnityEngine.Tilemaps.Tile` per unique GID |

## Wiring the scene

The scene ships with a `Player` and an `Enemy` GameObject already configured
with `PlayerController` and `EnemyPatrol` scripts, SpriteRenderers, and
Rigidbody2D colliders. After Unity imports the assets, wire them up:

### Player

1. Select the **Player** GameObject.
2. In the Inspector, click **Add Component** → search for **Pixhaus Animator**
   → add it.
3. Set **Default Tag** to `idle`.
4. Expand the **Tags** array to match the tags in `knight.pixhaussprite`:

   | Index | Name | Sprites | Durations | Wrap Mode |
   |---|---|---|---|---|
   | 0 | idle | knight 0–3 | 0.15 s each | Loop |
   | 1 | walk | knight 4–11 | 0.1 s each | Loop |
   | 2 | run | knight 12–17 | 0.08 s each | Loop |
   | 3 | attack | knight 18–23 | 0.08 s each | Once |
   | 4 | hurt | knight 24–26 | 0.12 s each | Once |
   | 5 | death | knight 27–32 | 0.12 s each | Once |

   For each tag, set the **Sprites** array to the corresponding `knight N`
   sub-assets from the `knight.pixhaussprite` import (visible in the Project
   window when you expand the asset).

5. Drag the imported `knight.pixhaussprite`'s first sub-asset (`knight 0`)
   onto the SpriteRenderer's **Sprite** field as the default display.

### Enemy

1. Select the **Enemy** GameObject.
2. Add **Pixhaus Animator** component.
3. Set **Default Tag** to `hop`.
4. Add one tag entry:

   | Index | Name | Sprites | Durations | Wrap Mode |
   |---|---|---|---|---|
   | 0 | hop | slime 4–9 | 0.1 s each | Loop |
   | 1 | hit | slime 10–12 | 0.12 s each | Once |

5. Drag `slime 0` onto the SpriteRenderer **Sprite** field.

### Tilemap

1. In the Project window, locate the `forest` Grid prefab produced by the
   TMX import.
2. Drag it into the scene. The two tilemap layers (`ground`, `decoration`)
   appear as children.
3. Set the **Order in Layer** on each TilemapRenderer if needed (ground = 0,
   decoration = 1).
4. Position the Grid at `(-16, -8, 1)` so the 32×16 map is centered
   behind the player and enemy (each tile = 1 unit at 16 PPU).

## Controls

| Key | Action |
|---|---|
| WASD / arrow keys | Move player |
| Left Shift + WASD | Run |
| Left Ctrl / left mouse | Attack |

## File layout

```
Assets/
  Pixhaus/
    knight.pixhaussprite   sprite sheet JSON — 33 frames, 32x32, 6 animation tags
    knight.png             256x160 placeholder sprite sheet
    slime.pixhaussprite    sprite sheet JSON — 21 frames, 16x16, 4 animation tags
    slime.png              112x48 placeholder sprite sheet
    forest.tmx             tilemap — 32x16 tiles, 2 layers
    forest.tsx             tileset definition — 17 tiles, 16x16 each
    tileset.png            272x16 placeholder tileset strip
  Scripts/
    PlayerController.cs    WASD movement + PixhausAnimator integration
    EnemyPatrol.cs         left-right patrol + PixhausAnimator integration
  Scenes/
    SampleScene.unity      pre-configured scene with Player and Enemy

generate-assets.mjs        regenerates placeholder PNGs from scratch
Packages/manifest.json     references app.pixhaus.unity via git URL
```

## Regenerating the placeholder PNGs

The PNG files are synthetic and committed so the project works on a fresh
clone. To regenerate them (e.g. after changing tile counts or dimensions):

```
node generate-assets.mjs
```

## What this project does not demonstrate

- **Animated tiles** — the current `TmxImporter` produces static
  `UnityEngine.Tilemaps.Tile` assets. The water animation defined in
  `forest.tsx` is preserved for future importer support; Unity shows water
  as a single static tile for now.
- **8-direction character** — the knight uses a single facing direction to
  keep the sample focused on the import pipeline. The full 8-direction sprite
  sheet lives in `examples/samples/character-knight.pixhaus`.
- **Animator controller** — the `PixhausAnimator` component drives animation
  without a Unity Animator. For blend trees or complex state machines, use
  the AnimationClip sub-assets directly in a standard AnimatorController.
- **Physics-based collision** — the BoxCollider2D on each character is sized
  to the sprite frame, not a pixel-accurate hitbox.

## License

The pixel data and project structure in this directory are released under
CC0 1.0 Universal.
