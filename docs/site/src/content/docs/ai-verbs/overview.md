---
title: AI verbs overview
description: How AI verbs work in Pixhaus and how to configure them.
---

AI verbs are commands that run inference in the context of your project — palette, layers, reference frames, and style examples all feed into the request. They are invoked from the command palette (`Ctrl+K`) or the `AI` menu, and they follow a preview-then-commit flow so you never overwrite work without reviewing first.

## The preview-then-commit flow

Every verb produces a preview layer. You see the result before it touches your project:

1. Invoke the verb (command palette or `AI` menu).
2. Configure options in the verb panel (optional).
3. The verb runs. Progress appears in the AI panel.
4. A preview layer appears above the active layer in the canvas.
5. Accept (`Enter`) to commit the preview to the undo stack, or reject (`Escape`) to discard it.

Committed verb results appear as a single undo entry labeled with the verb name.

## Context injection

Verbs receive:
- The active palette
- The active layer (or selection region if one is active)
- The last N frames (configurable)
- The project's learned style (if [style learning](/ai-verbs/style-learning/) has been run)
- Any reference layers you've marked with `Layer > Mark as reference`

You do not need to manually describe your palette or art style — the runtime assembles the context.

## Backend configuration

Verbs use inference backends — local (Ollama, ComfyUI) or cloud (Anthropic, OpenAI, Replicate, Stability). Configure backends in `Edit > Preferences > AI backends`. Each backend requires an API key (stored in the OS keychain, never written to disk in plaintext) or a local server URL.

Verbs declare their backend requirements. The runtime resolves the best available backend automatically, or you can pin a specific backend per verb in preferences.

## Cost and latency

The AI panel shows estimated cost and latency before you confirm a verb run. Local backends show latency estimates only. Cloud backends show token/API costs based on the current configuration.

## The full verb set

| Verb | Purpose |
|---|---|
| [Inbetween](/ai-verbs/inbetween/) | Generate intermediate frames between two key frames |
| [Continue](/ai-verbs/continue/) | Predict the next 1–3 frames given the last N |
| [Extend](/ai-verbs/extend/) | Generate multi-direction views from a single sprite |
| [Variant](/ai-verbs/variant/) | Palette swaps, equipment overlays, expression sets |
| [Cleanup](/ai-verbs/cleanup/) | Snap to palette, remove anti-aliasing, fix pivot drift |
| [Tile](/ai-verbs/tile/) | Generate a 47-tile autotile set from 1–3 examples |
| [Critique](/ai-verbs/critique/) | Vision-language analysis of a sprite or animation |
| [Style learning](/ai-verbs/style-learning/) | Train a per-project style LoRA from existing layers |
| [Conversational editing](/ai-verbs/conversational-editing/) | Natural language multi-step editor commands |
| [Motion from video](/ai-verbs/motion-from-video/) | Extract motion timing from a reference video |
| [Auto-mesh deformation](/ai-verbs/auto-mesh-deformation/) | No-bones deformation rig from a single sprite |
| [Audio-driven timing](/ai-verbs/audio-driven-timing/) | Beat detection and lip-sync timing |
| [Tileset from description](/ai-verbs/tileset-from-description/) | Generate a complete autotile tileset from a prompt |
| [Sketch finishing](/ai-verbs/sketch-finishing/) | Finish rough sketches in project style |
