# pixhaus-io

Import, export, and the on-disk project format (architecture bible sections 18,
19).

- **Owns:** the `.pixhaus` project format, PNG and sprite-sheet codecs, and the
  importer/exporter traits other crates register against.
- **Depends on:** `core`. External: `serde` plus concrete format crates as they
  land.
- **Used by:** `services`, the export and provider modules, `app`.
- **Status:** stub.

## Boundaries

- MUST NOT depend on `egui` or `wgpu`.
- MUST NOT mutate the project model directly — read into and write out of `core`
  types; mutation on load is a command in `core`/`services`.
- The format is versioned and must preserve unknown extension data on load
  (bible section 18.5), so a future module's data is not destroyed by an older app.
  Any compact binary-format crate (`rmp-serde`, `zstd`, `blake3`) is a candidate
  for when the save format lands, not adopted (bible section 33).
- Trace load, save, and format migration (`#[instrument]` on the load/save bodies);
  `warn!` on corrupt or unknown-extension data rather than failing silently. See the
  `pixhaus-tracing` skill.

Global rules: root `CLAUDE.md`. Architecture: `docs/pixhaus_architecture_bible.md`.
