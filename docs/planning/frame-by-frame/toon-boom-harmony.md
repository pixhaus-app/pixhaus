# Toon Boom Harmony

## Quick facts
- Vendor / maintainer: Toon Boom Animation (Autodesk subsidiary)
- License / pricing model: Subscription (monthly/annual)
- Price point (current): Essentials $28.50/month, Advanced $71/month, Premium $129.50/month (USD)
- Platforms: Windows, macOS
- First released: 1998 (original Toon Boom Studio)
- Last meaningful update: Continuous updates (as of May 2026)
- Source available: No
- Primary use case: High-end traditional 2D animation for television, film, and indie games

## Origin and purpose

Toon Boom Harmony originated from research into digital animation workflows at the University of British Columbia. The software entered the market in 1998 and became the industry standard for television animation production. Adopted by Disney, Warner Bros., and studios worldwide, Harmony dominates high-end frame-by-frame animation. The "Essentials" tier (formerly "Setup") introduced in recent years expanded access to student animators and indie developers, though the learning curve remains steep. Harmony is synonymous with professional hand-drawn animation in entertainment, particularly for television series and feature films with demanding animation schedules.

## Drawing and painting tools

Harmony offers both vector and bitmap drawing within a single environment. The Brush tool supports pressure-sensitive pen input with extensive customization: bristle dynamics, wet edge simulation, and texture blending. Colors are managed through swatches and can be organized by character or scene. The palette management system allows import of color palettes from Photoshop or external files, crucial for maintaining consistent character colors across episodes. Pencil tools create rough outlines; brush tools refine them. The eraser and layer blending modes support transparency. Anti-aliasing and brush smoothing prevent jagged edges on small strokes.

Unlike pure vector tools, Harmony treats bitmap strokes and vector strokes as equivalent, bridging traditional and digital workflows. This flexibility is central to Harmony's professional appeal.

## Animation timeline structure

The timeline presents frames horizontally across the top, with layers listed vertically. Each frame position is marked; the current frame is highlighted. Layer properties display exposure (hold frames), rotation, and transformations. The Xsheet (exposure sheet) is visible alongside or in a separate panel, showing frame numbers and which frames contain drawings. Navigation is frame-based with clear visual feedback. Playback scrubbing is smooth and instantaneous, critical for animators working at 12, 24, or 30 fps.

## Frame-by-frame workflow (onion skin, lightbox, hold frames, blank frames)

Onion skin is essential and highly configurable. Animators can display 1-10 previous and next frames as colored overlays (typically blue for before, red for after, orange for several frames back). The onion skin opacity adjusts independently per direction. Lightbox mode uses extreme onion skin transparency to trace over linework without erasing. Hold frames (exposure value > 1) repeat the same drawing across multiple frames, reducing redraw work. This is standard practice in TV animation where limited budgets require animation on twos or threes (one drawing per 2-3 frames). Blank frames create gaps or visual breaks. Harmony's hold-frame system is optimized for production efficiency.

## Tweening and interpolation

Harmony supports motion tweening: position, rotation, scale, and skew properties can be automatically interpolated between keyframes. The timing editor shows curves for easing control (linear, ease-in, ease-out, custom curves). Harmony excels at "inbetweening" — the process of automatically generating intermediate frames between a start and end pose. However, Harmony users emphasize that tweening is supplementary; the majority of professional animation still relies on frame-by-frame work for character performance. Automatic inbetweening is useful for background movement or simple mechanical motion, not for facial expression or body language nuance.

## Rigging and deformation

Harmony includes inverse kinematics (IK) bone rigging for cut-out style animation. Bones can be pinned to vector or bitmap shapes; joints define articulation points. Constraints (pole vector, aim, etc.) control bone behavior. Forward kinematics (FK) and inverse kinematics (IK) modes are switchable per bone chain. Squash and stretch can be applied per bone. However, Harmony's rigging is secondary to frame-by-frame animation. Cut-out animation (puppet-style) is an option for production, not the default. Professional studios use rigging for mouth shapes or simple movements, but the bulk of animation is hand-drawn, frame-by-frame.

## Vector vs raster

Harmony is hybrid. The Drawing Tool creates vector strokes; the Bitmap tool creates raster pixels. A single frame can mix both. Animators often use vector for clean linework and bitmap for texture or quick sketches. Both stroke types can be animated simultaneously on the same layer, providing flexibility. No forced commitment to one or the other, unlike pure vector (Animate) or pure bitmap (TVPaint) tools.

## Color and palette workflow

The Palette docker displays swatches. Colors are applied by selection. The Advanced Palette system allows per-character or per-scene color sets, crucial for episodes with multiple characters. Animators can swap palettes per frame for costume changes or lighting variations. The Palette API in Harmony Advanced/Premium tiers allows scripting of palette operations, useful for batch color corrections across scenes. RGB, HSV, and CMYK color spaces are supported. Export to Photoshop palette format (ASE) is standard.

## Layer system

Layers are displayed as rows in the timeline/Xsheet. Each layer has visibility toggle, lock, blend mode, and opacity controls. Layers can be nested (groups). Timeline tracks separate drawing layers from effect layers. Master control layers allow animating multiple child layers together (used for camera pan or group movement). Layer visibility can be toggled per frame, useful for layered character rigs or changing scene elements.

## Export and import (critical: which formats game devs actually use)

Harmony exports are primarily for downstream post-production, not direct game integration:

- **QuickTime / ProRes**: Video format for editorial review and delivery
- **PNG sequence**: Frame-by-frame PNG files (one per drawing), standard for visual effects pipelines
- **EXR (OpenEXR)**: High-fidelity format with alpha channels, used in professional compositing
- **PSD (Photoshop)**: Layer-by-layer export, one file per frame or all frames in one stack

For game developers:
- Sprite sheets are possible but not automatic. Developers must export PNG sequences and use external tools (Aseprite, Texture Packer) to assemble sprites into game-ready sheets.
- Direct game engine export is absent. Integration requires post-processing.

This is a critical difference from Animate or Aseprite: Harmony is optimized for film/TV pipelines where scenes are rendered to final video, not for game sprite systems. Game developers using Harmony (rare) typically hire artists who deliver completed animation sequences, which are then pre-rendered to sprite sheets offline.

## Scripting and extensibility

Harmony Advanced and Premium tiers include API access via JavaScript for automation. Python scripts are supported in some contexts. Animators can write custom tools to batch-process layers, apply color corrections, or export to custom formats. The community shares scripts for routine tasks. However, scripting requires programming knowledge, limiting accessibility. Open-source plugins are rare; most extensions are proprietary scripts shared among studios.

## Engine integration

No direct integration with game engines. Harmony serves as upstream production. Animation is exported as video or image sequences, then pre-rendered to sprite sheets for game import. Workflow: Harmony → final composite (After Effects or Nuke) → video render → sprite sheet extraction (external tool) → game engine.

Indie developers using Harmony typically work with animation studios or freelancers who deliver final sprite sheets, not who use Harmony directly in the game pipeline.

## Workflow strengths

1. Industry-standard for professional animation (TV, film)
2. Unmatched onion-skin and hold-frame systems optimized for production schedules
3. Hybrid vector/bitmap drawing in single environment
4. Inbetweening and timing controls are sophisticated
5. Batch processing and scripting for production pipelines
6. Color palette management and swapping highly developed
7. Network rendering for batch exports (Premium tier)
8. Stable, mature software with decades of proven workflow

## Workflow gaps

1. No built-in sprite sheet export (requires external tool chaining)
2. Rigging system is supplementary, not primary
3. No 3D integration (unlike modern software)
4. UI is dense and challenging for beginners
5. No AI-assisted in-betweening or motion generation (as of 2026)
6. Slow real-time playback on very high frame-count projects (10,000+ frames)
7. No built-in publishing to web or game engines

## Notable uses (especially game-related uses)

- **Cuphead (2017)** and **The Cuphead Show! (animated series)**: Allegedly used Harmony for its rubber-hose animation style, though full pipeline included custom workflow. The show's frame-by-frame 1930s-inspired animation is a marquee example.
- **TV animation**: SpongeBob SquarePants, Adventure Time, Gravity Falls, and hundreds of other animated series use Harmony as the standard tool.
- **Indie games**: Rarely. Too expensive and overkill for most indie projects. Developers prefer Aseprite or Krita.
- **Feature animation**: Studios use Harmony for pre-production or specific sequences; final compositing in Nuke or After Effects.

Game adoption is minimal compared to Animate or Aseprite. Harmony is the tool of animators working in studio environments; indie game developers almost never work in Harmony directly.

## Community and ecosystem

- Toon Boom Learn Portal and official documentation comprehensive but dense
- Student community active (education licensing subsidizes learning)
- Third-party plugins minimal (ecosystem is proprietary)
- YouTube tutorials abundant but skew professional/studio-focused
- Animation subreddits and Discord communities discuss Harmony workflows
- Commercial training courses (Bloop Animation, Animation Mentor) teach Harmony

## Pricing details

### Harmony Essentials (formerly Intro)
- $28.50 USD/month (subscription)
- Annual option: approximately $252/year
- Includes basic drawing, painting, and frame-by-frame tools
- No Xsheet, limited scripting
- Designed for hobbyists and entry-level animators

### Harmony Advanced
- $71 USD/month (subscription)
- Annual option: approximately $640/year
- Adds Xsheet, motion tweening, rigging, color palette API
- Designed for students and professional freelancers
- Most animators use this tier

### Harmony Premium (Professional)
- $129.50 USD/month (subscription)
- Annual option: approximately $1,320/year (or roughly $1,180 with 3-year commitment at reported $3,540)
- Adds network rendering, broadcast output, enterprise features
- Used by studios with large pipelines
- Rarely necessary for indie developers

Regional pricing varies (EUR, GBP, CAD variants). Student discounts available with verification.
