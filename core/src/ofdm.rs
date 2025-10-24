use crate::error::{AudioModemError, Result};
use crate::{NUM_SUBCARRIERS, SAMPLES_PER_SYMBOL, SAMPLE_RATE};
use crate::fhss;
use rustfft::{num_complex::Complex, FftPlanner};

pub struct OfdmModulator {
    fft_planner: FftPlanner<f32>,
    num_frequency_hops: usize,
}

pub struct OfdmDemodulator {
    fft_planner: FftPlanner<f32>,
    num_frequency_hops: usize,
}

impl OfdmModulator {
    pub fn new() -> Self {
        Self {
            fft_planner: FftPlanner::new(),
            num_frequency_hops: 1,
        }
    }

    pub fn with_frequency_hops(num_hops: usize) -> Self {
        Self {
            fft_planner: FftPlanner::new(),
            num_frequency_hops: num_hops,
        }
    }

    /// Modulate data bits into OFDM samples
    /// Each bit is BPSK modulated on a subcarrier at the configured frequency range
    pub fn modulate(&mut self, bits: &[bool]) -> Result<Vec<f32>> {
        self.modulate_with_band(bits, 0)
    }

    /// Modulate with a specific frequency band (for FHSS)
    pub fn modulate_with_band(&mut self, bits: &[bool], band_index: usize) -> Result<Vec<f32>> {
        if bits.len() > NUM_SUBCARRIERS {
            return Err(AudioModemError::InvalidInputSize);
        }

        // Get frequency range for this band
        let (band_min_freq, band_max_freq) = fhss::get_band_frequencies(band_index, self.num_frequency_hops)?;
        let band_bandwidth = band_max_freq - band_min_freq;
        let subcarrier_spacing_in_band = band_bandwidth / NUM_SUBCARRIERS as f32;

        // Create frequency domain symbols (BPSK: 1.0 for true, -1.0 for false)
        let mut freq_domain = vec![Complex::new(0.0, 0.0); SAMPLES_PER_SYMBOL];

        // Place subcarriers within the selected frequency band
        // Calculate bin index for each subcarrier frequency
        let sample_rate = SAMPLE_RATE as f32;
        for (i, &bit) in bits.iter().enumerate() {
            let amplitude = if bit { 1.0 } else { -1.0 };
            let frequency = band_min_freq + (i as f32) * subcarrier_spacing_in_band;
            // Convert frequency to FFT bin: bin = (frequency / sample_rate) * SAMPLES_PER_SYMBOL
            let bin = ((frequency / sample_rate) * SAMPLES_PER_SYMBOL as f32) as usize;
            if bin < SAMPLES_PER_SYMBOL {
                freq_domain[bin] = Complex::new(amplitude, 0.0);
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

        // Normalize to consistent amplitude (0.7 to prevent clipping)
        // This ensures all symbols have the same perceived loudness regardless of bit pattern
        let target_amplitude = 0.7;
        let scale = if peak > 0.0 { target_amplitude / peak } else { 0.0 };

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
            num_frequency_hops: 1,
        }
    }

    pub fn with_frequency_hops(num_hops: usize) -> Self {
        Self {
            fft_planner: FftPlanner::new(),
            num_frequency_hops: num_hops,
        }
    }

    /// Demodulate OFDM samples to retrieve data bits
    pub fn demodulate(&mut self, samples: &[f32]) -> Result<Vec<bool>> {
        self.demodulate_with_band(samples, 0)
    }

    /// Demodulate from a specific frequency band (for FHSS)
    pub fn demodulate_with_band(&mut self, samples: &[f32], band_index: usize) -> Result<Vec<bool>> {
        if samples.len() < SAMPLES_PER_SYMBOL {
            return Err(AudioModemError::InsufficientData);
        }

        // Get frequency range for this band
        let (band_min_freq, band_max_freq) = fhss::get_band_frequencies(band_index, self.num_frequency_hops)?;
        let band_bandwidth = band_max_freq - band_min_freq;
        let subcarrier_spacing_in_band = band_bandwidth / NUM_SUBCARRIERS as f32;

        // Convert to complex format
        let mut freq_domain = vec![Complex::new(0.0, 0.0); SAMPLES_PER_SYMBOL];
        for (i, &sample) in samples[..SAMPLES_PER_SYMBOL].iter().enumerate() {
            freq_domain[i] = Complex::new(sample, 0.0);
        }

        // FFT to frequency domain
        let fft = self.fft_planner.plan_fft_forward(SAMPLES_PER_SYMBOL);
        fft.process(&mut freq_domain);

        // Extract bits by threshold (BPSK detection) at band-specific subcarrier frequencies
        let mut bits = Vec::new();
        let sample_rate = SAMPLE_RATE as f32;
        for i in 0..NUM_SUBCARRIERS {
            let frequency = band_min_freq + (i as f32) * subcarrier_spacing_in_band;
            // Convert frequency to FFT bin
            let bin = ((frequency / sample_rate) * SAMPLES_PER_SYMBOL as f32) as usize;
            if bin < SAMPLES_PER_SYMBOL {
                // Threshold at 0: positive real part = 1, negative = 0
                let bit = freq_domain[bin].re > 0.0;
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
