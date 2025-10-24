use crate::error::Result;
use crate::fec::FecEncoder;
use crate::framing::{Frame, FrameEncoder, crc16};
use crate::ofdm_cp::OfdmModulatorCp;
use crate::sync::{generate_preamble_noise, generate_postamble_noise};
use crate::{MAX_PAYLOAD_SIZE, PREAMBLE_SAMPLES, POSTAMBLE_SAMPLES, NUM_SUBCARRIERS};

/// Encoder with Cyclic Prefix (CP) guard intervals
///
/// Uses OFDM with explicit CP prepended to each symbol to:
/// - Convert linear multipath convolution to circular convolution
/// - Completely eliminate Inter-Symbol Interference (ISI)
/// - Provide robustness for acoustic recorded/OTA transmission
///
/// Trade-off: 10% throughput reduction (1600 → 1760 samples per symbol)
pub struct EncoderCp {
    ofdm: OfdmModulatorCp,
    fec: FecEncoder,
}

impl EncoderCp {
    pub fn new() -> Result<Self> {
        Ok(Self {
            ofdm: OfdmModulatorCp::new(),
            fec: FecEncoder::new()?,
        })
    }

    /// Create encoder with custom CP length
    /// Typical values: 80-320 samples (5%-20% overhead)
    pub fn new_with_cp(cp_len: usize) -> Result<Self> {
        Ok(Self {
            ofdm: OfdmModulatorCp::new_with_cp(cp_len),
            fec: FecEncoder::new()?,
        })
    }

    /// Encode binary data into audio samples with Cyclic Prefix
    /// Returns: preamble + (frame data with CP-based OFDM) + postamble
    pub fn encode(&mut self, data: &[u8]) -> Result<Vec<f32>> {
        if data.len() > MAX_PAYLOAD_SIZE {
            return Err(crate::error::AudioModemError::InvalidInputSize);
        }

        // Create frame
        let payload = data.to_vec();
        let frame = Frame {
            payload_len: data.len() as u16,
            frame_num: 0,
            payload: payload.clone(),
            payload_crc: crc16(&payload),
        };

        let frame_data = FrameEncoder::encode(&frame)?;

        // Encode each byte with FEC
        let mut encoded_data = Vec::new();
        for chunk in frame_data.chunks(223) {
            let fec_chunk = self.fec.encode(chunk)?;
            encoded_data.extend_from_slice(&fec_chunk);
        }

        // Convert bytes to bits for OFDM
        let mut bits = Vec::new();
        for byte in encoded_data {
            for i in (0..8).rev() {
                bits.push((byte >> i) & 1 == 1);
            }
        }

        // Generate preamble as PRN noise burst (0.25s, distinct from postamble)
        let preamble = generate_preamble_noise(PREAMBLE_SAMPLES, 0.5);

        // Modulate data bits to OFDM symbols with CP
        let mut samples = preamble;

        // Process bits in OFDM symbol chunks (NUM_SUBCARRIERS bits per symbol)
        for symbol_bits in bits.chunks(NUM_SUBCARRIERS) {
            let symbol_samples = self.ofdm.modulate(symbol_bits)?;
            samples.extend_from_slice(&symbol_samples);
        }

        // Generate postamble as PRN noise burst (0.25s, different pattern than preamble)
        let postamble = generate_postamble_noise(POSTAMBLE_SAMPLES, 0.5);
        samples.extend_from_slice(&postamble);

        Ok(samples)
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

impl Default for EncoderCp {
    fn default() -> Self {
        Self::new().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encoder_cp_basic() {
        let mut encoder = EncoderCp::new().unwrap();
        let data = b"Hello";

        let result = encoder.encode(data);
        assert!(result.is_ok());

        let samples = result.unwrap();
        // Should have preamble + data symbols with CP + postamble
        // Preamble: 4000 samples
        // Data: varies by CP length
        // Postamble: 4000 samples
        assert!(samples.len() > 8000, "Encoded audio should have preamble + postamble");
    }

    #[test]
    fn test_encoder_cp_max_payload() {
        let mut encoder = EncoderCp::new().unwrap();
        let data = vec![0xFF; MAX_PAYLOAD_SIZE];

        let result = encoder.encode(&data);
        assert!(result.is_ok());
    }

    #[test]
    fn test_encoder_cp_oversized_payload() {
        let mut encoder = EncoderCp::new().unwrap();
        let data = vec![0xFF; MAX_PAYLOAD_SIZE + 1];

        let result = encoder.encode(&data);
        assert!(result.is_err());
    }

    #[test]
    fn test_encoder_cp_symbol_length() {
        let encoder = EncoderCp::new().unwrap();
        // Default CP is 160 samples
        assert_eq!(encoder.cp_len(), 160);
        // Total per symbol is 160 + 1600 = 1760
        assert_eq!(encoder.total_samples_per_symbol(), 1760);
    }

    #[test]
    fn test_encoder_cp_custom_cp_length() {
        let encoder = EncoderCp::new_with_cp(320).unwrap();
        // Custom CP is 320 samples
        assert_eq!(encoder.cp_len(), 320);
        // Total per symbol is 320 + 1600 = 1920
        assert_eq!(encoder.total_samples_per_symbol(), 1920);
    }
}
