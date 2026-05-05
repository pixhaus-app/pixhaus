---
title: Frame tags
description: Named animation ranges with loop directions.
---

Frame tags mark named ranges of frames that form a single animation clip — `idle`, `walk`, `attack`, and so on. They export directly to Unity animation clips via the sprite sheet JSON metadata.

## Creating tags

1. In the timeline, drag across the frame header numbers to select a range (e.g., frames 1–8).
2. Right-click and choose `Tag selection`, or press `Ctrl+B`.
3. Name the tag and choose a loop direction.

## Loop directions

| Direction | Behavior |
|---|---|
| Forward | Plays frames in order, then loops back to the first frame |
| Reverse | Plays frames in reverse order |
| Pingpong | Plays forward, then backward, then forward (never repeats the end frame) |
| Once | Plays through once, then stops on the last frame |

## Editing tags

Click a tag bar in the timeline to select it. Drag the left or right edge to resize the range. Right-click for rename, change loop direction, or delete.

## Tag display

Tags appear as colored bars above the frame numbers. Each tag gets a distinct color assigned automatically; you can override the color in the tag editor.

## Export

In sprite sheet JSON export (`File > Export > Sprite sheet (PNG)`), frame tags map to the `frameTags` array in the Aseprite-compatible JSON schema:

```json
{
  "frameTags": [
    { "name": "idle", "from": 0, "to": 3, "direction": "forward" },
    { "name": "walk", "from": 4, "to": 11, "direction": "forward" }
  ]
}
```

Unity's Pixhaus importer reads this array and creates one `AnimationClip` asset per tag automatically.
