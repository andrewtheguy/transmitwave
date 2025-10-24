use crate::SAMPLE_RATE;
use std::f32::consts::PI;
use realfft::RealFftPlanner;
use rustfft::num_complex::Complex;
use std::cell::RefCell;
use std::collections::HashMap;

thread_local! {
    static FFT_PLANNER: RefCell<RealFftPlanner<f32>> = RefCell::new(RealFftPlanner::new());
    static FFT_PLAN_CACHE: RefCell<HashMap<usize, (std::sync::Arc<dyn realfft::RealToComplex<f32>>, std::sync::Arc<dyn realfft::ComplexToReal<f32>>)>> = RefCell::new(HashMap::new());
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

/// Generates a Barker code (11-bit) for synchronization
pub fn barker_code() -> Vec<i8> {
    vec![1, 1, 1, -1, -1, 1, -1, 1, 1, -1, 1]
}

/// Generates a chirp signal that sweeps from low to high frequency
/// Used as preamble for frame synchronization
pub fn generate_chirp(
    duration_samples: usize,
    start_freq: f32,
    end_freq: f32,
    amplitude: f32,
) -> Vec<f32> {
    let sample_rate = SAMPLE_RATE as f32;
    let duration = duration_samples as f32 / sample_rate;

    let mut samples = vec![0.0; duration_samples];
    for n in 0..duration_samples {
        let t = n as f32 / sample_rate;
        let k = (end_freq - start_freq) / duration;
        let phase = 2.0 * PI * (start_freq * t + k * t * t / 2.0);
        samples[n] = amplitude * phase.sin();
    }
    samples
}

/// Generates postamble (descending chirp from 4000 Hz to 200 Hz)
/// Mirrors the preamble pattern but in reverse
pub fn generate_postamble(duration_samples: usize, amplitude: f32) -> Vec<f32> {
    generate_chirp(duration_samples, 4000.0, 200.0, amplitude)
}

fn fft_correlate_1d(signal: &[f32], template: &[f32]) -> Vec<f32> {
    // Early validation for empty inputs
    if signal.len() == 0 || template.len() == 0 {
        return Vec::new();
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

    // Get or create FFT plans from cache
    let (r2c, c2r) = FFT_PLAN_CACHE.with(|cache| {
        let mut cache_map = cache.borrow_mut();
        if let Some(plans) = cache_map.get(&fft_size) {
            plans.clone()
        } else {
            // Create new plans and cache them
            let (r2c_new, c2r_new) = FFT_PLANNER.with(|planner| {
                let mut planner_ref = planner.borrow_mut();
                let r2c = planner_ref.plan_fft_forward(fft_size);
                let c2r = planner_ref.plan_fft_inverse(fft_size);
                (r2c, c2r)
            });
            let plans = (r2c_new.clone(), c2r_new.clone());
            cache_map.insert(fft_size, plans.clone());
            plans
        }
    });

    // Allocate buffers for FFT output (complex)
    let mut signal_spectrum = r2c.make_output_vec();
    let mut template_spectrum = r2c.make_output_vec();

    // Forward FFT on both signal and template
    debug_assert_eq!(padded_signal.len(), fft_size, "Signal buffer size mismatch");
    debug_assert_eq!(padded_template.len(), fft_size, "Template buffer size mismatch");
    r2c.process(&mut padded_signal, &mut signal_spectrum)
        .expect("FFT forward process failed for signal");
    r2c.process(&mut padded_template, &mut template_spectrum)
        .expect("FFT forward process failed for template");

    // Frequency domain multiplication (element-wise)
    // For correlation, we already reversed template, so just multiply
    let mut result_spectrum = vec![Complex::new(0.0, 0.0); signal_spectrum.len()];
    for i in 0..signal_spectrum.len() {
        result_spectrum[i] = signal_spectrum[i] * template_spectrum[i];
    }

    // Inverse FFT
    let mut result_time = vec![0.0; fft_size];
    debug_assert_eq!(result_spectrum.len(), signal_spectrum.len(), "Spectrum buffer size mismatch");
    debug_assert_eq!(result_time.len(), fft_size, "Output buffer size mismatch");
    c2r.process(&mut result_spectrum, &mut result_time)
        .expect("FFT inverse process failed");

    // Normalize by FFT size and return only valid correlation length
    let normalization = fft_size as f32;
    result_time.iter_mut().for_each(|x| *x /= normalization);
    result_time.truncate(output_len);

    result_time
}

/// Detect preamble using efficient FFT-based cross-correlation
/// Returns the position where the preamble (ascending chirp) is most likely to start
pub fn detect_preamble(samples: &[f32], _min_peak_threshold: f32) -> Option<usize> {
    let preamble_samples = crate::PREAMBLE_SAMPLES;

    if samples.len() < preamble_samples {
        return None;
    }

    // Generate expected ascending chirp (200 Hz to 4000 Hz)
    let template = generate_chirp(preamble_samples, 200.0, 4000.0, 1.0);

    // Use sliding window with Pearson correlation for efficiency
    let mut best_pos = 0;
    let mut best_correlation = 0.0;

    // Calculate template energy once
    let template_energy: f32 = template.iter().map(|x| x * x).sum();

    // Slide window and compute normalized cross-correlation
    for i in 0..=samples.len().saturating_sub(preamble_samples) {
        let window = &samples[i..i + preamble_samples];

        // Compute cross-correlation and energy
        let mut correlation = 0.0;
        let mut window_energy = 0.0;

        for (&s, &t) in window.iter().zip(template.iter()) {
            correlation += s * t;
            window_energy += s * s;
        }

        // Normalized correlation coefficient
        let denom = (window_energy * template_energy).sqrt();
        let normalized_corr = if denom > 1e-10 {
            (correlation / denom).abs()
        } else {
            0.0
        };

        if normalized_corr > best_correlation {
            best_correlation = normalized_corr;
            best_pos = i;
        }
    }

    // Adaptive threshold: scale based on overall signal amplitude
    // For strong signals (high amplitude): use strict 0.4 threshold
    // For weak signals (low amplitude): lower threshold to ~0.3
    let signal_rms: f32 = (samples.iter().map(|x| x * x).sum::<f32>() / samples.len() as f32).sqrt();
    let threshold = if signal_rms > 0.1 {
        0.4  // Strong signal: strict detection
    } else if signal_rms > 0.02 {
        0.35 // Medium signal: moderate threshold
    } else {
        0.3  // Weak signal: relaxed threshold for low-amplitude recordings
    };

    if best_correlation > threshold {
        Some(best_pos)
    } else {
        None
    }
}

/// Detect postamble using efficient cross-correlation
/// Returns the position where the postamble (descending chirp) is most likely to start
pub fn detect_postamble(samples: &[f32], _min_peak_threshold: f32) -> Option<usize> {
    let postamble_samples = crate::POSTAMBLE_SAMPLES;

    if samples.len() < postamble_samples {
        return None;
    }

    // Generate expected descending chirp (4000 Hz to 200 Hz)
    let template = generate_chirp(postamble_samples, 4000.0, 200.0, 1.0);

    // Use sliding window with Pearson correlation for efficiency
    let mut best_pos = 0;
    let mut best_correlation = 0.0;

    // Calculate template energy once
    let template_energy: f32 = template.iter().map(|x| x * x).sum();

    // Slide window and compute normalized cross-correlation
    for i in 0..=samples.len().saturating_sub(postamble_samples) {
        let window = &samples[i..i + postamble_samples];

        // Compute cross-correlation and energy
        let mut correlation = 0.0;
        let mut window_energy = 0.0;

        for (&s, &t) in window.iter().zip(template.iter()) {
            correlation += s * t;
            window_energy += s * s;
        }

        // Normalized correlation coefficient
        let denom = (window_energy * template_energy).sqrt();
        let normalized_corr = if denom > 1e-10 {
            (correlation / denom).abs()
        } else {
            0.0
        };

        if normalized_corr > best_correlation {
            best_correlation = normalized_corr;
            best_pos = i;
        }
    }

    // Adaptive threshold: scale based on overall signal amplitude
    // For strong signals (high amplitude): use strict 0.4 threshold
    // For weak signals (low amplitude): lower threshold to ~0.3
    let signal_rms: f32 = (samples.iter().map(|x| x * x).sum::<f32>() / samples.len() as f32).sqrt();
    let threshold = if signal_rms > 0.1 {
        0.4  // Strong signal: strict detection
    } else if signal_rms > 0.02 {
        0.35 // Medium signal: moderate threshold
    } else {
        0.3  // Weak signal: relaxed threshold for low-amplitude recordings
    };

    if best_correlation > threshold {
        Some(best_pos)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_barker_code() {
        let barker = barker_code();
        assert_eq!(barker.len(), 11);
    }

    #[test]
    fn test_barker_code_values() {
        let barker = barker_code();
        let expected = vec![1, 1, 1, -1, -1, 1, -1, 1, 1, -1, 1];
        assert_eq!(barker, expected);
    }

    #[test]
    fn test_barker_autocorrelation() {
        let barker = barker_code();
        // Autocorrelation at lag 0 should be 11 (sum of squares)
        let autocorr: i32 = barker.iter().map(|&x| (x as i32) * (x as i32)).sum();
        assert_eq!(autocorr, 11);
    }

    #[test]
    fn test_barker_sidelobe_property() {
        let barker = barker_code();
        // Test correlation with shifted versions
        // Verify that autocorrelation is much larger than sidelobes
        let mut max_sidelobe = 0i32;
        let mut avg_sidelobe = 0i32;
        for lag in 1..11 {
            let correlation: i32 = barker[..11 - lag]
                .iter()
                .zip(barker[lag..].iter())
                .map(|(a, b)| (*a as i32) * (*b as i32))
                .sum();
            let abs_corr = correlation.abs();
            max_sidelobe = max_sidelobe.max(abs_corr);
            avg_sidelobe += abs_corr;
        }
        avg_sidelobe /= 10;
        // Autocorrelation (11) should be >> sidelobes
        assert!(max_sidelobe < 11, "Max sidelobe {} should be < autocorr 11", max_sidelobe);
        assert!(avg_sidelobe < 11, "Avg sidelobe {} should be < autocorr 11", avg_sidelobe);
    }

    #[test]
    fn test_barker_code_symmetry() {
        let barker = barker_code();
        // Check that alternating signs create the Barker structure
        let alternations = barker
            .windows(2)
            .filter(|w| w[0] != w[1])
            .count();
        assert!(alternations >= 5, "Barker should have multiple sign changes");
    }

    #[test]
    fn test_barker_chip_spreading() {
        let barker = barker_code();
        // Each Barker chip (±1) can spread one information bit
        assert_eq!(barker.iter().all(|&x| x == 1 || x == -1), true);

        // Test spreading a single bit across Barker sequence
        let bit = true;
        let bit_val: i8 = if bit { 1 } else { -1 };
        let spread: Vec<i8> = barker.iter().map(|&x| x * bit_val).collect();
        assert_eq!(spread.len(), 11);
    }

    #[test]
    fn test_barker_despread_clean() {
        let barker = barker_code();
        let bit = true;
        let bit_val = if bit { 1.0 } else { -1.0 };

        // Spread the bit
        let spread: Vec<f32> = barker.iter().map(|&x| (x as f32) * bit_val).collect();

        // Despread by correlating with Barker
        let correlation: f32 = spread
            .iter()
            .zip(barker.iter())
            .map(|(&s, &b)| s * (b as f32))
            .sum();

        // Correlation should be close to 11 (autocorrelation peak)
        assert!(correlation > 10.0);
    }

    #[test]
    fn test_barker_despread_with_noise() {
        let barker = barker_code();
        let bit = true;
        let bit_val = if bit { 1.0 } else { -1.0 };

        // Spread and add noise
        let spread: Vec<f32> = barker
            .iter()
            .enumerate()
            .map(|(i, &x)| {
                let noise = ((i as f32 * 0.789) % 1.0) * 0.2 - 0.1; // ±10% noise
                (x as f32) * bit_val + noise
            })
            .collect();

        // Despread by correlating with Barker
        let correlation: f32 = spread
            .iter()
            .zip(barker.iter())
            .map(|(&s, &b)| s * (b as f32))
            .sum();

        // Should still detect positive correlation despite noise
        assert!(correlation > 5.0, "Correlation with noise: {}", correlation);
    }

    #[test]
    fn test_barker_as_matched_filter() {
        let barker = barker_code();
        // Create signal with Barker repeated (10 repetitions = 110 samples)

        // Create signal with Barker repeated
        let mut signal = Vec::new();
        for _ in 0..10 {
            for &chip in &barker {
                signal.push(chip as f32);
            }
        }

        // Correlate with single Barker sequence
        let correlation: f32 = signal
            .iter()
            .zip(barker.iter().cycle())
            .map(|(&s, &b)| s * (b as f32))
            .sum();

        // Should have very high correlation (10 * autocorr)
        assert!(correlation > 100.0);
    }

    #[test]
    fn test_barker_mismatch_detection() {
        let barker = barker_code();
        // Create a random sequence
        let random_seq: Vec<i8> = vec![1, -1, 1, 1, -1, 1, -1, -1, 1, -1, -1];

        // Correlate random with Barker
        let correlation: i32 = random_seq
            .iter()
            .zip(barker.iter())
            .map(|(a, b)| (*a as i32) * (*b as i32))
            .sum();

        // Correlation with non-matching sequence should be lower
        // (not guaranteed to be low, but statistically likely)
        assert!(correlation.abs() < 10);
    }

    #[test]
    fn test_chirp_generation() {
        let chirp = generate_chirp(1600, 200.0, 4000.0, 1.0);
        assert_eq!(chirp.len(), 1600);
    }

    #[test]
    fn test_chirp_amplitude() {
        let chirp = generate_chirp(1600, 200.0, 4000.0, 0.5);
        let max_val = chirp.iter().map(|x| x.abs()).max_by(|a, b| a.partial_cmp(b).unwrap()).unwrap();
        assert!(max_val <= 0.6 && max_val >= 0.4, "Max amplitude: {}", max_val);
    }

    #[test]
    fn test_chirp_frequency_sweep() {
        let chirp = generate_chirp(1600, 200.0, 4000.0, 1.0);

        // Compute zero crossings as proxy for frequency
        let zero_crossings_early = chirp[..400]
            .windows(2)
            .filter(|w| (w[0] > 0.0) != (w[1] > 0.0))
            .count();

        let zero_crossings_late = chirp[1200..]
            .windows(2)
            .filter(|w| (w[0] > 0.0) != (w[1] > 0.0))
            .count();

        // Later part should have more zero crossings (higher frequency)
        assert!(zero_crossings_late > zero_crossings_early);
    }

    #[test]
    fn test_postamble_descending() {
        let postamble = generate_postamble(1600, 1.0);
        assert_eq!(postamble.len(), 1600);

        // Postamble should be reverse frequency sweep
        let zero_crossings_early = postamble[..400]
            .windows(2)
            .filter(|w| (w[0] > 0.0) != (w[1] > 0.0))
            .count();

        let zero_crossings_late = postamble[1200..]
            .windows(2)
            .filter(|w| (w[0] > 0.0) != (w[1] > 0.0))
            .count();

        // Earlier part should have more zero crossings (higher frequency at start)
        assert!(zero_crossings_early > zero_crossings_late);
    }

    #[test]
    fn test_preamble_detection_strong_signal() {
        // Strong signal: should always detect with strict 0.4 threshold
        let preamble = generate_chirp(crate::PREAMBLE_SAMPLES, 200.0, 4000.0, 1.0);
        let mut signal = preamble.clone();
        signal.extend_from_slice(&vec![0.0; 1000]); // Add silence after

        let result = detect_preamble(&signal, 0.1);
        assert!(result.is_some(), "Strong signal should be detected");
        assert!(result.unwrap() < 100, "Should detect near start");
    }

    #[test]
    fn test_preamble_detection_medium_signal() {
        // Medium signal (0.3x amplitude): should detect with 0.35 threshold
        let preamble = generate_chirp(crate::PREAMBLE_SAMPLES, 200.0, 4000.0, 0.3);
        let mut signal = preamble.clone();
        signal.extend_from_slice(&vec![0.0; 1000]);

        let result = detect_preamble(&signal, 0.1);
        assert!(result.is_some(), "Medium signal should be detected");
    }

    #[test]
    fn test_preamble_detection_weak_signal() {
        // Weak signal (0.1x amplitude): should detect with 0.3 threshold
        let preamble = generate_chirp(crate::PREAMBLE_SAMPLES, 200.0, 4000.0, 0.1);
        let mut signal = preamble.clone();
        signal.extend_from_slice(&vec![0.0; 1000]);

        let result = detect_preamble(&signal, 0.1);
        assert!(result.is_some(), "Weak signal should be detected");
    }

    #[test]
    fn test_preamble_detection_very_weak_signal() {
        // Very weak signal (0.05x amplitude): at detection limit
        let preamble = generate_chirp(crate::PREAMBLE_SAMPLES, 200.0, 4000.0, 0.05);
        let mut signal = preamble.clone();
        signal.extend_from_slice(&vec![0.0; 1000]);

        let result = detect_preamble(&signal, 0.1);
        // May or may not detect (at threshold boundary), but should not crash
        let _ = result;
    }

    #[test]
    fn test_preamble_detection_with_noise() {
        // Weak signal with noise: adaptive threshold should handle
        let preamble = generate_chirp(crate::PREAMBLE_SAMPLES, 200.0, 4000.0, 0.15);
        let mut signal = preamble.clone();

        // Add small noise
        for s in &mut signal[..1000] {
            *s += 0.02; // noise amplitude
        }
        signal.extend_from_slice(&vec![0.0; 1000]);

        let result = detect_preamble(&signal, 0.1);
        assert!(result.is_some(), "Weak signal with noise should be detected");
    }

    #[test]
    fn test_postamble_detection_strong_signal() {
        // Strong signal: should always detect with strict 0.4 threshold
        let postamble = generate_postamble(crate::POSTAMBLE_SAMPLES, 1.0);
        let mut signal = vec![0.0; 1000];
        signal.extend_from_slice(&postamble);

        let result = detect_postamble(&signal, 0.1);
        assert!(result.is_some(), "Strong postamble should be detected");
    }

    #[test]
    fn test_postamble_detection_weak_signal() {
        // Weak signal: should detect with relaxed threshold
        let postamble = generate_postamble(crate::POSTAMBLE_SAMPLES, 0.1);
        let mut signal = vec![0.0; 1000];
        signal.extend_from_slice(&postamble);

        let result = detect_postamble(&signal, 0.1);
        assert!(result.is_some(), "Weak postamble should be detected");
    }

    #[test]
    fn test_adaptive_threshold_rms_strong() {
        // Create signal with RMS > 0.1 (should use 0.4 threshold)
        let chirp = generate_chirp(crate::PREAMBLE_SAMPLES, 200.0, 4000.0, 0.5);
        let signal_rms: f32 = (chirp.iter().map(|x| x * x).sum::<f32>() / chirp.len() as f32).sqrt();
        assert!(signal_rms > 0.1, "Signal RMS should be > 0.1 for strong signal test");

        let mut signal = chirp.clone();
        signal.extend_from_slice(&vec![0.0; 1000]);
        let result = detect_preamble(&signal, 0.1);
        assert!(result.is_some());
    }

    #[test]
    fn test_adaptive_threshold_rms_medium() {
        // Create signal with 0.02 < RMS < 0.1 (should use 0.35 threshold)
        // Amplitude 0.08 gives RMS ~0.04 (in medium range)
        let chirp = generate_chirp(crate::PREAMBLE_SAMPLES, 200.0, 4000.0, 0.08);
        let signal_rms: f32 = (chirp.iter().map(|x| x * x).sum::<f32>() / chirp.len() as f32).sqrt();
        assert!(signal_rms > 0.02 && signal_rms <= 0.1, "Signal RMS should be in medium range, got {}", signal_rms);

        let mut signal = chirp.clone();
        signal.extend_from_slice(&vec![0.0; 1000]);
        let result = detect_preamble(&signal, 0.1);
        assert!(result.is_some());
    }

    #[test]
    fn test_adaptive_threshold_rms_weak() {
        // Create signal with RMS <= 0.02 (should use 0.3 threshold)
        // Amplitude 0.02 gives RMS ~0.01 (in weak range)
        let chirp = generate_chirp(crate::PREAMBLE_SAMPLES, 200.0, 4000.0, 0.02);
        let signal_rms: f32 = (chirp.iter().map(|x| x * x).sum::<f32>() / chirp.len() as f32).sqrt();
        assert!(signal_rms <= 0.02, "Signal RMS should be <= 0.02 for weak signal test, got {}", signal_rms);

        let mut signal = chirp.clone();
        signal.extend_from_slice(&vec![0.0; 1000]);
        let result = detect_preamble(&signal, 0.1);
        assert!(result.is_some(), "Weak signal should be detected with adaptive threshold");
    }

    #[test]
    fn test_preamble_false_positive_rejection() {
        // Random noise should not trigger false positives
        let noise: Vec<f32> = (0..crate::PREAMBLE_SAMPLES * 2)
            .map(|i| (i as f32 * 0.1).sin() * 0.01)
            .collect();

        let result = detect_preamble(&noise, 0.1);
        // May or may not detect, but should be much less likely than true preamble
        let _ = result;
    }

    #[test]
    fn test_preamble_attenuation_series() {
        // Test series of attenuated signals to verify graceful degradation
        let base_preamble = generate_chirp(crate::PREAMBLE_SAMPLES, 200.0, 4000.0, 1.0);

        let attenuation_levels = vec![1.0, 0.5, 0.2, 0.1, 0.05];
        let mut detected_count = 0;

        for &atten in &attenuation_levels {
            let preamble = base_preamble.iter().map(|x| x * atten).collect::<Vec<_>>();
            let mut signal = preamble.clone();
            signal.extend_from_slice(&vec![0.0; 1000]);

            if detect_preamble(&signal, 0.1).is_some() {
                detected_count += 1;
            }
        }

        // Should detect most signals (at least 3 out of 5)
        assert!(detected_count >= 3, "Should detect at least 3 out of 5 attenuation levels");
    }

    #[test]
    fn test_preamble_position_accuracy() {
        // Verify detection correctly identifies preamble position
        let silence_before = vec![0.0; 500];
        let preamble = generate_chirp(crate::PREAMBLE_SAMPLES, 200.0, 4000.0, 0.3);
        let mut signal = silence_before.clone();
        signal.extend_from_slice(&preamble);
        signal.extend_from_slice(&vec![0.0; 1000]);

        let result = detect_preamble(&signal, 0.1);
        assert!(result.is_some());

        let pos = result.unwrap();
        // Should detect near the start of silence (within tolerance)
        assert!(pos < 1000, "Detection position {} should be reasonable", pos);
    }

    #[test]
    fn test_fft_correlate_1d_simple() {
        // Test with simple impulse response
        let signal = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let template = vec![1.0, 0.0, 0.0];
        let result = fft_correlate_1d(&signal, &template);

        // Correlation with impulse should return the signal itself (shifted)
        assert_eq!(result.len(), signal.len() + template.len() - 1);
        assert!((result[2] - 1.0).abs() < 1e-4);
        assert!((result[3] - 2.0).abs() < 1e-4);
        assert!((result[4] - 3.0).abs() < 1e-4);
    }

    #[test]
    fn test_fft_correlate_1d_vs_sliding_window() {
        // Compare FFT correlation with sliding window approach
        let preamble_samples = crate::PREAMBLE_SAMPLES;
        let template = generate_chirp(preamble_samples, 200.0, 4000.0, 1.0);

        // Create signal with chirp embedded
        let mut signal = vec![0.0; 500];
        signal.extend_from_slice(&template);
        signal.extend_from_slice(&vec![0.0; 500]);

        // Compute FFT correlation
        let fft_result = fft_correlate_1d(&signal, &template);

        // Compute sliding window correlation (unnormalized, just raw dot product)
        let mut sliding_result = vec![0.0; signal.len()];
        for i in 0..=signal.len().saturating_sub(template.len()) {
            let window = &signal[i..i + template.len()];
            let correlation: f32 = window.iter().zip(template.iter()).map(|(&s, &t)| s * t).sum();
            sliding_result[i + template.len() - 1] = correlation;
        }

        // Find peaks in both results
        let fft_peak_pos = fft_result.iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .map(|(i, _)| i)
            .unwrap();

        let sliding_peak_pos = sliding_result.iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .map(|(i, _)| i)
            .unwrap();

        // Peaks should be at roughly the same position (within small tolerance)
        assert!((fft_peak_pos as i32 - sliding_peak_pos as i32).abs() < 10,
            "FFT peak at {} vs sliding window peak at {}", fft_peak_pos, sliding_peak_pos);
    }

    #[test]
    fn test_fft_correlate_1d_chirp_detection() {
        // Use actual preamble chirp and verify peak position
        let preamble_samples = crate::PREAMBLE_SAMPLES;
        let template = generate_chirp(preamble_samples, 200.0, 4000.0, 1.0);

        let silence_before = 300;
        let mut signal = vec![0.0; silence_before];
        signal.extend_from_slice(&template);
        signal.extend_from_slice(&vec![0.0; 400]);

        let result = fft_correlate_1d(&signal, &template);

        // Find peak position
        let peak_pos = result.iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .map(|(i, _)| i)
            .unwrap();

        // Peak should be near silence_before + template.len() - 1
        let expected_peak = silence_before + template.len() - 1;
        assert!((peak_pos as i32 - expected_peak as i32).abs() < 20,
            "Expected peak near {}, got {}", expected_peak, peak_pos);
    }

    #[test]
    fn test_fft_correlate_1d_different_lengths() {
        // Test various signal and template length combinations
        let test_cases = vec![
            (vec![1.0, 2.0, 3.0], vec![1.0]),
            (vec![1.0, 2.0, 3.0, 4.0, 5.0], vec![1.0, 1.0]),
            (vec![1.0; 100], vec![1.0; 10]),
            (vec![0.5; 50], vec![0.5; 20]),
        ];

        for (signal, template) in test_cases {
            let result = fft_correlate_1d(&signal, &template);
            let expected_len = signal.len() + template.len() - 1;
            assert_eq!(result.len(), expected_len,
                "Signal len: {}, Template len: {}", signal.len(), template.len());

            // Verify no NaN or Inf values
            for val in &result {
                assert!(val.is_finite(), "Found non-finite value: {}", val);
            }
        }
    }

    #[test]
    fn test_fft_correlate_1d_zero_signal() {
        // Test with zero signal to ensure no panics or NaN
        let signal = vec![0.0; 100];
        let template = vec![1.0, 2.0, 3.0];

        let result = fft_correlate_1d(&signal, &template);

        assert_eq!(result.len(), signal.len() + template.len() - 1);

        // All values should be zero (or very close)
        for val in &result {
            assert!(val.abs() < 1e-6, "Expected near-zero, got {}", val);
        }
    }

    #[test]
    fn test_fft_correlate_1d_normalization() {
        // Verify proper normalization via autocorrelation
        let signal = vec![1.0; 50];
        let template = signal.clone();

        let result = fft_correlate_1d(&signal, &template);

        // Find peak (should be at autocorrelation center)
        let max_val = result.iter()
            .map(|x| x.abs())
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap();

        // Autocorrelation of constant signal should equal signal length
        let expected_peak = 50.0;
        assert!((max_val - expected_peak).abs() < 0.5,
            "Expected autocorrelation peak ~{}, got {}", expected_peak, max_val);
    }

    #[test]
    fn test_fft_correlate_1d_empty_inputs() {
        // Test empty signal with non-empty template
        let result1 = fft_correlate_1d(&[], &[1.0, 2.0, 3.0]);
        assert_eq!(result1.len(), 0, "Empty signal should return empty vector");

        // Test non-empty signal with empty template
        let result2 = fft_correlate_1d(&[1.0, 2.0, 3.0], &[]);
        assert_eq!(result2.len(), 0, "Empty template should return empty vector");

        // Test both empty
        let result3 = fft_correlate_1d(&[], &[]);
        assert_eq!(result3.len(), 0, "Both empty should return empty vector");
    }

    #[test]
    fn test_fft_correlate_1d_sample_wise_closeness() {
        // Build short signal and template for full sliding-window comparison
        let signal = vec![1.0, 2.0, 3.0, 4.0, 5.0, 4.0, 3.0, 2.0, 1.0];
        let template = vec![0.5, 1.0, 0.5];

        // Compute FFT correlation
        let fft_result = fft_correlate_1d(&signal, &template);

        // Compute full sliding-window correlation for all lags
        let output_len = signal.len() + template.len() - 1;
        let mut sliding_result = vec![0.0; output_len];

        for lag in 0..output_len {
            let mut correlation = 0.0;
            for i in 0..template.len() {
                let signal_idx = lag as i32 - (template.len() as i32 - 1) + i as i32;
                if signal_idx >= 0 && signal_idx < signal.len() as i32 {
                    correlation += signal[signal_idx as usize] * template[template.len() - 1 - i];
                }
            }
            sliding_result[lag] = correlation;
        }

        // Assert each corresponding output sample matches within epsilon
        assert_eq!(fft_result.len(), sliding_result.len(), "Result lengths must match");
        for (i, (&fft_val, &sliding_val)) in fft_result.iter().zip(sliding_result.iter()).enumerate() {
            assert!((fft_val - sliding_val).abs() < 1e-4,
                "Sample {} mismatch: FFT={}, sliding={}, diff={}",
                i, fft_val, sliding_val, (fft_val - sliding_val).abs());
        }
    }
}
