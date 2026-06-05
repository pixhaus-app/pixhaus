# pixhaus-platform

The platform layer — the OS-facing edges (architecture bible section 4.6).

- **Owns:** native dialogs, clipboard, recent-files tracking, OS settings paths,
  app directories (`app_dirs` / `log_dir`, via `directories` — the five buckets
  config / data / cache / logs / autosave per bible section 18.6), GPU capability
  detection, and external-process supervision (e.g. local model workers).
- **Depends on:** `core`. External: `directories`, `thiserror`, and `rfd`/`arboard`
  and similar as they land.
- **Used by:** `app`, and `services` where it needs OS facilities.
- **Status:** first capability landed — app directories.

## Boundaries

- MUST NOT depend on `egui`.
- MUST NOT block the UI thread — native file/message dialogs use the async
  variants (see the `pixhaus-rfd` skill), not the blocking ones.
- Keep OS-specific `cfg` and quirks contained here; the rest of the workspace
  stays platform-agnostic.
- This crate resolves the log dir (`log_dir`), but the subscriber that writes there
  lives in `app` (`src/diagnostics.rs`) — `platform` computes the path, `app`
  configures logging. `directories` only computes paths; `log_dir` creates the
  directory before returning it. See the `pixhaus-directories` and `pixhaus-tracing`
  skills.
- This crate detects the OS language (via `sys-locale`) for `app`'s boot-time
  default; it reads the OS locale, it does not translate, and it does not host the
  locale bundles — those are compiled into `services`. See the `pixhaus-i18n` skill.

- Record the why: when a choice here is made for a non-obvious reason — a
  trade-off, a rejected alternative, a constraint, or a workaround — state that
  reason in a `//` comment at each spot it shaped, not just in the commit. See the
  root `CLAUDE.md` "Recording decisions" rule.

Global rules: root `CLAUDE.md`. Architecture: `docs/pixhaus_architecture_bible.md`.
