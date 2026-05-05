---
title: Onion skin
description: See previous and next frames as ghost overlays while drawing.
---

Onion skin shows neighboring frames as transparent ghost images overlaid on the active frame, helping you draw smooth motion.

## Enabling onion skin

Toggle with `View > Onion skin` or `Shift+F1`. The onion skin toolbar appears in the timeline header when enabled.

## Controls

| Control | Description |
|---|---|
| Previous frames | Number of frames before the active frame to show (default: 1) |
| Next frames | Number of frames after the active frame to show (default: 1) |
| Previous opacity | Opacity of previous-frame ghosts (default: 50%) |
| Next opacity | Opacity of next-frame ghosts (default: 50%) |
| Tint | Previous frames tinted red, next frames tinted blue (on by default) |

## Behavior

Ghost images are composited below the active layer stack. They show the fully composited frame, not individual layers, so multi-layer frames look the same in onion skin as they do in playback.

Onion skin only affects the viewport — it does not affect exported files.

## Useful settings for walk cycles

For a walk cycle, set previous and next to 2 each. This shows enough of the motion arc to place the foot contact frames correctly. Turn tint on so you can distinguish which direction you're looking.
