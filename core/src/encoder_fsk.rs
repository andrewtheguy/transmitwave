use crate::error::Result;
use crate::fec::{FecEncoder, FecMode};
use crate::framing::{Frame, FrameEncoder, crc16};
use crate::fsk::FskModulator;
use crate::sync::{generate_preamble, generate_postamble_signal};
use crate::{MAX_PAYLOAD_SIZE, PREAMBLE_SAMPLES, POSTAMBLE_SAMPLES};

/// Encoder using 4-FSK (Four-Frequency Shift Keying)
///
/// Uses 4 distinct audio frequencies (1200, 1600, 2000, 2400 Hz) to encode
/// 2 bits per symbol with non-coherent detection for maximum reliability
/// in over-the-air transmission scenarios.
///
/// Benefits:
/// - Highly robust to noise and distortion
/// - No phase synchronization required (non-coherent detection)
/// - Well-suited for speaker-to-microphone transmission
/// - Slower data rate but higher reliability than OFDM
pub struct EncoderFsk {
    fsk: FskModulator,
    fec: FecEncoder,
}

impl EncoderFsk {
    pub fn new() -> Result<Self> {
        Ok(Self {
            fsk: FskModulator::new(),
            fec: FecEncoder::new()?,
        })
    }

    /// Encode binary data into audio samples using 4-FSK modulation
    /// Returns: preamble + (FSK symbols) + postamble
    ///
    /// Uses variable Reed-Solomon parity based on payload size:
    /// - Small payloads (< 20 bytes): 8 parity bytes (75% less overhead)
    /// - Medium payloads (20-50 bytes): 16 parity bytes (50% less overhead)
    /// - Large payloads (> 50 bytes): 32 parity bytes (full protection)
    pub fn encode(&mut self, data: &[u8]) -> Result<Vec<f32>> {
        if data.len() > MAX_PAYLOAD_SIZE {
            return Err(crate::error::AudioModemError::InvalidInputSize);
        }

        // Create frame with header and CRC (without FEC mode yet)
        let payload = data.to_vec();

        // Determine FEC mode based on frame size (header + payload + CRC)
        let frame_data_size = 8 + data.len() + 2; // header(8) + payload + crc16(2)
        let fec_mode = FecMode::from_data_size(frame_data_size);

        let frame = Frame {
            payload_len: data.len() as u16,
            frame_num: 0,
            fec_mode: fec_mode.to_u8(),
            payload: payload.clone(),
            payload_crc: crc16(&payload),
        };

        let frame_data = FrameEncoder::encode(&frame)?;

        // Apply variable shortened Reed-Solomon FEC encoding
        let mut encoded_data = Vec::new();

        // Add 2-byte length prefix so decoder knows the frame data length
        let frame_len = frame_data.len() as u16;
        encoded_data.push((frame_len >> 8) as u8);
        encoded_data.push(frame_len as u8);

        for chunk in frame_data.chunks(223) {
            let chunk_len = chunk.len();

            // Shortened RS: prepend zeros, encode, remove zeros
            // This avoids transmitting padding bytes for small payloads
            let padding_needed = 223 - chunk_len;

            // Create padded data for RS encoder
            let mut padded = vec![0u8; padding_needed];
            padded.extend_from_slice(chunk);

            // Encode with variable RS parity based on FEC mode
            let fec_chunk = self.fec.encode_with_mode(&padded, fec_mode)?;

            // Only transmit: actual data + parity (skip the prepended zeros)
            // Parity size depends on FEC mode (8, 16, or 32 bytes)
            encoded_data.extend_from_slice(&fec_chunk[padding_needed..]);
        }

        // Convert bytes to bits for FSK modulation
        let mut bits = Vec::new();
        for byte in encoded_data {
            for i in (0..8).rev() {
                bits.push((byte >> i) & 1 == 1);
            }
        }

        // Ensure even number of bits (FSK encodes 2 bits per symbol)
        if bits.len() % 2 != 0 {
            bits.push(false); // Pad with zero if odd
        }

        // Generate preamble signal for synchronization
        let preamble = generate_preamble(PREAMBLE_SAMPLES, 0.5);

        // Modulate data bits using 4-FSK
        let mut samples = preamble;
        let fsk_samples = self.fsk.modulate(&bits)?;
        samples.extend_from_slice(&fsk_samples);

        // Generate postamble signal for frame boundary detection
        let postamble = generate_postamble_signal(POSTAMBLE_SAMPLES, 0.5);
        samples.extend_from_slice(&postamble);

        Ok(samples)
    }
}

impl Default for EncoderFsk {
    fn default() -> Self {
        Self::new().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encoder_fsk_basic() {
        let mut encoder = EncoderFsk::new().unwrap();
        let data = b"Hello";
        let samples = encoder.encode(data).unwrap();

        // Should have: preamble + FSK data + postamble
        assert!(samples.len() > PREAMBLE_SAMPLES + POSTAMBLE_SAMPLES);
    }

    #[test]
    fn test_encoder_fsk_empty_data() {
        let mut encoder = EncoderFsk::new().unwrap();
        let data = b"";
        let result = encoder.encode(data);
        // Empty data should still work (frame header only)
        assert!(result.is_ok());
    }

    #[test]
    fn test_encoder_fsk_max_payload() {
        let mut encoder = EncoderFsk::new().unwrap();
        let data = vec![0u8; MAX_PAYLOAD_SIZE];
        let result = encoder.encode(&data);
        assert!(result.is_ok());
    }

    #[test]
    fn test_encoder_fsk_exceeds_max_payload() {
        let mut encoder = EncoderFsk::new().unwrap();
        let data = vec![0u8; MAX_PAYLOAD_SIZE + 1];
        let result = encoder.encode(&data);
        assert!(result.is_err());
    }

    #[test]
    fn test_encoder_fsk_structure() {
        let mut encoder = EncoderFsk::new().unwrap();
        let data = b"Test";
        let samples = encoder.encode(data).unwrap();

        // Verify preamble is at the start (should be non-zero)
        let preamble_slice = &samples[0..PREAMBLE_SAMPLES];
        let preamble_has_signal = preamble_slice.iter().any(|&s| s.abs() > 0.01);
        assert!(preamble_has_signal, "Preamble should contain signal");

        // Verify postamble is at the end
        let postamble_start = samples.len() - POSTAMBLE_SAMPLES;
        let postamble_slice = &samples[postamble_start..];
        let postamble_has_signal = postamble_slice.iter().any(|&s| s.abs() > 0.01);
        assert!(postamble_has_signal, "Postamble should contain signal");
    }

    #[test]
    fn test_encoder_fsk_deterministic() {
        let mut encoder1 = EncoderFsk::new().unwrap();
        let mut encoder2 = EncoderFsk::new().unwrap();
        let data = b"Deterministic test";

        let samples1 = encoder1.encode(data).unwrap();
        let samples2 = encoder2.encode(data).unwrap();

        // Same input should produce same output
        assert_eq!(samples1.len(), samples2.len());
        for (i, (&s1, &s2)) in samples1.iter().zip(samples2.iter()).enumerate() {
            assert!(
                (s1 - s2).abs() < 1e-6,
                "Mismatch at sample {}: {} vs {}",
                i,
                s1,
                s2
            );
        }
    }
}
