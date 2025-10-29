use crate::error::Result;
use crate::fec::{FecEncoder, FecMode};
use crate::framing::{Frame, FrameEncoder, crc16};
use crate::dtmf::{DtmfModulator, DTMF_NUM_SYMBOLS};
use crate::sync::{generate_preamble, generate_postamble_signal};
use crate::{MAX_PAYLOAD_SIZE, PREAMBLE_SAMPLES, POSTAMBLE_SAMPLES, SYNC_SILENCE_SAMPLES};

/// Encoder using DTMF tones with Reed-Solomon FEC
///
/// Uses dual-tone DTMF signaling to encode 48 different symbols (0-47)
/// with Goertzel algorithm detection for maximum reliability
/// in over-the-air transmission scenarios.
///
/// Benefits:
/// - Highly robust to noise and distortion
/// - No phase synchronization required (non-coherent detection)
/// - Well-suited for speaker-to-microphone transmission
/// - Standard DTMF frequency range (697-1633 Hz)
/// - Extended symbol set (48 vs standard 16)
pub struct EncoderDtmf {
    dtmf: DtmfModulator,
    fec: FecEncoder,
}

impl EncoderDtmf {
    pub fn new() -> Result<Self> {
        Ok(Self {
            dtmf: DtmfModulator::new(),
            fec: FecEncoder::new()?,
        })
    }

    /// Encode binary data into audio samples using DTMF modulation
    /// Returns: silence + preamble + silence + DTMF data + silence + postamble + silence
    ///
    /// Each symbol encodes ~5.6 bits (48 possible values out of 64)
    /// Data is packed: 8 bits -> convert to base-48 representation
    ///
    /// Uses variable Reed-Solomon parity based on payload size:
    /// - Small payloads (< 20 bytes): 8 parity bytes (75% less overhead)
    /// - Medium payloads (20-50 bytes): 16 parity bytes (50% less overhead)
    /// - Large payloads (> 50 bytes): 32 parity bytes (full protection)
    pub fn encode(&mut self, data: &[u8]) -> Result<Vec<f32>> {
        if data.len() > MAX_PAYLOAD_SIZE {
            return Err(crate::error::AudioModemError::InvalidInputSize);
        }

        // Create frame with header and CRC
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
            let padding_needed = 223 - chunk_len;

            // Create padded data for RS encoder
            let mut padded = vec![0u8; padding_needed];
            padded.extend_from_slice(chunk);

            // Encode with variable RS parity based on FEC mode
            let fec_chunk = self.fec.encode_with_mode(&padded, fec_mode)?;

            // Only transmit: actual data + parity (skip the prepended zeros)
            encoded_data.extend_from_slice(&fec_chunk[padding_needed..]);
        }

        // Convert bytes to DTMF symbols
        // Each symbol can represent 0-47 (5.585 bits of data)
        // We use a simple mapping: each byte becomes multiple symbols
        let dtmf_symbols = self.bytes_to_dtmf_symbols(&encoded_data);

        // Generate preamble signal for synchronization
        let preamble = generate_preamble(PREAMBLE_SAMPLES, 0.5);

        // Build frame: silence → preamble → silence → DTMF payload → silence → postamble → silence
        let mut samples = Vec::new();

        // Add silence before preamble for clean frame start
        samples.extend_from_slice(&vec![0.0f32; SYNC_SILENCE_SAMPLES]);

        // Add preamble for synchronization
        samples.extend_from_slice(&preamble);

        // Add silence after preamble for symmetry and clear frame boundaries
        samples.extend_from_slice(&vec![0.0f32; SYNC_SILENCE_SAMPLES]);

        // Modulate DTMF symbols
        let dtmf_samples = self.dtmf.modulate(&dtmf_symbols)?;
        samples.extend_from_slice(&dtmf_samples);

        // Add silence before postamble to separate payload from end marker
        samples.extend_from_slice(&vec![0.0f32; SYNC_SILENCE_SAMPLES]);

        // Generate postamble signal for frame boundary detection
        let postamble = generate_postamble_signal(POSTAMBLE_SAMPLES, 0.5);
        samples.extend_from_slice(&postamble);

        // Add silence after postamble for clean frame end
        samples.extend_from_slice(&vec![0.0f32; SYNC_SILENCE_SAMPLES]);

        Ok(samples)
    }

    /// Convert bytes to DTMF symbols
    /// Maps 8-bit bytes to base-48 representation
    /// Each byte (0-255) -> multiple symbols (0-47)
    fn bytes_to_dtmf_symbols(&self, bytes: &[u8]) -> Vec<u8> {
        let mut symbols = Vec::new();

        for &byte in bytes {
            // Simple conversion: byte to base-48
            // 256 / 48 = 5.33, so each byte needs at most 2 symbols
            // First symbol: high part (byte / 48)
            // Second symbol: low part (byte % 48)
            let high = byte / DTMF_NUM_SYMBOLS;
            let low = byte % DTMF_NUM_SYMBOLS;

            symbols.push(high);
            symbols.push(low);
        }

        symbols
    }
}

impl Default for EncoderDtmf {
    fn default() -> Self {
        Self::new().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encoder_dtmf_basic() {
        let mut encoder = EncoderDtmf::new().unwrap();
        let data = b"Hello";
        let samples = encoder.encode(data).unwrap();

        // Should have: preamble + DTMF data + postamble
        assert!(samples.len() > PREAMBLE_SAMPLES + POSTAMBLE_SAMPLES);
    }

    #[test]
    fn test_encoder_dtmf_empty_data() {
        let mut encoder = EncoderDtmf::new().unwrap();
        let data = b"";
        let result = encoder.encode(data);
        // Empty data should still work (frame header only)
        assert!(result.is_ok());
    }

    #[test]
    fn test_encoder_dtmf_max_payload() {
        let mut encoder = EncoderDtmf::new().unwrap();
        let data = vec![0x55u8; MAX_PAYLOAD_SIZE];
        let result = encoder.encode(&data);
        assert!(result.is_ok());
    }

    #[test]
    fn test_encoder_dtmf_oversized_payload() {
        let mut encoder = EncoderDtmf::new().unwrap();
        let data = vec![0xAAu8; MAX_PAYLOAD_SIZE + 1];
        let result = encoder.encode(&data);
        assert!(result.is_err());
    }

    #[test]
    fn test_bytes_to_dtmf_symbols() {
        let encoder = EncoderDtmf::new().unwrap();

        // Test byte 0: should give [0, 0]
        let symbols = encoder.bytes_to_dtmf_symbols(&[0]);
        assert_eq!(symbols, vec![0, 0]);

        // Test byte 47: should give [0, 47]
        let symbols = encoder.bytes_to_dtmf_symbols(&[47]);
        assert_eq!(symbols, vec![0, 47]);

        // Test byte 48: should give [1, 0]
        let symbols = encoder.bytes_to_dtmf_symbols(&[48]);
        assert_eq!(symbols, vec![1, 0]);

        // Test byte 255: should give [5, 15] (255 = 5*48 + 15)
        let symbols = encoder.bytes_to_dtmf_symbols(&[255]);
        assert_eq!(symbols, vec![5, 15]);
    }

    #[test]
    fn test_dtmf_symbol_range() {
        let encoder = EncoderDtmf::new().unwrap();

        // Test all byte values produce valid symbols
        for byte_val in 0..=255u8 {
            let symbols = encoder.bytes_to_dtmf_symbols(&[byte_val]);
            assert_eq!(symbols.len(), 2);
            for &symbol in &symbols {
                assert!(symbol < DTMF_NUM_SYMBOLS, "Symbol {} out of range for byte {}", symbol, byte_val);
            }
        }
    }

    #[test]
    fn test_encoder_dtmf_various_sizes() {
        let mut encoder = EncoderDtmf::new().unwrap();

        // Test various payload sizes
        for size in [1, 5, 10, 20, 50, 100, MAX_PAYLOAD_SIZE] {
            let data = vec![0x42u8; size];
            let result = encoder.encode(&data);
            assert!(result.is_ok(), "Failed to encode {} bytes", size);
        }
    }

    #[test]
    fn test_dtmf_roundtrip_basic() {
        let mut encoder = EncoderDtmf::new().unwrap();
        let mut decoder = crate::DecoderDtmf::new().unwrap();

        let original_data = b"Hello, DTMF!";
        let samples = encoder.encode(original_data).unwrap();
        let decoded_data = decoder.decode(&samples).unwrap();

        assert_eq!(decoded_data, original_data);
    }

    #[test]
    fn test_dtmf_roundtrip_various_payloads() {
        let mut encoder = EncoderDtmf::new().unwrap();
        let mut decoder = crate::DecoderDtmf::new().unwrap();

        // Test various payloads
        let test_cases = vec![
            b"A".to_vec(),
            b"Test123".to_vec(),
            vec![0x00, 0xFF, 0xAA, 0x55],
            (0..50).map(|i| i as u8).collect::<Vec<u8>>(),
        ];

        for original_data in test_cases {
            let samples = encoder.encode(&original_data).unwrap();
            let decoded_data = decoder.decode(&samples).unwrap();
            assert_eq!(decoded_data, original_data);
        }
    }

    #[test]
    fn test_dtmf_roundtrip_with_noise() {
        let mut encoder = EncoderDtmf::new().unwrap();
        let mut decoder = crate::DecoderDtmf::new().unwrap();

        let original_data = b"Noise test!";
        let mut samples = encoder.encode(original_data).unwrap();

        // Add moderate noise (15dB SNR)
        let mut seed = 54321u64;
        let signal_rms = (samples.iter().map(|x| x * x).sum::<f32>() / samples.len() as f32).sqrt();
        let noise_rms = signal_rms / 10.0f32.powf(15.0 / 20.0);

        for sample in samples.iter_mut() {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let noise = ((seed >> 32) as f32) / (u32::MAX as f32) * 2.0 - 1.0;
            *sample += noise * noise_rms;
        }

        let decoded_data = decoder.decode(&samples).unwrap();
        assert_eq!(decoded_data, original_data);
    }

    #[test]
    fn test_dtmf_roundtrip_empty() {
        let mut encoder = EncoderDtmf::new().unwrap();
        let mut decoder = crate::DecoderDtmf::new().unwrap();

        let original_data = b"";
        let samples = encoder.encode(original_data).unwrap();
        let decoded_data = decoder.decode(&samples).unwrap();

        assert_eq!(decoded_data, original_data);
    }
}
