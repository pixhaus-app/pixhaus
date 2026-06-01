# pixhaus-mod-core

The host bootstrap module (architecture bible section 7.3). The foundational
module the others assume is present.

- **Registers:** project lifecycle, settings, the command and job systems, the
  event bus, and the asset and workspace registries.
- **Status:** stub.

## Boundaries

- Bootstraps the host, but still through registries — no back doors around the
  registry/command boundary.
- No editor UI here beyond wiring registries; feature panels and tools belong to
  the feature modules.
- Other modules may assume the registries this one sets up exist; it must not
  assume any feature module is present.

Shared module rules: `modules/CLAUDE.md`. Global rules: root `CLAUDE.md`.
