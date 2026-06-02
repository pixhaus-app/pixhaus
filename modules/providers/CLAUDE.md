# pixhaus-mod-providers

The provider modules — AI and compute backends (architecture bible sections 7.3,
14.2).

- **Registers:** AI/compute providers behind the capability-based provider
  abstraction — a mock provider for offline development, plus remote and
  local-model providers.
- **Status:** stub.

## Boundaries

- The app asks for capabilities ("who can generate a sprite from text?"), not
  specific providers. Provider-specific settings and logic stay here, never in the
  Generate workspace.
- A failing, unavailable, or missing provider must not crash the app — provider
  failures are isolated and surfaced as actionable errors.
- Ship the mock provider so UI and generation flows work without API keys, model
  downloads, or a GPU. Local models run out-of-process where practical.
- Providers run in the AI/model-worker lane, out-of-process where practical, under
  the background-worker contract (bible sections 31.2 and 13.6).
- A span per AI request with the provider-response duration; `error!` on a provider
  failure (isolated, surfaced as an actionable error). NEVER log an API key — it
  lives in the OS credential vault, not the log. See the `pixhaus-tracing` and
  `pixhaus-keyring` skills.
- Register provider labels and capability descriptions with keys
  (`provider.<id>.label`); ship its bundle when it gains UI. Provider-returned text,
  API model names, and API keys are DATA, never i18n keys — surface provider errors
  via keyed error strings. See the `pixhaus-i18n` skill.

Reach for `pixhaus-reqwest`/`pixhaus-keyring` skills when wiring real providers.
Shared module rules: `modules/CLAUDE.md`. Global rules: root `CLAUDE.md`.
