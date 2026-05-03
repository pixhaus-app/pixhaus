# Krita Animation Timeline

## Quick facts
- Vendor / maintainer: Krita Foundation (open source)
- License / pricing model: Open source (LGPL v2); free for Windows/Linux, optional donation on macOS (via Steam)
- Price point (current): Free (donations accepted)
- Platforms: Windows, macOS, Linux
- First released: 2005 (animation timeline added in version 3.0, 2016)
- Last meaningful update: Continuous development (as of May 2026)
- Source available: Yes (GitHub: krita-foundation/krita)
- Primary use case: Digital painting and frame-by-frame animation for indie games and personal projects

## Origin and purpose

Krita is an open-source digital painting and drawing application. The animation timeline (introduced in Krita 3.0 in 2016) was added as a secondary feature to support frame-by-frame animation and sprite creation. Krita's primary strength is painting and digital art; animation is a bonus capability, not the core focus. For game developers, Krita offers an all-in-one tool: draw characters in the canvas, animate frame-by-frame using the timeline, and export sprite sheets directly. This integration makes Krita appealing to indie game developers and small studios.

## Drawing and painting tools

Krita is built around a sophisticated brush engine with extensive customization. Brushes support pressure sensitivity, tilt, rotation input from pen tablets. Bristle dynamics, texture blending, and wet-edge simulation reproduce real painting behavior. The software includes hundreds of pre-made brushes (pencils, markers, oils, watercolors, charcoal, digital paint, etc.). Artists can create custom brushes via texture and brush shape control. Color picker, gradient tools, and palette management are integrated. Anti-aliasing and brush smoothing prevent jagged edges.

Krita's painting quality rivals TVPaint and professionals use it for comic art and illustration. For game developers, Krita enables both character creation and animation in a single application.

## Animation timeline structure

The Animation Timeline docker displays frames horizontally (one column per frame) with layers vertically organized. Each layer has its own frame exposure. Keyframes are marked; regular frames show empty slots. The playhead scrubs across frames. Playback speed adjusts (12, 24, 30 fps typical). Navigation is frame-based. The timeline integrates with the layers panel, showing which frames contain drawings per layer.

## Frame-by-frame workflow (onion skin, lightbox, hold frames, blank frames)

Onion skin is central to Krita's animation workflow. The Onion Skin docker shows configurable previous and next frames as semi-transparent overlays. Colors are customizable (default: blue before, red after). Opacity per direction adjusts independently. Lightbox mode sets onion skin transparency extremely high for tracing. Hold frames (exposing the same drawing across multiple frames) reduce redraw work. Blank frames create gaps or transitions. The workflow is intuitive for traditional frame-by-frame animation: draw frame 1, onion skin shows frame 1, animator draws frame 2, etc.

## Tweening and interpolation

Tweening is not native in Krita's animation system. The software does not automatically interpolate between keyframes. All animation is frame-by-frame, hand-drawn. This aligns with Krita's philosophy: the animator controls every frame. For simple motion (object sliding, spinning), animators manually draw the in-between frames or use external tweening tools. This is labor-intensive but gives full control. Game developers using Krita for detailed character animation accept this trade-off for expressiveness.

## Rigging and deformation

Krita has no bone system or rigging tools. Cut-out animation (puppet-style) is not supported. Animation is purely frame-by-frame, hand-drawn. Deformation and warping are not available in the animation timeline. Game developers seeking rigged characters must use Moho, Harmony, or OpenToonz, not Krita.

## Vector vs raster

Krita is bitmap (raster)-based. All drawing creates pixels, not mathematical vectors. This means:

- Strokes are soft, painterly, and expressive (advantage)
- Scaling and rotating artwork degrades quality unless pre-scaled (disadvantage)
- Files grow large with many frames (disadvantage)
- Painterly effects are native (advantage)
- Crisp vector-style lines require careful drawing (disadvantage)

For game development, bitmap animation means sprite sheets will have soft, anti-aliased edges unless artists intentionally create crisp artwork.

## Color and palette workflow

Krita includes color palette management. Palettes can be created, imported, and managed. Per-frame color changes are supported by manually editing colors on frames. Gradient and transparency tools control color blending. The color science is adequate for game development. Palette consistency requires manual attention (unlike Harmony's palette linking).

## Layer system

Layers are displayed in the timeline with visibility, lock, opacity, and blend mode controls. Layers can be nested into groups. The animation timeline shows frame exposure per layer, critical for managing multi-part animation (head, body, limbs on separate layers). Naming and organization are essential for frame-heavy projects.

## Export and import (critical: which formats game devs actually use)

Krita exports options:

- **PNG sequence**: Frame-by-frame PNG files (one per frame)
- **Sprite sheet (native)**: Krita can export the animation timeline as a sprite sheet PNG with automatic grid layout. This is critical for game developers.
- **GIF**: Animated GIF for quick preview
- **WebP**: Modern web image format
- **TIFF sequence**: Professional format
- **MP4 / AVI**: Video formats (via render plugin)

For game developers:
- **Sprite sheet export is native and direct**: Krita's Animation Timeline Exporter creates a PNG sprite sheet from the timeline frames. Frame size, grid layout, and padding are configurable. This is a direct path to game engines (Unity, Godot, GameMaker, etc.).
- No need for external tool chaining (unlike Harmony, OpenToonz, or Synfig)
- This is Krita's major advantage for game sprite development

## Scripting and extensibility

Krita supports Python scripting via PyKrita. Scripts can automate animation generation, batch export, or customize the UI. The scripting API is documented but less mature than commercial tools. A modest third-party plugin ecosystem exists (e.g., custom exporters). Extending Krita requires Python knowledge.

## Engine integration

Krita integrates directly with game engines via sprite sheet export. The workflow is: Krita → draw and animate → export sprite sheet → game engine import. No external conversion needed. This is the smoothest integration for indie game developers among all tools considered.

## Workflow strengths

1. Free and open-source (no licensing costs)
2. All-in-one tool: painting + animation + sprite sheet export
3. Excellent onion-skin and frame-by-frame animation tools
4. Native sprite sheet export (critical advantage for game developers)
5. Professional-grade painting quality
6. Cross-platform (Windows, macOS, Linux)
7. No vendor lock-in; source code modifiable
8. Intuitive UI for traditional animators
9. Layer organization and blend modes well-integrated
10. Large community and abundant tutorials for painting; growing resources for animation

## Workflow gaps

1. No tweening or automatic interpolation (all animation is hand-drawn)
2. No rigging or bone system (frame-by-frame only)
3. Raster-based means scaling/rotation degrades quality
4. No sophisticated color palette management or per-scene palette swapping
5. Performance on very large projects (10,000+ frames) can be sluggish
6. Bitmap-heavy files require disk space and memory
7. No 3D integration or 3D camera
8. Limited motion graphics capabilities (no data visualization or procedural animation)
9. No network rendering for batch export

## Notable uses (especially game-related uses)

- **Indie 2D games**: Pixel art and hand-drawn sprite animation for indie games (popular on itch.io)
- **Pixel art games**: Krita's painting tools and sprite sheet export make it ideal for pixel art animation
- **Comic art and illustration**: Krita's painting tools are professional-grade
- **Educational animation**: Teaching animation and game art in schools and courses
- **Visual novel games**: Hand-drawn character animation for visual novels and adventure games

Game adoption is growing, particularly among indie developers. Krita is increasingly chosen for frame-by-frame sprite animation due to direct sprite sheet export and low cost (free).

## Community and ecosystem

- Active open-source community with dedicated contributors
- Extensive YouTube tutorials (particularly for painting; animation resources growing)
- User forums and Discord communities are active
- Animation subreddits discuss Krita increasingly
- Educational use in animation schools and game development courses
- Commercial training courses (Ctrl Paint, School of Motion) teach Krita
- No commercial support, but community is vibrant and helpful

## Pricing details

Krita is free under LGPL v2 license:

- **Windows and Linux**: Free (open source)
- **macOS**: Free (via GitHub) or optional donation through Steam ($19.99 USD or regional equivalent for the Steam version, which funds development)
- No subscription, no licensing restrictions
- Commercial use permitted
- Source code modifiable and redistributable under LGPL terms

The free nature of Krita makes it attractive to indie game developers. The modest Steam donation option (~$20) funds ongoing development while maintaining free access.
