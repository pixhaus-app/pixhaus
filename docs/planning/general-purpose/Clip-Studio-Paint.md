# Clip Studio Paint

## Quick facts
- Vendor / maintainer: Celsys (Japanese graphics software company)
- License / pricing model: Perpetual license or subscription
- Price point (current): Perpetual: $258 (EX version); Subscription: $8.99/month (EX) or $4.49/month (PRO). Annual: $71.99 (EX) or $24.99 (PRO)
- Platforms: Windows, macOS, iPad, iPhone, Android tablets, Chromebooks
- First released: May 31, 2012 (as "Clip Studio Paint"; evolved from "Manga Studio" and "Comic Studio")
- Last meaningful update: Version 4.0 released March 2025
- Source available: No. Proprietary.
- Primary use case: Illustration, comics, webtoons, and animation with strong Japanese market presence

## Origin and purpose

Clip Studio Paint evolved from Comic Studio, released in Japan in 2001 as a digital comics tool. The application was sold as "Manga Studio" in Western markets by E Frontier America (until 2007) and later Smith Micro Software. In 2012, Japanese developer Celsys released Clip Studio Paint as a successor product based on the Illust Studio application. Celsys unified branding worldwide as "Clip Studio Paint" in 2016. The application targets illustrators, comic artists, webtoon creators, and animators with a focus on professional-grade tools for 2D content creation. Celsys is known for its strong presence in Japanese animation and manga industries. Clip Studio Paint is widely used in professional animation studios globally.

## Drawing and painting tools

Clip Studio Paint includes a comprehensive brush library with hundreds of hand-authored brushes for illustration, comics, animation, and effects. Brushes cover pencils, inks, markers, chalk, pastels, oils, watercolors, and airbrushes. The brush engine supports pressure sensitivity, tilt, speed, and rotation dynamics. Custom brushes can be created and edited. Layer blending modes are extensive. Transform tools (scale, rotate, skew, mesh transformation) assist with asset creation. The Perspective Ruler tool locks strokes to perspective grids, useful for backgrounds and 3D-aware illustration. Animation-specific tools include frame manipulation and cel animation features.

## Pixel-specific features (or "How artists use it for sprite work")

Clip Studio Paint does not have a dedicated pixel-art mode. However, sprite artists work with pixel constraints by:
- Using hard 1-pixel brushes with anti-aliasing disabled.
- Enabling grid display and snap-to-grid.
- Working with limited color palettes via indexed color export.

Many sprite animators use Clip Studio Paint because of its strong animation timeline and frame-by-frame support, combined with general illustration tools. Unlike Krita, which explicitly targets pixel art, Clip Studio Paint treats pixel art as a specialized use case within general illustration.

## Color and palette workflow

Clip Studio Paint supports RGB, CMYK, and Grayscale color spaces. The color picker offers standard hue/saturation/brightness sliders and spectrum picker. Color swatches can be saved and imported from palette files (ACO, ASE, etc.). The color history displays recently used colors. Indexed color mode exists for limiting output to fixed color palettes, useful for retro sprite constraints. Color-dynamic brushes can pick colors from gradients during painting. Recent updates (Ver. 4.0) include improved color organization in the timeline for animation projects.

## Layer system

Clip Studio Paint's layer system is hierarchical and feature-rich. Layers can be grouped in folders. Adjustment layers apply color corrections non-destructively (Levels, Curves, Hue/Saturation, Color Balance, Posterize, Threshold, Colorize). Layer masks control transparency and blending. Clipping masks bind layers. Layer opacity and blend modes (Multiply, Screen, Overlay, etc.) are standard. Raster layers, vector layers, text layers, and group layers are all supported. For animation, layers represent frames and are typically organized by animation sequence. Layer organization tools help manage large projects with hundreds of frames.

## Animation features

Clip Studio Paint's animation capabilities are professional-grade and represent one of the application's core strengths.

**Timeline**: The Timeline panel displays frames horizontally. Each layer becomes a frame in the sequence. The timeline shows frame duration (adjustable per-frame), and playback controls (play, stop, rewind, frame step) are embedded. Ver. 4.0 includes color-coded layers in the timeline, improving visual organization for complex projects.

**Frame management**: Frames can be duplicated, deleted, and reordered. Frame duration is adjustable globally or per-frame (e.g., 100ms for 10 fps, 41ms for 24 fps). Playback preview is real-time or approximate depending on hardware.

**Onion skin**: The Onion Skin feature shows semi-transparent previews of surrounding frames overlaid on the current frame. Opacity and frame count are customizable. Essential for frame-by-frame animation quality.

**Keyframe animation**: Supports keyframe-based animation for layer properties (position, scale, rotation, opacity). Keyframes are set on the timeline, and interpolation is automatic. Easing options are available for motion graphics.

**Camera actions**: Recent versions introduced camera pan, zoom, and rotation keyframes for cinematic animation effects (e.g., camera pans across a static background).

**Audio import**: Video files can be imported and their audio extracted as a separate audio track in the timeline. This allows sound-synchronized animation.

**Simple mode animation**: Ver. 4.0 added Simple Mode animation controls, simplifying the timeline interface for basic frame-by-frame animation.

**Export animation**: File > Export outputs animations in multiple formats:
- MP4 video (H.264).
- WebM video.
- GIF (animated, loopable).
- Frame sequence (individual image files).
- Sprite sheet (PNG grid with frame layout suitable for game engines).

Sprite sheet export is native and optimized; no plugins required.

## Export and import

Clip Studio Paint natively saves to CLIP (proprietary format, zipped XML + raster/vector data). Supports import of PNG, JPEG, TIFF, PSD, SVG, GIF, and video files. Export options include PNG, JPEG, TIFF, PSD, PDF, GIF (including animated), MP4, WebM, and sprite sheet.

For sprite work:
- PNG export with transparency for game asset import.
- Sprite sheet export directly generates a grid layout suitable for game engines.
- Animated GIF and video export for preview or engine integration.
- PSD export for compatibility with other tools.

## Scripting and extensibility

Clip Studio Paint supports scripting via JavaScript (ES6+). Scripts can automate tasks, extend the UI, and manipulate layers. The script API documentation is available in Japanese and English. Community scripts for animation helpers, batch processing, and sprite sheet generation are shared via GitHub and Clip Studio's material store. C++ plugins can be developed but require compilation and are less common than scripts.

## Engine integration

Clip Studio Paint is not a game engine. However, animations exported as sprite sheets, GIF, or video integrate seamlessly into game engines (Unity, Godot, Unreal, GameMaker). Sprite sheet export is optimized for game-engine sprite importers. Many professional game studios use Clip Studio Paint for sprite animation because of its timeline sophistication and native sprite sheet export.

## Workflow strengths

- Professional-grade animation timeline with keyframe support.
- Robust frame-by-frame animation tools with onion skinning.
- Native sprite sheet export optimized for game engines.
- Strong integration with Japanese animation and manga industries; widely used in professional studios.
- Perpetual license option available (no subscription requirement if preferred).
- Multi-platform (Windows, macOS, iPad, Android, Chromebooks).
- Extensive brush library and illustration tools.
- Camera actions and cinematic animation tools.
- Audio sync for dialogue and music-timed animation.
- Regular updates with new animation features (Ver. 4.0 color-coded timeline, Simple Mode animation).

## Workflow gaps

- Perpetual license prices are high ($258 for EX version); subscription may be more economical for casual users.
- No dedicated pixel-art mode (like Krita).
- Learning curve for animation workflows, particularly for new users unfamiliar with frame-by-frame paradigms.
- Limited community compared to Photoshop or Krita.
- No rigged or skeletal animation in the base application.
- Keyboard shortcuts and UI organization differ significantly from Western animation software (Harmony, Moho), requiring relearning for those tools.

## Notable uses

Clip Studio Paint is the dominant animation tool in Japanese 2D animation studios. Used extensively for webtoon production, manga creation, and indie game sprite animation. Professional studios worldwide use it for sprite animation, backgrounds, and visual effects. The strong presence in Japanese game development (particularly in pixel art and 2D game titles) reflects its popularity in that region.

## Community and ecosystem

Active community in Japan and growing Western presence. Official forums, Discord, and YouTube channels with tutorials. Material Store offers brushes, 3D models, textures, and scripts. Large collection of animation tutorials in Japanese and English. Professional users share workflows and shortcuts. Integration discussions with game engines (Unity, Godot) common in game development communities.

## Pricing details

Clip Studio Paint offers both perpetual license and subscription options as of Ver. 4.0 (March 2025):

**Perpetual License**:
- EX version (full features): $258 one-time purchase.
- PRO version (limited features, suitable for illustrations): Lower perpetual price.
- Update Pass (optional): $31.99/year to maintain perpetual license with feature updates.

**Subscription Plans** (monthly/annual):
- EX version: $8.99/month or $71.99/year (first-time users get up to 3 months free).
- PRO version: $4.49/month or $24.99/year.

**Platforms**: Available on Windows, macOS, iPad, iPhone (limited), Android tablets, and Chromebooks. Pricing is per-platform; a single license does not cover all platforms.

**Free trial**: 30-day free trial available.

The perpetual license model allows artists to avoid subscriptions, though perpetual prices are higher than monthly subscription costs over time.
