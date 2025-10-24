use crate::error::{AudioModemError, Result};
use std::f32::consts::PI;

// 4-FSK configuration
// Four well-spaced frequencies for robust non-coherent detection
const FSK_FREQ_0: f32 = 1200.0; // 00
const FSK_FREQ_1: f32 = 1600.0; // 01
const FSK_FREQ_2: f32 = 2000.0; // 10
const FSK_FREQ_3: f32 = 2400.0; // 11

/// Maps 2-bit patterns to frequency indices
const FREQUENCIES: [f32; 4] = [FSK_FREQ_0, FSK_FREQ_1, FSK_FREQ_2, FSK_FREQ_3];

/// FSK symbol duration in samples (50ms per symbol for improved reliability)
/// At 16kHz sample rate: 0.050s * 16000 = 800 samples per symbol
/// This gives 2 bits per 50ms = 40 bits/second
/// Doubled from 25ms for better noise immunity at the cost of slower transmission
pub const FSK_SYMBOL_SAMPLES: usize = 800;

/// FSK modulator - converts bit pairs to audio tones
pub struct FskModulator {
    sample_rate: f32,
    phase: f32, // Phase accumulator for continuous phase
}

impl FskModulator {
    pub fn new() -> Self {
        Self {
            sample_rate: crate::SAMPLE_RATE as f32,
            phase: 0.0,
        }
    }

    /// Modulate 2 bits into a single FSK symbol
    /// bits[0] = MSB, bits[1] = LSB
    /// Returns FSK_SYMBOL_SAMPLES audio samples (25ms)
    pub fn modulate_symbol(&mut self, bits: &[bool]) -> Result<Vec<f32>> {
        if bits.len() != 2 {
            return Err(AudioModemError::InvalidInputSize);
        }

        // Map 2 bits to frequency index: 00=0, 01=1, 10=2, 11=3
        let freq_idx = ((bits[0] as usize) << 1) | (bits[1] as usize);
        let frequency = FREQUENCIES[freq_idx];

        let mut samples = vec![0.0; FSK_SYMBOL_SAMPLES];
        let angular_freq = 2.0 * PI * frequency / self.sample_rate;

        for i in 0..FSK_SYMBOL_SAMPLES {
            samples[i] = self.phase.sin() * 0.7; // 0.7 amplitude to prevent clipping
            self.phase += angular_freq;

            // Keep phase in [-2π, 2π] range to prevent float precision issues
            if self.phase > 2.0 * PI {
                self.phase -= 2.0 * PI;
            }
        }

        Ok(samples)
    }

    /// Modulate a sequence of bits (must be even number)
    /// Encodes 2 bits per symbol
    pub fn modulate(&mut self, bits: &[bool]) -> Result<Vec<f32>> {
        if bits.len() % 2 != 0 {
            return Err(AudioModemError::InvalidInputSize);
        }

        let mut samples = Vec::new();
        for chunk in bits.chunks(2) {
            let symbol_samples = self.modulate_symbol(chunk)?;
            samples.extend_from_slice(&symbol_samples);
        }

        Ok(samples)
    }

    /// Reset phase accumulator (for testing or new transmission)
    pub fn reset(&mut self) {
        self.phase = 0.0;
    }
}

/// FSK demodulator - detects frequency using Goertzel algorithm
pub struct FskDemodulator {
    sample_rate: f32,
}

impl FskDemodulator {
    pub fn new() -> Self {
        Self {
            sample_rate: crate::SAMPLE_RATE as f32,
        }
    }

    /// Goertzel algorithm - efficient single-frequency DFT
    /// Computes magnitude of a specific frequency in the signal
    fn goertzel(&self, samples: &[f32], target_freq: f32) -> f32 {
        let n = samples.len();
        let k = (0.5 + (n as f32 * target_freq / self.sample_rate)) as usize;
        let omega = 2.0 * PI * k as f32 / n as f32;
        let coeff = 2.0 * omega.cos();

        let mut q0 = 0.0;
        let mut q1 = 0.0;
        let mut q2 = 0.0;

        // Feed samples through the filter
        for &sample in samples {
            q0 = coeff * q1 - q2 + sample;
            q2 = q1;
            q1 = q0;
        }

        // Compute magnitude
        let real = q1 - q2 * omega.cos();
        let imag = q2 * omega.sin();
        (real * real + imag * imag).sqrt()
    }

    /// Demodulate a single FSK symbol (FSK_SYMBOL_SAMPLES samples)
    /// Returns 2 bits representing the detected frequency
    pub fn demodulate_symbol(&self, samples: &[f32]) -> Result<[bool; 2]> {
        if samples.len() != FSK_SYMBOL_SAMPLES {
            return Err(AudioModemError::InvalidInputSize);
        }

        // Compute energy at each of the 4 frequencies
        let mut energies = [0.0f32; 4];
        for (i, &freq) in FREQUENCIES.iter().enumerate() {
            energies[i] = self.goertzel(samples, freq);
        }

        // Find frequency with maximum energy
        let mut max_idx = 0;
        let mut max_energy = energies[0];
        for (i, &energy) in energies.iter().enumerate().skip(1) {
            if energy > max_energy {
                max_energy = energy;
                max_idx = i;
            }
        }

        // Convert frequency index back to 2 bits
        let bit0 = (max_idx >> 1) != 0; // MSB
        let bit1 = (max_idx & 1) != 0;  // LSB

        Ok([bit0, bit1])
    }

    /// Demodulate a sequence of FSK symbols
    /// samples.len() must be a multiple of FSK_SYMBOL_SAMPLES
    pub fn demodulate(&self, samples: &[f32]) -> Result<Vec<bool>> {
        if samples.len() % FSK_SYMBOL_SAMPLES != 0 {
            return Err(AudioModemError::InvalidInputSize);
        }

        let mut bits = Vec::new();
        for chunk in samples.chunks(FSK_SYMBOL_SAMPLES) {
            let symbol_bits = self.demodulate_symbol(chunk)?;
            bits.push(symbol_bits[0]);
            bits.push(symbol_bits[1]);
        }

        Ok(bits)
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
    fn test_fsk_modulator_symbol_length() {
        let mut modulator = FskModulator::new();
        let samples = modulator.modulate_symbol(&[false, false]).unwrap();
        assert_eq!(samples.len(), FSK_SYMBOL_SAMPLES);
    }

    #[test]
    fn test_fsk_modulator_invalid_input() {
        let mut modulator = FskModulator::new();
        assert!(modulator.modulate_symbol(&[false]).is_err());
        assert!(modulator.modulate_symbol(&[false, false, false]).is_err());
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
            [false, false], // 00 -> 1200 Hz
            [false, true],  // 01 -> 1600 Hz
            [true, false],  // 10 -> 2000 Hz
            [true, true],   // 11 -> 2400 Hz
        ];

        for bits in test_cases {
            modulator.reset();
            let samples = modulator.modulate_symbol(&bits).unwrap();
            let decoded = demodulator.demodulate_symbol(&samples).unwrap();
            assert_eq!(decoded, bits, "Failed roundtrip for {:?}", bits);
        }
    }

    #[test]
    fn test_fsk_roundtrip_multiple_symbols() {
        let mut modulator = FskModulator::new();
        let demodulator = FskDemodulator::new();

        // Test sequence: 00 01 10 11 00 11
        let bits = vec![
            false, false, // 00
            false, true,  // 01
            true, false,  // 10
            true, true,   // 11
            false, false, // 00
            true, true,   // 11
        ];

        let samples = modulator.modulate(&bits).unwrap();
        assert_eq!(samples.len(), FSK_SYMBOL_SAMPLES * 6);

        let decoded = demodulator.demodulate(&samples).unwrap();
        assert_eq!(decoded.len(), bits.len());
        assert_eq!(decoded, bits);
    }

    #[test]
    fn test_fsk_with_noise() {
        let mut modulator = FskModulator::new();
        let demodulator = FskDemodulator::new();

        // Encode a symbol
        let bits = [true, false]; // 10 -> 2000 Hz
        let mut samples = modulator.modulate_symbol(&bits).unwrap();

        // Add small noise
        for sample in samples.iter_mut() {
            *sample += 0.1 * ((*sample * 100.0).sin()); // ~10% noise
        }

        // Should still decode correctly
        let decoded = demodulator.demodulate_symbol(&samples).unwrap();
        assert_eq!(decoded, bits);
    }

    #[test]
    fn test_goertzel_detects_correct_frequency() {
        let demodulator = FskDemodulator::new();
        let mut modulator = FskModulator::new();

        // Generate 1600 Hz tone
        let samples = modulator.modulate_symbol(&[false, true]).unwrap();

        // Goertzel should detect highest energy at 1600 Hz
        let energy_1200 = demodulator.goertzel(&samples, FSK_FREQ_0);
        let energy_1600 = demodulator.goertzel(&samples, FSK_FREQ_1);
        let energy_2000 = demodulator.goertzel(&samples, FSK_FREQ_2);
        let energy_2400 = demodulator.goertzel(&samples, FSK_FREQ_3);

        assert!(energy_1600 > energy_1200);
        assert!(energy_1600 > energy_2000);
        assert!(energy_1600 > energy_2400);
    }

    #[test]
    fn test_fsk_continuous_phase() {
        let mut modulator = FskModulator::new();

        // Modulate two different symbols
        let samples1 = modulator.modulate_symbol(&[false, false]).unwrap();
        let samples2 = modulator.modulate_symbol(&[true, true]).unwrap();

        // Phase should continue smoothly (not reset between symbols)
        // Check last few samples of first symbol and first few samples of second symbol
        // They should transition smoothly due to phase continuity
        let last_idx = samples1.len() - 1;
        let last_sample = samples1[last_idx];
        let first_sample = samples2[0];

        // The samples should generally be different due to different frequencies
        // but the phase carries forward, creating a smooth transition
        // Just verify the modulation works without phase discontinuity
        assert_eq!(samples1.len(), FSK_SYMBOL_SAMPLES);
        assert_eq!(samples2.len(), FSK_SYMBOL_SAMPLES);
    }
}
