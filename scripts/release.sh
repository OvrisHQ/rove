#!/usr/bin/env bash
set -euo pipefail

VERSION="${1:-$(cargo metadata --no-deps --format-version=1 | python3 -c 'import sys,json; print(json.load(sys.stdin)["packages"][0]["version"])')}"
echo "=== Rove Release v${VERSION} ==="
echo ""

# 1. Run full test suite
echo "--- Running tests ---"
TMPDIR=/tmp cargo test --workspace --exclude fs-editor --exclude terminal-plugin --exclude git-plugin --exclude screenshot-plugin
echo ""

# 2. Run clippy
echo "--- Running clippy ---"
cargo clippy --workspace --exclude fs-editor --exclude terminal-plugin --exclude git-plugin --exclude screenshot-plugin -- -D warnings
echo ""

# 3. Check formatting
echo "--- Checking format ---"
cargo fmt --all -- --check
echo ""

# 4. Build release
echo "--- Building release ---"
cargo build --release -p engine
echo ""

# 5. Build WASM plugins
echo "--- Building WASM plugins ---"
WASM_TARGET="wasm32-wasip1"
for plugin in fs-editor terminal-plugin git-plugin screenshot-plugin; do
    echo "  Building ${plugin}..."
    cargo build --release --target ${WASM_TARGET} -p ${plugin} 2>/dev/null || echo "  SKIP"
done
echo ""

# 6. Generate manifest
echo "--- Generating manifest ---"
if [ -f scripts/build-manifest.py ]; then
    python3 scripts/build-manifest.py
fi
echo ""

# 7. Report
echo "--- Release artifacts ---"
if [ -f target/release/rove ]; then
    SIZE=$(du -h target/release/rove | cut -f1)
    echo "  Rove binary: ${SIZE}"
fi

for wasm in target/${WASM_TARGET}/release/*.wasm; do
    [ -f "$wasm" ] && echo "  $(basename $wasm): $(du -h "$wasm" | cut -f1)"
done

echo ""
echo "=== Release v${VERSION} ready ==="
echo ""
echo "To publish:"
echo "  git tag v${VERSION}"
echo "  git push origin v${VERSION}"
echo "  gh release create v${VERSION} target/release/rove --title 'Rove v${VERSION}'"
