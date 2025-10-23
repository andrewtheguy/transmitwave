use crate::error::{AudioModemError, Result};
use crate::fec::FecDecoder;
use crate::framing::FrameDecoder;
use crate::ofdm::OfdmDemodulator;
use crate::spread::SpreadSpectrumDespreader;
use crate::sync::{detect_postamble, detect_preamble};
use crate::{FRAME_HEADER_SIZE, PREAMBLE_SAMPLES, RS_TOTAL_BYTES};

/// Decoder with Spread Spectrum for redundancy and noise robustness
///
/// Reverses the spreading to recover original OFDM symbols
/// Provides better reliability in noisy channels by leveraging
/// the redundancy added by Barker code spreading
pub struct DecoderSpread {
    ofdm: OfdmDemodulator,
    fec: FecDecoder,
    despreader: SpreadSpectrumDespreader,
    chip_duration: usize,
}

impl DecoderSpread {
    /// Create new decoder with spread spectrum
    /// chip_duration: samples per Barker chip (must match encoder)
    pub fn new(chip_duration: usize) -> Result<Self> {
        Ok(Self {
            ofdm: OfdmDemodulator::new(),
            fec: FecDecoder::new()?,
            despreader: SpreadSpectrumDespreader::new(chip_duration)?,
            chip_duration,
        })
    }

    /// Decode audio samples back to binary data
    /// Expects: preamble + (spread OFDM symbols) + postamble
    pub fn decode(&mut self, samples: &[f32]) -> Result<Vec<u8>> {
        let spread_symbol_size = 1600 * self.chip_duration; // 1600 OFDM samples × chip_duration

        if samples.len() < spread_symbol_size * 2 {
            return Err(AudioModemError::InsufficientData);
        }

        // Detect preamble
        let preamble_pos = detect_preamble(samples, 500.0)
            .ok_or(AudioModemError::PreambleNotFound)?;

        // Start reading data after preamble
        let data_start = preamble_pos + PREAMBLE_SAMPLES;

        if data_start + spread_symbol_size > samples.len() {
            return Err(AudioModemError::InsufficientData);
        }

        // Try to detect postamble in remaining samples
        let remaining = &samples[data_start..];
        let postamble_pos = detect_postamble(remaining, 100.0)
            .ok_or(AudioModemError::PostambleNotFound)?;

        let data_end = data_start + postamble_pos;

        // Despread and demodulate all symbols between data_start and data_end
        let mut bits = Vec::new();
        let mut pos = data_start;

        while pos + spread_symbol_size <= data_end {
            // Despread the symbol (remove Barker spreading)
            let spread_samples = &samples[pos..pos + spread_symbol_size];
            let ofdm_samples = self.despreader.despread(spread_samples)?;

            // Demodulate OFDM to get bits
            let symbol_bits = self.ofdm.demodulate(&ofdm_samples)?;
            bits.extend_from_slice(&symbol_bits);

            pos += spread_symbol_size;
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

    /// Get chip duration
    pub fn chip_duration(&self) -> usize {
        self.chip_duration
    }

    /// Get samples per symbol with spreading
    pub fn samples_per_spread_symbol(&self) -> usize {
        1600 * self.chip_duration
    }
}

impl Default for DecoderSpread {
    fn default() -> Self {
        Self::new(2).unwrap() // Default: 2 samples per Barker chip
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encoder_spread::EncoderSpread;

    #[test]
    fn test_decoder_spread_round_trip() {
        let mut encoder = EncoderSpread::new(2).unwrap();
        let original_data = b"hello";
        let samples = encoder.encode(original_data).unwrap();

        let mut decoder = DecoderSpread::new(2).unwrap();
        let decoded_data = decoder.decode(&samples).unwrap();

        assert_eq!(&decoded_data, original_data);
    }

    #[test]
    fn test_decoder_spread_empty_message() {
        let mut encoder = EncoderSpread::new(2).unwrap();
        let original_data = b"";
        let samples = encoder.encode(original_data).unwrap();

        let mut decoder = DecoderSpread::new(2).unwrap();
        let decoded_data = decoder.decode(&samples).unwrap();

        assert_eq!(&decoded_data, original_data);
    }

    #[test]
    fn test_decoder_spread_chip_duration() {
        let decoder = DecoderSpread::new(3).unwrap();
        assert_eq!(decoder.chip_duration(), 3);
        assert_eq!(decoder.samples_per_spread_symbol(), 1600 * 3);
    }
}
