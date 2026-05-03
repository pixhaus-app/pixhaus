# GIMP

## Quick facts
- Vendor / maintainer: GNOME Foundation (community-driven open source)
- License / pricing model: Free and open source (GPLv3 or later)
- Price point (current): Free
- Platforms: Windows, macOS, Linux
- First released: February 1996 (version 0.54)
- Last meaningful update: GIMP 2.10 series (stable); GIMP 3.0 in development
- Source available: Yes. Full source on GIMP GitLab (https://gitlab.gnome.org/GNOME/gimp)
- Primary use case: General-purpose raster image manipulation and photo editing

## Origin and purpose

GIMP (GNU Image Manipulation Program) was created in 1995 by Spencer Kimball and Peter Mattis as a semester project at UC Berkeley's eXperimental Computing Facility. Version 0.54 released February 1996 was the first public release and marked the first free software program capable of competing with commercial image editors like Photoshop. GIMP was architected with a plug-in system from the start, allowing third-party developers to extend functionality without modifying the core. The application is primarily developed by volunteers and is associated with both GNU and GNOME projects. While GIMP is a versatile image editor, it was not designed with animation or pixel art as primary use cases, though both are achievable via plugins and workarounds.

## Drawing and painting tools

GIMP includes basic painting tools: pencil, paintbrush, eraser, clone, heal, smudge, blur, sharpen, dodge, and burn. Brushes can be customized and imported. Pressure sensitivity for graphics tablets is supported. Layer blending modes (Multiply, Screen, Overlay, etc.) are available. However, GIMP's brush engine is less sophisticated than Krita or Photoshop. Texture brushes are limited; procedural brush creation is cumbersome. The Gfig plug-in allows geometric shapes. Text tools support basic typography but lack advanced text editing. For pixel art specifically, GIMP offers no dedicated pixel mode; artists must manually disable anti-aliasing per brush and use hard brushes sized to 1 pixel. The learning curve for animation in GIMP is steeper than in dedicated tools.

## Pixel-specific features (or "How artists use it for sprite work")

GIMP has no built-in pixel-art mode or onion skinning. To work with pixel art, artists disable anti-aliasing in individual brush settings and use hard round brushes. Grid display (Image > Guides > New Guide Set) can help with alignment. Filters > Distorts > Pixelize can reduce artwork to pixel constraints. For animation, the Filters > Animation menu provides frame-sequence tools, but GIMP does not treat animation as a first-class workflow. The onion-skin effect must be simulated by manually adjusting layer opacity or using third-party scripts. Sprite sheet generation requires community plugins (see "Scripting and extensibility" section). Most pixel artists avoid GIMP for primary sprite work due to the lack of animation-specific tooling; it is used primarily for asset preparation before import into engines or animation tools.

## Color and palette workflow

GIMP supports RGB, Grayscale, and Indexed color modes. Color space conversion (Image > Mode) allows switching between modes. The color picker (Eyedropper tool) samples colors from the canvas. Color swatches can be imported from palette files (ACO, GPL, PAL). The Indexed color mode limits output to a fixed palette, useful for retro sprite constraints. However, palette management in GIMP is minimal—swatches are stored as simple lists, and per-frame color restrictions are not enforced. Animated GIF export can use the indexed palette, which is useful for retro games. The color workflow is functional but not optimized for animation.

## Layer system

GIMP's layer system is hierarchical. Layers can be stacked and grouped (via layer directories in some builds). Layer masks and layer groups control transparency and compositing. Adjustment layers do not exist; color adjustments must be applied destructively to raster layers or as separate adjustment layers that are manually composited. Layer opacity and blend modes are standard. For animation, layers represent frames, which makes large sprite projects with hundreds of frames unwieldy and slow. Renaming layers with frame numbers aids organization but is tedious. Layer locking prevents accidental edits. Filters > Light and Shadow and other layer-based effects are available but not as sophisticated as Photoshop or Krita.

## Animation features

GIMP does not have native animation tools in the core application. Animation is handled entirely through plugins and the Filters > Animation menu.

**Frame-by-frame via layers**: Users can create frame sequences by stacking layers. The Filters > Animation > Unoptimize filter reverses GIF optimization. Filters > Animation > Optimize reverses this. To preview, use Filters > Animation > Playback.

**Animated GIF export**: File > Export As with .gif extension opens the GIF export dialog. Users set per-frame delays and choose looping. This is the primary animation output method.

**Community plugins for sprite sheets**: Several third-party plugins generate sprite sheets from GIMP layers:
- GimpSpriteAtlas (GitHub: BdR76/GimpSpriteAtlas) - packs layers into an optimal sprite atlas using a 2D packing algorithm and exports TexturePacker JSON, LibGDX, CSS, or XML metadata.
- Tilemancer (GitHub: malteehrlen/tilemancer) - generates sprite sheets with optional layer-group-based row organization.
- gimp-export-spritesheet (GitHub: jarnik/gimp-export-spritesheet) - exports layers to a PNG sprite sheet with XML metadata.

These plugins must be manually installed to ~/.config/GIMP/2.10/plug-ins/ or equivalent and are not part of the official GIMP distribution.

**Limitations**: No onion skinning. No keyframe animation. Timeline is absent. Playback is slow and non-interactive. Many sprite animators view GIMP as a preparation tool rather than a primary animation application.

## Export and import

GIMP natively reads and writes XCF (proprietary GIMP format, zipped XML + raster data). Supports import of PNG, JPEG, TIFF, BMP, GIF, WebP, and many other raster formats. Export options include PNG, JPEG, TIFF, WebP, GIF (including animated), PDF, and PostScript.

For sprite work:
- PNG export preserves transparency and quality.
- Animated GIF export with per-frame delay support.
- Sprite sheet export via community plugins generates PNG + metadata (JSON, XML).
- No native sprite sheet packing; plugins are required.

## Scripting and extensibility

GIMP supports scripting via Script-Fu (Scheme dialect) and Python-Fu (Python 2 and Python 3). Scripts can be placed in the plug-ins directory and are automatically loaded on startup. The Script-Fu and Python-Fu consoles allow interactive scripting. Documentation is available but scattered. Community scripts for sprite sheet generation, batch processing, and animation helpers are available on GitHub and the GIMP Plugin Registry (deprecated but archived). Writing custom plugins requires understanding the PDB (Procedure Database) API, which has a learning curve.

## Engine integration

GIMP is not a game engine. Sprite sheets and animations exported as PNG or GIF integrate into any game engine (Unity, Godot, Unreal, GameMaker). Sprite sheet metadata from third-party plugins aids engine integration. However, without native sprite sheet support, the workflow is fragmented and error-prone compared to Krita.

## Workflow strengths

- Free and open source; widely available.
- Mature and stable; millions of users.
- Good for general image manipulation and asset preparation.
- Extensible via plugins for specialized tasks.
- Cross-platform (Windows, macOS, Linux).
- Large community and extensive documentation.

## Workflow gaps

- No native animation timeline or onion skinning.
- No dedicated pixel-art mode.
- Animation requires manual layer-by-layer setup and external playback.
- Sprite sheet export is not native; requires community plugins.
- Brush engine is basic compared to modern digital painting software.
- Not designed for rapid iteration on sprite frames.
- Steep learning curve for animation workflows.

## Notable uses

GIMP is used primarily for photo editing, general image manipulation, and asset preparation in game development. It is rarely the primary tool for sprite animation due to lack of animation-specific features. Educational institutions often teach GIMP as a free Photoshop alternative. Hobbyist and indie developers use GIMP when budget is the primary constraint.

## Community and ecosystem

Large, mature community on GIMP Forums, Stack Exchange, Reddit (r/GIMP), and Discord. Extensive documentation on GIMP's official website. YouTube tutorials abundant. Third-party plugins and scripts available on GitHub and archived plugin registry. Active development continues under GNOME Foundation stewardship, though feature additions are incremental.

## Pricing details

GIMP is completely free. Source code is available under GPLv3 or later license. Distributed via:
- Official builds on gimp.org.
- Package managers on Linux (apt, yum, etc.).
- Homebrew on macOS.
- Microsoft Store on Windows.
- Flatpak and Snap containers.

No subscriptions, in-app purchases, or premium features. Donations are accepted but not required.
