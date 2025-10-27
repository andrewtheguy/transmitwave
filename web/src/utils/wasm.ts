/**
 * WASM module initialization and type definitions
 */

import init, {
    WasmEncoder,
    WasmDecoder,
    WasmFountainEncoder,
    WasmFountainDecoder,
    PreambleDetector,
    PostambleDetector,
} from 'transmitwave-wasm';

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
            '/transmitwave_wasm_bg.wasm',
            // Development: WASM is in node_modules via Vite's alias
            '/node_modules/transmitwave-wasm/transmitwave_wasm_bg.wasm',
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
 * FSK-only mode: maximum reliability for over-the-air audio transmission
 */
export {
    WasmEncoder,
    WasmDecoder,
    WasmFountainEncoder,
    WasmFountainDecoder,
    PreambleDetector,
    PostambleDetector,
};

/**
 * Utility types for WASM encoding/decoding
 * FSK-only mode for maximum reliability
 */
export interface EncoderOptions {
    // FSK is the only supported mode for over-the-air audio transmission
}

export interface DecoderOptions {
    // FSK is the only supported mode for over-the-air audio transmission
    detectionThreshold?: number | null; // Optional: null = adaptive (auto-adjust), 0.1-1.0 = fixed threshold
}

/**
 * Factory function to create an FSK encoder
 * FSK-only mode ensures maximum reliability for over-the-air audio transmission
 */
export async function createEncoder(
    options: EncoderOptions = {}
): Promise<WasmEncoder> {
    await initWasm();
    return new WasmEncoder();
}

/**
 * Factory function to create an FSK decoder
 * FSK-only mode ensures maximum reliability for over-the-air audio transmission
 */
export async function createDecoder(
    options: DecoderOptions = {}
): Promise<WasmDecoder> {
    await initWasm();
    const decoder = new WasmDecoder();

    // Set detection threshold if provided
    // null = adaptive (pass 0.0 to Rust), number = fixed threshold
    if (options.detectionThreshold !== undefined) {
        const threshold = options.detectionThreshold === null ? 0.0 : options.detectionThreshold;
        decoder.set_detection_threshold(threshold);
    }

    return decoder;
}

/**
 * Factory function to create a fountain encoder
 */
export async function createFountainEncoder(): Promise<WasmFountainEncoder> {
    await initWasm();
    return new WasmFountainEncoder();
}

/**
 * Factory function to create a fountain decoder
 */
export async function createFountainDecoder(
    options: DecoderOptions = {}
): Promise<WasmFountainDecoder> {
    await initWasm();
    const decoder = new WasmFountainDecoder();

    // Set detection threshold if provided
    // null = adaptive (pass 0.0 to Rust), number = fixed threshold
    if (options.detectionThreshold !== undefined) {
        const threshold = options.detectionThreshold === null ? 0.0 : options.detectionThreshold;
        decoder.set_detection_threshold(threshold);
    }

    return decoder;
}
