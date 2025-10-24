use crate::error::{AudioModemError, Result};
use crate::css::CssDemodulator;
use crate::fec::FecDecoder;
use crate::framing::FrameDecoder;
use crate::sync::{detect_postamble, detect_preamble};
use crate::{PREAMBLE_SAMPLES, RS_TOTAL_BYTES, CSS_SAMPLES_PER_SYMBOL};

pub struct DecoderCss {
    css: CssDemodulator,
    fec: FecDecoder,
}

impl DecoderCss {
    pub fn new() -> Result<Self> {
        Ok(Self {
            css: CssDemodulator::new()?,
            fec: FecDecoder::new()?,
        })
    }

    /// Decode audio samples back to binary data using CSS demodulation
    /// Expects: preamble + CSS-modulated data + postamble
    pub fn decode(&mut self, samples: &[f32]) -> Result<Vec<u8>> {
        if samples.len() < CSS_SAMPLES_PER_SYMBOL * 2 {
            return Err(AudioModemError::InsufficientData);
        }

        // Detect preamble
        let preamble_pos = detect_preamble(samples, 500.0)
            .ok_or(AudioModemError::PreambleNotFound)?;

        // Start reading data after preamble
        let data_start = preamble_pos + PREAMBLE_SAMPLES;

        if data_start + CSS_SAMPLES_PER_SYMBOL > samples.len() {
            return Err(AudioModemError::InsufficientData);
        }

        // Try to detect postamble in remaining samples
        let remaining = &samples[data_start..];
        let postamble_pos = detect_postamble(remaining, 100.0)
            .ok_or(AudioModemError::PostambleNotFound)?;

        let data_end = data_start + postamble_pos;

        // Demodulate all CSS symbols between data_start and data_end
        let data_samples = &samples[data_start..data_end];
        let mut bits = self.css.demodulate(data_samples)?;

        if bits.is_empty() {
            return Err(AudioModemError::InvalidFrameSize);
        }

        // Trim bits to complete bytes (multiple of 8)
        let complete_bytes = bits.len() / 8;
        bits.truncate(complete_bytes * 8);

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

        // Trim bytes to exact multiple of RS_TOTAL_BYTES (don't pad with zeros)
        // This ensures we don't feed malformed RS blocks to the decoder
        while bytes.len() % RS_TOTAL_BYTES != 0 {
            bytes.pop();
        }

        if bytes.len() < RS_TOTAL_BYTES {
            return Err(AudioModemError::InvalidFrameSize);
        }

        // Decode FEC
        let mut decoded_data = Vec::new();
        for chunk in bytes.chunks(RS_TOTAL_BYTES) {
            let fec_result = self.fec.decode(chunk)?;
            decoded_data.extend_from_slice(&fec_result);
        }

        // Decode frame
        let frame = FrameDecoder::decode(&decoded_data)?;
        Ok(frame.payload)
    }
}

impl Default for DecoderCss {
    fn default() -> Self {
        Self::new().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::EncoderCss;

    #[test]
    fn test_round_trip_simple_message() {
        let mut encoder = EncoderCss::new().unwrap();
        let mut decoder = DecoderCss::new().unwrap();

        let original_data = b"Hello";
        let samples = encoder.encode(original_data).unwrap();
        let decoded_data = decoder.decode(&samples).unwrap();

        assert_eq!(original_data.to_vec(), decoded_data, "Round-trip failed for simple message");
    }

    #[test]
    fn test_round_trip_longer_message() {
        let mut encoder = EncoderCss::new().unwrap();
        let mut decoder = DecoderCss::new().unwrap();

        let original_data = b"The quick brown fox jumps over the lazy dog";
        let samples = encoder.encode(original_data).unwrap();
        let decoded_data = decoder.decode(&samples).unwrap();

        assert_eq!(original_data.to_vec(), decoded_data);
    }

    #[test]
    fn test_round_trip_binary_data() {
        let mut encoder = EncoderCss::new().unwrap();
        let mut decoder = DecoderCss::new().unwrap();

        let original_data: Vec<u8> = (0..100).map(|i| (i * 17 + 42) as u8).collect();
        let samples = encoder.encode(&original_data).unwrap();
        let decoded_data = decoder.decode(&samples).unwrap();

        assert_eq!(original_data, decoded_data);
    }

    #[test]
    fn test_round_trip_all_bytes() {
        let mut encoder = EncoderCss::new().unwrap();
        let mut decoder = DecoderCss::new().unwrap();

        // Test various byte patterns
        let test_cases = vec![
            vec![0x00],
            vec![0xFF],
            vec![0xAA, 0x55],
            (0..256).map(|i| i as u8).collect::<Vec<u8>>()[0..100].to_vec(),
        ];

        for original_data in test_cases {
            let samples = encoder.encode(&original_data).unwrap();
            let decoded_data = decoder.decode(&samples).unwrap();
            assert_eq!(original_data, decoded_data, "Round-trip failed for pattern");
        }
    }

    #[test]
    fn test_round_trip_minimum_payload() {
        let mut encoder = EncoderCss::new().unwrap();
        let mut decoder = DecoderCss::new().unwrap();

        let original_data = vec![42u8];
        let samples = encoder.encode(&original_data).unwrap();
        let decoded_data = decoder.decode(&samples).unwrap();

        assert_eq!(original_data, decoded_data);
    }

    #[test]
    fn test_round_trip_maximum_payload() {
        let mut encoder = EncoderCss::new().unwrap();
        let mut decoder = DecoderCss::new().unwrap();

        let original_data = vec![0u8; crate::MAX_PAYLOAD_SIZE];
        let samples = encoder.encode(&original_data).unwrap();
        let decoded_data = decoder.decode(&samples).unwrap();

        assert_eq!(original_data, decoded_data);
    }

    #[test]
    fn test_decoder_insufficient_samples() {
        let mut decoder = DecoderCss::new().unwrap();

        // Too few samples to contain preamble and postamble
        let short_samples = vec![0.0; 1000];
        let result = decoder.decode(&short_samples);

        // Should error on missing preamble or postamble
        assert!(result.is_err());
    }

    #[test]
    fn test_decoder_empty_input() {
        let mut decoder = DecoderCss::new().unwrap();

        let result = decoder.decode(&[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_round_trip_multiple_encodes() {
        let mut encoder = EncoderCss::new().unwrap();
        let mut decoder = DecoderCss::new().unwrap();

        let messages = vec![
            b"First".to_vec(),
            b"Second message".to_vec(),
            b"Third".to_vec(),
        ];

        for original_data in messages {
            let samples = encoder.encode(&original_data).unwrap();
            let decoded_data = decoder.decode(&samples).unwrap();
            assert_eq!(original_data, decoded_data);
        }
    }

    #[test]
    fn test_round_trip_with_repetition() {
        let mut encoder = EncoderCss::new().unwrap();
        let mut decoder = DecoderCss::new().unwrap();

        // Test encoding/decoding the same data multiple times
        let original_data = b"Repeat test";

        for _ in 0..5 {
            let samples = encoder.encode(original_data).unwrap();
            let decoded_data = decoder.decode(&samples).unwrap();
            assert_eq!(original_data.to_vec(), decoded_data);
        }
    }

    #[test]
    fn test_round_trip_preserves_data_integrity() {
        let mut encoder = EncoderCss::new().unwrap();
        let mut decoder = DecoderCss::new().unwrap();

        // Create data with specific byte patterns
        let mut original_data = Vec::new();
        for i in 0..100 {
            original_data.push((i ^ 0xAB) as u8);
        }

        let samples = encoder.encode(&original_data).unwrap();
        let decoded_data = decoder.decode(&samples).unwrap();

        // Verify every single byte
        assert_eq!(original_data.len(), decoded_data.len());
        for (i, (orig, decoded)) in original_data.iter().zip(decoded_data.iter()).enumerate() {
            assert_eq!(orig, decoded, "Byte {} mismatch: {} vs {}", i, orig, decoded);
        }
    }

    #[test]
    fn test_round_trip_various_data_lengths() {
        let mut encoder = EncoderCss::new().unwrap();
        let mut decoder = DecoderCss::new().unwrap();

        // Test different data lengths
        for len in vec![1, 2, 5, 10, 50, 100, 150, crate::MAX_PAYLOAD_SIZE] {
            let original_data = vec![0x42; len];
            let samples = encoder.encode(&original_data).unwrap();
            let decoded_data = decoder.decode(&samples).unwrap();

            assert_eq!(original_data, decoded_data, "Round-trip failed for length {}", len);
        }
    }
}
