#!/usr/bin/env bash
# gen-updater-key.sh
#
# Generates the minisign keypair used to sign Pixhaus release artifacts
# for tauri-plugin-updater.
#
# Run this ONCE. Keep the private key in a password manager and add it
# to the repository's GitHub Secrets. Commit the public key into
# app/tauri.conf.json.
#
# Prerequisites: the Tauri CLI (installed via pnpm tauri / cargo tauri).
#
# Usage:
#   bash scripts/gen-updater-key.sh
#   bash scripts/gen-updater-key.sh --force   # overwrite existing key
#
# After running:
#   1. Copy the PUBLIC KEY output into app/tauri.conf.json →
#      plugins.updater.pubkey (replacing the REPLACE_WITH_... placeholder).
#   2. Add the PRIVATE KEY to GitHub Secrets as TAURI_SIGNING_PRIVATE_KEY.
#   3. If you set a password, add it as TAURI_SIGNING_PRIVATE_KEY_PASSWORD.
#   4. Commit the updated tauri.conf.json.

set -euo pipefail

FORCE=false
for arg in "$@"; do
  [[ "$arg" == "--force" ]] && FORCE=true
done

KEY_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/pixhaus"
KEY_PATH="$KEY_DIR/updater-key"

if [[ -f "$KEY_PATH" ]] && [[ "$FORCE" != "true" ]]; then
  echo "Key already exists at $KEY_PATH"
  echo "Use --force to regenerate (old signatures will stop verifying)."
  exit 1
fi

mkdir -p "$KEY_DIR"

echo "Generating Pixhaus updater signing key..."
echo "You will be prompted for an optional password to protect the private key."
echo "(Leave blank for no password — CI environments must then omit"
echo " TAURI_SIGNING_PRIVATE_KEY_PASSWORD from secrets.)"
echo ""

# tauri signer generate writes:
#   <path>          — private key (minisign format)
#   <path>.pub      — public key
pnpm tauri signer generate -w "$KEY_PATH"

echo ""
echo "================================================================"
echo "Keys written to:"
echo "  Private: $KEY_PATH"
echo "  Public:  ${KEY_PATH}.pub"
echo ""
echo "PUBLIC KEY (paste into app/tauri.conf.json → plugins.updater.pubkey):"
echo "----------------------------------------------------------------"
cat "${KEY_PATH}.pub"
echo "----------------------------------------------------------------"
echo ""
echo "PRIVATE KEY (add to GitHub Secret TAURI_SIGNING_PRIVATE_KEY):"
echo "----------------------------------------------------------------"
cat "$KEY_PATH"
echo "----------------------------------------------------------------"
echo ""
echo "Do NOT commit the private key file. Store it securely."
