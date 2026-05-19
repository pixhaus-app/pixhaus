# S54–S60 OpenToonz adoption — implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task.
> Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Land all seven OpenToonz-derived improvements (palette pages
+ animation, eight new blend modes, morphological AA, gap-closing
magic wand, procedural inbetween, centerline vectorization, SIMD
audit) on a single feature branch and ship them as one PR.

**Architecture:** One conventional commit per stream on
`feat/s54-s60-opentoonz-adoption`. Schema bumps `MAJOR=4 MINOR=0` →
`MAJOR=4 MINOR=1` in S54. S55 blend modes ride the same `minor=1`
contract. S59 introduces a new `pixhaus-vectorize` workspace crate.
All adapted code carries BSD-3 attribution to OpenToonz.

**Tech stack:** Rust 1.95 (edition 2024), Tauri 2, `serde` +
`rmp-serde`, `ts-rs`, `rstest`, `proptest`, `insta`, `image-compare`,
`thiserror`, `tracing`, `rayon`, `criterion`.

**Spec:** `docs/planning/work/s54-s60-opentoonz-adoption.md`

---

## Task 0: Setup — third-party notices and streams.md

**Files:**
- Create: `THIRD_PARTY_NOTICES.md`
- Modify: `docs/planning/work/streams.md`

### Task 0.1: Create THIRD_PARTY_NOTICES.md

- [ ] **Step 1: Create the file with BSD-3 + Dwango copyright**

```markdown
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
```

### Task 0.2: Add stream rows S54–S60 to streams.md

- [ ] **Step 1: Modify `docs/planning/work/streams.md`**

Append after the "AI verbs — second wave (S53)" subsection:

```markdown
### OpenToonz adoption (S54-S60)
| ID | Name | Critical |
|---|---|---|
| S54 | Palette pages and animation [opentoonz] |  |
| S55 | Linear/contrast blend modes [opentoonz] |  |
| S56 | Morphological anti-aliasing transform [opentoonz] |  |
| S57 | Gap-closing magic wand [opentoonz] |  |
| S58 | Procedural inbetween fallback [opentoonz] |  |
| S59 | Centerline vectorization crate [opentoonz] |  |
| S60 | SIMD audit and criterion baseline |  |
```

Then append seven detail subsections after the existing `### S53.`
detail section. Each follows this template:

```markdown
### S54. Palette pages and animation

**Scope:** Extend `core/src/project/palette.rs` with `PalettePage`
(named subset views over a palette's entries) and `PaletteAnimation`
(per-entry keyframed colors over the project's frame range). Both
fields are additive and default to empty; pre-S54 files load
unchanged. Schema minor bumps from 0 to 1.

**Depends on:** B2 (data model), S02 (palette ops).

**Interfaces:** S18 (palette panel UI), S07 (.pixhaus serializer).

**Reference:** `toonz/sources/include/tpalette.h` (BSD-3-Clause).
See `docs/planning/research/opentoonz-comparison.md` and
`docs/planning/work/s54-s60-opentoonz-adoption.md` for the
adaptation note and attribution.
```

Repeat the same template for S55–S60 with their scopes from the spec
doc.

### Task 0.3: Commit Task 0 results

- [ ] **Step 1: Stage and commit**

```bash
git add THIRD_PARTY_NOTICES.md docs/planning/work/streams.md
git commit -m "$(cat <<'EOF'
docs: add THIRD_PARTY_NOTICES and stream rows S54-S60

First adaptation from OpenToonz BSD-3-Clause source. Streams index
gains rows S54-S60 plus per-stream detail sections.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 1: S54 — palette pages and animation

**Files:**
- Modify: `core/src/project/id.rs`
- Modify: `core/src/project/palette.rs`
- Modify: `core/src/project/schema.rs`
- Modify: `core/src/project/mod.rs`
- Create: `io/tests/fixtures/legacy_v4_0/palette_no_pages.pixhaus`
- Modify: `io/src/pixhaus/read.rs` (test only)
- Create: `core/tests/palette_pages_animation.rs`

### Task 1.1: Add `PalettePageId` newtype

- [ ] **Step 1: Add the failing test inline in `core/src/project/id.rs`**

Add to the existing `#[cfg(test)] mod tests`:

```rust
#[test]
fn palette_page_id_round_trips_serde() {
    let id = PalettePageId::new(42);
    let bytes = rmp_serde::to_vec_named(&id).unwrap();
    let back: PalettePageId = rmp_serde::from_slice(&bytes).unwrap();
    assert_eq!(id, back);
}
```

- [ ] **Step 2: Run and confirm failure**

```bash
cargo nextest run -p pixhaus-core id::tests::palette_page_id_round_trips_serde
```

Expected: FAIL — `PalettePageId` not defined.

- [ ] **Step 3: Add the newtype**

In `core/src/project/id.rs`, following the existing `PaletteId`
pattern:

```rust
#[derive(
    Copy, Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd,
    Serialize, Deserialize, TS,
)]
#[ts(export)]
#[serde(transparent)]
pub struct PalettePageId(u32);

impl PalettePageId {
    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn value(self) -> u32 {
        self.0
    }
}

impl std::fmt::Display for PalettePageId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "page-{}", self.0)
    }
}
```

- [ ] **Step 4: Run and confirm pass**

```bash
cargo nextest run -p pixhaus-core id::tests::palette_page_id_round_trips_serde
```

Expected: PASS.

### Task 1.2: Add `PalettePage` struct

- [ ] **Step 1: Write the failing test**

Append to `core/src/project/palette.rs` tests:

```rust
#[test]
fn palette_page_serde_round_trip_msgpack() {
    let page = PalettePage {
        id: PalettePageId::new(1),
        name: "skin tones".into(),
        entry_indices: vec![0, 1, 2, 5, 8],
    };
    let bytes = rmp_serde::to_vec_named(&page).unwrap();
    let back: PalettePage = rmp_serde::from_slice(&bytes).unwrap();
    assert_eq!(page, back);
}
```

- [ ] **Step 2: Run and confirm failure**

```bash
cargo nextest run -p pixhaus-core palette::tests::palette_page_serde_round_trip_msgpack
```

Expected: FAIL — `PalettePage` not found.

- [ ] **Step 3: Add `PalettePage`**

In `core/src/project/palette.rs`:

```rust
use super::id::{PaletteId, PalettePageId};

/// A named subset view over a palette's entries.
///
/// Adapted from OpenToonz toonz/sources/include/tpalette.h under
/// BSD-3-Clause. See THIRD_PARTY_NOTICES.md.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PalettePage {
    pub id: PalettePageId,
    pub name: String,
    /// Indices into the parent palette's `colors` vector.
    pub entry_indices: Vec<u32>,
}

impl PalettePage {
    #[must_use]
    pub fn new(id: PalettePageId, name: impl Into<String>) -> Self {
        Self { id, name: name.into(), entry_indices: Vec::new() }
    }
}
```

- [ ] **Step 4: Confirm pass**

```bash
cargo nextest run -p pixhaus-core palette
```

Expected: every palette test passes.

### Task 1.3: Add `PaletteAnimation` struct with `resolve()`

- [ ] **Step 1: Write failing tests for `resolve()`**

Append to `core/src/project/palette.rs` tests:

```rust
use super::FrameIndex;

#[test]
fn animation_resolve_empty_returns_fallback() {
    let anim = PaletteAnimation::default();
    let fallback = Rgba::new(255, 0, 0, 255);
    assert_eq!(anim.resolve(0, FrameIndex::new(5), fallback), fallback);
}

#[test]
fn animation_resolve_returns_keyframe_color_at_exact_frame() {
    let mut anim = PaletteAnimation::default();
    let target = Rgba::new(0, 0, 255, 255);
    anim.set(0, FrameIndex::new(3), target);
    assert_eq!(
        anim.resolve(0, FrameIndex::new(3), Rgba::new(0, 0, 0, 255)),
        target
    );
}

#[test]
fn animation_resolve_between_keyframes_uses_step() {
    let mut anim = PaletteAnimation::default();
    let frame1 = Rgba::new(255, 0, 0, 255);
    let frame5 = Rgba::new(0, 255, 0, 255);
    anim.set(0, FrameIndex::new(1), frame1);
    anim.set(0, FrameIndex::new(5), frame5);
    // Step semantics: frames 1..=4 resolve to frame1, frames 5+ to frame5.
    let fallback = Rgba::new(0, 0, 0, 255);
    assert_eq!(anim.resolve(0, FrameIndex::new(3), fallback), frame1);
    assert_eq!(anim.resolve(0, FrameIndex::new(5), fallback), frame5);
    assert_eq!(anim.resolve(0, FrameIndex::new(99), fallback), frame5);
}

#[test]
fn animation_resolve_before_first_keyframe_returns_fallback() {
    let mut anim = PaletteAnimation::default();
    anim.set(0, FrameIndex::new(5), Rgba::new(0, 255, 0, 255));
    let fallback = Rgba::new(0, 0, 0, 255);
    assert_eq!(anim.resolve(0, FrameIndex::new(2), fallback), fallback);
}

#[test]
fn animation_serde_round_trip_msgpack() {
    let mut anim = PaletteAnimation::default();
    anim.set(0, FrameIndex::new(0), Rgba::new(255, 0, 0, 255));
    anim.set(0, FrameIndex::new(10), Rgba::new(0, 0, 255, 255));
    anim.set(3, FrameIndex::new(5), Rgba::new(0, 255, 0, 255));
    let bytes = rmp_serde::to_vec_named(&anim).unwrap();
    let back: PaletteAnimation = rmp_serde::from_slice(&bytes).unwrap();
    assert_eq!(anim, back);
}
```

- [ ] **Step 2: Run and confirm failure**

```bash
cargo nextest run -p pixhaus-core palette::tests::animation
```

Expected: FAIL — `PaletteAnimation` not defined.

- [ ] **Step 3: Add `PaletteAnimation`**

In `core/src/project/palette.rs`:

```rust
use std::collections::BTreeMap;

use super::id::FrameIndex;

/// Per-entry keyframed color changes over the project's frame range.
///
/// Adapted from OpenToonz toonz/sources/include/tpalette.h under
/// BSD-3-Clause. See THIRD_PARTY_NOTICES.md.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PaletteAnimation {
    /// Outer key: palette entry index. Inner key: frame index.
    /// Inner value: color at that frame. Frames between keys resolve
    /// to the closest preceding key (step semantics, no easing).
    pub keyframes: BTreeMap<u32, BTreeMap<FrameIndex, Rgba>>,
}

impl PaletteAnimation {
    /// Inserts a keyframe color at `frame` for the entry at
    /// `entry_index`. Replaces any existing keyframe at the same
    /// position.
    pub fn set(&mut self, entry_index: u32, frame: FrameIndex, color: Rgba) {
        self.keyframes
            .entry(entry_index)
            .or_default()
            .insert(frame, color);
    }

    /// Returns the color the entry at `entry_index` resolves to at
    /// `frame`, falling back to `fallback` when the entry has no
    /// keyframe at or before `frame`.
    #[must_use]
    pub fn resolve(
        &self,
        entry_index: u32,
        frame: FrameIndex,
        fallback: Rgba,
    ) -> Rgba {
        let Some(per_entry) = self.keyframes.get(&entry_index) else {
            return fallback;
        };
        per_entry
            .range(..=frame)
            .next_back()
            .map_or(fallback, |(_, color)| *color)
    }
}
```

- [ ] **Step 4: Confirm pass**

```bash
cargo nextest run -p pixhaus-core palette::tests::animation
```

Expected: all five animation tests pass.

### Task 1.4: Wire `pages` and `animation` into `Palette`

- [ ] **Step 1: Write the failing test**

Append:

```rust
#[test]
fn palette_with_pages_and_animation_round_trips_msgpack() {
    let mut p = Palette::from_colors(
        PaletteId::new(0),
        "test",
        vec![Rgba::new(255, 0, 0, 255), Rgba::new(0, 255, 0, 255)],
    );
    p.pages.push(PalettePage {
        id: PalettePageId::new(1),
        name: "warm".into(),
        entry_indices: vec![0],
    });
    let mut anim = PaletteAnimation::default();
    anim.set(0, FrameIndex::new(0), Rgba::new(255, 0, 0, 255));
    anim.set(0, FrameIndex::new(10), Rgba::new(0, 0, 255, 255));
    p.animation = Some(anim);

    let bytes = rmp_serde::to_vec_named(&p).unwrap();
    let back: Palette = rmp_serde::from_slice(&bytes).unwrap();
    assert_eq!(p, back);
}

#[test]
fn palette_legacy_load_defaults_pages_and_animation_empty() {
    // Serialize a palette without the new fields by emitting raw
    // msgpack matching the pre-S54 shape.
    let legacy = serde_json::json!({
        "id": 0,
        "name": "legacy",
        "colors": [{"color": [255, 0, 0, 255]}],
        "user_data": {}
    });
    let bytes = rmp_serde::to_vec_named(&legacy).unwrap();
    let p: Palette = rmp_serde::from_slice(&bytes).unwrap();
    assert!(p.pages.is_empty());
    assert!(p.animation.is_none());
}
```

- [ ] **Step 2: Run and confirm failure**

```bash
cargo nextest run -p pixhaus-core palette::tests::palette_with_pages_and_animation_round_trips_msgpack
```

Expected: FAIL — `Palette` has no `pages` field.

- [ ] **Step 3: Extend `Palette`**

In `core/src/project/palette.rs`, update the `Palette` struct:

```rust
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Palette {
    pub id: PaletteId,
    pub name: String,
    pub colors: Vec<PaletteEntry>,
    #[serde(skip_serializing_if = "UserData::is_empty", default)]
    pub user_data: UserData,
    /// Named subset views over `colors`. Empty by default.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pages: Vec<PalettePage>,
    /// Optional per-entry keyframed color changes over the frame range.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub animation: Option<PaletteAnimation>,
}
```

Also update `Palette::from_colors` to populate the new fields with
their defaults.

- [ ] **Step 4: Confirm pass**

```bash
cargo nextest run -p pixhaus-core palette
```

### Task 1.5: Bump schema minor

- [ ] **Step 1: Write the failing test**

In `core/src/project/schema.rs` tests:

```rust
#[test]
fn current_version_is_major_4_minor_1() {
    let v = SchemaVersion::current();
    assert_eq!(v.major, 4);
    assert_eq!(v.minor, 1);
}
```

- [ ] **Step 2: Run and confirm failure**

```bash
cargo nextest run -p pixhaus-core schema::tests::current_version_is_major_4_minor_1
```

Expected: FAIL — minor is 0.

- [ ] **Step 3: Bump MINOR**

In `core/src/project/schema.rs`, change:

```rust
pub const MINOR: u16 = 1;
```

Update the rustdoc above it to note: "Bumped to `1` for S54 palette
pages and animation, and S55 contrast blend modes."

- [ ] **Step 4: Add the schema-version warn on future-minor loads**

In the same file's loader-side check (or `io/src/pixhaus/read.rs` if
the check lives there — verify with `grep -n is_compatible_with`),
add a `tracing::warn!` when `other.minor > Self::MINOR`:

```rust
if other.minor > Self::MINOR {
    tracing::warn!(
        file_minor = other.minor,
        reader_minor = Self::MINOR,
        "loading file written by a newer minor schema; \
         unknown fields and enum variants may fail to deserialize",
    );
}
```

- [ ] **Step 5: Confirm pass**

```bash
cargo nextest run -p pixhaus-core schema
```

### Task 1.6: Create the legacy fixture and round-trip test

- [ ] **Step 1: Generate the legacy fixture**

Write a one-shot helper in
`io/tests/helpers/write_legacy_palette_fixture.rs` (gated under
`#[cfg(test)]`) that:
- Constructs a `Palette` (current shape).
- Manually serializes via `rmp_serde::to_vec_named` of a struct that
  omits `pages` and `animation` (use `serde_json::Value` then convert,
  or define a private `LegacyPalette` mirror struct).
- Writes the bytes to `io/tests/fixtures/legacy_v4_0/palette_no_pages.pixhaus`.

This helper is run once with `cargo run --bin generate-fixtures` (a
new bin under `io/`) and the resulting file is committed.

- [ ] **Step 2: Add an integration test that loads the fixture**

Create `io/tests/legacy_v4_load.rs`:

```rust
use pixhaus_core::project::Project;

#[test]
fn legacy_v4_0_palette_loads_with_empty_pages_and_none_animation() {
    let path = std::path::Path::new("tests/fixtures/legacy_v4_0/palette_no_pages.pixhaus");
    let bytes = std::fs::read(path).expect("fixture present");
    let project: Project = pixhaus_io::pixhaus::read::read_from_bytes(&bytes)
        .expect("loads cleanly");

    let palette = &project.sprites[0].palettes[0];
    assert!(palette.pages.is_empty());
    assert!(palette.animation.is_none());
}
```

Adjust paths to match the actual `Project` / `Sprite` accessors.

- [ ] **Step 3: Run and confirm**

```bash
cargo nextest run -p pixhaus-io legacy_v4_0_palette_loads
```

Expected: PASS.

### Task 1.7: Commit S54

- [ ] **Step 1: Commit**

```bash
git add core/src/project/id.rs core/src/project/palette.rs \
        core/src/project/schema.rs core/src/project/mod.rs \
        io/tests/fixtures/ io/tests/legacy_v4_load.rs \
        io/src/pixhaus/read.rs
git commit -m "$(cat <<'EOF'
feat(s54): palette pages and animation [opentoonz]

Add PalettePage (named subset views) and PaletteAnimation
(per-entry keyframed colors with step semantics) to Palette. Both
fields are additive and default to empty; pre-S54 files load
unchanged.

Schema MINOR bumps 0 -> 1. The schema loader now emits a
tracing::warn! when loading a file with a higher minor than the
reader, so subsequent serde failures are legible.

Adapted from OpenToonz toonz/sources/include/tpalette.h under
BSD-3-Clause. See THIRD_PARTY_NOTICES.md.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 2: S55 — eight missing blend modes

**Files:**
- Modify: `core/src/project/blend.rs`
- Modify: `core/src/canvas/blend.rs`
- Modify: `core/src/canvas/composite.rs`
- Modify: `io/src/aseprite/write.rs`
- Create: `core/tests/blend_modes_new.rs` (cross-module integration)

OpenToonz reference: `toonz/sources/stdfx/igs_color_blend.cpp`.

The eight new modes (math expressed per channel, using existing
`mul_un8` helpers where natural):

| Mode | Formula |
|---|---|
| `LinearBurn` | `max(b + s - 255, 0)` |
| `LinearDodge` | `min(b + s, 255)` (same as `Addition`; kept for UI parity) |
| `VividLight` | `s < 128 ? color_burn(b, 2s) : color_dodge(b, 2s - 255)` |
| `LinearLight` | `s < 128 ? linear_burn(b, 2s) : linear_dodge(b, 2s - 255)` |
| `PinLight` | `s < 128 ? darken(b, 2s) : lighten(b, 2s - 255)` |
| `HardMix` | `vivid_light(b, s) < 128 ? 0 : 255` |
| `DarkerColor` | RGBA-level: pick the channel-triple with the lower luminance |
| `LighterColor` | RGBA-level: pick the channel-triple with the higher luminance |

### Task 2.1: Add the eight enum variants

- [ ] **Step 1: Extend the existing `all_modes_round_trip` test**

In `core/src/project/blend.rs` tests, replace the existing modes
array with all 27 variants (19 current + 8 new):

```rust
let modes = [
    BlendMode::Normal, BlendMode::Darken, BlendMode::Multiply,
    BlendMode::ColorBurn, BlendMode::Lighten, BlendMode::Screen,
    BlendMode::ColorDodge, BlendMode::Addition, BlendMode::Overlay,
    BlendMode::SoftLight, BlendMode::HardLight, BlendMode::Difference,
    BlendMode::Exclusion, BlendMode::Subtract, BlendMode::Divide,
    BlendMode::Hue, BlendMode::Saturation, BlendMode::Color,
    BlendMode::Luminosity,
    BlendMode::LinearBurn, BlendMode::DarkerColor,
    BlendMode::LinearDodge, BlendMode::LighterColor,
    BlendMode::VividLight, BlendMode::LinearLight,
    BlendMode::PinLight, BlendMode::HardMix,
];
```

- [ ] **Step 2: Run and confirm failure**

```bash
cargo nextest run -p pixhaus-core blend::tests::all_modes_round_trip
```

Expected: FAIL — unknown variants.

- [ ] **Step 3: Add the eight variants to the enum**

In `core/src/project/blend.rs`:

```rust
    // ... existing variants ...
    /// `max(b + s - 255, 0)` per channel.
    LinearBurn,
    /// Pick the color whose RGB sum is lower.
    DarkerColor,
    /// `min(b + s, 255)` per channel. Equivalent to Addition; kept
    /// as a distinct variant for UI parity with Photoshop.
    LinearDodge,
    /// Pick the color whose RGB sum is higher.
    LighterColor,
    /// `s < 128 ? color_burn(b, 2s) : color_dodge(b, 2s - 255)`.
    VividLight,
    /// `s < 128 ? linear_burn(b, 2s) : linear_dodge(b, 2s - 255)`.
    LinearLight,
    /// `s < 128 ? darken(b, 2s) : lighten(b, 2s - 255)`.
    PinLight,
    /// `vivid_light(b, s) < 128 ? 0 : 255` per channel.
    HardMix,
```

- [ ] **Step 4: Confirm pass**

```bash
cargo nextest run -p pixhaus-core blend
```

### Task 2.2: Add `channel_linear_burn` + test

- [ ] **Step 1: Write the failing parameterized test**

Append to `core/src/canvas/blend.rs` tests:

```rust
#[rstest::rstest]
#[case(0, 0, 0)]
#[case(255, 255, 255)]
#[case(100, 100, 0)]
#[case(200, 200, 145)]
#[case(255, 0, 0)]
#[case(0, 255, 0)]
fn channel_linear_burn_table(
    #[case] b: u8,
    #[case] s: u8,
    #[case] expected: u8,
) {
    assert_eq!(channel_linear_burn(b, s), expected);
}
```

- [ ] **Step 2: Run and confirm failure**

```bash
cargo nextest run -p pixhaus-core channel_linear_burn_table
```

Expected: FAIL — unknown function.

- [ ] **Step 3: Implement**

In `core/src/canvas/blend.rs` (after the existing channel functions):

```rust
/// `max(b + s - 255, 0)` per channel.
///
/// Adapted from OpenToonz toonz/sources/stdfx/igs_color_blend.cpp
/// under BSD-3-Clause.
#[inline]
#[must_use]
pub const fn channel_linear_burn(b: u8, s: u8) -> u8 {
    let sum = b as u16 + s as u16;
    if sum <= 255 { 0 } else {
        (sum - 255) as u8
    }
}
```

- [ ] **Step 4: Confirm pass**

```bash
cargo nextest run -p pixhaus-core channel_linear_burn
```

### Task 2.3: Add `channel_linear_dodge` (alias for addition)

- [ ] **Step 1: Failing test**

```rust
#[rstest::rstest]
#[case(0, 0, 0)]
#[case(100, 100, 200)]
#[case(200, 200, 255)]
#[case(255, 1, 255)]
fn channel_linear_dodge_table(
    #[case] b: u8, #[case] s: u8, #[case] expected: u8,
) {
    assert_eq!(channel_linear_dodge(b, s), expected);
}
```

- [ ] **Step 2: Run, confirm fail.**
- [ ] **Step 3: Implement**

```rust
/// `min(b + s, 255)` per channel.
///
/// Mathematically equivalent to channel_addition; named separately
/// so the UI can expose Photoshop's "Linear Dodge" terminology.
///
/// Adapted from OpenToonz toonz/sources/stdfx/igs_color_blend.cpp
/// under BSD-3-Clause.
#[inline]
#[must_use]
pub const fn channel_linear_dodge(b: u8, s: u8) -> u8 {
    let sum = b as u16 + s as u16;
    if sum > 255 { 255 } else { sum as u8 }
}
```

- [ ] **Step 4: Confirm pass.**

### Task 2.4: Add `channel_vivid_light`

- [ ] **Step 1: Failing test**

```rust
#[rstest::rstest]
#[case(0, 0, 0)]      // s < 128, color_burn(0, 0) = 0
#[case(255, 0, 0)]    // s = 0 burns to 0
#[case(255, 255, 255)]// s = 255 dodges to 255
#[case(128, 128, 128)]// midpoint
#[case(0, 255, 0)]    // burn against white = 0
fn channel_vivid_light_table(
    #[case] b: u8, #[case] s: u8, #[case] expected: u8,
) {
    assert_eq!(channel_vivid_light(b, s), expected);
}
```

- [ ] **Step 2: Run, confirm fail.**
- [ ] **Step 3: Implement**

```rust
/// `s < 128 ? color_burn(b, 2s) : color_dodge(b, 2s - 255)`.
///
/// Adapted from OpenToonz toonz/sources/stdfx/igs_color_blend.cpp
/// under BSD-3-Clause.
#[inline]
#[must_use]
pub fn channel_vivid_light(b: u8, s: u8) -> u8 {
    if s < 128 {
        let doubled = s.saturating_mul(2);
        channel_color_burn(b, doubled)
    } else {
        let doubled = (s as u16 * 2).saturating_sub(255) as u8;
        channel_color_dodge(b, doubled)
    }
}
```

(`channel_color_burn` and `channel_color_dodge` already exist —
verify via `grep -n "channel_color_burn\|channel_color_dodge"
core/src/canvas/blend.rs` before relying on them; if they don't,
add them following the existing per-channel pattern from
`blend_funcs.cpp`.)

- [ ] **Step 4: Confirm pass.**

### Task 2.5: Add `channel_linear_light`

- [ ] **Step 1: Failing test**

```rust
#[rstest::rstest]
#[case(0, 0, 0)]
#[case(255, 0, 0)]
#[case(255, 255, 255)]
#[case(128, 128, 128)]
fn channel_linear_light_table(
    #[case] b: u8, #[case] s: u8, #[case] expected: u8,
) {
    assert_eq!(channel_linear_light(b, s), expected);
}
```

- [ ] **Step 2: Run, confirm fail.**
- [ ] **Step 3: Implement**

```rust
#[inline]
#[must_use]
pub fn channel_linear_light(b: u8, s: u8) -> u8 {
    if s < 128 {
        let doubled = s.saturating_mul(2);
        channel_linear_burn(b, doubled)
    } else {
        let doubled = (s as u16 * 2).saturating_sub(255) as u8;
        channel_linear_dodge(b, doubled)
    }
}
```

- [ ] **Step 4: Confirm pass.**

### Task 2.6: Add `channel_pin_light`

- [ ] **Step 1: Failing test**

```rust
#[rstest::rstest]
#[case(0, 0, 0)]
#[case(255, 0, 0)]      // darken(255, 0) = 0
#[case(0, 255, 0)]      // lighten(0, 255) = 255 ... wait
#[case(255, 255, 255)]
fn channel_pin_light_table(
    #[case] b: u8, #[case] s: u8, #[case] expected: u8,
) {
    assert_eq!(channel_pin_light(b, s), expected);
}
```

(Verify case-by-case against
`stdfx/igs_color_blend.cpp::pin_light`; values above are
illustrative — the engineer must compute expected from the formula.)

- [ ] **Step 2: Run, confirm fail.**
- [ ] **Step 3: Implement**

```rust
#[inline]
#[must_use]
pub fn channel_pin_light(b: u8, s: u8) -> u8 {
    if s < 128 {
        let doubled = s.saturating_mul(2);
        channel_darken(b, doubled)
    } else {
        let doubled = (s as u16 * 2).saturating_sub(255) as u8;
        channel_lighten(b, doubled)
    }
}
```

- [ ] **Step 4: Confirm pass.**

### Task 2.7: Add `channel_hard_mix`

- [ ] **Step 1: Failing test**

```rust
#[rstest::rstest]
#[case(0, 0, 0)]
#[case(255, 255, 255)]
#[case(100, 100, 0)]   // vivid_light < 128 -> 0
#[case(200, 200, 255)] // vivid_light >= 128 -> 255
fn channel_hard_mix_table(
    #[case] b: u8, #[case] s: u8, #[case] expected: u8,
) {
    assert_eq!(channel_hard_mix(b, s), expected);
}
```

- [ ] **Step 2: Run, confirm fail.**
- [ ] **Step 3: Implement**

```rust
#[inline]
#[must_use]
pub fn channel_hard_mix(b: u8, s: u8) -> u8 {
    if channel_vivid_light(b, s) < 128 { 0 } else { 255 }
}
```

- [ ] **Step 4: Confirm pass.**

### Task 2.8: Add `rgba_darker_color` and `rgba_lighter_color`

- [ ] **Step 1: Failing tests**

```rust
#[test]
fn rgba_darker_color_picks_lower_luminance() {
    let dark = Rgba::new(50, 50, 50, 255);
    let light = Rgba::new(200, 200, 200, 255);
    assert_eq!(rgba_darker_color(dark, light), dark);
    assert_eq!(rgba_darker_color(light, dark), dark);
}

#[test]
fn rgba_darker_color_breaks_tie_by_picking_backdrop() {
    let a = Rgba::new(100, 100, 100, 255);
    let b = Rgba::new(100, 100, 100, 255);
    assert_eq!(rgba_darker_color(a, b), a);
}

#[test]
fn rgba_lighter_color_picks_higher_luminance() {
    let dark = Rgba::new(50, 50, 50, 255);
    let light = Rgba::new(200, 200, 200, 255);
    assert_eq!(rgba_lighter_color(dark, light), light);
    assert_eq!(rgba_lighter_color(light, dark), light);
}
```

- [ ] **Step 2: Run, confirm fail.**
- [ ] **Step 3: Implement**

```rust
/// Picks the color whose perceived luminance is lower.
///
/// Uses Rec. 709 luma: `Y = 0.2126*R + 0.7152*G + 0.0722*B`.
///
/// Adapted from OpenToonz toonz/sources/stdfx/igs_color_blend.cpp
/// under BSD-3-Clause.
#[inline]
#[must_use]
pub fn rgba_darker_color(b: Rgba, s: Rgba) -> Rgba {
    if luma_rec709(s) < luma_rec709(b) { s } else { b }
}

#[inline]
#[must_use]
pub fn rgba_lighter_color(b: Rgba, s: Rgba) -> Rgba {
    if luma_rec709(s) > luma_rec709(b) { s } else { b }
}

#[inline]
fn luma_rec709(c: Rgba) -> u32 {
    // Integer math: 0.2126*256 = 54, 0.7152*256 = 183, 0.0722*256 = 19.
    // Sum stays in u32 to avoid overflow.
    54u32 * c.r as u32 + 183u32 * c.g as u32 + 19u32 * c.b as u32
}
```

- [ ] **Step 4: Confirm pass.**

### Task 2.9: Wire new modes into composite dispatcher

- [ ] **Step 1: Failing tests**

In `core/src/canvas/composite.rs` tests (or a new
`core/tests/blend_modes_new.rs`):

```rust
#[rstest::rstest]
#[case(BlendMode::LinearBurn)]
#[case(BlendMode::DarkerColor)]
#[case(BlendMode::LinearDodge)]
#[case(BlendMode::LighterColor)]
#[case(BlendMode::VividLight)]
#[case(BlendMode::LinearLight)]
#[case(BlendMode::PinLight)]
#[case(BlendMode::HardMix)]
fn dispatcher_routes_new_blend_modes(#[case] mode: BlendMode) {
    let src = Rgba::new(100, 150, 200, 255);
    let dst = Rgba::new(50, 75, 100, 255);
    // Dispatch via the public composite entry point.
    let out = composite::blend(mode, src, dst, 255);
    // The new modes must produce *some* output without panicking
    // and the result must be a valid Rgba (alpha == 255 here).
    assert_eq!(out.a, 255);
}
```

- [ ] **Step 2: Run, confirm fail.**
- [ ] **Step 3: Extend the dispatcher**

In `core/src/canvas/composite.rs`, locate the `match mode { ... }`
in the main `blend()` function and add eight arms:

```rust
BlendMode::LinearBurn => apply_channel(src, dst, opacity, channel_linear_burn),
BlendMode::DarkerColor => apply_rgba(src, dst, opacity, rgba_darker_color),
BlendMode::LinearDodge => apply_channel(src, dst, opacity, channel_linear_dodge),
BlendMode::LighterColor => apply_rgba(src, dst, opacity, rgba_lighter_color),
BlendMode::VividLight => apply_channel(src, dst, opacity, channel_vivid_light),
BlendMode::LinearLight => apply_channel(src, dst, opacity, channel_linear_light),
BlendMode::PinLight => apply_channel(src, dst, opacity, channel_pin_light),
BlendMode::HardMix => apply_channel(src, dst, opacity, channel_hard_mix),
```

Match the actual helper names found via
`grep -n "fn apply_channel\|fn apply_rgba" core/src/canvas/composite.rs`.
If those exact helpers don't exist, extend the dispatcher by
inlining the apply step using the existing pattern.

- [ ] **Step 4: Confirm pass.**

### Task 2.10: Aseprite export downgrade

- [ ] **Step 1: Failing test**

In `io/tests/aseprite_blend_downgrade.rs` (new):

```rust
use pixhaus_core::project::{BlendMode, Layer};

#[test]
fn aseprite_export_downgrades_new_modes_to_normal() {
    let mut project = pixhaus_io::test_helpers::sample_project_one_layer();
    project.sprites[0].layers[0].blend_mode = BlendMode::LinearLight;

    let bytes = pixhaus_io::aseprite::write::write_to_bytes(&project)
        .expect("export");
    let round = pixhaus_io::aseprite::read::read_from_bytes(&bytes)
        .expect("re-read");

    assert_eq!(round.sprites[0].layers[0].blend_mode, BlendMode::Normal);
}
```

(Names may need adjustment based on the actual public API; verify
with `grep -rn "pub fn write_to_bytes\|pub fn read_from_bytes"
io/src/aseprite/`.)

- [ ] **Step 2: Run, confirm fail.**
- [ ] **Step 3: Implement downgrade in `io/src/aseprite/write.rs`**

Find the function that maps `BlendMode` to Aseprite's blend byte.
Add the eight new modes mapping to Aseprite's `Normal` byte (`0`),
with a `tracing::warn!` per occurrence:

```rust
fn aseprite_blend_byte(mode: BlendMode) -> u16 {
    match mode {
        BlendMode::Normal => 0,
        // ... existing mappings ...
        BlendMode::LinearBurn
        | BlendMode::DarkerColor
        | BlendMode::LinearDodge
        | BlendMode::LighterColor
        | BlendMode::VividLight
        | BlendMode::LinearLight
        | BlendMode::PinLight
        | BlendMode::HardMix => {
            tracing::warn!(
                blend_mode = ?mode,
                "Aseprite has no equivalent for this blend mode; \
                 downgrading to Normal on export",
            );
            0
        }
    }
}
```

- [ ] **Step 4: Add a docs entry**

Append to `docs/migration/aseprite.md` (create if absent):

```markdown
### Blend mode loss on Aseprite export

Pixhaus blend modes `LinearBurn`, `DarkerColor`, `LinearDodge`,
`LighterColor`, `VividLight`, `LinearLight`, `PinLight`, and
`HardMix` have no equivalent in Aseprite's file format. Exporting a
project containing these modes downgrades them to `Normal` and
emits a `tracing::warn!` per affected layer. To preserve them,
export to `.pixhaus` instead.
```

- [ ] **Step 5: Confirm pass.**

```bash
cargo nextest run -p pixhaus-io aseprite_blend_downgrade
```

### Task 2.11: Commit S55

- [ ] **Step 1: Commit**

```bash
git add core/src/project/blend.rs core/src/canvas/blend.rs \
        core/src/canvas/composite.rs \
        io/src/aseprite/write.rs io/tests/aseprite_blend_downgrade.rs \
        docs/migration/aseprite.md
git commit -m "$(cat <<'EOF'
feat(s55): linear/contrast blend modes [opentoonz]

Add LinearBurn, DarkerColor, LinearDodge, LighterColor, VividLight,
LinearLight, PinLight, HardMix to BlendMode + canvas blend math +
composite dispatcher. Aseprite export downgrades to Normal with a
tracing::warn! per affected layer.

Eight new modes are part of the schema MINOR=1 contract bumped in
S54.

Adapted from OpenToonz toonz/sources/stdfx/igs_color_blend.cpp
under BSD-3-Clause. See THIRD_PARTY_NOTICES.md.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 3: S56 — morphological anti-aliasing

**Files:**
- Create: `core/src/transforms/antialias.rs`
- Modify: `core/src/transforms/mod.rs`
- Create: `core/tests/snapshots/mlaa_staircase_baseline.png`
- Create: `core/tests/antialias_visual.rs`

OpenToonz reference: `toonz/sources/common/trop/tantialias.cpp`.

### Task 3.1: Add `MlaaConfig`

- [ ] **Step 1: Create module skeleton with failing test**

Create `core/src/transforms/antialias.rs`:

```rust
//! Morphological anti-aliasing (MLAA).
//!
//! Adapted from OpenToonz toonz/sources/common/trop/tantialias.cpp
//! under BSD-3-Clause. See THIRD_PARTY_NOTICES.md.
//!
//! Algorithm: Alexander Reshetov, "Morphological Antialiasing"
//! (Intel Labs, 2009).

use crate::canvas::PixelBuffer;
use crate::project::Rgba;
use super::error::Result;

/// Configuration for [`morphological_antialias`].
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct MlaaConfig {
    pub threshold: u8,
    pub softness: u8,
}

impl Default for MlaaConfig {
    fn default() -> Self { Self { threshold: 16, softness: 128 } }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_documented_values() {
        let c = MlaaConfig::default();
        assert_eq!(c.threshold, 16);
        assert_eq!(c.softness, 128);
    }
}
```

Register the module in `core/src/transforms/mod.rs`:

```rust
pub mod antialias;
```

- [ ] **Step 2: Run and confirm pass**

```bash
cargo nextest run -p pixhaus-core antialias::tests::default_config_has_documented_values
```

### Task 3.2: Add `morphological_antialias` — idempotent-on-flat case

- [ ] **Step 1: Failing test**

```rust
#[test]
fn mlaa_flat_buffer_unchanged() {
    let buf = PixelBuffer::filled(8, 8, Rgba::new(128, 128, 128, 255));
    let out = morphological_antialias(&buf, &MlaaConfig::default()).unwrap();
    for y in 0..8 {
        for x in 0..8 {
            assert_eq!(out.pixel(x, y), buf.pixel(x, y));
        }
    }
}
```

- [ ] **Step 2: Run, confirm fail.**
- [ ] **Step 3: Stub implementation**

```rust
pub fn morphological_antialias(
    src: &PixelBuffer,
    config: &MlaaConfig,
) -> Result<PixelBuffer> {
    let _ = config;
    // Stub: return a clone. Subsequent tasks replace this with the
    // two-pass implementation.
    Ok(src.clone())
}
```

- [ ] **Step 4: Confirm pass.**

### Task 3.3: Detect separation lines (row pass — classifier)

- [ ] **Step 1: Failing test**

```rust
#[test]
fn mlaa_classifies_horizontal_edge() {
    // 8x4 buffer with top half black, bottom half white -> exactly
    // one classified edge between row 1 and row 2.
    let mut buf = PixelBuffer::new(8, 4);
    for y in 0..2 {
        for x in 0..8 {
            buf.set_pixel(x, y, Rgba::new(0, 0, 0, 255));
        }
    }
    for y in 2..4 {
        for x in 0..8 {
            buf.set_pixel(x, y, Rgba::new(255, 255, 255, 255));
        }
    }
    let edges = classify_row_edges(&buf, &MlaaConfig::default());
    assert_eq!(edges.iter().filter(|e| e.present).count(), 8);
}
```

- [ ] **Step 2: Run, confirm fail.**
- [ ] **Step 3: Implement classifier**

```rust
/// One classified edge between row y and row y+1, at column x.
#[derive(Copy, Clone, Debug, Default)]
pub(crate) struct RowEdge {
    pub present: bool,
}

pub(crate) fn classify_row_edges(
    src: &PixelBuffer,
    config: &MlaaConfig,
) -> Vec<RowEdge> {
    let w = src.width() as usize;
    let h = src.height() as usize;
    if h < 2 { return Vec::new(); }
    let mut out = vec![RowEdge::default(); w * (h - 1)];

    for y in 0..h - 1 {
        for x in 0..w {
            let Some(a) = src.pixel(x as u32, y as u32) else { continue };
            let Some(b) = src.pixel(x as u32, (y + 1) as u32) else { continue };
            let dr = (a[0] as i32 - b[0] as i32).abs() as u8;
            let dg = (a[1] as i32 - b[1] as i32).abs() as u8;
            let db = (a[2] as i32 - b[2] as i32).abs() as u8;
            let max = dr.max(dg).max(db);
            out[y * w + x] = RowEdge { present: max > config.threshold };
        }
    }
    out
}
```

(Adjust `src.pixel()` return shape based on the actual signature.)

- [ ] **Step 4: Confirm pass.**

### Task 3.4: Trace edge-run extents (row pass)

- [ ] **Step 1: Failing test**

```rust
#[test]
fn mlaa_traces_run_extents_at_boundary() {
    // Two rows of black-on-white with a 4-px horizontal run.
    // (test fixture set up similar to 3.3 but with a stair-step
    //  variation; the run extents should be [start, end] = [0, 3].)
    let runs = trace_row_runs(/* fixture */);
    assert!(runs.iter().any(|r| r.length() == 4));
}
```

- [ ] **Step 2: Run, confirm fail.**
- [ ] **Step 3: Implement `trace_row_runs`**

This is the core MLAA bookkeeping — scan each row-edge column,
group consecutive `present` flags into runs, and detect the L/U/Z
shape transitions at the run boundaries. Reference:
`tantialias.cpp::processLine`.

```rust
#[derive(Copy, Clone, Debug)]
pub(crate) struct EdgeRun {
    pub start_x: u32,
    pub end_x: u32,
    pub row_y: u32,
    pub shape: RunShape,
}

#[derive(Copy, Clone, Debug)]
pub(crate) enum RunShape {
    /// Run ends in an upward step on both sides.
    ConcaveUp,
    /// Run ends in a downward step on both sides.
    ConcaveDown,
    /// Step up at start, step down at end (Z shape).
    ZUpDown,
    /// Step down at start, step up at end (S shape).
    SDownUp,
    /// Run terminates against the buffer edge (single-sided).
    OpenEnded,
}

pub(crate) fn trace_row_runs(
    edges: &[RowEdge],
    width: usize,
    height: usize,
) -> Vec<EdgeRun> { /* ... */ }
```

Skeleton — refer to `tantialias.cpp` for the exact shape detection
logic.

- [ ] **Step 4: Confirm pass.**

### Task 3.5: Apply per-run coverage (row pass)

- [ ] **Step 1: Failing test**

```rust
#[test]
fn mlaa_softens_45_staircase() {
    // 16x16 black-on-white staircase. Output rendered against an
    // insta YAML snapshot of the modified pixel rows.
    let buf = staircase_buffer(16);
    let out = morphological_antialias(&buf, &MlaaConfig::default()).unwrap();
    let summary = summarize_row_changes(&buf, &out);
    insta::assert_yaml_snapshot!(summary);
}
```

- [ ] **Step 2: Run, confirm fail / commit baseline snapshot via
  `cargo insta accept` after manual review.**

- [ ] **Step 3: Implement run-coverage application**

The per-run code computes a triangular-area coverage along each
side of the run and blends the run-side colors into the affected
pixels. This is the part of MLAA that does the softening. Refer to
`tantialias.cpp::processLine` lines 200–340 for the geometry.

```rust
fn apply_row_coverage(
    src: &PixelBuffer,
    dst: &mut PixelBuffer,
    runs: &[EdgeRun],
    softness: u8,
) { /* ... */ }
```

- [ ] **Step 4: Confirm pass + review snapshot before accept.**

### Task 3.6: Column pass

- [ ] **Step 1: Failing test**

```rust
#[test]
fn mlaa_softens_vertical_staircase_with_column_pass() {
    // Vertical staircase fixture; rendered output compared via
    // image-compare against tests/snapshots/mlaa_staircase_baseline.png.
    let buf = vertical_staircase_buffer(16);
    let out = morphological_antialias(&buf, &MlaaConfig::default()).unwrap();
    let baseline = image::open("core/tests/snapshots/mlaa_vstaircase_baseline.png").unwrap();
    let score = image_compare::rgba_hybrid_compare(&out.into(), &baseline.into())
        .unwrap().score;
    assert!(score >= 0.98, "score={}", score);
}
```

- [ ] **Step 2: Run, confirm fail / commit baseline image.**

- [ ] **Step 3: Implement the column pass**

```rust
fn classify_col_edges(src: &PixelBuffer, config: &MlaaConfig) -> Vec<ColEdge> { /* ... */ }
fn trace_col_runs(...) -> Vec<EdgeRun> { /* ... */ }
fn apply_col_coverage(...) { /* ... */ }
```

Update `morphological_antialias` to run the row pass into a tmp
buffer, then the column pass into the output:

```rust
pub fn morphological_antialias(
    src: &PixelBuffer,
    config: &MlaaConfig,
) -> Result<PixelBuffer> {
    let row_edges = classify_row_edges(src, config);
    let row_runs = trace_row_runs(&row_edges, src.width() as usize, src.height() as usize);
    let mut tmp = src.clone();
    apply_row_coverage(src, &mut tmp, &row_runs, config.softness);

    let col_edges = classify_col_edges(&tmp, config);
    let col_runs = trace_col_runs(&col_edges, tmp.width() as usize, tmp.height() as usize);
    let mut dst = tmp.clone();
    apply_col_coverage(&tmp, &mut dst, &col_runs, config.softness);
    Ok(dst)
}
```

- [ ] **Step 4: Confirm pass.**

### Task 3.7: Visual regression integration test

- [ ] **Step 1: Failing test in `core/tests/antialias_visual.rs`**

```rust
//! Visual regression for morphological_antialias.

use image_compare::Algorithm;
use pixhaus_core::canvas::PixelBuffer;
use pixhaus_core::project::Rgba;
use pixhaus_core::transforms::antialias::{morphological_antialias, MlaaConfig};

#[test]
fn mlaa_diagonal_square_softens_edges_vs_bilinear_baseline() {
    let src = load_fixture("core/tests/fixtures/diag_square_input.png");
    let out = morphological_antialias(&src, &MlaaConfig::default()).unwrap();
    let baseline = load_fixture("core/tests/snapshots/diag_square_mlaa_baseline.png");

    let result = image_compare::rgba_hybrid_compare(
        &out.into(),
        &baseline.into(),
    ).unwrap();
    assert!(result.score >= 0.999, "score={}", result.score);
}

fn load_fixture(path: &str) -> PixelBuffer { /* helper that reads PNG -> PixelBuffer */ }
```

- [ ] **Step 2: Generate baselines** — render the inputs, manually
  verify the output, commit the baseline PNGs.

- [ ] **Step 3: Confirm pass.**

### Task 3.8: Parallelize the per-line work with rayon

- [ ] **Step 1: Add a benchmark sanity check (not gating)**

```rust
#[test]
fn mlaa_completes_512x512_under_500ms() {
    use std::time::Instant;
    let buf = staircase_buffer(512);
    let start = Instant::now();
    let _ = morphological_antialias(&buf, &MlaaConfig::default()).unwrap();
    let elapsed = start.elapsed();
    assert!(elapsed.as_millis() < 500, "took {} ms", elapsed.as_millis());
}
```

- [ ] **Step 2: Run** — if it passes already, skip the rayon change.

- [ ] **Step 3: If too slow, parallelize**

Replace the row-pass `for y in 0..h - 1` loop with
`rayon::iter::ParallelIterator`:

```rust
use rayon::iter::{IntoParallelIterator, ParallelIterator};

let chunks: Vec<_> = (0..h - 1).into_par_iter()
    .map(|y| { /* classify row y */ })
    .collect();
```

- [ ] **Step 4: Confirm tests still pass.**

### Task 3.9: Commit S56

```bash
git add core/src/transforms/antialias.rs core/src/transforms/mod.rs \
        core/tests/antialias_visual.rs core/tests/fixtures/ \
        core/tests/snapshots/
git commit -m "$(cat <<'EOF'
feat(s56): morphological anti-aliasing transform [opentoonz]

Two-pass MLAA driver (rows then columns) with edge classifier,
run-shape detector, and triangular-area coverage application.
Public entry point is morphological_antialias(src, config).

Per-line work parallelized via rayon, matching the scale.rs pattern.
Visual regression tests against committed PNG baselines using
image_compare >= 0.999.

Adapted from OpenToonz toonz/sources/common/trop/tantialias.cpp
under BSD-3-Clause; algorithm credit: Alexander Reshetov,
"Morphological Antialiasing", Intel Labs, 2009. See
THIRD_PARTY_NOTICES.md.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 4: S57 — gap-closing magic wand

**Files:**
- Create: `core/build.rs`
- Create: `core/src/selection/autoclose.rs`
- Create: `core/src/selection/skeleton_lut.rs` (generated, gitignored or committed depending on tooling preference; we commit the generator output for reviewability)
- Modify: `core/src/selection/mod.rs`
- Modify: `core/src/selection/algorithms.rs`
- Create: `core/tests/autoclose_integration.rs`

OpenToonz reference: `toonz/sources/common/trop/tautoclose.cpp` and
`skeletonlut.h`.

### Task 4.1: Set up `build.rs` skeleton-LUT generator

- [ ] **Step 1: Create `core/build.rs`**

```rust
// core/build.rs
//
// Generates src/selection/skeleton_lut.rs from the 8-neighbour
// classification rules. Run by cargo before compiling the crate.
//
// Adapted from OpenToonz toonz/sources/common/trop/tautoclose.cpp
// and toonz/sources/common/trop/skeletonlut.h under BSD-3-Clause.

use std::env;
use std::fs;
use std::io::Write;
use std::path::Path;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum Classification {
    Isolated, Endpoint, Border, Branch, Interior,
}

fn classify(code: u8) -> Classification {
    let n = code.count_ones();
    match n {
        0 => Classification::Isolated,
        1 => Classification::Endpoint,
        2 => {
            // Two set bits: endpoint if non-adjacent, border if
            // adjacent in the 8-neighbour ring.
            if adjacent_pair(code) { Classification::Border }
            else { Classification::Endpoint }
        }
        3 | 4 => {
            // Three or four set bits classify as branch if the
            // pattern has 3+ disjoint runs, border otherwise.
            if disjoint_runs(code) >= 3 { Classification::Branch }
            else { Classification::Border }
        }
        _ => Classification::Interior,
    }
}

fn adjacent_pair(code: u8) -> bool {
    for i in 0..8 {
        let a = (code >> i) & 1;
        let b = (code >> ((i + 1) % 8)) & 1;
        if a == 1 && b == 1 { return true; }
    }
    false
}

fn disjoint_runs(code: u8) -> u32 {
    // Count transitions from 1 to 0 going around the ring.
    let mut runs = 0;
    for i in 0..8 {
        let cur = (code >> i) & 1;
        let nxt = (code >> ((i + 1) % 8)) & 1;
        if cur == 1 && nxt == 0 { runs += 1; }
    }
    runs
}

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest = Path::new(&out_dir).join("skeleton_lut_table.rs");
    let mut f = fs::File::create(dest).unwrap();

    writeln!(f, "// AUTOGENERATED by core/build.rs — DO NOT EDIT").unwrap();
    writeln!(f, "pub(crate) static SKELETON_LUT: [Classification; 256] = [").unwrap();
    for code in 0u32..256 {
        let c = classify(code as u8);
        writeln!(f, "    Classification::{:?},", c).unwrap();
    }
    writeln!(f, "];").unwrap();

    println!("cargo:rerun-if-changed=build.rs");
}
```

- [ ] **Step 2: Add the include shim file**

`core/src/selection/skeleton_lut.rs`:

```rust
//! Generated 256-entry classification LUT for the 8-neighbour
//! code (see core/build.rs).
//!
//! Adapted from OpenToonz toonz/sources/common/trop/skeletonlut.h
//! under BSD-3-Clause. See THIRD_PARTY_NOTICES.md.

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) enum Classification {
    Isolated, Endpoint, Border, Branch, Interior,
}

include!(concat!(env!("OUT_DIR"), "/skeleton_lut_table.rs"));
```

- [ ] **Step 3: Add a sanity test**

In `core/src/selection/skeleton_lut.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn isolated_classified_correctly() {
        // code 0 = no neighbours -> isolated
        assert_eq!(SKELETON_LUT[0], Classification::Isolated);
    }

    #[test]
    fn single_neighbour_is_endpoint() {
        // code 1 = single bit set -> endpoint
        assert_eq!(SKELETON_LUT[1], Classification::Endpoint);
        assert_eq!(SKELETON_LUT[2], Classification::Endpoint);
        assert_eq!(SKELETON_LUT[128], Classification::Endpoint);
    }

    #[test]
    fn adjacent_pair_is_border() {
        // codes 0b0000_0011 and 0b1000_0001 are adjacent pairs.
        assert_eq!(SKELETON_LUT[0b0000_0011], Classification::Border);
        assert_eq!(SKELETON_LUT[0b1000_0001], Classification::Border);
    }

    #[test]
    fn opposing_pair_is_endpoint() {
        // codes 0b0001_0001 and 0b0100_0010 are non-adjacent.
        assert_eq!(SKELETON_LUT[0b0001_0001], Classification::Endpoint);
    }

    #[test]
    fn fully_surrounded_is_interior() {
        assert_eq!(SKELETON_LUT[0b1111_1111], Classification::Interior);
    }
}
```

- [ ] **Step 4: Run and confirm tests pass**

```bash
cargo nextest run -p pixhaus-core skeleton_lut
```

### Task 4.2: Add `GapCloseConfig` and `close_gaps` stub

- [ ] **Step 1: Failing test**

Create `core/src/selection/autoclose.rs`:

```rust
//! Gap-closing pre-pass for the magic wand.
//!
//! Adapted from OpenToonz toonz/sources/common/trop/tautoclose.cpp
//! under BSD-3-Clause. See THIRD_PARTY_NOTICES.md.

use crate::canvas::PixelBuffer;
use crate::project::IVec2;
use super::error::Result;
use super::mask::SelectionMask;
use super::skeleton_lut::{SKELETON_LUT, Classification};

#[derive(Copy, Clone, Debug)]
pub struct GapCloseConfig {
    pub closing_distance: u32,
    pub closing_angle_rad: f32,
    pub ink_threshold: u8,
}

impl Default for GapCloseConfig {
    fn default() -> Self {
        Self {
            closing_distance: 10,
            closing_angle_rad: std::f32::consts::FRAC_PI_2,
            ink_threshold: 128,
        }
    }
}

pub fn close_gaps(buffer: &PixelBuffer, config: &GapCloseConfig) -> Result<SelectionMask> {
    let _ = (buffer, config);
    todo!("subsequent tasks")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn config_defaults_match_doc() {
        let c = GapCloseConfig::default();
        assert_eq!(c.closing_distance, 10);
        assert!((c.closing_angle_rad - std::f32::consts::FRAC_PI_2).abs() < 1e-6);
        assert_eq!(c.ink_threshold, 128);
    }
}
```

Register in `core/src/selection/mod.rs`:

```rust
mod skeleton_lut;
pub mod autoclose;
```

- [ ] **Step 2: Run and confirm pass.**

### Task 4.3: Extract ink mask from buffer

- [ ] **Step 1: Failing test**

```rust
#[test]
fn ink_mask_marks_pixels_below_threshold() {
    let mut buf = PixelBuffer::new(4, 4);
    for x in 0..4 {
        buf.set_pixel(x, 0, Rgba::new(0, 0, 0, 255));     // ink
        buf.set_pixel(x, 1, Rgba::new(255, 255, 255, 255)); // bg
    }
    let mask = extract_ink_mask(&buf, 128);
    assert!(mask[0]);
    assert!(mask[1]);
    assert!(mask[2]);
    assert!(mask[3]);
    assert!(!mask[4]); // (0, 1)
}
```

- [ ] **Step 2: Run, confirm fail.**
- [ ] **Step 3: Implement**

```rust
pub(crate) fn extract_ink_mask(buf: &PixelBuffer, threshold: u8) -> Vec<bool> {
    let mut out = Vec::with_capacity(buf.width() as usize * buf.height() as usize);
    for y in 0..buf.height() {
        for x in 0..buf.width() {
            let p = buf.pixel(x, y).expect("in bounds");
            // Use luma: a pixel is "ink" if its luma is below threshold.
            let luma = (54u32 * p[0] as u32 + 183 * p[1] as u32 + 19 * p[2] as u32) / 256;
            out.push(luma < threshold as u32);
        }
    }
    out
}
```

- [ ] **Step 4: Confirm pass.**

### Task 4.4: Classify each ink pixel via neighbour code

- [ ] **Step 1: Failing test**

```rust
#[test]
fn classify_endpoint_at_line_terminus() {
    // 5-pixel horizontal line: (0,0)..(4,0).
    let buf = horizontal_line_buffer(5);
    let mask = extract_ink_mask(&buf, 128);
    let classes = classify_ink_pixels(&mask, 5, 1);
    // (0, 0) and (4, 0) are endpoints; (1..=3, 0) are borders.
    assert_eq!(classes[0], Classification::Endpoint);
    assert_eq!(classes[4], Classification::Endpoint);
    assert_eq!(classes[2], Classification::Border);
}
```

- [ ] **Step 2: Run, confirm fail.**
- [ ] **Step 3: Implement**

```rust
fn neighbour_code(mask: &[bool], w: usize, h: usize, x: usize, y: usize) -> u8 {
    let bit = |dx: i32, dy: i32, shift: u32| -> u8 {
        let nx = x as i32 + dx;
        let ny = y as i32 + dy;
        if nx < 0 || ny < 0 || nx as usize >= w || ny as usize >= h { return 0; }
        if mask[ny as usize * w + nx as usize] { 1 << shift } else { 0 }
    };
    // OpenToonz order (tautoclose.cpp:54-59):
    //   SW = 0, S = 1, SE = 2, W = 3, E = 4, NW = 5, N = 6, NE = 7
    bit(-1,  1, 0)
        | bit( 0,  1, 1)
        | bit( 1,  1, 2)
        | bit(-1,  0, 3)
        | bit( 1,  0, 4)
        | bit(-1, -1, 5)
        | bit( 0, -1, 6)
        | bit( 1, -1, 7)
}

pub(crate) fn classify_ink_pixels(
    mask: &[bool], w: usize, h: usize,
) -> Vec<Classification> {
    let mut out = vec![Classification::Isolated; mask.len()];
    for y in 0..h {
        for x in 0..w {
            if !mask[y * w + x] { continue; }
            let code = neighbour_code(mask, w, h, x, y);
            out[y * w + x] = SKELETON_LUT[code as usize];
        }
    }
    out
}
```

- [ ] **Step 4: Confirm pass.**

### Task 4.5: Endpoint pairing with distance + angle filter

- [ ] **Step 1: Failing test**

```rust
#[test]
fn pair_endpoints_at_2px_gap() {
    // Two short horizontal segments separated by a 2-px gap:
    // (0..3, 0) and (5..8, 0). Expect one pair: endpoints
    // (2, 0) and (5, 0).
    let buf = two_segments_with_gap();
    let mask = extract_ink_mask(&buf, 128);
    let classes = classify_ink_pixels(&mask, buf.width() as usize, buf.height() as usize);
    let pairs = pair_endpoints(&mask, &classes, buf.width() as usize, buf.height() as usize, 10, std::f32::consts::FRAC_PI_2);
    assert_eq!(pairs.len(), 1);
    let (a, b) = pairs[0];
    assert!((a == (2, 0) && b == (5, 0)) || (a == (5, 0) && b == (2, 0)));
}

#[test]
fn pair_endpoints_does_not_pair_12px_gap_at_default_distance() {
    let buf = two_segments_with_gap_size(12);
    let mask = extract_ink_mask(&buf, 128);
    let classes = classify_ink_pixels(&mask, buf.width() as usize, buf.height() as usize);
    let pairs = pair_endpoints(&mask, &classes, buf.width() as usize, buf.height() as usize, 10, std::f32::consts::FRAC_PI_2);
    assert!(pairs.is_empty());
}
```

- [ ] **Step 2: Run, confirm fail.**
- [ ] **Step 3: Implement**

```rust
fn estimate_tangent(mask: &[bool], w: usize, h: usize, x: usize, y: usize) -> (f32, f32) {
    // Walk 2-3 inward pixels along the connected component and
    // compute a unit direction vector. Reference: tautoclose.cpp
    // endpoint tangent estimation block.
    /* ... */
}

fn pair_endpoints(
    mask: &[bool],
    classes: &[Classification],
    w: usize,
    h: usize,
    max_distance: u32,
    max_angle: f32,
) -> Vec<((usize, usize), (usize, usize))> {
    let endpoints: Vec<_> = (0..h).flat_map(|y| (0..w).map(move |x| (x, y)))
        .filter(|&(x, y)| classes[y * w + x] == Classification::Endpoint)
        .collect();

    let max_d2 = (max_distance as i32).pow(2);
    let mut paired = vec![false; endpoints.len()];
    let mut out = Vec::new();

    for i in 0..endpoints.len() {
        if paired[i] { continue; }
        let (xi, yi) = endpoints[i];
        let ti = estimate_tangent(mask, w, h, xi, yi);
        let mut best: Option<(usize, i32)> = None;
        for j in (i + 1)..endpoints.len() {
            if paired[j] { continue; }
            let (xj, yj) = endpoints[j];
            let d2 = (xi as i32 - xj as i32).pow(2) + (yi as i32 - yj as i32).pow(2);
            if d2 > max_d2 { continue; }
            let tj = estimate_tangent(mask, w, h, xj, yj);
            // Endpoint tangents should point roughly *toward each other*
            // — i.e. the angle between ti and (j - i) is small, and
            // similarly for tj.
            if angle_compatible(ti, tj, (xi, yi), (xj, yj), max_angle) {
                if best.map_or(true, |(_, prev)| d2 < prev) {
                    best = Some((j, d2));
                }
            }
        }
        if let Some((j, _)) = best {
            paired[i] = true;
            paired[j] = true;
            out.push((endpoints[i], endpoints[j]));
        }
    }
    out
}

fn angle_compatible(
    ti: (f32, f32),
    tj: (f32, f32),
    pi: (usize, usize),
    pj: (usize, usize),
    max_angle: f32,
) -> bool {
    let dx = pj.0 as f32 - pi.0 as f32;
    let dy = pj.1 as f32 - pi.1 as f32;
    let inv_len = 1.0 / (dx * dx + dy * dy).sqrt();
    let nx = dx * inv_len;
    let ny = dy * inv_len;
    // Both tangents should align with the connecting vector
    // (within max_angle).
    let cos_i = ti.0 * nx + ti.1 * ny;
    let cos_j = -tj.0 * nx + -tj.1 * ny;
    cos_i >= max_angle.cos() && cos_j >= max_angle.cos()
}
```

- [ ] **Step 4: Confirm pass.**

### Task 4.6: Bresenham segment rasterizer

- [ ] **Step 1: Failing test**

```rust
#[test]
fn bresenham_horizontal_segment() {
    let mut mask = SelectionMask::new(10, 1).unwrap();
    rasterize_segment(&mut mask, (2, 0), (5, 0));
    for x in 2..=5 {
        assert!(mask.is_selected(x, 0));
    }
    assert!(!mask.is_selected(1, 0));
    assert!(!mask.is_selected(6, 0));
}

#[test]
fn bresenham_diagonal_segment() {
    let mut mask = SelectionMask::new(10, 10).unwrap();
    rasterize_segment(&mut mask, (0, 0), (4, 4));
    for i in 0..=4 {
        assert!(mask.is_selected(i, i));
    }
}
```

- [ ] **Step 2: Run, confirm fail.**
- [ ] **Step 3: Implement**

```rust
fn rasterize_segment(
    mask: &mut SelectionMask,
    a: (u32, u32),
    b: (u32, u32),
) {
    let (mut x0, mut y0) = (a.0 as i32, a.1 as i32);
    let (x1, y1) = (b.0 as i32, b.1 as i32);
    let dx = (x1 - x0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let dy = -(y1 - y0).abs();
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    loop {
        if x0 >= 0 && y0 >= 0 {
            let _ = mask.set_selected(x0 as u32, y0 as u32, true);
        }
        if x0 == x1 && y0 == y1 { break; }
        let e2 = 2 * err;
        if e2 >= dy { err += dy; x0 += sx; }
        if e2 <= dx { err += dx; y0 += sy; }
    }
}
```

(`SelectionMask::set_selected` signature — verify with grep before
relying on it; adjust to whatever API exists.)

- [ ] **Step 4: Confirm pass.**

### Task 4.7: Wire `close_gaps` end-to-end

- [ ] **Step 1: Failing test**

```rust
#[test]
fn close_gaps_bridges_short_gap() {
    let buf = two_segments_with_gap_size(2);
    let mask = close_gaps(&buf, &GapCloseConfig::default()).unwrap();
    // Bridge pixels at (3, 0) and (4, 0) should now be selected.
    assert!(mask.is_selected(3, 0));
    assert!(mask.is_selected(4, 0));
}

#[test]
fn close_gaps_leaves_large_gap_unbridged() {
    let buf = two_segments_with_gap_size(12);
    let mask = close_gaps(&buf, &GapCloseConfig::default()).unwrap();
    assert!(!mask.is_selected(3, 0));
}
```

- [ ] **Step 2: Run, confirm fail (currently todo!()).**
- [ ] **Step 3: Implement `close_gaps`**

```rust
pub fn close_gaps(
    buffer: &PixelBuffer,
    config: &GapCloseConfig,
) -> Result<SelectionMask> {
    let w = buffer.width() as usize;
    let h = buffer.height() as usize;
    let mask = extract_ink_mask(buffer, config.ink_threshold);
    let classes = classify_ink_pixels(&mask, w, h);
    let pairs = pair_endpoints(
        &mask, &classes, w, h,
        config.closing_distance,
        config.closing_angle_rad,
    );

    let mut closure = SelectionMask::new(buffer.width(), buffer.height())?;
    for (a, b) in pairs {
        rasterize_segment(
            &mut closure,
            (a.0 as u32, a.1 as u32),
            (b.0 as u32, b.1 as u32),
        );
    }
    Ok(closure)
}
```

- [ ] **Step 4: Confirm pass.**

### Task 4.8: Add `magic_wand_with_gap_close`

- [ ] **Step 1: Failing test in `core/src/selection/algorithms.rs`**

```rust
#[test]
fn magic_wand_with_gap_close_fills_through_2px_gap() {
    // Square outline 16x16 with a 2-px gap on the top edge.
    let buf = square_outline_with_top_gap(16, 2);
    let seed = IVec2 { x: 8, y: 8 };
    let mask = magic_wand_with_gap_close(
        &buf, seed, 10, Connectivity::Four,
        Some(GapCloseConfig::default()),
    ).unwrap();
    // Without the gap close, the fill would leak to the outside;
    // with it, only interior pixels are selected.
    assert!(mask.is_selected(8, 8));
    assert!(!mask.is_selected(0, 0)); // corner outside the square
}
```

- [ ] **Step 2: Run, confirm fail.**
- [ ] **Step 3: Implement**

```rust
pub fn magic_wand_with_gap_close(
    buffer: &PixelBuffer,
    seed: IVec2,
    tolerance: u8,
    connectivity: Connectivity,
    gap_config: Option<GapCloseConfig>,
) -> Result<SelectionMask> {
    // Build a temporary buffer where gap-bridge pixels are stamped
    // with the local ink color, then run the standard magic_wand on
    // it. This lets the existing flood-fill algorithm respect the
    // closure without modification.
    let mut buf = buffer.clone();
    if let Some(cfg) = gap_config {
        let closure = autoclose::close_gaps(buffer, &cfg)?;
        stamp_closure_pixels(&mut buf, &closure, cfg.ink_threshold);
    }
    magic_wand(&buf, seed, tolerance, connectivity)
}
```

Add `stamp_closure_pixels` as a small private helper.

- [ ] **Step 4: Confirm pass.**

### Task 4.9: Commit S57

```bash
git add core/build.rs core/src/selection/autoclose.rs \
        core/src/selection/skeleton_lut.rs \
        core/src/selection/mod.rs core/src/selection/algorithms.rs \
        core/tests/autoclose_integration.rs
git commit -m "$(cat <<'EOF'
feat(s57): gap-closing magic wand [opentoonz]

New core/src/selection/autoclose.rs adds endpoint detection,
distance + angle pairing, and Bresenham segment rasterization to
bridge short gaps in ink outlines. Existing magic_wand stays
unchanged; new magic_wand_with_gap_close threads the closure
through a temporary buffer.

8-neighbour skeleton LUT generated by core/build.rs from
classification rules so the 256-entry table is reproducible
rather than copy-pasted from skeletonlut.h.

Adapted from OpenToonz toonz/sources/common/trop/tautoclose.cpp
under BSD-3-Clause. See THIRD_PARTY_NOTICES.md.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 5: S58 — procedural inbetween fallback

**Files:**
- Create: `ai/src/verbs/inbetween/procedural.rs`
- Modify: `ai/src/verbs/inbetween/mod.rs`
- Create: `ai/tests/inbetween_procedural.rs`

OpenToonz reference:
`toonz/sources/common/tvrender/tinbetween.cpp:21-98`.

### Task 5.1: Add `InbetweenMode` enum

- [ ] **Step 1: Failing test**

In `ai/src/verbs/inbetween/mod.rs` tests:

```rust
#[test]
fn inbetween_mode_defaults_to_ai() {
    let m: InbetweenMode = Default::default();
    assert!(matches!(m, InbetweenMode::Ai));
}

#[test]
fn inbetween_mode_round_trips_json_for_all_variants() {
    let modes = [
        InbetweenMode::Procedural { variance_range: 2.5 },
        InbetweenMode::Ai,
        InbetweenMode::AiWithProceduralPreview { variance_range: 2.5 },
    ];
    for m in modes {
        let j = serde_json::to_string(&m).unwrap();
        let back: InbetweenMode = serde_json::from_str(&j).unwrap();
        assert_eq!(m, back);
    }
}
```

- [ ] **Step 2: Run, confirm fail.**
- [ ] **Step 3: Add the enum**

```rust
/// Inbetween generation strategy.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InbetweenMode {
    /// Procedural variance-rejected weighted averaging. No backend.
    Procedural { variance_range: f32 },
    /// Backend-driven frame interpolation.
    #[default]
    Ai,
    /// Procedural preview, then optional AI invocation.
    AiWithProceduralPreview { variance_range: f32 },
}
```

Add `mode: InbetweenMode` to `InbetweenInputs` with
`#[serde(default)]`.

- [ ] **Step 4: Confirm pass.**

### Task 5.2: Implement `interpolate_frames` (procedural)

- [ ] **Step 1: Failing tests in `procedural.rs`**

Create `ai/src/verbs/inbetween/procedural.rs`:

```rust
//! Variance-rejected weighted averaging for raster inbetweening.
//!
//! Adapted from OpenToonz toonz/sources/common/tvrender/tinbetween.cpp
//! under BSD-3-Clause. See THIRD_PARTY_NOTICES.md.

#[must_use]
pub(super) fn interpolate_frames(
    frame_a: &[u8],
    frame_b: &[u8],
    width: u32,
    height: u32,
    t: f32,
    variance_range: f32,
) -> Vec<u8> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t0_returns_frame_a_byte_for_byte() {
        let a = vec![255, 0, 0, 255, 0, 255, 0, 255];
        let b = vec![0, 0, 255, 255, 0, 255, 0, 255];
        let out = interpolate_frames(&a, &b, 2, 1, 0.0, 2.5);
        assert_eq!(out, a);
    }

    #[test]
    fn t1_returns_frame_b_byte_for_byte() {
        let a = vec![255, 0, 0, 255, 0, 255, 0, 255];
        let b = vec![0, 0, 255, 255, 0, 255, 0, 255];
        let out = interpolate_frames(&a, &b, 2, 1, 1.0, 2.5);
        assert_eq!(out, b);
    }

    #[test]
    fn t05_solid_buffers_same_color_returns_same() {
        let a = vec![100, 100, 100, 255; 16].into_iter().flatten().collect::<Vec<u8>>();
        // (simpler vec init below)
        let a = std::iter::repeat([100, 100, 100, 255]).take(4).flatten().collect::<Vec<u8>>();
        let b = a.clone();
        let out = interpolate_frames(&a, &b, 2, 2, 0.5, 2.5);
        assert_eq!(out, a);
    }

    #[test]
    fn deterministic() {
        let a = vec![10, 20, 30, 255, 40, 50, 60, 255];
        let b = vec![60, 50, 40, 255, 30, 20, 10, 255];
        let r1 = interpolate_frames(&a, &b, 2, 1, 0.5, 2.5);
        let r2 = interpolate_frames(&a, &b, 2, 1, 0.5, 2.5);
        assert_eq!(r1, r2);
    }
}
```

- [ ] **Step 2: Run, confirm fail.**
- [ ] **Step 3: Implement**

```rust
pub(super) fn interpolate_frames(
    frame_a: &[u8],
    frame_b: &[u8],
    width: u32,
    height: u32,
    t: f32,
    variance_range: f32,
) -> Vec<u8> {
    let pixels = (width * height) as usize;
    assert_eq!(frame_a.len(), pixels * 4);
    assert_eq!(frame_b.len(), pixels * 4);

    let mut out = vec![0u8; pixels * 4];
    for i in 0..pixels {
        for c in 0..4 {
            let a = frame_a[i * 4 + c] as f32;
            let b = frame_b[i * 4 + c] as f32;
            // Two-sample average is trivially the variance-rejected
            // weighted average for n=2 — no rejection possible.
            // For n>2 (sampling neighbours, see below), reject
            // outliers per OpenToonz's tinbetween.cpp.
            //
            // For raster inbetween from two frames we sample a
            // 3x3 neighbourhood from each frame and apply the
            // variance-rejection scheme. This preserves edges
            // better than naive lerp.
            let val = sample_variance_rejected(
                frame_a, frame_b, width, height, i, c, t, variance_range,
            );
            out[i * 4 + c] = val.round().clamp(0.0, 255.0) as u8;
        }
    }
    out
}

fn sample_variance_rejected(
    frame_a: &[u8],
    frame_b: &[u8],
    width: u32,
    height: u32,
    pixel_index: usize,
    channel: usize,
    t: f32,
    variance_range: f32,
) -> f32 {
    let x = (pixel_index as u32 % width) as i32;
    let y = (pixel_index as u32 / width) as i32;

    let mut samples = Vec::with_capacity(18);
    for dy in -1..=1 {
        for dx in -1..=1 {
            let nx = x + dx;
            let ny = y + dy;
            if nx < 0 || ny < 0 || nx >= width as i32 || ny >= height as i32 { continue; }
            let idx = (ny as u32 * width + nx as u32) as usize;
            let va = frame_a[idx * 4 + channel] as f32;
            let vb = frame_b[idx * 4 + channel] as f32;
            samples.push(va * (1.0 - t) + vb * t);
        }
    }

    let n = samples.len() as f32;
    let mean: f32 = samples.iter().sum::<f32>() / n;
    let variance: f32 = samples.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / n;

    let mut accum = 0.0;
    let mut count = 0u32;
    for s in samples {
        let err2 = (s - mean).powi(2);
        if err2 <= variance_range * variance {
            accum += s;
            count += 1;
        }
    }
    if count > 0 { accum / count as f32 } else { mean }
}
```

- [ ] **Step 4: Confirm pass.**

### Task 5.3: Palette snap option

- [ ] **Step 1: Failing test**

```rust
#[test]
fn snap_to_palette_picks_nearest_color() {
    let palette = vec![
        Rgba::new(0, 0, 0, 255),
        Rgba::new(255, 255, 255, 255),
    ];
    let input = vec![100, 100, 100, 255]; // closer to black
    let out = snap_buffer_to_palette(&input, &palette);
    assert_eq!(out, vec![0, 0, 0, 255]);
}
```

- [ ] **Step 2: Implement**

```rust
pub(super) fn snap_buffer_to_palette(buf: &[u8], palette: &[Rgba]) -> Vec<u8> {
    let mut out = Vec::with_capacity(buf.len());
    for chunk in buf.chunks_exact(4) {
        let here = Rgba::new(chunk[0], chunk[1], chunk[2], chunk[3]);
        let nearest = nearest_color_index(palette.iter().copied(), here)
            .map_or(here, |i| palette[i]);
        out.extend_from_slice(&[nearest.r, nearest.g, nearest.b, nearest.a]);
    }
    out
}
```

- [ ] **Step 3: Confirm pass.**

### Task 5.4: Wire procedural path into `InbetweenVerb::invoke`

- [ ] **Step 1: Failing integration test in `ai/tests/inbetween_procedural.rs`**

```rust
use pixhaus_ai::verbs::inbetween::{InbetweenInputs, InbetweenMode, InbetweenVerb};

#[tokio::test]
async fn procedural_mode_succeeds_without_backend() {
    let verb = InbetweenVerb::new();
    let inputs = make_inputs(InbetweenMode::Procedural { variance_range: 2.5 });
    let ctx = make_context_without_backend();
    let cancel = tokio_util::sync::CancellationToken::new();

    let out = verb.invoke(inputs, ctx, cancel).await.expect("ok");
    assert!(matches!(out.effects[0], pixhaus_ai::plugin::output::VerbEffect::AddFrames { .. }));
}
```

- [ ] **Step 2: Run, confirm fail.**
- [ ] **Step 3: Branch in `invoke`**

In `ai/src/verbs/inbetween/mod.rs::InbetweenVerb::invoke`:

```rust
match inputs.mode {
    InbetweenMode::Procedural { variance_range } => {
        let out_buffers = (1..=inputs.num_outputs).map(|i| {
            let t = i as f32 / (inputs.num_outputs + 1) as f32;
            procedural::interpolate_frames(
                &inputs.frame_a.pixels,
                &inputs.frame_b.pixels,
                inputs.frame_a.width,
                inputs.frame_a.height,
                t,
                variance_range,
            )
        }).collect::<Vec<_>>();
        return Ok(self.build_output_from_buffers(out_buffers, inputs));
    }
    InbetweenMode::Ai => { /* existing AI path */ }
    InbetweenMode::AiWithProceduralPreview { variance_range } => {
        let preview = procedural::interpolate_frames(/* ... */);
        // Emit preview as VerbProgressEvent::Partial, then proceed
        // with the AI path.
        ctx.progress.send_partial(/* ... */).await?;
        /* existing AI path */
    }
}
```

- [ ] **Step 4: Confirm pass.**

### Task 5.5: Commit S58

```bash
git add ai/src/verbs/inbetween/mod.rs ai/src/verbs/inbetween/procedural.rs \
        ai/tests/inbetween_procedural.rs
git commit -m "$(cat <<'EOF'
feat(s58): procedural inbetween fallback [opentoonz]

Add InbetweenMode (Procedural | Ai | AiWithProceduralPreview) to
InbetweenInputs. Procedural path runs variance-rejected weighted
averaging from a 3x3 neighbourhood per pixel, with no backend
dispatch. AiWithProceduralPreview emits the procedural result as a
VerbProgressEvent before invoking the configured backend.

Adapted from OpenToonz toonz/sources/common/tvrender/tinbetween.cpp
under BSD-3-Clause. See THIRD_PARTY_NOTICES.md.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 6: S59 — centerline vectorization (new `vectorize` crate)

This is the largest stream in the PR. The crate is decomposed into
six modules; each module gets its own task subgroup with TDD.

**Files:**
- Modify: `Cargo.toml` (workspace `members`)
- Create: `vectorize/Cargo.toml`
- Create: `vectorize/README.md`
- Create: `vectorize/src/lib.rs`
- Create: `vectorize/src/types.rs`
- Create: `vectorize/src/config.rs`
- Create: `vectorize/src/error.rs`
- Create: `vectorize/src/contour.rs`
- Create: `vectorize/src/skeleton.rs`
- Create: `vectorize/src/organize.rs`
- Create: `vectorize/src/stroke_fit.rs`
- Create: `vectorize/tests/e2e.rs`

OpenToonz references:
- `toonz/sources/toonzlib/tcenterlinevectorizer.cpp`
- `toonz/sources/toonzlib/centerlinepolygonizer.cpp`
- `toonz/sources/toonzlib/centerlineskeletonizer.cpp`
- `toonz/sources/toonzlib/centerlinetostroke.cpp`

### Task 6.1: Crate scaffold

- [ ] **Step 1: Create `vectorize/Cargo.toml`**

```toml
[package]
name = "pixhaus-vectorize"
version = "0.1.0"
edition = "2024"
description = "Centerline vectorization. Adapted from OpenToonz under BSD-3-Clause."
license = "MIT AND BSD-3-Clause"

[dependencies]
pixhaus-core = { path = "../core" }
palette = { workspace = true }
rayon = { workspace = true }
serde = { workspace = true, features = ["derive"] }
thiserror = { workspace = true }
tracing = { workspace = true }

[dev-dependencies]
rstest = { workspace = true }
insta = { workspace = true, features = ["yaml"] }
image = { workspace = true }
```

- [ ] **Step 2: Register in workspace `Cargo.toml`**

Add `"vectorize"` to the `members` array.

- [ ] **Step 3: Stub `vectorize/src/lib.rs`**

```rust
//! Centerline vectorization.
//!
//! Adapted from OpenToonz toonz/sources/toonzlib/ under BSD-3-Clause.
//! See THIRD_PARTY_NOTICES.md.

pub mod config;
pub mod error;
pub mod types;

mod contour;
mod organize;
mod skeleton;
mod stroke_fit;

pub use config::CenterlineConfig;
pub use error::{Error, Result};
pub use types::{VectorImage, Stroke, Vertex};

pub fn centerline_vectorize(
    raster: &pixhaus_core::canvas::PixelBuffer,
    palette: &pixhaus_core::project::Palette,
    config: &CenterlineConfig,
) -> Result<VectorImage> {
    let _ = (raster, palette, config);
    todo!("implemented in subsequent tasks")
}
```

- [ ] **Step 4: Verify the crate compiles**

```bash
cargo check -p pixhaus-vectorize
```

### Task 6.2: `types.rs` — `Vertex`, `Stroke`, `VectorImage`

- [ ] **Step 1: Failing tests**

In `vectorize/src/types.rs`:

```rust
//! Public types: Vertex, Stroke, VectorImage.
//!
//! Adapted from OpenToonz toonz/sources/include/tstroke.h shape
//! under BSD-3-Clause.

use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Vertex {
    pub x: f32,
    pub y: f32,
    /// Local stroke half-width at this vertex.
    pub thickness: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Stroke {
    pub vertices: Vec<Vertex>,
    pub style_id: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct VectorImage {
    pub strokes: Vec<Stroke>,
    pub width: u32,
    pub height: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vector_image_default_is_empty() {
        let v = VectorImage::default();
        assert!(v.strokes.is_empty());
    }
}
```

- [ ] **Step 2: Run, confirm pass.**

### Task 6.3: `config.rs` — `CenterlineConfig`

- [ ] **Step 1: Failing test**

```rust
#[test]
fn config_defaults_match_opentoonz() {
    let c = CenterlineConfig::default();
    assert!((c.max_thickness - 10.0).abs() < 1e-6);
    assert!((c.thickness_ratio - 1.0).abs() < 1e-6);
    assert_eq!(c.min_segment_length, 2);
    assert!((c.simplify_tolerance - 0.5).abs() < 1e-6);
}
```

- [ ] **Step 2: Implement**

```rust
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct CenterlineConfig {
    pub max_thickness: f32,
    pub thickness_ratio: f32,
    pub min_segment_length: u32,
    pub corner_threshold_rad: f32,
    pub simplify_tolerance: f32,
}

impl Default for CenterlineConfig {
    fn default() -> Self {
        Self {
            max_thickness: 10.0,
            thickness_ratio: 1.0,
            min_segment_length: 2,
            corner_threshold_rad: 0.5,
            simplify_tolerance: 0.5,
        }
    }
}
```

- [ ] **Step 3: Confirm pass.**

### Task 6.4: `error.rs`

```rust
//! Local error enum for pixhaus-vectorize.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("empty input raster")]
    EmptyRaster,
    #[error("palette has no entries")]
    EmptyPalette,
    #[error("skeleton extraction failed: {0}")]
    Skeleton(String),
    #[error("stroke fitting failed: {0}")]
    StrokeFit(String),
}

pub type Result<T> = std::result::Result<T, Error>;
```

### Task 6.5: `contour.rs` — Moore-neighbor contour extraction

Reference: `toonzlib/centerlinepolygonizer.cpp`.

- [ ] **Step 1: Failing tests**

```rust
//! Contour extraction (raster -> closed polygons).
//!
//! Adapted from OpenToonz toonz/sources/toonzlib/centerlinepolygonizer.cpp
//! under BSD-3-Clause.

use pixhaus_core::canvas::PixelBuffer;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Contour {
    pub vertices: Vec<(i32, i32)>,
    pub is_outer: bool,
}

pub(crate) fn polygonize(
    ink_mask: &[bool],
    width: u32,
    height: u32,
) -> Vec<Contour> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_pixel_blob_yields_one_contour() {
        let mut mask = vec![false; 9];
        mask[4] = true; // center of 3x3
        let contours = polygonize(&mask, 3, 3);
        assert_eq!(contours.len(), 1);
        assert!(contours[0].is_outer);
    }

    #[test]
    fn solid_square_yields_one_outer_contour() {
        let mask = vec![true; 16]; // 4x4 solid
        let contours = polygonize(&mask, 4, 4);
        assert_eq!(contours.len(), 1);
        assert!(contours[0].is_outer);
        assert!(contours[0].vertices.len() >= 4);
    }

    #[test]
    fn solid_square_with_hole_yields_outer_plus_inner() {
        // 6x6 with a 2x2 hole in the center.
        let mut mask = vec![true; 36];
        for y in 2..=3 { for x in 2..=3 { mask[y * 6 + x] = false; } }
        let contours = polygonize(&mask, 6, 6);
        assert_eq!(contours.len(), 2);
        let outer = contours.iter().filter(|c| c.is_outer).count();
        let inner = contours.iter().filter(|c| !c.is_outer).count();
        assert_eq!(outer, 1);
        assert_eq!(inner, 1);
    }
}
```

- [ ] **Step 2: Run, confirm fail.**
- [ ] **Step 3: Implement Moore-neighbor tracing**

```rust
pub(crate) fn polygonize(
    ink_mask: &[bool],
    width: u32,
    height: u32,
) -> Vec<Contour> {
    let w = width as usize;
    let h = height as usize;
    let mut visited = vec![false; ink_mask.len()];
    let mut out = Vec::new();

    for y in 0..h {
        for x in 0..w {
            if !ink_mask[y * w + x] || visited[y * w + x] { continue; }
            let is_outer = is_outer_boundary_pixel(ink_mask, w, h, x, y);
            if !is_outer && !is_inner_boundary_pixel(ink_mask, w, h, x, y) { continue; }
            let verts = trace_moore_neighbor(ink_mask, w, h, x, y, &mut visited);
            out.push(Contour { vertices: verts, is_outer });
        }
    }
    out
}

fn trace_moore_neighbor(/* ... */) -> Vec<(i32, i32)> { /* ... */ }
fn is_outer_boundary_pixel(/* ... */) -> bool { /* ... */ }
fn is_inner_boundary_pixel(/* ... */) -> bool { /* ... */ }
```

Reference the OpenToonz file for the exact direction-list and
backtrack logic.

- [ ] **Step 4: Confirm pass.**

### Task 6.6: `skeleton.rs` — distance-transform + thinning

Reference: `toonzlib/centerlineskeletonizer.cpp`.

OpenToonz uses Voronoi-of-vertices for skeletonization; we
implement an equivalent via distance transform + Zhang-Suen
thinning, which is simpler to verify and produces equivalent
results for raster ink. The distance transform supplies the
thickness annotation per skeleton pixel.

- [ ] **Step 1: Failing tests**

```rust
#[test]
fn skeleton_of_horizontal_bar_is_centerline() {
    // 20x6 horizontal bar of solid ink.
    let mask: Vec<bool> = (0..120).map(|i| i / 20 != 0 && i / 20 != 5).collect();
    let skel = skeletonize(&mask, 20, 6);
    // Centerline should be on row 2 or 3 (the middle two rows).
    let count_row_2 = (0..20).filter(|&x| skel.pixels[2 * 20 + x].on).count();
    let count_row_3 = (0..20).filter(|&x| skel.pixels[3 * 20 + x].on).count();
    assert!(count_row_2 + count_row_3 > 18);
}

#[test]
fn skeleton_records_thickness_at_each_node() {
    let mask: Vec<bool> = (0..120).map(|i| i / 20 != 0 && i / 20 != 5).collect();
    let skel = skeletonize(&mask, 20, 6);
    let on_pix = skel.pixels.iter().find(|p| p.on).unwrap();
    assert!(on_pix.thickness > 1.0);
}
```

- [ ] **Step 2: Run, confirm fail.**
- [ ] **Step 3: Implement**

```rust
//! Skeletonization (polygon -> medial-axis graph).
//!
//! Adapted from OpenToonz toonz/sources/toonzlib/centerlineskeletonizer.cpp
//! under BSD-3-Clause. We use distance-transform + Zhang-Suen
//! thinning rather than OpenToonz's Voronoi-of-vertices approach;
//! the two yield equivalent medial-axis graphs for raster ink.

#[derive(Copy, Clone, Debug)]
pub(crate) struct SkeletonPixel {
    pub on: bool,
    pub thickness: f32,
}

pub(crate) struct Skeleton {
    pub pixels: Vec<SkeletonPixel>,
    pub width: u32,
    pub height: u32,
}

pub(crate) fn skeletonize(
    mask: &[bool],
    width: u32,
    height: u32,
) -> Skeleton {
    let dt = distance_transform(mask, width, height);
    let mut thin = mask.to_vec();
    zhang_suen_thin(&mut thin, width, height);
    let pixels = thin.iter().zip(dt.iter())
        .map(|(&on, &t)| SkeletonPixel { on, thickness: t })
        .collect();
    Skeleton { pixels, width, height }
}

fn distance_transform(mask: &[bool], w: u32, h: u32) -> Vec<f32> { /* ... */ }
fn zhang_suen_thin(mask: &mut [bool], w: u32, h: u32) { /* ... */ }
```

Implement `distance_transform` as a two-pass chamfer (forward +
reverse with 3-4-5 or 5-7-11 weights). Implement `zhang_suen_thin`
iteratively until no pixel is removed in a full sub-iteration.

- [ ] **Step 4: Confirm pass.**

### Task 6.7: Skeleton graph extraction

- [ ] **Step 1: Failing test**

```rust
#[test]
fn extract_graph_yields_nodes_and_edges() {
    let skel = skeletonize_simple_bar();
    let graph = extract_graph(&skel);
    // Bar -> two endpoint nodes, one edge between them.
    assert_eq!(graph.endpoints().count(), 2);
    assert_eq!(graph.edges.len(), 1);
}
```

- [ ] **Step 2: Implement** — extract graph from binary skeleton via
neighbour counting + path tracing.

```rust
pub(crate) struct SkeletonGraph {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

pub(crate) struct Node {
    pub x: u32,
    pub y: u32,
    pub kind: NodeKind,
}

pub(crate) enum NodeKind { Endpoint, Branch }

pub(crate) struct Edge {
    pub a: usize,
    pub b: usize,
    pub path: Vec<(u32, u32, f32)>, // (x, y, thickness)
}

pub(crate) fn extract_graph(skel: &Skeleton) -> SkeletonGraph { /* ... */ }
```

- [ ] **Step 3: Confirm pass.**

### Task 6.8: `organize.rs` — branch pruning and merge

Reference: `tcenterlinevectorizer.cpp::organizeGraphs`.

- [ ] **Step 1: Failing test**

```rust
#[test]
fn organize_drops_branch_shorter_than_min_segment() {
    let graph = bar_with_stub_branch(/* main bar with a 1-px stub */);
    let cfg = CenterlineConfig { min_segment_length: 3, ..Default::default() };
    let out = organize(graph, &cfg);
    assert_eq!(out.edges.len(), 1); // stub pruned
}
```

- [ ] **Step 2: Implement**

```rust
pub(crate) fn organize(
    mut graph: SkeletonGraph,
    config: &CenterlineConfig,
) -> SkeletonGraph {
    // Repeatedly drop edges whose length < min_segment_length.
    // After each drop, merge through degree-2 nodes.
    /* ... */
    graph
}
```

- [ ] **Step 3: Confirm pass.**

### Task 6.9: `stroke_fit.rs` — Douglas-Peucker + Bézier fit

Reference: `tcenterlinevectorizer.cpp::conversionToStrokes`.

- [ ] **Step 1: Failing tests**

```rust
#[test]
fn douglas_peucker_straight_line_keeps_endpoints() {
    let path = vec![(0.0, 0.0), (1.0, 0.0), (2.0, 0.0), (3.0, 0.0)];
    let out = douglas_peucker(&path, 0.5);
    assert_eq!(out, vec![(0.0, 0.0), (3.0, 0.0)]);
}

#[test]
fn fit_stroke_straight_line_one_segment_within_tolerance() {
    let path = vec![(0.0, 0.0, 1.0), (10.0, 0.0, 1.0)];
    let stroke = fit_stroke(&path, &CenterlineConfig::default());
    // Single segment between the two vertices.
    assert_eq!(stroke.vertices.len(), 2);
}
```

- [ ] **Step 2: Implement**

```rust
pub(crate) fn douglas_peucker(
    path: &[(f32, f32)],
    tolerance: f32,
) -> Vec<(f32, f32)> { /* ... */ }

pub(crate) fn fit_stroke(
    path_with_thickness: &[(f32, f32, f32)],
    config: &CenterlineConfig,
) -> Stroke { /* ... */ }
```

- [ ] **Step 3: Confirm pass.**

### Task 6.10: Wire `centerline_vectorize`

- [ ] **Step 1: Failing end-to-end test in `vectorize/tests/e2e.rs`**

```rust
use pixhaus_core::canvas::PixelBuffer;
use pixhaus_core::project::{Palette, PaletteId, Rgba};
use pixhaus_vectorize::*;

#[test]
fn e2e_solid_square_outline_vectorizes() {
    let buf = square_outline_buffer(32);
    let palette = Palette::from_colors(
        PaletteId::new(0), "test",
        vec![Rgba::new(0, 0, 0, 255)],
    );
    let vi = centerline_vectorize(&buf, &palette, &CenterlineConfig::default())
        .expect("vectorize");
    assert!(!vi.strokes.is_empty());
    assert_eq!(vi.width, 32);
    assert_eq!(vi.height, 32);
}
```

- [ ] **Step 2: Implement `lib.rs::centerline_vectorize`**

```rust
pub fn centerline_vectorize(
    raster: &pixhaus_core::canvas::PixelBuffer,
    palette: &pixhaus_core::project::Palette,
    config: &CenterlineConfig,
) -> Result<VectorImage> {
    if raster.width() == 0 || raster.height() == 0 {
        return Err(Error::EmptyRaster);
    }
    if palette.colors.is_empty() {
        return Err(Error::EmptyPalette);
    }

    let ink_mask = extract_ink_mask(raster);
    let _contours = contour::polygonize(&ink_mask, raster.width(), raster.height());
    let skel = skeleton::skeletonize(&ink_mask, raster.width(), raster.height());
    let graph = skeleton::extract_graph(&skel);
    let organized = organize::organize(graph, config);

    let strokes: Vec<Stroke> = organized.edges.iter()
        .map(|e| stroke_fit::fit_stroke(&e.path, config))
        .collect();

    Ok(VectorImage {
        strokes,
        width: raster.width(),
        height: raster.height(),
    })
}

fn extract_ink_mask(raster: &PixelBuffer) -> Vec<bool> {
    let mut mask = Vec::with_capacity((raster.width() * raster.height()) as usize);
    for y in 0..raster.height() {
        for x in 0..raster.width() {
            let p = raster.pixel(x, y).expect("in bounds");
            let luma = (54u32 * p[0] as u32 + 183 * p[1] as u32 + 19 * p[2] as u32) / 256;
            mask.push(luma < 128);
        }
    }
    mask
}
```

- [ ] **Step 3: Confirm pass.**

### Task 6.11: Insta snapshot of vector output

- [ ] **Step 1: Add snapshot test**

```rust
#[test]
fn e2e_snapshot_simple_outline() {
    let buf = square_outline_buffer(16);
    let palette = simple_palette();
    let vi = centerline_vectorize(&buf, &palette, &CenterlineConfig::default()).unwrap();
    insta::assert_yaml_snapshot!(vi);
}
```

- [ ] **Step 2: Review the produced `.snap.new`, accept manually.**

- [ ] **Step 3: Commit the snapshot.**

### Task 6.12: README + Cargo.toml metadata

- [ ] **Step 1: Write `vectorize/README.md`**

```markdown
# pixhaus-vectorize

Centerline vectorization for raster ink layers. Public API:

```rust
let vi = centerline_vectorize(&raster, &palette, &config)?;
```

## Adaptations

The pipeline is adapted from OpenToonz under BSD-3-Clause. Specific
references:

- `toonz/sources/toonzlib/centerlinepolygonizer.cpp` -> `src/contour.rs`
- `toonz/sources/toonzlib/centerlineskeletonizer.cpp` -> `src/skeleton.rs`
- `toonz/sources/toonzlib/tcenterlinevectorizer.cpp` -> `src/lib.rs` + `src/organize.rs`
- `toonz/sources/toonzlib/centerlinetostroke.cpp` -> `src/stroke_fit.rs`

See the repo-root `THIRD_PARTY_NOTICES.md` for the full BSD-3-Clause
grant and Dwango copyright.
```

### Task 6.13: Commit S59

```bash
git add Cargo.toml vectorize/
git commit -m "$(cat <<'EOF'
feat(s59): centerline vectorization crate [opentoonz]

New pixhaus-vectorize workspace crate: raster ink -> stroked
VectorImage via Moore-neighbor contour extraction, distance-
transform + Zhang-Suen thinning skeletonization, branch pruning,
and Ramer-Douglas-Peucker + Bezier stroke fitting.

Public API: centerline_vectorize(raster, palette, config) ->
Result<VectorImage>.

Adapted from OpenToonz toonz/sources/toonzlib/:
- tcenterlinevectorizer.cpp
- centerlinepolygonizer.cpp
- centerlineskeletonizer.cpp
- centerlinetostroke.cpp
under BSD-3-Clause. See THIRD_PARTY_NOTICES.md and the crate-level
README for per-file attribution.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 7: S60 — SIMD audit + criterion baseline

**Files:**
- Create: `docs/planning/research/simd-hot-path-audit.md`
- Modify: `core/Cargo.toml` (add `[[bench]]` entries)
- Create: `core/benches/composite.rs`
- Create: `core/benches/transforms.rs`
- Create: `core/benches/selection.rs`

### Task 7.1: Write the audit doc

- [ ] **Step 1: Create `docs/planning/research/simd-hot-path-audit.md`**

Structure documented in the spec. The audit lists, per loop:
- Current implementation shape (scalar per-pixel, per-channel,
  branchy, etc.)
- SIMD eligibility (Y/N, with brief reason; e.g. "Y: per-channel
  arithmetic with no branches", "N: branchy classifier per pixel")
- Conflict with `palette` crate's typed color API (Y/N)
- Estimated payoff rank (1–10)

Plus a methodology section noting that the criterion benchmarks
were run on the development host; the numbers are baselines for
future SIMD comparison, not absolute targets.

### Task 7.2: Criterion bench scaffold

- [ ] **Step 1: Add `[[bench]]` entries to `core/Cargo.toml`**

```toml
[[bench]]
name = "composite"
harness = false

[[bench]]
name = "transforms"
harness = false

[[bench]]
name = "selection"
harness = false
```

`criterion` should already be in `[dev-dependencies]`; if not, add
it (use the workspace version).

### Task 7.3: Composite benches

- [ ] **Step 1: Create `core/benches/composite.rs`**

```rust
use criterion::{Criterion, criterion_group, criterion_main, BenchmarkId};
use pixhaus_core::canvas::{composite, PixelBuffer};
use pixhaus_core::project::{BlendMode, Rgba};

fn bench_normal_composite(c: &mut Criterion) {
    let src = filled(256, 256, Rgba::new(200, 100, 50, 255));
    let dst = filled(256, 256, Rgba::new(50, 100, 200, 255));
    c.bench_function("composite/normal/256x256", |b| {
        b.iter(|| composite_full(&src, &dst, BlendMode::Normal));
    });
}

fn bench_multiply_composite(c: &mut Criterion) { /* ... */ }
fn bench_overlay_composite(c: &mut Criterion) { /* ... */ }

fn filled(w: u32, h: u32, color: Rgba) -> PixelBuffer { /* ... */ }
fn composite_full(src: &PixelBuffer, dst: &PixelBuffer, mode: BlendMode) -> PixelBuffer { /* ... */ }

criterion_group!(benches, bench_normal_composite, bench_multiply_composite, bench_overlay_composite);
criterion_main!(benches);
```

### Task 7.4: Transform benches

- [ ] **Step 1: Create `core/benches/transforms.rs`**

```rust
use criterion::{Criterion, criterion_group, criterion_main};
use pixhaus_core::canvas::PixelBuffer;
use pixhaus_core::transforms::{rotate, scale};

fn bench_rotate_bilinear_45(c: &mut Criterion) {
    let buf = staircase(256);
    c.bench_function("transforms/rotate_bilinear/256x256/45deg", |b| {
        b.iter(|| rotate::rotate(&buf, 45.0_f32.to_radians(), rotate::Interp::Bilinear));
    });
}

fn bench_scale_nearest_2x(c: &mut Criterion) { /* ... */ }
fn bench_scale_bilinear_1_5x(c: &mut Criterion) { /* ... */ }

criterion_group!(benches, bench_rotate_bilinear_45, bench_scale_nearest_2x, bench_scale_bilinear_1_5x);
criterion_main!(benches);
```

### Task 7.5: Selection benches

- [ ] **Step 1: Create `core/benches/selection.rs`** with
  `bench_magic_wand_dense` and `bench_magic_wand_sparse`.

### Task 7.6: CI compile-check for benches

- [ ] **Step 1: Verify the suite compiles**

```bash
cargo bench --no-run -p pixhaus-core
```

Expected: no errors. Add this to `.github/workflows/ci.yml` (if not
already covered) as a non-gating advisory step. (Investigate the
existing CI file first; if there's already a `benches build` step,
no change is needed.)

### Task 7.7: Commit S60

```bash
git add docs/planning/research/simd-hot-path-audit.md \
        core/Cargo.toml core/benches/
git commit -m "$(cat <<'EOF'
docs(s60): SIMD audit and criterion baseline [opentoonz]

Audit markdown under docs/planning/research/ enumerating hot loops
in core/canvas + core/transforms + core/selection with current
shape, std::simd eligibility, and palette-crate conflict notes.

Criterion benches under core/benches/ establish baseline numbers
for: Normal/Multiply/Overlay composite at 256x256, rotate bilinear
at 45 degrees, scale nearest 2x and bilinear 1.5x, magic_wand
dense and sparse. CI compiles benches via cargo bench --no-run.

No SIMD implementation; produces measurements only. Inspired by
the shape of toonz/sources/common/trop/quickput.cpp.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 8: Pre-PR gate and PR

### Task 8.1: Run the full local gate

- [ ] **Step 1: Format**

```bash
cargo fmt --all
```

- [ ] **Step 2: Clippy**

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

- [ ] **Step 3: Test**

```bash
cargo nextest run --workspace
cargo test --doc --workspace
```

- [ ] **Step 4: TS**

```bash
pnpm typecheck
pnpm lint
pnpm test
```

- [ ] **Step 5: Build (sanity)**

```bash
cargo build --workspace --release
pnpm build
```

- [ ] **Step 6: Pre-PR script if present**

```bash
ls scripts/pre-pr.sh 2>/dev/null && ./scripts/pre-pr.sh
```

If any step fails, fix and re-run — never bypass.

### Task 8.2: Push and open PR

- [ ] **Step 1: Push the branch**

```bash
git push -u origin feat/s54-s60-opentoonz-adoption
```

- [ ] **Step 2: Open the PR**

```bash
gh pr create --base main --title "feat(s54-s60): adopt OpenToonz algorithms" --body "$(cat <<'EOF'
## Summary

Seven OpenToonz-derived improvements landing on a single branch:

- **S54** — palette pages + per-entry keyframed animation (schema MINOR=1)
- **S55** — eight new blend modes: LinearBurn, DarkerColor, LinearDodge, LighterColor, VividLight, LinearLight, PinLight, HardMix
- **S56** — morphological anti-aliasing transform (Reshetov MLAA)
- **S57** — gap-closing magic wand with skeleton LUT + Bresenham closure
- **S58** — procedural inbetween fallback using variance-rejected weighted averaging
- **S59** — new `pixhaus-vectorize` crate: raster ink → centerline VectorImage
- **S60** — SIMD audit doc + criterion baseline (no SIMD code)

Specs: `docs/planning/work/s54-s60-opentoonz-adoption.md`
Plan: `docs/planning/work/s54-s60-opentoonz-adoption-plan.md`

All adapted code is BSD-3-Clause from OpenToonz. Repo-level
`THIRD_PARTY_NOTICES.md` lands with the first commit; each adapted
file carries an inline attribution comment.

## Schema

`SchemaVersion::MINOR` bumps `0 → 1` in S54. All new fields are
serde-default; pre-PR `.pixhaus` files load unchanged. Pre-PR
readers loading new files emit a `tracing::warn!` and may fail on
unknown blend-mode variants — this is documented in the spec as
expected.

## Aseprite compatibility

Eight new blend modes have no Aseprite equivalent and downgrade to
`Normal` on export with a `tracing::warn!` per affected layer. See
`docs/migration/aseprite.md`.

## Test plan

- [ ] `cargo nextest run --workspace` green
- [ ] `cargo test --doc --workspace` green
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` clean
- [ ] `pnpm typecheck` / `pnpm test` green
- [ ] `cargo bench --no-run -p pixhaus-core` compiles
- [ ] Visual: `insta` snapshots accepted and reviewed in commit logs

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

- [ ] **Step 3: Return the PR URL.**

---

## Self-review checklist

The author (or executing agent) ticks each before declaring the
plan ready:

- [ ] Every spec section has at least one corresponding task.
- [ ] Each task has its own commit.
- [ ] No placeholder text ("TODO", "TBD", "add appropriate
      validation") in any step's code block.
- [ ] Function names introduced in early tasks (e.g.
      `channel_linear_burn`) match names used in later tasks (e.g.
      composite dispatcher).
- [ ] Attribution comments appear in every adapted file's header
      and in `THIRD_PARTY_NOTICES.md`.
- [ ] The two schema-related tests (legacy load, MINOR bump assertion)
      both exist and run.
- [ ] Each `cargo nextest run` invocation references a specific
      crate or test pattern, not a vague "run tests".
- [ ] The plan does not invoke any external tool that would block
      the agent on credentials we don't have (no real API calls).
