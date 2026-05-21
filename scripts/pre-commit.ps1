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

    if (Has-Command 'cargo-deny') {
        Write-Output 'pre-commit: cargo deny check'
        & cargo deny check --config .cargo/deny.toml
        if ($LASTEXITCODE -ne 0) { Fail 'cargo-deny: dependency, license, or advisory violation' }
    }
    else {
        [Console]::Error.WriteLine('pre-commit: cargo-deny not installed; skipping (cargo install cargo-deny)')
    }
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

# Call the same scripts CI runs (pnpm -r format:check / lint) so the local
# gate covers the exact glob CI does - including tests/, which the old
# hand-rolled src/** globs missed and let unformatted test files reach CI.
if ((Has-Command 'pnpm') -and (Test-Path -LiteralPath 'ui' -PathType Container)) {
    Write-Output 'pre-commit: pnpm format:check'
    & pnpm format:check
    if ($LASTEXITCODE -ne 0) { Fail 'prettier check failed (run: pnpm format)' }

    Write-Output 'pre-commit: pnpm lint'
    & pnpm lint
    if ($LASTEXITCODE -ne 0) { Fail 'eslint reported errors' }
}
else {
    [Console]::Error.WriteLine('pre-commit: pnpm not on PATH or ui/ missing; skipping TS checks')
}

Write-Output 'pre-commit: ok'
