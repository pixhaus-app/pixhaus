# Krita

## Quick facts
- Vendor / maintainer: Krita Foundation (community-driven, backed by Krita Foundation)
- License / pricing model: Free and open source (LGPL 3.0)
- Price point (current): Free (donations accepted)
- Platforms: Windows, macOS, Linux, Android (via Play Store)
- First released: 2004 (as KImageShop in KOffice 1.4); first public version 1.4.0 on June 21, 2005
- Last meaningful update: Current stable series is 5.3.x; Krita 6.0 released in 2024
- Source available: Yes. Full source on GitHub (https://github.com/KDE/krita)
- Primary use case: Digital painting and illustration with strong animation support

## Origin and purpose

Krita originated in 1998 as a Qt GUI hack for GIMP shown by KDE founder Matthias Ettrich at Linux Kongress. The project was initially named KImageShop, then Krayon, before settling on Krita in 2002. Krita shipped with KOffice 1.4 in 2005 as a general image editor. In 2009, the project pivoted toward becoming a digital painting application comparable to Corel Painter and Manga Studio. The Krita Foundation was formally established to steward the project and provide long-term sustainability. The application is funded through donations, Patreon, and periodic crowdfunding campaigns for major features.

## Drawing and painting tools

Krita includes a sophisticated brush engine with hundreds of hand-authored brushes. Brush engines include Pixel, Smudge, Shape, Spray, Calligraphy, Clone, Heal, Blend, and Curvebrush. Custom brushes can be created using the Brush Editor. The brush system supports pressure sensitivity, tilt, speed, and rotation parameters. Layer blending modes are extensive (Multiply, Screen, Overlay, Color Dodge, etc.). Digital painting tools include pencils, charcoals, oils, watercolors, pastels, and abstract brushes. Strokes support anti-aliasing and brush dynamics. Tablet support is first-class; Wacom, Huion, and other pressure-sensitive devices work natively. The Stabilizer tool reduces shake and jitter in brush strokes. Perspective tools and deformation brushes assist with rapid iteration.

## Pixel-specific features (or "How artists use it for sprite work")

Krita has a dedicated Pixel Art environment accessible via Window > Workspace > Pixel Art. This workspace displays pixel-specific dockers and tools. The Pixel Brush is available in the brush collection and creates clean, anti-aliased-off strokes at exact pixel sizes. When the pixel brush is selected, onion skinning and grid snapping automatically become available. The Onion Skin docker shows semi-transparent previews of surrounding frames to guide frame-by-frame animation. Pixel art mode disables anti-aliasing by default, making strokes crisper and more suited to retro aesthetics. Grid display (Windows > Grid and Guides > Grid) can be toggled and customized. Snap to Grid and Snap to Pixels options lock brush strokes to the grid. The pixel workflow is intentionally streamlined, making Krita a viable alternative to dedicated pixel-art tools like Aseprite for artists who want a unified painting and animation environment.

## Color and palette workflow

Krita supports RGB, CMYK, Lab, YCbCr, XYZ, and other color spaces with 8-bit and 16-bit channel depth. The Color Swatches docker allows management of color palettes. Import and export of palette files in multiple formats (ACO, GPL, XML) are supported. The color picker is accessible via right-click or dedicated Eyedropper tool. Palette switching during work is fast and non-disruptive. Indexed color mode exists for limiting output to fixed palettes, useful for retro sprite constraints. The Recent Colors widget displays previously used colors, aiding workflow continuity. Dockable color wheels, sliders, and history improve palette exploration. Unlike Photoshop, Krita treats palettes as first-class citizens; animators can create per-project color restrictions and enforce them across frames.

## Layer system

Krita's layer system is hierarchical with groups, adjustment layers, and masks. Layers can be nested arbitrarily. Adjustment layers (Levels, Curves, Hue/Saturation, Color Balance, Channel Mixer, Posterize, Desaturate) apply non-destructively. Layer masks and vector masks control transparency and blending. Blend modes cover standard compositing operations. Raster layers, vector layers, group layers, and text layers are all supported. Clipping masks bind layers to predecessors. Paint layers preserve stroke history through the Undo system. For animation, frames are typically organized as layers stacked vertically or grouped by animation clip. Layer locking prevents accidental edits. Layer visibility toggling is fast. On large projects with hundreds of frames, performance remains acceptable on modern hardware.

## Animation features

Krita's animation tools are accessed via Window > Dockable Dialogs > Animation. The Animation workspace pre-configures the interface for animation work.

**Timeline docker**: Displays layers as an editable timeline. Rows represent frames; columns represent time. Each layer becomes a frame. Frame duration is editable per-frame (e.g., 100ms, 200ms). Playback controls (play, stop, rewind) are embedded. Preview playback is real-time or as-close-as-hardware-allows.

**Onion Skin**: The Onion Skin docker (Animation > Onion Skin Settings) shows semi-transparent previews of previous and next frames overlaid on the current frame. Opacity, count (number of surrounding frames to show), and color tinting are customizable. This is critical for frame-by-frame animation quality.

**Keyframe support**: Krita supports keyframes for layer properties (position, opacity, scale, rotation). Keyframe animation is less emphasized than frame-by-frame but available for motion graphics.

**Export animation**: File > Export Animation opens the sprite sheet exporter. Users select the export format (PNG sprite sheet, animated GIF, WebM video, MP4, or individual frame sequence). Sprite sheet layout (grid dimensions, frame order, padding) is configurable. The exporter automatically arranges frames into a grid suitable for game engines.

**Limitations**: No inverse kinematics or skeletal animation. Onion skinning is basic compared to Toon Boom Harmony or TVPaint. Timeline is not as feature-rich as professional animation software but is adequate for indie sprite work.

## Export and import

Krita natively reads and writes KRA (zipped XML + layer data, proprietary but documented). Supports import of PNG, JPEG, TIFF, BMP, WebP, GIF, and PSD (limited). Export options include PNG, JPEG, TIFF, WebP, GIF, PDF, SVG, and OpenRaster (ORA, interchange format). File > Export Animation specifically handles frame sequences and sprite sheets.

For sprite work:
- PNG sprite sheet export directly generates a single image with all frames tiled in a grid. This is the primary workflow for game asset creation.
- Animated GIF export creates a loopable animated GIF with per-frame delay.
- Video export (WebM, MP4) is available for preview or engine integration.
- Sprite sheets include automatic layout optimization; no manual grid construction needed.

## Scripting and extensibility

Krita supports plugins via Python 3 and C++. Python scripts can extend the UI, automate tasks, and manipulate layers. Scripts are placed in ~/.config/krita/pykrita or system plugin directories. The Python API documentation is available on docs.krita.org. Community-contributed scripts for sprite sheet generation, batch processing, and animation helpers are available on GitHub and the Krita Artists forum. The C++ plugin API allows deeper integration but requires compilation.

## Engine integration

Krita is not a game engine. However, sprite sheets and animations exported as PNG or video integrate seamlessly into Unity, Godot, Unreal, GameMaker, and other engines. Sprite sheet JSON metadata can be generated during export, mapping frame positions for game-engine sprite importers. Many indie developers use Krita end-to-end for sprite animation because the native export workflow is optimized for this task.

## Workflow strengths

- Free and open source; no licensing restrictions or subscription fees.
- Dedicated pixel-art mode with onion skinning built-in.
- Sprite sheet export is native and optimized; no plugins required.
- Unified painting and animation environment; no tool switching.
- Large and active community; extensive tutorials on pixel art and animation.
- Cross-platform (Windows, macOS, Linux); portable builds available.
- Active development; regular feature additions and bug fixes.

## Workflow gaps

- No rigged or skeletal animation; frame-by-frame only.
- Timeline is simpler than professional animation software (no layers of audio sync, no advanced easing).
- Onion skinning lacks color-coded frame indicators (previous frames all render in one color).
- Performance can degrade with hundreds of frames on older hardware, though modern systems handle typical game sprites well.
- No built-in sound/audio integration for animation preview.
- Less mature than Photoshop or Clip Studio Paint in illustration features (fewer brush engines, less advanced blending).

## Notable uses

Krita is popular in indie game development, particularly for pixel art games (e.g., "Hollow Knight" contributors have used similar tools; many indie devs cite Krita as their primary sprite tool). Used in educational settings for teaching digital art and animation. Growing adoption in professional studios as a cost-free alternative to Clip Studio Paint or Procreate Dreams.

## Community and ecosystem

Active community on Krita Artists forum, Discord, Reddit (r/krita), and GitHub. Regular development updates and feature announcements via blog. Extensive documentation on docs.krita.org. YouTube tutorials abundant. Third-party brush packs, plugins, and scripts shared via GitHub, Gumroad, and community sites.

## Pricing details

Krita is completely free. Source code is available under LGPL 3.0 license. Donations via Krita Foundation website support ongoing development. No in-app purchases, subscriptions, or premium tiers. Builds are distributed via:
- Official binaries on krita.org.
- Steam (free with optional tip).
- Snap, Flatpak, and package managers on Linux.
- Google Play Store (Android).
- macOS via Homebrew.

Users are encouraged but not required to donate. Many studios and individuals contribute financially to support the Krita Foundation.
