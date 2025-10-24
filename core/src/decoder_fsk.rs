use crate::error::{AudioModemError, Result};
use crate::fec::FecDecoder;
use crate::framing::FrameDecoder;
use crate::fsk::FskDemodulator;
use crate::sync::{detect_postamble, detect_preamble};
use crate::{PREAMBLE_SAMPLES, RS_TOTAL_BYTES, SAMPLES_PER_SYMBOL};

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
    pub fn decode(&mut self, samples: &[f32]) -> Result<Vec<u8>> {
        if samples.len() < SAMPLES_PER_SYMBOL * 2 {
            return Err(AudioModemError::InsufficientData);
        }

        // Detect preamble to find start of data
        let preamble_pos = detect_preamble(samples, 500.0)
            .ok_or(AudioModemError::PreambleNotFound)?;

        // Data starts after preamble
        let data_start = preamble_pos + PREAMBLE_SAMPLES;

        if data_start + SAMPLES_PER_SYMBOL > samples.len() {
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
        let symbol_count = fsk_region.len() / SAMPLES_PER_SYMBOL;
        if symbol_count == 0 {
            return Err(AudioModemError::InsufficientData);
        }

        let valid_samples = symbol_count * SAMPLES_PER_SYMBOL;
        let fsk_samples = &fsk_region[..valid_samples];

        // Demodulate FSK symbols to bits
        let bits = self.fsk.demodulate(fsk_samples)?;

        if bits.is_empty() {
            return Err(AudioModemError::InvalidFrameSize);
        }

        // Convert bits back to bytes
        let mut bytes = Vec::new();
        for chunk in bits.chunks(8) {
            if chunk.len() == 8 {
                let mut byte = 0u8;
                for (i, &bit) in chunk.iter().enumerate() {
                    if bit {
                        byte |= 1 << (7 - i);
                    }
                }
                bytes.push(byte);
            }
        }

        if bytes.is_empty() {
            return Err(AudioModemError::InvalidFrameSize);
        }

        // Decode FEC (Reed-Solomon)
        let mut decoded_data = Vec::new();
        for chunk in bytes.chunks(RS_TOTAL_BYTES) {
            if chunk.len() == RS_TOTAL_BYTES {
                match self.fec.decode(chunk) {
                    Ok(decoded_chunk) => {
                        decoded_data.extend_from_slice(&decoded_chunk);
                    }
                    Err(_) => {
                        // FEC failed - might be partial frame or corruption
                        // Try to continue with remaining blocks
                        continue;
                    }
                }
            }
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
