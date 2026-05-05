---
title: FAQ
description: Frequently asked questions about Pixhaus.
---

## General

### Is Pixhaus free?

Yes. Pixhaus is MIT-licensed open-source software. There is no Pro tier, no license server, no subscription. If you build something with it, you owe nothing.

### Is it stable enough to use for real projects?

Check the release notes for the current status. The core editing loop (draw, layer, animate, export) is the first thing to stabilize. AI verbs and scripting land in later streams.

### Can I use it commercially?

Yes. The MIT license permits commercial use without restriction.

### Does Pixhaus send telemetry?

No telemetry by default. An optional opt-in crash reporting feature (stream S51) will prompt you on first launch and default to off. If you never enable it, nothing leaves your machine.

## Editing

### Does Pixhaus replace Aseprite?

For most pixel art and animation workflows, yes — that's the goal. The editing surface aims for full parity with Aseprite. If you find a gap, open a GitHub issue.

### Can I open my existing .aseprite files?

Yes. See [Aseprite compatibility](/reference/aseprite-compat/) for what round-trips cleanly.

### Is there a symmetry mode?

Yes. Enable symmetry in the tool options bar while a drawing tool is active. Horizontal, vertical, and both (4-way) modes are supported.

### Can I draw pixel-perfect lines?

Yes. Enable "pixel-perfect" in the pencil tool options to automatically remove doubled corner pixels.

## AI features

### Do I need an API key to use AI verbs?

For cloud backends (Anthropic, OpenAI, Replicate, Stability), yes. For local backends (Ollama, ComfyUI), no — you run the model locally. Configure backends in `Edit > Preferences > AI backends`.

### Does Pixhaus train on my art?

No. Pixhaus sends your art to inference APIs only when you explicitly invoke an AI verb. Nothing is sent passively. The style learning verb (S30) trains a LoRA on Replicate using your project's frames — read Replicate's terms to understand their data retention policy. Local training (via local Diffusers) never leaves your machine.

### Which AI verb is best for walk cycles?

**Inbetween** for filling frames between key poses. **Continue** for extending an animation past the key frames you've drawn. **Cleanup** as a final pass to snap output to your palette.

## Unity

### Which Unity versions are supported?

Unity 2022.3 LTS minimum, Unity 6 primary target.

### Where is the Unity importer?

The Pixhaus Unity importer package is in `unity/` in the repository. It's UPM-compatible and will be published on OpenUPM when stable.

### Does Pixhaus work with Godot or Unreal?

Not in the in-scope build. Sprite sheet exports are in the Aseprite-compatible JSON format, which any engine can read with a custom importer. Community-built engine integrations are welcome as plugins.

## Scripting and plugins

### Can I use my Aseprite scripts in Pixhaus?

With minor modifications in most cases. The Lua API mirrors Aseprite's `app` global. See [Aseprite compatibility](/reference/aseprite-compat/) and [Lua API reference](/scripting/lua-api/).

### Is there a plugin registry?

Planned but not yet live. For now, share plugins on GitHub and post in the community discussions.

## Contributing

### How do I report a bug?

Open a [GitHub issue](https://github.com/pixhaus-app/pixhaus/issues). Include the Pixhaus version, OS, steps to reproduce, and the `.pixhaus` file if relevant.

### How do I contribute code?

Read [CONTRIBUTING.md](https://github.com/pixhaus-app/pixhaus/blob/main/CONTRIBUTING.md). The project uses a stream-based parallelization model — each feature stream is a separate branch and PR.
