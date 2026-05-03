# GameMaker Studio Sprite Editor and Animation Curves

## Quick facts
- Vendor / maintainer: YoYo Games
- License / pricing model: Subscription (free tier available)
- Price point (current): Free (Creator), $5-20/month (Pro/Business)
- Platforms: Windows, macOS (editor); all platforms (runtime)
- First released: 1999 (GameMaker); modern Sprite Editor in 2.3 (2020)
- Last meaningful update: 2024 (continuous updates)
- Source available: No (proprietary)
- Primary use case: Sprite animation with easing curves, flipbook playback, and frame control

## Origin and purpose

GameMaker evolved as a game engine focused on 2D games, with tight integration between sprite assets, animation definitions, and code. The Sprite Editor and Animation Curves system are designed to enable rapid prototyping and iteration without external tools.

Unlike Unreal or Unity, GameMaker assumes most developers are creating art externally but animating frame-by-frame in-engine. The Animation Curves system (introduced in 2.3) brought professional easing and tweening to in-engine animation, reducing external dependency.

## Sprite / drawing capabilities

GameMaker has no pixel editor. Sprites are created externally (Aseprite, Photoshop, Krita) and imported as PNG/JPG files.

The Sprite Editor provides:

- **Frame import** — Import individual frames or spritesheets.
- **Frame organization** — Name frames, set per-frame display duration (speed in fps or frames-per-second).
- **Collision shape editing** — Define rectangular or rotated collision shapes per frame.
- **Origin point editing** — Visual placement of the sprite's anchor/pivot point.
- **Bounding box settings** — Used for overlap detection and drawing calculations.

No pixel-level editing, but frame organization and metadata are comprehensive.

## Animation system

GameMaker treats animation as frame sequences, not skeletal or state-driven:

**Flipbook Animation** — Each sprite is a sequence of frames with per-frame timing. You define the animation entirely within the sprite asset. Sprites have a single animation sequence (no multiple animations per sprite as in Godot's SpriteFrames).

**Animation Curves** (introduced 2.3) — A separate asset type that defines how values change over time using curves:

- Linear, smooth, or bezier interpolation.
- Multi-channel curves (e.g., X, Y position for movement; R, G, B, A for color).
- Preset library: easing curves (Ease In, Ease Out, Ease In-Out) with various functions (quadratic, cubic, exponential, back, elastic, bounce).

Curves are applied to moving objects via code, not sprites directly. Example: an explosion sprite might use a curve to scale up and fade out over time.

**Animation sequences** — GML (GameMaker Language) code directly controls which sprite is drawn, allowing frame-by-frame logic. This is more flexible than UI-driven state machines but requires more code.

## Pivot, slicing, and atlas tools

**Pivots (origins)** — Set visually in the Sprite Editor. The origin point defines the rotation center and drawing position (where x, y coordinates are placed).

**Slicing** — No automatic slice tool. You must either:

- Import individual frame files (one PNG per frame).
- Import a spritesheet and manually define frame boundaries (tedious; developers often use TexturePacker or external tools).

**Atlas** — No built-in automatic atlas generation. Sprites are managed individually or via TextureGroup (a manual grouping system). GameMaker does not optimize atlasing at build time like Unity.

However, TextureGroup allows you to group sprites for memory management, influencing how the runtime allocates textures.

## Layer / hierarchy model

GameMaker does not use a layer system for animation. A sprite is a single asset with one animation sequence (one set of frames). Complex characters with swappable parts use:

- **Multiple sprite assets** (one for each part: head, body, limbs).
- **Instance variables** to track which sprite to draw for each part.
- **Code-driven composition** in the Step or Draw events.

This is manual compared to Unity's Sprite Library, but straightforward.

## Export and import

**Import** — Sprites are imported as PNG/JPG images. GameMaker reads frame metadata (dimensions, offsets) from the file or from manual configuration in the Sprite Editor.

**Export** — No export of animation data or sprites from GameMaker to external tools. Sprites and curves are GameMaker assets, stored in the project file.

Animation Curves can be inspected as JSON within the project, but external tools do not naturally read them.

## Scripting and extensibility

Animation is heavily code-driven:

- Change current sprite at runtime: `sprite_index = spr_walk;`
- Change image index (frame): `image_index = 5;`
- Use Animation Curves via `animcurve_get_channel()` to sample easing values.
- Create animations entirely in code, keyframing sprite changes in a loop or via object events.

The Sprite Editor and Animation Curves UI are not extensible. You cannot add custom easing functions or curve types.

Third-party animation tools can export sprite sequences (Spine, Spriter) that GameMaker imports as frame collections, but skeletal data is lost.

## How it fits the asset pipeline

Typical workflow:

1. Artist creates animation in Aseprite or Spine, exporting frames.
2. Developer imports frames into a GameMaker Sprite asset.
3. Developer sets frame timing and origin point in Sprite Editor.
4. Developer creates Animation Curves for easing if needed (scaling, fading, etc.).
5. Developer uses code to play sprites and apply animations: `image_index += image_speed;`
6. Gameplay logic selects sprites and curves based on character state.

This is code-heavy compared to UI-driven animation systems but offers fine control and is natural to developers.

## Workflow strengths

- **Fast sprite import** — Drag frames in, set origin, play.
- **Frame-accurate control** — Code can directly manipulate current frame; useful for frame-perfect gameplay.
- **Animation Curves are powerful** — Easing library is comprehensive; no need for external tweening libraries.
- **Low overhead** — Minimal UI; games are simple to prototype.
- **Accessible** — Designed for indie developers; straightforward workflows.

## Workflow gaps

- **No automatic slicing** — Spritesheet import requires manual frame boundary definition.
- **No skeletal rigging** — Flipbook only; no bone deformation.
- **No state machine UI** — Animation flow is code-driven; complex transitions require logic.
- **No character composition tools** — Swappable parts are manual; no sprite library system.
- **No visual animation editor** — Unlike Unreal or Unity, there's no timeline/keyframe editor for sprite animations.

## Notable uses

- **Indie 2D games** — Most GameMaker games use the built-in sprite system.
- **Hyper Light Drifter, Katana Zero, Nuclear Throne** — Notable games using GameMaker's sprite workflows.
- **Pixel art games** — Natural fit for pixel-perfect animation.

## Community and ecosystem

GameMaker has a mature and large community:

- Official documentation and tutorials.
- Community sprite packs on itch.io and asset marketplaces.
- Integration with Aseprite (native export to GameMaker format in newer Aseprite versions).

Animation Curves library is well-documented; many developers use preset curves without writing custom easing.

## Pricing details

GameMaker offers a free tier (Creator) with limitations, and paid subscriptions (Pro $5/month, Business $20/month) unlocking features like console export and advanced compilation.

Sprite Editor and Animation Curves are available in all tiers.

## Version history and features

**2.3 (2020)** — Animation Curves introduced, bringing professional easing to the engine.

**2.3.x (ongoing)** — Continuous improvements to animation and sprite handling. Version 2025.x is current.

## Interaction with related tools

- **Aseprite** — Seamless export; Aseprite can export directly to GameMaker format.
- **Spine, Spriter** — Can export sprite sequences that GameMaker imports as flipbook frames.
- **TexturePacker** — Developers use it to prepare spritesheets before import.
