# Create a new git worktree for parallel agent work.
#
# Usage: pwsh -File scripts/new-worktree.ps1 <name> [base-branch]
#   name          short slug, e.g. stream-s07
#   base-branch   defaults to main
#
# Worktrees live next to the repo at ../pixhaus-worktrees/<name>.

[CmdletBinding()]
param(
    [Parameter(Position = 0)] [string]$Name,
    [Parameter(Position = 1)] [string]$Base = 'main'
)

$ErrorActionPreference = 'Stop'

if (-not $Name) {
    [Console]::Error.WriteLine('usage: new-worktree.ps1 <name> [base-branch]')
    exit 2
}

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot '..')
Set-Location -LiteralPath $repoRoot

$worktreePath = "../pixhaus-worktrees/$Name"
$branchName = "feat/$Name"

if (Test-Path -LiteralPath $worktreePath) {
    [Console]::Error.WriteLine("error: $worktreePath already exists")
    exit 1
}

New-Item -ItemType Directory -Path '../pixhaus-worktrees' -Force | Out-Null

Write-Output "==> Creating worktree at $worktreePath on $branchName (base: $Base)"
& git worktree add $worktreePath -b $branchName $Base
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

# Wire the project hooks so the pre-commit gate fires even when `pnpm bootstrap`
# was never run in this checkout. core.hooksPath is resolved relative to each
# worktree's root, and every worktree carries its own .githooks/ checkout.
& git -C $worktreePath config core.hooksPath .githooks

Write-Output ''
Write-Output 'Worktree ready.'
Write-Output "  path:   $worktreePath"
Write-Output "  branch: $branchName"
Write-Output ''
Write-Output "Next: pwsh scripts/ralph.ps1 $Name  (or: node scripts/run.mjs ralph $Name)"
