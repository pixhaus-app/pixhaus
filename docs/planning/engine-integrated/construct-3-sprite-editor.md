# Construct 3 Sprite Editor and Animation System

## Quick facts
- Vendor / maintainer: Scirra
- License / pricing model: Subscription (free tier available)
- Price point (current): Free (limited), $99/year (personal), $300/year (business)
- Platforms: Browser-based (works on Windows, macOS, Linux)
- First released: 2015 (Construct 3)
- Last meaningful update: 2024 (continuous monthly updates)
- Source available: No (proprietary)
- Primary use case: No-code sprite animation and frame-based character assembly

## Origin and purpose

Construct 3 is a browser-based game engine designed for rapid prototyping and no-code workflows. The integrated sprite and animation editor reflects this philosophy: artists and non-programmers should be able to create animated characters without touching code.

The Sprite Editor and Animations Editor are tightly woven into the design, allowing frame import, layout, and animation definition entirely within the browser.

## Sprite / drawing capabilities

Construct 3 has no pixel editor. Sprites are imported as image files (PNG, JPG, SVG supported).

The Sprite Editor provides:

- **Image point editing** — Define the origin (pivot) and multiple attachment points per sprite (for weapon sockets, VFX, limb connection).
- **Collision polygon editing** — Define per-frame collision shapes (polygon-based, not just rectangular).
- **Animation frame organization** — Import frames individually or from spritesheets, arrange them into named animations.
- **Per-frame properties** — Set duration (in milliseconds or frames), optional speed curve.
- **Bounding box visualization** — See collision and rendering boundaries.

Unlike GameMaker or Unity, Construct 3's image point system is visual and intuitive; you click to place points directly on the sprite.

## Animation system

Construct 3 uses a straightforward flipbook system:

**Animations** — Each Sprite object contains one or more named animations. Each animation is a sequence of frames with per-frame timing.

- Simple playback: `sprite.play("walk")`.
- Looping, ping-pong, and one-shot modes.
- Frame rate control (frames per second or milliseconds per frame).

**No state machine** — Unlike Unreal or Godot's AnimationTree, Construct 3 does not have hierarchical animation blending or condition-based transitions. You switch animations via action (code-like events).

**Events system** (no-code)** — Construct 3's event system allows logic like:
- On collision, play "hit" animation.
- If key pressed, play "walk" animation.
- After animation finishes, play "idle".

This replaces traditional programming; events are visual nodes.

## Pivot, slicing, and atlas tools

**Pivots (image points)** — Set visually in the Sprite Editor. Main image point (origin) is the rotation/drawing center. Additional image points are for attachment (weapon hands, etc.).

**Slicing** — Import individual frames or a spritesheet. For spritesheets, you manually define the grid (frame width, height) or manually select frame boundaries. No automatic detection.

External tools (TexturePacker, Aseprite) can generate sprite metadata; Construct 3 recognizes some formats (JSON) but doesn't natively parse them.

**Atlas** — No automatic atlas generation. Construct 3 manages sprite memory internally; developers don't directly control atlasing. The engine optimizes at export time.

## Layer / hierarchy model

Construct 3 does not support layered animation like Photoshop. Complex characters are built using:

- **Multiple Sprite objects** as children in a container or pinned together.
- **Visibility toggling** or **image offset** to swap between costume variants.
- **Bone/pin system** — Newer Construct 3 features include a simple skeletal pin system (not bone deformation, but positional rigging).

This is manual compared to dedicated rigging tools but sufficient for many games.

## Export and import

**Import** — Sprites are imported as standard image files. Construct 3 supports PNG, JPG, SVG.

**Export** — Animations and sprite definitions are Construct 3 assets, stored in the project file. There is no native export to external animation formats (FBX, Spine, etc.).

Sprites can be exported as individual frame images for use elsewhere, but animation definitions remain in Construct 3.

## Scripting and extensibility

Construct 3 is primarily event-driven (no-code), but JavaScript support is available:

- Trigger animations: `sprite.play("walk")`.
- Query animation state: `sprite.isAnimPlaying()`.
- Advanced logic via JavaScript for custom animation blending (rare).

The Sprite Editor UI is not extensible; you cannot add custom animation types or easing functions. However, Construct 3 supports plugins, allowing third-party extensions (though sprite editor plugins are uncommon).

## How it fits the asset pipeline

Typical workflow:

1. Artist creates animation in Aseprite, exporting frames or spritesheet.
2. Artist (or designer) imports frames into Construct 3 Sprite Editor.
3. Designer arranges frames into named animations and sets timing.
4. Designer uses the Events system to control animation playback (no code).
5. Game exports to HTML5, mobile app, or desktop.

This is the most user-friendly workflow for non-programmers.

## Workflow strengths

- **Integrated animations** — Everything is in the browser; no external tools required.
- **Visual event system** — Non-programmers can set up complex animation logic without code.
- **Fast iteration** — Browser-based; no compile step, instant preview.
- **Export to many platforms** — HTML5, mobile app, console (via third-party services).
- **Image points are intuitive** — Visual placement beats numerical input.

## Workflow gaps

- **No skeletal rigging** — Flipbook only; no bone deformation.
- **No state machine** — Animation transitions are event-based, not hierarchical.
- **Manual spritesheet slicing** — No automatic detection; tedious for large assets.
- **No character composition library** — Swappable parts are manual; no sprite swap system like Unity's Sprite Library.
- **Browser performance** — Complex projects may stutter in-editor; lag during preview.

## Notable uses

- **Indie browser games** — Construct 3 games often appear on itch.io.
- **Mobile games** — Export to iOS/Android via third-party wrapper.
- **Game jam entries** — Construct 3's speed and no-code philosophy make it popular for time-constrained projects.

## Community and ecosystem

Construct 3 has a supportive community:

- Official documentation and tutorials.
- Asset store with sprite packs and animations.
- Community forums with sprite animation advice.

Integration with external tools is manual; developers often use Aseprite or similar for animation, then import into Construct 3.

## Pricing details

- Free tier: Limited to 50 events, some features restricted.
- Personal: $99/year, unlimited events and exports.
- Business: $300/year, team collaboration features.

Sprite Editor and Animations Editor are available in all tiers.

## Version history and features

Construct 3 receives monthly updates. The Sprite Editor has been continuously refined since launch, with recent additions including advanced collision shapes and animation speed curves.

## Interaction with related tools

- **Aseprite** — Direct export integration in newer Aseprite versions for frame export.
- **Spine, Spriter** — Can export sprite sequences; skeletal data is not imported.
- **Browser-based workflow** — No local dependencies; works from any browser.
