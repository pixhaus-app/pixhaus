# Pixhaus Unity package

The first-party Unity importer and runtime helpers for Pixhaus exports.

Status: skeleton. The importer lands in stream S39 (see `../docs/planning/work/streams.md`).

## Install

When the package is published to OpenUPM:

```bash
openupm add app.pixhaus.unity
```

Manual install: add the following to `Packages/manifest.json` under `dependencies`:

```json
"app.pixhaus.unity": "https://github.com/pixhaus/pixhaus.git?path=/unity"
```

## Layout

- `Editor/` — importers and editor tooling
- `Runtime/` — runtime helpers consumed by player builds
- `Samples~/` — example projects users can copy into their own assets

## Minimum Unity version

2022.3 LTS. Primary target is Unity 6.0+.
