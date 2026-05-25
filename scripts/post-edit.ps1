# Post-edit hook: format and lint the Rust file Claude just touched.
# Receives Claude Code PostToolUse JSON on stdin (Edit / Write tool calls).
#
# Goal: surface format and clippy errors immediately so they go back into
# Claude's next turn. Rust-only, no Node. Mirror of scripts/post-edit.sh.

$ErrorActionPreference = 'Continue'

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot '..')
Set-Location -LiteralPath $repoRoot

function Write-Stderr([string]$msg) { [Console]::Error.WriteLine($msg) }

# Pull file_path out of the PostToolUse JSON payload on stdin.
# Fall back to $args[0] for manual invocation.
$jsonInput = ''
if ([Console]::IsInputRedirected) { $jsonInput = [Console]::In.ReadToEnd() }

$filePath = ''
if ($args.Count -ge 1 -and $args[0]) {
    $filePath = $args[0]
}
elseif ($jsonInput) {
    try {
        $payload = $jsonInput | ConvertFrom-Json
        if ($payload -and $payload.tool_input -and $payload.tool_input.file_path) {
            $filePath = [string]$payload.tool_input.file_path
        }
    }
    catch {
        # Malformed JSON - silently skip.
    }
}

if (-not $filePath) { exit 0 }

# Normalize backslashes to forward slashes for consistent matching.
$filePath = $filePath -replace '\\', '/'

foreach ($pattern in @('/target/', '/.git/')) {
    if ($filePath -like "*$pattern*") { exit 0 }
}

if ($filePath -notmatch '\.rs$') { exit 0 }

function Has-Command([string]$name) {
    $null -ne (Get-Command $name -ErrorAction SilentlyContinue)
}

if (-not (Has-Command 'cargo')) {
    Write-Stderr 'post-edit: cargo not on PATH; skipping Rust checks.'
    exit 0
}

# Walk up from the edited file to the nearest Cargo.toml that declares a
# [package], so fmt/clippy scope to the owning crate rather than the whole
# workspace. Fall back to workspace-wide when none is found (virtual manifest).
function Find-PackageManifest([string]$startPath) {
    $dir = Split-Path -Parent ([System.IO.Path]::GetFullPath($startPath))
    while ($dir) {
        $manifest = Join-Path $dir 'Cargo.toml'
        if (Test-Path -LiteralPath $manifest) {
            if (Select-String -LiteralPath $manifest -Pattern '^\s*\[package\]' -Quiet) {
                return $manifest
            }
        }
        $parent = Split-Path -Parent $dir
        if ($parent -eq $dir) { break }
        $dir = $parent
    }
    return ''
}

$manifest = Find-PackageManifest $filePath

if ($manifest) {
    Write-Stderr "post-edit: cargo fmt --manifest-path $manifest"
    & cargo fmt --manifest-path $manifest 2>&1 | ForEach-Object { Write-Stderr $_ }
    if ($LASTEXITCODE -ne 0) { Write-Stderr "post-edit: cargo fmt failed for $manifest" }

    Write-Stderr "post-edit: cargo clippy --manifest-path $manifest --tests -- -D warnings"
    & cargo clippy --manifest-path $manifest --tests -- -D warnings 2>&1 | ForEach-Object { Write-Stderr $_ }
    if ($LASTEXITCODE -ne 0) { Write-Stderr "post-edit: cargo clippy reported problems for $manifest" }
}
else {
    Write-Stderr 'post-edit: no owning crate found; running cargo fmt --all'
    & cargo fmt --all 2>&1 | ForEach-Object { Write-Stderr $_ }
    if ($LASTEXITCODE -ne 0) { Write-Stderr 'post-edit: cargo fmt --all failed' }
}

exit 0
