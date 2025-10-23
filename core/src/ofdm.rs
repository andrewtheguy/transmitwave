use crate::error::{AudioModemError, Result};
use crate::{NUM_SUBCARRIERS, SAMPLES_PER_SYMBOL};
use rustfft::{num_complex::Complex, FftPlanner};

pub struct OfdmModulator {
    fft_planner: FftPlanner<f32>,
}

pub struct OfdmDemodulator {
    fft_planner: FftPlanner<f32>,
}

impl OfdmModulator {
    pub fn new() -> Self {
        Self {
            fft_planner: FftPlanner::new(),
        }
    }

    /// Modulate data bits into OFDM samples
    /// Each bit is BPSK modulated on a subcarrier
    pub fn modulate(&mut self, bits: &[bool]) -> Result<Vec<f32>> {
        if bits.len() > NUM_SUBCARRIERS {
            return Err(AudioModemError::InvalidInputSize);
        }

        // Create frequency domain symbols (BPSK: 1.0 for true, -1.0 for false)
        let mut freq_domain = vec![Complex::new(0.0, 0.0); SAMPLES_PER_SYMBOL];

        for (i, &bit) in bits.iter().enumerate() {
            let amplitude = if bit { 1.0 } else { -1.0 };
            freq_domain[i] = Complex::new(amplitude, 0.0);
        }

        // IFFT to get time domain samples
        let fft = self.fft_planner.plan_fft_inverse(SAMPLES_PER_SYMBOL);
        let mut time_domain = vec![Complex::new(0.0, 0.0); SAMPLES_PER_SYMBOL];
        time_domain.copy_from_slice(&freq_domain);

        fft.process(&mut time_domain);

        // Extract real parts and normalize
        // Amplify by 4x to lift signal above acoustic recording noise floor
        // This ensures OFDM tail regions remain visible during recording/transmission
        let mut samples = vec![0.0; SAMPLES_PER_SYMBOL];
        let scale = 4.0 / (SAMPLES_PER_SYMBOL as f32).sqrt();
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

        // Extract bits by threshold (BPSK detection)
        let mut bits = Vec::new();
        for i in 0..NUM_SUBCARRIERS {
            // Threshold at 0: positive real part = 1, negative = 0
            let bit = freq_domain[i].re > 0.0;
            bits.push(bit);
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
