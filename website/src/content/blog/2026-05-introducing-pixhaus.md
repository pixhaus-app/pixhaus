---
title: "Introducing Pixhaus"
description: "An open-source, AI-native pixel art editor for sprites, animations, and tilemaps. Built for Unity devs."
date: "2026-05-06"
author: "Pixhaus team"
---

Pixhaus is an open-source pixel art editor that unifies sprite editing, frame animation, and tilemap design in a single project file — with AI verbs built in as first-class canvas commands.

## Why another pixel art editor?

The pixel-art world has two strong tools: Aseprite owns sprite editing and frame animation; Tiled owns tilemap design. Both are excellent. Both end where the other begins, and both predate the AI capabilities that have arrived in the last two years.

Pixhaus is the attempt to close those gaps at once:

1. Sprites, animations, and tilemaps share one project file, one undo stack, one palette.
2. AI verbs run inside the canvas with project context — your palette, your existing layers, your reference frames — not in a side panel that ignores everything around it.
3. MIT license, no paid tier, no license server.

## What we've built so far

The core is in place. Pixel buffer, blend modes (matching Aseprite byte-for-byte), color and palette ops, tilemap data structures, autotile rules, the full `.aseprite` file format read/write, and the `.pixhaus` native format. The canvas viewport renders at 60fps on 4096x4096 sprites with 50 layers. The verb runtime is wired to six AI backends: Anthropic, OpenAI, Replicate, Ollama, ComfyUI, and Stability.

The Unity importer package is on OpenUPM. It reads the sprite sheet JSON export, generates AnimationClip assets per frame tag, sets pivots from slice data.

## What's coming

The editing surface — brush engine, selection UI, timeline, transform handles — is the next wave. Then the AI verbs themselves: 14 built-in verbs from Inbetween to Sketch finishing.

If you want to follow along or contribute: the source is at [github.com/pixhaus-app/pixhaus](https://github.com/pixhaus-app/pixhaus). The task queue and stream design are in `work/queue.md` and `docs/planning/work/streams.md`.
