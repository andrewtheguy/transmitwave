/**
 * WASM module initialization and type definitions
 */

import init, {
    WasmEncoder,
    WasmDecoder,
    WasmEncoderSpread,
    WasmDecoderSpread,
    WasmEncoderLegacy,
    WasmDecoderLegacy,
    PreambleDetector,
    PostambleDetector,
} from 'testaudio-wasm';

let wasmInitialized = false;

/**
 * Initialize the WASM module
 */
export async function initWasm(): Promise<void> {
    if (wasmInitialized) {
        return;
    }

    try {
        // First try: let the WASM binding figure out the path (works in production)
        console.log('Initializing WASM module...');
        await init();
        wasmInitialized = true;
        console.log('WASM initialized successfully');
    } catch (error) {
        // Fallback: manually fetch from known locations
        console.log('Default WASM init failed, trying alternate paths...', error);

        const possiblePaths = [
            // Production: WASM is bundled in dist
            '/testaudio_wasm_bg.wasm',
            // Development: WASM is in node_modules via Vite's alias
            '/node_modules/testaudio-wasm/testaudio_wasm_bg.wasm',
        ];

        for (const wasmPath of possiblePaths) {
            try {
                console.log(`Trying WASM path: ${wasmPath}`);
                const response = await fetch(wasmPath);
                if (!response.ok) {
                    console.log(`Path not found: ${wasmPath} (${response.status})`);
                    continue;
                }
                await init(response);
                wasmInitialized = true;
                console.log(`WASM initialized from: ${wasmPath}`);
                return;
            } catch (err) {
                console.log(`Failed to load from ${wasmPath}:`, err);
                continue;
            }
        }

        // All paths failed
        throw new Error(`WASM initialization failed: could not load from any path`);
    }
}

/**
 * Check if WASM is initialized
 */
export function isWasmInitialized(): boolean {
    return wasmInitialized;
}

/**
 * Export WASM classes for use in the application
 */
export {
    WasmEncoder,
    WasmDecoder,
    WasmEncoderSpread,
    WasmDecoderSpread,
    WasmEncoderLegacy,
    WasmDecoderLegacy,
    PreambleDetector,
    PostambleDetector,
};

/**
 * Utility types for WASM encoding/decoding
 */
export interface EncoderOptions {
    type?: 'spread' | 'legacy' | 'chunked';
    chipDuration?: number;
    chunkBits?: number;
    interleaveFactor?: number;
}

export interface DecoderOptions {
    type?: 'spread' | 'legacy' | 'chunked';
    chipDuration?: number;
    chunkBits?: number;
}

/**
 * Factory function to create an encoder based on options
 */
export async function createEncoder(
    options: EncoderOptions = {}
): Promise<WasmEncoder | WasmEncoderLegacy | WasmEncoderSpread> {
    await initWasm();

    const { type = 'spread' } = options;

    if (type === 'legacy') {
        return new WasmEncoderLegacy();
    } else if (type === 'spread') {
        return new WasmEncoderSpread();
    } else {
        return new WasmEncoder();
    }
}

/**
 * Factory function to create a decoder based on options
 */
export async function createDecoder(
    options: DecoderOptions = {}
): Promise<WasmDecoder | WasmDecoderLegacy | WasmDecoderSpread> {
    await initWasm();

    const { type = 'spread' } = options;

    if (type === 'legacy') {
        return new WasmDecoderLegacy();
    } else if (type === 'spread') {
        return new WasmDecoderSpread();
    } else {
        return new WasmDecoder();
    }
}
