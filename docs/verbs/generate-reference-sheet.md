# Generate reference sheet

Generates 1–4 candidate reference-sheet images for a Reference-kind library entity.

## What it does

You point the verb at a Reference entity in the project library and provide a text description of the subject. The verb builds a structured backend prompt from the chosen composition template and dispatches an image-generation request. Each returned image is wrapped in a `SheetVariant` with panel layout metadata and generation provenance, then delivered to the host as candidates in the entity's history.

None of the candidates become canonical automatically. The user reviews them and approves one via the B10.3 approval flow. Once approved, the sheet becomes the anchor image for all subsequent AI verb invocations on entities that reference this entity via `anchor_reference_id`.

## Inputs

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `entity_id` | integer (≥ 0) | yes | — | ID of the target Reference entity in the project library |
| `template` | `"character"` \| `"item"` \| `"tileset"` \| `"custom"` | yes | — | Composition template controlling panel layout and prompt engineering |
| `prompt` | string (non-empty) | yes | — | User description of the subject (e.g. "32px fantasy hero with a blue cloak") |
| `negative_prompt` | string | no | `null` | Optional user-supplied negative prompt, appended after the template's own negative clauses |
| `num_variants` | integer, 1–4 | no | `1` | Number of candidate sheets to generate |
| `seed` | integer | no | `null` | RNG seed for reproducible generation |

## Output

A single `pixhaus.builtin.generate_reference_sheet.sheets` custom effect carrying a `GenerateSheetPayload`:

- `entity_id` — the target Reference entity
- `variants` — array of 1–4 `SheetVariantOutput` objects, each with:
  - `image_b64` — base64-encoded PNG sheet image
  - `composition` — panel layout (views, expressions, callouts, outfits, palette swatch) with pixel-coordinate `Rect`s
  - `generation` — provenance record (backend, model, prompt, negative\_prompt, seed)

The host inserts each variant into the Reference entity's `history`. Variant IDs are placeholders; the host assigns real IDs when committing.

## Composition templates

**Character** — five turnaround views (front, side-left, three-quarter, side-right, back) across the top, three expression panels below (neutral, happy, angry), a full-width palette swatch, two detail-callout panels, and one outfit-variant slot. Sheet dimensions: 1024×1536 px.

**Item** — four-angle turnaround in a 2×2 grid (front, side-left, back, side-right), palette swatch, and two detail-callout panels. Sheet dimensions: 1024×1024 px.

**Tileset** — tile-primitives row, transition-variants band, autotile-preview block, and palette swatch. Sheet dimensions: 1024×1024 px.

**Custom** — single centred full-body view and palette swatch. The simplest template; use when no other template fits or when you want maximum compositional freedom. Sheet dimensions: 1024×1024 px.

Each template adds layout instructions and negative-prompt clauses on top of the user's description before sending the request to the backend. See `docs/planning/work/b10-reference-sheets.md#b101` for the full spec.

## Backend requirements

Requires a backend with `IMAGE_GENERATION` capability. The verb runtime selects the highest-priority configured backend that satisfies this. If none is registered, the verb fails before invoking.

## Cost estimate

| | Typical | Maximum |
|---|---|---|
| Latency | 45 s | 5 min |
| Cost (USD) | ~$0.02 | ~$0.20 |

Estimates depend heavily on the backend, model, and number of variants requested.

## Approval flow

Approving a candidate is a separate flow (B10.3, not yet shipped). Until approval, generated variants accumulate in the Reference entity's `history` with no canonical sheet set. The existing AI verbs continue to work against entities without an anchor — consistency features simply don't activate until a sheet is approved.

## Related verbs

- `iterate-reference-sheet` (B10.2) — refine an existing candidate via panel-scoped inpainting
- `variant` — generate a sprite variant; consumes an approved anchor sheet if present
- `tile` — generate a 47-tile autotile set; see `docs/verbs/tile.md`
