---
title: Use AI verbs to inbetween a walk cycle
description: Let the Inbetween verb fill the gaps between your key frames automatically.
---

import { Steps, Aside } from "@astrojs/starlight/components";

This tutorial starts with a two-frame walk cycle — a contact pose and a passing pose — and uses the **Inbetween** AI verb to generate the intermediate frames that make the motion feel fluid.

**Starter file:** `examples/tutorials/walk-cycle-start.pixhaus` — a 32×32 character with two tagged key frames on a transparent background, indexed mode, 16-color palette.

<Steps>
1. **Open the starter file.** `File > Open` and navigate to `examples/tutorials/walk-cycle-start.pixhaus`. Two frames appear in the timeline, tagged `walk-key`.

2. **Review the key frames.** Scrub the timeline by clicking frame 1 and frame 2. Frame 1 is the contact pose (foot planted), frame 2 is the passing pose (leg swinging through). These are the anchors; the verb generates the frames between them.

3. **Open the AI panel.** `AI > Verbs` or press `Ctrl+Shift+A`. The verb list appears. Select **Inbetween**.

4. **Configure the verb.**
   - **Source frames:** 1 and 2 (the defaults match the timeline selection)
   - **Frames to generate:** 2 (produces a 4-frame cycle total)
   - **Palette mode:** Snap to project palette (ensures the output matches your 16-color palette exactly)
   - **Backend:** leave on the project default

5. **Run the verb.** Click **Preview**. Pixhaus sends the two key frames plus the project palette to the inference backend. A progress indicator appears in the verb panel while the backend processes the request. After 5–30 seconds (depending on backend and hardware), two preview frames appear in a ghost overlay on the canvas.

6. **Review the preview.** Scrub through the four-frame sequence in the preview panel. Check for:
   - Palette discipline — no stray colors outside the 16-color set
   - Silhouette continuity — the character outline should arc smoothly between poses
   - Pixel crispness — no anti-aliasing bleed

   If a frame looks off, adjust the verb settings and re-run. Common fixes: increase `style strength` to pull the output closer to your art style, or reduce `frames to generate` and fill the remaining frames by hand.

7. **Accept the result.** Click **Commit** in the verb panel. The two inbetween frames are inserted between your key frames in the timeline, tagged `walk-key`. A single undo entry covers the entire verb output — `Ctrl+Z` removes all inserted frames at once.

8. **Tag the full cycle.** Drag across all four frames in the timeline header. Right-click and choose `Tag selection`. Name it `walk`, loop direction `Forward`. Preview with `Space`.

9. **Export.** `File > Export > Sprite sheet (PNG)` and use the defaults to hand off to Unity, or `File > Export > Animated GIF` to share a preview.
</Steps>

<Aside>
The Inbetween verb snaps output to the project palette automatically when palette mode is set to "snap." If the generated frames contain unexpected colors, check that the source key frames use only palette colors — imported or painted art that bypassed the palette will leak non-palette colors into the context.
</Aside>

## Troubleshooting

**The preview takes more than 60 seconds.** The cloud backend may be under load. Check `AI > Backend status`. If the primary backend is unavailable, switch to a local backend (Ollama + an appropriate model) in `Edit > Preferences > AI`.

**Generated frames ignore the palette.** The backend returned RGBA pixels; the palette-snap step runs client-side. If snapping looks wrong, try a different dithering mode in the verb options (off, Floyd-Steinberg, Bayer 8×8).

**The inbetween motion is too mechanical.** The verb interpolates linearly by default. Enable `ease in/out` in the verb options to arc the timing. Alternatively, treat the verb output as a rough inbetween and refine individual frames by hand.

## Next steps

- [Export to Unity](/getting-started/export-unity/)
- Read the full [Inbetween verb reference](/ai-verbs/inbetween/)
- Try the [Variant verb](/ai-verbs/variant/) to generate palette-swapped versions of a finished sprite
