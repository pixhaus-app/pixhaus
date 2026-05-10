---
title: Auto-mesh deformation
description: Overlay a deformation mesh on a sprite, creating control points for no-bones posing without redrawing frames.
---

## What it does

Auto-mesh deformation overlays a parameterised grid on a sprite layer and places a control point at the centre of every cell that contains non-transparent pixels. The result is a Live2D-style rig with no explicit bones — you pose the sprite by dragging the control points, and the host warps the underlying pixels at render time so you do not have to redraw frames.

This iteration uses classical grid-based segmentation and runs entirely on the local CPU. A follow-up stream will swap the grid for semantically-aware regions (head, torso, limbs) backed by a `SEGMENTATION`-capable inference backend. The output format is stable — only the segmentation algorithm changes.

## Parameters

- **`pixels`** — `object` (required). The sprite layer to rig as RGBA8 pixel data. Must include `width`, `height`, `bytes_per_pixel`, `stride`, and a `bytes` array. The host fills this in from the active layer's cel at the active frame.
- **`mesh_resolution`** — `integer` (default: `8`, range `2`–`16`). Grid cells per dimension. A value of `8` produces an 8×8 grid (up to 64 control points). Lower values give coarser, easier-to-pose rigs; higher values give finer control at the cost of more sliders.
- **`layer_name`** — `string` (optional). Display name for the mesh-visualisation layer. Defaults to `Mesh rig`.

## Backend requirements

This verb runs entirely on the local CPU and does not require an inference backend.

## Output

- **`AddLayer`** — adds a visualisation layer above the source sprite. The layer renders semi-transparent blue grid lines and opaque red markers at every active control point so you can see the rig at a glance.
- **`Custom`** — emits a `pixhaus.builtin.auto-mesh-deformation.rig` payload carrying the full rig data: sprite dimensions, mesh resolution, the list of control points (each with id, label, and centre coordinates), and the list of active regions (each with grid cell bounds and the id of its driving control point). The host reads this to build a parameter-slider panel for posing.

If the source layer is fully transparent, the verb still returns successfully but adds a note that no control points were created. Apply it to a non-empty layer to get a usable rig.

## Cost and latency

- Typical: free, sub-second on sprites up to a few hundred pixels per side.
- Max: free, scales linearly with pixel count.

## Example

You have a finished idle sprite at `examples/samples/character-mage.pixhaus` and you want a quick body sway without animating it frame by frame. Run Auto-mesh deformation with `mesh_resolution: 6` against the `body` layer. The verb returns a preview with a `Mesh rig` overlay showing a 6×6 grid and a red marker on each cell that overlaps the silhouette — roughly 18 control points for a typical character. Accept the preview.

The host turns the rig payload into a slider panel labeled `cp_0_0`, `cp_0_1`, and so on. Drag the head-row sliders to one side and the foot-row sliders to the other, key the slider values into the timeline, and you have a sway animation built from a single sprite. To re-rig at a different resolution, run the verb again — the new mesh replaces the old one.

## Related verbs

- [Motion from video](/ai-verbs/motion-from-video/) — pair with auto-mesh to drive the rig sliders from a reference clip.
- [Variant](/ai-verbs/variant/) — apply variants (palette swaps, equipment overlays) to the underlying sprite without rebuilding the rig.
- [Inbetween](/ai-verbs/inbetween/) — fill in pose-keyed frames between two slider positions.
