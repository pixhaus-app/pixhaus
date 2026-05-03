# Unity Sprite Editor and Sprite Atlas

## Quick facts
- Vendor / maintainer: Unity Technologies
- License / pricing model: Free (part of Unity Engine)
- Price point (current): Included with Unity editor
- Platforms: Windows, macOS, Linux (editor); all platforms (runtime)
- First released: 2013 (first Sprite Editor) / 2017 (first Sprite Atlas)
- Last meaningful update: 2024 (Sprite Atlas V2 refined in 2022.2+)
- Source available: No (proprietary)
- Primary use case: Sprite slicing, pivot/origin definition, and automatic atlas packing

## Origin and purpose

The Sprite Editor emerged as Unity's native tool for handling sprite sheets and individual sprite metadata (pivots, collision masks). It was designed to eliminate the need for external sprite slicing tools, keeping all sprite setup within the editor.

The Sprite Atlas later became Unity's solution to texture memory optimization: automatically packing multiple sprites into fewer, larger atlases to reduce draw calls and memory overhead. This addressed a common pain point in mobile and console 2D games.

Together, they form the foundation of sprite asset management in Unity 2D projects.

## Sprite / drawing capabilities

The Sprite Editor does not support pixel-level drawing or painting. It is strictly a metadata and organizational tool for existing sprite textures. Its capabilities are confined to:

- **Sprite boundary detection** — Automatic slice of a spritesheet into individual sprites by analyzing alpha transparency.
- **Manual slicing** — Grid-based, polygon-based, or free-form sprite extraction.
- **Pivot point editing** — Visual placement of a sprite's rotation and attachment center.
- **Image points** — Multiple named attachment points per sprite (e.g., for weapon sockets or VFX origins).
- **Collision mask editing** — Define which pixels collide in physics simulation; separate from the sprite visual boundary.
- **Secondary Textures** — Attach normal maps, emission maps, or other supplementary textures to a sprite for shader effects.

All sprite creation (pixels, colors, layers) must happen externally in Photoshop, Aseprite, Krita, or similar.

## Animation system

The Sprite Editor itself has no animation system. Animation is handled by:

- **AnimationClips** — Standard Unity timeline clips that keyframe sprite index changes (flipbook animation) or other sprite properties.
- **Animator** — A state machine that blends and transitions between AnimationClips.
- **Sprite Swapping** — Keyframing which sprite from a sheet is rendered each frame, creating flipbook animation.

For skeletal animation (bones, weights, deformation), the separate 2D Animation package is required.

## Pivot, slicing, and atlas tools

**Pivot mode** — Lets you set a single pivot point per sprite (center of rotation) by clicking or entering coordinates. The pivot affects animation rotation and can be adjusted without re-exporting from external tools.

**Slicing tools:**

- Automatic Slice — Detects contiguous pixels and extracts each sprite. Fast but imperfect for dense spritesheets with touching sprites.
- Grid Slice — Regular grid division of the spritesheet.
- Polygon Slice — Free-form custom boundaries (rarely used; automatic is usually sufficient).

After slicing, each sprite gets its own metadata entry in the Sprite Editor.

**Sprite Atlas V2** (standard in Unity 2022.2+) — Automatically packs sprites into texture atlases. Key features:

- Tight Packing — Uses sprite mesh outlines instead of axis-aligned rectangles, reducing transparent padding.
- Alpha Dilation — Bleeds colors into transparent edges to avoid sampling artifacts at sprite boundaries.
- Rotation allowed — Rotates sprites to fit more densely (rarely necessary but available).
- Build-time or Play-mode packing — Control when atlases are composited.

The atlas system is not visible to the artist during animation design; it's a runtime optimization applied at build time.

## Layer / hierarchy model

The Sprite Editor operates on individual sprites or groups of sprites within a texture. There is no layer concept in the traditional 2D animation sense. Layer organization comes from:

- **Sprite library assets** — Define categories and variants (e.g., "Head", "Body") that organize sprites semantically, often used with the 2D Animation package.
- **Multiple textures** — Artists can organize sprites across different source textures and import them separately.

The Sprite Editor does not enforce or enforce any naming or grouping convention.

## Export and import

**Import** — Sprites are imported as Texture assets. The Sprite Editor reads texture data and metadata (slices, pivots) stored in the asset. No special format: just standard PNG or TGA textures.

**Export** — Sprites cannot be exported as new texture files from the Sprite Editor. However, you can:

- Copy sprite textures out of the project folder.
- Export AnimationClips (as FBX for external tools, though this is uncommon for 2D).

The Sprite Editor is primarily an import-time tool, not an export tool.

## Scripting and extensibility

The Sprite Editor is not script-accessible. Scripting can:

- Dynamically swap sprites at runtime via animator or direct sprite component modification.
- Generate Sprite Atlases programmatically using the AtlasRequestQueue API.
- Access sprite metadata (pivot, bounds) via the Sprite class.

You cannot extend the Sprite Editor UI or add custom slicing algorithms through scripts.

Third-party sprite tools (like Aseprite or Spine) export sprite sheets that the Sprite Editor then processes; there is no direct integration.

## How it fits the asset pipeline

Typical workflow:

1. Artist creates and exports sprite sheet from Aseprite, Photoshop, or similar.
2. Artist drops the texture into Unity project.
3. Unity auto-detects it as a sprite and opens Sprite Editor if needed.
4. Artist configures slices, pivots, collision masks (or leaves defaults).
5. Animator or 2D Animation package uses the sprites to create animations.
6. At build time, Sprite Atlas automatically packs sprites for optimization.
7. Game renders sprites, using the atlased versions at runtime.

The Sprite Editor is a mid-pipeline tool: after external art creation, before animation or gameplay.

## Workflow strengths

- **Zero-friction setup** — Import texture, set pivot, done. Minimal UI overhead.
- **Visual editing** — Drag pivots around, see collision masks in real-time.
- **Automatic slicing** — Saves time on dense spritesheets; fast iteration.
- **Atlas automation** — Packing happens at build time; no manual atlas creation or maintenance.
- **Integrated** — No external tools needed; everything in one editor.

## Workflow gaps

- **No pixel editing** — Cannot paint or clone within the Sprite Editor; must use external tools.
- **Basic slicing** — Automatic slice fails on overlapping sprites. Polygon slice is tedious for complex sheets.
- **Limited collision options** — Collision masks are pixel-based; no custom shape tools.
- **No layer support** — Cannot import Photoshop layers as separate sprites directly (requires PSD Importer package).
- **Atlas is opaque** — You cannot see or control individual sprite placement in the atlas; it's fully automatic.

## Notable uses

- **Standard 2D game projects** — Nearly all Unity 2D games use Sprite Editor for slicing and pivot setup.
- **Mobile games** — Sprite Atlas is heavily used to optimize texture memory on mobile platforms.
- **Tilemap games** — Tilemap systems rely on well-sliced spritesheets and Sprite Atlas for performance.

## Community and ecosystem

The Sprite Editor is fundamental to Unity 2D workflow. Learning resources are abundant:

- Official Unity documentation and tutorials.
- Countless asset store packs with pre-sliced sprites.
- Community guides on pivot placement for common character types (platformers, isometric).

Third-party integration is limited but not needed; the Sprite Editor is self-contained.

## Pricing details

Free. Included with all Unity Editor licenses.

## Version history and platform support

The Sprite Editor is part of core Unity. Sprite Atlas V2 became the default in Unity 2022.2 and is the recommended version going forward. Earlier projects using Sprite Atlas V1 can be upgraded automatically.

## Interaction with related tools

- **2D Animation package** — Uses sliced sprites and pivots as the foundation for skeletal rigging.
- **2D Tilemap Editor** — Slices and prepares tileset textures similarly.
- **PSD Importer** — Can import Photoshop layer structures and auto-slice them, bypassing manual slicing in Sprite Editor.
