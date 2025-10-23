use wasm_bindgen::prelude::*;
use testaudio_core::{Decoder, Encoder, DecoderSpread, EncoderSpread, detect_preamble, detect_postamble};

/// Default WASM Encoder (now uses spread spectrum)
#[wasm_bindgen]
pub struct WasmEncoder {
    inner: EncoderSpread,
}

#[wasm_bindgen]
impl WasmEncoder {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Result<WasmEncoder, JsValue> {
        EncoderSpread::new(2)
            .map(|encoder| WasmEncoder { inner: encoder })
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Encode binary data into audio samples with spread spectrum
    /// Takes a Uint8Array and returns Float32Array of audio samples
    #[wasm_bindgen]
    pub fn encode(&mut self, data: &[u8]) -> Result<Vec<f32>, JsValue> {
        self.inner
            .encode(data)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }
}

/// Default WASM Decoder (now uses spread spectrum)
#[wasm_bindgen]
pub struct WasmDecoder {
    inner: DecoderSpread,
}

#[wasm_bindgen]
impl WasmDecoder {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Result<WasmDecoder, JsValue> {
        DecoderSpread::new(2)
            .map(|decoder| WasmDecoder { inner: decoder })
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Decode audio samples back to binary data with spread spectrum
    /// Takes a Float32Array and returns Uint8Array of decoded data
    #[wasm_bindgen]
    pub fn decode(&mut self, samples: &[f32]) -> Result<Vec<u8>, JsValue> {
        self.inner
            .decode(samples)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }
}

/// Legacy WASM Encoder (without spread spectrum) for backwards compatibility
#[wasm_bindgen]
pub struct WasmEncoderLegacy {
    inner: Encoder,
}

#[wasm_bindgen]
impl WasmEncoderLegacy {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Result<WasmEncoderLegacy, JsValue> {
        Encoder::new()
            .map(|encoder| WasmEncoderLegacy { inner: encoder })
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Encode binary data into audio samples (legacy, no spread spectrum)
    #[wasm_bindgen]
    pub fn encode(&mut self, data: &[u8]) -> Result<Vec<f32>, JsValue> {
        self.inner
            .encode(data)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }
}

/// Legacy WASM Decoder (without spread spectrum) for backwards compatibility
#[wasm_bindgen]
pub struct WasmDecoderLegacy {
    inner: Decoder,
}

#[wasm_bindgen]
impl WasmDecoderLegacy {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Result<WasmDecoderLegacy, JsValue> {
        Decoder::new()
            .map(|decoder| WasmDecoderLegacy { inner: decoder })
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Decode audio samples back to binary data (legacy, no spread spectrum)
    #[wasm_bindgen]
    pub fn decode(&mut self, samples: &[f32]) -> Result<Vec<u8>, JsValue> {
        self.inner
            .decode(samples)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }
}

/// Microphone listener for detecting preamble in real-time audio stream
#[wasm_bindgen]
pub struct MicrophoneListener {
    audio_buffer: Vec<f32>,
    threshold: f32,
}

#[wasm_bindgen]
impl MicrophoneListener {
    /// Create a new microphone listener with specified threshold
    /// threshold: minimum correlation coefficient (0.0 - 1.0) to detect preamble
    /// Recommended: 0.4 for reliable detection
    #[wasm_bindgen(constructor)]
    pub fn new(threshold: f32) -> MicrophoneListener {
        MicrophoneListener {
            audio_buffer: Vec::new(),
            threshold: threshold.max(0.0).min(1.0),
        }
    }

    /// Add audio samples from microphone to the buffer
    /// Returns the detected preamble position if found, or -1 if not detected
    #[wasm_bindgen]
    pub fn add_samples(&mut self, samples: &[f32]) -> i32 {
        // Append new samples to buffer
        self.audio_buffer.extend_from_slice(samples);

        // Only check for preamble if we have enough samples
        if self.audio_buffer.len() < testaudio_core::PREAMBLE_SAMPLES {
            return -1;
        }

        // Try to detect preamble in current buffer
        match detect_preamble(&self.audio_buffer, self.threshold) {
            Some(pos) => {
                // Clear buffer up to detection point for next search
                let pos_usize = pos as usize;
                if pos_usize + testaudio_core::PREAMBLE_SAMPLES <= self.audio_buffer.len() {
                    self.audio_buffer.drain(0..pos_usize);
                }
                pos as i32
            }
            None => -1,
        }
    }

    /// Get current buffer size (for monitoring)
    #[wasm_bindgen]
    pub fn buffer_size(&self) -> usize {
        self.audio_buffer.len()
    }

    /// Get required buffer size to detect preamble
    #[wasm_bindgen]
    pub fn required_size() -> usize {
        testaudio_core::PREAMBLE_SAMPLES
    }

    /// Clear the audio buffer
    #[wasm_bindgen]
    pub fn clear(&mut self) {
        self.audio_buffer.clear();
    }

    /// Get the current threshold value
    #[wasm_bindgen]
    pub fn threshold(&self) -> f32 {
        self.threshold
    }

    /// Set a new threshold value
    #[wasm_bindgen]
    pub fn set_threshold(&mut self, threshold: f32) {
        self.threshold = threshold.max(0.0).min(1.0);
    }
}

/// Postamble detector for detecting end-of-frame marker in audio stream
#[wasm_bindgen]
pub struct PostambleDetector {
    audio_buffer: Vec<f32>,
    threshold: f32,
}

#[wasm_bindgen]
impl PostambleDetector {
    /// Create a new postamble detector with specified threshold
    /// threshold: minimum correlation coefficient (0.0 - 1.0) to detect postamble
    /// Recommended: 0.4 for reliable detection
    #[wasm_bindgen(constructor)]
    pub fn new(threshold: f32) -> PostambleDetector {
        PostambleDetector {
            audio_buffer: Vec::new(),
            threshold: threshold.max(0.0).min(1.0),
        }
    }

    /// Add audio samples from microphone to the buffer
    /// Returns the detected postamble position if found, or -1 if not detected
    #[wasm_bindgen]
    pub fn add_samples(&mut self, samples: &[f32]) -> i32 {
        // Append new samples to buffer
        self.audio_buffer.extend_from_slice(samples);

        // Only check for postamble if we have enough samples
        if self.audio_buffer.len() < testaudio_core::POSTAMBLE_SAMPLES {
            return -1;
        }

        // Try to detect postamble in current buffer
        match detect_postamble(&self.audio_buffer, self.threshold) {
            Some(pos) => {
                // Clear buffer up to detection point for next search
                let pos_usize = pos as usize;
                if pos_usize + testaudio_core::POSTAMBLE_SAMPLES <= self.audio_buffer.len() {
                    self.audio_buffer.drain(0..pos_usize);
                }
                pos as i32
            }
            None => -1,
        }
    }

    /// Get current buffer size (for monitoring)
    #[wasm_bindgen]
    pub fn buffer_size(&self) -> usize {
        self.audio_buffer.len()
    }

    /// Get required buffer size to detect postamble
    #[wasm_bindgen]
    pub fn required_size() -> usize {
        testaudio_core::POSTAMBLE_SAMPLES
    }

    /// Clear the audio buffer
    #[wasm_bindgen]
    pub fn clear(&mut self) {
        self.audio_buffer.clear();
    }

    /// Get the current threshold value
    #[wasm_bindgen]
    pub fn threshold(&self) -> f32 {
        self.threshold
    }

    /// Set a new threshold value
    #[wasm_bindgen]
    pub fn set_threshold(&mut self, threshold: f32) {
        self.threshold = threshold.max(0.0).min(1.0);
    }
}

#[wasm_bindgen(start)]
pub fn init() {
    // Optional panic hook setup
}
