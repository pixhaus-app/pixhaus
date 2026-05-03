# Bootstrap: idempotent first-time setup. Re-runnable any time.
#
# Steps: install dev tools, wire git hooks, fetch deps, build the ui/dist
# stub for tauri::generate_context!, verify the workspace compiles, run
# typecheck and tests, end with doctor.
#
# Exits non-zero on the first failing step with a fix hint.
#
# Mirror of scripts/bootstrap.sh.

$ErrorActionPreference = 'Continue'

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
Set-Location -LiteralPath $repoRoot

function Step([string]$label) { Write-Output ''; Write-Output "==> $label" }
function Abort([string]$step, [string]$hint) {
    Write-Output ''
    [Console]::Error.WriteLine("bootstrap failed at: $step")
    [Console]::Error.WriteLine("fix: $hint")
    exit 1
}

$runMjs = Join-Path $PSScriptRoot 'run.mjs'

Step '1/9  install dev tools'
& node $runMjs install-tools
if ($LASTEXITCODE -ne 0) { Abort 'install-tools' 'see scripts/install-tools.ps1 output above; rerun bootstrap once fixed' }

Step '2/9  wire git hooks'
& node $runMjs setup-git-hooks
if ($LASTEXITCODE -ne 0) { Abort 'setup-git-hooks' 'rerun: node scripts/run.mjs setup-git-hooks' }

Step '3/9  pnpm install'
& pnpm install
if ($LASTEXITCODE -ne 0) { Abort 'pnpm install' 'check pnpm-lock.yaml and the network; pnpm install --force may help' }

Step '4/9  cargo fetch'
& cargo fetch
if ($LASTEXITCODE -ne 0) { Abort 'cargo fetch' 'check Cargo.lock and the network' }

Step '5/9  ui/dist stub'
if (-not (Test-Path -LiteralPath 'ui/dist/index.html' -PathType Leaf)) {
    New-Item -ItemType Directory -Path 'ui/dist' -Force | Out-Null
    $utf8 = New-Object System.Text.UTF8Encoding($false)
    [System.IO.File]::WriteAllText((Join-Path $repoRoot 'ui/dist/index.html'), '<!doctype html><title>stub</title>', $utf8)
    Write-Output '    wrote ui/dist/index.html (placeholder for tauri::generate_context!)'
}
else {
    Write-Output '    ui/dist/index.html already present; skipping'
}

Step '6/9  cargo check --workspace --all-targets'
& cargo check --workspace --all-targets
if ($LASTEXITCODE -ne 0) { Abort 'cargo check' 'fix the compile errors above' }

Step '7/9  pnpm typecheck'
& pnpm typecheck
if ($LASTEXITCODE -ne 0) { Abort 'pnpm typecheck' 'fix the TypeScript errors above' }

Step '8/9  pnpm test'
& pnpm test
if ($LASTEXITCODE -ne 0) { Abort 'pnpm test' 'fix the failing test(s) above' }

Step '9/9  doctor'
& node $runMjs doctor
exit $LASTEXITCODE
