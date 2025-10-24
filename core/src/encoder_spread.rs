use crate::error::Result;
use crate::fec::FecEncoder;
use crate::framing::{Frame, FrameEncoder};
use crate::ofdm::OfdmModulator;
use crate::spread::SpreadSpectrumSpreader;
use crate::sync::{generate_chirp, generate_postamble};
use crate::fhss;
use crate::{MAX_PAYLOAD_SIZE, PREAMBLE_SAMPLES, POSTAMBLE_SAMPLES};

/// Encoder with Spread Spectrum for redundancy and noise-like properties
///
/// Uses Barker code spreading to:
/// - Add redundancy that improves reliability in noisy channels
/// - Create characteristic "hiss" sound by spreading across multiple frequencies
/// - Increase robustness to fading and interference
/// - Maintain compatibility with frequency-aware OFDM
///
/// Supports optional Frequency-Hopping Spread Spectrum (FHSS) for additional
/// resistance to narrowband interference
pub struct EncoderSpread {
    ofdm: OfdmModulator,
    fec: FecEncoder,
    spreader: SpreadSpectrumSpreader,
    chip_duration: usize,
    num_frequency_hops: usize,
}

impl EncoderSpread {
    /// Create new encoder with spread spectrum
    /// chip_duration: samples per Barker chip (1-10 typical, higher = more spreading/redundancy)
    pub fn new(chip_duration: usize) -> Result<Self> {
        Self::with_fhss(chip_duration, 1) // Default: FHSS disabled (1 band)
    }

    /// Create new encoder with spread spectrum and FHSS
    /// chip_duration: samples per Barker chip
    /// num_frequency_hops: number of frequency bands (1 = disabled, 2-4 = enabled)
    pub fn with_fhss(chip_duration: usize, num_frequency_hops: usize) -> Result<Self> {
        Ok(Self {
            ofdm: OfdmModulator::with_frequency_hops(num_frequency_hops),
            fec: FecEncoder::new()?,
            spreader: SpreadSpectrumSpreader::new(chip_duration)?,
            chip_duration,
            num_frequency_hops,
        })
    }

    /// Encode binary data into audio samples with spread spectrum
    /// Returns: preamble + (spread OFDM symbols) + postamble
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

        // Convert bytes to bits for OFDM
        let mut bits = Vec::new();
        for byte in encoded_data {
            for i in (0..8).rev() {
                bits.push((byte >> i) & 1 == 1);
            }
        }

        // Generate preamble (chirp)
        let preamble = generate_chirp(PREAMBLE_SAMPLES, 200.0, 4000.0, 0.5);

        // Modulate data bits to OFDM symbols with spread spectrum
        let mut samples = preamble;

        // Process bits in OFDM symbol chunks (48 bits per symbol)
        for (symbol_index, symbol_bits) in bits.chunks(48).enumerate() {
            // Get hopping band for this symbol
            let band_index = fhss::get_band_for_symbol(symbol_index, self.num_frequency_hops);

            // Modulate with band-specific frequencies
            let symbol_samples = self.ofdm.modulate_with_band(symbol_bits, band_index)?;

            // Apply spread spectrum to add redundancy and create "hiss" effect
            let spread_samples = self.spreader.spread(&symbol_samples)?;
            samples.extend_from_slice(&spread_samples);
        }

        // Generate postamble
        let postamble = generate_postamble(POSTAMBLE_SAMPLES, 0.5);
        samples.extend_from_slice(&postamble);

        Ok(samples)
    }

    /// Get chip duration (samples per Barker chip)
    pub fn chip_duration(&self) -> usize {
        self.chip_duration
    }

    /// Get samples per symbol with spreading
    /// Each 1600-sample OFDM symbol becomes 1600 * chip_duration samples
    pub fn samples_per_spread_symbol(&self) -> usize {
        1600 * self.chip_duration
    }

    /// Get number of frequency hops (1 = disabled)
    pub fn num_frequency_hops(&self) -> usize {
        self.num_frequency_hops
    }
}

impl Default for EncoderSpread {
    fn default() -> Self {
        Self::new(2).unwrap() // Default: 2 samples per Barker chip (2800 Hz hiss frequency)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encoder_spread_basic() {
        let mut encoder = EncoderSpread::new(2).unwrap();
        let data = b"hello";
        let samples = encoder.encode(data).unwrap();

        // Should have preamble + data + postamble
        assert!(!samples.is_empty());
        assert!(samples.len() > 10000); // Rough minimum for full encoded message
    }

    #[test]
    fn test_encoder_spread_max_payload() {
        let mut encoder = EncoderSpread::new(2).unwrap();
        let data = vec![42u8; MAX_PAYLOAD_SIZE];
        let samples = encoder.encode(&data).unwrap();
        assert!(!samples.is_empty());
    }

    #[test]
    fn test_encoder_spread_empty_data() {
        let mut encoder = EncoderSpread::new(2).unwrap();
        let data = b"";
        let samples = encoder.encode(data).unwrap();
        assert!(!samples.is_empty());
    }

    #[test]
    fn test_encoder_spread_chip_duration() {
        let encoder = EncoderSpread::new(3).unwrap();
        assert_eq!(encoder.chip_duration(), 3);
        assert_eq!(encoder.samples_per_spread_symbol(), 1600 * 3);
    }

    #[test]
    fn test_encoder_spread_with_fhss() {
        let mut encoder = EncoderSpread::with_fhss(2, 3).unwrap();
        let data = b"fhss_test";
        let samples = encoder.encode(data).unwrap();

        assert!(!samples.is_empty());
        assert_eq!(encoder.num_frequency_hops(), 3);
    }

    #[test]
    fn test_encoder_spread_fhss_disabled() {
        let mut encoder = EncoderSpread::with_fhss(2, 1).unwrap();
        let data = b"no_fhss";
        let samples = encoder.encode(data).unwrap();

        assert!(!samples.is_empty());
        assert_eq!(encoder.num_frequency_hops(), 1);
    }
}
