# Unity handoff reference exports

Reference output files for the Unity handoff format. The full spec is in
`docs/unity-handoff.md`.

These files are used as test fixtures by the exporter (S10) and the Unity
importer package (S39). If the spec changes and a format version bump
occurs, update these files to match.

## Contents

```
simple-sprite/
  hero.json     sprite sheet metadata — 16×16 hero, 4 frames, idle + walk tags
  hero.png      synthetic placeholder PNG (64×16 RGBA checkerboard)

tilemap/
  dungeon.tmx   Tiled 1.10 map — 8×8 tilemap, 2 layers, flip flags demonstrated
  dungeon.tsx   tileset definition — 6 tiles, 16×16 each
  dungeon.png   synthetic placeholder PNG (96×16 RGBA color-coded tiles)

generate-pngs.mjs   regenerates the placeholder PNGs from scratch
```

## Regenerating the PNGs

The PNG files are synthetic placeholders. To recreate them:

```
node generate-pngs.mjs
```

Requires Node.js 18+. No external dependencies.

## What the examples cover

`simple-sprite/hero.json` demonstrates:
- Two animation tags (`idle` frames 0–0, `walk` frames 1–3) with `forward` direction
- Two layers with different blend modes (`multiply`, `normal`)
- A nine-slice (`head` slice with `center` field)
- A pivot (`root` slice with `pivot` field)

`tilemap/dungeon.tmx` demonstrates:
- Two tilemap layers (`ground`, `decoration`) with the same tileset
- Regular tile placement (values 1–3 in the ground layer)
- Empty cells (value `0`)
- Flip X — `2147483652` (tile 4 mirrored horizontally)
- Flip Y — `1073741828` (tile 4 mirrored vertically)
- Flip X + Y — `3221225476` (tile 4 mirrored on both axes)
