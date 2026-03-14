#!/bin/bash
# setup.sh — Download dependencies for tachyon-simd
#
# Run this once before building with simdjson support:
#   ./setup.sh

set -e

SIMDJSON_VERSION="v4.3.0"
SIMDJSON_BASE="https://raw.githubusercontent.com/simdjson/simdjson/master/singleheader"
VENDOR_DIR="tachyon-simd/cpp/vendor"

echo "[tachyon] Setting up dependencies..."

# Download simdjson single-header files
if [ ! -f "$VENDOR_DIR/simdjson.h" ]; then
    echo "[tachyon] Downloading simdjson..."
    mkdir -p "$VENDOR_DIR"
    curl -sL "$SIMDJSON_BASE/simdjson.h" -o "$VENDOR_DIR/simdjson.h"
    curl -sL "$SIMDJSON_BASE/simdjson.cpp" -o "$VENDOR_DIR/simdjson.cpp"
    echo "[tachyon] simdjson downloaded to $VENDOR_DIR/"
else
    echo "[tachyon] simdjson already present in $VENDOR_DIR/"
fi

echo "[tachyon] Setup complete. Build with:"
echo "  cargo build --release"
echo ""
echo "  Or without simdjson (pure Rust fallback):"
echo "  cargo build --release --no-default-features"