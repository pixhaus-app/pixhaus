# LibreSprite

## Quick facts
- Vendor / maintainer: Community volunteers (Orama Interactive and others)
- License / pricing model: GNU General Public License v2 (GPLv2)
- Price point (current): Free
- Platforms: Windows, macOS, Linux
- First released: 2016 (forked from Aseprite's final GPLv2 commit)
- Last meaningful update: v1.1 (September 22, 2024)
- Source available: Yes (fully open-source)
- Primary use case: Free sprite animation and pixel art creation

## Origin and purpose

LibreSprite is a fork of Aseprite created in response to the August 2016 licensing transition. When Aseprite shifted from open-source GPLv2 to proprietary EULA, the community preserved the last public GPLv2 version (commit prior to August 2016). LibreSprite continues as an independently maintained open-source project with volunteer contributors. The fork retains core Aseprite features available at the time of the split but diverges in feature development due to resource differences. Aseprite, backed by paid developers, receives more frequent updates; LibreSprite follows a slower release cadence. Both tools share similar workflows and file format compatibility (though some Aseprite-specific features may not import).

## Drawing and painting tools

LibreSprite includes pencil, eraser, line, rectangle, and ellipse tools. Drawing tools feature customizable brush sizes and hardness. The toolset emphasizes pixel-perfect rendering. Freehand mode with pixel perfect option ensures clean lines without anti-aliasing. Multiple selection tools (rectangular, freehand, magic wand) allow region isolation. Custom brushes can be created and saved for reuse. Fill tools (bucket fill, stroke selection) accelerate large area painting.

## Pixel-specific features

Pixel-perfect drawing mode prevents brush drift and maintains crisp edges. Grid overlay is configurable and can snap drawn shapes to grid points. Tiled drawing mode creates seamless patterns and textures without edge artifacts. Symmetry tools allow horizontal and vertical mirroring during painting, with simultaneous dual-axis symmetry support (addressing a past Aseprite limitation). Wide pixels option draws pixels at 2x or custom scales for visibility. Rotation and flip commands apply to selections and layers.

## Color and palette workflow

LibreSprite manages color palettes with foreground/background selection and quick swap (X key). Indexed color mode supports palette-based workflows for retro game art. Palette import from images and custom palette creation are supported. RGB and grayscale modes offer full color support. Color swatches in the interface allow rapid palette switching. Palette animations (color cycling) are supported for animated palette effects.

## Layer system

Layers support transparency in RGB/grayscale modes and transparent color index in indexed mode. Multiple layers enable compositing and separation of artwork components. Layer organization reduces clutter in complex sprites. The timeline displays layers vertically with frame data. Each cel (frame/layer intersection) can be edited independently. Layer opacity is adjustable. Merge and flatten operations combine layers.

## Animation features

The timeline is central to animation workflow. Frame-based animation with customizable frame duration (in milliseconds). Real-time animation playback preview within the editor. Onion skinning displays adjacent frames semi-transparently, aiding motion reference during frame drawing. Adjustable onion skin range and opacity. Loop and play settings control preview behavior. Animation export as GIF and PNG frame sequences. Frame tagging is supported (though less robust than Aseprite). Multiple cels per frame (layers) enable complex animations.

## Export and import

LibreSprite exports animations as GIF files and PNG frame sequences. Sprite sheet export with customizable row/column layout. Layered PSD export is supported (though with limitations). PNG and BMP image export. Aseprite .ase file format import provides some compatibility with Aseprite projects, though newer Aseprite features may not transfer cleanly. Native LibreSprite format (.lsp) preserves all project data. Batch export and multi-frame processing are available.

## Scripting and extensibility

LibreSprite includes a scripting system documented in SCRIPTING.md on GitHub. Scripting capabilities allow automation of repetitive tasks. The plugin architecture is less mature than Aseprite's. Community contributions add functionality; the open-source nature permits community-driven feature additions. Lua support is available through the scripting system. Extending the tool requires source code modification and recompilation for custom tools or deep integration.

## Engine integration

LibreSprite is editor-only. Sprite sheet exports are compatible with game engines (Unity, Unreal, Godot) via standard image formats and frame data files. No native engine bridge plugins exist. GIF export suits web preview and documentation. PNG sequences suit custom game engine pipelines.

## Workflow strengths

Free and open-source removes licensing barriers. GPLv2 permits unlimited use and source modification. Feature parity with Aseprite's 2016 state covers core sprite animation. Timeline and onion skinning match Aseprite's responsiveness. Palette-based workflows suit retro game art. Horizontal and vertical symmetry support (including simultaneous dual-axis). Community contributions enable feature evolution. No subscription or licensing fees.

## Workflow gaps

Development is volunteer-driven, resulting in slower release cycles and fewer features than Aseprite. Newer Aseprite features (advanced text tool, slice resizing, diagonal symmetry) are absent. Scripting is less documented and less community-rich than Aseprite's. Importing newer Aseprite projects may lose compatibility. No commercial support or guaranteed maintenance. Smaller community means fewer tutorials and third-party tools. UI refresh lags Aseprite aesthetically.

## Notable uses

LibreSprite serves independent developers and hobbyists prioritizing free tools and open-source philosophy. Game modding communities use it for sprite creation where licensing is controlled locally. Educational projects benefit from source code access. Pixel art communities appreciate the open-source preservation of Aseprite's golden era.

## Community and ecosystem

The LibreSprite GitHub repository (LibreSprite/LibreSprite) hosts discussions, bug reports, and pull requests. Community contributions drive feature additions and bug fixes. Online forums and Reddit communities include LibreSprite discussions alongside Aseprite comparisons. Documentation on the official website (libresprite.github.io) covers basics. Smaller community than Aseprite means fewer third-party tools and tutorials. Enthusiasts appreciate the libre software ethos and source transparency.

## Pricing details

LibreSprite is completely free, released under GPLv2. No payment, subscription, or license key required. Source code is publicly available on GitHub for inspection, modification, and recompilation. Redistribution is permitted under GPLv2 terms (source must accompany binaries). Compiled binaries are available from GitHub releases, SourceForge, and package managers (Flathub, Debian, Ubuntu). No commercial restrictions on created artwork.
