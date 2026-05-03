# Synfig Studio

## Quick facts
- Vendor / maintainer: Synfig Foundation (community-driven open source)
- License / pricing model: Open source (GNU General Public License v3)
- Price point (current): Free
- Platforms: Windows, macOS, Linux
- First released: 2005
- Last meaningful update: Active development (as of May 2026)
- Source available: Yes (GitHub: synfig/synfig)
- Primary use case: Vector-based 2D animation with automatic tweening; indie and hobbyist projects

## Origin and purpose

Synfig Studio originated as an open-source alternative to commercial animation software. It targets vector-based animation workflows and is designed around "smart" tweening: animators define key poses, and Synfig automatically calculates in-between frames via mathematical interpolation. Unlike traditional frame-by-frame animation, Synfig reduces redraw work for simple movements. The software is free and open-source, appealing to students, hobbyists, and indie developers. Synfig is less powerful than Harmony or Toon Boom but suitable for web animation, indie games, and educational projects.

## Drawing and painting tools

Synfig provides basic vector drawing tools: pen, Bezier curves, shapes (rectangle, circle, polygon), and text. Strokes and fills are colored via a color picker. The software does not prioritize painting; it assumes artwork is pre-drawn and imported or created in Synfig using simple vector shapes. The brush system is minimal compared to TVPaint or Moho. Artists typically use Synfig for animation and rigging of imported vector artwork rather than creation. For game developers, artwork is usually created in Inkscape (vector graphics editor) and imported into Synfig for animation.

## Animation timeline structure

The timeline displays frames horizontally with layers vertically organized. Keyframes are marked; tweened frames are automatically generated and shown. The timeline integrates with the parameter system: animators can keyframe not just position but any parameter (scale, rotation, opacity, color). Navigation is frame-based with standard playhead control. Playback speed adjusts (12, 24, 30 fps). The interface emphasizes the parameter-driven approach rather than traditional layer-per-frame workflow.

## Frame-by-frame workflow (onion skin, lightbox, hold frames, blank frames)

Onion skin is supported but secondary to Synfig's primary tweening workflow. Frame-by-frame animation is possible but not optimized. The focus is on keyframe-based animation. Onion skin shows overlays of adjacent frames. Hold frames (duplicating a keyframe) are supported. However, artists using frame-by-frame animation in Synfig find the tool less intuitive than dedicated frame-by-frame tools (TVPaint, Harmony).

## Tweening and interpolation

This is Synfig's distinguishing feature. Rather than manual in-betweening, animators define key poses (keyframes) with properties (position, rotation, scale, opacity, color, custom parameters). Synfig automatically interpolates between keyframes using curves and mathematical functions. The "Action" system allows defining complex motion by keyframing parameters rather than drawing frames. This is dramatically faster than frame-by-frame animation for simple movements but limits expressiveness for detailed character acting.

Timing curves adjust easing (linear, ease-in, ease-out, custom). The workflow is: set keyframe 1 (pose A), move to frame 10, set keyframe 2 (pose B), and Synfig fills frames 2-9 automatically.

## Rigging and deformation

Synfig includes a bone system for cut-out animation and skeletal deformation. Bones are placed on artwork, and the skeleton controls movement. The system supports both forward and inverse kinematics. Squash-and-stretch deformation is supported. However, the bone system is less mature than Moho's. Character rigging in Synfig is possible but requires careful setup and understanding of the parameter system.

## Vector vs raster

Synfig is vector-based. All native drawing and animation operates on mathematical vector shapes. Bitmap artwork can be imported and animated but is treated as a static element. This means exported animations scale cleanly but require vector-sourced art for optimal results.

## Color and palette workflow

Color management is basic. A color picker is provided; colors are applied to strokes and fills. Per-frame color changes are supported by keyframing color parameters. There is no sophisticated palette system like Harmony or OpenToonz. Color consistency across scenes requires manual attention. For game developers, this is acceptable for simple sprite animation.

## Layer system

Layers are displayed in the timeline with visibility, lock, and blend mode controls. Layers can be nested. Special layer types include:

- **Shape layers**: Vector artwork
- **Group layers**: Organization
- **Import layers**: Imported external files
- **Bone layers**: Skeletal control
- **Effect layers**: Filters and distortions

The parameter system (distinct from layers) allows keyframing any property of any layer, providing flexibility for complex animation rigs.

## Export and import (critical: which formats game devs actually use)

Synfig exports options:

- **PNG sequence**: Frame-by-frame PNG files
- **AVI / MP4**: Video formats
- **WebP**: Modern web image format
- **OpenEXR**: Professional format with alpha channels
- **GIF**: Animated GIF for web

For game developers:
- Sprite sheet export is not native. Developers export PNG sequences and use external tools (ImageMagick, Aseprite, Texture Packer) to assemble into sprite sheets.
- Direct game engine export is absent. Workflow: Synfig → PNG sequence → external sprite sheet tool → game engine.

This overhead is similar to other non-game-focused tools. However, Synfig's automatic tweening means full animations can be created with fewer total frames, reducing sprite sheet size.

## Scripting and extensibility

Synfig supports C++ plugin development and has Python scripting capabilities in some contexts. The extension system allows custom tools and effects. However, extending Synfig requires programming knowledge. The plugin ecosystem is minimal; most users stick to built-in features. Community-shared scripts exist but are rare.

## Engine integration

No direct integration. Synfig produces video or image sequences. Game developers export PNG sequences and use external tools. The workflow is manual.

## Workflow strengths

1. Free and open-source (no licensing costs)
2. Automatic tweening dramatically reduces redraw work for simple animations
3. Parameter-based animation system is powerful for procedural motion
4. Suitable for web animation and indie projects
5. Cross-platform (Windows, macOS, Linux)
6. No vendor lock-in; source code is modifiable
7. Scalable vector export (clean at any resolution)
8. Learning curve is moderate for basic projects

## Workflow gaps

1. No sprite sheet export (requires external tool chaining)
2. Not suitable for detailed character animation (lacks expressiveness tools)
3. Frame-by-frame workflow is not optimized (use TVPaint or Harmony instead)
4. Rigging system is less polished than Moho's
5. UI is less intuitive than commercial tools
6. Performance on very large projects is slower than expected
7. Documentation is sparse; learning resources are limited
8. Community is smaller than commercial-tool communities
9. No professional support or training infrastructure
10. Bone system lacks constraints (pole vectors, aim, etc.)

## Notable uses (especially game-related uses)

- **Indie web games**: Simple animated web games using HTML5 Canvas or WebGL
- **Educational animation**: Teaching animation fundamentals with free software
- **Web animation**: Animated graphics for websites and online advertising
- **Fan projects**: Open-source animation and short films created by community members
- **Indie game cinematics**: Pre-rendered cutscenes for indie games (rarely, due to workflow overhead)

Game adoption is minimal. Synfig is used primarily for web animation and educational purposes, not game sprite animation.

## Community and ecosystem

- GitHub repository active with community contributions
- Documentation available but less comprehensive than commercial tools
- YouTube tutorials exist but are less abundant than for Harmony or Moho
- Animation subreddits occasionally discuss Synfig
- Community forums are active but smaller
- No commercial support; reliance on community help
- Open-source community values extensibility and modification

## Pricing details

Synfig is free, licensed under GNU General Public License v3 (GPLv3). No subscription, no pricing tiers. The software can be used for any purpose (commercial or non-commercial) and modified as needed. The trade-off is lack of professional support, polished UI, and abundant documentation. Support comes from community forums and GitHub issues.
