# Auto-mesh deformation

**Verb ID:** `pixhaus.builtin.auto-mesh-deformation`
**Stream:** S33
**Backend:** none (classical, local CPU)

Overlays a parameterised grid mesh on a sprite layer, creating a deformation
rig the host exposes as parameter sliders. The artist moves control points to
warp the sprite geometry without redrawing frames — Live2D-style deformation
without explicit bones.

## How it works

The verb divides the sprite canvas into a configurable N×N grid. Each cell
that contains at least one non-transparent pixel becomes an **active region**
with a **control point** at its centre. Cells that fall entirely outside the
sprite's opaque content are skipped; the rig only covers areas the artist
painted.

Two effects are committed on accept:

1. A mesh-visualisation layer — an RGBA overlay drawn on top of the sprite
   showing the grid lines and a small marker at each control point. The artist
   can toggle this layer's visibility; it does not affect rendering.
2. A rig-data payload (see Rig data format below) — the host reads this to
   build the deformation UI and apply warps at render time.

## Inputs

| Field | Type | Default | Description |
|---|---|---|---|
| `pixels` | PixelData (required) | — | Pixel data of the layer to rig |
| `mesh_resolution` | integer 2–16 | 8 | Grid cells per dimension |
| `layer_name` | string | `"Mesh rig"` | Name for the visualisation layer |

Passing `mesh_resolution: null` or omitting it uses the default value of 8,
which gives a 64-cell rig — appropriate for a typical 32×32 character sprite.
Increase to 12–16 for larger sprites with fine-grained deformation needs;
decrease to 2–4 for simple props.

## Outputs

### Visualisation layer (`VerbEffect::AddLayer`)

An RGBA8 image matching the sprite dimensions. Grid lines are drawn in
semi-transparent blue; control points appear as small red squares. The layer
is added directly above the active layer and can be hidden or deleted without
affecting the rig.

### Rig data (`VerbEffect::Custom`)

Effect name: `pixhaus.builtin.auto-mesh-deformation.rig`

The payload is a JSON object with the following shape:

```json
{
  "sprite_width": 32,
  "sprite_height": 32,
  "mesh_resolution": 8,
  "control_points": [
    { "id": 0, "label": "cp_0_0", "x": 2.0, "y": 2.0 },
    { "id": 1, "label": "cp_0_1", "x": 6.0, "y": 2.0 }
  ],
  "regions": [
    {
      "id": 0, "row": 0, "col": 0,
      "x0": 0, "y0": 0, "x1": 4, "y1": 4,
      "control_point_id": 0
    }
  ]
}
```

All coordinates are in canvas pixels. `x0`/`y0` are inclusive; `x1`/`y1` are
exclusive. Control-point positions are at cell centres and may be fractional.

## Edge cases

- **All-transparent sprite:** the rig has zero control points and zero
  regions. The verb succeeds (no error) but emits a note in the preview
  dialog. Apply to a layer that has visible content.
- **`mesh_resolution` larger than sprite dimension:** cells are 1 pixel wide
  or tall. This is valid but produces many control points; prefer a lower
  resolution for small sprites.

## Limitations (first iteration)

Segmentation is grid-only — every active cell gets one control point
regardless of what body part it covers. A follow-up stream will add a
backend-driven path (requiring `SEGMENTATION` capability) that uses a VLM or
dedicated segmentation model to produce semantically meaningful regions
("head", "torso", "left arm"). The rig format and protocol surface are stable;
only the segmentation algorithm changes in that update.

## Invoking from the command palette

1. Select the layer to rig in the layer panel.
2. Open the command palette (Ctrl/Cmd+K).
3. Search for "Auto-mesh deformation" and press Enter.
4. Adjust `mesh_resolution` in the verb form.
5. Click "Run" — a progress bar appears while the grid is computed.
6. Review the visualisation overlay in the preview.
7. Click "Apply" to commit both effects, or "Discard" to cancel.
