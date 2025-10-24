use crate::error::Result;
use crate::css::CssModulator;
use crate::fec::FecEncoder;
use crate::framing::{Frame, FrameEncoder};
use crate::sync::{generate_chirp, generate_postamble};
use crate::{MAX_PAYLOAD_SIZE, PREAMBLE_SAMPLES, POSTAMBLE_SAMPLES};

pub struct EncoderCss {
    css: CssModulator,
    fec: FecEncoder,
}

impl EncoderCss {
    pub fn new() -> Result<Self> {
        Ok(Self {
            css: CssModulator::new()?,
            fec: FecEncoder::new()?,
        })
    }

    /// Encode binary data into audio samples using CSS modulation
    /// Returns: preamble + CSS-modulated data + postamble
    pub fn encode(&mut self, data: &[u8]) -> Result<Vec<f32>> {
        if data.len() > MAX_PAYLOAD_SIZE {
            return Err(crate::error::AudioModemError::InvalidInputSize);
        }

        // Create frame
        let frame = Frame {
            payload_len: data.len() as u16,
            frame_num: 0,
            payload: data.to_vec(),
        };

        let frame_data = FrameEncoder::encode(&frame)?;

        // Encode each byte with FEC
        let mut encoded_data = Vec::new();
        for chunk in frame_data.chunks(223) {
            let fec_chunk = self.fec.encode(chunk)?;
            encoded_data.extend_from_slice(&fec_chunk);
        }

        // Convert bytes to bits for CSS
        let mut bits = Vec::new();
        for byte in encoded_data {
            for i in (0..8).rev() {
                bits.push((byte >> i) & 1 == 1);
            }
        }

        // Generate preamble (chirp)
        let preamble = generate_chirp(PREAMBLE_SAMPLES, 200.0, 4000.0, 0.5);

        // Modulate data bits using CSS
        let mut samples = preamble;
        let css_samples = self.css.modulate(&bits)?;
        samples.extend_from_slice(&css_samples);

        // Generate postamble
        let postamble = generate_postamble(POSTAMBLE_SAMPLES, 0.5);
        samples.extend_from_slice(&postamble);

        Ok(samples)
    }
}

impl Default for EncoderCss {
    fn default() -> Self {
        Self::new().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encoder_creates_valid_output() {
        let mut encoder = EncoderCss::new().unwrap();
        let data = b"Hello, World!";
        let samples = encoder.encode(data).unwrap();

        // Output should have preamble + data + postamble
        // Minimum: 4000 (preamble) + 800 (at least one data symbol) + 4000 (postamble)
        assert!(samples.len() > 8800, "Output too short");

        // All samples should be in valid range
        for &sample in &samples {
            assert!(sample.abs() <= 1.0, "Sample {} out of range", sample);
        }
    }

    #[test]
    fn test_encoder_empty_data() {
        let mut encoder = EncoderCss::new().unwrap();
        let data = b"";
        let samples = encoder.encode(data).unwrap();

        // Even empty data should produce preamble + encoded frame + postamble
        assert!(samples.len() > 0);
    }

    #[test]
    fn test_encoder_max_payload_size() {
        let mut encoder = EncoderCss::new().unwrap();
        let data = vec![0u8; crate::MAX_PAYLOAD_SIZE];
        let result = encoder.encode(&data);

        assert!(result.is_ok(), "Should accept max payload size");
    }

    #[test]
    fn test_encoder_exceeds_max_payload_size() {
        let mut encoder = EncoderCss::new().unwrap();
        let data = vec![0u8; crate::MAX_PAYLOAD_SIZE + 1];
        let result = encoder.encode(&data);

        assert!(result.is_err(), "Should reject payload exceeding max size");
    }

    #[test]
    fn test_encoder_various_payloads() {
        let mut encoder = EncoderCss::new().unwrap();

        let test_cases = vec![
            b"A".to_vec(),
            b"Test".to_vec(),
            b"The quick brown fox jumps over the lazy dog".to_vec(),
            vec![0x00; 100],
            vec![0xFF; 100],
            (0..200).map(|i| (i % 256) as u8).collect::<Vec<u8>>(),
        ];

        for data in test_cases {
            let result = encoder.encode(&data);
            assert!(result.is_ok(), "Failed to encode data of length {}", data.len());

            let samples = result.unwrap();
            assert!(samples.len() > 0);

            // Verify all samples are in valid range
            for &sample in &samples {
                assert!(sample.abs() <= 1.0, "Invalid sample value");
            }
        }
    }
}
