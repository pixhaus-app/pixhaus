# Affinity Photo / Affinity Designer

## Quick facts
- Vendor / maintainer: Serif (acquired by Canva in 2024; bundled as unified "Affinity" application in v3)
- License / pricing model: Freemium (v3); perpetual license option remains for v2
- Price point (current): Free (v3 with optional Canva Pro for AI features); v2 perpetual licenses discontinued but available secondhand
- Platforms: Windows, macOS, iPad, Android (v3)
- First released: October 2014 (Affinity Designer); February 2015 (Affinity Photo)
- Last meaningful update: Version 3.0 released October 2025 (unified Affinity application)
- Source available: No. Proprietary.
- Primary use case: Vector design, photo editing, and layout (v2 as perpetual apps; v3 unified freemium platform)

## Origin and purpose

Affinity Designer was released in October 2014 by Serif as a Photoshop alternative with emphasis on vector design. Affinity Photo followed in February 2015 as a Photoshop alternative for photo editing. Both were sold as one-time perpetual purchases, positioning themselves against Adobe's subscription Creative Cloud model. The applications gained popularity with designers seeking to avoid recurring subscription costs. In March 2024, Canva acquired Serif for approximately A$580 million. Following community backlash about licensing changes, Canva and Serif issued a pledge to maintain perpetual licenses for existing v2 owners. However, in October 2025, Affinity released version 3.0 as a free, unified application combining vector, raster, and layout capabilities. V3 is freemium; AI features (Generative Fill, Expand) require Canva Pro subscription. Existing v2 perpetual license holders retain access to v2; v2 is no longer actively maintained but remains stable.

## Drawing and painting tools

**Affinity Designer (vector-focused)**:
- Vector drawing tools (pen, pencil, node editing).
- Shape tools (rectangle, circle, polygon, star).
- Text tools with typography controls.
- No traditional painting brushes; focused on vector and shape-based design.

**Affinity Photo (raster-focused)**:
- Comprehensive brush engine with hundreds of brushes.
- Pencil, paintbrush, eraser, smudge, blur, sharpen, dodge, burn tools.
- Healing and clone tools for photo retouching.
- Layer blending modes and adjustment layers.
- No dedicated pixel-art mode, but brushes can be set to hard 1-pixel size with anti-aliasing disabled.

**Unified v3 Affinity**:
- Combines vector and raster capabilities in a single application.
- Users can switch between vector (Designer) and raster (Photo) "Personas" (modes) without leaving the app.
- Same brush and drawing tools as separate v2 Photo, plus vector tools from v2 Designer.

## Pixel-specific features (or "How artists use it for sprite work")

Neither Affinity Photo nor Designer has a dedicated pixel-art mode. Sprite artists work with pixel constraints by:
- Using hard 1-pixel brushes with anti-aliasing disabled.
- Enabling grid display and snap-to-grid.
- Working at integer zoom levels.

Affinity is rarely chosen as the primary tool for sprite animation due to lack of dedicated animation features. However, character design and asset creation workflows may use Affinity Photo's raster tools or Designer's vector tools before export to game engines or animation tools.

## Color and palette workflow

Affinity supports RGB, CMYK, Lab, and Grayscale color spaces. Color picker with standard hue/saturation/brightness sliders. Color swatches can be saved and imported. Palette management is functional but not animation-specific. Indexed color mode does not exist in either application; color palettes are managed as swatches only. For game sprite work requiring strict color budgets (e.g., 16-color palettes), artists must enforce constraints externally.

## Layer system

Both Affinity Photo and Designer support hierarchical layer systems. Layers can be grouped in folders. Adjustment layers apply color corrections non-destructively (Curves, Levels, Hue/Saturation, Color Balance, Posterize, etc.). Layer masks enable selective transparency. Clipping masks bind layers. Layer opacity and blend modes are standard. Vector layers (in Designer or unified v3) maintain scalability; raster layers (in Photo or v3) are fixed-resolution. The unified v3 application allows mixing vector and raster layers in the same document, enabling designers to switch between vector and raster workflows without layer conversion overhead.

## Animation features

Neither Affinity Photo nor Affinity Designer has animation support (timeline, keyframes, frame-by-frame). Both applications are design-focused, not animation-focused. Animation for sprite work must be handled in separate tools (Krita, Clip Studio Paint, Procreate Dreams) or in game engines directly.

This is a significant gap compared to Clip Studio Paint or Krita, both of which offer integrated animation timelines.

## Export and import

**Affinity Photo / Designer v2**:
- Native format: AFPHOTO (Photo) or AFDESIGN (Designer).
- Import: PNG, JPEG, TIFF, GIF, PSD, SVG, PDF.
- Export: PNG, JPEG, TIFF, PDF, SVG (Designer), WebP.
- No animated GIF or video export.
- No sprite sheet packing.

**Unified Affinity v3**:
- Native format: AFDESIGN (unified document).
- Import: PNG, JPEG, TIFF, GIF, PSD, SVG, PDF.
- Export: PNG, JPEG, TIFF, PDF, SVG, WebP.
- No animated GIF or video export.
- No sprite sheet packing.

For sprite work:
- PNG export with transparency for game asset import.
- PSD export for compatibility with other tools.
- No native sprite sheet export; manual grid layout required or external tools needed.

## Scripting and extensibility

Affinity v2 did not have a public scripting or plugin API. Workflows were extended via UI automation and manual processes.

Affinity v3 (unified application) plans for future API expansion but details are not yet public. Community requests for plugin support and scripting have been noted but are not yet implemented.

## Engine integration

Affinity is a creative tool, not a game engine. Assets created in Affinity are exported as PNG, SVG, or PSD and imported into game engines (Unity, Godot, Unreal, GameMaker) via their respective asset importers. The lack of animation features means sprites are typically created in Affinity and animated in a separate tool or engine.

## Workflow strengths

- **v3**: Completely free (freemium model); no cost barrier for basic design work.
- **v3**: Unified application combining vector and raster in a single interface.
- **v2 (legacy)**: Perpetual license option; one-time purchase with lifetime access (no longer available for new buyers, but existing owners retain access).
- **Both**: No subscription required for core features (v3 AI features require Canva Pro).
- Good for illustration and design work that later becomes sprite assets.
- Cross-platform (Windows, macOS, iPad, Android in v3).
- Professional-grade vector and raster tools.

## Workflow gaps

- No animation timeline, keyframes, or frame-by-frame support in either version.
- No dedicated pixel-art mode.
- No sprite sheet packing or export.
- Perpetual licenses no longer offered for new purchases (v3 is freemium).
- Affinity v2 is no longer actively maintained; bug fixes are rare.
- AI features (Generative Fill, Expand) in v3 require Canva Pro subscription (not included in free tier).
- Less mature community and ecosystem compared to Photoshop or Krita.
- No audio sync or sound integration.

## Notable uses

Affinity Photo and Designer were popular with designers avoiding Adobe subscriptions, particularly in Web Design, Graphic Design, and UI/UX workflows. In game development, Affinity was used for UI design and asset preparation rather than primary asset creation or animation. The free v3 release may increase adoption for casual users and students, though the lack of animation features limits use in game sprite workflows.

## Community and ecosystem

Moderate community on Affinity's official forums, Discord, and social media. YouTube tutorials abundant. Fewer third-party plugins and asset packs compared to Adobe or open-source tools. With the free v3 release and Canva acquisition, community growth is expected, but ecosystem maturity lags behind Photoshop.

## Pricing details

**Affinity v3 (Current, October 2025+)**:
- Free (freemium model).
- No upfront cost for core vector, raster, and layout features.
- Canva Pro subscription ($120/year or $13/month) required for AI features (Generative Fill, Generative Expand).
- Available on Windows, macOS, iPad, and Android.
- Regular free updates.

**Affinity v2 (Legacy, no longer sold)**:
- Affinity Designer 2: One-time purchase for perpetual license (no longer available for purchase; price was $69.99).
- Affinity Photo 2: One-time purchase for perpetual license (no longer available for purchase; price was $69.99).
- Affinity Publisher 2: One-time purchase for perpetual license (no longer available for purchase; price was $69.99).
- Existing v2 owners retain access; v2 is not actively developed but remains stable.
- Secondary market (Gumroad, eBay, resellers) may have v2 licenses, but pricing varies.

**Free trial for v3**: Free to use indefinitely; Canva Pro features available for trial period.

The transition from perpetual licenses to freemium reflects Canva's cloud-first strategy. For users who valued perpetual licenses, v2 remains usable but no longer receives updates.
