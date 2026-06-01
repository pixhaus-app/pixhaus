# pixhaus-platform

The platform layer — the OS-facing edges (architecture bible section 4.6).

- **Owns:** native dialogs, clipboard, recent-files tracking, OS settings paths,
  GPU capability detection, and external-process supervision (e.g. local model
  workers).
- **Depends on:** `core`. External: `rfd`, `arboard`, and similar as they land.
- **Used by:** `app`, and `services` where it needs OS facilities.
- **Status:** stub.

## Boundaries

- MUST NOT depend on `egui`.
- MUST NOT block the UI thread — native file/message dialogs use the async
  variants (see the `pixhaus-rfd` skill), not the blocking ones.
- Keep OS-specific `cfg` and quirks contained here; the rest of the workspace
  stays platform-agnostic.

Global rules: root `CLAUDE.md`. Architecture: `docs/pixhaus_architecture_bible.md`.
