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
        & node $runMjs finalize-task $WorktreeName $taskId 'ok' | Out-Null
    }
    else {
        $failReason = "claude exited non-zero (see $logFile)"
        & node $runMjs finalize-task $WorktreeName $taskId 'fail' $failReason | Out-Null
    }
}
