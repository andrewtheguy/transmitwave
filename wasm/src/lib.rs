use wasm_bindgen::prelude::*;
use transmitwave_core::{DecoderFsk, EncoderFsk, FountainConfig, detect_preamble, detect_postamble};
use transmitwave_core::decoder_fsk::DecodeStats;
use transmitwave_core::sync::DetectionThreshold;

// ============================================================================
// DECODE STATISTICS
// ============================================================================

/// Decode statistics exposed to JavaScript
#[wasm_bindgen]
pub struct WasmDecodeStats {
    /// Number of successfully decoded blocks (passed CRC)
    pub decoded_blocks: u32,
    /// Number of blocks that failed CRC check (corrupted)
    pub failed_blocks: u32,
}

#[wasm_bindgen]
impl WasmDecodeStats {
    /// Create a new decode statistics object
    #[wasm_bindgen(constructor)]
    pub fn new(decoded_blocks: u32, failed_blocks: u32) -> WasmDecodeStats {
        WasmDecodeStats {
            decoded_blocks,
            failed_blocks,
        }
    }
}

impl From<DecodeStats> for WasmDecodeStats {
    fn from(stats: DecodeStats) -> Self {
        WasmDecodeStats {
            decoded_blocks: stats.decoded_blocks,
            failed_blocks: stats.failed_blocks,
        }
    }
}

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

    /// Set the detection threshold for both preamble and postamble
    #[wasm_bindgen]
    pub fn set_detection_threshold(&mut self, fixed_value: f32) {
        let threshold = DetectionThreshold::Fixed(fixed_value.max(0.001).min(1.0));
        self.inner.set_detection_threshold(threshold);
    }

    /// Set the detection threshold for preamble only
    #[wasm_bindgen]
    pub fn set_preamble_threshold(&mut self, fixed_value: f32) {
        let threshold = DetectionThreshold::Fixed(fixed_value.max(0.001).min(1.0));
        self.inner.set_preamble_threshold(threshold);
    }

    /// Get the current preamble detection threshold
    #[wasm_bindgen]
    pub fn get_preamble_threshold(&self) -> f32 {
        match self.inner.get_preamble_threshold() {
            DetectionThreshold::Fixed(value) => value,
            DetectionThreshold::Adaptive => panic!("WASM should only use Fixed threshold, not Adaptive"),
        }
    }

    /// Set the detection threshold for postamble only
    #[wasm_bindgen]
    pub fn set_postamble_threshold(&mut self, fixed_value: f32) {
        let threshold = DetectionThreshold::Fixed(fixed_value.max(0.001).min(1.0));
        self.inner.set_postamble_threshold(threshold);
    }

    /// Get the current postamble detection threshold
    #[wasm_bindgen]
    pub fn get_postamble_threshold(&self) -> f32 {
        match self.inner.get_postamble_threshold() {
            DetectionThreshold::Fixed(value) => value,
            DetectionThreshold::Adaptive => panic!("WASM should only use Fixed threshold, not Adaptive"),
        }
    }

    /// Decode audio samples back to binary data with FSK
    /// Takes a Float32Array and returns Uint8Array of decoded data
    #[wasm_bindgen]
    pub fn decode(&mut self, samples: &[f32]) -> Result<Vec<u8>, JsValue> {
        self.inner
            .decode(samples)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Decode audio samples without preamble/postamble detection
    ///
    /// This method skips preamble and postamble detection and decodes the raw FSK data directly.
    /// Useful when the audio clip has already been trimmed or when pre/post amble detection
    /// would cause double-detection issues.
    /// Takes a Float32Array and returns Uint8Array of decoded data
    #[wasm_bindgen]
    pub fn decode_without_preamble_postamble(&mut self, samples: &[f32]) -> Result<Vec<u8>, JsValue> {
        self.inner
            .decode_without_preamble_postamble(samples)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }
}


// ============================================================================
// SIGNAL DETECTION (PREAMBLE & POSTAMBLE)
// ============================================================================

/// Generic signal detector for preamble/postamble detection
struct SignalDetector<F> {
    audio_buffer: Vec<f32>,
    threshold: DetectionThreshold,
    required_samples: usize,
    detect_fn: F,
}

impl<F> SignalDetector<F>
where
    F: Fn(&[f32], DetectionThreshold) -> Option<usize>,
{
    fn new(threshold: DetectionThreshold, required_samples: usize, detect_fn: F) -> Self {
        SignalDetector {
            audio_buffer: Vec::new(),
            threshold,
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
        match self.threshold {
            DetectionThreshold::Fixed(v) => v,
            DetectionThreshold::Adaptive => panic!("WASM should only use Fixed threshold, not Adaptive"),
        }
    }

    fn set_threshold(&mut self, threshold_enum: DetectionThreshold) {
        self.threshold = threshold_enum;
    }
}

/// Preamble detector for detecting start-of-frame marker in real-time audio stream
#[wasm_bindgen]
pub struct PreambleDetector {
    detector: SignalDetector<fn(&[f32], DetectionThreshold) -> Option<usize>>,
}

#[wasm_bindgen]
impl PreambleDetector {
    /// Create a new preamble detector with specified threshold
    #[wasm_bindgen(constructor)]
    pub fn new(fixed_value: f32) -> PreambleDetector {
        let threshold = DetectionThreshold::Fixed(fixed_value.max(0.001).min(1.0));
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
    pub fn set_threshold(&mut self, fixed_value: f32) {
        let threshold = DetectionThreshold::Fixed(fixed_value.max(0.001).min(1.0));
        self.detector.set_threshold(threshold);
    }
}

/// Postamble detector for detecting end-of-frame marker in audio stream
#[wasm_bindgen]
pub struct PostambleDetector {
    detector: SignalDetector<fn(&[f32], DetectionThreshold) -> Option<usize>>,
}

#[wasm_bindgen]
impl PostambleDetector {
    /// Create a new postamble detector with specified threshold
    #[wasm_bindgen(constructor)]
    pub fn new(fixed_value: f32) -> PostambleDetector {
        let threshold = DetectionThreshold::Fixed(fixed_value.max(0.001).min(1.0));
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
    pub fn set_threshold(&mut self, fixed_value: f32) {
        let threshold = DetectionThreshold::Fixed(fixed_value.max(0.001).min(1.0));
        self.detector.set_threshold(threshold);
    }
}


// ============================================================================
// FOUNTAIN CODE ENCODER/DECODER
// Continuous streaming mode using RaptorQ fountain codes (RFC 6330)
// ============================================================================

/// Fountain Code Encoder for continuous streaming
#[wasm_bindgen]
pub struct WasmFountainEncoder {
    inner: EncoderFsk,
}

#[wasm_bindgen]
impl WasmFountainEncoder {
    /// Create a new fountain encoder
    #[wasm_bindgen(constructor)]
    pub fn new() -> Result<WasmFountainEncoder, JsValue> {
        EncoderFsk::new()
            .map(|encoder| WasmFountainEncoder {
                inner: encoder,
            })
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Encode data into fountain-coded audio stream
    /// Returns a flat Float32Array of all audio samples (concatenated blocks)
    ///
    /// Parameters:
    /// - data: Input data to encode
    /// - timeout_secs: Audio duration in seconds (e.g., 30)
    /// - block_size: Symbol size in bytes (e.g., 64)
    /// - repair_ratio: Repair packet overhead (e.g., 0.5 for 50%)
    #[wasm_bindgen]
    pub fn encode_fountain(
        &mut self,
        data: &[u8],
        timeout_secs: u32,
        block_size: usize,
        repair_ratio: f32,
    ) -> Result<Vec<f32>, JsValue> {
        let config = FountainConfig {
            timeout_secs,
            block_size,
            repair_blocks_ratio: repair_ratio,
        };

        let stream = self.inner
            .encode_fountain(data, Some(config))
            .map_err(|e| JsValue::from_str(&e.to_string()))?;

        // Collect all blocks and concatenate into single audio buffer
        let all_samples: Vec<f32> = stream
            .flat_map(|block| block)
            .collect();

        Ok(all_samples)
    }
}

/// Fountain Code Decoder for continuous streaming
#[wasm_bindgen]
pub struct WasmFountainDecoder {
    inner: DecoderFsk,
    buffer: Vec<f32>,
    block_size: usize,
}

#[wasm_bindgen]
impl WasmFountainDecoder {
    /// Create a new fountain decoder
    #[wasm_bindgen(constructor)]
    pub fn new() -> Result<WasmFountainDecoder, JsValue> {
        DecoderFsk::new()
            .map(|decoder| WasmFountainDecoder {
                inner: decoder,
                buffer: Vec::new(),
                block_size: 64, // Default block size
            })
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Set the block size for decoding
    #[wasm_bindgen]
    pub fn set_block_size(&mut self, block_size: usize) {
        self.block_size = block_size;
    }

    /// Set the detection threshold for both preamble and postamble
    #[wasm_bindgen]
    pub fn set_detection_threshold(&mut self, fixed_value: f32) {
        let threshold = DetectionThreshold::Fixed(fixed_value.max(0.001).min(1.0));
        self.inner.set_detection_threshold(threshold);
    }

    /// Set the detection threshold for preamble only
    #[wasm_bindgen]
    pub fn set_preamble_threshold(&mut self, fixed_value: f32) {
        let threshold = DetectionThreshold::Fixed(fixed_value.max(0.001).min(1.0));
        self.inner.set_preamble_threshold(threshold);
    }

    /// Get the current preamble detection threshold
    #[wasm_bindgen]
    pub fn get_preamble_threshold(&self) -> f32 {
        match self.inner.get_preamble_threshold() {
            DetectionThreshold::Fixed(value) => value,
            DetectionThreshold::Adaptive => panic!("WASM should only use Fixed threshold, not Adaptive"),
        }
    }

    /// Set the detection threshold for postamble only
    #[wasm_bindgen]
    pub fn set_postamble_threshold(&mut self, fixed_value: f32) {
        let threshold = DetectionThreshold::Fixed(fixed_value.max(0.001).min(1.0));
        self.inner.set_postamble_threshold(threshold);
    }

    /// Get the current postamble detection threshold
    #[wasm_bindgen]
    pub fn get_postamble_threshold(&self) -> f32 {
        match self.inner.get_postamble_threshold() {
            DetectionThreshold::Fixed(value) => value,
            DetectionThreshold::Adaptive => panic!("WASM should only use Fixed threshold, not Adaptive"),
        }
    }

    /// Feed audio chunk to the decoder buffer
    #[wasm_bindgen]
    pub fn feed_chunk(&mut self, samples: &[f32]) {
        self.buffer.extend_from_slice(samples);
    }

    /// Get the current number of samples in the buffer
    #[wasm_bindgen]
    pub fn get_sample_count(&self) -> usize {
        self.buffer.len()
    }

    /// Try to decode the accumulated audio buffer
    /// Returns decoded data if successful, or error if decoding fails
    #[wasm_bindgen]
    pub fn try_decode(&mut self) -> Result<Vec<u8>, JsValue> {
        if self.buffer.is_empty() {
            return Err(JsValue::from_str("No audio data in buffer"));
        }

        let config = FountainConfig {
            timeout_secs: 30, // Not enforced in WASM
            block_size: self.block_size,
            repair_blocks_ratio: 0.5, // Not used by decoder
        };

        self.inner
            .decode_fountain(&self.buffer, Some(config))
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Reset the decoder and clear the buffer.
    ///
    /// Returns an error if decoder initialization fails. On success, both the
    /// buffer and decoder state are cleared. On failure, the decoder state
    /// is left unchanged and the buffer is cleared.
    #[wasm_bindgen]
    pub fn reset(&mut self) -> Result<(), JsValue> {
        self.buffer.clear();
        // Create a new inner decoder to reset its state
        DecoderFsk::new()
            .map(|decoder| {
                self.inner = decoder;
            })
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Get the number of successfully decoded blocks
    #[wasm_bindgen]
    pub fn get_decoded_blocks(&self) -> u32 {
        self.inner.stats.decoded_blocks
    }

    /// Get the number of blocks that failed CRC check
    #[wasm_bindgen]
    pub fn get_failed_blocks(&self) -> u32 {
        self.inner.stats.failed_blocks
    }

    /// Get all decode statistics as a WasmDecodeStats object
    #[wasm_bindgen]
    pub fn get_stats(&self) -> WasmDecodeStats {
        WasmDecodeStats::from(self.inner.stats.clone())
    }

    /// Decode fountain-coded audio stream back to data (non-streaming mode)
    ///
    /// Parameters:
    /// - samples: Audio samples from microphone/recording
    /// - timeout_secs: Maximum time to spend decoding (e.g., 30)
    /// - block_size: Symbol size in bytes (must match encoder, e.g., 64)
    #[wasm_bindgen]
    pub fn decode_fountain(
        &mut self,
        samples: &[f32],
        timeout_secs: u32,
        block_size: usize,
    ) -> Result<Vec<u8>, JsValue> {
        let config = FountainConfig {
            timeout_secs,
            block_size,
            repair_blocks_ratio: 0.5, // Not used by decoder
        };

        self.inner
            .decode_fountain(samples, Some(config))
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }
}


#[wasm_bindgen(start)]
pub fn init() {
    // Optional panic hook setup
}
