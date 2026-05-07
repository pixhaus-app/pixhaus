# Post-edit hook: format and type-check the file Claude just touched.
# Receives Claude Code PostToolUse JSON on stdin (Edit / Write tool calls).
#
# Goal: surface format and type errors immediately so they go back into
# Claude's next turn. Keep it fast - sub-second on the second run.
#
# Mirror of scripts/post-edit.sh - see that file for the canonical comments.

$ErrorActionPreference = 'Continue'

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot '..')
Set-Location -LiteralPath $repoRoot

function Write-Stderr([string]$msg) { [Console]::Error.WriteLine($msg) }

# Pull file_path out of the PostToolUse JSON payload on stdin.
# Fall back to $args[0] for manual invocation.
$jsonInput = ''
if (-not [Console]::IsInputRedirected) {
    # Interactive - no stdin to read.
}
else {
    $jsonInput = [Console]::In.ReadToEnd()
}

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

if (-not $filePath) {
    exit 0
}

# Normalize: convert backslashes to forward slashes for consistency with .sh.
$filePath = $filePath -replace '\\', '/'

# Skip files we don't own.
$skipPatterns = @('/target/', '/node_modules/', '/dist/', '/.git/')
foreach ($pattern in $skipPatterns) {
    if ($filePath -like "*$pattern*") { exit 0 }
}

$isRust = $filePath -match '\.rs$'
$isTs = $filePath -match '\.(ts|tsx)$'
if (-not ($isRust -or $isTs)) { exit 0 }

function Has-Command([string]$name) {
    $null -ne (Get-Command $name -ErrorAction SilentlyContinue)
}

if ($isRust) {
    if (-not (Has-Command 'cargo')) {
        Write-Stderr 'post-edit: cargo not on PATH; skipping Rust checks.'
        exit 0
    }

    & cargo fmt --manifest-path Cargo.toml -- "$filePath" 2>$null | Out-Null

    $crate = ''
    try {
        $crate = (& node "$PSScriptRoot/run.mjs" find-crate-for-file "$filePath" 2>$null) | Select-Object -First 1
    }
    catch { $crate = '' }

    if ($crate) {
        Write-Stderr "post-edit: cargo clippy --tests -p $crate -- -D warnings"
        & cargo clippy --tests -p $crate -- -D warnings 2>&1 | ForEach-Object { Write-Stderr $_ }
        if ($LASTEXITCODE -ne 0) {
            Write-Stderr "post-edit: cargo clippy failed in crate $crate"
            exit 0
        }
    }
    else {
        Write-Stderr "post-edit: could not locate owning crate for $filePath"
    }
}

if ($isTs) {
    if (-not (Has-Command 'pnpm')) {
        Write-Stderr 'post-edit: pnpm not on PATH; skipping TS checks.'
        exit 0
    }

    Push-Location ui
    try {
        & pnpm prettier --write "$filePath" 2>$null | Out-Null
        Write-Stderr 'post-edit: tsc --noEmit (ui)'
        & pnpm tsc --noEmit 2>&1 | ForEach-Object { Write-Stderr $_ }
        if ($LASTEXITCODE -ne 0) {
            Write-Stderr 'post-edit: tsc reported errors'
            exit 0
        }
    }
    finally {
        Pop-Location
    }
}

exit 0
