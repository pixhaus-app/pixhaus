## mod-stubs

Both stub modules are fully compliant. `modules/core` and `modules/pixel-art` are deliberate, well-documented compiling stubs: each is a single crate-level `//!` doc that names what the module will register, cites the architecture bible (section 7.3), and marks itself as scaffold-stage. With no executable code, none of the error, ownership, tracing, i18n, UI-token, or boundary rules can be violated, and both crates carry correct minimal manifests with `[lints] workspace = true` and zero dependencies. This is the clean, intentional stub state the roadmap (bible section 26) calls for, not a defect.

### Strengths

- Both crates open with a clear crate-level `//!` doc (`modules/core/src/lib.rs:1-8`, `modules/pixel-art/src/lib.rs:1-9`) that names exactly what the module will register and cites the architecture bible (section 7.3), so the boundary's intent is legible before any body exists.
- Both docs are honest about status ("Scaffold stage: a stub."), matching the CLAUDE.md guidance that a stub is a deliberate decision, and they avoid any `todo!`/`unimplemented!` that would trip the Stop gate.
- Doc-comment prose follows the Voice rules: sentence-case, straight quotes, no emoji, and none of the banned LLM tells.
- Each `Cargo.toml` is minimal and correct: workspace-inherited package fields, `[lints] workspace = true` to pick up the forbid-unsafe and deny-unwrap lint set, and zero dependencies — no premature reach for the dependency catalog.
- Neither stub installs a tracing subscriber, localizes, mutates core state, or registers a capability prematurely, so each honors the `modules/CLAUDE.md` boundary rules by not doing anything yet.

### Findings

No confirmed findings.

### Checked and cleared (false positives)

None.
