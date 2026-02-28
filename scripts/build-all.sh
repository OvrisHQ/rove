#!/usr/bin/env bash
set -euo pipefail

echo "=== Rove Build Script ==="
echo ""

# Build engine (release)
echo "--- Building engine (release) ---"
cargo build --release -p engine
ENGINE_SIZE=$(ls -la target/release/rove 2>/dev/null | awk '{print $5}')
echo "Engine binary size: ${ENGINE_SIZE:-N/A} bytes"
echo ""

# Build WASM plugins
echo "--- Building WASM plugins ---"
WASM_TARGET="wasm32-wasip1"

for plugin in fs-editor terminal-plugin git-plugin screenshot-plugin; do
    echo "  Building ${plugin}..."
    cargo build --release --target ${WASM_TARGET} -p ${plugin} 2>/dev/null && \
        echo "  OK" || echo "  SKIP (target not installed?)"
done

echo ""

# Report sizes
echo "--- Artifact sizes ---"
if [ -f target/release/rove ]; then
    du -h target/release/rove
fi

for wasm in target/${WASM_TARGET}/release/*.wasm; do
    [ -f "$wasm" ] && du -h "$wasm"
done

echo ""
echo "=== Build complete ==="
