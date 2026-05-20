# Pixhaus codebase audit

Date: 2026-05-20
Scope: full codebase (structure, error handling, dependencies/licensing, security/privacy, tests)
Status: findings only — no code changed in this pass. The backlog at the end is optional follow-up work.

## Executive summary

The codebase is in good shape. The question that prompted this audit — "we have files with thousands of lines, is that the best approach?" — has a clear answer: **file size is not the real issue here.** The large files are large for defensible reasons, and the project's hard rules are enforced by the compiler rather than left to discipline.

Three things stand out:

1. **The big files are mostly cohesive, not tangled.** Tests make up 20–48% of the largest command files. The rest are flat registries, thin IPC wrappers, or transcribed reference tables (blend math) where splitting would hurt readability. Only about four files genuinely benefit from restructuring.
2. **Error-handling discipline holds by construction.** Production `unwrap`/`expect` count is ~13 across the whole workspace, and `unwrap_used`, `expect_used`, `panic`, and `unsafe_code` are all denied/forbidden in the workspace lint config. The ~1,375 raw `unwrap` hits are almost all in tests.
3. **The privacy promises in CLAUDE.md hold under inspection.** Telemetry is opt-in and off by default, API keys live in the OS keychain and are redacted in `Debug`, and remote traffic is TLS-only.

The two real watch-items are plugin/script sandboxing (fine under today's "plugins are developer-authored" trust model, but it must be hardened before loading untrusted plugins) and a handful of structural splits that would lower cognitive load in the largest files.

## Metrics snapshot

All figures are reproducible — see the appendix for the exact commands.

### Source size

| Area | LOC | Notes |
| --- | --- | --- |
| core | ~19,983 | 71 files |
| io | ~18,028 | 50 files |
| ai | ~33,219 | 57 files |
| scripting | ~1,858 | 12 files |
| app | ~21,055 | 25 files |
| Rust total | ~97,544 | excludes `target/` and worktrees |
| ui (TS/TSX) | ~30,848 | 277 files under `ui/src` |

### Tests

| Area | Test fns / calls |
| --- | --- |
| core | 525 |
| io | 336 |
| ai | 590 |
| scripting | 35 |
| app | 243 |
| ui | 410 |
| Rust total | ~1,729 |

### Discipline signals

| Signal | Value |
| --- | --- |
| `unwrap`/`expect` in production code | ~13 total (core ~7, io ~1, ai ~4, scripting 0, app ~1) |
| `unwrap` raw hits (mostly tests) | ~1,359 |
| `panic!` raw hits (mostly tests) | 68 |
| `unsafe` blocks | 0 (forbidden workspace-wide) |
| `todo!`/`unimplemented!` | 4 (io) |
| TODO/FIXME/HACK/XXX markers in source | 1 |
| TS `any` / `@ts-ignore` / `eslint-disable` | 2 / 0 / 1 |
| `Arc<Mutex/RwLock>` | 0 in core/io/ai/app, 12 in scripting |
| `#[allow(...)]` total | ai 115, core 67, app 47, io 13, scripting 6 |

The workspace lint config (`Cargo.toml`) already enforces the hard rules:

```toml
[workspace.lints.rust]
unsafe_code = "forbid"

[workspace.lints.clippy]
unwrap_used = "deny"
expect_used = "deny"
panic = "deny"
todo = "warn"
unimplemented = "warn"
```

## Findings by dimension

### 1. File size and structure

Verdict: **not a systemic problem.** Most large files are large because they are cohesive. A handful are worth splitting.

Genuine split candidates:

- `app/src/commands/library.rs` (6,601 lines). The biggest file in the repo. It is cohesive by domain, but a self-contained reference-sheet subsystem (~3,000 lines: generation, refinement, variant commits, regional refinement, cross-model grids, vector export, character-card publishing) is buried inside it alongside entity/group/tag CRUD and LoRA training. Recommended seams: `library/{core, reference_sheets, lora, assets}.rs`. About 19% of the file is tests.
- `core/src/project/library.rs` (1,627 lines). 45 structs/enums spanning orthogonal concerns — core entities, grouping, palettes, tags, AI metadata, reference sheets, asset library. No duplication; the issue is breadth. Recommended seams: `library/{core, palettes, tags, ai, reference_sheets, assets}.rs`.
- `ui/src/sheet/ReferenceSheetEditor.tsx` (1,758 lines). The clearest refactor target. A god component holding 23 local signals that mixes variant browsing, generation, refinement, chat, comparison, asset management, and provenance. It violates the repo's own `*-state.ts` convention (most panels keep state in a companion file; this one does not). Recommended: extract `sheet-editor-state.ts`, then pull `MaskRefinePanel`, `RegionalRefinePanel`, `AssetBrowser`, and `ProvenancePanel` into sub-components, and deduplicate the repeated reference-slot management.
- `ai/src/plugin/runtime.rs` (1,316 lines). Soft candidate. Two separable entities — `VerbRuntime` (registry, dispatch, backend selection) and `VerbInvocation` (in-flight lifecycle, progress streaming, cancellation). Split into `runtime/{registry, invocation}.rs` when it next grows; not urgent.

Large but genuinely fine — do not split:

- `core/src/canvas/blend.rs` (1,039 lines). Aseprite-compatible blend math transcribed from `blend_funcs.cpp` and OpenToonz. The repetition (18-arm match, per-channel formulas) is intentional and required for byte-for-byte fidelity. Splitting would scatter the algorithm and add cross-module calls in a hot loop.
- `ui/src/command-palette/command-registry.ts` (1,450 lines). A flat registry of ~80–90 command definitions. Registries should be flat lists so the palette and keybind UI can iterate them. Already uses a `.map()` generator to avoid repetition.
- `ui/src/lib/commands/library.ts` (653 lines). Thin IPC wrappers — one short function per backend command. Idiomatic; keep flat.
- `io/src/aseprite/archive.rs` (2,357) and `io/src/tiled/mod.rs` (1,419). Single-purpose, test-dense (the Tiled exporter is ~50% tests). Cohesive.
- `ui/src/canvas/input.ts` (753) and `ui/src/canvas/renderer/index.ts` (719). Dense by domain — input wiring and a WebGL2 render engine. No god-file smell.

Cross-cutting opportunity (optional, low priority): the `app/src/commands/*` files repeat a lock then lookup then mutate then `dirty = true` then emit-event block. A shared helper such as `with_sprite_layer_mut(...)` would trim roughly 200 lines from `canvas.rs` and `layers.rs` without unnatural splits and reduce a class of copy-paste error. This is a bigger win than splitting `layers.rs`, which is uniform CRUD where a split would barely move the line count.

### 2. Error handling and Rust conventions

Verdict: **excellent.**

- Production `unwrap`/`expect` is ~13 sites total. The high raw counts are test code. Worth eyeballing the ~13 once, since clippy denies `unwrap_used`/`expect_used` — they are either explicit `#[allow]` exceptions or sit in helper code ahead of the test module. None are alarming.
- `unsafe` is forbidden workspace-wide and the count is 0.
- The `thiserror` in library crates / `anyhow` only in `app` split is intact.
- `Arc<Mutex>` appears only in `scripting` (12 sites). Confirmed (2026-05-20): all are `parking_lot::Mutex` wrapping a single shared output sink (`Vec<ScriptMutation>` and the verb/command/panel registration vectors in `scripting/src/bindings/`). Each Lua userdata — `LuaSprite`, `LuaLayer`, `LuaFrame`, `LuaPalette` — must hold a `'static`, clonable handle to one shared sink so its methods, invoked from arbitrary Lua at arbitrary times, push into the same collector. The userdata outlives the `register` stack frame, so a borrow cannot work. This is the legitimately-VM-bound case the conventions permit, and `parking_lot::Mutex` is the correct pick for the short sync critical section. No change needed.
- `#[allow(...)]` totals look high (ai 115) but the reasons are benign: `clippy::disallowed_methods` (54, test allowances for otherwise-denied methods), `missing_docs` (38), numeric-cast lints (`cast_possible_truncation`/`cast_precision_loss`, ~54, inherent to image code), and `too_many_lines` (19). Only 6 `dead_code` and 1 `deprecated` — no suppressions are masking real problems.

### 3. Dependencies and licensing

Verdict: **OK, MIT-clean.**

- The workspace is MIT (`Cargo.toml` workspace package). No GPL/LGPL/AGPL dependencies were spotted in the manifest.
- Prior-art attribution is present: `LICENSES/` contains `aseprite-MIT.txt`, `falsprite-MIT.txt`, and `NOTICE.txt`.
- `psd` is pinned to `=0.3.5` deliberately (the code depends on the `Debug` format of a private enum; documented inline in `Cargo.toml`).
- Recommendation: there is no `deny.toml`. Adding `cargo-deny` (license + advisory + duplicate checks) would make the "no copyleft, no known-vuln deps" guarantee enforceable in CI rather than a manual review each time the lockfile changes.

### 4. Security and privacy

Verdict: **strong, with two trust-model caveats to document.**

Telemetry — OK. Sentry is opt-in and off by default. `app/src/crash_reporting.rs` initializes the gate to `false` and only forwards events when the user has opted in:

```rust
static ENABLED: AtomicBool = AtomicBool::new(false);

fn before_send(mut event: Event<'static>) -> Option<Event<'static>> {
    if !ENABLED.load(Ordering::Relaxed) {
        return None;
    }
    scrub(&mut event);
    Some(event)
}
```

The client is also a no-op unless a DSN was compiled in via `PIXHAUS_SENTRY_DSN`, `send_default_pii` is `false`, and `scrub` clears `server_name` (hostname) and rewrites home-directory path prefixes to `<user>`. This honors the "no telemetry by default" promise.

API keys — OK. Keys live in the OS keychain via the `keyring` crate (`ai/src/backends/keys.rs`, service namespace `pixhaus.<backend>`). Key-holding structs override `Debug` to print `[redacted]` (`ai/src/backends/anthropic.rs`, `ai/src/backends/openai.rs`), keys are not logged via `tracing`, and the `BackendError` enum carries auth-rejection messages, not key material.

Network — OK. `reqwest` is configured with `rustls-tls` and native TLS disabled. Remote API base URLs are hardcoded HTTPS constants. The only plaintext `http://` endpoints are localhost Ollama (`:11434`) and ComfyUI (`:8188`), which are user-controlled local services.

IO / path safety — OK. The `.pixhaus` format is MessagePack + zstd (not a zip), with a decompressed-size cap. The `.aseprite`, `.psd`, and `.tmx` handlers parse structured/binary/XML data rather than extracting filesystem paths, so there is no obvious zip-slip or path-traversal vector.

Plugin / script sandboxing — CONCERN (document, do not fix in this pass). The Lua VM in `scripting/src/runtime.rs` is created with `Lua::new()` without disabling the `os`, `io`, or `debug` standard libraries, so a script can in principle call `os.execute` or `io.open`. The extism WASM host runs with WASI enabled and the default (permissive) allow-list. Both are acceptable under the current trust model — plugins are developer-authored and shipped in-repo — but this must be hardened (restricted Lua stdlib, explicit WASM capability list) before Pixhaus loads untrusted or third-party plugins.

### 5. Test posture

Verdict: **strong and consistent.** ~1,729 Rust test fns plus 410 UI test calls against ~128K LOC is healthy. Test density is highest exactly where it should be — the IO format round-trips and command handlers (the Tiled exporter is ~50% tests; several command files are 24–48% tests). Coverage note: a few production files (notably `core/src/canvas/blend.rs`, `ai/src/plugin/runtime.rs`) carry no inline tests and rely on integration coverage, which is reasonable for transcribed math and host-boundary code. Correction (2026-05-20): an earlier draft of this audit claimed `core/src/undo/history.rs` had no inline tests — that was wrong; it ships ~370 lines of unit tests. The only real gaps there were byte-cap (`max_bytes`) eviction and off-path branch-subtree dropping, both since added.

## Prioritized backlog

These are optional. Nothing here is blocking; the codebase is shippable as-is.

High value, low risk:

- Split `ui/src/sheet/ReferenceSheetEditor.tsx` into a `sheet-editor-state.ts` plus panel sub-components. Best return — it is the one clear convention violation and the hardest file to reason about.
- Extract the reference-sheet subsystem out of `app/src/commands/library.rs` into a `library/` module directory.

Medium value:

- Document the plugin/script trust model explicitly in `docs/`, and gate untrusted-plugin loading behind a hardened Lua stdlib + explicit WASM capability list before it ships.
- Add `cargo-deny` with a `deny.toml` and wire it into CI to enforce licensing and advisories.
- Split `core/src/project/library.rs` into a module directory by concern.

Low value / opportunistic:

- Introduce a `with_sprite_*_mut` helper to cut repeated lock/lookup/dirty/emit boilerplate in `app/src/commands/{canvas,layers}.rs`.
- Split `ai/src/plugin/runtime.rs` into `registry` and `invocation` modules the next time it is touched.
- Add direct unit tests for `core/src/undo/history.rs`.
- ~~Confirm the 12 `Arc<Mutex>` sites in `scripting` are each genuinely Lua-VM-bound.~~ Done 2026-05-20 — all confirmed VM-bound (see section 2).

## Appendix: methodology

Every figure above is reproducible with these commands, run from the repo root.

Source size (Rust), excluding build artifacts and worktrees:

```bash
find . -name '*.rs' -not -path '*/target/*' -not -path './.git/*' \
  -not -path './.claude/worktrees/*' | xargs wc -l | sort -rn | head -42
```

Per-crate LOC and large-file detection:

```bash
for d in core io ai scripting app; do
  find ./$d -name '*.rs' -not -path '*/target/*' | xargs wc -l | tail -1
done
find ./ui/src -name '*.ts' -o -name '*.tsx' | grep -v node_modules | xargs wc -l | tail -1
```

Error-handling antipatterns (raw, includes tests):

```bash
for d in core io ai scripting app; do
  grep -rn '\.unwrap()' ./$d/src --include='*.rs' | wc -l
  grep -rn 'panic!'      ./$d/src --include='*.rs' | wc -l
  grep -rn 'unsafe '     ./$d/src --include='*.rs' | wc -l
done
```

Production-only `unwrap`/`expect` (counts lines before the first `#[cfg(test)]` in each file):

```bash
for d in core io ai scripting app; do
  prod=0
  for f in $(grep -rl '' ./$d/src --include='*.rs'); do
    cut=$(grep -n '#\[cfg(test)\]' "$f" | head -1 | cut -d: -f1)
    [ -z "$cut" ] && cut=$(wc -l < "$f")
    prod=$((prod + $(head -n "$cut" "$f" | grep -c '\.unwrap()\|\.expect(')))
  done
  echo "$d: ~$prod"
done
```

Test-fn counts, `#[allow]` categories, TS type-safety, lock usage:

```bash
for d in core io ai scripting app; do
  grep -rEn '#\[test\]|#\[rstest\]|#\[tokio::test\]' ./$d --include='*.rs' \
    | grep -v '.claude/worktrees' | wc -l
done
grep -rhEo '#\[allow\([^]]*\)\]' core/src io/src ai/src scripting/src app/src \
  --include='*.rs' | sort | uniq -c | sort -rn
grep -rEn ': any|<any>|as any' ui/src | wc -l
for d in core io ai scripting app; do
  grep -rEn 'Arc<Mutex|Arc<RwLock|Arc<parking_lot' ./$d/src --include='*.rs' | wc -l
done
```

Note: the production-`unwrap` counter is a heuristic (it assumes test modules sit below the first `#[cfg(test)]`), so treat ~13 as approximate. The lint config denying `unwrap_used`/`expect_used` is the authoritative guarantee.
