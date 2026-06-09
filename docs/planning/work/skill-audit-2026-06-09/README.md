# Skill-compliance audit — 2026-06-09

Audit of the Rust codebase against three skills: `pixhaus-rust-conventions`,
`pixhaus-thiserror`, and `pixhaus-rust-modern`. The last skill is new this session
(the Rust 1.85-1.96 feature catalog), so the modernization angle had never been
audited; conventions and thiserror were re-verified after the 2026-06-08
crate-by-crate pass (`../code-audit-2026-06-08/`).

## Method

An ultracode workflow: one auditor agent per non-stub unit grepped its source for
the anti-pattern signals, then read the surrounding code to confirm. Every proposed
finding then went through an adversarial verifier that re-read the cited line and
killed false positives (default-reject when uncertain). 19 agents total.

- Scope: 13 non-stub units, ~37.5K lines. Skipped the three compiling stubs
  (`crates/io`, `modules/core`, `modules/pixel-art`).
- Toolchain at audit time: pin is still `1.95` (the bump to 1.96 is pending the
  guarded `rust-toolchain.toml` edit). The 1.96-only features (`assert_matches!`,
  `From<T>` for `LazyCell`/`LazyLock`, tuple-`!` coercion, `bool: TryFrom<int>`) were
  excluded from "apply now" recommendations; none reached a finding anyway.

## Result at a glance

The codebase adheres closely. The clippy gate (`-D warnings`, with `unwrap_used`/
`expect_used`/`panic` denied in non-test code) and the recent audit have already
removed the structural defects: no `unwrap`/`expect` in production code, no
`Box<dyn Error>` in public APIs, no `std::sync` locks, no lock-across-`.await`, no
`Vec<Vec<_>>` pixel data, no `unsafe`, and `thiserror` used correctly throughout (0
thiserror findings).

- 8 of 13 units fully clean: `crates/platform`, `crates/render`, `crates/ui`,
  `modules/export`, `modules/generation`, `modules/sprite-edit`, `modules/tiles`, `app`.
- 7 confirmed findings, 1 rejected.
- 4 of the 7 are one rule — A14 import grouping — and are **fixed this session**.
- The other 3 are `rust-modern` modernization opportunities, **applied this
  session** at the maintainer's request — which surfaced the MSRV cascade below.

## Fixed this session — A14 import grouping (conventions)

The convention orders `use` groups std / external / workspace(`pixhaus_*`) / local
with a blank line between groups. `cargo fmt` cannot enforce this — `group_imports`
is nightly-only and not enabled, so stable fmt sees one block and only sorts within
it. These four files put the workspace crate above the external crate (or omitted
the group blank line); all now read external-then-workspace with the blank line.

| File | Was | Now |
|---|---|---|
| `crates/services/src/error.rs:3` | `pixhaus_core` above `thiserror` | `thiserror`, blank, `pixhaus_core` |
| `crates/services/src/codex/reference.rs:13` | `pixhaus_core` above `thiserror` | `thiserror`, blank, `pixhaus_core` |
| `crates/services/src/codex/test_support.rs:15` (test) | `pixhaus_core` above `serde_json` | `serde_json`, blank, `pixhaus_core` |
| `modules/animation/src/animate.rs:10` | `egui` adjacent to `pixhaus_*` | `egui`, blank, `pixhaus_*` |

## Applied — rust-modern modernizations

The maintainer asked to apply all three. They are clean, behavior-preserving
improvements (all 1.88, live on the 1.95 pin), each carrying a `//` comment recording
why the modern form is used. The full test suite stays green. Applying them forced
the MSRV cascade documented below.

1. **`modules/codex/src/codex_ws/coverage.rs:194` — C2, let-chains (1.88).** Two
   nested `if let Some(_)` where the inner is the entire body of the outer. Flatten
   to `if let Some(slot_index) = … && let Some(item) = detail.coverage_items.get(slot_index) {…}`.
   Removes one indentation level. The cleanest of the three.
2. **`modules/providers/src/postprocess.rs:52` — C4, `as_chunks_mut::<4>()` (1.88).**
   A `chunks_exact_mut(4)` loop over an RGBA buffer indexing `pixel[0..=3]`. Switch
   to `let (pixels, _rest) = rgba.as_chunks_mut::<4>();` over `&mut [u8; 4]` arrays —
   fixed-size arrays let the optimizer drop the per-iteration bounds checks. Bind
   `_rest` explicitly (well-formed RGBA is a multiple of 4, so it is empty).
3. **`crates/core/src/composite.rs:95` — C4, `as_chunks_mut::<4>()` (1.88).** Same
   pattern on the compositor's output row. Marginal and asymmetric: only the
   destination can adopt it (the source reads on a separate stride), so lowest
   priority.

## MSRV cascade — clippy.toml and Cargo.toml to 1.95

Applying the 1.88 features tripped clippy's `incompatible_msrv` lint, because the
declared MSRV was still `1.85`: you cannot use a post-1.85 feature while the floor
is 1.85. So `rust-version` (`Cargo.toml`) and clippy `msrv` (`clippy.toml`) moved
from `1.85` to `1.95` — the current toolchain pin. MSRV tracks the pin from here, so
it goes to 1.96 when the guarded `rust-toolchain.toml` flip lands.

Raising the MSRV to 1.95 then activated four MSRV-gated clippy modernization lints
that were dormant at 1.85 — the same modern idioms, now machine-enforced. All four
were auto-applied with `cargo clippy --fix` and re-formatted:

- `collapsible_if` (1): `crates/services/src/codex/validation.rs:176` — a nested
  `if let` collapsed into a let-chain.
- `manual_is_multiple_of` (3): `modules/providers/src/postprocess.rs:84` (two) and
  `modules/animation/src/animate.rs:326` — `x % n == 0` became `x.is_multiple_of(n)`
  (1.87).

The broader lesson of the bump: setting the MSRV to the pin does not just permit
modern features, it makes clippy enforce them where they apply. The cascade was
small here (4 sites) because the tree was already clean.

## Rejected finding, and a correctness note on the audit itself

One finding was rejected: `crates/render/src/lib.rs:265` (C12, `strict_mul` for the
`width * 4` stride). It is correctly rejected — `width` is bounded at 8192 and the
buffer length is asserted, so the multiply cannot wrap `u32`; there is no real
release-path overflow to harden.

Worth recording: the verifier's *secondary* justification for that rejection was
**factually wrong** — it claimed `strict_mul` is "an unstable nightly-only feature
(strict_overflow_ops)." It is not. Compiling `100u32.strict_mul(4)` directly on both
installed toolchains returns exit 0:

```
rustc +1.95 --edition 2024  →  exit 0
rustc +1.96 --edition 2024  →  exit 0
```

The whole `pixhaus-rust-modern` shortlist (`strict_*`, `push_mut`, `get_disjoint_mut`,
`as_chunks`, `pop_if`, `extract_if`, `midpoint`, `fmt::from_fn`) compiles clean on
stable 1.95. The verifier reasoned from stale training data — exactly the drift the
`pixhaus-rust-modern` skill exists to counter — so the skill's 1.91 attribution for
`strict_*` stands, confirmed against the compiler rather than memory. The rejection
itself is unaffected: it still holds on the overflow-bound ground.

## Verification

Full Stop-gate green on toolchain 1.95 after every change in this session — the
four A14 import fixes, the three modernizations, the MSRV bump, and the four
`clippy --fix` lints:

- `cargo fmt --all --check` — no diff.
- `cargo clippy --workspace --all-targets -- -D warnings` — 0 errors, 0 warnings.
- `cargo nextest run --workspace` — 644 passed, 2 skipped.
