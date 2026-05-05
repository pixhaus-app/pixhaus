#!/usr/bin/env bash
# Regenerates the sample .pixhaus project files under examples/samples/.
#
# Usage: node scripts/run.mjs generate-samples
#
# The committed files are the source of truth. Only run this when the
# generator code or the wire format has intentionally changed.

set -euo pipefail

cd "$(dirname "$0")/.."

PIXHAUS_REGEN_SAMPLES=1 cargo nextest run \
    -p pixhaus-io \
    --test generate_sample_projects \
    generate_sample_projects \
    -- --nocapture
