use transmitwave_core::{detect_postamble, detect_preamble, DecoderFsk, EncoderFsk};
use wasm_bindgen::prelude::*;

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
            .map(|encoder| WasmEncoder { inner: encoder })
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

    /// Get the current number of redundant copies per symbol
    #[wasm_bindgen]
    pub fn redundancy_copies(&self) -> usize {
        self.inner.redundancy_copies()
    }

    /// Set the number of redundant copies per symbol (default: 2)
    #[wasm_bindgen]
    pub fn set_redundancy_copies(&mut self, copies: usize) {
        self.inner.set_redundancy_copies(copies);
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
            .map(|decoder| WasmDecoder { inner: decoder })
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

    /// Get the current number of redundant copies per symbol
    #[wasm_bindgen]
    pub fn redundancy_copies(&self) -> usize {
        self.inner.redundancy_copies()
    }

    /// Set the number of redundant copies per symbol (default: 2)
    /// Must match the encoder setting for proper decoding
    #[wasm_bindgen]
    pub fn set_redundancy_copies(&mut self, copies: usize) {
        self.inner.set_redundancy_copies(copies);
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
            detector: SignalDetector::new(
                threshold,
                transmitwave_core::PREAMBLE_SAMPLES,
                detect_preamble,
            ),
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
            detector: SignalDetector::new(
                threshold,
                transmitwave_core::POSTAMBLE_SAMPLES,
                detect_postamble,
            ),
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
