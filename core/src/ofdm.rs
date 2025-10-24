use crate::error::{AudioModemError, Result};
use crate::{NUM_SUBCARRIERS, SAMPLES_PER_SYMBOL, compute_carrier_bins, OFDM_TARGET_AMPLITUDE};
use rustfft::{num_complex::Complex, FftPlanner};
use std::f32::consts::PI;

/// Generate a deterministic phase offset for each subcarrier using a hash function
/// This creates quasi-random but reproducible phase offsets for phase randomization
pub(crate) fn deterministic_phase(subcarrier_index: usize, symbol_index: usize) -> f32 {
    // Combine subcarrier index and symbol index for per-symbol dithering
    let combined = subcarrier_index.wrapping_mul(2654435761) ^ symbol_index.wrapping_mul(2246822519);
    (combined % 1000000) as f32 / 1000000.0 * 2.0 * PI
}

pub struct OfdmModulator {
    fft_planner: FftPlanner<f32>,
    symbol_counter: usize,
}

pub struct OfdmDemodulator {
    fft_planner: FftPlanner<f32>,
    symbol_counter: usize,
}

impl OfdmModulator {
    pub fn new() -> Self {
        Self {
            fft_planner: FftPlanner::new(),
            symbol_counter: 0,
        }
    }

    /// Modulate data bits into OFDM samples
    /// Each bit is BPSK modulated on a subcarrier at the configured frequency range
    pub fn modulate(&mut self, bits: &[bool]) -> Result<Vec<f32>> {
        if bits.len() > NUM_SUBCARRIERS {
            return Err(AudioModemError::InvalidInputSize);
        }

        // Create frequency domain symbols (BPSK: 1.0 for true, -1.0 for false)
        let mut freq_domain = vec![Complex::new(0.0, 0.0); SAMPLES_PER_SYMBOL];

        // Place subcarriers at the configured frequency range with deterministic phase randomization
        let carrier_bins = compute_carrier_bins();
        let current_symbol = self.symbol_counter;
        self.symbol_counter = self.symbol_counter.wrapping_add(1);

        for (i, &bit) in bits.iter().enumerate() {
            let amplitude = if bit { 1.0 } else { -1.0 };
            let phase = deterministic_phase(i, current_symbol);
            let bin = carrier_bins[i];
            if bin < SAMPLES_PER_SYMBOL {
                // Apply BPSK with phase randomization (varies per symbol)
                freq_domain[bin] = Complex::new(amplitude * phase.cos(), amplitude * phase.sin());
            }
        }

        // IFFT to get time domain samples
        let fft = self.fft_planner.plan_fft_inverse(SAMPLES_PER_SYMBOL);
        let mut time_domain = vec![Complex::new(0.0, 0.0); SAMPLES_PER_SYMBOL];
        time_domain.copy_from_slice(&freq_domain);

        fft.process(&mut time_domain);

        // Extract real parts and normalize
        let mut samples = vec![0.0; SAMPLES_PER_SYMBOL];

        // Find peak amplitude to normalize consistently across all symbols
        let mut peak = 0.0f32;
        for sample in time_domain.iter() {
            peak = peak.max(sample.re.abs());
        }

        // Normalize to consistent amplitude to prevent clipping
        // This ensures all symbols have the same perceived loudness regardless of bit pattern
        let scale = if peak > 0.0 { OFDM_TARGET_AMPLITUDE / peak } else { 0.0 };

        for (i, sample) in time_domain.iter().enumerate() {
            samples[i] = sample.re * scale;
        }

        Ok(samples)
    }
}

impl OfdmDemodulator {
    pub fn new() -> Self {
        Self {
            fft_planner: FftPlanner::new(),
            symbol_counter: 0,
        }
    }

    /// Demodulate OFDM samples to retrieve data bits
    pub fn demodulate(&mut self, samples: &[f32]) -> Result<Vec<bool>> {
        if samples.len() < SAMPLES_PER_SYMBOL {
            return Err(AudioModemError::InsufficientData);
        }

        // Convert to complex format
        let mut freq_domain = vec![Complex::new(0.0, 0.0); SAMPLES_PER_SYMBOL];
        for (i, &sample) in samples[..SAMPLES_PER_SYMBOL].iter().enumerate() {
            freq_domain[i] = Complex::new(sample, 0.0);
        }

        // FFT to frequency domain
        let fft = self.fft_planner.plan_fft_forward(SAMPLES_PER_SYMBOL);
        fft.process(&mut freq_domain);

        // Extract bits by threshold (BPSK detection) with phase compensation
        let mut bits = Vec::new();
        let carrier_bins = compute_carrier_bins();
        let current_symbol = self.symbol_counter;
        self.symbol_counter = self.symbol_counter.wrapping_add(1);

        for i in 0..NUM_SUBCARRIERS {
            let phase = deterministic_phase(i, current_symbol);
            let bin = carrier_bins[i];
            if bin < SAMPLES_PER_SYMBOL {
                // Apply phase compensation to remove the randomization
                let phase_compensated = freq_domain[bin] * Complex::new(phase.cos(), -phase.sin());
                // Threshold at 0: positive real part = 1, negative = 0
                let bit = phase_compensated.re > 0.0;
                bits.push(bit);
            }
        }

        Ok(bits)
    }
}

impl Default for OfdmModulator {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for OfdmDemodulator {
    fn default() -> Self {
        Self::new()
    }
}
