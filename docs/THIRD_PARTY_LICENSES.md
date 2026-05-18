# Third-party licenses and attributions

Pixhaus borrows ideas, ports algorithms, ports shaders, and vendors a small
number of assets from third-party open-source projects. Every borrow is
recorded here, with the upstream URL, commit hash, license, and a list of
touch points inside Pixhaus.

Location note: this file currently lives at `docs/THIRD_PARTY_LICENSES.md`
because the repo layout in `docs/planning/architecture/stack.md` does not yet
allow new top-level paths. The canonical location for license attributions
is repo root (alongside `LICENSE`), where GitHub's license detection and
downstream packagers look for it. Moving this file to the root is a one-line
planning-doc revision that should land as a follow-up to this PR.

Rules for adding to this file:

1. One section per upstream project, ordered alphabetically.
2. Pin the upstream commit hash at the time of borrowing. Update the hash
   only when re-syncing against a newer upstream — never silently.
3. Quote the full upstream license text verbatim in a fenced block,
   including the copyright line. Do not paraphrase.
4. Maintain the four "What we use" buckets: vendored assets, ported
   shaders, ported algorithms, adopted designs. Append entries as
   implementation PRs land.

## Pixelorama

- Upstream: https://github.com/Orama-Interactive/Pixelorama
- Commit referenced for adoption: `b6dbb2b0bf8a8b04ed4a49d525cfec287ff9706b`
- License: MIT
- Adoption plan: [`planning/research/pixelorama-adoption.md`](planning/research/pixelorama-adoption.md)

```text
MIT License

Copyright (c) 2019-present Orama Interactive and contributors

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

### What we use from Pixelorama

Updated as implementation PRs land.

- **Vendored assets.** None yet. The adoption plan flags
  `assets/dither-matrices/bayer{2,4,8,16}.png` as the first candidate for
  verbatim vendoring under `assets/third-party/pixelorama/dither/`, to land
  alongside the gradient and posterize shader streams.
- **Ported shaders.** None yet. The adoption plan tags 15+ effect shaders
  and the layer compositor for translation from Godot's GDShader dialect to
  WebGL2 GLSL ES 3.00.
- **Ported algorithms.** None yet. The adoption plan tags the Allegro
  scanline flood fill, the seven pixel-art rotation algorithms, Scale3X,
  the midpoint ellipse rasterizer, autotile peering, smart-slice
  spritesheet import, and the Aseprite chunk parser as algorithm-level
  ports.
- **Adopted designs.** Refer to
  [`planning/research/pixelorama-adoption.md`](planning/research/pixelorama-adoption.md)
  for the full catalog of design ideas (file format shape, data
  structures, UX patterns) traced to Pixelorama.

### Per-file attribution rules for ported sources

```rust
// Ported from Pixelorama (MIT-licensed, Orama Interactive 2019-present).
// Upstream: https://github.com/Orama-Interactive/Pixelorama/blob/<commit>/<path>
// Original: <one-line summary of what the upstream file does>.
// See docs/THIRD_PARTY_LICENSES.md for the full upstream copyright notice.
```

Commit-message trailer for port PRs:

```text
Source: https://github.com/Orama-Interactive/Pixelorama/blob/<commit>/<path>
```
