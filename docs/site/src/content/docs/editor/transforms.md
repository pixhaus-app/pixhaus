---
title: Transforms
description: Move, scale, rotate, flip, and perspective transforms.
---

## Available transforms

| Transform | How to invoke |
|---|---|
| Move | Make a selection, then drag inside the selection bounds |
| Scale | Drag a corner or edge handle on the active selection |
| Rotate | Drag the rotation handle that appears above the selection |
| Flip horizontal | `Sprite > Flip horizontal` or `Ctrl+Shift+H` |
| Flip vertical | `Sprite > Flip vertical` or `Ctrl+Shift+V` |
| Free transform | `Edit > Free transform` (`Ctrl+T`) for numeric input on all axes |

## RotSprite for pixel art rotation

Pixhaus uses the RotSprite algorithm for rotations by default, which produces pixel-art-correct results without bilinear blurring. Rotation by 90°, 180°, and 270° is always pixel-perfect. Other angles use RotSprite's pixel-aware rotation.

Bilinear and bicubic interpolation are available as opt-in choices in the transform options panel for cases where softening is acceptable (e.g., transforming imported PSD layers).

## Integer scaling

When scaling up, the default is nearest-neighbor, which preserves the crisp pixel look. Integer-multiple scaling (2×, 3×, 4×) is also available and guarantees no sub-pixel artifacts.

## Precision input

Open `Edit > Free transform` (`Ctrl+T`) for a dialog with numeric fields: X, Y (position), width, height (size or percentage), rotation angle, skew X, skew Y. Changes preview live before you commit.

Press `Enter` to commit a transform. Press `Escape` to cancel. Committed transforms appear as a single undo entry.

## Scope

Transforms operate on:
- The active selection (if one exists)
- The entire active layer (if no selection)

To transform multiple layers simultaneously, select them all in the layer panel before invoking the transform.
