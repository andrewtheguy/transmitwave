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
    fn test_chirp_generation() {
        let chirp = generate_chirp(1600, 200.0, 4000.0, 1.0);
        assert_eq!(chirp.len(), 1600);
    }
}
