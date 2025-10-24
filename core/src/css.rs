use crate::error::{AudioModemError, Result};
use crate::fft_correlation::{fft_correlate_1d, Mode};
use crate::sync::generate_chirp;
use crate::{CSS_SAMPLES_PER_SYMBOL, CSS_START_FREQ, CSS_END_FREQ, SAMPLE_RATE};
use std::f32::consts::PI;

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

    /// Modulate bits into CSS chirp signals with phase continuity
    /// Bit 1: up-chirp (start_freq → end_freq)
    /// Bit 0: down-chirp (end_freq → start_freq)
    /// Maintains phase continuity between consecutive symbols
    pub fn modulate(&self, bits: &[bool]) -> Result<Vec<f32>> {
        let mut samples = Vec::new();
        let mut accumulated_phase = 0.0;

        for &bit in bits {
            let chirp = self.generate_chirp_with_phase(
                self.symbol_samples,
                if bit { self.start_freq } else { self.end_freq },
                if bit { self.end_freq } else { self.start_freq },
                0.5,
                accumulated_phase,
            );

            // Track the final phase for the next symbol
            let duration = self.symbol_samples as f32 / SAMPLE_RATE as f32;
            let start_f = if bit { self.start_freq } else { self.end_freq };
            let end_f = if bit { self.end_freq } else { self.start_freq };
            let k = (end_f - start_f) / duration;
            accumulated_phase += 2.0 * PI * (start_f * duration + k * duration * duration / 2.0);
            accumulated_phase = accumulated_phase % (2.0 * PI);

            samples.extend_from_slice(&chirp);
        }

        Ok(samples)
    }

    /// Generate a chirp with optional starting phase for continuity
    fn generate_chirp_with_phase(
        &self,
        duration_samples: usize,
        start_freq: f32,
        end_freq: f32,
        amplitude: f32,
        start_phase: f32,
    ) -> Vec<f32> {
        let sample_rate = SAMPLE_RATE as f32;
        let duration = duration_samples as f32 / sample_rate;

        let mut samples = vec![0.0; duration_samples];
        for n in 0..duration_samples {
            let t = n as f32 / sample_rate;
            let k = (end_freq - start_freq) / duration;
            let phase = start_phase + 2.0 * PI * (start_freq * t + k * t * t / 2.0);
            samples[n] = amplitude * phase.sin();
        }
        samples
    }
}

impl Default for CssModulator {
    fn default() -> Self {
        Self::new().unwrap()
    }
}

/// Chirp Spread Spectrum (CSS) demodulator
/// Decodes chirp signals back to bits using FFT-based correlation
/// Implements sliding correlation for timing recovery and normalized correlation
pub struct CssDemodulator {
    symbol_samples: usize,
    up_chirp_template: Vec<f32>,
    down_chirp_template: Vec<f32>,
    template_energy: f32,
    max_timing_offset: usize,
    min_correlation_threshold: f32,
}

impl CssDemodulator {
    pub fn new() -> Result<Self> {
        let symbol_samples = CSS_SAMPLES_PER_SYMBOL;

        // Pre-generate chirp templates for correlation
        let up_chirp_template = generate_chirp(symbol_samples, CSS_START_FREQ, CSS_END_FREQ, 1.0);
        let down_chirp_template = generate_chirp(symbol_samples, CSS_END_FREQ, CSS_START_FREQ, 1.0);

        // Compute template energy for normalization
        let template_energy: f32 = up_chirp_template.iter().map(|x| x * x).sum();

        // Allow timing offset of ±5% of symbol duration (up to 40 samples for 800-sample symbols)
        let max_timing_offset = (symbol_samples as f32 * 0.05).ceil() as usize;

        Ok(Self {
            symbol_samples,
            up_chirp_template,
            down_chirp_template,
            template_energy,
            max_timing_offset,
            min_correlation_threshold: 0.3,
        })
    }

    /// Demodulate CSS chirp signals back to bits with timing recovery
    /// Uses sliding correlation to find the best alignment within ±max_timing_offset
    /// Normalizes correlation by template and window energy
    pub fn demodulate(&self, samples: &[f32]) -> Result<Vec<bool>> {
        if samples.len() < self.symbol_samples {
            return Err(AudioModemError::InsufficientData);
        }

        let mut bits = Vec::new();
        let mut pos = 0;

        // Build prefix-sum array of squared samples for efficient window energy computation
        let mut sq_prefix = vec![0.0; samples.len() + 1];
        for k in 0..samples.len() {
            sq_prefix[k + 1] = sq_prefix[k] + samples[k] * samples[k];
        }

        while pos + self.symbol_samples <= samples.len() {
            // Use sliding correlation with search window
            let search_start = if pos >= self.max_timing_offset {
                pos - self.max_timing_offset
            } else {
                0
            };
            let search_end = (pos + self.symbol_samples + self.max_timing_offset).min(samples.len());

            if search_end - search_start < self.symbol_samples {
                return Err(AudioModemError::InsufficientData);
            }

            let window = &samples[search_start..search_end];

            // Correlate with both templates using Full mode for lag detection
            let up_corr = match fft_correlate_1d(window, &self.up_chirp_template, Mode::Full) {
                Ok(corr) => corr,
                Err(_) => return Err(AudioModemError::InvalidFrameSize),
            };

            let down_corr = match fft_correlate_1d(window, &self.down_chirp_template, Mode::Full) {
                Ok(corr) => corr,
                Err(_) => return Err(AudioModemError::InvalidFrameSize),
            };

            // Find peak correlations with normalized coefficients
            let (up_peak_norm, up_lag) =
                self.find_peak_with_normalization(&up_corr, window, &sq_prefix, search_start);
            let (down_peak_norm, down_lag) =
                self.find_peak_with_normalization(&down_corr, window, &sq_prefix, search_start);

            // Check if either correlation is too low
            if up_peak_norm < self.min_correlation_threshold && down_peak_norm < self.min_correlation_threshold {
                return Err(AudioModemError::InvalidFrameSize);
            }

            // Decide bit based on which normalized correlation is stronger
            let bit = up_peak_norm > down_peak_norm;
            bits.push(bit);

            // Update position with timing recovery offset using correct lag mapping
            let best_lag = if bit { up_lag } else { down_lag };

            // Convert lag index to window start position
            // In Full mode, lag corresponds to where template[T-1] aligns with window[lag]
            // So the window start is at lag - (T-1)
            let window_start = best_lag.saturating_sub(self.symbol_samples - 1);

            // Skip if the window_start is invalid (should not happen due to prior validation)
            if window_start + self.symbol_samples > window.len() {
                return Err(AudioModemError::InvalidFrameSize);
            }

            // Compute absolute index of the best-aligned symbol start
            let actual_start = search_start + window_start;

            // Compute signed delta relative to current pos
            let delta = actual_start as isize - pos as isize;

            // Clamp delta to allowed timing offset range
            let clamped_delta = delta.max(-(self.max_timing_offset as isize))
                                      .min(self.max_timing_offset as isize);

            // Update pos with drift correction, then advance by one symbol
            pos = ((pos as isize + clamped_delta) as usize).saturating_add(self.symbol_samples);

            // Safety check: prevent runaway timing drift and break if we've consumed all data
            if pos > samples.len() {
                break;
            }
        }

        Ok(bits)
    }

    /// Find the peak correlation value with normalized coefficient
    /// Returns (normalized_peak, lag_index)
    fn find_peak_with_normalization(
        &self,
        correlation: &[f32],
        window: &[f32],
        sq_prefix: &[f32],
        search_start: usize,
    ) -> (f32, usize) {
        let mut best_norm = 0.0;
        let mut best_lag = 0;

        // Iterate through valid positions
        for lag in 0..correlation.len() {
            let raw_corr = correlation[lag].abs();

            // Calculate window position and energy
            let window_start = if lag >= self.symbol_samples - 1 {
                lag - (self.symbol_samples - 1)
            } else {
                0
            };
            let window_end = (window_start + self.symbol_samples).min(window.len());

            if window_end - window_start < self.symbol_samples {
                continue;
            }

            // Compute window energy using O(1) prefix-sum lookup
            let actual_start = search_start + window_start;
            let actual_end = search_start + window_end;
            let window_energy = sq_prefix[actual_end] - sq_prefix[actual_start];

            // Compute normalized correlation coefficient
            let denom = (window_energy * self.template_energy).sqrt();
            let normalized = if denom > 1e-10 {
                raw_corr / denom
            } else {
                0.0
            };

            if normalized > best_norm {
                best_norm = normalized;
                best_lag = lag;
            }
        }

        (best_norm, best_lag)
    }
}

impl Default for CssDemodulator {
    fn default() -> Self {
        Self::new().unwrap()
    }
}
