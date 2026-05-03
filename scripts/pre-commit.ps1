# Pre-commit: gate formatting, lints, typos.
# Failures here block the commit. Fix the underlying issue rather than
# bypassing with --no-verify.
#
# Mirror of scripts/pre-commit.sh.

$ErrorActionPreference = 'Continue'

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot '..')
Set-Location -LiteralPath $repoRoot

function Has-Command([string]$name) {
    $null -ne (Get-Command $name -ErrorAction SilentlyContinue)
}

function Fail([string]$msg) {
    Write-Output ''
    [Console]::Error.WriteLine("pre-commit: $msg")
    [Console]::Error.WriteLine('pre-commit: blocked. Fix the issue above and try again.')
    exit 1
}

if (Has-Command 'cargo') {
    Write-Output 'pre-commit: cargo fmt --check'
    & cargo fmt --all -- --check
    if ($LASTEXITCODE -ne 0) { Fail 'rustfmt check failed (run: cargo fmt --all)' }

    Write-Output 'pre-commit: cargo clippy'
    & cargo clippy --workspace --all-targets -- -D warnings
    if ($LASTEXITCODE -ne 0) { Fail 'clippy reported warnings' }
}
else {
    [Console]::Error.WriteLine('pre-commit: cargo not on PATH; skipping Rust checks')
}

if (Has-Command 'typos') {
    Write-Output 'pre-commit: typos'
    & typos
    if ($LASTEXITCODE -ne 0) { Fail 'typos found (run: typos --write-changes)' }
}
else {
    [Console]::Error.WriteLine('pre-commit: typos not installed; skipping')
}

if ((Has-Command 'pnpm') -and (Test-Path -LiteralPath 'ui' -PathType Container)) {
    Write-Output 'pre-commit: prettier --check (ui)'
    Push-Location ui
    try {
        & pnpm prettier --check 'src/**/*.{ts,tsx,css,json}'
        if ($LASTEXITCODE -ne 0) {
            Pop-Location
            Fail 'prettier check failed (run: cd ui && pnpm format)'
        }

        Write-Output 'pre-commit: eslint (ui)'
        & pnpm eslint 'src/**/*.{ts,tsx}'
        if ($LASTEXITCODE -ne 0) {
            Pop-Location
            Fail 'eslint reported errors'
        }
    }
    finally {
        if ((Get-Location).Path -ne $repoRoot.Path) { Pop-Location }
    }
}
else {
    [Console]::Error.WriteLine('pre-commit: pnpm not on PATH or ui/ missing; skipping TS checks')
}

Write-Output 'pre-commit: ok'
