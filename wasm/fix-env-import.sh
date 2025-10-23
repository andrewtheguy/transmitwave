#!/bin/bash
# Fix wasm-bindgen 'env' module import issue for browser environment
# This script replaces the problematic 'import * as __wbg_star0 from 'env';'
# with a fallback object that provides required functions, since 'env' is not available in browsers

FILE="./pkg/testaudio_wasm.js"

if [ -f "$FILE" ]; then
    # Create the replacement with proper function implementations
    REPLACEMENT='\/\/ Fallback for missing '"'"'env'"'"' module (not available in browser environment)\n\/\/ Provide stub implementations for env imports used by wasm-bindgen\nlet __wbg_star0 = {\n    now: () => Date.now(),\n};'

    sed -i '' "s/^import \* as __wbg_star0 from 'env';$/$REPLACEMENT/" "$FILE"
    echo "✓ Fixed 'env' module import in $FILE"
else
    echo "✗ File not found: $FILE"
    exit 1
fi
