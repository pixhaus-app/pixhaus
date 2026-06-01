# pixhaus-mod-generation

The generation module — the AI-forward Generate workspace (architecture bible
sections 7.3, 6.5, 14).

- **Registers:** the Generate workspace, the prompt composer and recipe library,
  templates/structures/styles, the generation result and coverage panels, the AI
  job types, and the generated-asset type.
- **Status:** stub.

## Boundaries

- AI proposes; the artist decides. Generation produces results; applying a result
  is a command. AI output never mutates the canvas directly.
- This module defines the AI job types and the generated-asset type, but
  provider-specific logic and settings live in `mod-providers`, not here. Ask for
  capabilities, not specific providers.
- Generated results carry their metadata (prompt, recipe, provider, seed, source
  context) so results are reproducible and traceable.

Shared module rules: `modules/CLAUDE.md`. Global rules: root `CLAUDE.md`.
