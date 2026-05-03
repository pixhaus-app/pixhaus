# Mark a task done if CI is green, otherwise return it to the queue.
#
# Usage: pwsh -File scripts/finalize-task.ps1 <worktree-name> <task-id> <ok|fail> [reason]

[CmdletBinding()]
param(
    [Parameter(Position = 0)] [string]$WorktreeName,
    [Parameter(Position = 1)] [string]$TaskId,
    [Parameter(Position = 2)] [string]$Outcome = 'fail',
    [Parameter(Position = 3)] [string]$Reason = ''
)

$ErrorActionPreference = 'Continue'

if (-not $WorktreeName -or -not $TaskId) {
    [Console]::Error.WriteLine('usage: finalize-task.ps1 <worktree-name> <task-id> <ok|fail> [reason]')
    exit 2
}

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot '..')
Set-Location -LiteralPath $repoRoot

$queue = 'work/queue.md'
$lockDir = '.queue.lock'

if (-not (Test-Path -LiteralPath $queue -PathType Leaf)) {
    [Console]::Error.WriteLine("finalize: $queue missing")
    exit 1
}

$attempts = 0
while ($true) {
    try {
        New-Item -ItemType Directory -Path $lockDir -ErrorAction Stop | Out-Null
        break
    }
    catch {
        $attempts++
        if ($attempts -gt 60) {
            [Console]::Error.WriteLine("finalize: failed to acquire $lockDir after 30s; aborting")
            exit 1
        }
        Start-Sleep -Milliseconds 500
    }
}

try {
    # Read/write as UTF-8 (no BOM) explicitly - see note in claim-next-task.ps1.
    $utf8 = New-Object System.Text.UTF8Encoding($false)
    $queueAbs = (Resolve-Path -LiteralPath $queue).Path
    $lines = [System.IO.File]::ReadAllLines($queueAbs, $utf8)

    $lineNo = -1
    $needle = "^- \[~\] CLAIMED:${WorktreeName}:\s*${TaskId}"
    for ($i = 0; $i -lt $lines.Count; $i++) {
        if ($lines[$i] -match $needle) {
            $lineNo = $i
            break
        }
    }

    if ($lineNo -lt 0) {
        [Console]::Error.WriteLine("finalize: could not find claimed task ${TaskId} for ${WorktreeName}")
        exit 1
    }

    $line = $lines[$lineNo]

    if ($Outcome -eq 'ok') {
        $newLine = $line -replace "^- \[~\] CLAIMED:${WorktreeName}:", '- [x] DONE:'
        Write-Output "finalize: ${TaskId} -> done"
    }
    else {
        if (-not $Reason) { $Reason = 'returned to queue' }
        $base = $line -replace "^- \[~\] CLAIMED:${WorktreeName}:\s*${TaskId}", "- [ ] UNCLAIMED: ${TaskId}"
        $base = $base -replace ' \[FAIL: [^\]]+\]', ''
        $newLine = "${base} [FAIL: ${Reason}]"
        Write-Output "finalize: ${TaskId} -> returned to queue (reason: ${Reason})"
    }

    $lines[$lineNo] = $newLine
    # Force LF newlines (Environment.NewLine is CRLF on Windows; .gitattributes
    # mandates LF for queue.md, and a CRLF write would dirty the working tree).
    [System.IO.File]::WriteAllText($queueAbs, ($lines -join "`n") + "`n", $utf8)
}
finally {
    Remove-Item -LiteralPath $lockDir -Recurse -Force -ErrorAction SilentlyContinue
}
