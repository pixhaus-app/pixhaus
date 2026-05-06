---
title: Introduction
description: What Pixhaus is, who it's for, and how it fits into your workflow.
---

Pixhaus is an open-source pixel art editor built for game artists who work at the intersection of sprites, animations, and tilemaps — and who want AI as a first-class tool, not an afterthought.

## The thesis

The pixel-art world has two converged tools and one unsolved frontier. Aseprite owns sprite editing and frame animation. Tiled owns tilemap design. Both are excellent. Both end where the other begins, and both predate the AI capabilities that have arrived in the last few years. Pixhaus is the open-source, AI-native unification of those two domains, with AI verbs as first-class commands rather than bolted-on side panels.

The hand stays on the canvas. The artist is still the artist. The AI is the apprentice that handles the toil.

## Who it's for

- Indie game artists working across Aseprite + Tiled + AI generators (Scenario, PixelLab, Retro Diffusion, ComfyUI)
- Solo developers who can't afford a five-tool pipeline
- Pixel artists who want AI leverage without losing pixel-perfect discipline
- Studios that need a tool they can host, fork, and extend

## Engine target

The in-scope build ships Unity tooling only. The Pixhaus Unity importer package (UPM, OpenUPM-compatible) handles sprite sheet import, animation clip generation, and tilemap import. Other engines can be supported via community plugins.

## What Pixhaus is not

- **Not a vector editor.** Raster-only.
- **Not a skeletal animation tool.** No bones. Mesh deformation via the auto-mesh-deformation verb is the no-bones path to skeletal-class results.
- **Not a multi-engine tool.** Unity only in the in-scope build.
- **Not a subscription product.** MIT license. No Pro tier. No telemetry by default.
- **Not mobile or web.** Desktop — Windows, macOS, Linux.

## Next steps

- [Install Pixhaus](/getting-started/installation/) and open it for the first time
- [Draw your first sprite](/getting-started/first-sprite/)
- [Build your first animation](/getting-started/first-animation/)
- Coming from Aseprite? See the [migration guide](/migration/)
