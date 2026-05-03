# Atomically claim the next unclaimed task from work/queue.md.
#
# Lock strategy: New-Item -ItemType Directory is atomic on NTFS, mirroring
# the mkdir-based approach in claim-next-task.sh.
#
# Output on success (stdout):
#   line 1: TASK: <task-id>
#   line 2+: full brief text for the task
# Exit code:
#   0 = claimed
#   1 = no tasks available
#   2 = usage error

[CmdletBinding()]
param(
    [Parameter(Position = 0)]
    [string]$WorktreeName
)

$ErrorActionPreference = 'Continue'

if (-not $WorktreeName) {
    [Console]::Error.WriteLine('usage: claim-next-task.ps1 <worktree-name>')
    exit 2
}

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot '..')
Set-Location -LiteralPath $repoRoot

$queue = 'work/queue.md'
$lockDir = '.queue.lock'

if (-not (Test-Path -LiteralPath $queue -PathType Leaf)) {
    [Console]::Error.WriteLine("claim: $queue missing")
    exit 1
}

# Acquire lock - retry up to 30s.
$attempts = 0
while ($true) {
    try {
        New-Item -ItemType Directory -Path $lockDir -ErrorAction Stop | Out-Null
        break
    }
    catch {
        $attempts++
        if ($attempts -gt 60) {
            [Console]::Error.WriteLine("claim: failed to acquire $lockDir after 30s; aborting")
            exit 1
        }
        Start-Sleep -Milliseconds 500
    }
}

try {
    # Read as UTF-8 explicitly - Windows PowerShell 5.1's Get-Content/Set-Content
    # default to the current code page and will mangle multi-byte characters
    # (em-dashes, etc.) on round-trip.
    $utf8 = New-Object System.Text.UTF8Encoding($false)
    $queueAbs = (Resolve-Path -LiteralPath $queue).Path
    $lines = [System.IO.File]::ReadAllLines($queueAbs, $utf8)

    $taskLineNo = -1
    for ($i = 0; $i -lt $lines.Count; $i++) {
        if ($lines[$i] -match '^- \[ \] UNCLAIMED:') {
            $taskLineNo = $i
            break
        }
    }

    if ($taskLineNo -lt 0) { exit 1 }

    $taskLine = $lines[$taskLineNo]
    $idMatch = [regex]::Match($taskLine, '^- \[ \] UNCLAIMED:\s*([A-Za-z0-9_-]+)')
    if (-not $idMatch.Success) {
        [Console]::Error.WriteLine("claim: could not parse task id from line: $taskLine")
        exit 1
    }
    $taskId = $idMatch.Groups[1].Value

    $newLine = $taskLine -replace '^- \[ \] UNCLAIMED:', "- [~] CLAIMED:${WorktreeName}:"
    $lines[$taskLineNo] = $newLine
    # Force LF newlines (see comment in finalize-task.ps1).
    [System.IO.File]::WriteAllText($queueAbs, ($lines -join "`n") + "`n", $utf8)

    $briefMatch = [regex]::Match($taskLine, 'docs/planning/work/[a-zA-Z0-9_/-]+\.md(#[a-zA-Z0-9_-]+)?')
    $briefFile = ''
    if ($briefMatch.Success) {
        $briefFile = ($briefMatch.Value -split '#', 2)[0]
    }

    Write-Output "TASK: $taskId"
    if ($briefFile -and (Test-Path -LiteralPath $briefFile -PathType Leaf)) {
        $briefAbs = (Resolve-Path -LiteralPath $briefFile).Path
        [System.IO.File]::ReadAllText($briefAbs, $utf8) | Write-Output
    }
    else {
        Write-Output $taskLine
        Write-Output ''
        Write-Output '(No linked brief found. See docs/planning/work/bedrock.md or streams.md.)'
    }
}
finally {
    Remove-Item -LiteralPath $lockDir -Recurse -Force -ErrorAction SilentlyContinue
}
