# Iterate reference sheet

Refines an existing reference-sheet variant via panel-scoped inpainting, producing a new derived variant.

## What it does

You point the verb at an existing `SheetVariant` (typically one a user picked from the candidates produced by `generate-reference-sheet`) and supply a refinement instruction. The verb dispatches an `IMAGE_INPAINT` request that edits only the requested region; every pixel outside that region stays bit-identical to the source.

A `panel_label` scopes the edit to one named panel from the variant's `SheetComposition` (e.g. one of the views, expressions, callouts, outfits, or the palette swatch). The verb constructs a greyscale PNG mask sized to the sheet — white inside the panel rectangle, black everywhere else — and hands it to the backend. Without a `panel_label`, the backend edits the whole sheet from the prompt alone.

The new variant lands in the Reference entity's `history` alongside the source. The host wires lineage via `source_variant_id` so the Sheet view can display the iteration chain. Approval (B10.3) chooses which variant becomes canonical.

## Inputs

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `entity_id` | integer (≥ 0) | yes | — | ID of the target Reference entity |
| `source_variant_id` | integer (≥ 0) | yes | — | ID of the variant being refined |
| `sheet_image_b64` | string (PNG, base64) | yes | — | Full sheet image of the source variant |
| `composition` | `SheetComposition` | yes | — | Panel layout from the source variant, used to resolve `panel_label` |
| `panel_label` | string | no | `null` | Panel to scope inpainting to (e.g. `"front"`, `"happy"`, `"palette-swatch"`). Null edits the whole sheet |
| `prompt` | string (non-empty) | yes | — | Refinement instruction (e.g. `"make the hair longer"`) |
| `negative_prompt` | string | no | `null` | Optional user-supplied negative prompt |

The `sheet_image_b64` payload is validated as a PNG before the backend call; non-PNG bytes return `Schema` errors up-front so a bad caller doesn't waste an inference request.

`panel_label` resolution searches the composition's `views`, `expressions`, `callouts`, and `outfits`, plus the `"palette-swatch"` sentinel for the palette rectangle. Unknown labels return `Schema` errors before the backend is called.

## Output

A single `pixhaus.builtin.iterate_reference_sheet.variant` custom effect carrying an `IterateSheetPayload`:

- `entity_id` — the target Reference entity
- `source_variant_id` — the variant this iteration was derived from (for history-strip lineage)
- `variant` — a new `SheetVariantOutput` with:
  - `image_b64` — base64-encoded PNG of the inpainted sheet
  - `composition` — copied unchanged from the source variant; panel rectangles don't move under iteration
  - `generation` — provenance (backend, model, prompt, negative_prompt). The iterate verb records `seed: None` because `ImageEditRequest` has no seed field; iteration is intentionally convergent rather than reproducible

The host inserts the new variant into `history`. Variant IDs are placeholders; the host assigns real IDs when committing.

## Backend requirements

Requires a backend with `IMAGE_INPAINT` capability. The verb runtime selects the highest-priority configured backend that satisfies this. If none is registered, the verb fails before invoking.

## Cost estimate

| | Typical | Maximum |
|---|---|---|
| Latency | 20 s | 2 min |
| Cost (USD) | ~$0.01 | ~$0.08 |

Inpainting is typically faster and cheaper than full generation because the backend only synthesises the masked region.

## Threading

The mask PNG is decoded, written per pixel, and re-encoded synchronously on a `tokio::task::spawn_blocking` thread so the async reactor stays free for backend I/O. This matches the convention in `docs/verb-protocol.md` (Threading): verbs that touch every pixel in a buffer self-schedule.

## Related verbs

- `generate-reference-sheet` (B10.1) — produce the initial candidate sheets this verb refines
- `variant` — generate a sprite variant from an approved anchor sheet
- Approval flow (B10.3, not yet shipped) — pick a canonical variant from the iteration history

## Spec

`docs/planning/work/b10-reference-sheets.md#b102`
