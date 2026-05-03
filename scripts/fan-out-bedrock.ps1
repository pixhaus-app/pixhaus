# Fan-out: print the parallel ralph commands for unclaimed bedrock tasks.
#
# By default scans work/queue.md for UNCLAIMED B3..B7 and prints a numbered
# command block for each (worktree create + cd + ralph). Honors the
# [OPUS-REQUIRED] tag by setting PIXHAUS_RALPH_MODEL.
#
# With -Background, instead of printing, runs each ralph loop as a
# Start-Job background job. Bash uses nohup + disown; PowerShell uses
# Start-Job - these genuinely diverge.
#
# Bedrock = B2..B7. B2 is run manually first.
#
# Mirror of scripts/fan-out-bedrock.sh.

[CmdletBinding()]
param(
    [switch]$Background
)

$ErrorActionPreference = 'Continue'

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
Set-Location -LiteralPath $repoRoot

$queue = 'work/queue.md'
if (-not (Test-Path -LiteralPath $queue -PathType Leaf)) {
    [Console]::Error.WriteLine("fan-out: $queue missing")
    exit 1
}

$utf8 = New-Object System.Text.UTF8Encoding($false)
$queueLines = [System.IO.File]::ReadAllLines((Resolve-Path -LiteralPath $queue).Path, $utf8)

$tasks = @()
foreach ($line in $queueLines) {
    if ($line -match '^- \[ \] UNCLAIMED:\s*(B[3-7])(\s|$)') {
        $tasks += [pscustomobject]@{
            Id    = $Matches[1]
            Line  = $line
            Opus  = $line -match 'OPUS-REQUIRED'
        }
    }
}

if ($tasks.Count -eq 0) {
    Write-Output "fan-out: no UNCLAIMED bedrock tasks (B3..B7) in $queue"
    exit 0
}

$runMjs = Join-Path $PSScriptRoot 'run.mjs'
$startedJobs = @()

$n = 0
foreach ($t in $tasks) {
    $n++
    $worktree = "stream-" + $t.Id.ToLower()
    $model = if ($t.Opus) { 'claude-opus-4-7' } else { '' }

    if ($Background) {
        New-Item -ItemType Directory -Path 'logs/ralph' -Force | Out-Null
        $logFile = Join-Path $repoRoot "logs/ralph/$($t.Id).log"
        Write-Output "fan-out: starting ralph for $($t.Id) (worktree=$worktree, log=$logFile)"

        $worktreePath = Join-Path (Split-Path -Parent $repoRoot) "pixhaus-worktrees/$worktree"
        if (-not (Test-Path -LiteralPath $worktreePath -PathType Container)) {
            & node $runMjs new-worktree $worktree *>> $logFile
        }

        # Capture variables into the job scriptblock via -ArgumentList.
        $job = Start-Job -Name "ralph-$($t.Id)" -ArgumentList @($runMjs, $worktree, $model, $logFile, $repoRoot) -ScriptBlock {
            param($mjs, $wt, $modelName, $log, $root)
            Set-Location -LiteralPath $root
            if ($modelName) { $env:PIXHAUS_RALPH_MODEL = $modelName }
            & node $mjs ralph $wt *>> $log
        }
        $startedJobs += [pscustomobject]@{ Id = $t.Id; JobId = $job.Id; JobName = $job.Name }
    }
    else {
        Write-Output ''
        Write-Output "# Terminal $n  ($($t.Id))"
        Write-Output "node scripts/run.mjs new-worktree $worktree"
        Write-Output "cd ../pixhaus-worktrees/$worktree"
        if ($t.Opus) {
            Write-Output "`$env:PIXHAUS_RALPH_MODEL='claude-opus-4-7'; node scripts/run.mjs ralph $worktree"
        }
        else {
            Write-Output "node scripts/run.mjs ralph $worktree"
        }
    }
}

if ($Background) {
    Write-Output ''
    Write-Output "fan-out: started $($startedJobs.Count) ralph loop(s):"
    foreach ($j in $startedJobs) {
        Write-Output "  $($j.Id)  Job $($j.JobId)  ($($j.JobName))"
    }
    Write-Output ''
    Write-Output 'Stop a loop:  Stop-Job -Name ralph-<task>'
    Write-Output 'Stop all:     Get-Job -Name ralph-* | Stop-Job'
}
else {
    Write-Output ''
    Write-Output '# Run each block above in a separate terminal.'
}
