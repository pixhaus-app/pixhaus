# Third-party notices

Pixhaus is MIT-licensed. It bundles or adapts the third-party work below. Each
item is compatible with redistribution under those terms.

## Bundled assets

### Phosphor Icons

The UI icon glyphs come from the Phosphor icon font, via the `egui-phosphor`
crate. Phosphor is licensed under the MIT License.

- https://phosphoricons.com
- Copyright (c) 2023 Phosphor Icons

### Geist / Geist Mono

The Geist and Geist Mono typefaces are bundled under the SIL Open Font License
1.1.

- https://vercel.com/font
- Copyright (c) 2023 Vercel, Inc.

## Adapted algorithms and concepts

### Aseprite

Blend-mode math (`core/src/canvas/blend.rs`) reproduces Aseprite's
`src/doc/blend_funcs.cpp` so files round-trip without altering visual output,
and cel linking follows Aseprite's model. Aseprite's source is referenced for
behavioural compatibility only; no Aseprite code is copied.

### OpenToonz

Palette pages and palette animation (`core/src/project/palette.rs`), the
onion-skin model (mobile/fixed ghosts), the gap-closing skeleton
classification LUT (`core/src/selection/skeleton_lut.rs`, `core/build.rs`), and
the morphological antialias driver (`core/src/transforms/antialias.rs`, ported
from `toonz/sources/common/trop/tantialias.cpp`, implementing Reshetov's 2009
MLAA) are adapted from OpenToonz, licensed under BSD-3-Clause.

- https://github.com/opentoonz/opentoonz
- Copyright (c) 2016 DWANGO Co., Ltd.

### Pixelorama

The cel-matrix timeline layout, cel linking, per-frame duration multiplier, and
export-time loop direction are adapted from Pixelorama, licensed under the MIT
License.

- https://github.com/Orama-Interactive/Pixelorama
- Copyright (c) 2019 Orama Interactive
