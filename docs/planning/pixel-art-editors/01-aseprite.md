# Aseprite

## Quick facts
- Vendor / maintainer: David Capello / Igara Studio Ltda
- License / pricing model: Proprietary EULA (source code available for personal use)
- Price point (current): $19.99 USD (one-time purchase)
- Platforms: Windows, macOS, Linux
- First released: June 2001
- Last meaningful update: v1.3.16 (December 2025)
- Source available: Yes (compiled from source for personal use only)
- Primary use case: Professional sprite animation and pixel art creation

## Origin and purpose

Aseprite began in 1998 as a hobby project by Chilean programmer David Capello, initially called the Allegro Sprite Editor. The first public release (v0.0.1) appeared in June 2001 under the GNU General Public License v2, using the Allegro library in C. Around 2006, the codebase shifted to C++ for improved maintainability. A stable v1.0 release came on June 6, 2014. 

In August 2016, Aseprite transitioned from open-source GPLv2 to a proprietary EULA, while keeping source code publicly available on GitHub for personal compilation and use (though redistribution of compiled binaries is prohibited). Steam release followed on February 22, 2016. This licensing shift prompted creation of the LibreSprite fork, maintaining the last GPLv2 version independently.

## Drawing and painting tools

Aseprite includes a brush engine with customizable brushes, pencil tool, eraser, line tool, rectangle and ellipse tools, and filled shape tools. The palette-based workflow makes it particularly suited to indexed color mode sprites, though RGB and grayscale modes are supported. Brush dynamics and brush sizes are fully adjustable. Magic wand selection, rectangular selection, and freehand selection tools allow isolating regions for editing.

## Pixel-specific features

The editor enforces pixel-perfect alignment and provides grid overlay with configurable spacing. Tiled drawing mode permits seamless pattern and texture creation. Symmetry tools enable horizontal and vertical mirroring during drawing, reducing work on symmetrical designs. Rotation and flipping commands apply to selections or entire layers.

## Color and palette workflow

Aseprite emphasizes palette management. Users can select foreground and background colors from custom palettes, build palettes from imported images, or use pre-loaded palettes. Indexed color mode reduces file size and is essential for Game Boy and NES-era sprite work. The foreground/background color swapping (X key) is a standard workflow accelerator. Palette-based animation is supported, where changing colors in the palette affects all frames using those indices.

## Layer system

Layers support transparency (alpha channel in RGB/grayscale, transparent color index in indexed mode). Layer groups organize complex sprites. The timeline displays layers vertically and frames horizontally, with each frame/layer intersection called a cel. Blend modes control how overlapping layers combine (multiply, screen, overlay, etc.), though layer group opacity and blend modes are not currently supported. Layer opacity ranges from 0 (transparent) to 255 (opaque) and can be set via Lua scripting.

## Animation features

The timeline is the core animation interface. Keyframe animation is frame-based rather than skeletal. Onion skinning displays adjacent frames semi-transparently (previous frames in red, next frames in blue by default) with customizable opacity and range. This aids motion continuity. Frame durations are editable per-frame (in milliseconds). Animation playback is real-time within the editor. Aseprite exports animations as GIF files, PNG sprite sheets, PNG frame sequences, or sprite sheet JSON metadata for game engines. The playback preview shows loop behavior and timing.

## Export and import

Aseprite can import and export sprite sheets (with XML or JSON metadata), animated GIFs, PNG sequences, and PSD files (limited compatibility). Native Aseprite format (.ase) preserves all layers, frames, and animation data. Texture atlas export formats include JSON and binary variants. The "Sprite Bounds" export option crops transparent borders. Export dialogs allow batch processing and format selection per project. GIF export includes color table and dither options.

## Scripting and extensibility

Lua scripting provides programmatic access to the editor via the `app` variable. Scripts can read/write sprite data, manipulate layers, create and modify frames, and access the active palette. The API allows querying layer properties, cel contents, and animation state. Custom tool creation is not directly supported; scripting focuses on automation and data manipulation rather than UI extension. The scripting documentation at aseprite.org/api provides detailed API references. Community scripts for common workflows (color reduction, sprite sheet generation, batch export) are available.

## Engine integration

Aseprite is editor-only; it does not directly integrate with game engines, but exports sprite sheets compatible with Unity, Unreal, Godot, and custom engines. JSON export includes frame timing, collider data (if slices are used), and cel positioning. Texture atlas export reduces drawcalls by packing multiple sprites into single textures. No built-in plugin system for engine bridges exists, though community tools automate reimport workflows.

## Workflow strengths

Aseprite's timeline/animation tools are industry-standard for sprite animation. Onion skinning is responsive and configurable. Palette-based workflows are efficient for retro game art (NES, SNES, Sega). Indexed color mode with palette swaps enables color cycling animations. Real-time playback preview speeds iteration. Frame tag support organizes multi-part animations (walk, run, idle) within a single file. Quick export to multiple formats. Keyboard shortcuts and hotkey remapping support rapid workflows. One-time purchase model eliminates subscription friction.

## Workflow gaps

Layer group opacity/blend modes are unsupported, requiring workarounds for complex layer hierarchies. No built-in tilemap editor (though community tools exist). Text tool is limited; typography-heavy projects redirect to other software. 3D layer support is absent; voxel and isometric workflows route to specialized tools. No non-destructive effects (adjustment layers, smart objects). Scripting cannot create custom UI panels or tools. Animation preview at exact frame rate is approximate, not frame-accurate. No collaboration features (multiplayer editing).

## Notable uses

Aseprite is the de facto indie game standard. Hollow Knight, Blasphemous, Celeste, and Salt and Sanctuary were created or heavily iterated in Aseprite. Professional studios at Yacht Club Games, Wayforward, and HalfBrick use it. 2D pixel animation in modern games like Dead Cells and Risk of Rain 2 relied on Aseprite. The tool is cited in postmortems across indie game development.

## Community and ecosystem

The official community forum at community.aseprite.org hosts user discussions, feature requests, and shared scripts. GitHub issues and release discussions track development. Itch.io hosts hundreds of Aseprite-created games. Educational content on YouTube covers animation, palette workflow, and scripting. Third-party tools automate sprite sheet parsing and game engine integration. Social media pixel art communities (Twitter, Reddit r/pixelart) feature Aseprite artwork. The one-time purchase model has built strong user loyalty.

## Pricing details

Aseprite costs $19.99 USD for Windows, macOS, and Linux via the official store, Steam, Humble Bundle, and itch.io. Occasional sales (historical low $9.99) appear during platform promotions. A freeware trial version exists with save functionality disabled. Purchasing grants access to the compiled binary and source code for personal compilation. No subscription; no per-seat licensing; no commercial use restrictions on created artwork. Educational discounts are not officially listed.
