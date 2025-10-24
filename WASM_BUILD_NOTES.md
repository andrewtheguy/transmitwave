# WASM Build Notes

## Known Issues and Fixes

### 'env' Module Import Error

**Error:** `Uncaught TypeError: Failed to resolve module specifier "env". Relative references must start with either "/", "./", or "../".`

**Cause:** wasm-bindgen generates an import statement for a non-existent 'env' module when building for browser targets.

**Solution:** After building with `wasm-pack`, run the post-build fix script:

```bash
cd wasm
wasm-pack build --target web
./fix-env-import.sh
```

**What the fix does:**
- Replaces `import * as __wbg_star0 from 'env';` with a fallback empty object
- Allows the WASM module to load correctly in browser environments

## Build Instructions

```bash
# From the project root
cd wasm
wasm-pack build --target web
./fix-env-import.sh
```

The fixed WASM module will be available at `wasm/pkg/transmitwave_wasm.js` and can be imported in HTML demos:

```javascript
import init, { WasmEncoder, WasmDecoder } from './wasm/pkg/transmitwave_wasm.js';
```

## Why This Happens

wasm-bindgen sometimes generates code that references Node.js or other runtime-specific modules. The 'env' module is one such case. Since we're targeting a browser environment with `--target web`, this module doesn't exist, and we need to provide a fallback.
