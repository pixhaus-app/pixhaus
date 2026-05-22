# Third-party notices

Pixhaus is MIT-licensed. Specific subsystems adapt source code from
other open-source projects under their respective licenses; this
file lists those projects and surfaces their license terms in full.

## Pixelorama (MIT)

Copyright (c) 2019-present Orama Interactive and contributors.

The following Pixhaus code adopts ideas or ports algorithms from
[Pixelorama](https://github.com/Orama-Interactive/Pixelorama),
upstream commit `b6dbb2b0bf8a8b04ed4a49d525cfec287ff9706b`. See
`docs/planning/research/pixelorama-adoption.md` for the full catalog and
`docs/planning/work/pixelorama-adoption-implementation.md` for what landed.

Ported algorithms:

- `core/src/color/ops.rs` — `similar_colors` squared-distance color
  comparison, from `src/Autoload/DrawingAlgos.gd`.
- `core/src/canvas/effects.rs` — CPU ports of the per-layer effect shaders
  in `src/Shaders/Effects/` (outline, drop shadow, brightness, invert).
- `core/src/import/smart_slice.rs` — transparency-based sprite-sheet frame
  detection, from `src/UI/Dialogs/ImportPreviewDialog.gd`.

Adopted designs:

- `core/src/project/effect.rs` — per-layer non-destructive effect stack,
  from `src/Classes/Layers/BaseLayer.gd` `effects`.
- `core/src/project/tileset.rs` + `core/src/tilemap/geometry.rs` — the
  `TileShape` / `tile_offset_axis` model for square, isometric, and hex
  grids, from `src/Classes/Cels/CelTileMap.gd`.

Each ported file carries an additional inline comment naming the specific
Pixelorama source it derives from.

### License terms

Pixelorama is distributed under the following MIT license:

> MIT License
>
> Copyright (c) 2019-present Orama Interactive and contributors
>
> Permission is hereby granted, free of charge, to any person obtaining a
> copy of this software and associated documentation files (the "Software"),
> to deal in the Software without restriction, including without limitation
> the rights to use, copy, modify, merge, publish, distribute, sublicense,
> and/or sell copies of the Software, and to permit persons to whom the
> Software is furnished to do so, subject to the following conditions:
>
> The above copyright notice and this permission notice shall be included in
> all copies or substantial portions of the Software.
>
> THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
> IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
> FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
> AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
> LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
> FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
> DEALINGS IN THE SOFTWARE.

## OpenToonz (BSD-3-Clause)

Copyright (c) 2016, Dwango Co., Ltd.

The following Pixhaus files adapt code from
[OpenToonz](https://github.com/opentoonz/opentoonz):

- `core/src/project/palette.rs` — `PalettePage`, `PaletteAnimation`
  derived from `toonz/sources/include/tpalette.h`.
- `core/src/project/id.rs` — `PalettePageId` newtype derived from
  the page-identity concept in `toonz/sources/include/tpalette.h`.
- `core/src/canvas/blend.rs` — eight blend-mode math functions
  (LinearBurn, DarkerColor, LinearDodge, LighterColor, VividLight,
  LinearLight, PinLight, HardMix) derived from
  `toonz/sources/stdfx/igs_color_blend.cpp`.
- `core/src/transforms/antialias.rs` — morphological anti-aliasing
  derived from `toonz/sources/common/trop/tantialias.cpp`.
- `core/src/selection/autoclose.rs` + `core/src/selection/skeleton_lut.rs`
  — gap-closing flood fill derived from
  `toonz/sources/common/trop/tautoclose.cpp` and the rules in its
  companion `skeletonlut.h`.
- `ai/src/verbs/inbetween/procedural.rs` — variance-rejected
  weighted averaging derived from
  `toonz/sources/common/tvrender/tinbetween.cpp`.
- `vectorize/src/*.rs` — centerline vectorization derived from
  `toonz/sources/toonzlib/tcenterlinevectorizer.cpp`,
  `centerlinepolygonizer.cpp`, `centerlineskeletonizer.cpp`, and
  `centerlinetostroke.cpp`.

Each adapted file carries an additional inline comment naming the
specific OpenToonz source it derives from.

### License terms

OpenToonz is distributed under the following BSD-3-Clause license:

> Copyright (c) 2016, Dwango Co., Ltd.
> All rights reserved.
>
> Redistribution and use in source and binary forms, with or without
> modification, are permitted provided that the following conditions
> are met:
>
> 1. Redistributions of source code must retain the above copyright
>    notice, this list of conditions and the following disclaimer.
> 2. Redistributions in binary form must reproduce the above copyright
>    notice, this list of conditions and the following disclaimer in
>    the documentation and/or other materials provided with the
>    distribution.
> 3. Neither the name of the copyright holder nor the names of its
>    contributors may be used to endorse or promote products derived
>    from this software without specific prior written permission.
>
> THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS
> "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT
> LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS
> FOR A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE
> COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT,
> INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING,
> BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES;
> LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
> CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT
> LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN
> ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE
> POSSIBILITY OF SUCH DAMAGE.
