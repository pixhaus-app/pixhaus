## io

The `io` crate is in full compliance. It is a deliberate compiling stub exactly as CLAUDE.md and the bible (sections 18, 19) describe: `src/lib.rs` carries a single crate-level `//!` doc and zero items, so error-policy, stack, license, and i18n rules hold trivially and nothing trips the Stop gate. The `Cargo.toml` inherits all workspace metadata and lints and pulls in no dependency before a body needs one. No violations found.

### Strengths

- `src/lib.rs` is an honest stub: a crate-level `//!` doc that names what `io` will own (the `.pixhaus` format, PNG, sprite sheets, importer/exporter traits), states the core-only / never-UI dependency boundary, and explicitly flags "Scaffold stage: a stub" with a pointer to bible sections 18 and 19 — a documented decision, not an accidental gap.
- No half-built bodies and none of `todo!`/`unimplemented!`/`unreachable!`/`panic!`/`unwrap`/`expect`/`dbg!`/`println!` anywhere, so nothing fails the Stop gate's `-D warnings` or the no-unwrap clippy rule. The stub compiles clean precisely because it declares no items.
- `Cargo.toml` is minimal and correct: every field (version, edition, rust-version, authors, license, repository, homepage) inherits from the workspace, lints inherit via `[lints] workspace = true`, and no dependency is pulled in before a body needs it — matching the "no dependency until a crate earns it" stack rule and the bible's note that `rmp-serde`/`zstd`/`blake3` are candidates, not adopted.
- `missing_docs = warn` (workspace) is satisfied by the `//!` crate doc with no undocumented public items, so the stub stays warning-clean.
- The crate doc keeps load/save framing as English developer text and stores ids and keys, consistent with the i18n boundary: `core` and `io` never localize as the source of truth.

### Findings

No confirmed findings.

### Checked and cleared (false positives)

None.
