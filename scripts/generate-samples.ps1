# Regenerates the sample .pixhaus project files under examples/samples/.
#
# Usage: node scripts/run.mjs generate-samples
#
# The committed files are the source of truth. Only run this when the
# generator code or the wire format has intentionally changed.

$ErrorActionPreference = "Stop"

Set-Location (Split-Path $PSScriptRoot -Parent)

$env:PIXHAUS_REGEN_SAMPLES = "1"
cargo nextest run `
    -p pixhaus-io `
    --test generate_sample_projects `
    generate_sample_projects `
    -- --nocapture
$rc = $LASTEXITCODE

# Clear the env var even on failure so a subsequent CI test run doesn't
# inherit it, then propagate the exit code so the caller (and CI) sees
# the failure. PowerShell's $ErrorActionPreference doesn't catch native-
# command non-zero exits — we have to check $LASTEXITCODE explicitly.
Remove-Item Env:PIXHAUS_REGEN_SAMPLES -ErrorAction SilentlyContinue
if ($rc -ne 0) {
    Write-Error "generate-samples: cargo nextest exited with code $rc"
    exit $rc
}
