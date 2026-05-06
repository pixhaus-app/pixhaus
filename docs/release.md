# Release process

Pixhaus follows a tag-driven release model. A `v*` git tag triggers CI to
build signed native installers for all platforms, generate the auto-update
manifest, and publish a GitHub Release.

## One-time setup

These steps are required before the first release. Do them once.

### 1. Generate the updater signing key

The updater keypair lets the installed app verify that a downloaded update
actually came from this project.

**Unix:**
```bash
bash scripts/gen-updater-key.sh
```

**Windows:**
```powershell
.\scripts\gen-updater-key.ps1
```

The script wraps `pnpm tauri signer generate` and prints the public and
private keys when done.

**After the script runs:**

1. Copy the public key (the full contents of the `.pub` file) into
   `app/tauri.conf.json` → `plugins.updater.pubkey`, replacing the
   `REPLACE_WITH_OUTPUT_OF_scripts/gen-updater-key` placeholder.
2. Commit `app/tauri.conf.json`.
3. Add the private key to GitHub repository secrets as
   `TAURI_SIGNING_PRIVATE_KEY`. If you chose a password, add it as
   `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`.

The private key file stays off disk (or in a password manager). Never
commit it.

### 2. Add platform code-signing secrets

Code signing is optional for development builds but required for production
releases. Without it, users see "unidentified developer" warnings.

#### macOS (Apple Developer)

| Secret | Value |
|---|---|
| `APPLE_CERTIFICATE` | Base64-encoded `.p12` exported from Keychain (Developer ID Application cert) |
| `APPLE_CERTIFICATE_PASSWORD` | Password for the `.p12` |
| `APPLE_SIGNING_IDENTITY` | Cert common name, e.g. `Developer ID Application: Pixhaus (TEAMID)` |
| `APPLE_ID` | Apple ID email used for notarization |
| `APPLE_PASSWORD` | App-specific password generated at appleid.apple.com |
| `APPLE_TEAM_ID` | 10-character Apple Developer Team ID |

Export the certificate:
```bash
# In Keychain Access: right-click "Developer ID Application: ..." → Export
# Choose .p12 format, set a strong password.
base64 -i DeveloperIDApplication.p12 | pbcopy
# Paste into APPLE_CERTIFICATE secret.
```

#### Windows (Authenticode)

| Secret | Value |
|---|---|
| `WINDOWS_CERTIFICATE` | Base64-encoded `.pfx` (code-signing certificate) |
| `WINDOWS_CERTIFICATE_PASSWORD` | Password for the `.pfx` |

Obtain a certificate from DigiCert, Sectigo, or a similar CA. Encode:
```powershell
[Convert]::ToBase64String([IO.File]::ReadAllBytes("cert.pfx")) | Set-Clipboard
```

#### Linux (optional GPG)

| Secret | Value |
|---|---|
| `GPG_PRIVATE_KEY` | ASCII-armored GPG private key (`gpg --export-secret-keys --armor`) |
| `GPG_KEY_ID` | Key fingerprint or email matching the secret |

GPG signatures are uploaded alongside the AppImage as `.asc` files for
users who verify downloads. The AppImage itself is not code-signed (no
Linux equivalent of Authenticode/GateKeeper exists for general use).

### 3. Verify CI reads secrets

After adding secrets, trigger a `workflow_dispatch` release with a
pre-release tag (e.g. `v0.1.0-rc.1`) and verify:

- macOS DMGs are signed and notarized (check with `spctl --assess`)
- Windows MSIs show the publisher name in the installer
- `latest.json` is present on the release with all four platform entries
- The installed app reports the correct version in About

## Release checklist

Follow this order every time you ship a release.

### Step 1: Prepare the changelog

Edit `CHANGELOG.md` and add a section for the new version:

```markdown
## [0.2.0] - 2026-06-01

### Added
- Brush engine with pencil, eraser, fill, line tools (S15)

### Fixed
- Layer rename now commits on blur, not only on Enter (S17-followup)
```

The release workflow extracts the matching section to populate the GitHub
Release body. The format must match: `## [VERSION]` or `## [VERSION] - DATE`.

### Step 2: Bump the version

Version lives in two places that must stay in sync:

```toml
# Cargo.toml (workspace root)
[workspace.package]
version = "0.2.0"
```

```json
// app/tauri.conf.json
{
  "version": "0.2.0"
}
```

Run the cargo lockfile update:
```bash
cargo check --workspace
```

Commit the version bump:
```bash
git add Cargo.toml Cargo.lock app/tauri.conf.json CHANGELOG.md
git commit -m "chore: bump version to 0.2.0"
git push origin main
```

### Step 3: Tag the release

```bash
git tag v0.2.0
git push origin v0.2.0
```

The `v*` push triggers the release workflow automatically.

### Step 4: Monitor CI

Watch the `Release` workflow on GitHub Actions. The job sequence is:

1. **create-release** — creates a draft GitHub Release with the changelog body.
2. **build (4 parallel jobs)** — builds and signs installers for Linux,
   Windows, macOS Intel, macOS Apple Silicon. Each job uploads its bundles
   to the draft release.
3. **publish** — downloads all artifacts, generates `latest.json`, and
   flips the release from draft to live.

If any build job fails, fix the issue, delete the tag and release, and
re-tag:
```bash
git tag -d v0.2.0
git push origin :refs/tags/v0.2.0
# fix the issue, then re-tag
```

### Step 5: Verify the published release

After CI finishes:

1. Open the GitHub Release page and check that all installers are present.
2. Download the installer for your platform and verify it runs.
3. Check that `latest.json` is present and contains all four platform entries.
4. Install the previous version on a test machine and verify the auto-update
   prompt appears within a few minutes of launching the app.

### Step 6: Announce

Post to the community channels (Discord `#announcements`, GitHub
Discussions, social media). Link to the GitHub Release and the changelog
section. The release body in GitHub automatically becomes the announcement
body if you copy it.

## Auto-update flow

The auto-updater is powered by `tauri-plugin-updater`.

**Startup check:** on every launch, the app fetches
`https://github.com/pixhaus-app/pixhaus/releases/latest/download/latest.json`
in the background. If the manifest lists a version newer than the running
build, the `updater:available` event fires.

**UI response:** the frontend listens for `updater:available` and shows a
toast notification: "Pixhaus 0.2.0 is available. Install now?"

**Install:** clicking "Install now" calls the `updater_install` IPC command.
The backend downloads the signed bundle, emits `updater:progress` events
as bytes arrive, and calls `updater:ready` when installation is staged.
On macOS and Linux the app restarts automatically. On Windows with NSIS
the user may see a brief installer prompt.

**Verification:** `tauri-plugin-updater` verifies the downloaded bundle's
minisign signature against the public key in `app/tauri.conf.json` before
staging it. A tampered or unsigned bundle is rejected.

## Signing verification (local)

To verify a signed artifact manually:

```bash
# Install minisign
brew install minisign   # macOS
apt install minisign    # Ubuntu

# Extract the public key from tauri.conf.json and save it
# Then verify:
minisign -V -p pixhaus.pub \
  -m pixhaus_0.2.0_amd64.AppImage.tar.gz \
  -x pixhaus_0.2.0_amd64.AppImage.tar.gz.sig
```

## Rollback

The auto-updater does not ship a rollback mechanism. If a release is
broken:

1. Yank the GitHub Release (mark it as pre-release or delete it).
2. Push a patch release (e.g. `v0.2.1`) with the fix as quickly as
   possible.
3. Users who updated to the broken build must manually install the patch
   release installer until the auto-updater picks up `v0.2.1`.

## Secrets reference

| Secret | Used by | Required |
|---|---|---|
| `TAURI_SIGNING_PRIVATE_KEY` | All platforms (update artifact signing) | Yes |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | All platforms | If key has a password |
| `APPLE_CERTIFICATE` | macOS signing | For production macOS releases |
| `APPLE_CERTIFICATE_PASSWORD` | macOS signing | With APPLE_CERTIFICATE |
| `APPLE_SIGNING_IDENTITY` | macOS signing | With APPLE_CERTIFICATE |
| `APPLE_ID` | macOS notarization | With APPLE_CERTIFICATE |
| `APPLE_PASSWORD` | macOS notarization | With APPLE_ID |
| `APPLE_TEAM_ID` | macOS notarization | With APPLE_ID |
| `WINDOWS_CERTIFICATE` | Windows Authenticode | For production Windows releases |
| `WINDOWS_CERTIFICATE_PASSWORD` | Windows Authenticode | With WINDOWS_CERTIFICATE |
| `GPG_PRIVATE_KEY` | Linux AppImage GPG detached sig | Optional |
| `GPG_KEY_ID` | Linux AppImage GPG detached sig | With GPG_PRIVATE_KEY |
