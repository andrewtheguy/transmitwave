use crate::error::{AudioModemError, Result};
use std::f32::consts::PI;

// Multi-tone FSK configuration (ggwave-compatible)
//
// This implementation uses the EXACT frequency band from ggwave's audible protocols:
// - ggwave audible protocols: freqStart = 40, using 96 bins (40-135)
// - With ggwave's hzPerSample = 46.875 Hz: 40 * 46.875 = 1875 Hz base
// - Frequency range: 1875 Hz to 6328.125 Hz (identical to ggwave)
//
// ggwave protocol parameters (at 48kHz, 1024 samples/frame):
// - Normal:  9 frames/tx = 192ms per symbol
// - Fast:    6 frames/tx = 128ms per symbol
// - Fastest: 3 frames/tx = 64ms per symbol
//
// Our implementation (at 16kHz):
// - Uses reduced frequency spacing (20 Hz) for lower range
// - Uses very low frequency range (400-2320 Hz) for sub-bass band
// - Transmits 3 bytes (6 nibbles) per symbol like ggwave
// - Symbol duration increased 2.3x to compensate for lower delta (reduces data rate)

/// Base frequency in Hz (very low for sub-bass range)
/// Original: 1875 Hz, reduced to 400 Hz for low-frequency band
const FSK_BASE_FREQ: f32 = 400.0;

/// Frequency spacing in Hz between adjacent bins (reduced for lower range)
/// Original: 46.875 Hz, reduced to 20.0 Hz to lower frequency span
/// This reduces data rate by ~2.3x but keeps frequencies well below 2kHz
const FSK_FREQ_DELTA: f32 = 20.0;

/// Total number of frequency bins (ggwave uses bins 40-135 for audible)
/// 6 nibbles * 16 tones per nibble = 96 bins
const FSK_NUM_BINS: usize = 96;

/// Number of nibbles transmitted per symbol (ggwave bytesPerTx = 3)
pub const FSK_NIBBLES_PER_SYMBOL: usize = 6;

/// Number of bytes transmitted per symbol (ggwave standard)
pub const FSK_BYTES_PER_SYMBOL: usize = 3;

/// Speed mode variants (matching ggwave protocol speeds)
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FskSpeed {
    /// Normal: ~128ms per symbol at 16kHz (similar to ggwave Fast)
    Normal,
    /// Fast: ~64ms per symbol at 16kHz (similar to ggwave Fastest)
    Fast,
    /// Fastest: ~32ms per symbol at 16kHz (faster than ggwave for robustness testing)
    Fastest,
}

impl FskSpeed {
    /// Get symbol duration in samples for this speed at 16kHz sample rate
    /// Increased 2.3x from original to compensate for tighter frequency spacing (20Hz vs 46.875Hz)
    pub fn samples_per_symbol(&self) -> usize {
        match self {
            FskSpeed::Normal => 4704,    // ~294ms at 16kHz (was 128ms, now 2.3x slower)
            FskSpeed::Fast => 2352,      // ~147ms at 16kHz (was 64ms, now 2.3x slower)
            FskSpeed::Fastest => 1176,   // ~74ms at 16kHz (was 32ms, now 2.3x slower)
        }
    }

    /// Get approximate data rate in bytes/second
    pub fn bytes_per_second(&self) -> f32 {
        let symbol_duration_sec = self.samples_per_symbol() as f32 / 16000.0;
        FSK_BYTES_PER_SYMBOL as f32 / symbol_duration_sec
    }
}

/// Default FSK symbol duration (Normal speed: ~294ms at 16kHz)
/// Increased 2.3x from original to compensate for tighter frequency spacing
/// - Original: 2048 samples = 128ms
/// - Now: 4704 samples = 294ms (2.3x slower for lower frequency delta)
pub const FSK_SYMBOL_SAMPLES: usize = 4704;

/// Calculate frequency for a given bin index
/// freq_hz = FSK_BASE_FREQ + bin_index * FSK_FREQ_DELTA
fn bin_to_freq(bin: usize) -> f32 {
    FSK_BASE_FREQ + (bin as f32) * FSK_FREQ_DELTA
}

/// Calculate approximate bin index for a given frequency
/// Returns None if frequency is outside valid range
fn freq_to_bin(freq: f32) -> Option<usize> {
    if freq < FSK_BASE_FREQ {
        return None;
    }
    let bin = ((freq - FSK_BASE_FREQ) / FSK_FREQ_DELTA).round() as usize;
    if bin < FSK_NUM_BINS {
        Some(bin)
    } else {
        None
    }
}

/// FSK modulator - generates multi-tone audio for simultaneous transmission
///
/// Transmits 3 bytes (6 nibbles) per symbol by using 6 simultaneous frequencies.
/// Each nibble (4 bits, value 0-15) selects one frequency from a band of 16 frequencies.
///
/// Supports multiple speed modes matching ggwave's protocol speeds (adjusted for 16kHz).
pub struct FskModulator {
    sample_rate: f32,
    speed: FskSpeed,
}

impl FskModulator {
    pub fn new() -> Self {
        Self {
            sample_rate: crate::SAMPLE_RATE as f32,
            speed: FskSpeed::Normal,
        }
    }

    /// Create a new modulator with specific speed
    pub fn with_speed(speed: FskSpeed) -> Self {
        Self {
            sample_rate: crate::SAMPLE_RATE as f32,
            speed,
        }
    }

    /// Get current speed mode
    pub fn speed(&self) -> FskSpeed {
        self.speed
    }

    /// Set speed mode
    pub fn set_speed(&mut self, speed: FskSpeed) {
        self.speed = speed;
    }

    /// Modulate 3 bytes into a multi-tone FSK symbol
    ///
    /// Each byte is split into two 4-bit nibbles.
    /// Each nibble selects a frequency from its dedicated band:
    /// - Nibble 0 (byte[0] high): bins 0-15
    /// - Nibble 1 (byte[0] low):  bins 16-31
    /// - Nibble 2 (byte[1] high): bins 32-47
    /// - Nibble 3 (byte[1] low):  bins 48-63
    /// - Nibble 4 (byte[2] high): bins 64-79
    /// - Nibble 5 (byte[2] low):  bins 80-95
    ///
    /// All 6 tones are generated simultaneously and superimposed.
    /// Symbol duration depends on the configured speed mode.
    pub fn modulate_symbol(&mut self, bytes: &[u8]) -> Result<Vec<f32>> {
        if bytes.len() != FSK_BYTES_PER_SYMBOL {
            return Err(AudioModemError::InvalidInputSize);
        }

        let symbol_samples = self.speed.samples_per_symbol();
        let mut samples = vec![0.0f32; symbol_samples];

        // Extract 6 nibbles from 3 bytes
        let nibbles = [
            (bytes[0] >> 4) & 0x0F,  // High nibble of byte 0
            bytes[0] & 0x0F,         // Low nibble of byte 0
            (bytes[1] >> 4) & 0x0F,  // High nibble of byte 1
            bytes[1] & 0x0F,         // Low nibble of byte 1
            (bytes[2] >> 4) & 0x0F,  // High nibble of byte 2
            bytes[2] & 0x0F,         // Low nibble of byte 2
        ];

        // Generate and superimpose all 6 tones
        for (nibble_idx, &nibble_val) in nibbles.iter().enumerate() {
            // Each nibble has a dedicated band of 16 frequencies
            let band_offset = nibble_idx * 16;
            let bin = band_offset + (nibble_val as usize);

            if bin >= FSK_NUM_BINS {
                return Err(AudioModemError::InvalidInputSize);
            }

            let frequency = bin_to_freq(bin);
            let angular_freq = 2.0 * PI * frequency / self.sample_rate;

            // Add this tone to the output
            for i in 0..symbol_samples {
                samples[i] += (angular_freq * i as f32).sin();
            }
        }

        // Scale by 1/6 to prevent clipping when superimposing 6 tones
        // Also apply 0.7 overall amplitude
        let scale = 0.7 / FSK_NIBBLES_PER_SYMBOL as f32;
        for sample in samples.iter_mut() {
            *sample *= scale;
        }

        Ok(samples)
    }

    /// Modulate a sequence of bytes
    /// Input length must be a multiple of FSK_BYTES_PER_SYMBOL (3)
    pub fn modulate(&mut self, bytes: &[u8]) -> Result<Vec<f32>> {
        if bytes.len() % FSK_BYTES_PER_SYMBOL != 0 {
            return Err(AudioModemError::InvalidInputSize);
        }

        let mut samples = Vec::new();
        for chunk in bytes.chunks(FSK_BYTES_PER_SYMBOL) {
            let symbol_samples = self.modulate_symbol(chunk)?;
            samples.extend_from_slice(&symbol_samples);
        }

        Ok(samples)
    }
}

/// FSK demodulator - detects multiple simultaneous frequencies using FFT
///
/// Analyzes the spectrum to find 6 simultaneous tones, each representing a nibble.
/// Supports multiple speed modes matching ggwave's protocol speeds.
pub struct FskDemodulator {
    sample_rate: f32,
    speed: FskSpeed,
}

impl FskDemodulator {
    pub fn new() -> Self {
        Self {
            sample_rate: crate::SAMPLE_RATE as f32,
            speed: FskSpeed::Normal,
        }
    }

    /// Create a new demodulator with specific speed
    pub fn with_speed(speed: FskSpeed) -> Self {
        Self {
            sample_rate: crate::SAMPLE_RATE as f32,
            speed,
        }
    }

    /// Get current speed mode
    pub fn speed(&self) -> FskSpeed {
        self.speed
    }

    /// Set speed mode
    pub fn set_speed(&mut self, speed: FskSpeed) {
        self.speed = speed;
    }

    /// Compute power spectrum using simple DFT for our specific frequency bins
    ///
    /// This is more efficient than full FFT since we only need 96 specific bins.
    /// For each bin, we compute the magnitude using Goertzel-like approach.
    fn compute_spectrum(&self, samples: &[f32]) -> Vec<f32> {
        let n = samples.len();
        let mut spectrum = vec![0.0f32; FSK_NUM_BINS];

        for bin in 0..FSK_NUM_BINS {
            let freq = bin_to_freq(bin);
            let k = (0.5 + (n as f32 * freq / self.sample_rate)) as usize;
            let omega = 2.0 * PI * k as f32 / n as f32;
            let coeff = 2.0 * omega.cos();

            let mut q0 = 0.0;
            let mut q1 = 0.0;
            let mut q2 = 0.0;

            // Goertzel filter
            for &sample in samples {
                q0 = coeff * q1 - q2 + sample;
                q2 = q1;
                q1 = q0;
            }

            // Compute power (magnitude squared)
            let real = q1 - q2 * omega.cos();
            let imag = q2 * omega.sin();
            spectrum[bin] = real * real + imag * imag;
        }

        spectrum
    }

    /// Demodulate a single multi-tone FSK symbol
    ///
    /// Detects 6 simultaneous tones, one from each band of 16 frequencies.
    /// Returns the 3 bytes encoded in the symbol.
    /// Symbol duration depends on the configured speed mode.
    pub fn demodulate_symbol(&self, samples: &[f32]) -> Result<[u8; FSK_BYTES_PER_SYMBOL]> {
        let expected_samples = self.speed.samples_per_symbol();
        if samples.len() != expected_samples {
            return Err(AudioModemError::InvalidInputSize);
        }

        // Compute power spectrum
        let spectrum = self.compute_spectrum(samples);

        // Detect the strongest frequency in each of the 6 bands
        let mut nibbles = [0u8; FSK_NIBBLES_PER_SYMBOL];

        for nibble_idx in 0..FSK_NIBBLES_PER_SYMBOL {
            let band_start = nibble_idx * 16;
            let band_end = band_start + 16;

            // Find bin with maximum energy in this band
            let mut max_bin_in_band = 0;
            let mut max_energy = spectrum[band_start];

            for (offset, &energy) in spectrum[band_start..band_end].iter().enumerate() {
                if energy > max_energy {
                    max_energy = energy;
                    max_bin_in_band = offset;
                }
            }

            // The nibble value is the offset within the band
            nibbles[nibble_idx] = max_bin_in_band as u8;
        }

        // Reconstruct 3 bytes from 6 nibbles
        let bytes = [
            (nibbles[0] << 4) | nibbles[1],  // Byte 0
            (nibbles[2] << 4) | nibbles[3],  // Byte 1
            (nibbles[4] << 4) | nibbles[5],  // Byte 2
        ];

        Ok(bytes)
    }

    /// Demodulate a sequence of multi-tone FSK symbols
    /// samples.len() must be a multiple of the configured speed's samples_per_symbol
    pub fn demodulate(&self, samples: &[f32]) -> Result<Vec<u8>> {
        let symbol_samples = self.speed.samples_per_symbol();
        if samples.len() % symbol_samples != 0 {
            return Err(AudioModemError::InvalidInputSize);
        }

        let mut bytes = Vec::new();
        for chunk in samples.chunks(symbol_samples) {
            let symbol_bytes = self.demodulate_symbol(chunk)?;
            bytes.extend_from_slice(&symbol_bytes);
        }

        Ok(bytes)
    }
}

impl Default for FskModulator {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for FskDemodulator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bin_to_freq() {
        assert_eq!(bin_to_freq(0), FSK_BASE_FREQ);
        assert_eq!(bin_to_freq(16), FSK_BASE_FREQ + 16.0 * FSK_FREQ_DELTA);
        assert_eq!(bin_to_freq(95), FSK_BASE_FREQ + 95.0 * FSK_FREQ_DELTA);
    }

    #[test]
    fn test_freq_to_bin() {
        assert_eq!(freq_to_bin(FSK_BASE_FREQ), Some(0));
        assert_eq!(freq_to_bin(FSK_BASE_FREQ + 16.0 * FSK_FREQ_DELTA), Some(16));
        assert_eq!(freq_to_bin(FSK_BASE_FREQ - 100.0), None); // Too low
        assert_eq!(freq_to_bin(FSK_BASE_FREQ + 200.0 * FSK_FREQ_DELTA), None); // Too high
    }

    #[test]
    fn test_fsk_modulator_symbol_length() {
        let mut modulator = FskModulator::new();
        let bytes = [0xAB, 0xCD, 0xEF];
        let samples = modulator.modulate_symbol(&bytes).unwrap();
        assert_eq!(samples.len(), FSK_SYMBOL_SAMPLES);
    }

    #[test]
    fn test_fsk_modulator_invalid_input() {
        let mut modulator = FskModulator::new();
        // Wrong number of bytes
        assert!(modulator.modulate_symbol(&[0xAB]).is_err());
        assert!(modulator.modulate_symbol(&[0xAB, 0xCD]).is_err());
        assert!(modulator.modulate_symbol(&[0xAB, 0xCD, 0xEF, 0x12]).is_err());
    }

    #[test]
    fn test_fsk_demodulator_symbol_length() {
        let demodulator = FskDemodulator::new();
        let samples = vec![0.0; FSK_SYMBOL_SAMPLES];
        assert!(demodulator.demodulate_symbol(&samples).is_ok());
    }

    #[test]
    fn test_fsk_roundtrip_single_symbol() {
        let mut modulator = FskModulator::new();
        let demodulator = FskDemodulator::new();

        let test_cases = vec![
            [0x00, 0x00, 0x00], // All zeros
            [0xFF, 0xFF, 0xFF], // All ones
            [0xAB, 0xCD, 0xEF], // Mixed values
            [0x12, 0x34, 0x56], // Another pattern
            [0x0F, 0xF0, 0x55], // Edge cases
        ];

        for bytes in test_cases {
            let samples = modulator.modulate_symbol(&bytes).unwrap();
            let decoded = demodulator.demodulate_symbol(&samples).unwrap();
            assert_eq!(
                decoded, bytes,
                "Failed roundtrip for {:02X?}",
                bytes
            );
        }
    }

    #[test]
    fn test_fsk_roundtrip_multiple_symbols() {
        let mut modulator = FskModulator::new();
        let demodulator = FskDemodulator::new();

        // Test sequence: 2 symbols = 6 bytes
        let bytes = vec![
            0xAB, 0xCD, 0xEF, // Symbol 1
            0x12, 0x34, 0x56, // Symbol 2
        ];

        let samples = modulator.modulate(&bytes).unwrap();
        assert_eq!(samples.len(), FSK_SYMBOL_SAMPLES * 2);

        let decoded = demodulator.demodulate(&samples).unwrap();
        assert_eq!(decoded.len(), bytes.len());
        assert_eq!(decoded, bytes);
    }

    #[test]
    fn test_fsk_with_noise() {
        let mut modulator = FskModulator::new();
        let demodulator = FskDemodulator::new();

        // Encode a symbol
        let bytes = [0xAB, 0xCD, 0xEF];
        let mut samples = modulator.modulate_symbol(&bytes).unwrap();

        // Add small noise (5% amplitude)
        for sample in samples.iter_mut() {
            *sample += 0.05 * ((*sample * 100.0).sin());
        }

        // Should still decode correctly
        let decoded = demodulator.demodulate_symbol(&samples).unwrap();
        assert_eq!(decoded, bytes);
    }

    #[test]
    fn test_spectrum_computation() {
        let demodulator = FskDemodulator::new();
        let mut modulator = FskModulator::new();

        // Generate a known signal with specific frequencies
        let bytes = [0x00, 0x00, 0x00]; // All nibbles = 0, uses bins 0, 16, 32, 48, 64, 80
        let samples = modulator.modulate_symbol(&bytes).unwrap();

        let spectrum = demodulator.compute_spectrum(&samples);
        assert_eq!(spectrum.len(), FSK_NUM_BINS);

        // The bins corresponding to the transmitted frequencies should have highest energy
        // Nibble 0 (value 0) -> bin 0
        // Nibble 1 (value 0) -> bin 16
        // etc.
        let expected_bins = [0, 16, 32, 48, 64, 80];

        for &bin in &expected_bins {
            // Energy at expected bin should be significantly higher than adjacent bins
            if bin > 0 && bin < FSK_NUM_BINS - 1 {
                assert!(
                    spectrum[bin] > spectrum[bin - 1] * 2.0,
                    "Bin {} should have higher energy than bin {}",
                    bin,
                    bin - 1
                );
                assert!(
                    spectrum[bin] > spectrum[bin + 1] * 2.0,
                    "Bin {} should have higher energy than bin {}",
                    bin,
                    bin + 1
                );
            }
        }
    }

    #[test]
    fn test_fsk_byte_patterns() {
        let mut modulator = FskModulator::new();
        let demodulator = FskDemodulator::new();

        let patterns = vec![
            vec![0x00; 6],       // All zeros
            vec![0xFF; 6],       // All ones
            vec![0xAA; 6],       // Alternating bits
            vec![0x55; 6],       // Alternating bits (inverse)
            vec![0x00, 0xFF, 0x00, 0xFF, 0x00, 0xFF], // Alternating bytes
        ];

        for bytes in patterns {
            let samples = modulator.modulate(&bytes).unwrap();
            let decoded = demodulator.demodulate(&samples).unwrap();
            assert_eq!(decoded, bytes, "Failed for pattern {:02X?}", bytes);
        }
    }

    #[test]
    fn test_modulate_length_validation() {
        let mut modulator = FskModulator::new();

        // Valid lengths (multiples of 3)
        assert!(modulator.modulate(&[0x00, 0x00, 0x00]).is_ok());
        assert!(modulator.modulate(&[0x00; 6]).is_ok());
        assert!(modulator.modulate(&[0x00; 9]).is_ok());

        // Invalid lengths (not multiples of 3)
        assert!(modulator.modulate(&[0x00]).is_err());
        assert!(modulator.modulate(&[0x00, 0x00]).is_err());
        assert!(modulator.modulate(&[0x00; 4]).is_err());
        assert!(modulator.modulate(&[0x00; 5]).is_err());
    }

    #[test]
    fn test_demodulate_length_validation() {
        let demodulator = FskDemodulator::new();

        // Valid length
        let samples_valid = vec![0.0; FSK_SYMBOL_SAMPLES];
        assert!(demodulator.demodulate(&samples_valid).is_ok());

        // Invalid lengths
        let samples_short = vec![0.0; FSK_SYMBOL_SAMPLES - 1];
        assert!(demodulator.demodulate(&samples_short).is_err());

        let samples_odd = vec![0.0; FSK_SYMBOL_SAMPLES + 100];
        assert!(demodulator.demodulate(&samples_odd).is_err());
    }
}
