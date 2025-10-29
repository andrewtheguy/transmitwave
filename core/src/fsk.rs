use crate::error::{AudioModemError, Result};
use crate::fft_correlation::{fft_correlate_1d, Mode};
use std::cmp::Ordering;
use std::f32::consts::PI;

// Multi-tone FSK configuration optimized for mobile phone speakers
//
// Frequency band design:
// - Uses 96 frequency bins with 20 Hz spacing
// - Base frequency: 800 Hz (optimal for mobile speakers)
// - Maximum frequency: 2700 Hz (800 + 95*20)
// - Optimized for excellent mobile speaker compatibility (iPhone, Android)
//
// Symbol parameters (at 16kHz sample rate):
// - 3072 samples = 192ms per symbol (robust detection)
//
// Data encoding:
// - Transmits 3 bytes (6 nibbles) per symbol
// - Each nibble (4 bits) selects one of 16 frequencies from a band
// - Uses Reed-Solomon FEC for error correction
// - Includes preamble/postamble for frame synchronization

/// Base frequency in Hz (optimal range for mobile phone speakers)
const FSK_BASE_FREQ: f32 = 800.0;

/// Frequency spacing in Hz between adjacent bins
const FSK_FREQ_DELTA: f32 = 20.0;

/// Total number of frequency bins (96 provides redundancy and flexibility)
const FSK_NUM_BINS: usize = 96;

/// Number of nibbles transmitted per symbol (6 nibbles = 3 bytes)
pub const FSK_NIBBLES_PER_SYMBOL: usize = 6;

/// Number of bytes transmitted per symbol
pub const FSK_BYTES_PER_SYMBOL: usize = 3;


/// Configuration for fountain mode streaming
#[derive(Debug, Clone)]
pub struct FountainConfig {
    /// Timeout for sender to keep transmitting (in seconds)
    pub timeout_secs: u32,
    /// Size of each fountain block in bytes (before fountain encoding)
    pub block_size: usize,
    /// Ratio of repair blocks to source blocks (e.g., 0.5 = 50% overhead)
    pub repair_blocks_ratio: f32,
}

impl Default for FountainConfig {
    fn default() -> Self {
        Self {
            timeout_secs: 30,
            block_size: 64,
            repair_blocks_ratio: 0.5,
        }
    }
}


/// FSK symbol duration (192ms at 16kHz sample rate)
pub const FSK_SYMBOL_SAMPLES: usize = 3072;

/// Chirp FSK symbol duration (240ms at 16kHz sample rate, 100 bits/sec throughput)
/// Balanced for pleasant sound while maintaining good data rate
pub const CHIRP_SYMBOL_SAMPLES: usize = 3840;

/// Apply a smooth envelope to reduce spectral splatter near symbol edges.
const FSK_EDGE_TAPER_RATIO: f32 = 0.08; // 8% of the symbol on each side

/// Ensure we always have a minimum attack/decay regardless of speed.
const FSK_MIN_TAPER_SAMPLES: usize = 64;

/// Number of bins dedicated to each nibble band.
const FSK_BINS_PER_BAND: usize = 16;

/// Analysis window taper ratio for demodulator signal conditioning.
const FSK_ANALYSIS_TAPER_RATIO: f32 = 0.06;

/// Minimum taper window used on the demodulator input.
const FSK_ANALYSIS_MIN_TAPER_SAMPLES: usize = 32;

/// Target RMS level used by the demodulator AGC.
const FSK_TARGET_RMS: f32 = 0.5;

/// Minimum RMS guard to avoid unstable gain.
const FSK_MIN_RMS: f32 = 1e-4;

/// Bias applied to the median noise floor estimate before subtraction.
const FSK_NOISE_FLOOR_EPSILON: f32 = 1e-3;

/// Hard lower bound for the estimated noise floor.
const FSK_MIN_NOISE_FLOOR: f32 = 1e-6;

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

/// Generate a raised-cosine style window that softly ramps amplitude at both edges.
fn raised_cosine_window(len: usize, taper_len: usize) -> Vec<f32> {
    if taper_len == 0 || len == 0 {
        return vec![1.0; len];
    }

    let taper = taper_len.min(len / 2);
    if taper == 0 {
        return vec![1.0; len];
    }

    let mut window = vec![1.0; len];
    for i in 0..taper {
        // Smoothly increase from 0 to 1 using a sine-squared profile
        let progress = i as f32 / taper as f32;
        let value = (PI * progress / 2.0).sin().powi(2);
        window[i] = value;
        window[len - 1 - i] = value;
    }

    window
}

/// FSK modulator - generates multi-tone audio for simultaneous transmission
///
/// Transmits 3 bytes (6 nibbles) per symbol using 6 simultaneous frequencies.
/// Each nibble (4 bits, value 0-15) selects one frequency from a band of 16 frequencies.
/// The 6 frequencies are transmitted simultaneously in the same time slot.
pub struct FskModulator {
    sample_rate: f32,
}

impl FskModulator {
    pub fn new() -> Self {
        Self {
            sample_rate: crate::SAMPLE_RATE as f32,
        }
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
    pub fn modulate_symbol(&mut self, bytes: &[u8]) -> Result<Vec<f32>> {
        if bytes.len() != FSK_BYTES_PER_SYMBOL {
            return Err(AudioModemError::InvalidInputSize);
        }

        let symbol_samples = FSK_SYMBOL_SAMPLES;
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
            let band_offset = nibble_idx * FSK_BINS_PER_BAND;
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

        self.apply_edge_taper(&mut samples);

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

    fn taper_length(&self, symbol_samples: usize) -> usize {
        let mut taper =
            ((symbol_samples as f32) * FSK_EDGE_TAPER_RATIO).round() as usize;
        if taper < FSK_MIN_TAPER_SAMPLES {
            taper = FSK_MIN_TAPER_SAMPLES;
        }
        let half_symbol = symbol_samples / 2;
        if taper > half_symbol {
            taper = half_symbol;
        }
        taper
    }

    fn apply_edge_taper(&self, samples: &mut [f32]) {
        let taper_len = self.taper_length(samples.len());
        if taper_len == 0 {
            return;
        }

        let window = raised_cosine_window(samples.len(), taper_len);
        let avg = window.iter().sum::<f32>() / samples.len() as f32;
        let normalization = if avg > 0.0 { 1.0 / avg } else { 1.0 };

        for (sample, &weight) in samples.iter_mut().zip(window.iter()) {
            *sample *= weight * normalization;
        }
    }
}

/// FSK demodulator - detects multiple simultaneous frequencies using FFT
///
/// Analyzes the spectrum to find 6 simultaneous tones, each representing a nibble.
pub struct FskDemodulator {
    sample_rate: f32,
}

impl FskDemodulator {
    pub fn new() -> Self {
        Self {
            sample_rate: crate::SAMPLE_RATE as f32,
    }
    }

    /// Compute power spectrum using simple DFT for our specific frequency bins
    ///
    /// This is more efficient than full FFT since we only need 96 specific bins.
    /// For each bin, we compute the magnitude using Goertzel-like approach.
    fn compute_spectrum(&self, samples: &[f32]) -> Vec<f32> {
        let conditioned = self.preprocess_symbol(samples);
        let n = conditioned.len();
        let mut spectrum = vec![0.0f32; FSK_NUM_BINS];

        for bin in 0..FSK_NUM_BINS {
            let freq = bin_to_freq(bin);
            let k = (0.5 + (n as f32 * freq / self.sample_rate)) as usize;
            let omega = 2.0 * PI * k as f32 / n as f32;
            let coeff = 2.0 * omega.cos();

            let mut q1 = 0.0;
            let mut q2 = 0.0;

            // Goertzel filter
            for &sample in &conditioned {
                let q0 = coeff * q1 - q2 + sample;
                q2 = q1;
                q1 = q0;
            }

            // Compute power (magnitude squared)
            let real = q1 - q2 * omega.cos();
            let imag = q2 * omega.sin();
            spectrum[bin] = real * real + imag * imag;
        }

        self.suppress_band_noise(&mut spectrum);
        spectrum
    }

    /// Demodulate a single multi-tone FSK symbol
    ///
    /// Detects 6 simultaneous tones, one from each band of 16 frequencies.
    /// Returns the 3 bytes encoded in the symbol.
    pub fn demodulate_symbol(&self, samples: &[f32]) -> Result<[u8; FSK_BYTES_PER_SYMBOL]> {
        if samples.len() != FSK_SYMBOL_SAMPLES {
            return Err(AudioModemError::InvalidInputSize);
        }

        // Compute power spectrum
        let spectrum = self.compute_spectrum(samples);

        // Detect the strongest frequency in each of the 6 bands
        let mut nibbles = [0u8; FSK_NIBBLES_PER_SYMBOL];

        for nibble_idx in 0..FSK_NIBBLES_PER_SYMBOL {
            let band_start = nibble_idx * FSK_BINS_PER_BAND;
            let band_end = band_start + FSK_BINS_PER_BAND;

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
    /// samples.len() must be a multiple of FSK_SYMBOL_SAMPLES
    pub fn demodulate(&self, samples: &[f32]) -> Result<Vec<u8>> {
        if samples.len() % FSK_SYMBOL_SAMPLES != 0 {
            return Err(AudioModemError::InvalidInputSize);
        }

        let mut bytes = Vec::new();
        for chunk in samples.chunks(FSK_SYMBOL_SAMPLES) {
            let symbol_bytes = self.demodulate_symbol(chunk)?;
            bytes.extend_from_slice(&symbol_bytes);
        }

        Ok(bytes)
    }

    fn preprocess_symbol(&self, samples: &[f32]) -> Vec<f32> {
        let mut buffer = samples.to_vec();
        if buffer.is_empty() {
            return buffer;
        }

        // Remove DC so that leakage into low bins does not trip detection.
        let mean = buffer.iter().sum::<f32>() / buffer.len() as f32;
        for sample in buffer.iter_mut() {
            *sample -= mean;
        }

        let taper_len = self.analysis_taper_length(buffer.len());
        if taper_len > 0 {
            let window = raised_cosine_window(buffer.len(), taper_len);
            for (sample, weight) in buffer.iter_mut().zip(window.iter()) {
                *sample *= *weight;
            }
        }

        // Apply light AGC so we focus on frequency content, not amplitude.
        let rms = (buffer.iter().map(|&s| s * s).sum::<f32>() / buffer.len() as f32).sqrt();
        if rms > FSK_MIN_RMS {
            let gain = FSK_TARGET_RMS / rms;
            for sample in buffer.iter_mut() {
                *sample *= gain;
            }
        }

        buffer
    }

    fn analysis_taper_length(&self, len: usize) -> usize {
        if len == 0 {
            return 0;
        }
        let mut taper = ((len as f32) * FSK_ANALYSIS_TAPER_RATIO).round() as usize;
        if taper < FSK_ANALYSIS_MIN_TAPER_SAMPLES {
            taper = FSK_ANALYSIS_MIN_TAPER_SAMPLES;
        }
        let half = len / 2;
        if taper > half {
            taper = half;
        }
        taper
    }

    fn suppress_band_noise(&self, spectrum: &mut [f32]) {
        for band_start in (0..FSK_NUM_BINS).step_by(FSK_BINS_PER_BAND) {
            let band_end = band_start + FSK_BINS_PER_BAND;
            let band_slice = &mut spectrum[band_start..band_end];

            let mut sorted = band_slice.to_vec();
            sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));
            let median = sorted[sorted.len() / 2];
            let floor = (median + FSK_NOISE_FLOOR_EPSILON).max(FSK_MIN_NOISE_FLOOR);

            for value in band_slice.iter_mut() {
                *value = (*value - floor).max(0.0);
            }
        }
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
    fn test_fsk_symbol_has_edge_taper() {
        let mut modulator = FskModulator::new();
        let bytes = [0x10, 0x32, 0x54];
        let samples = modulator.modulate_symbol(&bytes).unwrap();
        let taper_len = modulator.taper_length(samples.len());
        assert!(taper_len > 0);

        // Edge samples should be strongly suppressed
        assert!(samples[0].abs() < 1e-4);
        assert!(samples[samples.len() - 1].abs() < 1e-4);

        // Average energy near the center should be higher than at the edges
        let edge_energy: f32 = samples
            .iter()
            .take(taper_len)
            .map(|s| s.abs())
            .sum::<f32>()
            / taper_len as f32;
        let mid_start = samples.len() / 2 - taper_len / 2;
        let mid_energy: f32 = samples
            .iter()
            .skip(mid_start)
            .take(taper_len)
            .map(|s| s.abs())
            .sum::<f32>()
            / taper_len as f32;

        assert!(
            mid_energy > edge_energy,
            "mid_energy={} edge_energy={}",
            mid_energy,
            edge_energy
        );
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

    #[test]
    fn test_fsk_demodulator_gain_invariance() {
        let mut modulator = FskModulator::new();
        let demodulator = FskDemodulator::new();
        let bytes = [0x5A, 0xC3, 0x9F];
        let samples = modulator.modulate_symbol(&bytes).unwrap();

        for gain in [0.1, 0.5, 1.0, 2.5, 5.0] {
            let scaled: Vec<f32> = samples.iter().map(|s| s * gain).collect();
            let decoded = demodulator.demodulate_symbol(&scaled).unwrap();
            assert_eq!(decoded, bytes, "Failed at gain {}", gain);
        }
    }

    #[test]
    fn test_fsk_demodulator_dc_rejection() {
        let mut modulator = FskModulator::new();
        let demodulator = FskDemodulator::new();
        let bytes = [0x42, 0x01, 0x9C];
        let base = modulator.modulate_symbol(&bytes).unwrap();

        for offset in [-0.2, 0.3] {
            let offset_samples: Vec<f32> = base.iter().map(|s| s + offset).collect();
            let decoded = demodulator.demodulate_symbol(&offset_samples).unwrap();
            assert_eq!(decoded, bytes, "Failed with offset {}", offset);
        }
    }
}
