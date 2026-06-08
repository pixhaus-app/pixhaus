## mod-export-tiles

Both modules clear the bar as disciplined registration shells: they wire capabilities without reimplementing codecs, reference shared sprite-edit panels and tools by id, route every mutation through `Intent::RunAction` over read-only `ContribCtx`, key all real labels through `MsgKey`, and lean entirely on theme tokens and phosphor icons with no hex literals, emoji, or `unsafe`. Compliance is high. The one material defect is a doubled-header bug across all four export dock panels — the exact anti-pattern the ui-conventions skill names by name — plus a missing registration `info!` landmark in tiles and nine prose `TODO(luis)` comments the rust-conventions rule bans.

### Strengths

- Capability-wiring discipline is exemplary: export keeps all codec logic in `io` and only registers the workspace, presets, and validators; both modules reference sprite-edit's shared Console/Assets panels and Hand/Zoom/TOOL_RAIL tools by id, with explicit comments stating they are never re-registered.
- Deferred-intent model is followed exactly: every panel is a `&self` unit struct, reads only through `scope.ctx`, and pushes `Intent::RunAction` for all mutation — no `RefCell`/`Cell`/`Mutex` smuggling, no direct model mutation.
- i18n is correct where it counts: real labels are `MsgKey` constants in the right namespaces (`workspace.*`, `panel.*`, `command.*`) with values present in `export.yaml`/`tiles.yaml`, and in-body buttons reuse the same `command.*` keys as the registered actions so wording can never fork.
- `StatusItem` usage cleanly separates keyed labels (`StatusItem::key` with a `MsgKey`) from documented mock-data placeholders (`StatusItem::data`), with comments recording why the mock count stays verbatim until the validator feeds a real keyed count.
- Design-system compliance is total: all color, spacing, and radius come from theme tokens, AI affordances use `icons::SPARKLE` with `theme.accent.ai`, and there is no `Color32::from_rgb` or emoji in either file.
- Tests cover the registration surface meaningfully — layout inventory, workspace meta and shortcut, panel ids and default regions — and the cast `#[allow]` attributes on `tile_grid`/`seamless_preview` are narrowly scoped with a comment justifying that the bounded constants cannot truncate.

### Findings

| ID | File:Lines | Severity | Category | Issue -> Fix |
|----|------------|----------|----------|--------------|
| U30-1 | modules/export/src/export_ws.rs:109, 142, 179, 221 | medium | ui-widgets | All four export dock panels call `widgets::section_header` with their own panel meta title key, but the shell already wraps each right-dock body in `widgets::card` (`right_dock.rs:40`) and `card` draws `meta.title.tr()` (`card.rs:37`) — the doubled-header bug ui-conventions rule 2 names. Fix: remove the four `section_header(... panel title ...)` calls, start each body with its real content, and delete the stale "section header mirrors the panel title" comments; if an in-body divider is later needed, give it its own sub-key, never the panel's own title key. |
| U30-2 | modules/tiles/src/tiles_ws.rs:342-368 | low | tracing | `tiles_ws::register` wires the workspace, six panels, and five actions but emits no tracing event, violating the modules/CLAUDE.md and tiles/CLAUDE.md rule that module registration carries an `info!` landmark (the pixhaus-tracing always-trace flow). Sibling `export_ws.rs:326` does this; tiles is the lone gap. Fix: add `tracing::info!(module = "tiles", "registered the Tiles workspace");` as the last line of `register()`, mirroring export. |
| U30-3 | modules/export/src/export_ws.rs:110, 143, 180, 222, 244, 282 | low | docs | Six prose `TODO(luis)` comments mark mock rows for future i18n; the pixhaus-rust-conventions Comments rule bans prose `// TODO` in the tree in favor of an issue with at most a `// see #NNN` breadcrumb, and none of these reference an issue. Fix: replace each with an issue reference, or drop the prose and rely on the existing "throwaway mock" rationale comments. |
| U30-4 | modules/tiles/src/tiles_ws.rs:167, 196, 225 | low | docs | Three bare prose `TODO(luis)` comments in the tiles mock panels reference no issue — same pixhaus-rust-conventions violation as U30-3, and terser (no rationale attached). Fix: convert to a `// see #NNN` issue reference or remove, consistent with U30-3. |

### Checked and cleared (false positives)

None.
