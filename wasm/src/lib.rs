use wasm_bindgen::prelude::*;
use transmitwave_core::{Decoder, Encoder, DecoderSpread, EncoderSpread, DecoderFsk, EncoderFsk, detect_preamble, detect_postamble};

// ============================================================================
// DEFAULT ENCODER/DECODER CONFIGURATION
// Default mode: Multi-tone FSK (ggwave-compatible) for maximum reliability
// ============================================================================

/// Default WASM Encoder (uses FSK for maximum reliability)
#[wasm_bindgen]
pub struct WasmEncoder {
    inner: EncoderFsk,
}

#[wasm_bindgen]
impl WasmEncoder {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Result<WasmEncoder, JsValue> {
        EncoderFsk::new()
            .map(|encoder| WasmEncoder {
                inner: encoder,
            })
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Encode binary data into audio samples with FSK
    /// Takes a Uint8Array and returns Float32Array of audio samples
    #[wasm_bindgen]
    pub fn encode(&mut self, data: &[u8]) -> Result<Vec<f32>, JsValue> {
        self.inner
            .encode(data)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }
}

/// Default WASM Decoder (uses FSK for maximum reliability)
#[wasm_bindgen]
pub struct WasmDecoder {
    inner: DecoderFsk,
}

#[wasm_bindgen]
impl WasmDecoder {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Result<WasmDecoder, JsValue> {
        DecoderFsk::new()
            .map(|decoder| WasmDecoder {
                inner: decoder,
            })
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Decode audio samples back to binary data with FSK
    /// Takes a Float32Array and returns Uint8Array of decoded data
    #[wasm_bindgen]
    pub fn decode(&mut self, samples: &[f32]) -> Result<Vec<u8>, JsValue> {
        self.inner
            .decode(samples)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }
}

/// Spread spectrum WASM Encoder (for backwards compatibility)
#[wasm_bindgen]
pub struct WasmEncoderSpread {
    inner: EncoderSpread,
}

#[wasm_bindgen]
impl WasmEncoderSpread {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Result<WasmEncoderSpread, JsValue> {
        EncoderSpread::new(2)
            .map(|encoder| WasmEncoderSpread { inner: encoder })
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Encode binary data into audio samples with spread spectrum
    #[wasm_bindgen]
    pub fn encode(&mut self, data: &[u8]) -> Result<Vec<f32>, JsValue> {
        self.inner
            .encode(data)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }
}

/// Spread spectrum WASM Decoder (for backwards compatibility)
#[wasm_bindgen]
pub struct WasmDecoderSpread {
    inner: DecoderSpread,
}

#[wasm_bindgen]
impl WasmDecoderSpread {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Result<WasmDecoderSpread, JsValue> {
        DecoderSpread::new(2)
            .map(|decoder| WasmDecoderSpread { inner: decoder })
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Decode audio samples back to binary data with spread spectrum
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

/// FSK WASM Encoder (for explicit FSK encoding)
#[wasm_bindgen]
pub struct WasmEncoderFsk {
    inner: EncoderFsk,
}

#[wasm_bindgen]
impl WasmEncoderFsk {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Result<WasmEncoderFsk, JsValue> {
        EncoderFsk::new()
            .map(|encoder| WasmEncoderFsk { inner: encoder })
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Encode binary data into audio samples with FSK
    #[wasm_bindgen]
    pub fn encode(&mut self, data: &[u8]) -> Result<Vec<f32>, JsValue> {
        self.inner
            .encode(data)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }
}

/// FSK WASM Decoder (for explicit FSK decoding)
#[wasm_bindgen]
pub struct WasmDecoderFsk {
    inner: DecoderFsk,
}

#[wasm_bindgen]
impl WasmDecoderFsk {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Result<WasmDecoderFsk, JsValue> {
        DecoderFsk::new()
            .map(|decoder| WasmDecoderFsk { inner: decoder })
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Decode audio samples back to binary data with FSK
    #[wasm_bindgen]
    pub fn decode(&mut self, samples: &[f32]) -> Result<Vec<u8>, JsValue> {
        self.inner
            .decode(samples)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }
}

// ============================================================================
// SIGNAL DETECTION (PREAMBLE & POSTAMBLE)
// ============================================================================

/// Generic signal detector for preamble/postamble detection
struct SignalDetector<F> {
    audio_buffer: Vec<f32>,
    threshold: f32,
    required_samples: usize,
    detect_fn: F,
}

impl<F> SignalDetector<F>
where
    F: Fn(&[f32], f32) -> Option<usize>,
{
    fn new(threshold: f32, required_samples: usize, detect_fn: F) -> Self {
        SignalDetector {
            audio_buffer: Vec::new(),
            threshold: threshold.max(0.0).min(1.0),
            required_samples,
            detect_fn,
        }
    }

    fn add_samples(&mut self, samples: &[f32]) -> i32 {
        self.audio_buffer.extend_from_slice(samples);

        if self.audio_buffer.len() < self.required_samples {
            return -1;
        }

        match (self.detect_fn)(&self.audio_buffer, self.threshold) {
            Some(pos) => {
                let pos_usize = pos as usize;
                if pos_usize + self.required_samples <= self.audio_buffer.len() {
                    self.audio_buffer.drain(0..pos_usize);
                }
                pos as i32
            }
            None => -1,
        }
    }

    fn buffer_size(&self) -> usize {
        self.audio_buffer.len()
    }

    fn clear(&mut self) {
        self.audio_buffer.clear();
    }

    fn threshold(&self) -> f32 {
        self.threshold
    }

    fn set_threshold(&mut self, threshold: f32) {
        self.threshold = threshold.max(0.0).min(1.0);
    }
}

/// Preamble detector for detecting start-of-frame marker in real-time audio stream
#[wasm_bindgen]
pub struct PreambleDetector {
    detector: SignalDetector<fn(&[f32], f32) -> Option<usize>>,
}

#[wasm_bindgen]
impl PreambleDetector {
    /// Create a new preamble detector with specified threshold
    /// threshold: minimum correlation coefficient (0.0 - 1.0) to detect preamble
    /// Recommended: 0.4 for reliable detection
    #[wasm_bindgen(constructor)]
    pub fn new(threshold: f32) -> PreambleDetector {
        PreambleDetector {
            detector: SignalDetector::new(threshold, transmitwave_core::PREAMBLE_SAMPLES, detect_preamble),
        }
    }

    /// Add audio samples from microphone to the buffer
    /// Returns the detected preamble position if found, or -1 if not detected
    #[wasm_bindgen]
    pub fn add_samples(&mut self, samples: &[f32]) -> i32 {
        self.detector.add_samples(samples)
    }

    /// Get current buffer size (for monitoring)
    #[wasm_bindgen]
    pub fn buffer_size(&self) -> usize {
        self.detector.buffer_size()
    }

    /// Get required buffer size to detect preamble
    #[wasm_bindgen]
    pub fn required_size() -> usize {
        transmitwave_core::PREAMBLE_SAMPLES
    }

    /// Clear the audio buffer
    #[wasm_bindgen]
    pub fn clear(&mut self) {
        self.detector.clear();
    }

    /// Get the current threshold value
    #[wasm_bindgen]
    pub fn threshold(&self) -> f32 {
        self.detector.threshold()
    }

    /// Set a new threshold value
    #[wasm_bindgen]
    pub fn set_threshold(&mut self, threshold: f32) {
        self.detector.set_threshold(threshold);
    }
}

/// Postamble detector for detecting end-of-frame marker in audio stream
#[wasm_bindgen]
pub struct PostambleDetector {
    detector: SignalDetector<fn(&[f32], f32) -> Option<usize>>,
}

#[wasm_bindgen]
impl PostambleDetector {
    /// Create a new postamble detector with specified threshold
    /// threshold: minimum correlation coefficient (0.0 - 1.0) to detect postamble
    /// Recommended: 0.4 for reliable detection
    #[wasm_bindgen(constructor)]
    pub fn new(threshold: f32) -> PostambleDetector {
        PostambleDetector {
            detector: SignalDetector::new(threshold, transmitwave_core::POSTAMBLE_SAMPLES, detect_postamble),
        }
    }

    /// Add audio samples from microphone to the buffer
    /// Returns the detected postamble position if found, or -1 if not detected
    #[wasm_bindgen]
    pub fn add_samples(&mut self, samples: &[f32]) -> i32 {
        self.detector.add_samples(samples)
    }

    /// Get current buffer size (for monitoring)
    #[wasm_bindgen]
    pub fn buffer_size(&self) -> usize {
        self.detector.buffer_size()
    }

    /// Get required buffer size to detect postamble
    #[wasm_bindgen]
    pub fn required_size() -> usize {
        transmitwave_core::POSTAMBLE_SAMPLES
    }

    /// Clear the audio buffer
    #[wasm_bindgen]
    pub fn clear(&mut self) {
        self.detector.clear();
    }

    /// Get the current threshold value
    #[wasm_bindgen]
    pub fn threshold(&self) -> f32 {
        self.detector.threshold()
    }

    /// Set a new threshold value
    #[wasm_bindgen]
    pub fn set_threshold(&mut self, threshold: f32) {
        self.detector.set_threshold(threshold);
    }
}


#[wasm_bindgen(start)]
pub fn init() {
    // Optional panic hook setup
}
