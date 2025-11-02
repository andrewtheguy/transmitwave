use crate::error::{AudioModemError, Result};
use crate::fec::{FecDecoder, FecMode};
use crate::framing::{FrameDecoder};
use crate::dtmf::{DtmfDemodulator, DTMF_NUM_SYMBOLS, DTMF_SYMBOL_SAMPLES};
use crate::sync::{detect_postamble, detect_preamble, DetectionThreshold};
use crate::{PREAMBLE_SAMPLES, SYNC_SILENCE_SAMPLES};

/// Decoder using DTMF tones with Reed-Solomon FEC
///
/// Demodulates DTMF dual-tone symbols using Goertzel algorithm
/// to recover the original binary data.
/// Includes Reed-Solomon error correction for robustness against channel impairments.
pub struct DecoderDtmf {
    dtmf: DtmfDemodulator,
    fec: FecDecoder,
    preamble_threshold: DetectionThreshold,
    postamble_threshold: DetectionThreshold,
}

impl DecoderDtmf {
    pub fn new() -> Result<Self> {
        Ok(Self {
            dtmf: DtmfDemodulator::new(),
            fec: FecDecoder::new()?,
            preamble_threshold: DetectionThreshold::Adaptive,
            postamble_threshold: DetectionThreshold::Adaptive,
        })
    }

    /// Set the detection threshold for preamble detection
    pub fn set_preamble_threshold(&mut self, threshold: DetectionThreshold) {
        self.preamble_threshold = match threshold {
            DetectionThreshold::Adaptive => DetectionThreshold::Adaptive,
            DetectionThreshold::Fixed(value) => DetectionThreshold::Fixed(value.max(0.001).min(1.0)),
        };
    }

    /// Get the current preamble detection threshold
    pub fn get_preamble_threshold(&self) -> DetectionThreshold {
        self.preamble_threshold
    }

    /// Set the detection threshold for postamble detection
    pub fn set_postamble_threshold(&mut self, threshold: DetectionThreshold) {
        self.postamble_threshold = match threshold {
            DetectionThreshold::Adaptive => DetectionThreshold::Adaptive,
            DetectionThreshold::Fixed(value) => DetectionThreshold::Fixed(value.max(0.001).min(1.0)),
        };
    }

    /// Get the current postamble detection threshold
    pub fn get_postamble_threshold(&self) -> DetectionThreshold {
        self.postamble_threshold
    }

    /// Set both preamble and postamble detection thresholds to the same value
    pub fn set_detection_threshold(&mut self, threshold: DetectionThreshold) {
        self.set_preamble_threshold(threshold);
        self.set_postamble_threshold(threshold);
    }

    /// Get the preamble detection threshold
    pub fn get_detection_threshold(&self) -> DetectionThreshold {
        self.get_preamble_threshold()
    }

    /// Decode audio samples back to binary data
    /// Expects: preamble + (DTMF symbols) + postamble
    ///
    /// More forgiving: attempts to decode even with partial data or noise
    /// Handles shortened Reed-Solomon decoding by restoring padding zeros
    /// before RS decoding, then removing them after.
    pub fn decode(&mut self, samples: &[f32]) -> Result<Vec<u8>> {
        // More lenient: just need at least one symbol worth of data
        if samples.len() < DTMF_SYMBOL_SAMPLES {
            return Err(AudioModemError::InsufficientData);
        }

        // Detect preamble to find start of data
        let preamble_pos = detect_preamble(samples, self.preamble_threshold)
            .ok_or(AudioModemError::PreambleNotFound)?;

        // Data starts after preamble + silence gap
        let data_start = preamble_pos + PREAMBLE_SAMPLES + SYNC_SILENCE_SAMPLES;

        // More lenient: just check if we have some data after preamble
        if data_start >= samples.len() {
            return Err(AudioModemError::InsufficientData);
        }

        // Try to detect postamble to find end of data
        let remaining = &samples[data_start..];

        // More lenient: if postamble not found, use all remaining data
        let data_end = if let Some(postamble_pos) = detect_postamble(remaining, self.postamble_threshold) {
            data_start + postamble_pos
        } else {
            // No postamble found, but continue with all remaining data
            samples.len()
        };

        // Extract DTMF data region
        let dtmf_region = &samples[data_start..data_end];

        // More lenient: try to decode even with less data
        if dtmf_region.is_empty() {
            return Err(AudioModemError::InsufficientData);
        }

        // Demodulate DTMF symbols (demodulator handles gaps and skips bad symbols)
        let symbols = self.dtmf.demodulate(dtmf_region)?;

        // Convert DTMF symbols back to bytes
        let bytes = self.dtmf_symbols_to_bytes(&symbols)?;

        if bytes.len() < 2 {
            return Err(AudioModemError::InvalidFrameSize);
        }

        // Read 2-byte length prefix to determine frame data length
        let frame_len = ((bytes[0] as u16) << 8) | (bytes[1] as u16);
        let mut byte_idx = 2;

        // First pass: decode the first block to get FEC mode from header
        let first_chunk_len = (frame_len as usize).min(223);
        let padding_needed_first = 223 - first_chunk_len;

        // Try with different FEC modes to find the right one
        let mut decoded_first_block = None;
        let mut detected_fec_mode = FecMode::Light;

        for mode in [FecMode::Light, FecMode::Medium, FecMode::Full] {
            let parity_bytes = mode.parity_bytes();
            let encoded_len = first_chunk_len + parity_bytes;

            if byte_idx + encoded_len <= bytes.len() {
                let shortened_block = &bytes[byte_idx..byte_idx + encoded_len];
                let mut full_block = vec![0u8; padding_needed_first];
                full_block.extend_from_slice(shortened_block);

                // Try decoding with this FEC mode
                if let Ok(decoded_chunk) = self.fec.decode_with_mode(&full_block, mode) {
                    // Check if this produces a valid header
                    let decoded_data = &decoded_chunk[padding_needed_first..];
                    if decoded_data.len() >= 8 {
                        if let Ok((_, _, fec_mode_byte)) = FrameDecoder::decode_header(decoded_data) {
                            if let Ok(parsed_mode) = FecMode::from_u8(fec_mode_byte) {
                                if parsed_mode == mode {
                                    // Found the correct FEC mode!
                                    decoded_first_block = Some((decoded_data.to_vec(), encoded_len));
                                    detected_fec_mode = mode;
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }

        let (first_decoded, first_encoded_len) = decoded_first_block
            .ok_or(AudioModemError::FecDecodeFailure)?;

        // Now decode remaining blocks using the detected FEC mode
        let mut decoded_data = first_decoded;
        byte_idx += first_encoded_len;
        let mut remaining_len = frame_len as usize - first_chunk_len;

        while remaining_len > 0 {
            let chunk_len = remaining_len.min(223);
            let padding_needed = 223 - chunk_len;
            let parity_bytes = detected_fec_mode.parity_bytes();
            let encoded_len = chunk_len + parity_bytes;

            // Check if we have enough bytes
            if byte_idx + encoded_len > bytes.len() {
                break;
            }

            // Extract the shortened RS block
            let shortened_block = &bytes[byte_idx..byte_idx + encoded_len];
            byte_idx += encoded_len;

            // Restore to full RS block by prepending zeros
            let mut full_block = vec![0u8; padding_needed];
            full_block.extend_from_slice(shortened_block);

            // Decode with RS using detected FEC mode
            match self.fec.decode_with_mode(&full_block, detected_fec_mode) {
                Ok(decoded_chunk) => {
                    // Remove the prepended zeros (padding)
                    decoded_data.extend_from_slice(&decoded_chunk[padding_needed..]);
                }
                Err(_) => {
                    // FEC failed - might be corruption
                    return Err(AudioModemError::FecDecodeFailure);
                }
            }

            remaining_len -= chunk_len;
        }

        if decoded_data.is_empty() {
            return Err(AudioModemError::FecDecodeFailure);
        }

        // Decode frame structure
        let frame = FrameDecoder::decode(&decoded_data)?;

        // Verify frame size is reasonable
        if frame.payload_len as usize > decoded_data.len() {
            return Err(AudioModemError::InvalidFrameSize);
        }

        Ok(frame.payload)
    }

    /// Decode audio samples without preamble/postamble detection
    ///
    /// More forgiving: skips preamble and postamble detection and decodes the raw DTMF data directly.
    /// Useful when the audio clip has already been trimmed.
    /// Attempts to decode even with partial or noisy data.
    pub fn decode_without_preamble_postamble(&mut self, samples: &[f32]) -> Result<Vec<u8>> {
        if samples.is_empty() {
            return Err(AudioModemError::InsufficientData);
        }

        // Demodulate DTMF symbols (demodulator handles gaps and skips bad symbols)
        let symbols = self.dtmf.demodulate(samples)?;

        // Convert DTMF symbols back to bytes
        let bytes = self.dtmf_symbols_to_bytes(&symbols)?;

        if bytes.len() < 2 {
            return Err(AudioModemError::InvalidFrameSize);
        }

        // Read 2-byte length prefix
        let frame_len = ((bytes[0] as u16) << 8) | (bytes[1] as u16);
        let mut byte_idx = 2;

        // Decode with FEC (try all modes)
        let first_chunk_len = (frame_len as usize).min(223);
        let padding_needed_first = 223 - first_chunk_len;

        let mut decoded_first_block = None;
        let mut detected_fec_mode = FecMode::Light;

        for mode in [FecMode::Light, FecMode::Medium, FecMode::Full] {
            let parity_bytes = mode.parity_bytes();
            let encoded_len = first_chunk_len + parity_bytes;

            if byte_idx + encoded_len <= bytes.len() {
                let shortened_block = &bytes[byte_idx..byte_idx + encoded_len];
                let mut full_block = vec![0u8; padding_needed_first];
                full_block.extend_from_slice(shortened_block);

                if let Ok(decoded_chunk) = self.fec.decode_with_mode(&full_block, mode) {
                    let decoded_data = &decoded_chunk[padding_needed_first..];
                    if decoded_data.len() >= 8 {
                        if let Ok((_, _, fec_mode_byte)) = FrameDecoder::decode_header(decoded_data) {
                            if let Ok(parsed_mode) = FecMode::from_u8(fec_mode_byte) {
                                if parsed_mode == mode {
                                    decoded_first_block = Some((decoded_data.to_vec(), encoded_len));
                                    detected_fec_mode = mode;
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }

        let (first_decoded, first_encoded_len) = decoded_first_block
            .ok_or(AudioModemError::FecDecodeFailure)?;

        let mut decoded_data = first_decoded;
        byte_idx += first_encoded_len;
        let mut remaining_len = frame_len as usize - first_chunk_len;

        while remaining_len > 0 {
            let chunk_len = remaining_len.min(223);
            let padding_needed = 223 - chunk_len;
            let parity_bytes = detected_fec_mode.parity_bytes();
            let encoded_len = chunk_len + parity_bytes;

            if byte_idx + encoded_len > bytes.len() {
                break;
            }

            let shortened_block = &bytes[byte_idx..byte_idx + encoded_len];
            byte_idx += encoded_len;

            let mut full_block = vec![0u8; padding_needed];
            full_block.extend_from_slice(shortened_block);

            match self.fec.decode_with_mode(&full_block, detected_fec_mode) {
                Ok(decoded_chunk) => {
                    decoded_data.extend_from_slice(&decoded_chunk[padding_needed..]);
                }
                Err(_) => {
                    return Err(AudioModemError::FecDecodeFailure);
                }
            }

            remaining_len -= chunk_len;
        }

        if decoded_data.is_empty() {
            return Err(AudioModemError::FecDecodeFailure);
        }

        let frame = FrameDecoder::decode(&decoded_data)?;

        if frame.payload_len as usize > decoded_data.len() {
            return Err(AudioModemError::InvalidFrameSize);
        }

        Ok(frame.payload)
    }

    /// Convert DTMF symbols back to bytes
    /// Inverse of bytes_to_dtmf_symbols in encoder
    fn dtmf_symbols_to_bytes(&self, symbols: &[u8]) -> Result<Vec<u8>> {
        if symbols.len() % 2 != 0 {
            return Err(AudioModemError::InvalidInputSize);
        }

        let mut bytes = Vec::new();

        for chunk in symbols.chunks(2) {
            let high = chunk[0];
            let low = chunk[1];

            // Reconstruct byte: byte = high * 48 + low
            let byte_val = high * DTMF_NUM_SYMBOLS + low;
            bytes.push(byte_val);
        }

        Ok(bytes)
    }
}

impl Default for DecoderDtmf {
    fn default() -> Self {
        Self::new().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decoder_dtmf_creation() {
        let decoder = DecoderDtmf::new();
        assert!(decoder.is_ok());
    }

    #[test]
    fn test_dtmf_symbols_to_bytes() {
        let decoder = DecoderDtmf::new().unwrap();

        // Test [0, 0] -> 0
        let bytes = decoder.dtmf_symbols_to_bytes(&[0, 0]).unwrap();
        assert_eq!(bytes, vec![0]);

        // Test [0, 47] -> 47
        let bytes = decoder.dtmf_symbols_to_bytes(&[0, 47]).unwrap();
        assert_eq!(bytes, vec![47]);

        // Test [1, 0] -> 48
        let bytes = decoder.dtmf_symbols_to_bytes(&[1, 0]).unwrap();
        assert_eq!(bytes, vec![48]);

        // Test [5, 15] -> 255
        let bytes = decoder.dtmf_symbols_to_bytes(&[5, 15]).unwrap();
        assert_eq!(bytes, vec![255]);
    }

    #[test]
    fn test_dtmf_symbols_to_bytes_invalid_length() {
        let decoder = DecoderDtmf::new().unwrap();

        // Odd length should fail
        let result = decoder.dtmf_symbols_to_bytes(&[0, 1, 2]);
        assert!(result.is_err());
    }

    #[test]
    fn test_threshold_setters() {
        let mut decoder = DecoderDtmf::new().unwrap();

        // Test adaptive threshold
        decoder.set_preamble_threshold(DetectionThreshold::Adaptive);
        assert_eq!(decoder.get_preamble_threshold(), DetectionThreshold::Adaptive);

        // Test fixed threshold
        decoder.set_preamble_threshold(DetectionThreshold::Fixed(0.5));
        assert_eq!(decoder.get_preamble_threshold(), DetectionThreshold::Fixed(0.5));

        // Test clamping
        decoder.set_preamble_threshold(DetectionThreshold::Fixed(2.0));
        assert_eq!(decoder.get_preamble_threshold(), DetectionThreshold::Fixed(1.0));

        decoder.set_preamble_threshold(DetectionThreshold::Fixed(0.0001));
        assert_eq!(decoder.get_preamble_threshold(), DetectionThreshold::Fixed(0.001));
    }
}
