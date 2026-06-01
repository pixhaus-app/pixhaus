# modules/ — internal capability modules

Each crate here registers capabilities with the host — workspaces, panels, tools,
commands, asset types, providers, importers/exporters, validators (architecture
bible section 7). They behave like plugins architecturally but are compiled in,
not dynamically loaded. Pixhaus deliberately does not build a native dynamic
plugin system.

## Rules for every module in this tree

- MUST NOT own or mutate `core` data except through registered commands.
- MUST NOT create hidden global state — state belongs to a service or a registry.
- MUST NOT assume it is the only contributor to a workspace; compose, don't
  monopolize.
- Must be disable-able in dev/test builds.
- Depend on `core` + `services` + `ui` (and `io`/`render` where genuinely needed);
  never depend on `app/`, and never on another module unless the layering is
  deliberate and one-directional.
- Keep provider-, format-, and OS-specific logic in the crate that owns it
  (`providers` module, `io`, `platform`) — a feature module wires capabilities up,
  it doesn't reimplement them.

Per the bible's agent contracts (section 25.3): an agent working in one module
stays in that module's lane. Global conventions live in the root `CLAUDE.md`; the
architecture is in `docs/pixhaus_architecture_bible.md`.
