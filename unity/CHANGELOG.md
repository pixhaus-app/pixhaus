# Pixhaus Unity package — changelog

All notable changes to the Pixhaus Unity package are documented here. The format
follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the package
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-05-04

### Added

- `PixhausSpriteImporter` — ScriptedImporter for `.pixhaussprite` files. Reads the
  Pixhaus sprite sheet JSON format (docs/unity-handoff.md), loads the co-located PNG,
  and produces a Texture2D (main asset), one Sprite per frame, and one AnimationClip
  per frame tag as sub-assets.
- `PixhausSpriteImporterEditor` — custom Inspector for sprite import settings (pixels
  per unit, filter mode, mip maps, mesh type).
- `TmxImporter` — ScriptedImporter for `.tmx` tilemap files. Reads Tiled 1.10-
  compatible TMX and co-located TSX tilesets; produces a Grid prefab with Tilemap
  layers and Tile sub-assets. Handles all eight flip/rotate flag combinations.
- `PixhausAnimationTag` — serializable runtime data class holding a frame sequence
  (sprites + per-frame durations) and wrap mode for one animation tag.
- `PixhausAnimator` — MonoBehaviour for scripted sprite playback without an Animator
  or AnimatorController. Supports Play, Stop, Pause, Resume, and an OnAnimationComplete
  event.
- Sprite sheet import sample in `Samples~/SpriteSheetImport/`.
