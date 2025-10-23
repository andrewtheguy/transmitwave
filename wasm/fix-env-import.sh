#!/bin/bash
# Fix wasm-bindgen 'env' module import issue for browser environment
# This script replaces the problematic 'import * as __wbg_star0 from 'env';'
# with a fallback empty object, since 'env' is not available in browsers

FILE="./pkg/testaudio_wasm.js"

if [ -f "$FILE" ]; then
    sed -i '' "s/^import \* as __wbg_star0 from 'env';$/\/\/ Fallback for missing 'env' module (not available in browser environment)\nlet __wbg_star0 = {};/" "$FILE"
    echo "✓ Fixed 'env' module import in $FILE"
else
    echo "✗ File not found: $FILE"
    exit 1
fi
