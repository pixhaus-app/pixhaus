# Install the agent-side dev tools Pixhaus expects.
# Idempotent - re-runs are cheap once everything is installed.

$ErrorActionPreference = 'Continue'

function Has-Command([string]$name) {
    $null -ne (Get-Command $name -ErrorAction SilentlyContinue)
}

Write-Output '==> Installing cargo dev tools'

$cargoTools = @(
    'cargo-nextest',
    'cargo-deny',
    'cargo-audit',
    'cargo-machete',
    'cargo-watch',
    'typos-cli',
    'bacon'
)

foreach ($tool in $cargoTools) {
    Write-Output "  - $tool"
    & cargo install --locked $tool
    if ($LASTEXITCODE -ne 0) {
        Write-Output "    (skipped: $tool already installed or unavailable)"
    }
}

Write-Output '==> Installing Tauri CLI'
& cargo install --locked tauri-cli --version '^2.0.0'
if ($LASTEXITCODE -ne 0) {
    Write-Output '  (skipped: tauri-cli already installed or unavailable)'
}

Write-Output '==> Ensuring pnpm is available'
if (-not (Has-Command 'pnpm')) {
    if (Has-Command 'corepack') {
        Write-Output '  - enabling pnpm via corepack'
        & corepack enable pnpm
        & corepack prepare pnpm@latest --activate
    }
    elseif (Has-Command 'npm') {
        Write-Output '  - installing pnpm via npm'
        & npm install -g pnpm
    }
    else {
        Write-Output '  ! neither corepack nor npm found; install Node 20+ then re-run'
        exit 1
    }
}

Write-Output '==> Done.'
