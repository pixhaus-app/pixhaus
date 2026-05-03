# Where AI-native interaction has actual leverage

This file is opinionated. It maps the gaps to where AI can move the needle and, just as importantly, where AI would regress the workflow. The temptation in this category is to AI-flavor every interaction. That's how you ship a tool artists hate.

The framing: AI is leverage, not replacement. The artist is still the artist. Their hand is still on the canvas. The AI is the apprentice that handles the toil — the variants, the in-betweens, the cleanup, the QA — so the artist can spend their hours on the parts that need taste.

## High-leverage AI surfaces

These are the places where AI can do real work without taking craft away from the artist.

### Frame interpolation that respects palette discipline

Hand-drawn animation costs because every frame is hand-drawn. AI-assisted in-betweens — interpolating between two key poses while staying inside the palette — would close the cost gap to skeletal rigging while preserving the hand-drawn aesthetic. The non-trivial part is palette respect. A diffusion model that produces antialiased pixels is a regression. A model that produces palette-locked pixels with quantization-aware training is a step change. Retro Diffusion and PixelLab have started here. Neither has delivered production-quality 2x in-betweening for arbitrary palettes.

This is the single most valuable AI feature in the category. Get it right and the math of indie 2D animation changes.

### Variant generation: palette swaps, equipment overlays, expressions

Given a base character, generate the same character in alternate palettes (player skins), with equipment layers (helmets, weapons, capes), in alternate expressions (happy, angry, hurt). Game artists do this manually. AI can do it as derived layers — the artist defines the rule once ("apply this palette swap to this layer set") and the system generates the variants on demand.

The honest version of this is mostly automation, not generation. A "smart palette swap" is half rule-based, half ML refinement. The AI part handles the cases where straight color substitution looks wrong (a character in fire palette versus ice palette has subtle shading differences that a rule engine misses). Layer.ai and Scenario gesture at this; neither has nailed it for sprite-resolution work.

### Reference matching: animate like this clip

The artist drops in a reference video — a real-world walk cycle, an animal motion, a parkour clip — and the AI extracts the motion timing and key poses. The artist still draws the frames, but the timing is already there. Cascadeur does this for 3D rigs. Pose estimation models (OpenPose, MediaPipe) extract 2D motion from video. Bridging that to a sprite animation timeline is not a research problem; it's an engineering problem.

This converts a previously impossible task ("animate like a video reference") into a tractable one. It's leverage, not replacement — the artist still owns every frame.

### Smart cleanup: snap to palette, remove sub-pixel artifacts, fix pivot drift

Diffusion-generated pixel art has known artifacts: anti-aliased edges, palette violations, pivot drift across frames. A "fix it" pass that snaps to the palette, removes sub-pixel anti-aliasing, and aligns pivots is mostly classical image processing with ML for the ambiguous decisions. Retro Diffusion's neural pixelate tool is a primitive version. A serious version of this should be a one-click fix pass on any imported sprite, AI-generated or hand-drawn.

### Multi-angle generation from a single base

Given a sprite drawn in one direction, generate it in 3 / 5 / 7 other directions. PixelLab's directional generation is the only serious attempt in market. The technique combines pose estimation, view synthesis, and style transfer. It's not solved — outputs require manual touch-up — but the time savings versus hand-drawing 8 directions are real.

### Asset library QA

A rules-and-ML hybrid that scans a project and surfaces problems: missing frames, palette violations, inconsistent pivots, frames where a character moved off-canvas, pose discontinuities. Some of this is straightforward image diffing. Some requires ML to judge "this frame looks subtly wrong." Either way, it's a category nobody ships and every studio needs.

### Tile autotile generation from examples

Show the AI three transitions and have it generate the other 44 in the same style. Tilesetter does the geometric part (which tile goes where in the 47-blob layout); generating the actual tile pixels for each transition slot is the AI value-add. This is a relatively well-defined problem with a small output space and a clear evaluation rubric.

### Style transfer for new frames matching existing project style

The artist has drawn 20 frames of a character. They sketch the 21st loosely, the AI fills in the style — same line weight, same shading, same palette discipline. This is style-consistency-from-context, the inverse of "generate a new sprite from a prompt." It only works if the model has the existing project as context. ComfyUI workflows with palette reference latents are the closest current approach. Retro Diffusion's chained-frame consistency techniques point the same direction.

## Low-leverage or anti-leverage AI surfaces

These are places where AI is tempting and wrong. Including them as features ships a worse tool than not having them.

### Generating finished sprites from text prompts

Text-to-sprite is what Midjourney and Leonardo do well. It produces concept art, not game assets. Game assets need to fit a project's existing style, palette, dimensions, and animation rig. A text prompt cannot carry that context. Tools that lead with "type what you want and we'll make a sprite" miss the actual job. Generation has to happen with project context, not in a vacuum.

### Replacing the brush

The brush is the artist's primary instrument. AI smoothing, AI auto-color, AI auto-cleanup-while-painting all sound helpful and all interfere with what the artist is actually doing. Photoshop's "Generative Fill" works for photographers; for pixel artists who deliberately place every pixel, it's the wrong abstraction. Keep AI out of the brushstroke. Put it on commands the artist explicitly invokes.

### Auto-animating without artist intent

A button that says "animate this sprite" and outputs a walk cycle is a slot machine. The artist had no input into how it walks, what mood it has, what the timing is. The output will be generic at best, off-brand at worst. AI can extract motion from a reference (good), in-between known key poses (good), or generate variants of a known animation (good). Inventing animations from a still sprite is bad UX masquerading as a feature.

### Hallucinating palette colors

Diffusion models trained on RGB images do not natively respect palettes. A pixel-art tool that emits non-palette colors and tells the artist "we'll fix it later" violates the core discipline of the medium. Palette discipline must be a hard constraint at generation time, not a post-process. This rules out a lot of off-the-shelf model usage and demands custom training pipelines or quantization-aware decoders.

### Real-time AI suggestions in the canvas

Constant AI suggestions while the artist works (the GitHub Copilot pattern) is wrong for visual work. It interrupts the flow state. It pulls the artist's eye to a UI element that's predicting what they should draw next. The artist's medium is the canvas; the AI's medium should be commands the artist invokes. Not autocomplete. More like a verb on a menu.

## The strategic implication

Every gap in `gaps.md` falls into one of three buckets:

| Bucket | Examples | What fixes it |
|---|---|---|
| AI moves the needle | Frame interpolation, variant generation, reference matching, multi-angle, smart cleanup, asset QA, tile generation, style transfer | New ML capability built into the editor |
| AI doesn't help | Real-time engine preview, collaboration, frame-accurate playback, asset library workflow | Engineering problems unrelated to AI |
| AI is the wrong answer | Replacing brushstrokes, auto-animation from stills, finished-sprite-from-text | Don't ship these as features |

A tool that confuses these buckets ships AI features that backfire and engineering features that go missing. SpriteMaster's design north star should be: do bucket one well, do bucket two anyway, and don't ship bucket three even when investors ask.

## What "AI-native" should mean here

It does not mean "Claude / GPT / diffusion in every menu."

It means the tool is architected so that AI commands have access to project context — palette, character rig, existing frames, style examples — by default. It means generation happens in-place with constraints, not in a side panel. It means the artist's hand stays on the canvas and the AI is the menu of leverage commands they reach for when the toil starts piling up.

Aseprite plus a smart in-betweener is more interesting than any AI generator built from scratch. The real product is the editor that already feels like Aseprite to a working pixel artist, with the AI features they specifically asked for and not the ones the marketing deck demanded.
