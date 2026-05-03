# Engine-Integrated Sprite and Animation Tools

This directory documents the sprite editing, rigging, and animation systems built into game engines. These tools represent the "canvas and brushes" that artists work with in-engine, constraining and enabling workflows before reaching external editors.

## Coverage

Engine-integrated tools fall into two categories:

**Core sprite systems** — 2D sprite rendering, frame selection, pivot/origin point control.

**Animation and rigging systems** — Bone-rigging, IK, animation blending, state machines, flipbook animation playback.

The split between these two is important: some engines (Unity, Godot) separate sprite editing (in Sprite Editor or isolated) from animation (in dedicated editors), while others (GameMaker, Construct 3) integrate both tightly.

## Key strategic questions for SpriteMaster

- What must artists do in-engine vs. externally? (e.g., Unity requires the Skinning Editor for bone rigging; artists cannot create rigs in Photoshop then import them.)
- How much iteration happens in-engine? (Godot's AnimationPlayer allows frame-by-frame editing; Unreal's PaperZD does not.)
- How do these engines treat layer information and sprite slicing? (Unity's PSD Importer converts layers to sprites; Godot expects pre-sliced spritesheets.)
- What's the export path? (Can artists export animations back out for version control or collaboration?)

## Tools documented

- **Unity 2D Animation Package + PSD Importer** — Professional bone-rigging in-engine.
- **Unity Sprite Editor + Sprite Atlas** — Sprite slicing, pivots, atlas packing.
- **Godot 4.x AnimatedSprite2D + AnimationTree** — Lightweight flipbook and state-machine animation.
- **Unreal Engine Paper2D + PaperZD** — Sprite-based 2D in a 3D engine, with plugin enhancements.
- **GameMaker Studio sprite editor and animation curves** — Visual animation tool with easing control.
- **Construct 3 sprite editor and animation** — Browser-based, no-code animation system.
- **Defold sprite/animation tools** — Lightweight game engine with minimal in-editor sprite work.
