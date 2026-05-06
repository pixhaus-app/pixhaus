# Privacy policy

Pixhaus is free, open-source software. It does not collect data, run
background services, or phone home by default.

## Crash reporting

Crash reporting is **off by default**. The first time Pixhaus starts, a
dialog asks whether you want to help improve the editor by sending
anonymous crash reports. You can change your choice at any time in
**Preferences → Privacy**.

### What is collected

When crash reporting is enabled, the following information is sent when
Pixhaus encounters an unexpected error or panic:

- Stack trace of the failure
- Operating system name and version
- Pixhaus version
- An anonymous stable identifier generated locally on first launch (a
  random UUID stored in browser `localStorage`; it does not identify
  you and is not linked to any account)

### What is never collected

- Project content (pixel data, layer names, frame data)
- File paths or file names of opened projects
- Palette contents
- Your hostname or IP address (hostname is stripped before the report
  leaves the process)
- Any credential, key, or token
- Any other personally identifiable information

### Where reports go

Reports are sent to a self-hosted [GlitchTip](https://glitchtip.com)
instance operated by the Pixhaus maintainers. GlitchTip is an
open-source, Sentry-compatible error tracking tool. Reports are retained
for 90 days, then deleted.

No report data is shared with third parties or sold.

### Opting out

Open **Preferences → Privacy** and uncheck "Send anonymous crash reports".
The change takes effect immediately. No data is sent after opting out.

To remove the local anonymous UID, clear `pixhaus:crash-reporting-uid`
from your browser's `localStorage` for the Pixhaus app origin (or reset
Pixhaus by clearing all stored preferences).

### For maintainers: infrastructure

The reporting endpoint is configured via build-time environment variables:

| Variable | Purpose |
|---|---|
| `PIXHAUS_SENTRY_DSN` | Sentry/GlitchTip DSN for the Rust process (compiled into the binary) |
| `VITE_SENTRY_DSN` | Same DSN for the JavaScript layer (injected by Vite at build time) |
| `VITE_APP_VERSION` | Release version string attached to reports (optional) |

Both variables must be set at build time for crash reporting to be active
in a release build. Builds without these variables (including all
developer builds and third-party forks) never send any data regardless of
the user preference.

To spin up a GlitchTip instance, follow the [GlitchTip self-hosting
guide](https://glitchtip.com/documentation/install). Create a project,
copy the DSN, and set the above variables in the release CI pipeline.
