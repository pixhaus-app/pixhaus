#!/usr/bin/env bash
# Install tauri-driver and the platform WebDriver server needed for the
# Pixhaus end-to-end suite (tests/e2e). Idempotent.
#
# Linux: requires `webkit2gtk-driver` from your package manager.
# macOS: not supported — tauri-driver can't drive WKWebView yet.
# Windows: see scripts/setup-e2e.ps1; this script is *nix only.

set -euo pipefail

echo "==> Installing tauri-driver"
cargo install --locked tauri-driver \
    || echo "    (skipped: tauri-driver already installed or unavailable)"

case "$(uname -s)" in
    Darwin)
        echo "!  macOS is not supported by tauri-driver."
        echo "   See https://v2.tauri.app/develop/tests/webdriver/"
        exit 1
        ;;
    Linux)
        echo "==> Checking for WebKitWebDriver"
        if ! command -v WebKitWebDriver >/dev/null 2>&1; then
            echo "    WebKitWebDriver not found in PATH."
            if command -v apt-get >/dev/null 2>&1; then
                echo "    On Debian/Ubuntu install with:"
                echo "        sudo apt-get install -y webkit2gtk-driver"
            elif command -v dnf >/dev/null 2>&1; then
                echo "    On Fedora install with:"
                echo "        sudo dnf install -y webkit2gtk4.1-devel"
            elif command -v pacman >/dev/null 2>&1; then
                echo "    On Arch install with:"
                echo "        sudo pacman -S webkit2gtk"
            else
                echo "    Use your distro's package manager to install webkit2gtk-driver."
            fi
            exit 1
        fi
        echo "    WebKitWebDriver: $(command -v WebKitWebDriver)"
        ;;
    *)
        echo "!  Unrecognised OS: $(uname -s). Skipping platform driver check."
        ;;
esac

echo "==> Done. Build the binary next:"
echo "        pnpm tauri:build:debug"
echo "    Then run:"
echo "        pnpm e2e"
