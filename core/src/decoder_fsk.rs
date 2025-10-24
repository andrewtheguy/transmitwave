use crate::error::{AudioModemError, Result};
use crate::fec::{FecDecoder, FecMode};
use crate::framing::FrameDecoder;
use crate::fsk::FskDemodulator;
use crate::sync::{detect_postamble, detect_preamble};
use crate::PREAMBLE_SAMPLES;
use crate::fsk::FSK_SYMBOL_SAMPLES;

/// Decoder using 4-FSK (Four-Frequency Shift Keying)
///
/// Demodulates 4-FSK symbols using non-coherent energy detection (Goertzel algorithm)
/// to recover the original binary data. Highly robust to noise and distortion.
pub struct DecoderFsk {
    fsk: FskDemodulator,
    fec: FecDecoder,
}

impl DecoderFsk {
    pub fn new() -> Result<Self> {
        Ok(Self {
            fsk: FskDemodulator::new(),
            fec: FecDecoder::new()?,
        })
    }

    /// Decode audio samples back to binary data
    /// Expects: preamble + (FSK symbols) + postamble
    ///
    /// Handles shortened Reed-Solomon decoding by restoring padding zeros
    /// before RS decoding, then removing them after.
    pub fn decode(&mut self, samples: &[f32]) -> Result<Vec<u8>> {
        if samples.len() < FSK_SYMBOL_SAMPLES * 2 {
            return Err(AudioModemError::InsufficientData);
        }

        // Detect preamble to find start of data
        let preamble_pos = detect_preamble(samples, 500.0)
            .ok_or(AudioModemError::PreambleNotFound)?;

        // Data starts after preamble
        let data_start = preamble_pos + PREAMBLE_SAMPLES;

        if data_start + FSK_SYMBOL_SAMPLES > samples.len() {
            return Err(AudioModemError::InsufficientData);
        }

        // Try to detect postamble to find end of data
        let remaining = &samples[data_start..];
        let postamble_pos = detect_postamble(remaining, 100.0)
            .ok_or(AudioModemError::PostambleNotFound)?;

        let data_end = data_start + postamble_pos;

        // Extract FSK data region
        let fsk_region = &samples[data_start..data_end];

        // Ensure we have complete symbols
        let symbol_count = fsk_region.len() / FSK_SYMBOL_SAMPLES;
        if symbol_count == 0 {
            return Err(AudioModemError::InsufficientData);
        }

        let valid_samples = symbol_count * FSK_SYMBOL_SAMPLES;
        let fsk_samples = &fsk_region[..valid_samples];

        // Demodulate multi-tone FSK symbols to bytes
        let bytes = self.fsk.demodulate(fsk_samples)?;

        if bytes.len() < 2 {
            return Err(AudioModemError::InvalidFrameSize);
        }

        // Read 2-byte length prefix to determine frame data length
        let frame_len = ((bytes[0] as u16) << 8) | (bytes[1] as u16);
        let mut byte_idx = 2;

        // First pass: decode the first block to get FEC mode from header
        // We need to peek at the header to determine FEC mode
        let first_chunk_len = (frame_len as usize).min(223);
        let padding_needed_first = 223 - first_chunk_len;

        // Try with different FEC modes to find the right one
        // Start with Light (smallest overhead) and work up
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
}

impl Default for DecoderFsk {
    fn default() -> Self {
        Self::new().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encoder_fsk::EncoderFsk;

    #[test]
    fn test_decoder_fsk_basic_roundtrip() {
        let mut encoder = EncoderFsk::new().unwrap();
        let mut decoder = DecoderFsk::new().unwrap();

        let data = b"Hello FSK!";
        let samples = encoder.encode(data).unwrap();
        let decoded = decoder.decode(&samples).unwrap();

        assert_eq!(decoded, data);
    }

    #[test]
    fn test_decoder_fsk_empty_data() {
        let mut encoder = EncoderFsk::new().unwrap();
        let mut decoder = DecoderFsk::new().unwrap();

        let data = b"";
        let samples = encoder.encode(data).unwrap();
        let decoded = decoder.decode(&samples).unwrap();

        assert_eq!(decoded, data);
    }

    #[test]
    fn test_decoder_fsk_various_lengths() {
        let mut encoder = EncoderFsk::new().unwrap();
        let mut decoder = DecoderFsk::new().unwrap();

        let test_cases = vec![
            b"A".to_vec(),
            b"AB".to_vec(),
            b"ABC".to_vec(),
            b"The quick brown fox".to_vec(),
            vec![0u8; 50],
            vec![255u8; 30],
        ];

        for data in test_cases {
            let samples = encoder.encode(&data).unwrap();
            let decoded = decoder.decode(&samples).unwrap();
            assert_eq!(decoded, data, "Failed for data length {}", data.len());
        }
    }

    #[test]
    fn test_decoder_fsk_binary_data() {
        let mut encoder = EncoderFsk::new().unwrap();
        let mut decoder = DecoderFsk::new().unwrap();

        // Test binary data (not just ASCII)
        let data: Vec<u8> = (0..100).map(|i| i as u8).collect();
        let samples = encoder.encode(&data).unwrap();
        let decoded = decoder.decode(&samples).unwrap();

        assert_eq!(decoded, data);
    }

    #[test]
    fn test_decoder_fsk_with_noise() {
        let mut encoder = EncoderFsk::new().unwrap();
        let mut decoder = DecoderFsk::new().unwrap();

        let data = b"Noisy channel test";
        let mut samples = encoder.encode(data).unwrap();

        // Add small amount of noise (5% of signal)
        for sample in samples.iter_mut() {
            let noise = (*sample * 123.456).sin() * 0.05;
            *sample += noise;
        }

        let decoded = decoder.decode(&samples).unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn test_decoder_fsk_insufficient_data() {
        let mut decoder = DecoderFsk::new().unwrap();

        // Too few samples
        let samples = vec![0.0; 100];
        let result = decoder.decode(&samples);
        assert!(result.is_err());
    }

    #[test]
    fn test_decoder_fsk_no_preamble() {
        let mut decoder = DecoderFsk::new().unwrap();

        // Random noise without preamble
        let samples: Vec<f32> = (0..10000).map(|i| (i as f32 * 0.1).sin() * 0.1).collect();
        let result = decoder.decode(&samples);
        assert!(result.is_err());
    }

    #[test]
    fn test_decoder_fsk_large_payload() {
        let mut encoder = EncoderFsk::new().unwrap();
        let mut decoder = DecoderFsk::new().unwrap();

        // Test with larger payload (near maximum)
        let data = vec![42u8; 180];
        let samples = encoder.encode(&data).unwrap();
        let decoded = decoder.decode(&samples).unwrap();

        assert_eq!(decoded, data);
    }

    #[test]
    fn test_decoder_fsk_repeating_patterns() {
        let mut encoder = EncoderFsk::new().unwrap();
        let mut decoder = DecoderFsk::new().unwrap();

        // Test with repeating byte patterns
        let patterns = vec![
            vec![0x00; 20],      // All zeros
            vec![0xFF; 20],      // All ones
            vec![0xAA; 20],      // Alternating bits
            vec![0x55; 20],      // Alternating bits (inverse)
        ];

        for data in patterns {
            let samples = encoder.encode(&data).unwrap();
            let decoded = decoder.decode(&samples).unwrap();
            assert_eq!(decoded, data, "Failed for pattern {:02X}", data[0]);
        }
    }
}
