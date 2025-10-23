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

/// Detect preamble by correlating with expected chirp
pub fn detect_preamble(samples: &[f32], _min_peak_threshold: f32) -> Option<usize> {
    if samples.len() < 1000 {
        return None;
    }

    // Generate expected chirp
    let chirp = generate_chirp(1000, 200.0, 4000.0, 1.0);

    // Compute correlation with energy normalization
    let mut best_pos = 0;
    let mut best_ratio = 0.0;

    let window = 1000;
    for i in 0..=samples.len().saturating_sub(window) {
        let mut corr = 0.0;
        let mut sample_energy = 0.0;

        for j in 0..window {
            corr += samples[i + j] * chirp[j];
            sample_energy += samples[i + j] * samples[i + j];
        }
        corr = corr.abs();

        // Normalize correlation by sample energy
        let ratio = if sample_energy > 0.0 {
            corr / sample_energy.sqrt()
        } else {
            0.0
        };

        if ratio > best_ratio {
            best_ratio = ratio;
            best_pos = i;
        }
    }

    // Accept if we found a reasonable match (lower threshold to be more permissive)
    if best_ratio > 0.01 {
        Some(best_pos)
    } else {
        None
    }
}

/// Detect postamble by correlating with expected descending chirp
pub fn detect_postamble(samples: &[f32], _min_peak_threshold: f32) -> Option<usize> {
    if samples.len() < 800 {
        return None;
    }

    // Generate expected descending chirp (4000 Hz to 200 Hz)
    // Use 800 samples matching POSTAMBLE_SAMPLES (50ms at 16kHz)
    let chirp = generate_chirp(800, 4000.0, 200.0, 1.0);

    // Compute correlation with energy normalization
    let mut best_pos = 0;
    let mut best_ratio = 0.0;

    let window = 800;
    for i in 0..=samples.len().saturating_sub(window) {
        let mut corr = 0.0;
        let mut sample_energy = 0.0;

        for j in 0..window {
            corr += samples[i + j] * chirp[j];
            sample_energy += samples[i + j] * samples[i + j];
        }
        corr = corr.abs();

        // Normalize correlation by sample energy
        let ratio = if sample_energy > 0.0 {
            corr / sample_energy.sqrt()
        } else {
            0.0
        };

        if ratio > best_ratio {
            best_ratio = ratio;
            best_pos = i;
        }
    }

    // Accept if we found a reasonable match
    if best_ratio > 0.01 {
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
