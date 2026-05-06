#!/usr/bin/env sh
# Build the WASM plugin and copy it into the plugin folder.
#
# Prerequisites:
#   rustup target add wasm32-wasip1
#   cargo install extism-cli   (optional, for testing outside Pixhaus)
set -e

CRATE_NAME="invert_colors_verb"
PLUGIN_DIR="$(dirname "$0")"

cargo build --release --target wasm32-wasip1

cp "target/wasm32-wasip1/release/${CRATE_NAME}.wasm" \
   "${PLUGIN_DIR}/plugin.wasm"

echo "Built plugin.wasm"
