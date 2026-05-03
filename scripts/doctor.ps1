# Doctor: read-only environment check.
# Prints PASS / WARN / FAIL per probe, grouped by section. WARNs do not
# fail the script; FAILs do. Exit 0 if zero FAILs, 1 otherwise.
#
# Mirror of scripts/doctor.sh. Never prints the value of secrets.

$ErrorActionPreference = 'Continue'

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
Set-Location -LiteralPath $repoRoot

$script:PassCount = 0
$script:WarnCount = 0
$script:FailCount = 0

function Pass([string]$label) {
    $script:PassCount++
    Write-Output "PASS  $label"
}
function Warn([string]$label, [string]$hint) {
    $script:WarnCount++
    if ($hint) { Write-Output "WARN  $label ($hint)" } else { Write-Output "WARN  $label" }
}
function Fail([string]$label, [string]$hint) {
    $script:FailCount++
    if ($hint) { Write-Output "FAIL  $label ($hint)" } else { Write-Output "FAIL  $label" }
}
function Section([string]$title) {
    Write-Output ''
    Write-Output "== $title =="
}
function Has-Command([string]$name) {
    $null -ne (Get-Command $name -ErrorAction SilentlyContinue)
}

# ---- Toolchain ----
Section 'Toolchain'

if (Has-Command 'rustup') {
    Pass 'rustup'
}
else {
    Fail 'rustup' 'install from https://rustup.rs'
}

$expectedChannel = ''
if (Test-Path -LiteralPath 'rust-toolchain.toml') {
    $rt = Get-Content -LiteralPath 'rust-toolchain.toml' -Raw
    $m = [regex]::Match($rt, '(?m)^\s*channel\s*=\s*"([^"]+)"')
    if ($m.Success) { $expectedChannel = $m.Groups[1].Value }
}

if (Has-Command 'rustc') {
    $rustcOut = (& rustc --version 2>$null) -join "`n"
    $rustcMatch = [regex]::Match($rustcOut, 'rustc\s+(\S+)')
    $rustcVersion = if ($rustcMatch.Success) { $rustcMatch.Groups[1].Value } else { '' }
    if (-not $expectedChannel) {
        Pass "rustc ($rustcVersion)"
    }
    elseif ($rustcVersion -and $rustcVersion -like "$expectedChannel.*" -or $rustcVersion -eq $expectedChannel) {
        Pass "rust toolchain ($rustcVersion, expected $expectedChannel)"
    }
    else {
        Fail 'rust toolchain' "have $rustcVersion, expected $expectedChannel; run: rustup install $expectedChannel"
    }
}
else {
    $hint = if ($expectedChannel) { "rustup install $expectedChannel" } else { 'install Rust via rustup' }
    Fail 'rustc' $hint
}

if (Has-Command 'cargo') { Pass 'cargo' } else { Fail 'cargo' 'install Rust via rustup' }

# ---- Cargo subcommands ----
Section 'Cargo subcommands'

$subcommands = @(
    @{ name = 'cargo-nextest'; install = 'cargo install cargo-nextest' },
    @{ name = 'cargo-deny';    install = 'cargo install cargo-deny' },
    @{ name = 'cargo-audit';   install = 'cargo install cargo-audit' },
    @{ name = 'cargo-machete'; install = 'cargo install cargo-machete' },
    @{ name = 'cargo-watch';   install = 'cargo install cargo-watch' },
    @{ name = 'typos';         install = 'cargo install typos-cli' },
    @{ name = 'bacon';         install = 'cargo install bacon' },
    @{ name = 'cargo-tauri';   install = 'cargo install tauri-cli --version ^2.0.0' }
)
foreach ($s in $subcommands) {
    if (Has-Command $s.name) { Pass $s.name } else { Warn $s.name $s.install }
}

# ---- Node ----
Section 'Node'

if (Has-Command 'node') {
    $nodeOut = (& node --version 2>$null) -join "`n"
    $nodeMatch = [regex]::Match($nodeOut, 'v(\d+)')
    if ($nodeMatch.Success) {
        $major = [int]$nodeMatch.Groups[1].Value
        if ($major -ge 22) {
            Pass "node ($nodeOut)"
        }
        else {
            Fail 'node' "have $nodeOut, need >= 22; install from https://nodejs.org"
        }
    }
    else {
        Fail 'node' 'could not parse node --version output'
    }
}
else {
    Fail 'node' 'install from https://nodejs.org (need >= 22)'
}

if (Has-Command 'pnpm') {
    $pnpmVer = ((& pnpm --version 2>$null) -join "`n").Trim()
    $pnpmMajor = -1
    if ($pnpmVer -match '^(\d+)') { $pnpmMajor = [int]$Matches[1] }
    if ($pnpmMajor -ge 10) {
        Pass "pnpm ($pnpmVer)"
    }
    else {
        Fail 'pnpm' "have $pnpmVer, need >= 10; corepack enable pnpm"
    }
}
else {
    Fail 'pnpm' 'corepack enable pnpm (or: npm install -g pnpm)'
}

# ---- Git ----
Section 'Git'

if (Has-Command 'git') { Pass 'git' } else { Fail 'git' 'install git' }

& git rev-parse --git-dir *> $null
if ($LASTEXITCODE -eq 0) {
    Pass 'inside a git repo'
}
else {
    Fail 'git repo' 'run from a git checkout'
}

$hooksPath = ((& git config --get core.hooksPath 2>$null) -join "`n").Trim()
if ($hooksPath -eq '.githooks') {
    Pass 'core.hooksPath = .githooks'
}
else {
    $current = if ($hooksPath) { $hooksPath } else { 'unset' }
    Fail 'core.hooksPath' "currently '$current'; run: pnpm bootstrap (or: node scripts/run.mjs setup-git-hooks)"
}

# Executable bit is N/A on Windows; the .githooks/pre-commit shim is invoked
# by Git Bash regardless of the bit. Skip the check on Windows.

# ---- GitHub auth ----
Section 'GitHub auth'

if (Has-Command 'gh') {
    Pass 'gh CLI'
    & gh auth status *> $null
    if ($LASTEXITCODE -eq 0) {
        Pass 'gh authenticated'
    }
    else {
        Warn 'gh auth' 'run: gh auth login'
    }
}
else {
    Warn 'gh CLI' 'install from https://cli.github.com (needed to open PRs from ralph)'
}

# ---- Anthropic ----
Section 'Anthropic'

# Pixhaus uses Claude Code via subscription (Pro/Max). The claude CLI handles
# auth itself via OAuth login; ANTHROPIC_API_KEY is only needed for direct
# API mode. Don't print its value if set.
if (Has-Command 'claude') {
    Pass 'claude CLI'
}
else {
    Fail 'claude CLI' 'install Claude Code; see https://claude.com/claude-code'
}

if ($env:ANTHROPIC_API_KEY) {
    Pass 'ANTHROPIC_API_KEY set (API mode)'
}
else {
    Pass 'ANTHROPIC_API_KEY not set (subscription mode; claude CLI handles auth)'
}

# ---- Repo state ----
Section 'Repo state'

if (Test-Path -LiteralPath 'work/queue.md' -PathType Leaf) {
    $queue = Get-Content -LiteralPath 'work/queue.md'
    $unclaimed = @($queue | Where-Object { $_ -match '^- \[ \] UNCLAIMED:' }).Count
    if ($unclaimed -gt 0) {
        Pass "work/queue.md ($unclaimed unclaimed)"
    }
    else {
        Fail 'work/queue.md' 'no UNCLAIMED tasks; nothing to dispatch'
    }
}
else {
    Fail 'work/queue.md' 'missing'
}

# Disk space. PowerShell uses Get-PSDrive against the drive holding the
# repo root; bash uses df. Genuinely divergent.
try {
    $driveLetter = (Get-Item -LiteralPath $repoRoot).PSDrive.Name
    $drive = Get-PSDrive -Name $driveLetter -ErrorAction Stop
    $freeGb = [math]::Floor($drive.Free / 1GB)
    if ($freeGb -ge 5) {
        Pass "disk space (${freeGb}G free)"
    }
    else {
        Warn 'disk space' "${freeGb}G free; worktrees + cargo target/ want >= 5G"
    }
}
catch {
    Warn 'disk space' 'could not determine free space'
}

# ---- Summary ----
Write-Output ''
Write-Output ("Summary: {0} PASS, {1} WARN, {2} FAIL" -f $script:PassCount, $script:WarnCount, $script:FailCount)

if ($script:FailCount -gt 0) { exit 1 }
exit 0
