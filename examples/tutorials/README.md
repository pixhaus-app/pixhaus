# Pixhaus tutorial starter files

Starter and finished `.pixhaus` project files for the getting-started tutorials.
Open a `-start.pixhaus` file in Pixhaus to follow along; open the matching
`-finished.pixhaus` to see the end state.

## Files

| File | Tutorial | Canvas | Frames | Notes |
|---|---|---|---|---|
| `walk-cycle-start.pixhaus` | [Use AI verbs to inbetween a walk cycle](/getting-started/ai-inbetween/) | 32×32 | 2 | Indexed, 16-color knight palette. Two key frames tagged `walk-key`. |
| `walk-cycle-finished.pixhaus` | [Use AI verbs to inbetween a walk cycle](/getting-started/ai-inbetween/) | 32×32 | 4 | Same palette. Four frames tagged `walk` with simulated inbetweens. |
| `export-unity-start.pixhaus` | [Export to Unity](/getting-started/export-unity/) | 32×32 | 18 | Indexed, 16-color knight palette. Tags: `idle` (4), `walk` (8), `attack` (6). |
| `lua-palette-start.pixhaus` | [Write your first Lua script](/getting-started/first-lua-script/) | 32×32 | 1 | Indexed, 16-color palette in arbitrary luminance order. |
| `lua-palette-finished.pixhaus` | [Write your first Lua script](/getting-started/first-lua-script/) | 32×32 | 1 | Same palette sorted by luminance (dark to light, index 0 preserved). |

The keybinds and themes tutorial works entirely within editor preferences — no
project file is needed.

## Placeholder art

Pixel data is procedurally generated. Each frame is a solid fill with a 1px
border and a progress bar. Structure (layer stack, animation tags, palette
discipline) is the deliverable; replace pixel data with final art when the
editor is ready.

## Regenerating

If the `.pixhaus` wire format changes (a breaking change to B3), regenerate with:

```
PIXHAUS_REGEN_TUTORIALS=1 cargo nextest run -p pixhaus-io \
    --test generate_tutorial_projects
```

The generator lives at `io/tests/generate_tutorial_projects.rs`.

## License

CC0 1.0 Universal. Use, modify, and distribute without restriction.

See `LICENSE` in this directory.
