# Install tauri-driver and msedgedriver needed for the Pixhaus end-to-end
# suite (tests/e2e). Idempotent.
#
# Requires Microsoft Edge Driver matching the local Edge browser version;
# version mismatches cause WebDriver sessions to hang. The chippers tool
# below handles version matching automatically.

$ErrorActionPreference = 'Continue'

function Has-Command([string]$name) {
    $null -ne (Get-Command $name -ErrorAction SilentlyContinue)
}

Write-Output '==> Installing tauri-driver'
& cargo install --locked tauri-driver
if ($LASTEXITCODE -ne 0) {
    Write-Output '    (skipped: tauri-driver already installed or unavailable)'
}

Write-Output '==> Checking for msedgedriver'
if (-not (Has-Command 'msedgedriver')) {
    Write-Output '    msedgedriver not found in PATH.'
    Write-Output '    Install the matching version with:'
    Write-Output '        cargo install --git https://github.com/chippers/msedgedriver-tool'
    Write-Output '        & "$HOME/.cargo/bin/msedgedriver-tool.exe"'
    Write-Output '    Then add msedgedriver.exe to PATH (or place it next to tauri-driver).'
    Write-Output ''
    Write-Output '    Alternatively, download manually from:'
    Write-Output '        https://developer.microsoft.com/en-us/microsoft-edge/tools/webdriver/'
    exit 1
}
$mePath = (Get-Command msedgedriver).Source
Write-Output "    msedgedriver: $mePath"

Write-Output '==> Done. Build the binary next:'
Write-Output '        pnpm tauri:build:debug'
Write-Output '    Then run:'
Write-Output '        pnpm e2e'
