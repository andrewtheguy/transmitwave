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
    MicrophoneListener,
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
        await init();
        wasmInitialized = true;
    } catch (error) {
        console.error('Failed to initialize WASM:', error);
        throw new Error('WASM initialization failed');
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
    MicrophoneListener,
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
