use crate::error::{AudioModemError, Result};
use crate::fft_correlation::{fft_correlate_1d, Mode};
use crate::sync::generate_chirp;
use crate::{CSS_SAMPLES_PER_SYMBOL, CSS_START_FREQ, CSS_END_FREQ};

/// Chirp Spread Spectrum (CSS) modulator
/// Encodes bits as chirp signals: up-chirp for 1, down-chirp for 0
pub struct CssModulator {
    symbol_samples: usize,
    start_freq: f32,
    end_freq: f32,
}

impl CssModulator {
    pub fn new() -> Result<Self> {
        Ok(Self {
            symbol_samples: CSS_SAMPLES_PER_SYMBOL,
            start_freq: CSS_START_FREQ,
            end_freq: CSS_END_FREQ,
        })
    }

    /// Modulate bits into CSS chirp signals
    /// Bit 1: up-chirp (start_freq → end_freq)
    /// Bit 0: down-chirp (end_freq → start_freq)
    pub fn modulate(&self, bits: &[bool]) -> Result<Vec<f32>> {
        let mut samples = Vec::new();

        for &bit in bits {
            let chirp = if bit {
                // Up-chirp for bit 1
                generate_chirp(
                    self.symbol_samples,
                    self.start_freq,
                    self.end_freq,
                    0.5,
                )
            } else {
                // Down-chirp for bit 0
                generate_chirp(
                    self.symbol_samples,
                    self.end_freq,
                    self.start_freq,
                    0.5,
                )
            };
            samples.extend_from_slice(&chirp);
        }

        Ok(samples)
    }
}

impl Default for CssModulator {
    fn default() -> Self {
        Self::new().unwrap()
    }
}

/// Chirp Spread Spectrum (CSS) demodulator
/// Decodes chirp signals back to bits using FFT-based correlation
pub struct CssDemodulator {
    symbol_samples: usize,
    start_freq: f32,
    end_freq: f32,
    up_chirp_template: Vec<f32>,
    down_chirp_template: Vec<f32>,
}

impl CssDemodulator {
    pub fn new() -> Result<Self> {
        let symbol_samples = CSS_SAMPLES_PER_SYMBOL;
        let start_freq = CSS_START_FREQ;
        let end_freq = CSS_END_FREQ;

        // Pre-generate chirp templates for correlation
        let up_chirp_template = generate_chirp(symbol_samples, start_freq, end_freq, 1.0);
        let down_chirp_template = generate_chirp(symbol_samples, end_freq, start_freq, 1.0);

        Ok(Self {
            symbol_samples,
            start_freq,
            end_freq,
            up_chirp_template,
            down_chirp_template,
        })
    }

    /// Demodulate CSS chirp signals back to bits
    /// Correlates each symbol with both up-chirp and down-chirp templates
    /// Decides bit value based on which correlation is stronger
    pub fn demodulate(&self, samples: &[f32]) -> Result<Vec<bool>> {
        if samples.len() < self.symbol_samples {
            return Err(AudioModemError::InsufficientData);
        }

        let mut bits = Vec::new();
        let mut pos = 0;

        while pos + self.symbol_samples <= samples.len() {
            let symbol_samples = &samples[pos..pos + self.symbol_samples];

            // Correlate with both templates
            let up_corr = match fft_correlate_1d(symbol_samples, &self.up_chirp_template, Mode::Valid) {
                Ok(corr) => {
                    // Find the peak correlation value
                    corr.iter().copied().fold(f32::NEG_INFINITY, f32::max)
                }
                Err(_) => 0.0,
            };

            let down_corr = match fft_correlate_1d(symbol_samples, &self.down_chirp_template, Mode::Valid) {
                Ok(corr) => {
                    // Find the peak correlation value
                    corr.iter().copied().fold(f32::NEG_INFINITY, f32::max)
                }
                Err(_) => 0.0,
            };

            // Decide bit based on which correlation is stronger
            let bit = up_corr > down_corr;
            bits.push(bit);

            pos += self.symbol_samples;
        }

        Ok(bits)
    }
}

impl Default for CssDemodulator {
    fn default() -> Self {
        Self::new().unwrap()
    }
}
