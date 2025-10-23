use crate::SAMPLE_RATE;
use std::f32::consts::PI;

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

    // STRICT: Use high threshold (0.4+) for accurate position detection
    // Only accept strong matches to ensure correct synchronization
    if best_correlation > 0.4 {
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

    // STRICT: Use high threshold (0.4+) for accurate position detection
    // Only accept strong matches to ensure correct synchronization
    if best_correlation > 0.4 {
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
}
