# pixhaus-mod-core

The host bootstrap module (architecture bible section 7.3). The foundational
module the others assume is present.

- **Registers:** project lifecycle, settings, the command and job systems, the
  event bus, and the asset and workspace registries — these are the
  `CommandBus`/`JobManager`/registry seams of the bible's target container set
  (section 22.7).
- **Status:** stub.

## Boundaries

- Bootstraps the host, but still through registries — no back doors around the
  registry/command boundary.
- No editor UI here beyond wiring registries; feature panels and tools belong to
  the feature modules.
- Other modules may assume the registries this one sets up exist; it must not
  assume any feature module is present.
- Trace the host bootstrap: an `info!` as the command and job systems and the asset
  and workspace registries register. See the `pixhaus-tracing` skill.
- This module wires the localization service into the host alongside the command and
  job systems; it does not own translations or call `tr()` itself. See the
  `pixhaus-i18n` skill.

- Record the why: when a choice here is made for a non-obvious reason — a
  trade-off, a rejected alternative, a constraint, or a workaround — state that
  reason in a `//` comment at each spot it shaped, not just in the commit. See the
  root `CLAUDE.md` "Recording decisions" rule.

Shared module rules: `modules/CLAUDE.md`. Global rules: root `CLAUDE.md`.
