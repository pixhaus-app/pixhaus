# Dispatch: one-shot Claude run for a single queue task.
#
# Validates the task is UNCLAIMED, creates the worktree if needed, claims
# atomically, runs Claude once with the brief, finalizes ok/fail, tees the
# transcript to logs/dispatch/<timestamp>-<task>.json. Differs from ralph
# in that it dispatches once and exits, and lets you override the model.
#
# Usage: dispatch <task-id> [--model <model>] [--worktree <name>]
#
# Mirror of scripts/dispatch.sh.

[CmdletBinding()]
param(
    [Parameter(Position = 0)] [string]$TaskId,
    [string]$Model = 'claude-sonnet-4-6',
    [string]$Worktree = ''
)

$ErrorActionPreference = 'Continue'

if (-not $TaskId) {
    [Console]::Error.WriteLine('usage: dispatch <task-id> [--model <model>] [--worktree <name>]')
    exit 2
}

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
Set-Location -LiteralPath $repoRoot

if (-not $Worktree) { $Worktree = "stream-" + $TaskId.ToLower() }

$queue = 'work/queue.md'
if (-not (Test-Path -LiteralPath $queue -PathType Leaf)) {
    [Console]::Error.WriteLine("dispatch: $queue missing")
    exit 1
}

# Read with explicit UTF-8 to keep em-dashes intact (see finalize-task.ps1).
$utf8 = New-Object System.Text.UTF8Encoding($false)
$queueLines = [System.IO.File]::ReadAllLines((Resolve-Path -LiteralPath $queue).Path, $utf8)

# Validate the task is currently UNCLAIMED.
$pattern = "^- \[ \] UNCLAIMED:\s*$([regex]::Escape($TaskId))(\s|$)"
$found = $false
foreach ($line in $queueLines) {
    if ($line -match $pattern) { $found = $true; break }
}
if (-not $found) {
    [Console]::Error.WriteLine("dispatch: task $TaskId is not UNCLAIMED in $queue")
    [Console]::Error.WriteLine('         (already claimed, done, or unknown id)')
    exit 2
}

$runMjs = Join-Path $PSScriptRoot 'run.mjs'
$worktreePath = Join-Path (Split-Path -Parent $repoRoot) "pixhaus-worktrees/$Worktree"

if (-not (Test-Path -LiteralPath $worktreePath -PathType Container)) {
    Write-Output "dispatch: creating worktree $Worktree"
    & node $runMjs new-worktree $Worktree
    if ($LASTEXITCODE -ne 0) {
        [Console]::Error.WriteLine('dispatch: new-worktree failed')
        exit 1
    }
}
else {
    Write-Output "dispatch: reusing existing worktree $Worktree"
}

# Claim atomically. Verify the claimed id matches the requested one.
Write-Output "dispatch: claiming next task as worktree=$Worktree"
$claimOutput = & node $runMjs claim-next-task $Worktree 2>&1
$claimRc = $LASTEXITCODE
if ($claimRc -ne 0 -or -not $claimOutput) {
    [Console]::Error.WriteLine('dispatch: claim-next-task failed')
    $claimOutput | ForEach-Object { [Console]::Error.WriteLine($_) }
    exit 1
}

$claimLines = @($claimOutput)
$claimedId = ($claimLines[0] -replace '^TASK:\s*', '').Trim()
$taskBrief = ($claimLines | Select-Object -Skip 1) -join "`n"

if ($claimedId -ne $TaskId) {
    [Console]::Error.WriteLine("dispatch: claim returned $claimedId, expected $TaskId")
    [Console]::Error.WriteLine('         (someone else claimed first; releasing)')
    & node $runMjs finalize-task $Worktree $claimedId 'fail' "wrong task; dispatch wanted $TaskId" | Out-Null
    exit 1
}

# Append the shipping addendum so the agent commits, pushes, and opens the
# PR autonomously instead of stopping at "should I commit?".
$addendumFile = Join-Path $repoRoot 'scripts/dispatch-addendum.md'
if (Test-Path -LiteralPath $addendumFile -PathType Leaf) {
    $addendum = [System.IO.File]::ReadAllText($addendumFile, $utf8)
    $taskBrief = "$taskBrief`n`n$addendum"
}
else {
    [Console]::Error.WriteLine("dispatch: warning: $addendumFile missing; running without ship-it instructions")
}

# Prepare the log file in the main repo (not the worktree).
$logDir = Join-Path $repoRoot 'logs/dispatch'
New-Item -ItemType Directory -Path $logDir -Force | Out-Null
$ts = (Get-Date).ToUniversalTime().ToString('yyyyMMddTHHmmssZ')
$logFile = Join-Path $logDir "$ts-$TaskId.json"

Write-Output "dispatch: running claude (model=$Model)"
Write-Output "dispatch: log -> $logFile"

if (-not (Get-Command claude -ErrorAction SilentlyContinue)) {
    [Console]::Error.WriteLine('dispatch: claude CLI not on PATH; releasing claim')
    & node $runMjs finalize-task $Worktree $TaskId 'fail' 'claude CLI missing' | Out-Null
    exit 1
}

Set-Location -LiteralPath $worktreePath

# Tee-Object writes to file and pipeline simultaneously.
& claude --model $Model --print $taskBrief --permission-mode bypassPermissions --output-format json 2>&1 | Tee-Object -FilePath $logFile
$claudeRc = $LASTEXITCODE

Set-Location -LiteralPath $repoRoot

if ($claudeRc -eq 0) {
    # Don't mark DONE yet. The DONE flip is the human's call after the PR
    # merges, via `pnpm finalize <wt> <id> ok`.
    Write-Output ''
    Write-Output "dispatch: $TaskId ready for review (queue stays CLAIMED:${Worktree}: until merge)"
    $logContent = ''
    if (Test-Path -LiteralPath $logFile -PathType Leaf) {
        $logContent = [System.IO.File]::ReadAllText($logFile, $utf8)
    }
    $prMatch = [regex]::Match($logContent, 'https://github\.com/[^\s"\\]+/pull/[0-9]+')
    if ($prMatch.Success) {
        Write-Output "dispatch: PR -> $($prMatch.Value)"
    }
    else {
        [Console]::Error.WriteLine('dispatch: no PR URL found in transcript; check the log or the worktree manually')
    }
    Write-Output "dispatch: after the PR merges, run: pnpm finalize $Worktree $TaskId ok"
}
else {
    $reason = "claude exited non-zero (see $logFile)"
    & node $runMjs finalize-task $Worktree $TaskId 'fail' $reason
    [Console]::Error.WriteLine("dispatch: $TaskId returned to queue (see $logFile)")
    exit 1
}

Write-Output "dispatch: log saved to $logFile"
