# Unity 2D Animation Package

## Quick facts
- Vendor / maintainer: Unity Technologies
- License / pricing model: Free (part of Unity Engine)
- Price point (current): Included with Unity editor
- Platforms: Windows, macOS, Linux (editor); all platforms (runtime)
- First released: 2019 (first public version 1.0)
- Last meaningful update: 2024 (version 9.x series)
- Source available: No (proprietary)
- Primary use case: Importing layered PSD/PSB files and rigging characters for skeletal animation

## Origin and purpose

The 2D Animation package emerged from Unity's push to make professional 2D character rigging accessible within the editor, eliminating the need for external rigging tools. It ships as a separate package (com.unity.2d.animation) starting in Unity 2019.3 and is maintained alongside the 2D PSD Importer for a unified workflow from Photoshop to in-engine animation.

The design philosophy emphasizes speed: artists import a layered Photoshop file, the PSD Importer automatically extracts layers as sprites, then the Skinning Editor enables bone placement and weight painting without leaving Unity.

## Sprite / drawing capabilities

The 2D Animation package does not include drawing or pixel-level sprite editing. Instead, it assumes sprites already exist as imported textures or PSD layers. The package's contribution to sprite handling is organizational:

- Sprite Library assets, which group related sprites by category (body parts, emotions, outfits) for runtime swapping.
- Sprite Sheet definition and frame management within the Sprite Editor (a separate package component).

The actual sprite creation (pixels, layers) happens in external tools like Photoshop, Aseprite, or Krita.

## Animation system

The 2D Animation package's animation system is bone-rigging focused:

**Skinning Editor** — A specialized view within the Sprite Editor that lets you create a skeleton by placing bones and painting weight maps. Bones are stored as part of the sprite asset, making them reusable across animation clips.

**AnimationClips** — Standard Unity AnimationClips that manipulate bone transforms. You can keyframe bone position, rotation, and scale, and keyframe sprite swaps (via Sprite Library) in the same clip.

**Skeletal deformation** — Weighted bones deform the sprite mesh at runtime. Each vertex of the sprite mesh is influenced by one or more bones according to painted weights. Moving a bone moves the attached pixels.

**Auto-Bone generation** — Recent versions (9.x) include smart skeleton suggestions. The tool analyzes a sprite's silhouette, detects extremities (hands, feet, head), and proposes a skeleton structure that the artist can then refine. This significantly speeds up rigging large character libraries.

**Bone hierarchy** — Bones form a tree structure (parent-child relationships), enabling realistic limb animation where moving an upper arm bone also moves the child forearm.

## Pivot, slicing, and atlas tools

Pivot points are managed in the Sprite Editor's Pivot mode. You can set a single pivot per sprite (the center of rotation) or use multiple image points as attachment points for props or VFX.

**Slicing** — The Sprite Editor's Automatic Slice tool lets you detect sprite boundaries in a spritesheet. Manual slicing via grid or custom polygon is also supported. This is essential before rigging: each sprite must have clean boundaries.

**Sprite Atlas** — The Sprite Atlas V2 system (standard in Unity 2022.2+) packs multiple sprites into optimized texture atlases at build time or Editor play. Atlasing happens after rigging and animation are complete. It's a runtime optimization, not a design-time constraint.

## Layer / hierarchy model

The Skeleton Editor defines a bone hierarchy. Bones are arranged parent-to-child, and in AnimationClips you key individual bones. There's no concept of "layers" in the traditional 2D animation sense (e.g., one layer for the head, another for arms). Instead, you swap sprites via keyframes and Sprite Library assets, which allows you to change which sprite is rendered for a given bone.

The Sprite Library provides organizational structure: you group sprites by category (e.g., "Head", "Torso", "LeftArm") and assign variants to each category (e.g., "Happy", "Sad" for Head). At runtime, you change the library variant to swap all sprites at once.

## Export and import

**Import** — The PSD Importer is the primary workflow. It reads .psb or .psd files from Photoshop, extracts layers as individual sprites, and optionally auto-generates a prefab skeleton from the layer stack. Supported Photoshop features: layer visibility, transparency, basic blend modes (ignored, rendered as flat). Unsupported: Photoshop effects, blend mode visual fidelity, layer opacity.

**Export** — There is no native export of rigs or animations from Unity back to Photoshop or external formats. AnimationClips are stored as Unity assets (.anim files). If you need to version-control animations or share them, you export to FBX (which creates a 3D-style skeletal animation file), but this is uncommon for 2D workflows. Most iteration stays in-engine.

## Scripting and extensibility

The 2D Animation package exposes APIs for runtime sprite swapping via Sprite Library and ISpriteSkinExtension (custom deformation). You can script sprite swaps, bone control, and IK chains in C#. Extensibility is limited: you cannot add custom rigging tools or modify the Skinning Editor UI directly.

Third-party tools (like Spine or Dragon Bones) do not integrate directly; you must export their outputs as sprite sequences and AnimationClips, losing skeletal structure.

## How it fits the asset pipeline

Typical workflow:

1. Artist creates a multi-layer character in Photoshop or similar (layers for body parts).
2. Artist imports the .psb file into Unity via PSD Importer.
3. PSD Importer extracts each layer as a separate Sprite.
4. Artist opens the Skinning Editor and places bones on a reference sprite.
5. Artist paints weight maps to define bone influence.
6. Artist creates AnimationClips that key bone rotations and sprite swaps.
7. Prefab references the rigged character; gameplay code instantiates it and plays animations.

This workflow replaces traditional tools like Spine or Dragon Bones for simple to moderately complex characters. It keeps iteration within Unity, reducing external tool dependencies.

## Workflow strengths

- **In-engine rigging** — No need to buy or learn Spine, Dragon Bones, or similar. Rigging happens in the same tool you're already using.
- **Layer-to-sprite automation** — The PSD Importer saves hours by converting Photoshop layers to sprites automatically.
- **Fast iteration** — Change bone placement, weight paint, or animation, and play immediately without re-exporting from external tools.
- **Integrated sprite swapping** — Sprite Library makes costume and expression changes trivial to animate.
- **Bind-pose editing** — You can adjust bone positions post-import, useful if the PSD structure wasn't rigging-friendly.

## Workflow gaps

- **No 2D drawing** — You cannot create or edit pixels in-engine. You must use external tools for art creation.
- **Limited to skeletal deformation** — No mesh transformation (bend, skew without bones), no bezier handles for organic curves.
- **No inverse kinematics (IK)** — You must key bone rotations manually. IK chains are not built-in (possible via scripting but not UI-driven).
- **No bone constraints** — Bones cannot be constrained (e.g., child bone locked to parent's end position). You must animate these relationships manually.
- **Rigging UI is minimal** — Weight painting is basic; no mirroring, no symmetry tools. Spine's painting tools are more powerful.
- **No export to external tools** — Rigs cannot be exported back to Photoshop or Spine. You are committed to in-engine workflows.

## Notable uses

- **Idle Masters, Catacomb Kids** — Indie platformers using skeletal animation for characters.
- **Professional mobile games** — Common in studios choosing Unity for 2D, as it avoids third-party tool licensing.
- **Asset store templates** — Many 2D character packs ship with rigged prefabs ready for animation.

## Community and ecosystem

The 2D Animation package has a growing community but smaller than Spine's. Learning resources:

- Official Unity Learn tutorials on rigging.
- Community GitHub repos demonstrating Sprite Library usage.
- Asset Store templates with pre-rigged characters.

Fewer third-party integrations than Spine, but enough to make the package self-contained.

## Pricing details

Free. The package is included with Unity Editor licenses (Personal, Plus, Pro). No per-project or per-character fees. The PSD Importer is also free and included with the same licenses.

## Package versioning and compatibility

Version 9.x requires Unity 2022.2 or later. Version 8.x targets 2022.1. Version 7.x targets 2021.2. The package updates with Unity minor releases, so upgrading Unity often means updating the package automatically.

## Related packages

The 2D Animation package relies on or pairs with:

- **2D PSD Importer** (com.unity.2d.psdimporter) — Imports Photoshop files as sprite collections.
- **2D Sprite** (com.unity.2d.sprite) — Core sprite rendering system.
- **2D Common** — Shared utilities for 2D packages.

These are separate packages but installed together in the 2D workflow template.
