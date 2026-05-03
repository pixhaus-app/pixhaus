# Naming

## Recommendation: Pixhaus

The portmanteau of pixel and Bauhaus, pronounced PIX-house. The Bauhaus reference is the thesis: a modernist design school built on the idea that form follows function, that aesthetic and utility are one thing, that modular grids and basic geometric units are the building blocks of design. Pixel art is exactly that, at the smallest possible scale — a grid of unit squares whose composition is the entire art form. The name carries the design pedigree without spelling it out.

Two syllables, short, memorable. The "pix" prefix lands instantly as pixel. The "haus" ending is distinctive enough to be searchable and ownable. It's not descriptive in a generic sense — it's a name with a point of view.

### Why Pixhaus over the alternatives

**Mosaica** — the strong runner-up — is clean across every availability check and has a perfectly accurate metaphor. Pixel art and tilemaps are mosaics of small unit tiles. Both are collections of tesserae forming larger images. The metaphor maps cleanly to the product scope. The drawback is generic-feeling. The -ica suffix reads classical and a little dated, and the word doesn't carry a thesis the way Pixhaus does. Mosaica tells you what the product is. Pixhaus tells you what the product is for.

**Pixelforge** is descriptive and safe but sits in the well-traveled "X-forge" naming category. It's also the only candidate where modern TLD availability is meaningfully blocked — pixelforge.io is registered through 2026. For an open-source tool that wants a clean .io / .dev presence, that's a real strike.

**Pixaria** is a coined word with no collisions and full availability, but it's vague. Nothing about "pixaria" tells the user what the product does, and a name without a hook is hard to brand around.

### Availability summary

The deeper verification round confirmed both Pixhaus and Mosaica are clean on the dimensions that matter for an open-source project:

| | Pixhaus | Mosaica |
|---|---|---|
| github.com/&lt;name&gt; | available (404) | available (404) |
| npm package | available (404) | available (404) |
| .com | taken (legacy registration, no active product) | taken (legacy registration, no active product) |
| .io | likely available (CAPTCHA blocked full check) | likely available (CAPTCHA blocked full check) |
| .dev | likely available | likely available |
| Same-space prior art | none found | none found |
| Trademark risk | low | low |

Both names need .io / .dev confirmation before moving. The CAPTCHA-blocked verification means we don't have a 100% clean signal — but neither domain resolves to anything, neither has WHOIS data showing recent registration, and the agent's read of the registrar flow suggested both are likely available.

**Update (post-research):** pixhaus.app secured. The `.app` TLD is HTTPS-required by Google policy, so plain-HTTP squatting isn't a risk — better than `.io` for a modern OSS project.

Remaining action: reserve `github.com/pixhaus` and the npm package `pixhaus` before any public commitment. Optionally also grab `pixhaus.dev` as a redirect target.

### What was disqualified and why

| Name | Reason |
|---|---|
| Tessera | Existing data visualization library (Tessera by RStudio) creates real prior-art conflict |
| Quilt | Quilt data versioning tool actively occupies the developer-tool namespace |
| Spryte | Existing game library named Spryte |
| Spritely | Spritely Scheme programming language project |
| Pixelforge | .io locked through 2026; descriptive name in crowded "X-forge" category |
| Spritery | GitHub org claimed |
| Pixmint | GitHub org claimed |
| Embergrid | GitHub org claimed; no clear pixel art association |
| Cels | "Cel" vs "cell" vs cellular networks creates search noise |
| Pyx | Obscure, pronunciation ambiguous, existing Cards Against Humanity backend uses the name |
| SpriteMaster | GitHub org taken; also the project's working codename, public-private confusion risk |

### Naming rationale and aesthetic

Open-source pixel art tools have a tradition of naming that runs from descriptive (Pixelorama, Pixilart, GraphicsGale) to mythic (Krita = Sanskrit for "create") to acronym-evolved (Aseprite from Allegro Sprite Editor). Pixhaus sits in a fourth category that's underused in this space: design-school reference. Bauhaus, De Stijl, Constructivism, the Memphis Group — the modernist schools have specific aesthetic ideologies that pixel art quietly inherits. Naming the tool Pixhaus claims that lineage explicitly.

### Steps before committing

1. ~~Register pixhaus.io and pixhaus.dev~~ — **pixhaus.app secured.** Optionally grab pixhaus.dev as a redirect.
2. Reserve github.com/pixhaus as an organization.
3. Reserve the npm package name "pixhaus" with a placeholder.
4. Reserve social handles where it matters (twitter/x, mastodon, bluesky, discord).
5. Do a one-pass trademark sanity check via the USPTO TESS database for "Pixhaus" — not legal advice, just due diligence before public launch.

### See also

Full per-name verification: [name-research.md](name-research.md).
