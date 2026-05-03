# Defold Sprite and Animation Tools

## Quick facts
- Vendor / maintainer: King (Defold Foundation, open-source)
- License / pricing model: MIT open-source (free)
- Price point (current): Free
- Platforms: Windows, macOS, Linux (editor and runtime)
- First released: 2014 (initial release)
- Last meaningful update: 2024 (ongoing development)
- Source available: Yes (GitHub, full source code)
- Primary use case: Lightweight 2D sprite animation and tile-based games

## Origin and purpose

Defold is a lightweight, mobile-first game engine developed by King (creators of Candy Crush). The sprite and animation system reflects this design: minimal overhead, fast iteration, and direct control.

Unlike Unity or Unreal, Defold provides very little in-engine sprite editing. The philosophy is to keep the editor lean and push content creation (art, sprites) to external tools, with Defold handling only sprite composition and playback.

## Sprite / drawing capabilities

Defold has no pixel editor or sprite slicing tool. Sprites are created and exported externally:

- Aseprite, Photoshop, Krita for art creation.
- Sprite sheets must be prepared in external tools.

Defold assumes sprites are pre-made and organized as:

- Individual sprite image files.
- Spritesheets with frame metadata (JSON, XML).

The Defold editor is used only for:

- **Image referencing** — Point to a sprite texture.
- **Animation definition** — Create a Tile Source or Sprite component that plays frames.

There is no visual sprite editor in Defold; all setup is via file references and configuration files.

## Animation system

Defold offers two animation paths:

**Tile Source animations** — For tilemap-based games. A Tile Source asset defines a tileset and can include animations:

- Frames are specified by tile indices.
- Animations play by cycling through tile indices.
- Each animation has a playback speed.
- Simple and lightweight; used in tilemap games.

**Sprite animations** — For sprite-based games. A Sprite asset references a Tile Source or image and plays animations defined in the Tile Source.

**Flipbook animation** — Both paths use frame-by-frame animation; no skeletal rigging or state machine.

**Animation scripting** — Control is entirely code-driven via Lua:

- `sprite.play_animation("#sprite", "walk")` to play an animation.
- No visual event system or UI-driven transitions.

This gives developers precise control but requires more code than no-code engines.

## Pivot, slicing, and atlas tools

**Pivots** — Not directly configurable in Defold. The origin is assumed to be the top-left corner of the sprite. Workarounds involve adjusting sprite position in code or using offset positioning.

This is a notable limitation compared to engines like Unity or Godot.

**Slicing** — No slicing tool. You must prepare spritesheets externally using TexturePacker, Aseprite, or custom scripts.

**Atlas** — No automatic atlas generation. Defold manages texture memory internally, but artists must manually organize atlases using external tools.

## Layer / hierarchy model

Defold does not support animation layers. Complex characters are built using:

- **Multiple Sprite instances** positioned and parented in the game object hierarchy.
- **Sprite swapping** — Change which sprite is visible based on logic.
- **Manual rigging** — Position sprites relative to each other in code.

This is less structured than Unity's layer system but straightforward for simple characters.

## Export and import

**Import** — Defold reads sprite images (PNG, mostly) and Tile Source definitions (a Defold custom format).

**Export** — Sprites and animations are Defold assets; they do not export to external formats. There is no round-trip animation editing.

## Scripting and extensibility

Defold's core language is Lua, and animation is entirely Lua-driven:

```lua
sprite.play_animation("#sprite", "walk")
sprite.set_animation("walk")
```

The animation system cannot be extended with custom easing or state machines through the UI. Custom animation logic is written in Lua.

Extensions are possible via native C/C++ code, but animation system customization is not common.

Third-party animation tools do not integrate; you export sprite sequences and play them in Defold's flipbook system.

## How it fits the asset pipeline

Typical workflow:

1. Artist creates animation in Aseprite or Spine, exporting frames or spritesheet.
2. Artist uses TexturePacker or manual organization to create a Tile Source with animation metadata.
3. Developer imports the Tile Source into Defold.
4. Developer creates a Sprite or Tilemap component referencing the Tile Source.
5. Developer writes Lua code to play animations and respond to game events.
6. Game compiles and deploys to mobile, web, or desktop.

This is more hands-on than Unity or Godot but suits Defold's lean philosophy.

## Workflow strengths

- **Minimal overhead** — No fancy UI; direct file-based configuration.
- **Mobile-optimized** — Designed for mobile performance; lightweight runtime.
- **Open-source** — Full control and transparency; fork-friendly.
- **Fast compilation** — Quick deploy cycles on mobile devices.
- **Lua is accessible** — Easier for non-programmers than C++ or C#.

## Workflow gaps

- **No visual sprite editor** — All sprite setup is file-based.
- **No pivot editing** — Origins are fixed; workarounds are required.
- **No automatic atlas generation** — Must use external tools like TexturePacker.
- **No skeletal rigging** — Flipbook only.
- **No visual animation editor** — All animation definition is code or file-based.
- **Limited documentation** — Smaller community means fewer tutorials.

## Notable uses

- **Mobile games** — Defold is popular in mobile studios (King uses it internally).
- **Indie 2D games** — Growing indie adoption, especially among developers wanting lightweight engines.
- **HTML5 games** — Defold exports to web with good performance.

## Community and ecosystem

Defold has a smaller but dedicated community:

- Official documentation and examples.
- Community GitHub repos with shared libraries (animation helpers, etc.).
- Forum support, though less active than Unity or Godot.

Third-party integration is minimal; most asset pipeline work is manual.

## Pricing details

Free and open-source. No licensing fees, no per-project costs. The Defold Foundation maintains the engine.

## Version history and features

Defold has been in development since 2014 and is mature for production use. Recent updates focus on performance and mobile optimization rather than feature additions.

## Interaction with related tools

- **TexturePacker** — Commonly used to generate Tile Source definitions for sprite atlases.
- **Aseprite** — For frame animation and spritesheet export.
- **Spine, Spriter** — Can export sprite sequences; skeletal data is not imported into Defold's animation system.
