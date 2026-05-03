# Unreal Engine Paper2D and PaperZD

## Quick facts
- Vendor / maintainer: Epic Games (Paper2D); Community/Free (PaperZD plugin)
- License / pricing model: Free (part of Unreal Engine)
- Price point (current): Included with Unreal Engine 5
- Platforms: Windows, macOS, Linux (editor); all platforms (runtime)
- First released: Paper2D in UE4.6 (2014); PaperZD as community plugin (2019)
- Last meaningful update: 2024 (UE 5.4+)
- Source available: Yes (Unreal Engine source on GitHub)
- Primary use case: 2D and 2.5D games in a 3D engine, with skeletal animation via PaperZD plugin

## Origin and purpose

Paper2D was Epic's answer to developers wanting 2D games within Unreal Engine, a tool built for 3D. It treats sprites as flat 3D meshes, leveraging Unreal's powerful 3D animation systems (skeletal animation, blend spaces, locomotion) for 2D workflows.

PaperZD emerged from the community as a free plugin to improve Paper2D's usability, particularly for skeletal animation workflows. It adds a visual animation editor and makes the rigging-to-animation pipeline more artist-friendly.

Together, they enable professional 2D game development on Unreal's architecture.

## Sprite / drawing capabilities

Paper2D has no built-in pixel editor or sprite creation tools. Sprites must be created externally:

- Aseprite, Krita, Photoshop for pixel art.
- Sprite sheet creation in external tools.

Sprite metadata (slicing, pivots) is handled via:

**Paper2D Sprite Asset** — An asset that references a texture and defines its frame, collision shape, and pivot. You manually set the texture region and origin point in the Inspector.

There is no visual sprite editor in-engine like Unity's Sprite Editor. You set properties numerically.

## Animation system

Paper2D integrates with Unreal's standard animation systems:

**Flipbook Animation** — A Paper2D-specific asset that defines a sequence of frames with timing. Simple flipbook playback without blending.

**Skeletal Animation (via PaperZD)** — The main advantage of PaperZD is that it unlocks skeletal rigging for Paper2D:

- Define bones on a reference sprite.
- Paint weight maps (per-bone influence on pixels).
- Create AnimationSequences (keyframed bone transforms).
- Use Unreal's AnimBlueprintGeneratedClass (state machines, blending, locomotion).

This makes PaperZD-rigged characters significantly more powerful than flipbook-only Paper2D.

**Blend Spaces** — Unreal's standard blend space system works with PaperZD, enabling smooth transitions between animations (e.g., walk-to-run blending based on speed).

**Animation Notifies** — Trigger events during animation playback (footstep sounds, VFX, etc.). Supported in both flipbook and PaperZD workflows.

## Pivot, slicing, and atlas tools

**Pivots** — Set manually in the Paper2D Sprite asset Inspector by entering coordinates or clicking in a visual preview. No dedicated pivot editor.

**Slicing** — No automatic slicing tool. You must manually specify texture regions for each frame, or use a script/external tool to generate sprite assets from a spritesheet.

Some developers use TexturePacker or write custom Python scripts to generate Paper2D Sprite assets programmatically.

**Atlas** — Unreal's standard texture atlasing (handled by the renderer) works with Paper2D. No specialized atlas tool; the engine handles optimization automatically at cook time.

## Layer / hierarchy model

Paper2D sprites are individual assets, not layered like in Photoshop. Complex characters are built using:

- **Paper2D Character Blueprint** — A Blueprint that combines multiple Sprite components (body, head, clothing) as children.
- **Skeletal Mesh (via PaperZD)** — A single rigged skeleton can deform multiple sprites, or a character uses multiple rigged pieces.

PaperZD supports hierarchical skeletons, so a parent bone can control child bones (e.g., upper arm controls forearm).

## Export and import

**Import** — Paper2D recognizes sprite textures (PNG, TGA) imported as standard Unreal textures. No special importer.

PaperZD requires manual rigging setup; there is no PSD Importer equivalent for automatic layer extraction.

**Export** — There is no built-in export of animations or rigs back to external formats. AnimationSequences are Unreal assets. You can export to FBX (which creates a 3D skeletal format), but this is not the intended workflow.

## Scripting and extensibility

Paper2D and PaperZD are scriptable via C++:

- Create sprites, flipbooks, and skeletons programmatically.
- AnimBlueprints can query animation states and trigger transitions.
- Custom animation nodes can be added to AnimGraphs.

Extensibility is strong if you write C++, but the visual editor (AnimGraph) is not easily extended for custom nodes.

Third-party skeletal animation tools (Spine, Dragon Bones) can export sprite sequences, but direct integration is limited.

## How it fits the asset pipeline

Typical workflow with PaperZD:

1. Artist creates multi-layer character in Photoshop/Aseprite.
2. Artist exports spritesheet(s).
3. Developer imports textures into Unreal.
4. Developer (or rigging specialist) creates Paper2D Sprites and defines a PaperZD skeleton.
5. Developer paints weight maps in PaperZD's editor.
6. Developer creates AnimationSequences and AnimBlueprints for gameplay logic.
7. Character Blueprint references the rigged actor; gameplay instantiates and animates it.

This is more involved than Unity's PSD Importer workflow but more powerful than Godot's manual setup.

## Workflow strengths

- **Professional animation system** — Unreal's animation state machines and blend spaces are industry-standard; powerful for complex character control.
- **2.5D hybrid ready** — Paper2D sprites are 3D-aware; easy to layer 2D sprites with 3D geometry for parallax and depth.
- **Animation notifies** — Built-in event system for footsteps, attacks, and other frame-based triggers.
- **Skeletal rigging (PaperZD)** — Enables deformation animation, superior to flipbook-only workflows.
- **Mature engine** — Unreal's animation tools are battle-tested; used in AAA games.

## Workflow gaps

- **No visual sprite editor** — Must set pivots, slicing numerically or via script.
- **No automatic rigging** — Unlike Unity's Auto-Bone, you must place bones manually (though PaperZD provides a visual interface).
- **PaperZD is third-party** — It's not official; community-maintained, though reliable.
- **No PSD importer** — Layer-to-sprite automation doesn't exist; require manual export.
- **Complexity overhead** — Unreal is a heavy engine; 2D-only projects may feel overengineered.

## Notable uses

- **Indie 2D platformers** — Several Unreal-powered 2D games use Paper2D + PaperZD.
- **2.5D games** — Paper2D's 3D integration makes it ideal for parallax and depth effects.
- **Castlevania-style games** — Community examples of side-scroller RPGs using Paper2D.

## Community and ecosystem

Paper2D has a smaller community than Unity 2D, but PaperZD has grown the ecosystem:

- Official Unreal documentation on Paper2D.
- Community tutorials on PaperZD rigging and animation.
- Asset Marketplace packs with Paper2D characters (less common than Unity).

PaperZD development is active and community-driven, with regular updates.

## Pricing details

Free. Unreal Engine is free to use and develop with (royalty applies only on game sales after $1M USD revenue, or $1M+ in lifetime revenue).

## Version history and platform support

Paper2D has been in Unreal since 4.6. Unreal 5.x continues to support Paper2D with incremental improvements. PaperZD is actively maintained on GitHub and compatible with modern Unreal versions.

## Interaction with related tools

- **Skeletal Animation System** — Paper2D leverages Unreal's skeleton and animation systems, sharing tools with 3D workflows.
- **AnimGraph** — The visual animation state machine editor works identically for Paper2D rigged characters and 3D characters.
- **Niagara (VFX)** — Works seamlessly with Paper2D characters for effects and particle spawning.
