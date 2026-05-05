---
title: Your first animation
description: Build a simple walk cycle using the Pixhaus timeline.
---

import { Steps, Aside } from "@astrojs/starlight/components";

This guide walks through creating a 4-frame idle animation.

<Steps>
1. **Open your sprite** or start from the project saved in [Your first sprite](/getting-started/first-sprite/).

2. **Open the timeline.** `View > Timeline`. The timeline panel appears alongside the layers. Frame 1 is already there.

3. **Add frames.** Right-click in the frame header area and choose `Insert frame after`. Repeat until you have 4 frames. Alternatively, `Frame > New Frame`.

4. **Draw each frame.** Click on a frame number in the timeline to make it active. Draw changes to the canvas for that frame only. Use onion skin (`View > Onion Skin`) to see the previous frame as a faint ghost.

5. **Set frame duration.** Click the duration field above each frame (shows `100ms` by default). Change it to `150ms` for a slower idle.

6. **Tag the animation.** Drag across frame numbers in the timeline header to select frames 1–4. Right-click and choose `Tag selection`. Name it `idle` with loop direction `Forward`.

7. **Preview.** Press `Space` or click the play button in the timeline controls. The animation loops in the canvas.

8. **Export.** `File > Export > Animated GIF` to share, or `File > Export > Sprite sheet (PNG)` to hand off to Unity.
</Steps>

<Aside>
The AI verb **Inbetween** can generate intermediate frames between two key frames automatically. See [Inbetween](/ai-verbs/inbetween/) for details.
</Aside>

## Next steps

- [Build your first tilemap](/getting-started/first-tilemap/)
- Read the full [timeline reference](/animation/timeline/)
