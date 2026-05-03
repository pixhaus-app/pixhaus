# Moho (Anime Studio)

## Quick facts
- Vendor / maintainer: Lost Marble, Inc.
- License / pricing model: Perpetual license (one-time purchase) with optional annual subscription for updates
- Price point (current): Moho Debut ~$60-80 USD; Moho Pro ~$180-250 USD (prices vary by region and promotions)
- Platforms: Windows, macOS
- First released: 2001 (originally Anime Studio)
- Last meaningful update: Version 13+ (ongoing updates)
- Source available: No
- Primary use case: Character animation via bone rigging; cut-out animation for games and animation

## Origin and purpose

Moho, originally branded as "Anime Studio," was created by Lost Marble to enable animators to rig characters with bones and animate through skeletal movement rather than frame-by-frame drawing. The software targets a middle ground between traditional frame-by-frame animation and full 3D character animation. Renamed to "Moho" in 2016 to broaden appeal beyond anime. The software is popular among indie game developers and animators who want faster character animation than hand-drawing every frame. Moho is used for web animation, indie games, and television series (less prestigious than Harmony or OpenToonz but adequate for time-constrained projects).

## Drawing and painting tools

Moho includes basic vector drawing tools: pen, brush, shapes (rectangle, circle, polygon), and eraser. Strokes and fills can be colored via a color picker. The software does not prioritize painting; it assumes artwork is pre-drawn elsewhere (Photoshop, Illustrator, Clip Studio Paint) and imported. Artists often sketch in Moho but refine art in dedicated software. Stroke smoothing and pressure sensitivity are supported for pen tablets. The focus is on rigging imported art, not creating art within the tool.

## Animation timeline structure

The timeline is organized by layers (rows) and frames (columns). Keyframes are marked; interpolated frames are shown as in-between frames. The playhead scrubs across time. Playback is frame-based, typical of animation software. The timeline integrates with the bone/puppet layer system, showing which bones are keyframed at each frame.

## Frame-by-frame workflow (onion skin, lightbox, hold frames, blank frames)

Moho supports onion skin for reference to previous/next frames, though this is secondary to the primary rigging workflow. The onion skin shows overlays of neighboring frames with adjustable opacity. This is useful when hand-drawing supplementary artwork or refining poses, but does not dominate the workflow as it does in frame-by-frame-focused tools. Hold frames and blank frames are supported; hold frames extend a pose (no new keyframe required on the next frame).

## Tweening and interpolation

Moho automatically interpolates between bone keyframes. When an animator sets the position of a bone (via skeleton manipulation) at frame 1 and frame 10, Moho calculates intermediate positions for frames 2-9. The interpolation curve can be adjusted for easing (linear, ease-in, ease-out, custom). This is the core efficiency gain: 8 frames of movement are automatically calculated, not hand-drawn. Inverse Kinematics (IK) and Forward Kinematics (FK) modes control how bones respond to keyframes, affecting interpolation style.

## Rigging and deformation

Rigging is the primary feature. Moho allows artists to place bones on artwork (vector or imported bitmap), define joints and articulation, and then pose the skeleton. The skeleton can be manipulated via bone handles or inverse kinematics targets. Key features:

- **Bone system**: FK (Forward Kinematics) and IK (Inverse Kinematics) chains. FK moves child bones relative to parent; IK allows setting a target position for the end of a chain, and parent bones automatically adjust.
- **Squash and Stretch**: Automatic deformation of bones, useful for cartoony character movement.
- **Constraints**: Pole vectors, aim constraints, and other controls refine bone behavior.
- **Pin bones**: Fixed points that do not move, used for anchoring limbs.
- **Smart bones**: Advanced control for complex articulations.

The workflow: import character artwork → place bones on joints → set keyframes for bone positions → interpolation fills in frames automatically. This is dramatically faster than hand-drawing every frame but requires pre-rigged artwork and careful bone placement.

## Vector vs raster

Moho handles both. Artwork can be vector (imported from Illustrator) or raster (bitmap, e.g., from Photoshop). Moho's rigging system applies to both equally. However, the software does not excel at creating vector art; artists typically import pre-drawn characters. Raster artwork (bitmap) requires careful layering to rig (each part of the body on a separate layer), making vector setup slightly easier.

## Color and palette workflow

Color management is minimal. Moho assumes colors are baked into imported artwork. Changing colors requires editing source art in Photoshop or similar, then re-importing. No sophisticated palette system like Harmony or OpenToonz. This is acceptable for indie game developers; TV studios would find it limiting.

## Layer system

Layers are displayed in the timeline. Each layer can be:

- **Drawing layer**: Imported artwork
- **Bone/Puppet layer**: The rigging structure for a character or object
- **Group layer**: Organizing related layers
- **Camera layer**: For panning/zooming

Visibility and lock controls are standard. Naming layers is essential for managing complex rigs (left arm, right arm, head, etc.). Hierarchy is straightforward.

## Export and import (critical: which formats game devs actually use)

Moho exports options:

- **QuickTime / MP4**: Video formats for preview or cinematic delivery
- **PNG sequence**: Frame-by-frame PNG files
- **AVI**: Windows video format (legacy)
- **Sprite sheets**: Moho can export rasterized animation as sprite sheets, a critical feature for game developers. Sprite sheet settings include frame size, layout (grid), and padding. This is a direct path to game engines.

For game developers, the sprite sheet export is valuable. Animators rig characters in Moho, play the animation, and export a PNG sprite sheet suitable for Unity, Godot, or any engine that reads sprite sheets. This is faster than frame-by-frame drawing but still requires careful animation setup.

## Scripting and extensibility

Moho supports Lua scripting for automation and custom tools. Scripts can batch-process projects, generate animations, or create custom effects. The Lua API is documented and allows plugin creation. However, scripting requires programming knowledge, limiting accessibility. The third-party plugin ecosystem is modest but exists (e.g., custom deformers, export tools).

## Engine integration

No direct integration. Moho produces video or sprite sheets. Game developers import sprite sheets into Unity, Godot, GameMaker, or similar engines. The workflow is: Moho → rig character → animate → export sprite sheet → game engine. This is simpler than Harmony or OpenToonz (which require external sprite sheet conversion) but more involved than Aseprite (which is designed for sprite game workflows).

## Workflow strengths

1. Fast character animation via rigging (dramatically faster than frame-by-frame for simple characters)
2. Sprite sheet export native (suitable for game integration)
3. Intuitive bone system; artists can learn rigging relatively quickly
4. Inverse Kinematics make natural-looking limb motion easy
5. Squash and stretch deformation adds cartoony appeal
6. One-time purchase option (Moho Debut); no forced subscription
7. Lower cost than Harmony or Toon Boom suite
8. Suitable for indie game developers and web animation studios
9. Active community with tutorial abundance

## Workflow gaps

1. Not suitable for detailed character performance (facial expressions, subtle acting) — character is too simple/puppet-like
2. Rigging requires careful setup; mistakes are difficult to correct mid-project
3. No sophisticated in-betweening for organic movement (bone interpolation is mechanical)
4. Limited color/palette management (requires external tool changes)
5. Onion skin and frame-by-frame tools are secondary, not primary
6. No network rendering or batch export optimization for large projects
7. Cannot compete with hand-drawn animation in expressiveness or appeal

## Notable uses (especially game-related uses)

- **Web animation**: Moho is popular for animated web banners and online ads
- **Indie 2D games**: Character animation in adventure games, RPGs, and action games (e.g., simple platformer characters)
- **Educational animation**: Explainer videos and educational content (rigging makes repetitive motion easy)
- **TV series**: Some animated series use Moho for character animation, typically on lower budgets than Harmony productions
- **Stop-motion substitute**: Moho is used to create effects similar to puppet/stop-motion animation

Game examples are difficult to cite specifically (developers rarely publicize which tools were used), but Moho is favored for indie 2D games where production time and budget are limited and character complexity is moderate.

## Community and ecosystem

- Active forums and user community
- YouTube tutorials abundant and beginner-friendly
- Third-party resources and rigs shared on platforms like Gumroad
- Animation subreddits discuss Moho regularly
- No formal certification program, but the learning curve is accessible
- Hobbyist to professional adoption across the spectrum

## Pricing details

### Moho Debut
- Approximately $60-80 USD (one-time purchase; varies by region)
- Perpetual license (no subscription required)
- Includes basic bone rigging, animation, and sprite sheet export
- Ideal for hobbyists and indie game developers

### Moho Pro
- Approximately $180-250 USD (one-time purchase; varies by region)
- Perpetual license with optional annual subscription for updates (updates are optional)
- Adds advanced bone constraints, scripting API, Lua extensibility, and professional export options
- Ideal for professional studios and game developers requiring advanced rigging

### Update Subscription
- Optional annual subscription for major updates (price varies)
- Can continue using current version without subscription
- Unlike Harmony or Animate, users are not forced to subscribe

Regional pricing: EUR, GBP, CAD, and other currencies available. Promotional discounts (25-50% off) are common during sales events. Educational licenses available at discounted rates.

**Key advantage**: One-time purchase option means users can own the software perpetually, avoiding ongoing subscription costs (unlike Harmony or Animate, which require continuous subscription for any updates).
