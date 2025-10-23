use wasm_bindgen::prelude::*;
use testaudio_core::{Decoder, Encoder};

#[wasm_bindgen]
pub struct WasmEncoder {
    inner: Encoder,
}

#[wasm_bindgen]
impl WasmEncoder {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Result<WasmEncoder, JsValue> {
        Encoder::new()
            .map(|encoder| WasmEncoder { inner: encoder })
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Encode binary data into audio samples
    /// Takes a Uint8Array and returns Float32Array of audio samples
    #[wasm_bindgen]
    pub fn encode(&mut self, data: &[u8]) -> Result<Vec<f32>, JsValue> {
        self.inner
            .encode(data)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }
}

#[wasm_bindgen]
pub struct WasmDecoder {
    inner: Decoder,
}

#[wasm_bindgen]
impl WasmDecoder {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Result<WasmDecoder, JsValue> {
        Decoder::new()
            .map(|decoder| WasmDecoder { inner: decoder })
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Decode audio samples back to binary data
    /// Takes a Float32Array and returns Uint8Array of decoded data
    #[wasm_bindgen]
    pub fn decode(&mut self, samples: &[f32]) -> Result<Vec<u8>, JsValue> {
        self.inner
            .decode(samples)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }
}

#[wasm_bindgen(start)]
pub fn init() {
    // Optional panic hook setup
}
