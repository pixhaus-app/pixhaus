# Tool research template

Every tool file follows this shape. Sections may be expanded or compressed based on what's actually distinctive about the tool, but skipping sections is discouraged — a missing section is itself information.

```markdown
# <Tool Name>

## Quick facts
- Vendor / maintainer:
- License / pricing model:
- Price point (current):
- Platforms:
- First released:
- Last meaningful update:
- Source available:
- Primary use case:

## Origin and purpose
Where it came from, why it exists, who the original audience was, how that audience evolved.

## Drawing and painting tools
Brush engine, eraser, fill, gradient, shape primitives, selection tools, transforms (move, rotate, scale, skew, flip), symmetry, references, color picker behavior. Note anything pixel-specific (pixel-perfect strokes, no-anti-aliasing modes).

## Pixel-specific features
Indexed color modes, palette locking, tile-aware drawing, pattern fills, dithering brushes, rotsprite or other rotation algorithms, sub-pixel handling.

## Color and palette workflow
Palette format, palette swap workflow, indexed vs RGB modes, color ramps, gradient maps, harmony tools, sharing palettes between files, importing palettes (.aco, .gpl, .pal, .hex, lospec).

## Layer system
Layer types (raster, vector, group, reference, tilemap, animation cel), blend modes, masks, layer effects, layer linking across frames, per-layer animation.

## Animation features
Timeline structure, frame types (key, hold, blank), onion skin, frame tags, loop types, tweening (none, linear, eased), inverse kinematics, mesh deformation, skeletal rigging, motion paths, easing curves, audio sync, preview controls.

## Export and import
File formats read and written. Sprite sheet packing options. Atlas formats. JSON / XML metadata exports. Animation data exports (skeletal data, frame timing). Layered export formats. Lossy vs lossless considerations.

## Scripting and extensibility
Scripting language. API surface. Hot-reload of scripts. Plugin marketplace or registry. Headless or CLI mode. Automation hooks (export pipelines, asset processing).

## Engine integration
Unity importers, Godot importers, Unreal pipelines, GameMaker integration, custom runtime libraries. Whether the tool ships with runtimes or relies on third-party loaders.

## Workflow strengths
What this tool does better than the rest of the field. Be specific — name the feature, name the workflow.

## Workflow gaps
What this tool can't do, does poorly, or makes painful. Be specific.

## Notable uses
Shipped games, studios that use it, well-known artists who use it.

## Community and ecosystem
Tutorial coverage, asset packs, third-party plugins, active development, alternative builds or forks.

## Pricing details
Tiers, education licenses, studio licenses, source code access, perpetual vs subscription.
```

This template is a floor, not a ceiling. If a tool has something unique (Aseprite's RotSprite, Spine's IK with constraints, Live2D's mesh deformers), call it out in its own section.
