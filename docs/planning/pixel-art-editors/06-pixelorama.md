# Pixelorama

## Quick facts
- Vendor / maintainer: Orama Interactive (community open-source project)
- License / pricing model: MIT License (open-source)
- Price point (current): Free
- Platforms: Windows, Linux, macOS, Web
- First released: Unknown (active development, multiple platforms)
- Last meaningful update: v1.1.10 (2024); v1.2 in beta
- Source available: Yes (GitHub: Orama-Interactive/Pixelorama, fully open-source)
- Primary use case: Open-source pixel art multitool with advanced animation and tilemap support

## Origin and purpose

Pixelorama is an open-source pixel art editor built entirely with the Godot Engine, showcasing Godot's versatility beyond game development. Developed by Orama Interactive with community contributions, the project emphasizes accessibility, feature richness, and cross-platform availability. MIT licensing guarantees free updates forever. The choice to build with Godot creates a showcase project for the engine and demonstrates pixel art tool capabilities in a modern, open-source framework. Active development adds features quarterly, positioning Pixelorama as a competitive free alternative to Aseprite.

## Drawing and painting tools

Pencil, eraser, brush, line, rectangle, and ellipse tools. Customizable brush sizes, opacity, and hardness. Custom brush creation and library management. Anti-aliasing toggle for pixel-perfect edges. Fill tool with color replacement. Selection tools (rectangular, freehand, magic wand, color range select). Stroke and fill selections. Color picker (eyedropper). Line tool with width adjustment. Shape tools (polygon, bucket fill with pattern). The drawing toolkit is modern and responsive, rivaling Aseprite's feature set.

## Pixel-specific features

Grid overlay with configurable spacing, color, and snapping. Zoom levels from close work to overview. Ruler and guide systems for alignment. Tiled drawing mode for seamless pattern creation. Rotation and flip commands on selections and layers. Symmetry tools (horizontal, vertical, and diagonal) reduce work on symmetrical designs. Mirror drawing mode. Canvas size and composition tools. Perspective tool for isometric and 3D-inspired artwork. Measurement and alignment guides.

## Color and palette workflow

Foreground and background color selection with swatch management. Color history tracks recent colors. Custom palette creation and import from images. RGB color picker with HSV and other color space options. Color range selection. Eyedropper with average color option. Palette editing and rearrangement. Indexed color mode support for retro game workflows. Gradient fills and pattern fills. Color exchange (batch color replacement). Palette pinning for quick access (feature added in v1.3.15+).

## Layer system

Multiple layers with transparency and blending. Layer groups for complex hierarchies. Layer visibility toggling and locking. Layer opacity control from 0 to 255. Blend modes (multiply, screen, overlay, add, subtract, darken, lighten, color dodge, color burn, etc.). Non-destructive layer effects (outline, gradient map, drop shadow, palettize) with individual toggles and customization. 3D layers enable 3D shapes and models embedded in 2D canvas. Layer renaming and organization. Merging and flattening operations. Layer duplication. Layer effects are fully customizable and can be stacked.

## Animation features

Timeline composed of frames and layers. Frame-based animation with per-frame duration (in milliseconds). Real-time animation playback preview responsive and adjustable FPS. Onion skinning with customizable range, colors (previous frames in red, next frames in blue by default), and opacity. Frame tags organize multi-part animations (walk, run, idle) and enable independent export per tag. Audio synchronization for lip-sync and rhythm animation. Draw while animation plays option for live animation feedback. Cel editing with independent control per frame/layer. Animation preview includes loop, ping-pong, and playback speed settings. Frame duplication and shifting. Animation timeline is responsive and intuitive.

## Export and import

Animated PNG (APNG) export preserves animation in standard PNG format. GIF export for web sharing. PNG sprite sheet export with customizable rows/columns and options for texture filtering. Video export (MP4, WebM) for animation preview and documentation. PNG sequences for frame-by-frame export. Aseprite (.ase) import/export for Aseprite project compatibility. Image import (PNG, JPG, BMP) to populate frames. Sprite sheet import with automatic tile detection. Batch export with per-frame-tag organization (creates separate files per tag). Frame data export as JSON (coordinates, durations, tags).

## Scripting and extensibility

Extension system based on GDScript (Godot's native language). Main.gd script runs when extension loads, providing entry point for custom functionality. ExtensionsApi exposes UI, canvas, and editing functionality to extensions. Extensions access all of Pixelorama's internals (tools, brushes, palettes, layer system, canvas). Custom tool creation by attaching scripts to tool objects. Built-in updaters for tool system. Extensions exported as PCK files (Zip files also recognized, PCK preferred). Community-made extensions available (e.g., 2D-to-3D voxel conversion tools). Extension manager in Preferences > Extensions. Drag-and-drop installation of extensions. API documentation available on Pixelorama's website (pixelorama.org/extension_system/extension_api). Open-source codebase permits direct modification and contribution.

## Engine integration

Sprite sheet exports compatible with Unity, Unreal, Godot, Phaser, and custom engines. JSON export includes frame timing, coordinates, and animation tags. Tilemap layers export as data structures suitable for tile-based engines. PNG and video exports suit web preview and documentation. Godot integration is seamless (project built with Godot); developers using Godot can directly reference Pixelorama workflows. No native plugin system for other engines, but standard export formats enable manual integration.

## Workflow strengths

Comprehensive feature set rivaling Aseprite at zero cost. Advanced layer effects (non-destructive) surpass many competitors. Frame tagging and audio sync excel for game animation. Tilemap layers with collision support suit tile-based game art. 3D layers for hybrid 2D/3D workflows. Symmetric drawing tools (including diagonal) reduce repetitive work. Open-source permits community contributions and custom builds. Cross-platform (Windows, macOS, Linux, Web) supports diverse workflows. Active development with quarterly updates. MIT license guarantees free updates forever. Responsive timeline and real-time playback. Extension system enables customization without source modification. Social features (community extensions library).

## Workflow gaps

Smaller community than Aseprite means fewer tutorials and third-party tools. Documentation is improving but lags Aseprite's depth. Lua scripting unsupported (GDScript required for extensions). Performance on very large canvases may lag compared to native tools. Text tool is minimal (no typography-heavy workflows). Web version limitations (browser rendering constraints). Animation preview is approximate, not frame-accurate. No collaboration features (real-time multiplayer). Version history/undo depth limitations possible. Onion skinning color scheme is fixed (red/blue, not customizable).

## Notable uses

Indie game developers building 2D games (platformers, roguelikes, shmups) use Pixelorama for full sprite and tilemap creation. Game jam participants choose Pixelorama for free, feature-rich pixel art workflows. Educational institutions teaching game development benefit from open-source accessibility and Godot integration. Godot game developers use Pixelorama as the native pixel art complement to the engine. Pixel art communities appreciate the active development and GPL-friendly ethos. Aspiring pixel artists benefit from comprehensive feature set and tutorials.

## Community and ecosystem

Active open-source community. GitHub repository (Orama-Interactive/Pixelorama) hosts source code, issues, discussions, and pull requests. Official website (pixelorama.org) provides documentation, user manual, tutorials, and API references. Discord server connects developers and users. YouTube channel and tutorials cover features and workflows. Itch.io page links to games created with Pixelorama. Reddit communities (r/pixelart, r/godot, r/gamedev) reference Pixelorama. Steam release expands discoverability. Flathub, Snap, and package manager availability (Debian, Ubuntu) ease installation on Linux.

## Pricing details

Pixelorama is completely free. MIT License permits use, modification, and redistribution. No payment, subscription, or license key required. Binaries available for Windows, macOS, and Linux from GitHub releases, official website, Itch.io, Steam (free), Flathub, package managers (Debian, Ubuntu, Arch), and Snap. Web version accessible at pixelorama.org/pixel-art-software (or via web app). Source code available on GitHub for inspection and modification. Created artwork has no commercial restrictions. No premium tier or feature gating.
