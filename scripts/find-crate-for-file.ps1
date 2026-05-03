# Walk up from a file to the nearest Cargo.toml and print the crate name.
# Usage: pwsh -File scripts/find-crate-for-file.ps1 path/to/file.rs

[CmdletBinding()]
param(
    [Parameter(Position = 0)]
    [string]$Path
)

$ErrorActionPreference = 'Stop'

if (-not $Path) {
    [Console]::Error.WriteLine("usage: find-crate-for-file.ps1 <file>")
    exit 2
}

$path = $Path -replace '\\', '/'

if (Test-Path -LiteralPath $path -PathType Container) {
    $dir = (Resolve-Path -LiteralPath $path).Path
}
elseif (Test-Path -LiteralPath $path) {
    $dir = (Resolve-Path -LiteralPath (Split-Path -Parent $path)).Path
}
else {
    # File may not exist on disk yet (e.g. about to be written). Walk from
    # whichever ancestor does exist.
    $candidate = Split-Path -Parent $path
    while ($candidate -and -not (Test-Path -LiteralPath $candidate)) {
        $candidate = Split-Path -Parent $candidate
    }
    if (-not $candidate) { exit 1 }
    $dir = (Resolve-Path -LiteralPath $candidate).Path
}

while ($dir) {
    $cargoToml = Join-Path $dir 'Cargo.toml'
    if (Test-Path -LiteralPath $cargoToml -PathType Leaf) {
        $content = Get-Content -LiteralPath $cargoToml -Raw
        if ($content -match '(?m)^\[package\]') {
            # Match the first `name = "..."` line. Cargo manifests sometimes
            # have other `name =` keys (bin/lib targets) but [package] sits at
            # the top, so head-of-file matching is good enough.
            $match = [regex]::Match($content, '(?m)^\s*name\s*=\s*"([^"]+)"')
            if ($match.Success) {
                Write-Output $match.Groups[1].Value
                exit 0
            }
        }
    }

    $parent = Split-Path -Parent $dir
    if (-not $parent -or $parent -eq $dir) { break }
    $dir = $parent
}

exit 1
