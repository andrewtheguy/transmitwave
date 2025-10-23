#!/bin/bash
# Build WASM and apply necessary fixes for browser compatibility

cd "$(dirname "$0")"

echo "Building WASM with wasm-pack..."
wasm-pack build --target web

echo ""
echo "Applying browser compatibility fixes..."
bash fix-env-import.sh

echo ""
echo "✅ WASM build complete and ready for browser!"
