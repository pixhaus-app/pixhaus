# Animation-tool dossier roadmap

The seven existing dossiers in this directory cover Aseprite, OpenToonz, Pixelorama, Sprite Fusion (grid-snap), FalSprite, the AI sprite-pipeline methodology, and a comparative project-library survey. They do not cover the rest of the animation-tool landscape, and the landscape is rich. This file catalogs tools worth a dossier, categorized by license (which dictates what we can integrate vs only study), and recommends the next batch.

Read this before writing a new dossier. Use [`_research-template.md`](_research-template.md) for the dossier shape itself.

## How license constrains integration

[`CLAUDE.md`](../../../CLAUDE.md) is explicit: Pixhaus is MIT, and GPL / LGPL / AGPL dependencies require explicit approval. That gates what each candidate can contribute:

- **MIT / BSD / Apache** — port-friendly. Code can be lifted with attribution per [`../synthesis/prior-art.md`](../synthesis/prior-art.md) § "Attribution discipline".
- **GPL / LGPL / AGPL** — inspire-only. Read the source, understand the design, reimplement in clean-room Rust/TypeScript. Cite the inspiration; do not lift code.
- **Proprietary / closed source** — design study only. Behavior is observable, source is not. Dossiers in this category are workflow-and-feature notes, not pseudocode walks.

The three categories below are sorted by what we can do with each.

## Open-source, MIT/BSD/Apache-compatible (port-friendly)

These are the highest-leverage candidates because the code itself is portable.

- **Rive** (MIT runtime, [rive.app](https://rive.app/)) — interactive animation runtime with a state-machine timeline. The editor is closed but the runtime is open. State-machine-driven animation is a Pixhaus-friendly idea for AI verbs that need run-time pose blending, and the runtime data shape is exactly the kind of contract our Unity importer would consume.

That's the entire MIT-compatible category. Animation is dominated by GPL and proprietary code — when a permissively-licensed tool appears, it is worth a dossier.

## Open-source, GPL/AGPL (inspire-only per CLAUDE.md)

Cannot port; can study. Convention space, data model, and UX patterns are still fair game for reimplementation.

- **Krita** (GPL, [krita.org](https://krita.org/)) — the most mature open-source painting tool. Full frame-by-frame animation, advanced brush engine, color management, layer compositor. Likely contributions to Pixhaus design: brush engine architecture, animation timeline UX, layer effects, color management, scripting surface. Krita's brush engine alone is a graduate course.
- **LibreSprite** (GPL, [github.com/LibreSprite/LibreSprite](https://github.com/LibreSprite/LibreSprite)) — the fork of Aseprite from its last GPL version (pre-2016 proprietary turn). Divergent for nearly a decade. Likely contributions: features that LibreSprite added or kept that current Aseprite dropped, comparison notes on the divergence (which choices Aseprite made post-fork are worth reconsidering).
- **Pencil2D** (GPL, [pencil2d.org](https://pencil2d.org/)) — lightweight 2D animation focused on traditional frame-by-frame. Smaller surface than Krita; easier to study end-to-end. Likely contributions: minimum-viable animation timeline, tablet-first input model.
- **Synfig Studio** (GPL, [synfig.org](https://synfig.org/)) — vector animation but the timeline patterns are general. Likely contributions: timeline interaction model, animated parameter system (a precursor to Live2D's parameters).
- **Tahoma2D** (AGPL fork of OpenToonz, [tahoma2d.org](https://tahoma2d.org/)) — modernized OpenToonz with cleaner UI and active development. AGPL is even more restrictive than GPL; treat strictly as inspire-only. Likely contributions: what they fixed in OpenToonz, what they kept, what new patterns emerged from active development.

## Proprietary (design study only)

Cannot port; cannot read source. Dossiers in this category document features, workflows, and conventions from public materials and behavior.

- **TVPaint** ([tvpaint.com](https://www.tvpaint.com/)) — industry standard for traditional 2D frame-by-frame animation. Closed and expensive. Likely contributions: pure-raster animation workflow conventions, light-table / onion-skin patterns at scale.
- **Toon Boom Harmony** ([toonboom.com](https://www.toonboom.com/)) — the other industry standard. Hybrid raster + vector + rigging. Likely contributions: how a top-end pipeline integrates frame-by-frame and skeletal in one tool.
- **Pro Motion NG** ([cosmigo.com](https://www.cosmigo.com/promotion)) — niche but venerated indie pixel editor, perpetual license, decades of development. Audience overlaps heavily with Pixhaus. Likely contributions: feature set that survived decades, specific workflows Aseprite lacks.
- **Pyxel Edit** ([pyxeledit.com](https://pyxeledit.com/)) — pixel and tile editor with strong tile workflow. Likely contributions: tile-first authoring patterns, tile referencing across maps.
- **GraphicsGale** ([graphicsgale.com](https://graphicsgale.com/)) — old-school pixel and animation editor, free since 2017. Likely contributions: minimum-viable indexed-color workflow, palette-cycling patterns.
- **Adobe Animate** (formerly Flash) — symbols, instances, motion tweens, the original component-instance pattern. Likely contributions: symbol / instance architecture (close cousin of link-set variants).
- **Procreate Dreams** ([procreate.com/dreams](https://procreate.com/dreams)) — iPad-native animation app with a tracks-based timeline (not horizontal layers). Likely contributions: tracks vs layers timeline pattern, gesture-first input. Procreate Dreams is already partly covered in [`project-library-research.md`](project-library-research.md) § Procreate Dreams.
- **Moho / Anime Studio** ([lostmarble.com/moho](https://moho.lostmarble.com/)) — skeletal + frame hybrid for 2D character animation. Likely contributions: how skeletal and frame coexist in one timeline.
- **Cavalry** ([cavalry.scenegroup.co](https://cavalry.scenegroup.co/)) — procedural-graph animation with a scrub-bar timeline. Likely contributions: procedural-vs-frame interaction patterns.
- **Spine** ([esotericsoftware.com](http://esotericsoftware.com/)) — already covered in [`project-library-research.md`](project-library-research.md) § Spine. A standalone dossier on Spine's runtime data shape and skin architecture would deepen what the project-library survey only sketches.
- **DragonBones** — open-source Spine alternative (MIT runtime, closed editor). The runtime is technically port-friendly; it belongs in the MIT category for that reason but the editor patterns are study-only.
- **Live2D Cubism** ([live2d.com/en](https://www.live2d.com/en/)) — already covered in [`project-library-research.md`](project-library-research.md) § Live2D. Like Spine, worth a dedicated dossier for the parameter system specifically.

## Recommended next dossier batch

Pick three for the next research push. Each picks chosen for high leverage given its license category:

1. **Krita** (GPL, inspire-only) — the most mature open-source painting tool. Pixhaus's brush engine, layer compositor, and color management all benefit from a structured walk of how a graduate-grade tool solves them. Brush engine alone justifies the dossier; the rest is upside.
2. **Rive** (MIT runtime, port-friendly) — the only MIT-licensed candidate in the catalog. State-machine timelines are a forward-looking idea for Pixhaus's verbs and for the Unity runtime. Rive's runtime data shape is a real reference for our own export format.
3. **Pro Motion NG** (proprietary, study-only) — the niche-but-venerated indie pixel editor. The audience overlap with Pixhaus is exact. A dossier here surfaces specific features Aseprite lacks that Pro Motion has kept for decades, and answers "what did the indie pixel market converge on that we are missing."

These three together cover the brush-and-paint depth (Krita), the runtime-and-state-machine forward-looking work (Rive), and the indie-pixel-conventions ground truth (Pro Motion NG). After they land, the next batch should consider Tahoma2D (to refresh the OpenToonz comparison with modernized choices) and LibreSprite (to surface the post-2016 divergence from Aseprite).

## How a new dossier lands

1. Copy [`_research-template.md`](_research-template.md) to `<tool-shortname>-prior-art.md`.
2. Fill it in, focusing on subsystems whose "Pixhaus landing" rows lift cleanly into [`../synthesis/prior-art.md`](../synthesis/prior-art.md)'s port roadmap.
3. Update [`../synthesis/prior-art.md`](../synthesis/prior-art.md): add the new file to **Sources**, add any new recurring pattern, add new conflicts to **Open decisions**, add new rows to the port roadmap.
4. Update [`../product/integrations.md`](../product/integrations.md) if the dossier adds capabilities the PRD does not yet cover.
5. Update [`README.md`](README.md) in this directory to list the new dossier.

One dossier per PR, following the convention PRs #213–#219 established.
