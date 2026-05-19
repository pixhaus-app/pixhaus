# Gaps the field hasn't solved

The patterns in `patterns.md` are the wins — the parts of sprite art that mature tools have collectively figured out. This file documents the losses. These are workflows that hurt across every tool researched, that artists work around with rituals and hacks, and that no one has fixed yet.

A new tool that closes even two of these gaps without regressing the converged patterns has a real reason to exist.

## Pixel-perfect editing and skeletal animation don't coexist

Pick one: pixel-perfect indexed-color sprite editing (Aseprite, Pro Motion NG) or bone-rigged skeletal animation (Spine, DragonBones). No serious tool does both well. Aseprite has frame-by-frame animation but no rigging. Spine has rigging but expects you to bring the sprites from Aseprite. Skeletal-rigged pixel art is rare in shipped games partly because the toolchain forces a hand-off and the round-trip is painful.

The closest thing is Spine's mesh-deformed sprite attachments at low resolutions, but the workflow still treats pixels as a final-frame concern, not a first-class constraint during rigging.

## Cross-frame consistency for hand-drawn animation is grueling

Every traditional animation tool (TVPaint, Toon Boom Harmony, OpenToonz, Krita) shares the same pain: 24 fps means 24 hand-drawn frames per second of animation, and the consistency of character proportions across frames is on the artist. Onion skin helps. Light tables help. References help. None of them remove the hand work. This is why so many indie 2D games default to skeletal rigging — not because hand-drawn looks worse, but because hand-drawn costs ten times as much.

## Style consistency is the AI tool curse

Every AI sprite tool — Scenario, PixelLab, Retro Diffusion, Leonardo, Layer — wrestles with the same problem: generating two sprites in the same style is hard, and generating thirty consistent frames of a walk cycle is harder. Custom model training (Scenario), reference images (PixelLab), and LoRA fine-tunes (ComfyUI ecosystem) are the current answers. None of them solve it cleanly. A cohesive 8-direction character with idle, walk, run, attack, hurt, and death animations in identical style is still a multi-day human-led effort even with the best AI tools.

## Asset variations: storage is solved, generation is not

Palette swaps for player skins, equipment overlays for armor, expression sets for portraits, alternate poses for cutscenes — these are bread-and-butter for game artists. The deep dossiers added in May (see [`prior-art.md`](prior-art.md) § "Sparse, link-set variants over duplication" and § D-05) shift this entry: the storage problem *is* solved across the field. Spine has skins, Aseprite splits Cel and CelData for linked cels, Pixelorama uses explicit link-set IDs. The shape varies — the principle that variants share data by reference does not.

The remaining gap is generation with style consistency. Every tool surveyed expects the artist to author each variant by hand or via scripted automation. None of them have an integrated "generate this character in fire palette, with this helmet, with an angry expression" path that respects the project's style and palette discipline. The variant-storage architecture is ready — the variant-generation workflow is the open frontier.

## Tile autotile generation is still tedious

Wang tiles, 47-tile blob sets, terrain rules — the math is well-known, the workflow is not. Tiled and LDtk both have rule-based autotiling, but configuring the rules for a new tileset is a multi-hour task. Tilesetter exists specifically to address this gap (generate the 47 transitions from a single base), and its existence as a $13 add-on tool is itself the evidence that the bigger tools haven't solved it.

## Real-time game engine preview during sprite editing doesn't exist

You're animating an enemy. You want to see how the run cycle looks at the actual size, in the actual scene, with the actual camera, against the actual background. You can't. You export a sprite sheet, switch to Unity or Godot, drop it in, hit play, see it's wrong, switch back, fix it, export again. Every tool researched does this round-trip. Hot-reload importers (community-built) shave seconds off but don't close the loop.

## Multi-angle / 8-directional character generation is rare

Top-down and isometric games need characters in 4 or 8 directions. Generating those by hand is grueling. PixelLab has 4/8-direction generation as a flagship feature precisely because nobody else handles it well. Sprite tools assume the artist will hand-draw each angle. There's no concept of "this character, viewed from these angles" as a project primitive.

## Collaboration is non-existent

Open Aseprite, Pro Motion NG, Spine, Photoshop, or any other tool surveyed. Try to invite a collaborator to edit the same file in real time. You can't. Every sprite tool is single-user, file-based, with version control bolted on through Git or Dropbox. Figma reset everyone's expectations for design tools five years ago. Sprite tools haven't responded.

## Frame-accurate animation preview is approximate

Aseprite, Pro Motion NG, and most animation tools play back at "approximate" frame timing because the editor's render loop isn't synchronized to a fixed-step game loop. The animation that looks right in the editor sometimes doesn't look right in the engine. Game devs work around this with extensive testing in-engine, which compounds the engine-preview gap above.

## Sprite asset QA is manual

Are all 8 directions of the character at the same vertical pivot? Do all frames respect the palette? Are any frames missing? Did the artist accidentally use a non-palette color? These are eyeball-and-spreadsheet checks across every tool. None of them ship asset-validation rules. Tilesetter does some of this for tilesets; nobody does it for character sprites.

## Cleanup pipelines exist but stay outside the editor

The seven-technique pipeline documented in [`../research/grid-snap-quantize-techniques.md`](../research/grid-snap-quantize-techniques.md) (k-means quantization → Sobel gradient profiling → step estimation → walker cut placement → cross-axis stabilization → majority-vote downsampling) and the eighth-stage normalization in [`../research/sprite-pipeline-methodology.md`](../research/sprite-pipeline-methodology.md) show that the cleanup problem is already engineered — but as standalone tools, not as editor verbs. Every artist who works with AI-generated or scanned pixel art reaches for these techniques outside the editor, then imports the result back. See [`prior-art.md`](prior-art.md) § D-03 for the open decision on folding the full pipeline into the Cleanup verb (S27).

## Animation reference matching is manual

"Make this character walk like that reference video." Animators handle this by watching the reference, scrubbing frame by frame, and translating motion into key poses by hand. Cascadeur does this for 3D. No 2D sprite tool does it. AI motion-extraction-to-bones is technically possible (pose estimation models exist) and practically absent from the toolchain.

## Multiplayer / asset library workflows are a regression to file servers

Studios that need multiple artists working on shared characters, palettes, and tilesets fall back to shared drives, Git LFS, Perforce, or custom Asset Manager tools. The sprite tools themselves don't ship asset library or shared palette features. Pixilart's social network gestures at this for hobbyists; nothing equivalent exists for professional pipelines.

## Aseprite's specific gaps are felt across the indie world

Because Aseprite is the dominant indie tool, its specific limitations are the field's de facto pain points. They include: layer group blend modes don't work, no live game-engine preview, scripting cannot extend the UI with custom panels or tools, no built-in multi-cursor or multi-window editing, animation preview FPS is approximate. Closing these gaps in a competing tool is a clear value proposition for any artist already on Aseprite.

## What this list is not

These are not "things AI can fix." Some of them are. Some of them have nothing to do with AI — collaborative editing is a CRDT problem, real-time engine preview is a runtime integration problem, asset QA is a rules engine. Treating "the gap" and "the AI opportunity" as the same set is the trap that kills AI-native products. The gaps are real. The AI subset of them is in `ai-opportunity.md`.

Attribution discipline is also not a user-facing gap — but it is a tooling gap that every dossier in [`../research/`](../research/) independently re-derives (per-file headers, license file copies, `THIRD_PARTY_LICENSES.md`, sibling LICENSE files for vendored assets). The canonical form is consolidated once in [`prior-art.md`](prior-art.md) § "Attribution discipline" so each new port doesn't have to invent its own.
