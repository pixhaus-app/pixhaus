# Aseprite round-trip fixtures

Binary `.aseprite` fixtures for testing Pixhaus read/write compatibility.
These files cover the feature matrix defined in `docs/aseprite-compat.md`.

The S08 stream is responsible for generating these files and implementing the
round-trip test suite that reads each one, passes it through Pixhaus, and
verifies the output.

## Fixture inventory

| File | Color mode | Features exercised |
|---|---|---|
| `raster-rgba.aseprite` | RGBA | Raster layers, group layers, blend modes (all 19), per-layer opacity, frame tags (all 4 loop directions), palette, slices (bounds + nine-slice + pivot), user data (text + color) |
| `raster-indexed.aseprite` | Indexed | Indexed palette (256 colors with names), transparent color index, linked cels |
| `raster-grayscale.aseprite` | Grayscale | Grayscale color mode, multi-frame, per-frame durations |
| `tilemap-inline.aseprite` | RGBA | Tilemap layer, tileset (inline, with base index 1), tile flip flags (X, Y, diagonal), tilemap cel |
| `linked-cels.aseprite` | RGBA | Linked cel type (reused frames across layers), multi-frame animation |
| `slices-ninepatch.aseprite` | RGBA | Nine-slice center rects, pivot points, per-frame slice keys |
| `userdata-full.aseprite` | RGBA | User data on sprite, layers, cels, tags, slices, tileset (text + color on all) |
| `gap-icc-profile.aseprite` | RGBA | ICC color profile chunk present — verifies warning emitted, file still loads |
| `gap-zindex.aseprite` | RGBA | Non-zero z-index on cels — verifies warning emitted, cels load at default order |
| `gap-pixel-ratio.aseprite` | RGBA | Non-1:1 pixel ratio in header — verifies warning emitted |

## Round-trip test expectations

For each fixture, the test suite does the following:

1. Load the fixture with `io::aseprite::read(path)`.
2. Convert to the Pixhaus project data model.
3. Write back to a temporary `.aseprite` file with `io::aseprite::write(project, temp_path)`.
4. Load the written file with `io::aseprite::read(temp_path)`.
5. Assert that the reloaded project matches the original on all supported fields.

Fields that are intentionally dropped (ICC profile, z-index, user data properties)
must not cause assertion failures; the loss is expected and documented.

## Adding new fixtures

Generate fixtures using Aseprite's Lua scripting API (`app.command.SaveFile`).
All `.aseprite` files in this directory are committed as binary blobs — they are
test inputs, not generated artifacts. Keep each fixture minimal: include only the
features listed for that file. Large pixel art is not needed; a 16×16 sprite with a
few frames is sufficient.
