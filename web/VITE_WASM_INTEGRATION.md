# Vite + WASM Integration Guide

## Overview

This document explains how the Vite web project integrates with the Rust-compiled WebAssembly module and how to resolve WASM loading issues.

## Project Setup

### WASM Module Integration

The WASM module (`testaudio-wasm`) is installed as a **file path dependency** in `package.json`:

```json
{
  "dependencies": {
    "testaudio-wasm": "file:../wasm/pkg"
  }
}
```

This creates a symbolic link in `node_modules/` to the compiled WASM package at `../wasm/pkg/`.

### File Structure

```
testaudio/
├── wasm/pkg/                    # Compiled WASM output
│   ├── package.json
│   ├── testaudio_wasm.js        # JavaScript bindings
│   ├── testaudio_wasm.d.ts      # TypeScript definitions
│   └── testaudio_wasm_bg.wasm   # WASM binary (~363 KB)
│
└── web/
    ├── node_modules/
    │   └── testaudio-wasm/      # Symlink to ../wasm/pkg
    ├── src/
    │   └── utils/
    │       └── wasm.ts          # WASM initialization wrapper
    ├── vite.config.ts           # Vite configuration
    └── package.json
```

## WASM Initialization

### Default Flow (Production)

When `init()` is called without arguments, the WASM JavaScript binding uses `import.meta.url` to locate the `.wasm` binary relative to the JS file:

```typescript
// From generated testaudio_wasm.js (line 648)
if (typeof module_or_path === 'undefined') {
    module_or_path = new URL('testaudio_wasm_bg.wasm', import.meta.url);
}
```

This works in production where:
- The WASM binary is bundled with the JavaScript in `dist/`
- Both files are in the same directory
- URLs are relative to the output location

### Development Mode Challenge

In Vite dev mode, the issue occurs because:

1. The JS binding constructs a URL like `/path/to/testaudio_wasm_bg.wasm`
2. Vite's `import.meta.url` points to the file system path (starting with `file://`)
3. The browser's `fetch()` security model blocks direct file system access
4. Result: "403 Forbidden" or similar error

### Solution: Fallback Initialization

The `src/utils/wasm.ts` file implements a robust initialization strategy:

```typescript
export async function initWasm(): Promise<void> {
    if (wasmInitialized) {
        return;
    }

    try {
        // Try default initialization first (works in production)
        await init();
        wasmInitialized = true;
    } catch (error) {
        // Fallback: try known paths for development
        const possiblePaths = [
            '/testaudio_wasm_bg.wasm',           // Production
            '/node_modules/testaudio-wasm/testaudio_wasm_bg.wasm', // Development
        ];

        for (const wasmPath of possiblePaths) {
            try {
                const response = await fetch(wasmPath);
                if (response.ok) {
                    await init(response);
                    wasmInitialized = true;
                    return;
                }
            } catch (err) {
                // Try next path
                continue;
            }
        }

        throw new Error('WASM initialization failed');
    }
}
```

## Vite Configuration

### Key Settings

**`vite.config.ts`:**

```typescript
{
  server: {
    fs: {
      // Allow serving from parent directory (where wasm/pkg is located)
      allow: ['..'],
    },
  },

  optimizeDeps: {
    // Don't pre-bundle WASM module
    exclude: ['testaudio-wasm'],
  },

  resolve: {
    // Alias for module resolution
    alias: {
      'testaudio-wasm': path.resolve(__dirname, 'node_modules/testaudio-wasm'),
    },
  },
}
```

### Why These Settings Matter

| Setting | Purpose |
|---------|---------|
| `fs.allow: ['..']` | Allows Vite dev server to serve files from parent directory |
| `exclude: ['testaudio-wasm']` | Prevents Vite from pre-bundling WASM (keeps it as-is) |
| `resolve.alias` | Ensures consistent module path resolution |

## Build Output

### Production Build

When you run `npm run build`:

1. **Input**: Source files in `src/` + WASM binary from `node_modules/`
2. **Processing**: Vite bundles and optimizes everything
3. **Output**: Single bundle with embedded WASM binary

```
dist/
├── index.html                       # Entry point
├── index.TQRyLE3l.js                # App code (minified)
├── testaudio_wasm_bg.DqWgEE_u.wasm  # WASM binary (inlined hash)
└── index.D47QtDsf.css               # Styles (minified)
```

The WASM binary **stays separate** from the JavaScript bundle (not inlined). Vite automatically renames it with a hash for cache-busting.

### File Sizes (Gzipped)

- HTML: ~300 B
- JavaScript: ~7.7 KB
- CSS: ~1.6 KB
- WASM: ~363 KB
- **Total**: ~373 KB (gzipped)

## Troubleshooting

### Issue: "403 Forbidden" on WASM Load

**Symptom:**
```
GET http://localhost:5173/@fs/Users/it3/.../testaudio_wasm_bg.wasm 403
```

**Cause**: Vite is trying to serve a file system path directly, which violates security restrictions.

**Solution**: The `wasm.ts` fallback handler will attempt alternative paths. Check browser console for logs:
```
Initializing WASM module...
Default WASM init failed, trying alternate paths...
Trying WASM path: /testaudio_wasm_bg.wasm
Trying WASM path: /node_modules/testaudio-wasm/testaudio_wasm_bg.wasm
WASM initialized from: /node_modules/testaudio-wasm/testaudio_wasm_bg.wasm
```

### Issue: WASM Not Found in Production

**Symptom**: App works in dev but fails after deployment.

**Cause**: WASM file not included in deployment.

**Solution**:
1. Ensure `dist/` directory is uploaded completely
2. Check that `testaudio_wasm_bg.*.wasm` file exists in `dist/`
3. Verify web server serves static files correctly
4. Check browser console for actual WASM path being fetched

### Issue: TypeScript Errors with WASM

**Symptom:**
```
Cannot find module 'testaudio-wasm'
```

**Cause**: WASM package not installed.

**Solution**:
```bash
cd web
npm install
```

This installs dependencies including the symlinked WASM package.

## Development Workflow

### Starting Dev Server

```bash
cd web
npm install          # If not already done
npm run dev          # Starts on http://localhost:5173
```

The dev server:
- Watches files for changes (Hot Module Replacement)
- Serves WASM from `node_modules/` via Vite's dev server
- Logs WASM initialization status to console
- Automatically reloads on source changes

### Rebuilding WASM

If you modify the Rust code in `../wasm/src/`:

```bash
# From the wasm directory
cd ../wasm
wasm-pack build --target web

# Then return to web directory
cd ../web
# Dev server automatically picks up the changes
```

### Production Build

```bash
npm run build          # Creates optimized dist/
npm run preview        # Test the build locally
```

## Module Resolution

### Vite's Module Resolution Order

When TypeScript imports `'testaudio-wasm'`:

1. Check `vite.config.ts` `resolve.alias`
2. Resolve to `node_modules/testaudio-wasm`
3. Read `package.json` to find entry point
4. Load JavaScript bindings + WASM binary

### WASM Import in Code

```typescript
// src/utils/wasm.ts
import init, { WasmEncoder, WasmDecoder, ... } from 'testaudio-wasm';

// This resolves to:
// node_modules/testaudio-wasm/testaudio_wasm.js
```

The JavaScript file then loads the `.wasm` binary:

```javascript
// node_modules/testaudio-wasm/testaudio_wasm.js
// Uses import.meta.url to find testaudio_wasm_bg.wasm
```

## Performance Considerations

### WASM Loading

- **Size**: ~363 KB (not gzipped, binary can't be compressed effectively)
- **Load Time**: Varies by network, typically 1-3 seconds on 4G
- **Initialization**: Once loaded, stays in memory (see `wasmInitialized` flag)

### Optimization Tips

1. **Lazy Load**: Only initialize WASM when needed (already done in the app)
2. **Cache**: Browser caches WASM binary using hash-based filenames
3. **Service Workers**: Cache WASM aggressively on repeat visits
4. **Code Splitting**: Only send WASM to pages that use it

## TypeScript Support

### Type Definitions

The WASM package includes TypeScript definitions:

```
node_modules/testaudio-wasm/
├── testaudio_wasm.d.ts        # Type definitions for exported classes
└── testaudio_wasm_bg.wasm.d.ts # Low-level type definitions
```

These are automatically picked up by TypeScript, providing full IDE support:

```typescript
// Full type checking and autocomplete
const encoder = new WasmEncoder();  // ✓ Type safe
const samples = await encoder.encode(data);  // ✓ Knows return type
```

## Common Questions

### Q: Why is the WASM file separate from JavaScript?

**A:** WASM binaries can't be minified or gzip-compressed effectively. Keeping it separate:
- Allows separate caching strategies
- Prevents bloating the main JavaScript bundle
- Enables loading only when needed

### Q: Can I reduce WASM size?

**A:** The WASM binary is generated from Rust:
- Compiled with `--release` in `wasm-pack build` command
- Strip symbols with `wasm-opt` (optional)
- Current size (~363 KB) is optimal for the encoder/decoder functionality

### Q: Does WASM work on all browsers?

**A:** Yes, all modern browsers support WebAssembly:
- Chrome 57+
- Firefox 52+
- Safari 14.1+
- Edge 79+

### Q: How do I test locally before deployment?

**A:** Run the preview server:

```bash
npm run build
npm run preview
# Opens http://localhost:4173
```

This serves the production build locally, exactly as it would be deployed.

## Summary

The Vite + WASM integration uses:

1. **File path dependency** to keep WASM linked to source
2. **Vite's dev server** to serve WASM during development
3. **Bundle-time optimization** for production builds
4. **Fallback initialization** to handle both dev and prod modes
5. **TypeScript definitions** for full IDE support

This setup provides a modern development experience while ensuring the WASM module works reliably in both development and production environments.
