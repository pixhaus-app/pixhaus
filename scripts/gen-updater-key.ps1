# gen-updater-key.ps1
#
# Generates the minisign keypair used to sign Pixhaus release artifacts
# for tauri-plugin-updater. Windows variant of gen-updater-key.sh.
#
# Run this ONCE. Keep the private key in a password manager and add it
# to the repository's GitHub Secrets. Commit the public key into
# app/tauri.conf.json.
#
# Prerequisites: the Tauri CLI (installed via pnpm tauri / cargo tauri).
#
# Usage:
#   .\scripts\gen-updater-key.ps1
#   .\scripts\gen-updater-key.ps1 -Force   # overwrite existing key
#
# After running:
#   1. Copy the PUBLIC KEY output into app/tauri.conf.json ->
#      plugins.updater.pubkey (replacing the REPLACE_WITH_... placeholder).
#   2. Add the PRIVATE KEY to GitHub Secrets as TAURI_SIGNING_PRIVATE_KEY.
#   3. If you set a password, add it as TAURI_SIGNING_PRIVATE_KEY_PASSWORD.
#   4. Commit the updated tauri.conf.json.

param([switch]$Force)

$KeyDir = Join-Path $env:LOCALAPPDATA "pixhaus"
$KeyPath = Join-Path $KeyDir "updater-key"

if ((Test-Path $KeyPath) -and -not $Force) {
    Write-Host "Key already exists at $KeyPath"
    Write-Host "Use -Force to regenerate (old signatures will stop verifying)."
    exit 1
}

New-Item -ItemType Directory -Force -Path $KeyDir | Out-Null

Write-Host "Generating Pixhaus updater signing key..."
Write-Host "You will be prompted for an optional password to protect the private key."
Write-Host "(Leave blank for no password -- CI environments must then omit"
Write-Host " TAURI_SIGNING_PRIVATE_KEY_PASSWORD from secrets.)"
Write-Host ""

# tauri signer generate writes:
#   <path>      -- private key (minisign format)
#   <path>.pub  -- public key
pnpm tauri signer generate -w $KeyPath

Write-Host ""
Write-Host "================================================================"
Write-Host "Keys written to:"
Write-Host "  Private: $KeyPath"
Write-Host "  Public:  ${KeyPath}.pub"
Write-Host ""
Write-Host "PUBLIC KEY (paste into app/tauri.conf.json -> plugins.updater.pubkey):"
Write-Host "----------------------------------------------------------------"
Get-Content "${KeyPath}.pub"
Write-Host "----------------------------------------------------------------"
Write-Host ""
Write-Host "PRIVATE KEY (add to GitHub Secret TAURI_SIGNING_PRIVATE_KEY):"
Write-Host "----------------------------------------------------------------"
Get-Content $KeyPath
Write-Host "----------------------------------------------------------------"
Write-Host ""
Write-Host "Do NOT commit the private key file. Store it securely."
