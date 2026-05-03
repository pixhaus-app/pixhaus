# Patterns the field has settled on

After mapping ~60 tools across the spectrum, the convergence is striking. The features below show up in nearly every serious sprite-and-animation tool that has lasted more than a few years. They aren't conventions because someone said so — they're conventions because artists have rejected the alternatives.

A new tool can break these patterns, but it should know what it's giving up first.

## The timeline is the spine

Every animation tool — pixel-focused, skeletal, traditional — uses a horizontal timeline with frames left-to-right and tracks/layers stacked vertically. Aseprite, Spine, Toon Boom Harmony, Adobe Animate, Krita, Procreate Dreams, OpenToonz: same shape. Frame ranges, scrubbing, timeline zoom, and the playhead are universal.

The exceptions prove the rule. Rive replaces the linear timeline with a state machine for runtime-driven animation, but it still has a timeline beneath the state machine for individual states. Cavalry uses a procedural node graph but its scrub bar is still a horizontal timeline. Nobody has shipped a serious sprite tool that abandons the timeline.

## Onion skin is sacred

Showing the previous N frames as semi-transparent ghosts under the current frame is the single most important animation aid. Every animation tool has it — Aseprite, Krita, Pixelorama, Pyxel Edit, Pro Motion NG, Procreate Dreams, OpenToonz, TVPaint, Toon Boom, Adobe Animate, Moho, Cavalry. The configurable axis is range (how many frames before/after) and tinting (red previous / blue next is the most common default, originating from Aseprite). Tools that lack onion skinning are second-class for animation work.

## Indexed color is non-negotiable for pixel art

Real pixel artists work in indexed color mode where every pixel references a palette index, not an RGB triplet. This enables palette swaps, color cycling, and the strict palette discipline retro aesthetics demand. Aseprite, Pro Motion NG, GraphicsGale, and LibreSprite are all indexed-mode-first. The general-purpose painting tools (Photoshop, Krita, Procreate) treat indexed color as an afterthought, and pixel artists who use them know it.

The implication for AI generation: outputting RGB pixels and calling it pixel art produces sub-pixel anti-aliasing, dithering noise, and palette violations. The tools that take pixel art seriously (Retro Diffusion, PixelLab) post-process to snap to palette indices.

## Sprite sheet + JSON metadata is the engine handoff

Every meaningful sprite tool exports as a packed PNG sprite sheet plus a metadata file (JSON, XML, or both) describing frame rectangles, durations, tags, and pivots. Aseprite's JSON export is the de facto standard among indie engines because it's clean, complete, and supported by community importers for Unity, Godot, GameMaker, Phaser, and others. Spine's `.json`/`.skel` runtime data plays the same role for skeletal animation.

Tools that ship a proprietary binary format without an open metadata path get marginalized fast. Even Pyxel Edit's relative obscurity traces partly to its proprietary `.pyxel` format being harder to integrate.

## Frame tags organize multi-animation files

Aseprite's frame tags — named ranges within one timeline (idle: 0-3, walk: 4-11, jump: 12-15) with per-tag loop direction — became the dominant pattern. LDtk, Pixelorama, Pro Motion NG, and most engine importers respect them. Storing all character animations in one file with tags beats one file per animation. Skeletal tools (Spine, DragonBones) use named animations as their analogue.

## Layer groups are table stakes; blend modes are expected

Even small pixel editors now have layer groups (Pixelorama, Pro Motion NG, Aseprite). Blend modes (multiply, screen, overlay, add, etc.) follow the Photoshop list. Aseprite's known limitation that group opacity/blend modes don't work is one of the few visible gaps in an otherwise polished tool — and it's been on the wishlist for years.

## Tile-aware drawing is bolted on, not native

Most pixel editors treat tilemaps as a separate problem — Tiled and LDtk live in their own ecosystem, with sprite editors handing tilesheets over. The ones that integrate tile editing (Pyxel Edit, Pro Motion NG, Pixelorama) treat it as a mode, not the default. Aseprite added tilemap layers in v1.3 (2023) but the workflow is still secondary to its frame-based animation focus.

This split is a workflow tax. Artists who paint a tileset in Aseprite still hop to LDtk to test it in a level. Nobody has fully solved single-tool tile-paint-and-level-design.

## Skeletal animation has settled on bones + mesh deformation + IK + state machines

Spine defined the pattern: bones with parent/child hierarchy, mesh-deformed image attachments, IK constraints, transform constraints, path constraints, and skins (alternate visual sets sharing one rig). Every other skeletal tool — DragonBones, Spriter, Rive, Creature, Live2D — follows variations of this. Live2D adds parameter-driven mesh deformation as its differentiator (faces and expressions). Rive adds the runtime state machine. The core idea — bones drive meshes, animations drive bones, runtimes interpolate — is unchanged since Spine.

## Runtime libraries are the actual product for skeletal tools

For Spine, DragonBones, Live2D, and Rive, the editor is half the product. The runtime library — the code that loads the rig data and plays it back in the game — is the other half. Spine ships runtimes for Unity, Unreal, Godot, web, Cocos, libGDX, MonoGame, and a dozen others; that ecosystem is why Spine wins. DragonBones and Rive both built open-source runtimes as a strategy. A skeletal animation tool without runtime support is dead on arrival.

## Lua is the indie scripting language; Python is the studio scripting language

Aseprite, Tiled, GameMaker, Defold, Roblox, LÖVE — Lua dominates the indie space. Toon Boom Harmony and Krita use Python. Photoshop has JavaScript and the new UXP. ComfyUI, AnimateDiff, and AI workflows live in Python. A tool's scripting choice signals its target audience. There's no convergence on one language; there is convergence on having a scripting surface at all.

## Subscriptions are tolerated for studio tools, resented for indie tools

Aseprite at $19.99 perpetual is a phenomenon. Pro Motion NG at $24.99 perpetual is a phenomenon. Pixaki at $30 perpetual is a phenomenon. Indie pixel artists will pay one-time but resent subscriptions. Adobe Animate, Photoshop, and Toon Boom Harmony charge subscriptions because their users are studios where the cost is operational. Spine charges per-seat with tiered pricing (Essential / Professional / Enterprise) — also accepted. The subscription-or-perpetual choice should match the user, not the vendor's revenue preference.

## Free, open-source forks happen when proprietary tools betray their users

LibreSprite forked from Aseprite's last GPL version when Aseprite went proprietary in 2016. Pixelorama exists in part because the community wanted an MIT-licensed alternative not controlled by one company. OpenToonz is a release of Toonz under a permissive license. The pattern: when a beloved tool changes its license model, a fork appears within months. It rarely overtakes the parent, but it consistently survives.

## What converged versus what fragmented

| Converged | Fragmented |
|---|---|
| Timeline shape | Pricing model (one-time vs subscription) |
| Onion skin | Scripting language (Lua / Python / JS) |
| Frame tags pattern | File format (proprietary, with JSON sidecar) |
| Sprite sheet + JSON export | Tile editing approach |
| Skeletal rig structure | Runtime library coverage |
| Layer groups + blend modes | Whether to ship runtime libraries at all |
| Indexed color discipline | Cross-platform vs platform-native |

The converged column is non-negotiable for SpriteMaster. The fragmented column is where strategic choices live.
