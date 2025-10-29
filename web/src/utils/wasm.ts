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
    // Enable hybrid chirp FSK for improved noise robustness
    // Trades some CPU for better multipath/interference immunity
    useChirp?: boolean;
}

export interface DecoderOptions {
    // FSK is the only supported mode for over-the-air audio transmission
    // Optional threshold settings (defaults to 0.4 for both)
    preambleThreshold?: number;
    postambleThreshold?: number;
    // Enable hybrid chirp FSK demodulation (must match encoder)
    useChirp?: boolean;
}

/**
 * Factory function to create an FSK encoder
 * FSK-only mode ensures maximum reliability for over-the-air audio transmission
 */
export async function createEncoder(options: EncoderOptions = {}): Promise<WasmEncoder> {
    await initWasm();
    const useChirp = options.useChirp ?? false;

    if (useChirp) {
        return (WasmEncoder as any).newWithChirp();
    } else {
        return new WasmEncoder();
    }
}

/**
 * Factory function to create an FSK decoder
 * FSK-only mode ensures maximum reliability for over-the-air audio transmission
 * Uses Fixed(0.4) threshold by default for both preamble and postamble detection
 */
export async function createDecoder(
    options: DecoderOptions = {}
): Promise<WasmDecoder> {
    await initWasm();
    const useChirp = options.useChirp ?? false;

    // Create decoder with appropriate mode (chirp or standard)
    const decoder = useChirp
        ? await (WasmDecoder as any).newWithChirp()
        : new WasmDecoder();

    // Set thresholds with 0.4 as default
    const preambleThreshold = options.preambleThreshold ?? 0.4;
    const postambleThreshold = options.postambleThreshold ?? 0.4;

    decoder.set_preamble_threshold(preambleThreshold);
    decoder.set_postamble_threshold(postambleThreshold);

    return decoder;
}

/**
 * Factory function to create a fountain encoder
 */
export async function createFountainEncoder(options: EncoderOptions = {}): Promise<WasmFountainEncoder> {
    await initWasm();
    const useChirp = options.useChirp ?? false;

    if (useChirp) {
        return (WasmFountainEncoder as any).newWithChirp();
    } else {
        return new WasmFountainEncoder();
    }
}

/**
 * Factory function to create a fountain decoder
 * Supports preamble threshold configuration only (fountain mode has no postamble)
 */
export async function createFountainDecoder(
    options: DecoderOptions = {}
): Promise<WasmFountainDecoder> {
    await initWasm();
    const useChirp = options.useChirp ?? false;
    const preambleThreshold = options.preambleThreshold ?? 0.4;

    // Create decoder with appropriate mode (chirp or standard)
    const decoder = useChirp
        ? await (WasmFountainDecoder as any).newWithChirp()
        : new WasmFountainDecoder();

    // Set preamble threshold (clamped to valid range)
    decoder.set_preamble_threshold(Math.max(0.1, Math.min(0.9, preambleThreshold)));

    return decoder;
}
