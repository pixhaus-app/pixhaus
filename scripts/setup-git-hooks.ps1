# Wire .githooks/ as the project hooks directory.
# Idempotent.

$ErrorActionPreference = 'Stop'

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot '..')
Set-Location -LiteralPath $repoRoot

if (-not (Test-Path -LiteralPath '.githooks' -PathType Container)) {
    [Console]::Error.WriteLine('error: .githooks directory missing')
    exit 1
}

& git config core.hooksPath .githooks
if ($LASTEXITCODE -ne 0) {
    [Console]::Error.WriteLine('error: git config failed')
    exit 1
}

# Windows ignores the executable bit - chmod is a no-op here, skip it.
# Git for Windows still invokes hooks via Git Bash regardless of permissions.

$current = & git config --get core.hooksPath
Write-Output "Git hooks configured. core.hooksPath = $current"
