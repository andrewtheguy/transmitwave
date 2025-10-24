//! FFT-based correlation for 1D real-valued signals
//!
//! Provides efficient cross-correlation using FFT with configurable output modes (Full, Same, Valid)
//! matching scipy/numpy conventions. Uses thread-local FFT planner caching for optimal performance.

use std::cell::RefCell;
use realfft::RealFftPlanner;
use crate::error::{AudioModemError, Result};

thread_local! {
    static FFT_PLANNER: RefCell<RealFftPlanner<f32>> = RefCell::new(RealFftPlanner::new());
}

/// Output mode for correlation
///
/// Determines the size of the correlation output:
/// - `Full`: Complete correlation (length = signal.len() + template.len() - 1)
/// - `Same`: Centered output matching signal size (length = signal.len())
/// - `Valid`: Only fully-overlapping region (length = signal.len() - template.len() + 1)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Full correlation output (signal.len() + template.len() - 1 samples)
    Full,
    /// Centered output matching first input size (signal.len() samples)
    Same,
    /// Only fully-overlapping region (signal.len() - template.len() + 1 samples)
    Valid,
}

fn next_power_of_two(n: usize) -> usize {
    if n == 0 {
        return 1;
    }
    let mut power = 1;
    while power < n {
        power <<= 1;
    }
    power
}

/// Correlate two 1D signals using FFT
///
/// Computes cross-correlation efficiently using FFT with O(N log N) complexity.
/// The `mode` parameter controls output size:
/// - `Mode::Full`: Returns complete correlation (signal.len() + template.len() - 1)
/// - `Mode::Same`: Returns centered output matching signal.len()
/// - `Mode::Valid`: Returns only fully-overlapping region (signal.len() - template.len() + 1)
///
/// Returns an empty vector if either input is empty or if Valid mode is used
/// with signal shorter than template.
///
/// # Errors
///
/// Returns `AudioModemError::FftError` if FFT processing fails.
pub fn fft_correlate_1d(signal: &[f32], template: &[f32], mode: Mode) -> Result<Vec<f32>> {
    // Early validation for empty inputs
    if signal.is_empty() || template.is_empty() {
        return Ok(Vec::new());
    }

    let output_len = signal.len() + template.len() - 1;
    let fft_size = next_power_of_two(output_len);

    // Zero-pad both signal and template to fft_size
    let mut padded_signal = vec![0.0; fft_size];
    let mut padded_template = vec![0.0; fft_size];
    padded_signal[..signal.len()].copy_from_slice(signal);

    // Reverse template for correlation (equivalent to conjugation in frequency domain)
    for (i, &val) in template.iter().enumerate().rev() {
        padded_template[template.len() - 1 - i] = val;
    }

    // Get FFT plans from planner (RealFftPlanner has internal caching)
    let (r2c, c2r) = FFT_PLANNER.with(|planner| {
        let mut planner_ref = planner.borrow_mut();
        let r2c = planner_ref.plan_fft_forward(fft_size);
        let c2r = planner_ref.plan_fft_inverse(fft_size);
        (r2c, c2r)
    });

    // Allocate buffers for FFT output (complex)
    let mut signal_spectrum = r2c.make_output_vec();
    let mut template_spectrum = r2c.make_output_vec();

    // Forward FFT on both signal and template
    debug_assert_eq!(padded_signal.len(), fft_size, "Signal buffer size mismatch");
    debug_assert_eq!(padded_template.len(), fft_size, "Template buffer size mismatch");
    r2c.process(&mut padded_signal, &mut signal_spectrum)
        .map_err(|e| AudioModemError::FftError(format!("FFT forward process failed for signal: {:?}", e)))?;
    r2c.process(&mut padded_template, &mut template_spectrum)
        .map_err(|e| AudioModemError::FftError(format!("FFT forward process failed for template: {:?}", e)))?;

    // Frequency domain multiplication (element-wise)
    // For correlation, we already reversed template, so just multiply in-place
    for i in 0..signal_spectrum.len() {
        signal_spectrum[i] *= template_spectrum[i];
    }

    // Inverse FFT
    let mut result_time = vec![0.0; fft_size];
    debug_assert_eq!(signal_spectrum.len(), r2c.make_output_vec().len(), "Spectrum buffer size mismatch");
    debug_assert_eq!(result_time.len(), fft_size, "Output buffer size mismatch");
    c2r.process(&mut signal_spectrum, &mut result_time)
        .map_err(|e| AudioModemError::FftError(format!("FFT inverse process failed: {:?}", e)))?;

    // Normalize by FFT size
    let normalization = fft_size as f32;
    result_time.iter_mut().for_each(|x| *x /= normalization);

    // Mode-based output trimming
    match mode {
        Mode::Full => {
            result_time.truncate(output_len);
            Ok(result_time)
        }
        Mode::Same => {
            let start = (output_len - signal.len()) / 2;
            Ok(result_time[start..start + signal.len()].to_vec())
        }
        Mode::Valid => {
            if signal.len() < template.len() {
                return Ok(Vec::new());
            }
            let valid_len = signal.len() - template.len() + 1;
            let start = template.len() - 1;
            Ok(result_time[start..start + valid_len].to_vec())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_next_power_of_two() {
        assert_eq!(next_power_of_two(0), 1);
        assert_eq!(next_power_of_two(1), 1);
        assert_eq!(next_power_of_two(2), 2);
        assert_eq!(next_power_of_two(3), 4);
        assert_eq!(next_power_of_two(7), 8);
        assert_eq!(next_power_of_two(8), 8);
        assert_eq!(next_power_of_two(9), 16);
        assert_eq!(next_power_of_two(1000), 1024);
    }

    #[test]
    fn test_fft_correlate_mode_full_length() {
        let signal = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let template = vec![1.0, 0.0, 0.0];
        let result = fft_correlate_1d(&signal, &template, Mode::Full).unwrap();
        assert_eq!(result.len(), signal.len() + template.len() - 1);

        let signal = vec![1.0; 100];
        let template = vec![1.0; 10];
        let result = fft_correlate_1d(&signal, &template, Mode::Full).unwrap();
        assert_eq!(result.len(), 109);
    }

    #[test]
    fn test_fft_correlate_mode_same_length() {
        let signal = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let template = vec![1.0, 0.0, 0.0];
        let result = fft_correlate_1d(&signal, &template, Mode::Same).unwrap();
        assert_eq!(result.len(), signal.len());

        let signal = vec![1.0; 100];
        let template = vec![1.0; 10];
        let result = fft_correlate_1d(&signal, &template, Mode::Same).unwrap();
        assert_eq!(result.len(), 100);
    }

    #[test]
    fn test_fft_correlate_mode_valid_length() {
        let signal = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let template = vec![1.0, 0.0, 0.0];
        let result = fft_correlate_1d(&signal, &template, Mode::Valid).unwrap();
        assert_eq!(result.len(), signal.len() - template.len() + 1);

        let signal = vec![1.0; 100];
        let template = vec![1.0; 10];
        let result = fft_correlate_1d(&signal, &template, Mode::Valid).unwrap();
        assert_eq!(result.len(), 91);

        // Template longer than signal
        let signal = vec![1.0, 2.0];
        let template = vec![1.0; 10];
        let result = fft_correlate_1d(&signal, &template, Mode::Valid).unwrap();
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_fft_correlate_mode_full_impulse() {
        let signal = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let template = vec![1.0, 0.0, 0.0];
        let result = fft_correlate_1d(&signal, &template, Mode::Full).unwrap();

        // Correlation with impulse at position 0 should return signal shifted
        assert_eq!(result.len(), 7);
        assert!((result[2] - 1.0).abs() < 1e-4);
        assert!((result[3] - 2.0).abs() < 1e-4);
        assert!((result[4] - 3.0).abs() < 1e-4);
        assert!((result[5] - 4.0).abs() < 1e-4);
        assert!((result[6] - 5.0).abs() < 1e-4);
    }

    #[test]
    fn test_fft_correlate_mode_same_centering() {
        let signal = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let template = vec![1.0, 0.0, 0.0];

        let full_result = fft_correlate_1d(&signal, &template, Mode::Full).unwrap();
        let same_result = fft_correlate_1d(&signal, &template, Mode::Same).unwrap();

        // Same mode should be centered slice of Full mode
        let output_len = full_result.len();
        let start = (output_len - signal.len()) / 2;
        let expected_same = &full_result[start..start + signal.len()];

        assert_eq!(same_result.len(), expected_same.len());
        for (a, b) in same_result.iter().zip(expected_same.iter()) {
            assert!((a - b).abs() < 1e-4);
        }
    }

    #[test]
    fn test_fft_correlate_mode_valid_no_edges() {
        let signal = vec![0.0, 0.0, 1.0, 2.0, 3.0, 0.0, 0.0];
        let template = vec![1.0, 1.0, 1.0];

        let valid_result = fft_correlate_1d(&signal, &template, Mode::Valid).unwrap();

        // Valid mode should have length 5 for this case
        assert_eq!(valid_result.len(), 5);

        // Check that peak is in the center where template fully overlaps with signal
        let max_idx = valid_result.iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .map(|(i, _)| i)
            .unwrap();

        assert!(max_idx >= 1 && max_idx <= 3);
    }

    #[test]
    fn test_fft_correlate_modes_consistency() {
        let signal = vec![1.0, 2.0, 3.0, 4.0, 5.0, 4.0, 3.0, 2.0, 1.0];
        let template = vec![0.5, 1.0, 0.5];

        let full = fft_correlate_1d(&signal, &template, Mode::Full).unwrap();
        let same = fft_correlate_1d(&signal, &template, Mode::Same).unwrap();
        let valid = fft_correlate_1d(&signal, &template, Mode::Valid).unwrap();

        // Check Same is centered slice of Full
        let output_len = full.len();
        let start = (output_len - signal.len()) / 2;
        for (i, &val) in same.iter().enumerate() {
            assert!((val - full[start + i]).abs() < 1e-4);
        }

        // Check Valid is appropriate slice of Full
        let valid_start = template.len() - 1;
        for (i, &val) in valid.iter().enumerate() {
            assert!((val - full[valid_start + i]).abs() < 1e-4);
        }
    }

    #[test]
    fn test_fft_correlate_vs_sliding_window() {
        // Naive sliding window correlation for comparison
        fn sliding_window_correlate(signal: &[f32], template: &[f32]) -> Vec<f32> {
            let output_len = signal.len() + template.len() - 1;
            let mut result = vec![0.0; output_len];

            for lag in 0..output_len {
                let mut correlation = 0.0;
                for i in 0..template.len() {
                    let signal_idx = lag as i32 - (template.len() as i32 - 1) + i as i32;
                    if signal_idx >= 0 && signal_idx < signal.len() as i32 {
                        correlation += signal[signal_idx as usize] * template[template.len() - 1 - i];
                    }
                }
                result[lag] = correlation;
            }
            result
        }

        let signal = vec![1.0, 2.0, 3.0, 4.0, 5.0, 4.0, 3.0, 2.0, 1.0];
        let template = vec![0.5, 1.0, 0.5];

        let fft_result = fft_correlate_1d(&signal, &template, Mode::Full).unwrap();
        let sliding_result = sliding_window_correlate(&signal, &template);

        assert_eq!(fft_result.len(), sliding_result.len());
        for (i, (&fft_val, &sliding_val)) in fft_result.iter().zip(sliding_result.iter()).enumerate() {
            assert!((fft_val - sliding_val).abs() < 1e-4,
                "Sample {} mismatch: FFT={}, sliding={}", i, fft_val, sliding_val);
        }
    }

    #[test]
    fn test_fft_correlate_chirp_signals() {
        use std::f32::consts::PI;

        // Generate chirp from 200-4000 Hz
        fn generate_chirp(samples: usize, f_start: f32, f_end: f32) -> Vec<f32> {
            let sample_rate = 16000.0;
            let duration = samples as f32 / sample_rate;
            let mut signal = vec![0.0; samples];
            for n in 0..samples {
                let t = n as f32 / sample_rate;
                let k = (f_end - f_start) / duration;
                let phase = 2.0 * PI * (f_start * t + k * t * t / 2.0);
                signal[n] = phase.sin();
            }
            signal
        }

        let template = generate_chirp(1600, 200.0, 4000.0);
        let mut signal = vec![0.0; 500];
        signal.extend_from_slice(&template);
        signal.extend_from_slice(&vec![0.0; 500]);

        // Test all three modes
        let full = fft_correlate_1d(&signal, &template, Mode::Full).unwrap();
        let same = fft_correlate_1d(&signal, &template, Mode::Same).unwrap();
        let valid = fft_correlate_1d(&signal, &template, Mode::Valid).unwrap();

        // All should find peak
        let full_peak = full.iter().map(|x| x.abs()).max_by(|a, b| a.partial_cmp(b).unwrap()).unwrap();
        let same_peak = same.iter().map(|x| x.abs()).max_by(|a, b| a.partial_cmp(b).unwrap()).unwrap();
        let valid_peak = valid.iter().map(|x| x.abs()).max_by(|a, b| a.partial_cmp(b).unwrap()).unwrap();

        assert!(full_peak > 100.0);
        assert!(same_peak > 100.0);
        assert!(valid_peak > 100.0);
    }

    #[test]
    fn test_fft_correlate_empty_inputs() {
        let result1 = fft_correlate_1d(&[], &[1.0, 2.0, 3.0], Mode::Full).unwrap();
        assert_eq!(result1.len(), 0);

        let result2 = fft_correlate_1d(&[1.0, 2.0, 3.0], &[], Mode::Full).unwrap();
        assert_eq!(result2.len(), 0);

        let result3 = fft_correlate_1d(&[], &[], Mode::Full).unwrap();
        assert_eq!(result3.len(), 0);
    }

    #[test]
    fn test_fft_correlate_single_element() {
        let signal = vec![5.0];
        let template = vec![2.0];

        let full = fft_correlate_1d(&signal, &template, Mode::Full).unwrap();
        assert_eq!(full.len(), 1);
        assert!((full[0] - 10.0).abs() < 1e-4);

        let same = fft_correlate_1d(&signal, &template, Mode::Same).unwrap();
        assert_eq!(same.len(), 1);
        assert!((same[0] - 10.0).abs() < 1e-4);

        let valid = fft_correlate_1d(&signal, &template, Mode::Valid).unwrap();
        assert_eq!(valid.len(), 1);
        assert!((valid[0] - 10.0).abs() < 1e-4);
    }

    #[test]
    fn test_fft_correlate_equal_length() {
        let signal = vec![1.0, 2.0, 3.0];
        let template = vec![0.5, 1.0, 1.5];

        let full = fft_correlate_1d(&signal, &template, Mode::Full).unwrap();
        assert_eq!(full.len(), 5);

        let same = fft_correlate_1d(&signal, &template, Mode::Same).unwrap();
        assert_eq!(same.len(), 3);

        let valid = fft_correlate_1d(&signal, &template, Mode::Valid).unwrap();
        assert_eq!(valid.len(), 1);
    }

    #[test]
    fn test_fft_correlate_template_longer() {
        let signal = vec![1.0, 2.0];
        let template = vec![1.0; 10];

        let full = fft_correlate_1d(&signal, &template, Mode::Full).unwrap();
        assert_eq!(full.len(), 11);

        let same = fft_correlate_1d(&signal, &template, Mode::Same).unwrap();
        assert_eq!(same.len(), 2);

        let valid = fft_correlate_1d(&signal, &template, Mode::Valid).unwrap();
        assert_eq!(valid.len(), 0);
    }

    #[test]
    fn test_fft_correlate_normalization() {
        // Autocorrelation test
        let signal = vec![1.0; 50];
        let template = signal.clone();

        let result = fft_correlate_1d(&signal, &template, Mode::Full).unwrap();

        let max_val = result.iter()
            .map(|x| x.abs())
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap();

        // Autocorrelation peak should equal signal length
        assert!((max_val - 50.0).abs() < 0.5);
    }
}
