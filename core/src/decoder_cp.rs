use crate::error::{AudioModemError, Result};
use crate::fec::FecDecoder;
use crate::framing::FrameDecoder;
use crate::ofdm_cp::OfdmDemodulatorCp;
use crate::sync::{detect_postamble, detect_preamble};
use crate::{FRAME_HEADER_SIZE, PREAMBLE_SAMPLES, RS_TOTAL_BYTES};

/// Decoder with Cyclic Prefix (CP) guard intervals
///
/// Uses OFDM with CP to handle:
/// - Automatic removal of CP from symbols
/// - Robustness to multipath/ISI in acoustic channels
/// - Complete ISI elimination when CP length > channel delay spread
pub struct DecoderCp {
    ofdm: OfdmDemodulatorCp,
    fec: FecDecoder,
}

impl DecoderCp {
    pub fn new() -> Result<Self> {
        Ok(Self {
            ofdm: OfdmDemodulatorCp::new(),
            fec: FecDecoder::new()?,
        })
    }

    /// Create decoder with custom CP length
    /// Must match the encoder's CP length
    pub fn new_with_cp(cp_len: usize) -> Result<Self> {
        Ok(Self {
            ofdm: OfdmDemodulatorCp::new_with_cp(cp_len),
            fec: FecDecoder::new()?,
        })
    }

    /// Decode audio samples back to binary data
    /// Expects: preamble + (frame data with CP) + postamble
    pub fn decode(&mut self, samples: &[f32]) -> Result<Vec<u8>> {
        let total_samples_per_symbol = self.ofdm.total_samples_per_symbol();

        if samples.len() < total_samples_per_symbol * 2 {
            return Err(AudioModemError::InsufficientData);
        }

        // Detect preamble
        let preamble_pos = detect_preamble(samples, 500.0)
            .ok_or(AudioModemError::PreambleNotFound)?;

        // Start reading data after preamble
        let data_start = preamble_pos + PREAMBLE_SAMPLES;

        if data_start + total_samples_per_symbol > samples.len() {
            return Err(AudioModemError::InsufficientData);
        }

        // Try to detect postamble in remaining samples
        let remaining = &samples[data_start..];
        let postamble_pos = detect_postamble(remaining, 100.0)
            .ok_or(AudioModemError::PostambleNotFound)?;

        let data_end = data_start + postamble_pos;

        // Demodulate all symbols between data_start and data_end
        // Each symbol with CP is: cp_len + SAMPLES_PER_SYMBOL samples
        let mut bits = Vec::new();
        let mut pos = data_start;

        while pos + total_samples_per_symbol <= data_end {
            let symbol_bits = self.ofdm.demodulate(&samples[pos..])?;
            bits.extend_from_slice(&symbol_bits);
            pos += total_samples_per_symbol;
        }

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

        // Pad bytes to multiple of RS_TOTAL_BYTES for FEC decoding
        while bytes.len() % RS_TOTAL_BYTES != 0 && bytes.len() < FRAME_HEADER_SIZE + 256 {
            bytes.push(0);
        }

        if bytes.len() < RS_TOTAL_BYTES {
            return Err(AudioModemError::InvalidFrameSize);
        }

        // Decode FEC chunks
        let mut decoded_data = Vec::new();
        for chunk in bytes.chunks(RS_TOTAL_BYTES) {
            if chunk.len() == RS_TOTAL_BYTES {
                let decoded_chunk = self.fec.decode(chunk)?;
                decoded_data.extend_from_slice(&decoded_chunk);
            }
        }

        if decoded_data.len() < FRAME_HEADER_SIZE {
            return Err(AudioModemError::InvalidFrameSize);
        }

        // Decode frame header and payload
        let frame = FrameDecoder::decode(&decoded_data)?;

        Ok(frame.payload)
    }

    /// Get CP length in samples
    pub fn cp_len(&self) -> usize {
        self.ofdm.cp_len()
    }

    /// Get total samples per symbol (CP + OFDM symbol)
    pub fn total_samples_per_symbol(&self) -> usize {
        self.ofdm.total_samples_per_symbol()
    }
}

impl Default for DecoderCp {
    fn default() -> Self {
        Self::new().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encoder_cp::EncoderCp;

    #[test]
    fn test_decoder_cp_basic() {
        let mut encoder = EncoderCp::new().unwrap();
        let mut decoder = DecoderCp::new().unwrap();

        let data = b"Hello";
        let encoded = encoder.encode(data).unwrap();
        let decoded = decoder.decode(&encoded).unwrap();

        assert_eq!(decoded, data);
    }

    #[test]
    fn test_decoder_cp_round_trip_mixed_bits() {
        let mut encoder = EncoderCp::new().unwrap();
        let mut decoder = DecoderCp::new().unwrap();

        let data = b"Test message with cyclic prefix";
        let encoded = encoder.encode(data).unwrap();
        let decoded = decoder.decode(&encoded).unwrap();

        assert_eq!(decoded, data);
    }

    #[test]
    fn test_decoder_cp_custom_length() {
        let mut encoder = EncoderCp::new_with_cp(80).unwrap();
        let mut decoder = DecoderCp::new_with_cp(80).unwrap();

        let data = b"CP80";
        let encoded = encoder.encode(data).unwrap();
        let decoded = decoder.decode(&encoded).unwrap();

        assert_eq!(decoded, data);
    }

    #[test]
    fn test_decoder_cp_insufficient_data() {
        let mut decoder = DecoderCp::new().unwrap();
        let short_data = vec![0.0; 100];

        let result = decoder.decode(&short_data);
        assert!(result.is_err());
    }

    #[test]
    fn test_decoder_cp_symbol_length() {
        let decoder = DecoderCp::new().unwrap();
        // Default CP is 160 samples
        assert_eq!(decoder.cp_len(), 160);
        // Total per symbol is 160 + 1600 = 1760
        assert_eq!(decoder.total_samples_per_symbol(), 1760);
    }

    #[test]
    fn test_decoder_cp_mismatched_cp_length() {
        // Encoder with 160 CP, decoder with 320 CP - should fail gracefully
        let mut encoder = EncoderCp::new_with_cp(160).unwrap();
        let mut decoder = DecoderCp::new_with_cp(320).unwrap();

        let data = b"Mismatch";
        let encoded = encoder.encode(data).unwrap();
        let result = decoder.decode(&encoded);

        // Should fail because symbol timing is off
        assert!(result.is_err());
    }

    #[test]
    fn test_decoder_cp_max_payload() {
        let mut encoder = EncoderCp::new().unwrap();
        let mut decoder = DecoderCp::new().unwrap();

        let data = vec![0xFF; 200]; // MAX_PAYLOAD_SIZE
        let encoded = encoder.encode(&data).unwrap();
        let decoded = decoder.decode(&encoded).unwrap();

        assert_eq!(decoded, data);
    }
}
