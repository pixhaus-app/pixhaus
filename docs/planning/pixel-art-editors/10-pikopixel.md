# PikoPixel

## Quick facts
- Vendor / maintainer: Original developer (open-source community ports)
- License / pricing model: Free (closed-source original; open-source Godot ports emerging)
- Price point (current): Free
- Platforms: macOS (original, Intel and Apple Silicon); Linux, BSD via GNUstep framework
- First released: 1990s (original development); beta status maintained (v1.0 Beta 10)
- Last meaningful update: Unknown (active maintenance, community ports in progress)
- Source available: No (original closed-source); community ports may have source
- Primary use case: Free, lightweight macOS pixel art editor for simple sprite and icon creation

## Origin and purpose

PikoPixel is a free, lightweight macOS pixel editor created as a desktop native application. Built on Objective-C and Cocoa frameworks, it emphasizes simplicity and performance. The tool serves macOS and Unix-like systems (Linux, BSD) through GNUstep framework ports, avoiding bloat and system resource overhead. PikoPixel targets pixel artists seeking a straightforward, no-nonsense pixel editor without feature overload. The tool prioritizes core pixel art functionality: drawing, layers, unlimited undo, and export. It competes with GraphicsGale (Windows-only) by providing a lightweight alternative for macOS users.

## Drawing and painting tools

Pencil, eraser, brush, line, rectangle, and oval drawing tools. Rectangular and freehand selection tools. Magic wand (color-select) selection. Magnifier tool for zoom. Move tool for repositioning selections. Color sampler (eyedropper). Filled shapes (filled rectangle, filled oval). The toolkit is minimal but complete for typical pixel art work. No advanced brush dynamics or custom brushes; emphasis is on simplicity.

## Pixel-specific features

Grid overlay with customizable spacing. Pixel-perfect line drawing. Zoom levels from close work to spritesheet overview. Tiled drawing mode for seamless pattern creation. Rotation and flip transformations on selections. Canvas background customization (transparent, checkboard, solid color). Grid color and opacity customization. Guide and ruler tools for alignment. Symmetric drawing is not mentioned; single-axis mirroring only.

## Color and palette workflow

Foreground and background color selection with swatch. Color picker. Eyedropper tool. RGB color mode. Color history (unclear depth). Custom palette support is minimal; emphasis is on direct RGB color picking. No palette animation or indexed color mode for retro game workflows.

## Layer system

Multiple layers with transparency. Layer visibility toggling and locking. Layer merging and flattening. Layer opacity control. Layers are organized vertically in the interface. Selection of active layer for editing. Layer organization for sprite composition. The layer system is straightforward without advanced features (no groups, no blend modes).

## Animation features

Unknown. No frame-based timeline or animation playback mentioned in available documentation. The tool is primarily for static sprite and icon creation rather than animation workflows. Multiple frames are not standard features.

## Export and import

PNG export with transparency. BMP export. Scaled image export (upscaling pixel art without anti-aliasing for print or larger display). Export upscaled images feature allows 2x, 3x, or custom scale factors while preserving pixel edges. PNG and image import for populating the canvas. Clipboard import/export for rapid sharing. The export focus is on PNG for web and scaled versions for print.

## Scripting and extensibility

No scripting or plugin system. The tool is closed-source proprietary with no extensibility mechanism. Customization relies on future developer updates.

## Engine integration

PNG export is compatible with game engines via standard formats. No native engine integrations or plugins. Simple sprite and icon export suits web and game frameworks.

## Workflow strengths

Lightweight and responsive on macOS hardware. Unlimited undo provides safety net for experimentation. Simple, uncluttered interface suits beginners and quick mockups. Minimal system resource overhead compared to bloated alternatives. Free and open for personal use. Gamma-correct color blending ensures accurate color representation. GNUstep ports extend availability to Linux and BSD. Python scripting support (unclear if this applies to main PikoPixel or a variant). Direct RGB color workflow without palette constraints. Export upscaling feature aids asset generation for multiple resolutions.

## Workflow gaps

Limited animation features; not suitable for frame-by-frame sprite animation (use GraphicsGale or Aseprite instead). No layers blend modes or advanced compositing. No text tool. No symmetry or mirror drawing (single-axis mirroring only). No palette management or indexed color workflows for retro game art. Closed-source limits extensibility. No scripting or automation. Documentation is sparse; tutorials are minimal. Small community with few third-party resources. macOS-centric; Windows users must use alternatives. No tilemap or level editor. Performance on very large canvases unknown. Inactive recent development (beta status maintained indefinitely); no roadmap clarity.

## Notable uses

macOS pixel artists use PikoPixel for quick sprite sketches and icon creation. macOS educators teaching pixel art benefit from the free, lightweight tool. GNOME and Linux desktop community members use GNUstep ports for retro pixel art. Game icon creation and favicon design workflows benefit from PikoPixel's simplicity. Hobbyists creating 1-bit and simple pixel art enjoy the minimalist feature set. Developers seeking a lightweight embedded pixel editor for desktop applications may fork PikoPixel.

## Community and ecosystem

Minimal active community. Official website (twilightedge.com/mac/pikopixel) provides downloads and minimal documentation. User guide is brief. macOS App Store availability (if current) aids discoverability. No official forums or chat communities documented. GitHub forks exist for open-source ports (e.g., PikoPixelM1 for Apple Silicon modernization). Package manager availability (Ubuntu, Debian) aids Linux adoption via GNUstep. No YouTube tutorials or educational content documented. Reddit communities (r/pixelart, r/retrogaming) may reference PikoPixel in comparison lists.

## Pricing details

PikoPixel is completely free. No payment, license key, or subscription required. Binaries for macOS are available from the official website (twilightedge.com) and macOS App Store (if listed). Linux and BSD versions available through package managers (Debian, Ubuntu) and GNUstep distributions. Community-maintained forks (PikoPixelM1) available on GitHub for free. No commercial restrictions on created artwork. No premium features or tiers.
