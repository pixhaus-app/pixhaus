# <Tool or methodology name> prior-art dossier

> Template for new entries in `docs/planning/research/`. Copy this file, rename to `<tool-shortname>-prior-art.md` (or `<topic>-research.md` for non-tool surveys), and replace every angle-bracket placeholder. Delete this blockquote when done.
>
> Goal: produce a dossier whose "Pixhaus landing" rows can be lifted into `../synthesis/prior-art.md` (the consolidated digest) without re-derivation. The digest is the source of truth for *what to do*; this dossier is the source of truth for *how the upstream does it*.

## Context

One paragraph. Why does this dossier exist? Which Pixhaus stream, bedrock spec, or open question prompted the research? What decision is downstream of it?

## Upstream license & repo

- **Project**: <name>
- **Repo**: <https URL>
- **License**: <SPDX identifier; note if mixed (e.g., MIT for `src/foo/`, EULA for `src/app/`)>
- **Commit pin**: <full SHA or version tag at time of research — research goes stale; pin matters>
- **Language(s)**: <primary>
- **Size**: <approx LOC or file count, so a reader can calibrate effort>
- **Why this license is acceptable for Pixhaus**: <one sentence; MIT/BSD-3/Apache-2 are fine, GPL/LGPL/AGPL are not — see `pixelorama-adoption.md` § MIT-compliance mechanics for the standard discipline>

## Subsystem breakdown

Map the upstream's source tree to the units worth attention. Keep it scannable — a table or a short bulleted list per subsystem with one-line summaries. Don't paste the entire file tree.

| Upstream path | What it does | Portable? | License |
| --- | --- | --- | --- |
| `<path>` | <one line> | <yes / no / inspire-only> | <SPDX> |

## Per-technique deep dives

Repeat one section per algorithm, data structure, or pattern worth porting. Use this shape so the rows lift cleanly into the digest's port-roadmap matrix.

### <N>. <Technique name> — `<upstream path>` (<license>)

#### What it does

Two to four sentences. Focus on *behavior*, not implementation.

#### How the upstream decomposes it

Brief walk of the key types / files / functions. Cite line numbers when they help future review (`src/foo/bar.cpp:142`).

#### Why the decomposition pays off

What does the upstream's choice make easy? What would a naive implementation get wrong?

#### Pseudocode or algorithm walk (when not obvious)

Code blocks fine. Keep them short — link to the upstream for the full source.

#### Pixhaus landing

State this in a form that lifts directly into the digest's port roadmap:

- **Insight / algorithm**: <one line>
- **Target file**: `<crate>/src/<path>` (or `ui/src/<path>`)
- **Bedrock / stream**: <B# or S#>
- **Size**: S / M / L / XL
- **Open questions**: <any decisions that need resolution before porting>

#### Attribution checklist

- [ ] Per-file header on the target file (upstream repo, commit pin, copyright, SPDX, license-file path)
- [ ] `licenses/<upstream-shortname>-<spdx>.txt` exists in the repo
- [ ] Repo-level `THIRD_PARTY_LICENSES.md` updated
- [ ] Tests + verification reference upstream behavior when applicable

## Recommended adoption verdict

Pick one and justify in one paragraph:

- **Port** — lift the code with attribution.
- **Adapt** — design adoption (Pixelorama's tier D); reimplement in Rust/TS guided by upstream design, attribute the design idea.
- **Vendor** — ship upstream assets as-is (prompts, palettes, themes); sibling `LICENSE` file.
- **Reference only** — cite as inspiration in planning docs; no code or asset lift.

Most dossiers will mix verdicts across subsystems. Aseprite L0–L3 is *port*; L4+ is *inspire-only*. Pixelorama tiers A (asset reuse), S (shader port), P (port), D (design adoption) is a useful four-way refinement.

## Cross-references

Where does this dossier intersect other planning material?

- **Other dossiers**: `<file>` § <section> — <what overlaps>
- **Synthesis digest**: which recurring pattern(s) in `../synthesis/prior-art.md` does this dossier feed?
- **Bedrock specs**: which `../work/bedrock.md` sections or `../work/b9-*` files does this dossier inform?
- **Streams**: which `../work/streams.md` entries consume this?

## Conspicuous absences

What does the upstream notably *lack*? Sometimes the absence is the lesson (e.g., Aseprite ships without a project library; sprite artists work around it with CLI tooling — see `project-library-research.md` § Aseprite).

## Open questions

Decisions that need to be made before porting can start. Prefer to surface conflicts to the digest's "Open decisions" log rather than burying them here.

## References

External links, papers, blog posts, commit refs, talks. One link per line, with a one-line description.
