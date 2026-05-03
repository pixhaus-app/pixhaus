# Security policy

## Supported versions

Pixhaus is in pre-1.0 development. Security fixes target the latest `main`
branch. Once a 1.0 ships, this policy will expand to cover the most recent
minor releases.

## Reporting a vulnerability

Email **luis@agsense.es** with:

- A description of the issue and its impact
- Steps to reproduce (proof-of-concept welcome)
- Affected versions or commits
- Your name and any acknowledgement preference

Do **not** open a public GitHub issue for security reports. Public issues are
appropriate after a fix has shipped and a coordinated disclosure window has
elapsed.

## Response timeline

- **Initial acknowledgement:** within 72 hours.
- **Triage and assessment:** within 7 days.
- **Fix or mitigation plan:** communicated within 14 days of triage.
- **Coordinated disclosure:** typically 30–90 days from initial report,
  negotiated with the reporter based on severity and complexity.

## Scope

In scope:

- The Pixhaus desktop application (the Tauri shell, IPC commands, file
  format readers and writers)
- The Rust workspace crates (`core`, `io`, `ai`, `scripting`, `app`)
- The Pixhaus Unity package
- Repo infrastructure (CI workflows, hooks, build scripts) where a flaw
  could compromise downstream consumers

Out of scope:

- Vulnerabilities in third-party dependencies — report those upstream and
  link the upstream advisory in your report so we can track it.
- Self-XSS or social-engineering issues that require user-supplied scripts
  to execute (Pixhaus runs Lua scripts users opt into).
- Issues in user-installed AI backends or third-party plugins.

## Acknowledgements

Reporters who follow this policy are credited in the release notes for the
fix unless they request otherwise.
