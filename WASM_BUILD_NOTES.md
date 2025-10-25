# WASM Build Notes

## Build Instructions (Current)

```bash
# From the project root
cd wasm
wasm-pack build --target web
```

The WASM module will be available at `wasm/pkg/transmitwave_wasm.js` and can be imported in HTML demos:

```javascript
import init, { WasmEncoder, WasmDecoder } from './wasm/pkg/transmitwave_wasm.js';
```

## Dependency Migration

### ✅ Resolved: 'env' Module Import Issue

**Previous Issue:** wasm-bindgen generated imports for a non-existent 'env' module.

**Root Cause:** The `instant` crate (used by transitive dependencies) required a browser-compatible time function.

**Solution Implemented:** Migrated from `reed-solomon-erasure` (which depended on outdated `parking_lot` 0.11) to `reed-solomon-simd` v3.1.0.

**Benefits:**
- ✅ No more 'env' module errors
- ✅ No parking_lot dependency issues
- ✅ Better performance with SIMD optimizations
- ✅ No need for post-build workarounds
- ✅ Cleaner dependency chain

### Previous Workaround (Deprecated)

Previously, a `fix-env-import.sh` script was needed to patch the WASM output. This is **no longer necessary** with the current dependency chain.

## Dependency Details

- **reed-solomon-simd** v3.1.0: Modern implementation with Leopard-RS algorithm
  - Uses fixedbitset and once_cell instead of parking_lot
  - No Node.js/Node-specific dependencies
  - SIMD optimizations (AVX2, SSSE3, Neon)

## Build Output

```
wasm/
├── Cargo.toml
├── src/
│   └── lib.rs
└── pkg/
    ├── transmitwave_wasm.js      (Generated)
    ├── transmitwave_wasm_bg.js   (Generated)
    ├── transmitwave_wasm_bg.wasm (Binary)
    └── package.json              (Generated)
```

No additional processing or scripts required.
