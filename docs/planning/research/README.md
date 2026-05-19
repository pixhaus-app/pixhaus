# Prior-art research

Deep dossiers on tools and methodologies we mine for patterns, algorithms, and design choices. The dossiers go *deep* on one upstream each; the consolidated digest at [`../synthesis/prior-art.md`](../synthesis/prior-art.md) tells you what to actually do.

**Start there.** Open a dossier below only when the digest points you at a specific port unit and you need the upstream's pseudocode, source-tree walk, or rationale.

## Dossiers

- [`aseprite-prior-art.md`](aseprite-prior-art.md) — Aseprite document model, file format, rendering, undo, dithering, palette. MIT levels 0–3 portable; EULA levels 4+ inspire-only.
- [`opentoonz-comparison.md`](opentoonz-comparison.md) — Production-tested algorithms from Studio Ghibli's pipeline: 22 blend modes, morphological anti-aliasing, gap-closing flood fill, centerline vectorization, palette pages. BSD-3.
- [`pixelorama-adoption.md`](pixelorama-adoption.md) — ZIP+JSON project format, sparse palette, indexed mode, cel linking, shader ports, tilemap autotile. MIT, with explicit tiered adoption plan (A asset / S shader / P port / D design).
- [`falsprite-prior-art.md`](falsprite-prior-art.md) — Two-stage LLM prompt structure (CHARACTER × CHOREOGRAPHY) for grid-shaped sprite-sheet animation via fal.ai; row-major frame math; worker-pool GIF export. MIT.
- [`grid-snap-quantize-techniques.md`](grid-snap-quantize-techniques.md) — Sprite Fusion's seven-technique pixel-snap pipeline: k-means quantization, Sobel gradient profiling, step estimation, walker cut placement, majority-vote downsampling. MIT.
- [`sprite-pipeline-methodology.md`](sprite-pipeline-methodology.md) — Anchor-first generation, directional economy (flip > regenerate), neutral anchor reset, seven-step normalization. AI sprite-sheet methodology. MIT.
- [`project-library-research.md`](project-library-research.md) — Comparative survey of multi-asset organization across Blender, Spine, Live2D, Unity, Adobe Animate, Aseprite, Pixelorama, Procreate Dreams, Krita, Scenario, ComfyUI, Midjourney.

## Adding a new dossier

Copy [`_research-template.md`](_research-template.md) to `<tool-shortname>-prior-art.md` and fill it in. The template enforces the section shape every dossier above follows, so its "Pixhaus landing" rows lift cleanly into the consolidated digest without re-derivation.

After the dossier merges, update [`../synthesis/prior-art.md`](../synthesis/prior-art.md):

- Add the new file to the **Sources** list.
- Add any new recurring pattern that crosses with the existing dossiers, with `Seen in:` and `Lands in:` cross-references.
- Add new conflicts to the **Open decisions** table, or close decisions the new evidence resolves.
- Add port-roadmap rows from the dossier's "Pixhaus landing" sections.

## Related material

- [`../synthesis/prior-art.md`](../synthesis/prior-art.md) — the consolidated digest. Read this first.
- [`../synthesis/patterns.md`](../synthesis/patterns.md), [`gaps.md`](../synthesis/gaps.md), [`ai-opportunity.md`](../synthesis/ai-opportunity.md) — broader tool-catalog synthesis from the May 3 survey; complements (does not duplicate) the prior-art digest.
- [`../work/bedrock.md`](../work/bedrock.md) and [`../work/streams.md`](../work/streams.md) — implementation drivers. The digest's port-roadmap rows reference these IDs.
