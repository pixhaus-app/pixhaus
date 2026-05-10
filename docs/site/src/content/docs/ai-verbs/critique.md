---
title: Critique
description: Visual quality analysis — pose continuity, palette violations, missing frames, pivot drift, and style inconsistency.
---

## What it does

Critique sends the active sprite — its metadata, palette, and a composite image of the visible layers — to a vision-language model and reports back a structured list of technical issues. It checks five categories: pose continuity, palette violations, missing frames, pivot drift, and style inconsistency.

It is a read-only verb. Findings appear in the Critique panel as a clickable list; clicking a finding jumps the canvas to the relevant frame and layer. Nothing is committed to the undo stack — Critique tells you what to fix, it does not fix anything for you.

## Parameters

- **`checks`** — `string[]` (default: `[]`, optional). Which categories to run. Each value is one of `pose_continuity`, `palette_violations`, `missing_frames`, `pivot_drift`, or `style_inconsistency`. An empty list means "run all five".
- **`notes`** — `string | null` (default: `null`, optional). Up to ~500 characters of artist context for the model — for example, "this is a side-scrolling enemy, ignore depth-cue inconsistencies". Useful for steering the model away from false positives.

## Backend requirements

- **`VISION_LANGUAGE`** — needed to read the composited sprite image and reason about pose, palette, and style.

The runtime selects the first registered backend that advertises this capability. Anthropic Claude and OpenAI GPT-class models both qualify; local Ollama models with vision support work too if you've configured one.

## Output

Critique emits a single `Critique` effect carrying a list of findings. Each finding has a category, severity (`info`, `warning`, or `error`), a one-sentence summary, and optional frame and layer indices. No pixel data changes; no undo entry is created on commit.

## Cost and latency

- Typical: ~10s, ~$0.005 per call
- Max: ~60s, ~$0.05 per call

Cost scales with sprite resolution and the number of frames composited into the request. A 32×32 four-frame walk cycle stays at the low end; a 256×256 sheet with twenty frames pushes toward the max.

## Example

You're working on `examples/samples/character-knight.pixhaus` and the walk cycle feels off. Run Critique with the default settings and the panel returns three findings:

- *warning, palette_violation, frame 2*: "Pixel at (12, 18) uses #7E7E7E, which is not in the active palette."
- *warning, pose_continuity, frame 3*: "Right foot moves backward between frames 2 and 3, breaking the forward gait."
- *info, pivot_drift, frame 5*: "Pivot is 1px lower than the rest of the cycle."

Click each finding to jump to the offending frame, fix it by hand or with another verb, then re-run Critique to confirm.

## Related verbs

- [Cleanup](/ai-verbs/cleanup/) — fix the issues Critique surfaces, in bulk
- [Conversational editing](/ai-verbs/conversational-editing/) — describe a fix in plain language and let the model plan the edits
- [Style learning](/ai-verbs/style-learning/) — train a project style model so style-inconsistency findings have a reference to check against
