# OpenToonz

## Quick facts
- Vendor / maintainer: Dwango Co., Ltd. (originally); community maintenance via GitHub
- License / pricing model: Open source (New BSD License)
- Price point (current): Free
- Platforms: Windows, macOS, Linux
- First released: 2018 (open-source release; original Toonz dates to 1998 by Digital Video S.p.A.)
- Last meaningful update: Active development (as of May 2026)
- Source available: Yes (GitHub: opentoonz/opentoonz)
- Primary use case: Production-quality 2D animation for film and television; emerging use in indie game animation

## Origin and purpose

OpenToonz is the open-source version of Toonz, proprietary animation software originally developed by Digital Video S.p.A. in Italy and used in Ghibli studios' production pipeline since the early 2000s. Toonz was customized extensively by Studio Ghibli for their needs and became synonymous with hand-drawn feature-film animation. In 2018, Dwango Co., Ltd. released OpenToonz as free, open-source software under the New BSD License, enabling artists worldwide to access production-grade animation tools without licensing costs.

OpenToonz is less known than Harmony outside animation circles but is functionally comparable in power. Its primary value proposition is cost (free) and openness (source code modifiable). Unlike Animate or Harmony, OpenToonz is chosen by animators and studios committed to open-source workflows or unable to afford commercial software.

## Drawing and painting tools

OpenToonz supports both vector (Toonz Vector) and raster (bitmap) drawing in a unified workspace. The Brush tool offers pressure sensitivity and customizable bristle dynamics. The Pencil tool creates rough, sketch-like strokes. Eraser, paint bucket, and eyedropper tools are standard. Stroke color and fill color are controlled via separate swatches. The stroke editor adjusts thickness, softness, and anti-aliasing per brush. Textures can be applied to brushes for varied appearance.

Unique to OpenToonz is the **antialiasing vector output**, which produces clean, crisp lines suitable for feature animation. The software handles both hand-drawn input (via tablet) and imported image sequences, making it compatible with traditional workflows where animators draw on paper, scan, and ink/paint digitally.

## Animation timeline structure

The timeline displays frames horizontally with layers vertically organized. The Xsheet (exposure sheet) view shows frame numbers and drawing assignments per layer, critical for understanding animation hierarchy. The multiplane camera system visualizes depth across layers. Navigation is frame-based with standard playhead scrubbing. Playback speed is adjustable (12, 24, 30 fps common). The timeline integrates directly with rendering options, allowing per-frame control over output.

## Frame-by-frame workflow (onion skin, lightbox, hold frames, blank frames)

Onion skin is highly configurable. Animators can display previous and next frames with customizable colors (default: blue before, red after) and opacity levels. The onion skin depth (how many frames back/forward) is adjustable per frame. Lightbox mode sets onion skin transparency extremely high for tracing. Hold frames (field exposure extending a drawing across multiple frames) are core to production efficiency. Animation is typically done on twos (one new drawing per 2 frames, typical for TV) or threes (one per 3 frames, less common but used for limited animation). Blank frames create gaps or transitions.

The Xsheet view is integral to hold-frame workflow; it displays which drawing is exposed on which frame across the entire shot, essential for coordinating with other animators on a team.

## Tweening and interpolation

OpenToonz supports motion tweening between keyframes for position, rotation, scale, and perspective transformation. The timing curve editor allows custom easing. Automatic in-betweening (motion interpolation between keyframes) is supported but not emphasized; frame-by-frame animation dominates professional workflows. Tweening is used for camera movement, simple object motion, or supplementary effects. Character animation relies on hand-drawn frames.

The software integrates with vector tweening tools for morphing shapes, less common than frame-by-frame in traditional animation.

## Rigging and deformation

OpenToonz includes a skeleton (bone) system for cut-out animation and character deformation. Bones are placed on vector or bitmap artwork and define joints. The system supports both forward kinematics (FK) and inverse kinematics (IK). Constraints and pin bones control articulation. Squash-and-stretch deformation applies to bones, useful for cartoony movement.

However, rigging is a secondary workflow. Professional Ghibli-style animation is almost entirely frame-by-frame, hand-drawn. Rigging is employed for background movement, repeating cycles (walking loops), or supplementary puppet animation. The core animation methodology is traditional: one artist per character, drawing each frame.

## Vector vs raster

OpenToonz is genuinely hybrid. The Toonz Vector engine creates mathematical vector outlines. The bitmap engine (Raster) works on pixels. A single animation can mix both seamlessly. Animators often sketch in raster (rough, fast) and clean up in vector (crisp, production-quality). This flexibility is a strength for production workflows where speed and quality must coexist.

## Color and palette workflow

Color palettes are managed via the Palette docker. Colors are organized per character or scene. Palette entries can be swapped or adjusted globally, useful for night/day lighting changes across an entire shot or episode. The color picker supports RGB, HSV, and other models. Palette import/export uses standard formats (Ghibli's custom palette files also supported). Advanced palette linking allows one palette change to propagate across multiple scenes or drawings.

## Layer system

Layers are displayed in the timeline with visibility, lock, and blend mode controls. Layers can be nested into groups (folders). Special layer types include:

- **Drawing layers**: Hold animated artwork
- **Pegbar layers**: Virtual camera control for layer positioning
- **Effect layers**: Apply compositing or visual effects
- **Camera layer**: 3D camera control for multiplane effects

The layer stack is hierarchical and allows complex composition of multiple animation elements.

## Export and import (critical: which formats game devs actually use)

OpenToonz exports are primarily for film/TV pipelines:

- **QuickTime / ProRes**: Video format for editorial and delivery
- **PNG sequence**: Frame-by-frame PNG files (one per frame), standard for downstream compositing
- **EXR (OpenEXR)**: Professional format with alpha channels, used in Nuke and other compositing software
- **TGA (Targa)**: Legacy format, sometimes used in studios
- **AVI**: Windows video format (legacy)

For game developers:
- Sprite sheet export is not native. Developers must export PNG sequences and use external tools (ImageMagick, Aseprite, Texture Packer) to assemble into game-ready sheets.
- No direct game engine integration. The workflow is: OpenToonz → PNG sequence → external sprite sheet tool → game engine.

This limitation is the same as Harmony: both tools are designed for film/TV output (video), not game sprites. Game adoption is rare.

## Scripting and extensibility

OpenToonz supports script extensions via C++ plugins. Python support is emerging in newer versions. The open-source nature allows community contributions and custom tool development. However, extending OpenToonz requires C++ or scripting knowledge, limiting accessibility compared to tools with visual scripting or no-code customization. The community shares plugins for common tasks (e.g., frame range copy, batch export), but the plugin ecosystem is much smaller than proprietary software communities.

## Engine integration

No direct integration. OpenToonz produces video or image sequences, not sprite sheets. Game developers would need to export to sequences and use external tools to create game-ready assets. This is a significant workflow overhead for game development, one reason why game developers prefer Aseprite, Krita, or sprite-focused tools.

## Workflow strengths

1. Free and open-source: no licensing costs
2. Production-grade quality comparable to Harmony (both originate from similar codebases)
3. Multiplane camera and 3D depth capabilities
4. Excellent onion-skin and Xsheet workflow for TV/film production
5. Hybrid vector/bitmap drawing
6. Network rendering for large projects (can be set up on multiple machines)
7. Cross-platform (Windows, macOS, Linux)
8. Community-driven development and customization potential
9. No vendor lock-in: source code is modifiable and redistributable

## Workflow gaps

1. No built-in sprite sheet export (major limitation for game developers)
2. UI is complex and less polished than commercial software
3. Documentation is sparse compared to Harmony; community resources are limited
4. Performance on very large projects (100,000+ frames) is slower than expected
5. Rigging system is less sophisticated than modern skeletal animation tools
6. No cloud synchronization or team collaboration features
7. Plugin ecosystem is minimal; extending the software requires C++ knowledge
8. Learning curve is steep for beginners (requires familiarity with professional animation pipelines)

## Notable uses (especially game-related uses)

- **Studio Ghibli**: The historical use case. OpenToonz was customized for films including Princess Mononoke, Spirited Away, The Boy and the Heron. These are the marquee examples.
- **Indie game animation**: Emerging but rare. Some indie developers use OpenToonz for cinematic sequences, then pre-render to sprite sheets. Example: niche projects in visual novel or adventure game communities.
- **Fan animation**: Open-source community has produced fan films and original shorts using OpenToonz, demonstrating capability.
- **Educational use**: Animation schools and universities adopting OpenToonz for curriculum, given cost-free licensing.

Game adoption remains minimal compared to Harmony (which is also low). OpenToonz is not a game animation tool; it is a film/TV animation tool that happens to be free and open.

## Community and ecosystem

- GitHub repository active; contributors are primarily Dwango and community volunteers
- OpenToonz documentation on GitHub and ReadTheDocs is improving but remains incomplete
- User forums exist but are smaller than commercial-tool communities
- YouTube tutorials exist, skewing professional/film-focused
- Animation subreddits mention OpenToonz; Mastodon communities discuss open-source animation
- Commercial training is scarce; most learning is self-directed or community-shared

## Pricing details

OpenToonz is free, licensed under the New BSD License. No subscription, no pricing tiers, no licensing restrictions. Suitable for commercial use, non-commercial use, and modification. The trade-off is that professional support, training, and polish lag behind commercial tools. No official customer support; community forums and GitHub issues are the primary support channels.
