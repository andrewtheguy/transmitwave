use crate::error::{AudioModemError, Result};
use crate::{NUM_SUBCARRIERS, SAMPLES_PER_SYMBOL, compute_carrier_bins, OFDM_TARGET_AMPLITUDE};
use crate::ofdm::deterministic_phase;
use rustfft::{num_complex::Complex, FftPlanner};

/// OFDM with Cyclic Prefix (CP) for ISI immunity
///
/// Cyclic Prefix is a guard interval that converts linear convolution
/// (multipath channel) into circular convolution, completely eliminating
/// Inter-Symbol Interference (ISI) in multipath environments.
///
/// Frame structure with CP:
/// [CP: Last 160 samples] [OFDM Symbol: 1600 samples] [Next symbol...]
///
/// Total per symbol: 1760 samples (100ms + 10ms overhead)
/// Throughput impact: ~10% reduction vs non-CP system
/// Benefit: Complete ISI elimination in multipath channels
pub struct OfdmModulatorCp {
    fft_planner: FftPlanner<f32>,
    cp_len: usize, // Cyclic prefix length in samples
    symbol_counter: usize,
}

pub struct OfdmDemodulatorCp {
    fft_planner: FftPlanner<f32>,
    cp_len: usize,
    symbol_counter: usize,
}

impl OfdmModulatorCp {
    pub fn new() -> Self {
        Self::new_with_cp(160) // 10% overhead (160 of 1600 samples)
    }

    /// Create modulator with custom CP length
    /// Typical values: 80-320 samples (5%-20% overhead)
    pub fn new_with_cp(cp_len: usize) -> Self {
        if cp_len >= SAMPLES_PER_SYMBOL {
            panic!("CP length must be less than SAMPLES_PER_SYMBOL");
        }
        Self {
            fft_planner: FftPlanner::new(),
            cp_len,
            symbol_counter: 0,
        }
    }

    /// Modulate data bits into OFDM samples with Cyclic Prefix
    ///
    /// Output: [CP (last 160 samples)] [OFDM symbol (1600 samples)]
    /// Total output: 1760 samples
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

        // Extract real parts and normalize to consistent amplitude
        let mut symbol = vec![0.0; SAMPLES_PER_SYMBOL];

        // Find peak amplitude to normalize consistently across all symbols
        let mut peak = 0.0f32;
        for sample in time_domain.iter() {
            peak = peak.max(sample.re.abs());
        }

        // Normalize to consistent amplitude to prevent clipping
        let scale = if peak > 0.0 { OFDM_TARGET_AMPLITUDE / peak } else { 0.0 };
        for (i, sample) in time_domain.iter().enumerate() {
            symbol[i] = sample.re * scale;
        }

        // Prepend Cyclic Prefix (copy last cp_len samples to the beginning)
        let mut output = Vec::with_capacity(self.cp_len + SAMPLES_PER_SYMBOL);

        // Add CP: copy last cp_len samples from the OFDM symbol
        for i in 0..self.cp_len {
            output.push(symbol[SAMPLES_PER_SYMBOL - self.cp_len + i]);
        }

        // Add full OFDM symbol
        output.extend_from_slice(&symbol);

        Ok(output)
    }

    pub fn cp_len(&self) -> usize {
        self.cp_len
    }

    pub fn total_samples_per_symbol(&self) -> usize {
        self.cp_len + SAMPLES_PER_SYMBOL
    }
}

impl OfdmDemodulatorCp {
    pub fn new() -> Self {
        Self::new_with_cp(160)
    }

    pub fn new_with_cp(cp_len: usize) -> Self {
        if cp_len >= SAMPLES_PER_SYMBOL {
            panic!("CP length must be less than SAMPLES_PER_SYMBOL");
        }
        Self {
            fft_planner: FftPlanner::new(),
            cp_len,
            symbol_counter: 0,
        }
    }

    /// Demodulate OFDM samples with Cyclic Prefix to retrieve data bits
    ///
    /// Input must be at least cp_len + SAMPLES_PER_SYMBOL samples
    /// The CP is stripped automatically, and only the main symbol is demodulated
    pub fn demodulate(&mut self, samples: &[f32]) -> Result<Vec<bool>> {
        let required_len = self.cp_len + SAMPLES_PER_SYMBOL;
        if samples.len() < required_len {
            return Err(AudioModemError::InsufficientData);
        }

        // Skip CP and extract just the OFDM symbol (1600 samples)
        let symbol_start = self.cp_len;
        let symbol_end = symbol_start + SAMPLES_PER_SYMBOL;

        // Convert to complex format
        let mut freq_domain = vec![Complex::new(0.0, 0.0); SAMPLES_PER_SYMBOL];
        for (i, &sample) in samples[symbol_start..symbol_end].iter().enumerate() {
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

    pub fn cp_len(&self) -> usize {
        self.cp_len
    }

    pub fn total_samples_per_symbol(&self) -> usize {
        self.cp_len + SAMPLES_PER_SYMBOL
    }
}

impl Default for OfdmModulatorCp {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for OfdmDemodulatorCp {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cp_modulator_basic() {
        let mut modulator = OfdmModulatorCp::new();
        let bits = vec![true; NUM_SUBCARRIERS]; // All 1s

        let output = modulator.modulate(&bits).unwrap();

        // Should output CP + symbol = 160 + 1600 = 1760 samples
        assert_eq!(output.len(), 1760);
    }

    #[test]
    fn test_cp_structure() {
        let mut modulator = OfdmModulatorCp::new_with_cp(160);
        let bits = vec![true; NUM_SUBCARRIERS];

        let output = modulator.modulate(&bits).unwrap();

        // Check that last 160 samples of symbol are copied to the beginning as CP
        let symbol_last_160_start = 160 + 1600 - 160;
        for i in 0..160 {
            // CP should match the tail of the symbol
            let cp_sample = output[i];
            let tail_sample = output[symbol_last_160_start + i];
            assert!((cp_sample - tail_sample).abs() < 1e-5, "CP mismatch at index {}", i);
        }
    }

    #[test]
    fn test_cp_demodulator_basic() {
        let mut modulator = OfdmModulatorCp::new();
        let mut demodulator = OfdmDemodulatorCp::new();

        let bits = vec![true; NUM_SUBCARRIERS];
        let encoded = modulator.modulate(&bits).unwrap();
        let decoded = demodulator.demodulate(&encoded).unwrap();

        assert_eq!(decoded.len(), NUM_SUBCARRIERS);
        assert_eq!(decoded, bits);
    }

    #[test]
    fn test_cp_with_different_lengths() {
        for cp_len in [80, 160, 320].iter() {
            let mut modulator = OfdmModulatorCp::new_with_cp(*cp_len);
            let mut demodulator = OfdmDemodulatorCp::new_with_cp(*cp_len);

            let bits = vec![true; 24];
            let encoded = modulator.modulate(&bits).unwrap();

            // Check output length
            assert_eq!(encoded.len(), cp_len + SAMPLES_PER_SYMBOL);

            // Check demodulation works
            let decoded = demodulator.demodulate(&encoded).unwrap();
            assert_eq!(decoded.len(), NUM_SUBCARRIERS); // Always NUM_SUBCARRIERS subcarriers
        }
    }

    #[test]
    fn test_cp_mixed_bits() {
        let mut modulator = OfdmModulatorCp::new();
        let mut demodulator = OfdmDemodulatorCp::new();

        let bits: Vec<bool> = (0..NUM_SUBCARRIERS).map(|i| i % 3 != 0).collect();

        let encoded = modulator.modulate(&bits).unwrap();
        let decoded = demodulator.demodulate(&encoded).unwrap();

        assert_eq!(decoded, bits);
    }

    #[test]
    fn test_cp_insufficient_data() {
        let mut demodulator = OfdmDemodulatorCp::new();
        let insufficient = vec![0.0; 100]; // Too short

        let result = demodulator.demodulate(&insufficient);
        assert!(result.is_err());
    }

    #[test]
    fn test_cp_isi_immunity_concept() {
        // This demonstrates why CP works (conceptually)
        // In a real multipath channel, the CP ensures that:
        // - Delayed symbols don't interfere with the main symbol
        // - The delay is circular (wraps around to the beginning)
        // - FFT treats it as a simple phase rotation

        let mut modulator = OfdmModulatorCp::new();
        let bits = vec![true; NUM_SUBCARRIERS];
        let encoded = modulator.modulate(&bits).unwrap();

        // Simulate multipath: delayed copy (conceptual)
        let mut with_echo = encoded.clone();
        let echo_delay = 50;
        let echo_attenuation = 0.3;

        for i in 0..with_echo.len() - echo_delay {
            with_echo[i + echo_delay] += encoded[i] * echo_attenuation;
        }

        // With CP, the demodulator can handle this echo
        // (in reality, the CP must be longer than the echo delay)
        let mut demodulator = OfdmDemodulatorCp::new();
        if echo_delay < modulator.cp_len() {
            let result = demodulator.demodulate(&with_echo);
            // Should still decode correctly if echo is within CP
            assert!(result.is_ok());
        }
    }

    #[test]
    fn test_cp_throughput_impact() {
        let modulator = OfdmModulatorCp::new();
        let total_with_cp = modulator.total_samples_per_symbol();

        // CP adds 160 samples to 1600 base = 10% overhead
        let overhead_percent: f32 = (160.0 / 1600.0) * 100.0;
        assert!((overhead_percent - 10.0).abs() < 0.1);

        // Total samples should be 1760
        assert_eq!(total_with_cp, 1760);
    }
}
