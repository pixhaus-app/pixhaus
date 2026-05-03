# Piskel

## Quick facts
- Vendor / maintainer: Community volunteers (originally created at Google)
- License / pricing model: Open-source (Apache 2.0)
- Price point (current): Free
- Platforms: Web browser (Chrome, Firefox, Safari, Edge); desktop apps for Windows, macOS, Linux
- First released: 2012 (as Google Labs project)
- Last meaningful update: Active development (unclear exact version/date)
- Source available: Yes (GitHub: piskelapp/piskel)
- Primary use case: Free, accessible sprite and pixel animation for web and indie games

## Origin and purpose

Piskel began as a Google Labs project in 2012, exploring browser-based creative tools. The open-source Apache 2.0 license enabled community adoption and contributions. Google's initial backing lent credibility and resources; ongoing development is now community-driven. The web-first approach made pixel art accessible without downloads or installation, lowering barriers for artists and educators. Desktop applications (Electron-based) extended reach to offline workflows. Piskel's simplicity and zero-cost model position it as an entry point for pixel art novices and a supplementary tool for professionals.

## Drawing and painting tools

Pencil, eraser, brush, line, rectangle, and ellipse tools. Customizable brush sizes. Fill tool (bucket fill) with color replacement options. Selection tools (rectangular, freehand) for region isolation. Color picker (eyedropper). Custom brushes can be created and saved. The drawing toolkit is straightforward and responsive, suitable for sprite creation and animation without steep learning curves.

## Pixel-specific features

Grid overlay with configurable spacing and snapping to grid. Zoom levels support detailed work and overview. Pixel-perfect line drawing. Tiled drawing mode for seamless pattern creation. Rotation and flip commands on selections and layers. Dithering patterns available for some tools. Anti-aliasing can be toggled for crisp pixel edges. Canvas resizing and cropping. Symmetry tools for bilateral drawing (if available; feature parity varies by browser).

## Color and palette workflow

Foreground and background color selection with quick swap. Color history tracking recent colors. Palette import from images or manual palette creation. RGB color mode. Color swatches for rapid switching. Eyedropper tool. Palette management is functional for typical sprite work. Limited palette animation or indexed color mode compared to dedicated tools.

## Layer system

Multiple layers enable composition and separation of sprite elements. Layer visibility toggling (eye icon). Layer opacity control with adjustable values. Layers are organized vertically in the timeline. Merging and flattening operations combine layers. Layer-based organization suits multi-element sprites (background, character, effects). Layer group support is unknown/limited.

## Animation features

Frame-based animation with timeline displaying frames horizontally. Adjustable frame duration (in milliseconds). Onion skinning shows adjacent frames semi-transparently, aiding motion continuity. Customizable onion skin opacity and range. Real-time playback preview at specified frame rate. Loop and playback settings (loop, ping-pong, single play). Frame duplication and deletion. Animation preview is responsive and updates in real-time. Animation state persists during editing.

## Export and import

Animated GIF export for sharing and social media embedding. PNG sprite sheet export with customizable rows and columns. PNG frame sequences for custom workflows. PNG image import to populate frames. GIF import to convert existing animations into Piskel projects. SVG support (limited, likely vector-to-raster conversion). PISKEL project format (.piskel) saves all animation data and layer information for reopening in Piskel. Batch export capabilities for processing multiple projects.

## Scripting and extensibility

No scripting or plugin system in Piskel itself. Browser-based architecture limits deep integration. Community extensions or userscripts may exist but are not officially supported. Export formats (JSON, PNG, GIF) enable external automation workflows. Open-source codebase permits forking for custom builds.

## Engine integration

Piskel sprite sheets (PNG) with layout data integrate into game engines (Unity, Godot, Phaser, custom engines). Frame duration and grid information must be manually coded into engine asset pipelines or extracted from Piskel's JSON export. No native engine plugins, but spritesheet format is universal. GIF export suits web preview and documentation.

## Workflow strengths

Web-based accessibility: no installation required; works in any modern browser. Offline desktop apps available for Windows, macOS, Linux. Free and open-source removes licensing barriers. Simple, uncluttered UI suitable for beginners and quick mockups. Real-time animation preview with responsive playback. Multiple export formats (GIF, PNG sheet, frame sequences). Grid snapping and onion skinning support smooth animation. Sprite sheet export ready for game engines. No account required for local work; optional sign-up for cloud save. Cross-platform consistency (web and desktop).

## Workflow gaps

Limited advanced features: no text tool, no custom brushes beyond basics, no blend modes or advanced layer effects. No non-destructive editing (adjustment layers, smart objects). Palette animation and indexed color workflows unsupported. No scripting or plugins. UI responsiveness varies by browser (browser rendering limitations). Desktop versions are Electron-based, requiring system resources. No collaboration tools (real-time multiplayer editing). Frame count limits unknown; performance may degrade with large animation counts. Documentation and tutorials are limited. Community is smaller than Aseprite's, reducing third-party resources.

## Notable uses

Piskel is widely used in educational contexts: computer science classes, art camps, and hackathons benefit from zero-cost access and no-install model. Game jam participants use Piskel for quick sprite prototyping. Indie game hobbyists use it for simple 2D pixel art. Web game developers (using Phaser, Babylon.js) export Piskel sprite sheets. Pixel art newcomers often begin with Piskel before graduating to Aseprite.

## Community and ecosystem

Small but active open-source community. GitHub repository (piskelapp/piskel) hosts source code, issues, and pull requests. Official website (piskelapp.com) provides access to the web editor and documentation. Discord or community forums exist but are modest in size. YouTube tutorials cover basic workflows. Reddit communities (r/pixelart, r/gamedev) reference Piskel. Educational resource sites integrate Piskel into curricula. No commercial support or development roadmap documentation.

## Pricing details

Piskel is completely free. No payment, license, or subscription required. Open-source Apache 2.0 license permits use, modification, and redistribution. Web editor requires no account (data stored locally in browser). Optional account creation for cloud save (unclear if freemium). Desktop applications available for free download. No premium tier or feature limitations based on payment. Created artwork has no commercial restrictions.
