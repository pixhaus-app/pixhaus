# Pixhaus sample projects

Five `.pixhaus` project files covering the asset types described in the S45
stream brief. They serve three purposes:

1. **Tutorial starting points** — open any file in Pixhaus and start painting
   over the placeholder art.
2. **Test fixtures** — consumed by integration tests that verify the editor
   loads, renders, and exports them without errors.
3. **Marketing demos** — referenced by `examples/unity-sample/` and the
   documentation site.

## Files

| File | Canvas | Frames | Notes |
|---|---|---|---|
| `character-knight.pixhaus` | 32×32 | 167 | Indexed, 16-color palette. Idle/walk/run/attack/hurt/death, 8 directions where applicable. |
| `tileset-forest.pixhaus` | 16×272 | 3 | RGBA. 17 tiles (grass, dirt, stone, water). Animated water via `TileAnimation`. |
| `enemy-slime.pixhaus` | 16×16 | 21 | Indexed, 10-color palette. Idle/hop/hit/split. |
| `ui-sprites.pixhaus` | 96×72 | 1 | RGBA. Health bar, mana bar, button states, dialogue box. Nine-slice data on all elements. |
| `level-forest.pixhaus` | 512×256 | 1 | RGBA tilemap. 32×16 tile grid using the inline forest tileset. |

## Placeholder art

Pixel data is procedurally generated. Each frame is a solid-filled rectangle
with a 1px border and a progress bar indicating frame position within the
animation. The structure — layer stack, animation tags, palette discipline,
slice metadata — is the deliverable; replace the pixel data with final art
when the editor is ready.

## Palette discipline

- `character-knight.pixhaus`: 16-color RPG palette (`knight`). Transparent
  index 0.
- `enemy-slime.pixhaus`: 10-color slime palette (`slime`). Transparent
  index 0.
- UI and tileset files use RGBA mode with no indexed palette.

## Regenerating

If the `.pixhaus` wire format changes (a breaking change to B3), regenerate
the files with:

```
PIXHAUS_REGEN_SAMPLES=1 cargo nextest run -p pixhaus-io \
    --test generate_sample_projects
```

The generator lives at `io/tests/generate_sample_projects.rs`. Commit the
resulting binary files.

## License

The pixel data and project structure in this directory are released under
CC0 1.0 Universal. You may use, modify, and distribute them without
restriction, including for commercial purposes.

See `LICENSE` in this directory.
