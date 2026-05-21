# Pre-PR gate (PowerShell). See pre-pr.sh for the canonical version.
# Heavier than pre-commit - adds the full test suite and build.

[CmdletBinding()]
param()

$ErrorActionPreference = 'Continue'
$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
Set-Location $RepoRoot

$script:Failed = $false
function Section($title) { Write-Host "`n==> $title" }
function Fail($msg)      { $script:Failed = $true; Write-Error "    FAILED: $msg" }

function Has-Command($name) {
    $null -ne (Get-Command $name -ErrorAction SilentlyContinue)
}

if (Has-Command cargo) {
    Section "cargo fmt --check"
    cargo fmt --all -- --check
    if ($LASTEXITCODE -ne 0) { Fail "rustfmt (run: cargo fmt --all)" }

    Section "cargo clippy --all-targets -D warnings"
    cargo clippy --workspace --all-targets -- -D warnings
    if ($LASTEXITCODE -ne 0) { Fail "clippy (fix warnings or annotate)" }

    Section "cargo test --workspace"
    if (Has-Command cargo-nextest) {
        cargo nextest run --workspace --no-fail-fast
    } else {
        cargo test --workspace --no-fail-fast
    }
    if ($LASTEXITCODE -ne 0) { Fail "tests" }

    Section "cargo doc --no-deps -D warnings"
    $env:RUSTDOCFLAGS = "-D warnings"
    cargo doc --workspace --no-deps --document-private-items
    if ($LASTEXITCODE -ne 0) { Fail "rustdoc (broken intra-doc link or missing docs)" }
    Remove-Item Env:\RUSTDOCFLAGS -ErrorAction SilentlyContinue

    if (Has-Command cargo-deny) {
        Section "cargo deny check"
        cargo deny check --config .cargo/deny.toml
        if ($LASTEXITCODE -ne 0) { Fail "cargo-deny (dependency, license, or advisory violation)" }
    } else {
        Write-Warning "pre-pr: cargo-deny not installed; install with cargo install cargo-deny"
    }
} else {
    Write-Warning "pre-pr: cargo not on PATH; Rust checks skipped"
    $script:Failed = $true
}

if (Has-Command typos) {
    Section "typos"
    typos
    if ($LASTEXITCODE -ne 0) { Fail "typos (run: typos --write-changes)" }
} else {
    Write-Warning "pre-pr: typos not installed; install with cargo install typos-cli"
}

if ((Has-Command pnpm) -and (Test-Path ui)) {
    Section "pnpm typecheck"; pnpm typecheck
    if ($LASTEXITCODE -ne 0) { Fail "pnpm typecheck" }

    Section "pnpm lint"; pnpm lint
    if ($LASTEXITCODE -ne 0) { Fail "pnpm lint" }

    Section "pnpm format:check"; pnpm format:check
    if ($LASTEXITCODE -ne 0) { Fail "pnpm format:check (run: pnpm format)" }

    Section "pnpm test"; pnpm test
    if ($LASTEXITCODE -ne 0) { Fail "pnpm test" }

    Section "pnpm ui:build"; pnpm ui:build
    if ($LASTEXITCODE -ne 0) { Fail "pnpm ui:build" }
} else {
    Write-Warning "pre-pr: pnpm not on PATH or ui/ missing; UI checks skipped"
    $script:Failed = $true
}

Write-Host ""
if (-not $script:Failed) {
    Write-Host "pre-pr: ok - ready to open the PR"
    exit 0
} else {
    Write-Error "pre-pr: failures above; fix them before opening the PR"
    exit 1
}
