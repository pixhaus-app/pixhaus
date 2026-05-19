# Third-party notices

Pixhaus is MIT-licensed. Specific subsystems adapt source code from
other open-source projects under their respective licenses; this
file lists those projects and surfaces their license terms in full.

## OpenToonz (BSD-3-Clause)

Copyright (c) 2016, Dwango Co., Ltd.

The following Pixhaus files adapt code from
[OpenToonz](https://github.com/opentoonz/opentoonz):

- `core/src/project/palette.rs` — `PalettePage`, `PaletteAnimation`
  derived from `toonz/sources/include/tpalette.h`.
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
