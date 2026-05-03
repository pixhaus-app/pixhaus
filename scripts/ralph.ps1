# Ralph loop: claim a task, dispatch Claude with the brief, finalize on green.
# One ralph loop per worktree. Stop with Ctrl-C; the in-flight claim is
# released when finalize runs.
#
# Usage: pwsh -File scripts/ralph.ps1 <worktree-name>
#
# Environment:
#   PIXHAUS_RALPH_MODEL   Claude model (default: claude-sonnet-4-6)
#   PIXHAUS_RALPH_SLEEP   Seconds to sleep when queue is empty (default: 300)
#   PIXHAUS_RALPH_MAX     Max iterations before exit (default: unlimited)

[CmdletBinding()]
param(
    [Parameter(Position = 0)]
    [string]$WorktreeName
)

$ErrorActionPreference = 'Continue'

if (-not $WorktreeName) {
    [Console]::Error.WriteLine('usage: pwsh -File scripts/ralph.ps1 <worktree-name>')
    exit 2
}

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot '..')
Set-Location -LiteralPath $repoRoot

$model = if ($env:PIXHAUS_RALPH_MODEL) { $env:PIXHAUS_RALPH_MODEL } else { 'claude-sonnet-4-6' }
$sleepSec = if ($env:PIXHAUS_RALPH_SLEEP) { [int]$env:PIXHAUS_RALPH_SLEEP } else { 300 }
$maxIter = if ($env:PIXHAUS_RALPH_MAX) { [int]$env:PIXHAUS_RALPH_MAX } else { 0 }

$logDir = 'logs/ralph'
New-Item -ItemType Directory -Path $logDir -Force | Out-Null

function Has-Command([string]$name) {
    $null -ne (Get-Command $name -ErrorAction SilentlyContinue)
}

$runMjs = Join-Path $PSScriptRoot 'run.mjs'

$iter = 0
while ($true) {
    $iter++
    if ($maxIter -gt 0 -and $iter -gt $maxIter) {
        Write-Output "ralph[${WorktreeName}]: reached max iterations ($maxIter), exiting"
        break
    }

    Write-Output ''
    Write-Output "ralph[${WorktreeName}]: iteration $iter - claiming next task"

    $claimOutput = & node $runMjs claim-next-task $WorktreeName 2>&1
    $claimRc = $LASTEXITCODE

    if ($claimRc -ne 0 -or -not $claimOutput) {
        Write-Output "ralph[${WorktreeName}]: no tasks available; sleeping ${sleepSec}s"
        Start-Sleep -Seconds $sleepSec
        continue
    }

    $claimLines = @($claimOutput)
    $taskId = ($claimLines[0] -replace '^TASK:\s*', '').Trim()
    $taskBrief = ($claimLines | Select-Object -Skip 1) -join "`n"

    if (-not $taskId) {
        Write-Output "ralph[${WorktreeName}]: claim returned no task id; sleeping ${sleepSec}s"
        Start-Sleep -Seconds $sleepSec
        continue
    }

    # Append the shipping addendum so the agent commits, pushes, and opens
    # the PR autonomously.
    $addendumFile = Join-Path $repoRoot 'scripts/dispatch-addendum.md'
    if (Test-Path -LiteralPath $addendumFile -PathType Leaf) {
        $utf8 = New-Object System.Text.UTF8Encoding($false)
        $addendum = [System.IO.File]::ReadAllText($addendumFile, $utf8)
        $taskBrief = "$taskBrief`n`n$addendum"
    }
    else {
        [Console]::Error.WriteLine("ralph[${WorktreeName}]: warning: $addendumFile missing; running without ship-it instructions")
    }

    $ts = (Get-Date).ToUniversalTime().ToString('yyyyMMddTHHmmssZ')
    $logFile = Join-Path $logDir "${ts}-${WorktreeName}-${taskId}.json"

    Write-Output "ralph[${WorktreeName}]: claimed $taskId; dispatching Claude (model=$model)"
    Write-Output "ralph[${WorktreeName}]: log -> $logFile"

    if (-not (Has-Command 'claude')) {
        Write-Output "ralph[${WorktreeName}]: claude CLI not on PATH; releasing claim and exiting"
        & node $runMjs finalize-task $WorktreeName $taskId 'fail' 'claude CLI missing' | Out-Null
        exit 1
    }

    & claude --model $model --print $taskBrief --output-format json *> $logFile
    if ($LASTEXITCODE -eq 0) {
        # Don't mark DONE. The DONE flip is the human's call after the PR
        # merges, via `pnpm finalize <wt> <id> ok`.
        Write-Output ''
        Write-Output "ralph[${WorktreeName}]: $taskId ready for review (queue stays CLAIMED:${WorktreeName}: until merge)"
        $logContent = ''
        if (Test-Path -LiteralPath $logFile -PathType Leaf) {
            $utf8r = New-Object System.Text.UTF8Encoding($false)
            $logContent = [System.IO.File]::ReadAllText($logFile, $utf8r)
        }
        $prMatch = [regex]::Match($logContent, 'https://github\.com/[^\s"\\]+/pull/[0-9]+')
        if ($prMatch.Success) {
            Write-Output "ralph[${WorktreeName}]: PR -> $($prMatch.Value)"
        }
        else {
            [Console]::Error.WriteLine("ralph[${WorktreeName}]: no PR URL found in transcript; check the log or the worktree")
        }
        Write-Output "ralph[${WorktreeName}]: after the PR merges, run: pnpm finalize $WorktreeName $taskId ok"
    }
    else {
        $failReason = "claude exited non-zero (see $logFile)"
        & node $runMjs finalize-task $WorktreeName $taskId 'fail' $failReason | Out-Null
        [Console]::Error.WriteLine("ralph[${WorktreeName}]: $taskId returned to queue (see $logFile)")
    }
    # Either way, this worktree is now on a feature branch, not main.
    # Looping again would commit the next task on the wrong branch, so exit.
    break
}
