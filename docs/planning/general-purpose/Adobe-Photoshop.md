# Adobe Photoshop

## Quick facts
- Vendor / maintainer: Adobe Systems
- License / pricing model: Subscription only (Creative Cloud)
- Price point (current): $22.99/month single app (annual commitment), $34.49/month (month-to-month)
- Platforms: Windows, macOS
- First released: February 19, 1990 (version 1.0 on Macintosh)
- Last meaningful update: October 2025 (version 27.0)
- Source available: No. Proprietary.
- Primary use case: Photo editing, retouching, digital design, and general-purpose image manipulation

## Origin and purpose

Photoshop was created by Thomas and John Knoll beginning in 1988. Version 0.87 (March 1989) was the first commercially available version, distributed as "Barneyscan XP". Photoshop 1.0 released February 19, 1990, for Macintosh at $895 list price. Version 2.5 (November 1992) brought Windows support. Version 3.0 (September 1994) introduced layers, becoming the industry standard. From version 2020 onward, Adobe transitioned to subscription-only licensing via Creative Cloud, ending perpetual license sales. Photoshop now receives continuous feature updates rather than version-numbered releases.

## Drawing and painting tools

Photoshop includes a comprehensive brush engine with over 1000 brushes available through various libraries. The brush system supports texture brushes, bristle simulation, and custom brush creation. Procedural brushes can be built with the Brush panel. Tablet pressure sensitivity and pen tilt are fully supported. The pencil tool offers hard-edged drawing; the paintbrush provides soft feathering. Smudge, blur, sharpen, dodge, burn, and sponge tools available. Gradient and fill buckets work predictably. For pixel art specifically, artists often disable anti-aliasing and use 1-pixel pencil or hard brushes at 100% opacity. The grid and rulers can be enabled to assist with pixel-perfect placement. However, Photoshop was not designed for pixel-art-first workflows, so sprite artists typically resort to workarounds like locking pixels to grid and using custom brushes rather than leveraging native pixel-art features.

## Pixel-specific features (or "How artists use it for sprite work")

Photoshop has no dedicated pixel-art mode. Sprite artists work around this by disabling anti-aliasing in brush settings and using hard round brushes sized to 1 pixel. The Snap to Pixel option can be enabled to lock brush strokes to pixel boundaries. Grid display (Window > Guides and Grid) and pixel-precise zoom levels help with alignment. Layers can be rasterized to lock them to the pixel grid. Many game artists abuse Photoshop's general tools for sprite work because they are already familiar with the software, even though dedicated pixel-art tools (Aseprite, Piskel) are faster for this workflow.

## Color and palette workflow

Photoshop has a standard color picker with RGB, CMYK, Lab, and other color spaces. The color swatches panel allows preset palettes to be imported. Indexed color mode exists for limiting output to a fixed palette, useful for retro game sprites with strict color limits (e.g., 16-color or 256-color palettes). The Color Range selection tool can select all pixels of a similar color. The Eyedropper samples colors from the canvas. Palette editing is not deeply integrated—swatches are stored as simple lists. Many sprite artists export to indexed color, but Photoshop's palette tools lack the per-frame control that animation-focused tools offer.

## Layer system

Photoshop's layer system is mature and hierarchical. Layers can be grouped into folders. Adjustment layers (Levels, Curves, Hue/Saturation, Color Balance, etc.) apply non-destructively. Layer masks enable selective transparency and blending. Smart Objects embed external files as linked or embedded content. Layer opacity and blend modes (Multiply, Screen, Overlay, etc.) are standard. For sprite animation, each frame is typically placed on a separate layer, grouped by animation clip. Clipping masks and layer group blending are available. Timeline-based animation still uses layers, making large sprite projects unwieldy if not organized carefully.

## Animation features

Photoshop includes two animation workflows: frame-based and timeline (keyframe-based).

**Frame-based animation**: The Animation panel (Window > Animation) shows layers as frames. Each layer becomes a frame in the sequence. The user sets frame delays and plays back the animation within the panel. This is simpler for sprite animation but less flexible for complex motion. Frame sequences can be exported as animated GIF or video. No onion skinning in frame mode.

**Timeline animation**: The Timeline panel (Window > Timeline) allows keyframe-based animation. Layers can have keyframes set for position, opacity, scale, rotation, and style properties. Photoshop interpolates between keyframes automatically. This workflow is better for motion graphics but overkill for pixel-art frame-by-frame animation. Timeline supports 24, 25, 30, 60 fps and custom frame rates. The playhead shows the current frame.

Export options: File > Export > Render Video exports to MP4, MOV, WebM at user-defined frame rates. File > Export > Save for Web (Legacy) exports animated GIF. Neither export method is sprite-sheet focused—you must manually arrange frames into a grid or use a third-party plugin to pack sprites efficiently. Video Copilot's Element 3D or similar plugins do not integrate natively.

Limitations: No built-in sprite sheet generator. No onion skinning (showing previous/next frames semi-transparently). No frame-by-frame drawing mode that locks the canvas to a single active frame. Timeline playback is slow and non-interactive compared to dedicated animation tools. Many sprite animators find Photoshop's animation workflow cumbersome because it conflates general motion graphics (keyframes on properties) with frame-by-frame animation (drawing a new image on each frame).

## Export and import

Photoshop natively reads and writes PSD (proprietary Adobe format). Supports import of JPEG, PNG, TIFF, GIF, BMP, WebP, and many other raster formats. Supports export to JPEG, PNG, TIFF, WebP, PDF, and GIF (animated and static). File > Export As offers batch export to multiple formats.

For sprite work:
- PNG export preserves transparency and quality, suitable for game asset import.
- Animated GIF export via Save for Web creates a single looped or one-shot GIF.
- Video export (MP4, WebM) works but requires video-player integration.
- Sprite sheets must be manually constructed or scripted—no native "pack layers into grid" feature. Third-party plugins exist (e.g., layer to sprite sheet scripts) but are not built-in.

## Scripting and extensibility

Photoshop supports scripting via ExtendScript (JavaScript-like language), UXP (Unified Extensible Platform for plugin development), and Batchplay. ExtendScript is deprecated in favor of UXP. UXP plugins can automate workflows but are sandboxed and have limited API access compared to legacy plugins. Batchplay is Adobe's new scripting API but is not yet fully documented. Community scripts (via Gumroad, GitHub) exist for sprite sheet generation and animation batching, but they are not official Adobe products.

## Engine integration

Photoshop is not a game engine and has no built-in export for game-engine-specific formats. However, sprite sheets and animations exported as PNG/GIF/video can be imported into any game engine (Unity, Godot, Unreal, GameMaker) that accepts these formats. Many indie developers work in Photoshop, then import PSD files or exported PNGs into their engine's sprite importer.

## Workflow strengths

- Familiar to many designers and artists; large community support.
- Powerful non-destructive editing with adjustment layers, masks, and blend modes.
- Good for illustration and concept art that later becomes sprite assets.
- Integrated with Adobe Creative Cloud (Illustrator, Lightroom, XD, Premiere) for cross-tool workflows.
- Industry standard for photo manipulation and digital design.

## Workflow gaps

- No pixel-art-first tools (no dedicated pixel brush, no pixel grid snap by default, no dedicated onion skinning).
- Animation timeline is slow and not optimized for frame-by-frame sprite animation.
- No sprite sheet packing or export.
- Subscription-only pricing; no perpetual license option.
- Learning curve for animation workflows (confusing to mix keyframe and frame-based animation paradigms).
- Not designed for rapid iteration on sprite frames.

## Notable uses

Photoshop remains common in game development studios for concept art, UI mockups, and asset preparation, but rarely as the primary tool for sprite animation. Larger studios often use Photoshop for initial design, then transition to dedicated animation software or hand off to technical artists for sprite sheet assembly. Indie developers sometimes use it end-to-end due to familiarity, but efficiency suffers.

## Community and ecosystem

Massive community. Thousands of tutorials, templates, presets, and scripts available online. Adobe provides official documentation and regular updates. Third-party plugins available on Adobe Exchange and independent sites. Stack Exchange, Reddit, and forums have extensive Photoshop animation discussions.

## Pricing details

As of January 2025, Photoshop is subscription-only through Adobe Creative Cloud. Pricing tiers:
- Single App plan (Photoshop only): $22.99/month with annual commitment or $34.49/month month-to-month. Includes 100GB storage.
- Creative Cloud All Apps: Bundles Photoshop with Illustrator, InDesign, Premiere Pro, etc. Higher tier pricing.
- Photography plan ($9.99/month or $119.88/year) includes Photoshop and Lightroom with 20GB storage.
- Student/teacher discounts available.
- Free trial: 7 days.

Perpetual licenses ended with Photoshop 2020. All current subscriptions are cloud-based; work is not tied to desktop unless explicitly saved locally.
