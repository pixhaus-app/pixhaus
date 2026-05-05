---
title: Timeline
description: The animation timeline — frames, cels, and playback.
---

The timeline panel sits at the bottom of the editor. Open it with `Tab` or `View > Timeline`.

## Layout

The timeline is a two-axis grid:
- **Y axis (rows)** — layers, mirroring the layer panel order
- **X axis (columns)** — frames, numbered from 1

Each cell is a **cel** — the pixel content of that layer at that frame. A cel can be empty, linked (shares pixel data with another frame), or unique.

## Frame operations

Right-click any frame column header for:
- Insert frame before / after
- Delete frame
- Duplicate frame
- Copy / paste frame
- Reverse selected frames
- Set duration

Hold `Shift` to select a range of frames; hold `Ctrl` to select individual frames.

## Cel operations

Right-click a cel cell for:
- Clear cel (make it empty)
- Link cel (share pixel data with another frame)
- Unlink cel (break a link, creating an independent copy)
- Copy / paste cel

## Frame duration

Each frame has an independent duration in milliseconds. Click the duration field above a frame column to edit it. The default is 100ms. Selecting multiple frames and editing one duration sets all selected frames to the same value.

## Playback

| Action | Shortcut |
|---|---|
| Play / pause | `Space` |
| Stop | `Shift+Space` |
| Next frame | `.` |
| Previous frame | `,` |
| First frame | `Home` |
| Last frame | `End` |
| Toggle loop | `L` (in timeline focus) |

## Performance

The timeline supports 200 frames × 50 layers and scrolls at 60 fps. Thumbnails update within 100ms of a paint operation, batched to avoid repainting on every stroke pixel.
