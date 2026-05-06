# Verb: Critique

Sends the active sprite to a vision-language model and surfaces structured
findings across five quality categories. The verb is read-only: it never
modifies the project.

## When to use it

Run Critique when you want a second opinion before shipping. It excels at
catching mechanical issues — pivot drift that snuck in across a long animation,
a stray off-palette pixel hidden in a shadow frame, or a frame that lands twice
as long as its neighbours and breaks the walk cycle rhythm.

It is not a style judge. Critique ignores aesthetic choices (line width, shading
philosophy, colour harmony preferences) and only flags things that are objectively
wrong given the data it can see.

## Inputs

| Field | Type | Default | Description |
|---|---|---|---|
| `checks` | `string[]` | `[]` (all) | Categories to check. Empty list runs all five. |
| `notes` | `string` | — | Optional context for the VLM, max ~500 chars. |

Valid `checks` values:

- `"pose_continuity"` — frame-to-frame limb position breaks
- `"palette_violations"` — pixels outside the active palette
- `"missing_frames"` — animation timing gaps or implausible durations
- `"pivot_drift"` — pivot point that moves across frames
- `"style_inconsistency"` — style mismatch relative to project references

## Output

Returns one `Critique` effect containing an ordered list of findings.
Each finding has:

| Field | Type | Description |
|---|---|---|
| `category` | `CritiqueCategory` | Which category the issue belongs to. |
| `severity` | `"info" \| "warning" \| "error"` | How serious. |
| `summary` | `string` | One sentence describing the specific issue. |
| `frame` | `number \| null` | 0-based frame index to jump to. |
| `layer` | `number \| null` | 0-based layer index to highlight. |
| `region` | `Rect \| null` | Canvas region to highlight (reserved; currently null). |

Critique effects are not committed to the undo stack — accepting the preview
does not change the document.

## Requirements

Critique requires a backend with `VISION_LANGUAGE` capability (bit 1). Configure
an Anthropic or OpenAI backend in Settings → AI Backends before invoking.

## Image supply

The verb reads pixel data from the `references` field of the verb context. The
app IPC layer is responsible for compositing the active sprite's visible layers
to PNG and attaching the result before calling `verb_invoke`. If no reference
images are provided, Critique falls back to a metadata-only analysis (frame
counts, durations, palette size) and notes this in its findings.

## Cost estimate

- Typical latency: 10 s
- Max latency: 60 s
- Typical cost: ~$0.005 (Sonnet 4.6 pricing, May 2026)

## Example

Invoke via the IPC command catalog (B4):

```json
{
  "verb_id": "pixhaus.builtin.critique",
  "inputs": {
    "checks": ["palette_violations", "pivot_drift"],
    "notes": "Character is a 4-directional top-down RPG sprite, 8 walk frames per direction."
  }
}
```

Example response (one finding):

```json
{
  "summary": "1 finding",
  "effects": [{
    "kind": "critique",
    "findings": [{
      "category": { "kind": "palette_violation" },
      "severity": "warning",
      "summary": "Frame 5 contains #3C2B1A which is not in the active 16-color palette.",
      "frame": 4,
      "layer": null,
      "region": null
    }]
  }]
}
```
