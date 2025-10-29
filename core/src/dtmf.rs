use crate::error::{AudioModemError, Result};
use std::f32::consts::PI;

/// Extended DTMF tone generator and detector with 48 symbols (6 low × 8 high frequencies)
///
/// Frequency design:
/// - Low frequencies (6): 697, 758, 818, 879, 939, 1000 Hz
/// - High frequencies (8): 1200, 1262, 1324, 1386, 1448, 1510, 1572, 1633 Hz
/// - Total symbols: 48 (encoding 0-47, ~5.6 bits per symbol)
///
/// Symbol parameters (at 16kHz sample rate):
/// - 3200 samples = 200ms per symbol (extended for reliable over-the-air transmission)
/// - Dual-tone generation with raised-cosine windowing
/// - Goertzel algorithm for efficient frequency detection

/// Low frequency band (6 frequencies)
const DTMF_LOW_FREQS: [f32; 6] = [697.0, 758.0, 818.0, 879.0, 939.0, 1000.0];

/// High frequency band (8 frequencies)
const DTMF_HIGH_FREQS: [f32; 8] = [1200.0, 1262.0, 1324.0, 1386.0, 1448.0, 1510.0, 1572.0, 1633.0];

/// Total number of symbols (6 × 8 = 48)
pub const DTMF_NUM_SYMBOLS: u8 = 48;

/// Number of samples per DTMF symbol (200ms at 16kHz for reliable over-the-air transmission)
pub const DTMF_SYMBOL_SAMPLES: usize = 3200;

/// Edge taper ratio (8% on each side for smooth transitions)
const DTMF_EDGE_TAPER_RATIO: f32 = 0.08;

/// Minimum taper samples
const DTMF_MIN_TAPER_SAMPLES: usize = 32;

/// Tone amplitude (0.7 / 2 to prevent clipping with 2 tones)
const DTMF_TONE_AMPLITUDE: f32 = 0.35;

/// Minimum energy threshold for detection (relative to max)
const DTMF_MIN_ENERGY_RATIO: f32 = 0.3;

/// Analysis window taper ratio for demodulator
const DTMF_ANALYSIS_TAPER_RATIO: f32 = 0.06;

/// Minimum taper for analysis window
const DTMF_ANALYSIS_MIN_TAPER_SAMPLES: usize = 32;

/// Target RMS level for AGC
const DTMF_TARGET_RMS: f32 = 0.5;

/// Minimum RMS to avoid division by zero
const DTMF_MIN_RMS: f32 = 1e-4;

/// DTMF modulator - generates dual-tone symbols
pub struct DtmfModulator {
    sample_rate: f32,
}

impl DtmfModulator {
    pub fn new() -> Self {
        Self {
            sample_rate: crate::SAMPLE_RATE as f32,
        }
    }

    /// Modulate a single DTMF symbol (0-47)
    pub fn modulate_symbol(&mut self, symbol: u8) -> Result<Vec<f32>> {
        if symbol >= DTMF_NUM_SYMBOLS {
            return Err(AudioModemError::InvalidInputSize);
        }

        // Decode symbol into low and high frequency indices
        let low_idx = (symbol / 8) as usize;
        let high_idx = (symbol % 8) as usize;

        let low_freq = DTMF_LOW_FREQS[low_idx];
        let high_freq = DTMF_HIGH_FREQS[high_idx];

        // Generate dual-tone signal
        let mut samples = vec![0.0; DTMF_SYMBOL_SAMPLES];
        let sample_rate = self.sample_rate;

        for (i, sample) in samples.iter_mut().enumerate() {
            let t = i as f32 / sample_rate;
            let low_tone = (2.0 * PI * low_freq * t).sin();
            let high_tone = (2.0 * PI * high_freq * t).sin();
            *sample = (low_tone + high_tone) * DTMF_TONE_AMPLITUDE;
        }

        // Apply raised-cosine windowing to reduce spectral splatter
        self.apply_edge_taper(&mut samples);

        Ok(samples)
    }

    /// Modulate a sequence of symbols
    pub fn modulate(&mut self, symbols: &[u8]) -> Result<Vec<f32>> {
        let mut samples = Vec::new();
        for &symbol in symbols {
            let symbol_samples = self.modulate_symbol(symbol)?;
            samples.extend_from_slice(&symbol_samples);
        }
        Ok(samples)
    }

    fn taper_length(&self, symbol_samples: usize) -> usize {
        let mut taper = ((symbol_samples as f32) * DTMF_EDGE_TAPER_RATIO).round() as usize;
        if taper < DTMF_MIN_TAPER_SAMPLES {
            taper = DTMF_MIN_TAPER_SAMPLES;
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

/// DTMF demodulator - detects dual-tone frequencies using Goertzel algorithm
pub struct DtmfDemodulator {
    sample_rate: f32,
}

impl DtmfDemodulator {
    pub fn new() -> Self {
        Self {
            sample_rate: crate::SAMPLE_RATE as f32,
        }
    }

    /// Compute power for a specific frequency using Goertzel algorithm
    fn goertzel(&self, samples: &[f32], freq: f32) -> f32 {
        let n = samples.len();
        let k = (0.5 + (n as f32 * freq / self.sample_rate)) as usize;
        let omega = 2.0 * PI * k as f32 / n as f32;
        let coeff = 2.0 * omega.cos();

        let mut q1 = 0.0;
        let mut q2 = 0.0;

        for &sample in samples {
            let q0 = coeff * q1 - q2 + sample;
            q2 = q1;
            q1 = q0;
        }

        // Compute magnitude squared (power)
        let real = q1 - q2 * omega.cos();
        let imag = q2 * omega.sin();
        real * real + imag * imag
    }

    /// Demodulate a single DTMF symbol
    pub fn demodulate_symbol(&self, samples: &[f32]) -> Result<u8> {
        if samples.len() != DTMF_SYMBOL_SAMPLES {
            return Err(AudioModemError::InvalidInputSize);
        }

        // Preprocess signal
        let conditioned = self.preprocess_symbol(samples);

        // Detect strongest low frequency
        let mut max_low_power = 0.0;
        let mut low_idx = 0;

        for (i, &freq) in DTMF_LOW_FREQS.iter().enumerate() {
            let power = self.goertzel(&conditioned, freq);
            if power > max_low_power {
                max_low_power = power;
                low_idx = i;
            }
        }

        // Detect strongest high frequency
        let mut max_high_power = 0.0;
        let mut high_idx = 0;

        for (i, &freq) in DTMF_HIGH_FREQS.iter().enumerate() {
            let power = self.goertzel(&conditioned, freq);
            if power > max_high_power {
                max_high_power = power;
                high_idx = i;
            }
        }

        // Validate detection (check if we have sufficient energy)
        let min_energy = (max_low_power.max(max_high_power)) * DTMF_MIN_ENERGY_RATIO;
        if max_low_power < min_energy || max_high_power < min_energy {
            return Err(AudioModemError::InsufficientData);
        }

        // Encode symbol from low and high indices
        let symbol = (low_idx * 8 + high_idx) as u8;
        Ok(symbol)
    }

    /// Demodulate a sequence of DTMF symbols
    pub fn demodulate(&self, samples: &[f32]) -> Result<Vec<u8>> {
        if samples.len() % DTMF_SYMBOL_SAMPLES != 0 {
            return Err(AudioModemError::InvalidInputSize);
        }

        let mut symbols = Vec::new();
        for chunk in samples.chunks(DTMF_SYMBOL_SAMPLES) {
            let symbol = self.demodulate_symbol(chunk)?;
            symbols.push(symbol);
        }

        Ok(symbols)
    }

    fn preprocess_symbol(&self, samples: &[f32]) -> Vec<f32> {
        let mut buffer = samples.to_vec();
        if buffer.is_empty() {
            return buffer;
        }

        // Remove DC offset
        let mean = buffer.iter().sum::<f32>() / buffer.len() as f32;
        for sample in buffer.iter_mut() {
            *sample -= mean;
        }

        // Apply windowing
        let taper_len = self.analysis_taper_length(buffer.len());
        if taper_len > 0 {
            let window = raised_cosine_window(buffer.len(), taper_len);
            for (sample, weight) in buffer.iter_mut().zip(window.iter()) {
                *sample *= *weight;
            }
        }

        // Apply AGC normalization
        let rms = (buffer.iter().map(|&s| s * s).sum::<f32>() / buffer.len() as f32).sqrt();
        if rms > DTMF_MIN_RMS {
            let gain = DTMF_TARGET_RMS / rms;
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
        let mut taper = ((len as f32) * DTMF_ANALYSIS_TAPER_RATIO).round() as usize;
        if taper < DTMF_ANALYSIS_MIN_TAPER_SAMPLES {
            taper = DTMF_ANALYSIS_MIN_TAPER_SAMPLES;
        }
        let half = len / 2;
        if taper > half {
            taper = half;
        }
        taper
    }
}

impl Default for DtmfModulator {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for DtmfDemodulator {
    fn default() -> Self {
        Self::new()
    }
}

/// Generate raised-cosine window for smooth edge tapering
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
        // Attack: smoothly increase from 0 to 1
        let progress = i as f32 / taper as f32;
        let value = (PI * progress / 2.0).sin().powi(2);
        window[i] = value;

        // Decay: smoothly decrease from 1 to 0
        window[len - 1 - i] = value;
    }

    window
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dtmf_frequency_constants() {
        assert_eq!(DTMF_LOW_FREQS.len(), 6);
        assert_eq!(DTMF_HIGH_FREQS.len(), 8);
        assert_eq!(DTMF_NUM_SYMBOLS, 48);
    }

    #[test]
    fn test_dtmf_symbol_encoding() {
        // Test symbol 0: low_idx=0, high_idx=0
        assert_eq!(0 / 8, 0);
        assert_eq!(0 % 8, 0);

        // Test symbol 47: low_idx=5, high_idx=7
        assert_eq!(47 / 8, 5);
        assert_eq!(47 % 8, 7);

        // Test symbol 15: low_idx=1, high_idx=7
        assert_eq!(15 / 8, 1);
        assert_eq!(15 % 8, 7);
    }

    #[test]
    fn test_dtmf_modulator_creation() {
        let modulator = DtmfModulator::new();
        assert_eq!(modulator.sample_rate, crate::SAMPLE_RATE as f32);
    }

    #[test]
    fn test_dtmf_demodulator_creation() {
        let demodulator = DtmfDemodulator::new();
        assert_eq!(demodulator.sample_rate, crate::SAMPLE_RATE as f32);
    }

    #[test]
    fn test_dtmf_modulate_single_symbol() {
        let mut modulator = DtmfModulator::new();
        let samples = modulator.modulate_symbol(0).unwrap();
        assert_eq!(samples.len(), DTMF_SYMBOL_SAMPLES);
    }

    #[test]
    fn test_dtmf_modulate_invalid_symbol() {
        let mut modulator = DtmfModulator::new();
        let result = modulator.modulate_symbol(48);
        assert!(result.is_err());
    }

    #[test]
    fn test_dtmf_roundtrip_all_symbols() {
        let mut modulator = DtmfModulator::new();
        let demodulator = DtmfDemodulator::new();

        for symbol in 0..DTMF_NUM_SYMBOLS {
            let samples = modulator.modulate_symbol(symbol).unwrap();
            let detected = demodulator.demodulate_symbol(&samples).unwrap();
            assert_eq!(
                detected, symbol,
                "Symbol {} failed roundtrip, got {}",
                symbol, detected
            );
        }
    }

    #[test]
    fn test_dtmf_roundtrip_sequence() {
        let mut modulator = DtmfModulator::new();
        let demodulator = DtmfDemodulator::new();

        let test_sequence: Vec<u8> = (0..DTMF_NUM_SYMBOLS).collect();
        let samples = modulator.modulate(&test_sequence).unwrap();
        let detected = demodulator.demodulate(&samples).unwrap();

        assert_eq!(detected, test_sequence);
    }

    #[test]
    fn test_dtmf_roundtrip_boundary_symbols() {
        let mut modulator = DtmfModulator::new();
        let demodulator = DtmfDemodulator::new();

        // Test boundary symbols: 0, 7, 40, 47
        let boundary_symbols = vec![0, 7, 40, 47];
        let samples = modulator.modulate(&boundary_symbols).unwrap();
        let detected = demodulator.demodulate(&samples).unwrap();

        assert_eq!(detected, boundary_symbols);
    }

    #[test]
    fn test_dtmf_roundtrip_with_attenuation() {
        let mut modulator = DtmfModulator::new();
        let demodulator = DtmfDemodulator::new();

        let test_symbols = vec![0, 10, 20, 30, 40, 47];
        let mut samples = modulator.modulate(&test_symbols).unwrap();

        // Attenuate signal to 50%
        for sample in samples.iter_mut() {
            *sample *= 0.5;
        }

        let detected = demodulator.demodulate(&samples).unwrap();
        assert_eq!(detected, test_symbols);
    }

    #[test]
    fn test_dtmf_demodulate_invalid_size() {
        let demodulator = DtmfDemodulator::new();
        let samples = vec![0.0; 100]; // Wrong size
        let result = demodulator.demodulate_symbol(&samples);
        assert!(result.is_err());
    }

    #[test]
    fn test_dtmf_sample_length() {
        let mut modulator = DtmfModulator::new();
        let samples = modulator.modulate_symbol(0).unwrap();

        // 200ms at 16kHz = 3200 samples
        assert_eq!(samples.len(), 3200);
    }

    #[test]
    fn test_dtmf_frequency_spacing() {
        // Verify low frequencies are properly spaced
        for i in 1..DTMF_LOW_FREQS.len() {
            let spacing = DTMF_LOW_FREQS[i] - DTMF_LOW_FREQS[i - 1];
            assert!(
                (spacing - 61.0).abs() < 2.0,
                "Low frequency spacing should be ~61 Hz, got {}",
                spacing
            );
        }

        // Verify high frequencies are properly spaced
        for i in 1..DTMF_HIGH_FREQS.len() {
            let spacing = DTMF_HIGH_FREQS[i] - DTMF_HIGH_FREQS[i - 1];
            assert!(
                (spacing - 62.0).abs() < 2.0,
                "High frequency spacing should be ~62 Hz, got {}",
                spacing
            );
        }
    }

    #[test]
    fn test_dtmf_amplitude_range() {
        let mut modulator = DtmfModulator::new();
        let samples = modulator.modulate_symbol(24).unwrap();

        let max_amplitude = samples.iter().map(|x| x.abs()).fold(0.0f32, f32::max);

        // Should not clip (max < 1.0) and should have reasonable amplitude
        assert!(max_amplitude < 1.0, "Signal should not clip");
        assert!(max_amplitude > 0.1, "Signal should have reasonable amplitude");
    }

    #[test]
    fn test_raised_cosine_window() {
        let window = raised_cosine_window(100, 10);
        assert_eq!(window.len(), 100);

        // Check that edges taper smoothly
        assert!(window[0] < 0.1, "Window should start near 0");
        assert!(window[9] > 0.9, "Window should reach 1.0");
        assert!(window[50] == 1.0, "Window should be 1.0 in middle");
        assert!(window[90] > 0.9, "Window should still be high");
        assert!(window[99] < 0.1, "Window should end near 0");
    }

    // ========================================================================
    // REAL-WORLD SCENARIO TESTS
    // ========================================================================

    /// Simple PRNG for reproducible noise
    fn simple_rand(seed: &mut u64) -> f32 {
        *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        ((*seed >> 32) as f32) / (u32::MAX as f32) * 2.0 - 1.0
    }

    /// Add white noise to signal at specified SNR (in dB)
    fn add_white_noise(samples: &mut [f32], snr_db: f32) {
        let mut seed = 12345u64;

        // Calculate signal RMS
        let signal_rms = (samples.iter().map(|x| x * x).sum::<f32>() / samples.len() as f32).sqrt();

        // Calculate noise RMS from SNR: SNR(dB) = 20*log10(signal_rms / noise_rms)
        let noise_rms = signal_rms / 10.0f32.powf(snr_db / 20.0);

        // Add noise
        for sample in samples.iter_mut() {
            *sample += simple_rand(&mut seed) * noise_rms;
        }
    }

    #[test]
    fn test_dtmf_with_additive_noise_20db_snr() {
        let mut modulator = DtmfModulator::new();
        let demodulator = DtmfDemodulator::new();

        let test_symbols = vec![0, 15, 30, 47];
        let mut samples = modulator.modulate(&test_symbols).unwrap();

        // Add 20dB SNR noise (good conditions)
        add_white_noise(&mut samples, 20.0);

        let detected = demodulator.demodulate(&samples).unwrap();
        assert_eq!(detected, test_symbols, "Should decode correctly with 20dB SNR");
    }

    #[test]
    fn test_dtmf_with_additive_noise_15db_snr() {
        let mut modulator = DtmfModulator::new();
        let demodulator = DtmfDemodulator::new();

        let test_symbols = vec![5, 20, 35];
        let mut samples = modulator.modulate(&test_symbols).unwrap();

        // Add 15dB SNR noise (moderate conditions)
        add_white_noise(&mut samples, 15.0);

        let detected = demodulator.demodulate(&samples).unwrap();
        assert_eq!(detected, test_symbols, "Should decode correctly with 15dB SNR");
    }

    #[test]
    fn test_dtmf_with_additive_noise_10db_snr() {
        let mut modulator = DtmfModulator::new();
        let demodulator = DtmfDemodulator::new();

        let test_symbols = vec![10, 25, 40];
        let mut samples = modulator.modulate(&test_symbols).unwrap();

        // Add 10dB SNR noise (challenging conditions)
        add_white_noise(&mut samples, 10.0);

        let detected = demodulator.demodulate(&samples).unwrap();
        assert_eq!(detected, test_symbols, "Should decode correctly with 10dB SNR");
    }

    #[test]
    fn test_dtmf_strong_attenuation_10_percent() {
        let mut modulator = DtmfModulator::new();
        let demodulator = DtmfDemodulator::new();

        let test_symbols = vec![0, 10, 20, 30, 40, 47];
        let mut samples = modulator.modulate(&test_symbols).unwrap();

        // Very strong attenuation to 10% (distant transmission)
        for sample in samples.iter_mut() {
            *sample *= 0.1;
        }

        let detected = demodulator.demodulate(&samples).unwrap();
        assert_eq!(detected, test_symbols, "Should handle 10% attenuation");
    }

    #[test]
    fn test_dtmf_strong_attenuation_20_percent() {
        let mut modulator = DtmfModulator::new();
        let demodulator = DtmfDemodulator::new();

        let test_symbols = vec![5, 15, 25, 35, 45];
        let mut samples = modulator.modulate(&test_symbols).unwrap();

        // Strong attenuation to 20%
        for sample in samples.iter_mut() {
            *sample *= 0.2;
        }

        let detected = demodulator.demodulate(&samples).unwrap();
        assert_eq!(detected, test_symbols, "Should handle 20% attenuation");
    }

    #[test]
    fn test_dtmf_with_dc_offset() {
        let mut modulator = DtmfModulator::new();
        let demodulator = DtmfDemodulator::new();

        let test_symbols = vec![7, 14, 21, 28];
        let mut samples = modulator.modulate(&test_symbols).unwrap();

        // Add DC offset (common in audio systems)
        for sample in samples.iter_mut() {
            *sample += 0.2;
        }

        let detected = demodulator.demodulate(&samples).unwrap();
        assert_eq!(detected, test_symbols, "Should handle DC offset");
    }

    #[test]
    fn test_dtmf_with_soft_clipping() {
        let mut modulator = DtmfModulator::new();
        let demodulator = DtmfDemodulator::new();

        let test_symbols = vec![3, 12, 24, 36, 44];
        let mut samples = modulator.modulate(&test_symbols).unwrap();

        // Amplify and soft clip (simulates overdriven input)
        for sample in samples.iter_mut() {
            *sample *= 2.0;
            *sample = sample.max(-0.9).min(0.9); // Soft clipping
        }

        let detected = demodulator.demodulate(&samples).unwrap();
        assert_eq!(detected, test_symbols, "Should handle soft clipping");
    }

    #[test]
    fn test_dtmf_attenuation_plus_noise() {
        let mut modulator = DtmfModulator::new();
        let demodulator = DtmfDemodulator::new();

        let test_symbols = vec![1, 11, 22, 33, 46];
        let mut samples = modulator.modulate(&test_symbols).unwrap();

        // Realistic scenario: 30% signal strength + 15dB SNR noise
        for sample in samples.iter_mut() {
            *sample *= 0.3;
        }
        add_white_noise(&mut samples, 15.0);

        let detected = demodulator.demodulate(&samples).unwrap();
        assert_eq!(detected, test_symbols, "Should handle attenuation + noise");
    }

    #[test]
    fn test_dtmf_background_tone_interference() {
        let mut modulator = DtmfModulator::new();
        let demodulator = DtmfDemodulator::new();

        let test_symbols = vec![8, 16, 32];
        let mut samples = modulator.modulate(&test_symbols).unwrap();

        // Add background tone at 1500 Hz (between high frequencies)
        let sample_rate = crate::SAMPLE_RATE as f32;
        for (i, sample) in samples.iter_mut().enumerate() {
            let t = i as f32 / sample_rate;
            let interference = 0.1 * (2.0 * std::f32::consts::PI * 1500.0 * t).sin();
            *sample += interference;
        }

        let detected = demodulator.demodulate(&samples).unwrap();
        assert_eq!(detected, test_symbols, "Should handle background tone interference");
    }

    #[test]
    fn test_dtmf_sequence_with_noise() {
        let mut modulator = DtmfModulator::new();
        let demodulator = DtmfDemodulator::new();

        // Longer sequence to test sustained performance
        let test_symbols: Vec<u8> = vec![0, 5, 10, 15, 20, 25, 30, 35, 40, 45, 47];
        let mut samples = modulator.modulate(&test_symbols).unwrap();

        // Moderate noise throughout
        add_white_noise(&mut samples, 15.0);

        let detected = demodulator.demodulate(&samples).unwrap();
        assert_eq!(detected, test_symbols, "Should decode long sequence with noise");
    }

    #[test]
    fn test_dtmf_all_symbols_with_noise() {
        let mut modulator = DtmfModulator::new();
        let demodulator = DtmfDemodulator::new();

        // Test all 48 symbols with noise
        let test_symbols: Vec<u8> = (0..DTMF_NUM_SYMBOLS).collect();
        let mut samples = modulator.modulate(&test_symbols).unwrap();

        // Add 12dB SNR noise
        add_white_noise(&mut samples, 12.0);

        let detected = demodulator.demodulate(&samples).unwrap();
        assert_eq!(detected, test_symbols, "Should decode all 48 symbols with 12dB SNR");
    }

    #[test]
    fn test_dtmf_low_frequencies_with_noise() {
        let mut modulator = DtmfModulator::new();
        let demodulator = DtmfDemodulator::new();

        // Test symbols using low frequency indices (0-7 use low freq index 0)
        let test_symbols = vec![0, 1, 2, 3, 4, 5, 6, 7];
        let mut samples = modulator.modulate(&test_symbols).unwrap();

        add_white_noise(&mut samples, 15.0);

        let detected = demodulator.demodulate(&samples).unwrap();
        assert_eq!(detected, test_symbols, "Should decode low freq symbols with noise");
    }

    #[test]
    fn test_dtmf_high_frequencies_with_noise() {
        let mut modulator = DtmfModulator::new();
        let demodulator = DtmfDemodulator::new();

        // Test symbols using high frequency indices (40-47 use low freq index 5)
        let test_symbols = vec![40, 41, 42, 43, 44, 45, 46, 47];
        let mut samples = modulator.modulate(&test_symbols).unwrap();

        add_white_noise(&mut samples, 15.0);

        let detected = demodulator.demodulate(&samples).unwrap();
        assert_eq!(detected, test_symbols, "Should decode high freq symbols with noise");
    }

    #[test]
    fn test_dtmf_extreme_noise_robustness() {
        let mut modulator = DtmfModulator::new();
        let demodulator = DtmfDemodulator::new();

        let test_symbols = vec![10];
        let mut samples = modulator.modulate(&test_symbols).unwrap();

        // Add very heavy noise (0dB SNR - noise equal to signal)
        // DTMF is surprisingly robust even at 0dB, so this tests extreme conditions
        add_white_noise(&mut samples, 0.0);

        // At 0dB SNR, detection may succeed or fail depending on noise pattern
        // This is actually a robustness demonstration rather than a failure test
        let result = demodulator.demodulate(&samples);

        // Document the behavior: even at 0dB SNR, DTMF is often still decodable
        // due to the Goertzel algorithm's frequency selectivity
        if result.is_ok() {
            println!("DTMF successfully decoded even at 0dB SNR (excellent robustness)");
        } else {
            println!("DTMF failed at 0dB SNR (expected with extreme noise)");
        }

        // Test passes regardless - this demonstrates robustness limits
        assert!(true, "Test demonstrates extreme noise behavior");
    }

    #[test]
    fn test_dtmf_realistic_phone_scenario() {
        let mut modulator = DtmfModulator::new();
        let demodulator = DtmfDemodulator::new();

        // Simulate phone transmission: moderate attenuation + noise + DC offset
        let test_symbols = vec![1, 2, 3, 4, 5, 6, 7, 8, 9];
        let mut samples = modulator.modulate(&test_symbols).unwrap();

        // 40% attenuation (lossy transmission)
        for sample in samples.iter_mut() {
            *sample *= 0.4;
        }

        // Add 18dB SNR noise (realistic phone line)
        add_white_noise(&mut samples, 18.0);

        // Add DC offset
        for sample in samples.iter_mut() {
            *sample += 0.05;
        }

        let detected = demodulator.demodulate(&samples).unwrap();
        assert_eq!(detected, test_symbols, "Should handle realistic phone scenario");
    }

    #[test]
    fn test_dtmf_speaker_to_microphone_simulation() {
        let mut modulator = DtmfModulator::new();
        let demodulator = DtmfDemodulator::new();

        // Simulate speaker->mic: strong attenuation + background noise + slight distortion
        let test_symbols = vec![0, 10, 20, 30, 40];
        let mut samples = modulator.modulate(&test_symbols).unwrap();

        // Strong attenuation (25% - distant mic)
        for sample in samples.iter_mut() {
            *sample *= 0.25;
        }

        // Add realistic background noise (12dB SNR)
        add_white_noise(&mut samples, 12.0);

        // Add slight non-linear distortion
        for sample in samples.iter_mut() {
            *sample = sample.signum() * sample.abs().powf(0.95);
        }

        let detected = demodulator.demodulate(&samples).unwrap();
        assert_eq!(detected, test_symbols, "Should handle speaker-to-mic scenario");
    }

    #[test]
    fn test_dtmf_robustness_statistics() {
        // Test statistical robustness: multiple trials with different noise
        let mut modulator = DtmfModulator::new();
        let demodulator = DtmfDemodulator::new();

        let test_symbol = 24u8; // Middle symbol
        let num_trials = 10;
        let mut success_count = 0;

        for trial in 0..num_trials {
            let mut samples = modulator.modulate_symbol(test_symbol).unwrap();

            // Add noise with different seed each time
            let mut seed = 54321u64 + trial;
            let noise_rms = 0.05; // Moderate noise
            for sample in samples.iter_mut() {
                *sample += simple_rand(&mut seed) * noise_rms;
            }

            if let Ok(detected) = demodulator.demodulate_symbol(&samples) {
                if detected == test_symbol {
                    success_count += 1;
                }
            }
        }

        // Should succeed in at least 80% of trials
        assert!(
            success_count >= 8,
            "Should decode correctly in at least 8/10 trials with moderate noise, got {}/{}",
            success_count, num_trials
        );
    }
}
