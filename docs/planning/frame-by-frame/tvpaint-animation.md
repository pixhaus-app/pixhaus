# TVPaint Animation

## Quick facts
- Vendor / maintainer: TVPaint Development SARL
- License / pricing model: Perpetual license (one-time purchase)
- Price point (current): Standard €500 (professional), €325 (student); Professional €1,250 (professional), €650 (student)
- Platforms: Windows, macOS
- First released: 1998
- Last meaningful update: Version 11+ (ongoing updates)
- Source available: No
- Primary use case: Bitmap-first traditional 2D animation; frame-by-frame animation for film, TV, and indie games

## Origin and purpose

TVPaint is a specialized tool for traditional animation built on bitmap (raster) technology. Unlike vector-first tools (Animate) or hybrid tools (Harmony), TVPaint prioritizes digital painting and drawing to replicate the hand-drawn animation experience. Developed in France, TVPaint has cultivated a dedicated user base among traditional animators, particularly in European studios. The software targets artists who value painterly control and hand-drawn expressiveness over vector efficiency. TVPaint is less well-known than Harmony in English-speaking markets but commands loyalty among professional independent animators and smaller studios.

## Drawing and painting tools

TVPaint is built around a powerful brush engine with extensive customization. Brushes support pressure sensitivity, tilt, and rotation input from pen tablets. Bristle dynamics, texture blending, and wet-edge simulation reproduce real painting behavior. The software includes hundreds of pre-made brushes (pencils, markers, oils, watercolors, chalk, charcoal). Artists can create custom brushes via texture and shape control. Color picker and gradient tools are available. Layers support blend modes (multiply, screen, overlay, etc.). Anti-aliasing is built-in.

The philosophy is that TVPaint is a digital canvas optimized for drawing, not a production pipeline tool. This is the core differentiator from Harmony or Toon Boom.

## Animation timeline structure

The timeline displays frames horizontally with layers vertically organized. The "Exposure Sheet" (Xsheet-equivalent) shows frame numbers and layer assignments. Playback is frame-based with adjustable speed (12, 24, 30 fps standard). The playhead scrubs across frames. Onion skin is tightly integrated into the timeline, making it a central interaction point. Unlike Harmony, which separates timeline and Xsheet views, TVPaint unifies them, reducing UI complexity.

## Frame-by-frame workflow (onion skin, lightbox, hold frames, blank frames)

Onion skin is core to TVPaint. The software excels at configuring onion skins: previous and next frames displayed as semi-transparent overlays with independent opacity per direction. Colors are customizable (default: blue before, red after). The depth (how many frames to show) adjusts per frame. Lightbox mode sets onion skin opacity to maximum transparency for tracing over previous drawings. This is where TVPaint shines compared to other tools.

Hold frames extend a single drawing across multiple frames without redraw. Blank frames create gaps. The workflow is intuitive: animators draw frame 1, then frame 2 with onion skin guiding their lines, etc. The UI prioritizes this over scripting or tweening.

## Tweening and interpolation

Tweening is supported but secondary. Motion paths allow objects to move along curves, and timing curves control acceleration. Shape morphing (one shape transforming into another) is possible. However, the emphasis is on hand-drawn animation, not automatic interpolation. TVPaint lacks the sophisticated in-betweening found in Harmony. Animators who use TVPaint typically prefer to draw every frame or every other frame (twos).

## Rigging and deformation

Rigging is absent. TVPaint has no bone system. Cut-out animation (puppet-style) is not supported. This is a deliberate choice to focus on traditional, hand-drawn workflows. Game developers seeking rigged characters must use Moho, Harmony, or OpenToonz. For TVPaint users, character animation is entirely frame-by-frame, hand-drawn.

## Vector vs raster

TVPaint is purely bitmap (raster)-based. All drawing creates pixels, not mathematical vectors. This means:

- Strokes are soft, painterly, and expressive (advantage)
- Scaling and rotating artwork degrades quality unless pre-scaled (disadvantage)
- Files grow large with many frames (disadvantage)
- Painterly effects are native (advantage)
- Vector-style crisp lines are difficult (disadvantage)

For game development, the raster approach means sprite sheets will have soft edges unless artists specifically create crisp artwork.

## Color and palette workflow

TVPaint includes color palette management. Palettes can be created, imported, and swapped. Per-frame palette changes allow costume or lighting variations without redrawing. Indexed color modes (limited palette) are supported for retro-style games or optimization. Gradient and transparency tools control color blending. The color science is adequate but not as sophisticated as Harmony's multi-character palette linking.

## Layer system

Layers are displayed in the timeline with visibility, lock, opacity, and blend mode controls. Layers can be organized into groups (folders). Special layer types include:

- **Paint layers**: Standard bitmap drawing
- **Vector layers**: Limited vector drawing (TVPaint's secondary feature)
- **Group layers**: Organization
- **Effect layers**: Compositing effects

Most workflow uses paint layers. The hierarchy is straightforward.

## Export and import (critical: which formats game devs actually use)

TVPaint exports options:

- **PNG sequence**: Frame-by-frame PNG files, standard for downstream use
- **AVI / MP4**: Video formats for preview or delivery
- **QuickTime**: Professional video format
- **PSD (Photoshop)**: One file per frame or all frames in one stack
- **TIFF sequence**: Professional image format
- **GIF**: Animated GIF for web sharing

For game developers:
- Sprite sheet export is not native. Artists must export PNG sequences and use external tools (Aseprite, Texture Packer, ImageMagick) to assemble into game-ready sprite sheets.
- Direct game engine export is absent. The workflow is: TVPaint → PNG sequence → external sprite sheet tool → game engine.

This is a significant overhead, similar to Harmony and OpenToonz. However, TVPaint's lack of rigging means the entire animation must be frame-complete before export (no skeletal interpolation to fill frames), making sprite sheet assembly more straightforward (all frames are final).

## Scripting and extensibility

TVPaint supports scripting via LUA. Scripts can batch-process projects, automate frame generation, or customize the UI. The scripting API is documented but less comprehensive than Moho or Harmony. Third-party plugins are rare; most users stick to built-in features. Extending TVPaint requires programming knowledge.

## Engine integration

No direct integration. TVPaint produces video or image sequences, not sprite sheets. Game developers export PNG sequences and use external tools to create game-ready assets. The workflow is manual and time-consuming compared to sprite-focused tools like Aseprite.

## Workflow strengths

1. Excellent onion-skin and frame-by-frame animation tools (arguably the best among commercial tools)
2. Brush engine is powerful and expressive; painting feels natural
3. Perpetual license; no forced subscription
4. Lower cost than Harmony when purchased outright
5. UI is less cluttered than Harmony; more intuitive for traditional animators
6. Layer blend modes and effects are well-integrated
7. Suitable for independent animators and small studios
8. Export to PNG sequences is straightforward for external processing

## Workflow gaps

1. No sprite sheet export (requires external tool chaining)
2. No rigging or cut-out animation support (frame-by-frame only)
3. Raster-based means scaling and rotation degrade quality
4. No sophisticated in-betweening or motion tweening
5. Performance on very large projects (10,000+ frames) can be sluggish
6. Bitmap-heavy files require disk space and memory
7. No 3D integration or 3D camera
8. Color palette management is less powerful than Harmony's
9. Plugin ecosystem is minimal

## Notable uses (especially game-related uses)

- **Independent animators**: TVPaint is popular among freelance traditional animators creating animation for ads, shorts, and independent projects
- **Indie game animation**: Some indie developers use TVPaint for hand-drawn character animation, though sprite sheet integration is manual
- **Educational animation**: Animation schools use TVPaint for teaching traditional animation techniques
- **TV animation**: Some European studios use TVPaint for episodic animation, though Harmony dominates in English-speaking markets

Game adoption is lower than Harmony or Moho due to lack of rigging and sprite sheet export. Indie game developers using TVPaint typically hire specialized artists or use the tool for cinematics that are pre-rendered offline.

## Community and ecosystem

- Active user community, particularly in Europe and among traditional animators
- YouTube tutorials and online courses available
- User forums and Discord communities discuss TVPaint workflows
- Commercial training services offer TVPaint courses
- Documentation is adequate but less comprehensive than Harmony
- Third-party plugin ecosystem is minimal

## Pricing details

TVPaint uses a one-time purchase model with optional annual updates:

### Standard Edition
- Professional: €500 (approximately $590 USD at current rates)
- Student: €325 (approximately $385 USD; requires student verification)
- One-time purchase; no subscription required
- Updates are optional (can remain on current version indefinitely)

### Professional Edition
- Professional: €1,250 (approximately $1,480 USD)
- Student: €650 (approximately $770 USD)
- One-time purchase; no subscription required
- Includes advanced features, network rendering, and priority support

### Update Subscription (Optional)
- Annual subscription available for major version updates
- Not required; users can maintain current version indefinitely

Regional pricing varies (USD, GBP, CAD variants available). Educational discounts require verification. Promotional pricing during sales events is common.

**Key advantage**: Perpetual ownership with optional updates, no forced subscription. This appeals to independent animators on tight budgets.
